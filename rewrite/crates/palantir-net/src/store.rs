//! The hash-keyed content store: one verified copy per content hash.
//!
//! Game files are named by their content's SHA-1 in the metadata, and the
//! same bytes appear under many names -- every version bundles the same
//! libraries, and an asset index names the same object twice. A store keyed
//! by hash holds each byte string once, and the layout paths (a library's
//! Maven location, an asset's object path) are materialized views of it:
//! hard links where the filesystem allows, copies where it does not.
//!
//! The store is honest by construction: bytes enter through [`put_file`],
//! which verifies the hash before accepting them, so a store entry is a
//! promise kept. Everything else trusts the name.

use std::path::{Path, PathBuf};

use crate::download::sha1_file;
use crate::error::{Error, Result};

/// Content-addressed storage under one directory.
pub struct ContentStore {
    dir: PathBuf,
}

impl ContentStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Where this hash's bytes live: `<dir>/<first two hex>/<hash>`, the
    /// bucketing the game's own object store uses (one directory per byte
    /// pair keeps directories small enough to list).
    pub fn path(&self, sha1: &str) -> Result<PathBuf> {
        let prefix = sha1
            .get(..2)
            .filter(|p| p.len() == 2)
            .ok_or_else(|| Error::Invalid {
                what: "content hash",
                why: format!("{sha1:?} is too short to bucket"),
            })?;
        Ok(self.dir.join(prefix).join(sha1))
    }

    /// Are these bytes already held?
    pub fn contains(&self, sha1: &str) -> bool {
        self.path(sha1).map(|path| path.is_file()).unwrap_or(false)
    }

    /// Take verified bytes into the store. The hash is checked here, at the
    /// door: a store entry is a kept promise. If the store already holds the
    /// hash, `src` is dropped in favour of the copy already trusted.
    pub fn put_file(&self, src: &Path, sha1: &str) -> Result<PathBuf> {
        let dest = self.path(sha1)?;
        // Verify at the door even when the store already holds the hash: a
        // caller that believes wrong bytes carry this hash has a bug worth
        // reporting, not hiding behind a copy we already trust.
        let actual = sha1_file(src)?;
        if !actual.eq_ignore_ascii_case(sha1) {
            remove_if_present(src)?;
            return Err(Error::Hash {
                path: src.to_path_buf(),
                expected: sha1.to_string(),
                actual,
            });
        }
        if dest.is_file() {
            remove_if_present(src)?;
            return Ok(dest);
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        // Move when we can (one directory tree), copy when we cannot.
        if std::fs::rename(src, &dest).is_err() {
            std::fs::copy(src, &dest).map_err(|source| Error::Io {
                path: dest.clone(),
                source,
            })?;
            remove_if_present(src)?;
        }
        Ok(dest)
    }

    /// Make `dest` name the stored bytes: a hard link when the filesystem
    /// allows one (no second copy of a 60 MB jar), a copy when it does not.
    /// A destination already in place is left alone.
    pub fn materialize(&self, sha1: &str, dest: &Path) -> Result<()> {
        if dest.is_file() {
            return Ok(());
        }
        let src = self.path(sha1)?;
        if !src.is_file() {
            return Err(Error::Invalid {
                what: "content store",
                why: format!("no stored bytes for {sha1}"),
            });
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        if std::fs::hard_link(&src, dest).is_err() {
            std::fs::copy(&src, dest).map_err(|source| Error::Io {
                path: dest.to_path_buf(),
                source,
            })?;
        }
        Ok(())
    }

    /// The directory the store lives in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

fn remove_if_present(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> (PathBuf, ContentStore) {
        let dir =
            std::env::temp_dir().join(format!("palantirmc-store-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (dir.clone(), ContentStore::new(dir))
    }

    /// SHA-1 of "hello\n" (confirmed with `printf 'hello\n' | sha1sum`).
    const HELLO: &str = "f572d396fae9206628714fb2ce00f72e94f2258f";

    #[test]
    fn a_store_entry_is_a_kept_promise() {
        let (dir, store) = temp_store("verify");
        let src = dir.join("incoming.bin");
        std::fs::write(&src, b"hello\n").unwrap();
        let stored = store.put_file(&src, HELLO).unwrap();
        assert_eq!(stored, store.path(HELLO).unwrap());
        assert!(store.contains(HELLO));
        // Wrong content is refused and not kept.
        let bad = dir.join("bad.bin");
        std::fs::write(&bad, b"goodbye\n").unwrap();
        assert!(store.put_file(&bad, HELLO).is_err());
        assert!(!bad.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn materialize_reuses_one_copy() {
        let (dir, store) = temp_store("link");
        let src = dir.join("incoming.bin");
        std::fs::write(&src, b"hello\n").unwrap();
        store.put_file(&src, HELLO).unwrap();
        let a = dir.join("names").join("a.bin");
        let b = dir.join("other").join("b.bin");
        store.materialize(HELLO, &a).unwrap();
        store.materialize(HELLO, &b).unwrap();
        assert_eq!(std::fs::read(&a).unwrap(), b"hello\n");
        assert_eq!(std::fs::read(&b).unwrap(), b"hello\n");
        assert!(store.materialize("ab", &dir.join("nope")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bucketing_needs_two_hex_characters() {
        let (dir, store) = temp_store("bucket");
        assert!(store.path("").is_err());
        assert!(store.path("a").is_err());
        assert!(store.path(&"x".repeat(40)).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
