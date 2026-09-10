//! JAR scanning without extraction.
//!
//! Reads the zip central directory of a Java archive to recover loader and
//! mod metadata: `META-INF/MANIFEST.MF` (`Main-Class`), `fabric.mod.json`
//! (`id`), `META-INF/mods.toml` (`modId`) and the `.class` entry count.

use std::io::Read as _;
use std::path::Path;

/// Errors from JAR scanning.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Filesystem failure with the path that caused it.
    #[error("io error for {path}: {source}")]
    Io {
        /// The path involved in the failed operation.
        path: std::path::PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// Zip container failure.
    #[error("zip error: {0}")]
    Zip(String),
}

/// Result alias for JAR scanning.
pub type Result<T> = std::result::Result<T, Error>;

/// Summary of a scanned Java archive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JarInfo {
    /// `Main-Class` from `META-INF/MANIFEST.MF`, when present.
    pub manifest_main_class: Option<String>,
    /// `id` from `fabric.mod.json`, when present.
    pub fabric_mod_id: Option<String>,
    /// `modId` from `META-INF/mods.toml`, when present.
    pub forge_mod_id: Option<String>,
    /// Number of `.class` entries in the archive.
    pub class_count: usize,
    /// Whether the manifest main class looks like a loader installer.
    pub is_loader_installer: bool,
}

/// Scan JAR bytes without extracting.
///
/// Finds `META-INF/MANIFEST.MF` (`Main-Class`), `fabric.mod.json` (`id`),
/// `META-INF/mods.toml` (`modId`) and counts `.class` entries. Missing
/// metadata files yield `None`; only a corrupt zip is an error.
pub fn scan_jar(jar_bytes: impl AsRef<[u8]>) -> Result<JarInfo> {
    let data = jar_bytes.as_ref();
    let cursor = std::io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| Error::Zip(e.to_string()))?;
    let mut info = JarInfo::default();
    let mut manifest_text: Option<String> = None;
    let mut fabric_text: Option<String> = None;
    let mut forge_text: Option<String> = None;

    let len = archive.len();
    let mut index: usize = 0;
    while index < len {
        let mut file = archive.by_index(index).map_err(|e| Error::Zip(e.to_string()))?;
        let name = file.name().to_owned();
        if name.ends_with(".class") && !file.is_dir() {
            info.class_count += 1;
        }
        if name == "META-INF/MANIFEST.MF" && manifest_text.is_none() {
            let mut text = String::new();
            if file.read_to_string(&mut text).is_ok() {
                manifest_text = Some(text);
            }
        } else if name == "fabric.mod.json" && fabric_text.is_none() {
            let mut text = String::new();
            if file.read_to_string(&mut text).is_ok() {
                fabric_text = Some(text);
            }
        } else if name == "META-INF/mods.toml" && forge_text.is_none() {
            let mut text = String::new();
            if file.read_to_string(&mut text).is_ok() {
                forge_text = Some(text);
            }
        }
        index += 1;
    }

    if let Some(text) = manifest_text.as_deref() {
        info.manifest_main_class = parse_manifest_main_class(text);
    }
    if let Some(text) = fabric_text.as_deref() {
        info.fabric_mod_id = parse_fabric_id(text);
    }
    if let Some(text) = forge_text.as_deref() {
        info.forge_mod_id = parse_forge_mod_id(text);
    }
    info.is_loader_installer = info
        .manifest_main_class
        .as_deref()
        .map(|s| s.to_lowercase().contains("installer"))
        .unwrap_or(false);
    Ok(info)
}

/// Scan a JAR file at `path` without extracting.
pub fn scan_jar_file(path: impl AsRef<Path>) -> Result<JarInfo> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|e| Error::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    scan_jar(bytes.as_slice())
}

/// Parse `Main-Class` from manifest text, unfolding continuation lines.
fn parse_manifest_main_class(text: &str) -> Option<String> {
    let mut logical: Vec<String> = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.starts_with(' ') && !logical.is_empty() {
            let last_index = logical.len() - 1;
            let continuation = line.strip_prefix(' ').unwrap_or(line);
            logical[last_index].push_str(continuation);
        } else {
            logical.push(line.to_string());
        }
    }
    for line in logical {
        let mut parts = line.splitn(2, ':');
        let key = match parts.next() {
            Some(k) => k.trim(),
            None => continue,
        };
        let value = match parts.next() {
            Some(v) => v.trim(),
            None => continue,
        };
        if key.eq_ignore_ascii_case("Main-Class") && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// Parse `id` from `fabric.mod.json` text; returns `None` on invalid JSON.
fn parse_fabric_id(text: &str) -> Option<String> {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return None,
    };
    value
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Parse the first `modId` from `mods.toml` text without a TOML parser.
fn parse_forge_mod_id(text: &str) -> Option<String> {
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Strip inline comments that are outside quotes (best effort).
        let code = strip_toml_comment(line);
        let code = code.trim();
        if !code.contains("modId") {
            continue;
        }
        let eq = match code.find('=') {
            Some(i) => i,
            None => continue,
        };
        let (left, right) = code.split_at(eq);
        if !left.contains("modId") {
            continue;
        }
        let value = right.strip_prefix('=').unwrap_or(right).trim();
        let unquoted = value
            .strip_prefix('"')
            .and_then(|s| s.split('"').next())
            .or_else(|| value.strip_prefix('\'').and_then(|s| s.split('\'').next()));
        match unquoted {
            Some(id) if !id.is_empty() => return Some(id.to_string()),
            _ => continue,
        }
    }
    None
}

/// Strip a `#` comment that appears outside double/single quotes.
fn strip_toml_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    for (idx, ch) in line.char_indices() {
        if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '#' && !in_single && !in_double {
            return line[..idx].trim_end();
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write as _};

    fn build_jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let buf = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn scans_fabric_mod() {
        let manifest = b"Manifest-Version: 1.0\n";
        let fabric = br#"{"schemaVersion": 1, "id": "mymod", "version": "1.0.0"}"#;
        let bytes = build_jar(&[
            ("META-INF/MANIFEST.MF", manifest),
            ("fabric.mod.json", fabric),
            ("com/example/A.class", &[0xCA, 0xFE, 0xBA, 0xBE]),
            ("com/example/B.class", &[0xCA, 0xFE, 0xBA, 0xBE]),
        ]);
        let info = scan_jar(bytes.as_slice()).unwrap();
        assert_eq!(info.fabric_mod_id.as_deref(), Some("mymod"));
        assert_eq!(info.forge_mod_id, None);
        assert_eq!(info.manifest_main_class, None);
        assert_eq!(info.class_count, 2);
        assert!(!info.is_loader_installer);
    }

    #[test]
    fn scans_forge_mod_and_manifest() {
        let manifest = b"Manifest-Version: 1.0\nMain-Class: net.minecraft.client.main.Main\n";
        let mods = b"# comment\nmodLoader=\"javafml\"\n[[mods]]\nmodId=\"examplemod\" # trailing\n";
        let bytes = build_jar(&[
            ("META-INF/MANIFEST.MF", manifest),
            ("META-INF/mods.toml", mods),
            ("a/B.class", &[1, 2, 3]),
        ]);
        let info = scan_jar(bytes.as_slice()).unwrap();
        assert_eq!(
            info.manifest_main_class.as_deref(),
            Some("net.minecraft.client.main.Main")
        );
        assert_eq!(info.forge_mod_id.as_deref(), Some("examplemod"));
        assert_eq!(info.fabric_mod_id, None);
        assert_eq!(info.class_count, 1);
        assert!(!info.is_loader_installer);
    }

    #[test]
    fn detects_loader_installer() {
        let manifest =
            b"Manifest-Version: 1.0\nMain-Class: net.fabricmc.installer.Main\n";
        let bytes = build_jar(&[("META-INF/MANIFEST.MF", manifest)]);
        let info = scan_jar(bytes.as_slice()).unwrap();
        assert_eq!(
            info.manifest_main_class.as_deref(),
            Some("net.fabricmc.installer.Main")
        );
        assert!(info.is_loader_installer);
        assert_eq!(info.class_count, 0);
    }

    #[test]
    fn empty_jar_has_no_metadata() {
        let bytes = build_jar(&[("data.txt", b"hi")]);
        let info = scan_jar(bytes.as_slice()).unwrap();
        assert_eq!(info.manifest_main_class, None);
        assert_eq!(info.fabric_mod_id, None);
        assert_eq!(info.forge_mod_id, None);
        assert_eq!(info.class_count, 0);
        assert!(!info.is_loader_installer);
    }

    #[test]
    fn corrupt_bytes_are_an_error() {
        let err = scan_jar(b"not a zip".as_slice()).unwrap_err();
        assert!(matches!(err, Error::Zip(_)));
    }
}
