//! Safe archive extraction with zip-slip protection.
//!
//! Mirrors Prism's file-extraction safety: entries with absolute paths or
//! `..` components that would escape the destination are rejected instead
//! of written. Unix permission bits are preserved where present.

use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Errors from safe archive extraction.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Filesystem failure with the path that caused it.
    #[error("io error for {path}: {source}")]
    Io {
        /// The path involved in the failed operation.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// Zip container failure.
    #[error("zip error: {0}")]
    Zip(String),
    /// Tar/gzip container failure.
    #[error("tar error: {0}")]
    Tar(String),
    /// Archive entry would escape the destination (absolute path or `..`).
    #[error("unsafe archive path: {0}")]
    UnsafePath(String),
}

/// Result alias for archive operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Helper to build an [`Error::Io`] from a path and an [`std::io::Error`].
fn io_err(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.to_path_buf(), source }
}

/// A source of zip bytes: either in-memory bytes or a file on disk.
///
/// Implemented for byte containers (`Vec<u8>`, `&[u8]`, fixed arrays) and
/// for path types (`PathBuf`, `&Path`, `String`, `&str`, ...). Path inputs
/// are read from disk; byte inputs are used directly.
pub trait ZipSource {
    /// Read the full zip container into memory.
    fn read_zip_bytes(&self) -> Result<Vec<u8>>;
}

/// A source of tar.gz bytes: either in-memory bytes or a file on disk.
///
/// Implemented for the same set of types as [`ZipSource`].
pub trait TarGzSource {
    /// Read the full tar.gz container into memory.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>>;
}

/// Read a file from disk for a [`ZipSource`]/[`TarGzSource`] path input.
fn read_file_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| io_err(path, e))
}

impl ZipSource for Vec<u8> {
    /// Clone in-memory zip bytes.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.clone())
    }
}

impl ZipSource for &Vec<u8> {
    /// Clone in-memory zip bytes behind a reference.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        Ok((*self).clone())
    }
}

impl ZipSource for &[u8] {
    /// Copy in-memory zip bytes behind a slice reference.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.to_vec())
    }
}

impl<const N: usize> ZipSource for [u8; N] {
    /// Copy in-memory zip bytes from a fixed array.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.to_vec())
    }
}

impl<const N: usize> ZipSource for &[u8; N] {
    /// Copy in-memory zip bytes from a fixed array reference.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.to_vec())
    }
}

impl ZipSource for PathBuf {
    /// Read a zip file from disk.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(self.as_path())
    }
}

impl ZipSource for &PathBuf {
    /// Read a zip file from disk behind a reference.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(self.as_path())
    }
}

impl ZipSource for &Path {
    /// Read a zip file from disk behind a path reference.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(self)
    }
}

impl ZipSource for String {
    /// Read a zip file whose path is given as an owned string.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(Path::new(self.as_str()))
    }
}

impl ZipSource for &String {
    /// Read a zip file whose path is given as a string reference.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(Path::new(self.as_str()))
    }
}

impl ZipSource for &str {
    /// Read a zip file whose path is given as a string slice.
    fn read_zip_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(Path::new(self))
    }
}

impl TarGzSource for Vec<u8> {
    /// Clone in-memory tar.gz bytes.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.clone())
    }
}

impl TarGzSource for &Vec<u8> {
    /// Clone in-memory tar.gz bytes behind a reference.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        Ok((*self).clone())
    }
}

impl TarGzSource for &[u8] {
    /// Copy in-memory tar.gz bytes behind a slice reference.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.to_vec())
    }
}

impl<const N: usize> TarGzSource for [u8; N] {
    /// Copy in-memory tar.gz bytes from a fixed array.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.to_vec())
    }
}

impl<const N: usize> TarGzSource for &[u8; N] {
    /// Copy in-memory tar.gz bytes from a fixed array reference.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.to_vec())
    }
}

impl TarGzSource for PathBuf {
    /// Read a tar.gz file from disk.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(self.as_path())
    }
}

impl TarGzSource for &PathBuf {
    /// Read a tar.gz file from disk behind a reference.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(self.as_path())
    }
}

impl TarGzSource for &Path {
    /// Read a tar.gz file from disk behind a path reference.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(self)
    }
}

impl TarGzSource for String {
    /// Read a tar.gz file whose path is given as an owned string.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(Path::new(self.as_str()))
    }
}

impl TarGzSource for &String {
    /// Read a tar.gz file whose path is given as a string reference.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(Path::new(self.as_str()))
    }
}

impl TarGzSource for &str {
    /// Read a tar.gz file whose path is given as a string slice.
    fn read_tar_gz_bytes(&self) -> Result<Vec<u8>> {
        read_file_bytes(Path::new(self))
    }
}

/// Join an archive entry to `dest`, rejecting absolute paths and `..` escapes.
fn join_safe(dest: &Path, entry: &Path) -> Result<PathBuf> {
    if entry.is_absolute() {
        return Err(Error::UnsafePath(entry.to_string_lossy().into_owned()));
    }
    for comp in entry.components() {
        match comp {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::UnsafePath(entry.to_string_lossy().into_owned()));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(dest.join(entry))
}

/// Extract a zip container (in-memory bytes or file path) into `dest`.
///
/// Rejects absolute paths and `..` escapes with [`Error::UnsafePath`] and
/// preserves Unix permission bits where present.
pub fn extract_zip(source: impl ZipSource, dest: impl AsRef<Path>) -> Result<()> {
    let bytes = source.read_zip_bytes()?;
    extract_zip_bytes(bytes.as_slice(), dest)
}

/// Extract raw zip bytes into `dest` with zip-slip protection.
pub fn extract_zip_bytes(data: &[u8], dest: impl AsRef<Path>) -> Result<()> {
    let dest = dest.as_ref();
    std::fs::create_dir_all(dest).map_err(|e| io_err(dest, e))?;
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| Error::Zip(e.to_string()))?;
    let len = archive.len();
    let mut index: usize = 0;
    while index < len {
        let mut file = archive.by_index(index).map_err(|e| Error::Zip(e.to_string()))?;
        let raw_name = file.name().to_owned();
        let enclosed_opt = file.enclosed_name();
        let enclosed = match enclosed_opt {
            Some(p) => p,
            None => return Err(Error::UnsafePath(raw_name)),
        };
        #[allow(clippy::needless_borrow)]
        let out_path = join_safe(dest, &enclosed)?;
        if file.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| io_err(&out_path, e))?;
        } else {
            if let Some(parent) = out_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
                }
            }
            let mut out =
                std::fs::File::create(&out_path).map_err(|e| io_err(&out_path, e))?;
            std::io::copy(&mut file, &mut out).map_err(|e| io_err(&out_path, e))?;
            #[cfg(unix)]
            {
                if let Some(mode) = file.unix_mode() {
                    let perms = std::fs::Permissions::from_mode(mode);
                    std::fs::set_permissions(&out_path, perms)
                        .map_err(|e| io_err(&out_path, e))?;
                }
            }
        }
        index += 1;
    }
    Ok(())
}

/// Extract a zip file at `path` into `dest` with zip-slip protection.
pub fn extract_zip_file(path: impl AsRef<Path>, dest: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let bytes = read_file_bytes(path)?;
    extract_zip_bytes(bytes.as_slice(), dest)
}

/// Extract a tar.gz container (file path or in-memory bytes) into `dest`.
///
/// Rejects absolute paths and `..` escapes with [`Error::UnsafePath`] and
/// preserves Unix permission bits where present. Symlinks and other special
/// entries are skipped for safety.
pub fn extract_tar_gz(source: impl TarGzSource, dest: impl AsRef<Path>) -> Result<()> {
    let bytes = source.read_tar_gz_bytes()?;
    extract_tar_gz_bytes(bytes.as_slice(), dest)
}

/// Extract raw tar.gz bytes into `dest` with traversal protection.
pub fn extract_tar_gz_bytes(data: &[u8], dest: impl AsRef<Path>) -> Result<()> {
    let dest = dest.as_ref();
    std::fs::create_dir_all(dest).map_err(|e| io_err(dest, e))?;
    let decoder = flate2::read::GzDecoder::new(data);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(|e| Error::Tar(e.to_string()))?;
    for entry_res in entries {
        let mut entry = entry_res.map_err(|e| Error::Tar(e.to_string()))?;
        let entry_path = entry
            .path()
            .map_err(|e| Error::Tar(e.to_string()))?
            .into_owned();
        let out_path = join_safe(dest, entry_path.as_path())?;
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| io_err(&out_path, e))?;
        } else if kind.is_file() {
            if let Some(parent) = out_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
                }
            }
            let mut out =
                std::fs::File::create(&out_path).map_err(|e| io_err(&out_path, e))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| io_err(&out_path, e))?;
            #[cfg(unix)]
            {
                match entry.header().mode() {
                    Ok(mode) => {
                        let perms = std::fs::Permissions::from_mode(mode);
                        std::fs::set_permissions(&out_path, perms)
                            .map_err(|e| io_err(&out_path, e))?;
                    }
                    Err(_) => {}
                }
            }
        } else {
            // Skip symlinks, hard links and other special entries.
            continue;
        }
    }
    Ok(())
}

/// Extract a tar.gz file at `path` into `dest` with traversal protection.
pub fn extract_tar_gz_file(path: impl AsRef<Path>, dest: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let bytes = read_file_bytes(path)?;
    extract_tar_gz_bytes(bytes.as_slice(), dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write as _};

    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
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

    fn build_tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let enc = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
            let mut builder = tar::Builder::new(enc);
            for (name, data) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, name, *data).unwrap();
            }
            builder.into_inner().unwrap().finish().unwrap();
        }
        buf
    }

    /// Build a tar.gz with raw headers, bypassing `tar::Builder` path
    /// validation so malicious `..` entries can be represented (other tools
    /// happily produce them; our extractor must still reject them).
    fn build_tar_gz_raw(entries: &[(&str, &[u8])]) -> Vec<u8> {
        fn write_octal(buf: &mut [u8], value: u64, width: usize) {
            let s = format!("{:0width$o}", value, width = width);
            let bytes = s.as_bytes();
            let len = bytes.len().min(buf.len());
            buf[..len].copy_from_slice(&bytes[..len]);
        }
        let mut tar_data: Vec<u8> = Vec::new();
        for (name, data) in entries {
            let mut header = [0u8; 512];
            let name_bytes = name.as_bytes();
            let copy_len = name_bytes.len().min(100);
            header[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
            // mode 0000644\0
            header[100..108].copy_from_slice(b"0000644\0");
            header[108..116].copy_from_slice(b"0000000\0");
            header[116..124].copy_from_slice(b"0000000\0");
            write_octal(&mut header[124..136], data.len() as u64, 11);
            header[135] = 0;
            write_octal(&mut header[136..148], 0, 11);
            header[147] = 0;
            // chksum placeholder: spaces
            for b in header[148..156].iter_mut() {
                *b = b' ';
            }
            header[156] = b'0';
            header[257..263].copy_from_slice(b"ustar\0");
            header[263..265].copy_from_slice(b"00");
            let sum: u32 = header.iter().map(|b| *b as u32).sum();
            let sum_str = format!("{:06o}\0 ", sum);
            header[148..156].copy_from_slice(sum_str.as_bytes());
            tar_data.extend_from_slice(&header);
            tar_data.extend_from_slice(data);
            let pad = (512 - (data.len() % 512)) % 512;
            tar_data.extend(std::iter::repeat(0u8).take(pad));
        }
        // Two zero blocks = end of archive.
        tar_data.extend(std::iter::repeat(0u8).take(1024));
        let mut out = Vec::new();
        {
            let mut enc = flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
            enc.write_all(&tar_data).unwrap();
            enc.finish().unwrap();
        }
        out
    }

    #[test]
    fn zip_round_trips_nested_files() {
        let bytes = build_zip(&[("a.txt", b"hello"), ("sub/b.txt", b"world")]);
        let dir = tempfile::tempdir().unwrap();
        extract_zip(bytes.as_slice(), dir.path()).unwrap();
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), b"hello");
        assert_eq!(std::fs::read(dir.path().join("sub").join("b.txt")).unwrap(), b"world");
    }

    #[test]
    fn zip_generic_accepts_file_path_source() {
        let bytes = build_zip(&[("x.txt", b"data")]);
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("pack.zip");
        std::fs::write(&zip_path, &bytes).unwrap();
        let out = dir.path().join("out");
        extract_zip(zip_path.as_path(), &out).unwrap();
        assert_eq!(std::fs::read(out.join("x.txt")).unwrap(), b"data");
        // Explicit file helper agrees.
        let out2 = dir.path().join("out2");
        extract_zip_file(&zip_path, &out2).unwrap();
        assert_eq!(std::fs::read(out2.join("x.txt")).unwrap(), b"data");
    }

    #[test]
    fn zip_rejects_parent_escape() {
        let bytes = build_zip(&[("../evil.txt", b"bad")]);
        let dir = tempfile::tempdir().unwrap();
        let err = extract_zip(bytes.as_slice(), dir.path()).unwrap_err();
        assert!(matches!(err, Error::UnsafePath(_)));
        assert!(!dir.path().join("evil.txt").exists());
    }

    #[test]
    fn zip_rejects_absolute_path() {
        let bytes = build_zip(&[("/tmp/evil.txt", b"bad")]);
        let dir = tempfile::tempdir().unwrap();
        let err = extract_zip_bytes(bytes.as_slice(), dir.path()).unwrap_err();
        assert!(matches!(err, Error::UnsafePath(_)));
    }

    #[test]
    fn tar_gz_round_trips() {
        let bytes = build_tar_gz(&[("a.txt", b"hello"), ("sub/b.txt", b"world")]);
        let dir = tempfile::tempdir().unwrap();
        extract_tar_gz(bytes.as_slice(), dir.path()).unwrap();
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), b"hello");
        assert_eq!(std::fs::read(dir.path().join("sub").join("b.txt")).unwrap(), b"world");
    }

    #[test]
    fn tar_gz_rejects_parent_escape() {
        // `tar::Builder` refuses `..` paths itself, so craft the archive with
        // raw headers like a real attacker would.
        let evil = build_tar_gz_raw(&[("../evil.txt", b"bad")]);
        let dir = tempfile::tempdir().unwrap();
        let err = extract_tar_gz_bytes(evil.as_slice(), dir.path()).unwrap_err();
        assert!(matches!(err, Error::UnsafePath(_)));
    }

    #[test]
    fn tar_gz_generic_accepts_file_path_source() {
        let bytes = build_tar_gz(&[("y.txt", b"data")]);
        let dir = tempfile::tempdir().unwrap();
        let tar_path = dir.path().join("pack.tar.gz");
        std::fs::write(&tar_path, &bytes).unwrap();
        let out = dir.path().join("out");
        extract_tar_gz(tar_path.as_path(), &out).unwrap();
        assert_eq!(std::fs::read(out.join("y.txt")).unwrap(), b"data");
        let out2 = dir.path().join("out2");
        extract_tar_gz_file(&tar_path, &out2).unwrap();
        assert_eq!(std::fs::read(out2.join("y.txt")).unwrap(), b"data");
    }

    #[test]
    fn preserves_unix_permissions_where_present() {
        let buf = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o755);
        writer.start_file("run.sh", options).unwrap();
        writer.write_all(b"#!/bin/sh\n").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let dir = tempfile::tempdir().unwrap();
        extract_zip(bytes.as_slice(), dir.path()).unwrap();
        assert!(dir.path().join("run.sh").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(dir.path().join("run.sh")).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
    }
}
