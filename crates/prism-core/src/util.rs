//! Filesystem and misc helpers mirroring parts of Prism's `FileSystem.cpp`.

use crate::error::{Error, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Read a file fully, wrapping failures with the path.
pub fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|e| Error::io(path, e))
}

/// Read a UTF-8 file. Replacement characters are used for invalid bytes
/// (matches `QTextStream` leniency that Prism relies on for old files).
pub fn read_text(path: &Path) -> Result<String> {
    let bytes = read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Write a file atomically: data is written to a sibling temporary file and
/// then renamed over the target (mirrors `QSaveFile` / `PSaveFile` commit).
/// The temp file is removed on any failure.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let name = path.file_name().map(std::ffi::OsStr::to_owned).unwrap_or_default();
    let mut tmp = dir.join(format!(".{}.tmp-{}", name.to_string_lossy(), std::process::id()));
    // Avoid clashing with a leftover temp from a crashed run.
    for attempt in 0..64u32 {
        if !tmp.exists() {
            break;
        }
        tmp = dir.join(format!(".{}.tmp-{}-{}", name.to_string_lossy(), std::process::id(), attempt));
    }
    let write_result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all().ok();
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(Error::io(path, e));
    }
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(Error::io(path, e))
        }
    }
}

/// Ensure a directory exists (like `FS::ensureFolderPathExists`).
pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| Error::io(path, e))
}

/// Characters forbidden in folder names on Windows; also rejected by Prism's
/// `FS::RemoveInvalidPathChars`.
const INVALID_NAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// Sanitize a display name into a directory name (`FS::DirNameFromString`).
/// Forbidden characters and control characters are replaced with `_`; leading
/// dots and trailing dots/spaces (illegal on Windows) are trimmed. An empty
/// result yields `Unnamed`.
pub fn sanitize_dir_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if INVALID_NAME_CHARS.contains(&c) || c.is_control() {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let trimmed = out.trim_start_matches('.').trim_end_matches(['.', ' ']).to_string();
    if trimmed.is_empty() { "Unnamed".to_string() } else { trimmed }
}

/// Produce a directory name that does not yet exist inside `parent`,
/// appending `_1`, `_2`, ... as needed (mirrors `FS::DirNameFromString`).
pub fn unique_dir_name(parent: &Path, name: &str) -> Result<String> {
    let base = sanitize_dir_name(name);
    let mut candidate = base.clone();
    let mut n: u32 = 0;
    while parent.join(&candidate).exists() {
        n += 1;
        if n > 4096 {
            return Err(Error::InvalidInstanceName(base));
        }
        candidate = format!("{base}_{n}");
    }
    Ok(candidate)
}

/// Current wall time in milliseconds since the Unix epoch.
pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_forbidden_and_control_chars() {
        assert_eq!(sanitize_dir_name("a/b\\c:d*e?f\"g<h>i|j"), "a_b_c_d_e_f_g_h_i_j");
        assert_eq!(sanitize_dir_name("ok\u{7}name"), "ok_name");
    }

    #[test]
    fn sanitize_trims_windows_hazards_and_handles_empty() {
        assert_eq!(sanitize_dir_name("name."), "name");
        assert_eq!(sanitize_dir_name(" name "), " name"); // trailing space trimmed (Windows hazard)
        assert_eq!(sanitize_dir_name("..."), "Unnamed");
        assert_eq!(sanitize_dir_name(""), "Unnamed");
    }

    #[test]
    fn unique_dir_name_appends_suffix_until_free() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert_eq!(unique_dir_name(root, "My Pack").unwrap(), "My Pack");
        std::fs::create_dir_all(root.join("My Pack")).unwrap();
        assert_eq!(unique_dir_name(root, "My Pack").unwrap(), "My Pack_1");
    }

    #[test]
    fn atomic_write_roundtrips_and_cleans_temp_on_rename_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("f.cfg");
        atomic_write(&path, b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        // no temp leftovers
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains(".tmp-")).collect();
        assert!(leftovers.is_empty());
    }
}
