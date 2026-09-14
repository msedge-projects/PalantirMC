//! The Java runtime metadata: what the service publishes so a launcher can
//! install the Java a version asks for.
//!
//! A version file does not only name a main class: it says which Java it will
//! run on, in two fields Prism publishes and Mojang's own launcher does not —
//! `compatibleJavaMajors` (`[21]` for 1.21.1, `[8]` for 1.12.2) and
//! `compatibleJavaName` (`java-runtime-delta`, `jre-legacy`). When the machine
//! has no runtime at an accepted major, the alternative to fetching one is a JVM
//! refusing the game with `UnsupportedClassVersionError`, which reads as "this
//! launcher is broken" rather than "install a Java".
//!
//! The runtimes are one more metadata component, `net.minecraft.java`:
//!
//! ```text
//! <base>/net.minecraft.java/index.json   → java8 … java25, one entry per major
//! <base>/net.minecraft.java/java21.json  → runtimes: [{runtimeOS, name, …}]
//! ```
//!
//! Each entry in a `runtimes` array names the operating-system/architecture pair
//! it serves, the runtime it is, and how to get it. Two download types exist:
//! Mojang's own per-file `manifest`, and an `archive` (a `.tar.gz` from Eclipse
//! Adoptium or Azul) for the platforms Mojang does not build for.
//!
//! This module is parsing only — no filesystem, no downloads — which is what
//! lets the live test in `tests/live.rs` ask the real service for these files
//! and feed them through the same code the launcher runs. Installing what is
//! parsed happens in `palantir-desktop`'s `java_runtime` module, because it is
//! filesystem work under a data root that this crate knows nothing about.

use palantir_core::version::RuntimeContext;
use serde_json::Value;

/// The metadata component that lists Java runtimes.
pub const JAVA_RUNTIMES_UID: &str = "net.minecraft.java";

/// Where a managed runtime is unpacked, under a data root.
///
/// Prism's own folder name, so a Java downloaded by either launcher is found by
/// the other.
pub const MANAGED_DIR: &str = "java";

/// The runtime the host needs, as the metadata spells it.
///
/// Mirrors Prism's `SysInfo::getSupportedJavaArchitecture`, including the part
/// that matters most: an architecture it does not know is *appended* rather than
/// guessed at, so a new platform gets its own tag instead of being handed x64
/// binaries that cannot execute.
pub fn host_runtime_os(ctx: &RuntimeContext) -> String {
    let arch = ctx.mapped_arch();
    match ctx.system.as_str() {
        "windows" => match arch {
            "x86_64" => "windows-x64".to_string(),
            "x86" => "windows-x86".to_string(),
            other => format!("windows-{other}"),
        },
        "osx" => match arch {
            "arm64" => "mac-os-arm64".to_string(),
            "x86_64" | "x64" => "mac-os-x64".to_string(),
            "x86" => "mac-os-x86".to_string(),
            other => format!("mac-os-{other}"),
        },
        "linux" => match arch {
            "x86_64" => "linux-x64".to_string(),
            "x86" => "linux-x86".to_string(),
            other => format!("linux-{other}"),
        },
        other => format!("{other}-{arch}"),
    }
}

/// The version-list URL for the runtimes.
pub fn runtime_list_url(base_url: &str) -> String {
    format!("{}/{}/index.json", base_url.trim_end_matches('/'), JAVA_RUNTIMES_UID)
}

/// The version-file URL for one runtime major (`java21`).
pub fn runtime_file_url(base_url: &str, version: &str) -> String {
    format!("{}/{}/{}.json", base_url.trim_end_matches('/'), JAVA_RUNTIMES_UID, version)
}

/// The metadata version that answers one Java major (`21` → `java21`).
pub fn runtime_version(major: i64) -> String {
    format!("java{major}")
}

/// One runtime as the service describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEntry {
    /// Operating-system/architecture tag (`windows-x64`).
    pub os: String,
    /// Runtime name (`java-runtime-delta`) — what `compatibleJavaName` names.
    pub name: String,
    /// `manifest` or `archive`.
    pub kind: String,
    /// Where the runtime's own description lives.
    pub url: String,
    /// Published digest of what `url` serves; empty when none was published.
    pub sha1: String,
    /// Major the entry belongs to, when it says.
    pub major: i64,
}

impl RuntimeEntry {
    /// Whether this entry is Mojang's per-file manifest — the only kind this
    /// launcher can install, because the other is a tarball.
    pub fn is_manifest(&self) -> bool {
        self.kind == "manifest"
    }
}

/// Parse a `runtimes` array (the body of `net.minecraft.java/<version>.json`).
///
/// An entry without a URL or without a platform is dropped rather than turned
/// into a download of nothing; the caller reports "no runtime for this
/// platform" when nothing is left, which is the honest outcome.
pub fn parse_runtimes(bytes: &[u8], path: &std::path::Path) -> crate::Result<Vec<RuntimeEntry>> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        crate::Error::json(path, error.to_string())
    })?;
    let entries = value.get("runtimes").and_then(Value::as_array).ok_or_else(|| {
        crate::Error::format(path, "a Java runtime file must carry a 'runtimes' array")
    })?;
    let mut out = Vec::new();
    for entry in entries {
        let Some(object) = entry.as_object() else {
            continue;
        };
        let url = object.get("url").and_then(Value::as_str).unwrap_or_default();
        let os = object.get("runtimeOS").and_then(Value::as_str).unwrap_or_default();
        if url.is_empty() || os.is_empty() {
            continue;
        }
        out.push(RuntimeEntry {
            os: os.to_string(),
            name: object.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
            kind: object
                .get("downloadType")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            url: url.to_string(),
            // The entry's `checksum` is the SHA-1 of the *manifest* the URL
            // serves — checked by hand against the live service for every
            // manifest entry of `java8` and `java21`, and asserted in
            // `tests/live.rs` because a wrong belief here would refuse every
            // install as tampered with.
            sha1: object
                .get("checksum")
                .and_then(|checksum| checksum.get("hash"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            major: object
                .get("version")
                .and_then(|version| version.get("major"))
                .and_then(Value::as_i64)
                .unwrap_or(0),
        });
    }
    Ok(out)
}

/// The first runtime for `os` whose name is `name`, exactly as Prism matches it.
///
/// Both halves are required: the service lists several names for one tag (the
/// `java17` file offers `java-runtime-beta`, `-gamma` and a snapshot for
/// `windows-x64`), so matching the tag alone would install a runtime the
/// version did not ask for.
pub fn pick_runtime<'a>(
    entries: &'a [RuntimeEntry],
    os: &str,
    name: &str,
) -> Option<&'a RuntimeEntry> {
    entries.iter().find(|entry| entry.os == os && entry.name == name)
}

/// One file of a JRE manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFile {
    /// Path inside the runtime directory, always `/`-separated.
    pub path: String,
    /// Where the bytes are.
    pub url: String,
    /// Published SHA-1 (lowercase hex); empty when none was published.
    pub sha1: String,
    /// Published size; `0` when the manifest did not say.
    pub size: i64,
    /// Whether the executable bit is expected (`bin/java.exe` on Windows,
    /// `bin/java` everywhere else).
    pub executable: bool,
}

/// Parse Mojang's JRE manifest: `{"files": {"<path>": {...}}}`.
///
/// Directory entries carry no `downloads` block and are skipped, because the
/// files under them create them. A *file* entry with nothing to fetch is
/// skipped too — writing it as an empty file would produce a runtime that is
/// quietly incomplete, which fails much later and much less clearly.
pub fn parse_manifest(bytes: &[u8], path: &std::path::Path) -> crate::Result<Vec<RuntimeFile>> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| crate::Error::json(path, error.to_string()))?;
    let files = value.get("files").and_then(Value::as_object).ok_or_else(|| {
        crate::Error::format(path, "a JRE manifest must carry a 'files' object")
    })?;
    let mut out = Vec::new();
    for (rel, entry) in files {
        if entry.get("type").and_then(Value::as_str).unwrap_or("file") != "file" {
            continue;
        }
        let Some(raw) = entry.get("downloads").and_then(|downloads| downloads.get("raw")) else {
            continue;
        };
        let url = raw.get("url").and_then(Value::as_str).unwrap_or_default();
        if url.is_empty() {
            continue;
        }
        out.push(RuntimeFile {
            path: rel.replace('\\', "/"),
            url: url.to_string(),
            sha1: raw.get("sha1").and_then(Value::as_str).unwrap_or_default().to_string(),
            size: raw.get("size").and_then(Value::as_i64).unwrap_or(0),
            executable: entry.get("executable").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(system: &str, arch: &str) -> RuntimeContext {
        RuntimeContext {
            java_architecture: "64".to_string(),
            java_real_architecture: arch.to_string(),
            system: system.to_string(),
        }
    }

    #[test]
    fn host_tags_use_the_published_spellings() {
        assert_eq!(host_runtime_os(&ctx("windows", "amd64")), "windows-x64");
        assert_eq!(host_runtime_os(&ctx("windows", "i386")), "windows-x86");
        assert_eq!(host_runtime_os(&ctx("windows", "aarch64")), "windows-arm64");
        assert_eq!(host_runtime_os(&ctx("linux", "amd64")), "linux-x64");
        assert_eq!(host_runtime_os(&ctx("linux", "aarch64")), "linux-arm64");
        assert_eq!(host_runtime_os(&ctx("linux", "riscv64")), "linux-riscv64");
        assert_eq!(host_runtime_os(&ctx("osx", "aarch64")), "mac-os-arm64");
        assert_eq!(host_runtime_os(&ctx("osx", "amd64")), "mac-os-x64");
    }

    #[test]
    fn an_unknown_architecture_is_appended_rather_than_guessed() {
        // The failure this prevents is handing a new CPU architecture x64
        // binaries, which installs a runtime that cannot execute.
        assert_eq!(host_runtime_os(&ctx("linux", "loongarch64")), "linux-loongarch64");
        assert_eq!(host_runtime_os(&ctx("windows", "arm32")), "windows-arm32");
    }

    #[test]
    fn runtime_urls_follow_the_metadata_layout() {
        assert_eq!(
            runtime_list_url("https://meta.example/v1"),
            "https://meta.example/v1/net.minecraft.java/index.json"
        );
        assert_eq!(
            runtime_file_url("https://meta.example/v1/", "java21"),
            "https://meta.example/v1/net.minecraft.java/java21.json"
        );
        assert_eq!(runtime_version(21), "java21");
    }

    #[test]
    fn runtimes_parse_and_entries_without_a_platform_are_dropped() {
        let bytes = serde_json::to_vec(&json!({
            "uid": "net.minecraft.java", "version": "java21",
            "runtimes": [
                {"runtimeOS": "windows-x64", "name": "java-runtime-delta",
                 "downloadType": "manifest", "url": "https://piston/manifest.json",
                 "checksum": {"type": "sha1", "hash": "abc"},
                 "version": {"major": 21}},
                {"runtimeOS": "not-a-runtime", "name": "broken"},
            ]
        }))
        .unwrap();
        let entries = parse_runtimes(&bytes, std::path::Path::new("java21.json")).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].os, "windows-x64");
        assert_eq!(entries[0].name, "java-runtime-delta");
        assert_eq!(entries[0].sha1, "abc");
        assert_eq!(entries[0].major, 21);
        assert!(entries[0].is_manifest());
    }

    #[test]
    fn a_runtime_file_without_a_runtimes_array_is_an_error() {
        let err = parse_runtimes(b"{\"uid\": \"net.minecraft.java\"}", std::path::Path::new("java21.json"))
            .unwrap_err();
        assert!(err.to_string().contains("runtimes"), "{err}");
    }

    #[test]
    fn the_entry_pick_requires_both_the_tag_and_the_name() {
        let entry = |os: &str, name: &str, url: &str| RuntimeEntry {
            os: os.into(),
            name: name.into(),
            kind: "manifest".into(),
            url: url.into(),
            sha1: String::new(),
            major: 17,
        };
        let entries = vec![
            entry("windows-x64", "java-runtime-gamma", "https://gamma"),
            entry("windows-x64", "java-runtime-delta", "https://delta"),
            entry("linux-x64", "java-runtime-delta", "https://delta-linux"),
        ];
        // One tag, several names: the name decides.
        assert_eq!(
            pick_runtime(&entries, "windows-x64", "java-runtime-delta").unwrap().url,
            "https://delta"
        );
        // Right name, wrong platform: not installable here.
        assert!(pick_runtime(&entries, "windows-arm64", "java-runtime-delta").is_none());
        assert!(pick_runtime(&entries, "windows-x64", "jre-legacy").is_none());
    }

    #[test]
    fn a_manifest_parses_its_files_and_skips_what_has_nothing_to_fetch() {
        let bytes = serde_json::to_vec(&json!({
            "files": {
                "bin": {"type": "directory"},
                "bin/java.exe": {
                    "type": "file", "executable": true,
                    "downloads": {
                        "raw": {"sha1": "aa", "size": 12, "url": "https://objects/java.exe"},
                        "lzma": {"sha1": "bb", "size": 4, "url": "https://objects/java.exe.lzma"}
                    }
                },
                "lib/only-lzma": {
                    "type": "file",
                    "downloads": {"lzma": {"sha1": "cc", "size": 2, "url": "https://objects/x.lzma"}}
                },
                "release": {"type": "file", "downloads": {"raw": {"sha1": "dd", "size": 5, "url": "https://objects/release"}}}
            }
        }))
        .unwrap();
        let files = parse_manifest(&bytes, std::path::Path::new("manifest.json")).unwrap();
        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, vec!["bin/java.exe", "release"]);
        assert!(files[0].executable);
        assert_eq!(files[0].size, 12);
        assert!(!paths.contains(&"lib/only-lzma"));
    }

    #[test]
    fn a_manifest_without_files_is_an_error() {
        let err = parse_manifest(b"{}", std::path::Path::new("manifest.json")).unwrap_err();
        assert!(err.to_string().contains("files"), "{err}");
    }

    #[test]
    fn windows_separators_in_a_manifest_are_normalized() {
        let bytes = serde_json::to_vec(&json!({
            "files": {
                "bin\\java.exe": {
                    "type": "file", "executable": true,
                    "downloads": {"raw": {"sha1": "aa", "size": 1, "url": "https://objects/java.exe"}}
                }
            }
        }))
        .unwrap();
        let files = parse_manifest(&bytes, std::path::Path::new("manifest.json")).unwrap();
        assert_eq!(files[0].path, "bin/java.exe");
    }
}
