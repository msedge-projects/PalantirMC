//! The content store: every file named by its own digest, and shared.
//!
//! The launcher has had three trees that hold the same things twice: a
//! `libraries/` of jars, an `assets/` of objects, and a `.minecraft/versions/`
//! copy of what a version asks for. The same LWJGL jar is downloaded once per
//! Minecraft version that wants it, the same Fabric loader once per instance, and
//! the same mod jar once when a modpack installs it and again when the user adds
//! it by hand. The fix is the oldest one in package management and it is what the
//! client this rewrite is copying does too: **name the file after its contents**,
//! so a file that is already here is already here under every name that asks for
//! it.
//!
//! Three things fall out of that, and they are the reason this is called a store
//! rather than a cache:
//!
//! * **Deduplication is free.** `has(digest)` is a path lookup, not a comparison,
//!   and two instances sharing a jar share the bytes as well as the check.
//! * **Verification is free to state.** The name *is* the digest, so "is this
//!   file the one that was asked for?" is a hash of the file against its own name
//!   -- and `adopt` refuses to file anything that does not answer correctly, which
//!   is what keeps the store from becoming a place where a corrupt download is
//!   cached forever.
//! * **Resume keeps working.** Nothing is renamed into the store until it
//!   verifies, so an interrupted transfer leaves a `.part` under the store's
//!   temporary name and the next attempt continues it.
//!
//! ## Three digests, because the services publish three
//!
//! Mojang publishes a `sha1` per library and per asset object, Modrinth publishes
//! a `sha1` and a `sha512` per file, and Prism's metadata publishes a `sha256`.
//! [`Digest`] carries all three rather than picking a favourite: a store that
//! insisted on one would have to hash the bytes again before it could even look
//! up what the service said, and the point of naming by digest is that the
//! lookup and the check are the same operation. Files are hashed in chunks, so a
//! 37 MB library costs 64 KB of memory to verify rather than 37 MB -- which
//! matters here because the first install of a modern version verifies hundreds
//! of them.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::engine::cancel::Cancel;
use crate::engine::download::{fetch_to_file, Download};
use crate::engine::request::Fetch;
use crate::engine::retry::Backoff;
use crate::Error;

/// How many bytes are read at a time when a file is hashed.
///
/// The same 64 KB the engine transfers with: one buffer size for the whole
/// engine means a transfer's cost is measured once, and a file that is being
/// written by one thread and verified by another costs the same as either.
const HASH_CHUNK: usize = 64 * 1024;

/// A digest, of one of the three kinds the services publish.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Digest {
    /// 40 hex characters. Mojang's per-library and per-asset digest, and one of
    /// the two Modrinth publishes per file.
    Sha1(String),
    /// 64 hex characters. What Prism's metadata publishes, and what the engine's
    /// own downloads verify with.
    Sha256(String),
    /// 128 hex characters. Modrinth's other per-file digest.
    Sha512(String),
}

impl Digest {
    /// Read a digest from what a service published, lowercased.
    ///
    /// The length is what decides the kind, because that is what HTTP gives a
    /// client: a bare hex string in a JSON field, with the algorithm named only
    /// by the field it sits in. An unknown length is refused rather than guessed
    /// at -- a 32-character digest is MD5, and the launcher does not verify with
    /// a digest it cannot compute, so a caller that hands one over has a bug
    /// worth hearing about.
    pub fn parse(text: &str) -> Result<Digest, Error> {
        let hex = text.trim().to_ascii_lowercase();
        if !hex.bytes().all(is_hex_digit) {
            return Err(Error::format("<digest>", format!("'{text}' is not hex")));
        }
        match hex.len() {
            40 => Ok(Digest::Sha1(hex)),
            64 => Ok(Digest::Sha256(hex)),
            128 => Ok(Digest::Sha512(hex)),
            other => Err(Error::format(
                "<digest>",
                format!("{other} characters is not a sha1, sha256 or sha512"),
            )),
        }
    }

    /// The hex digest, as it is used for the store's file name.
    pub fn hex(&self) -> &str {
        match self {
            Digest::Sha1(hex) | Digest::Sha256(hex) | Digest::Sha512(hex) => hex,
        }
    }

    /// Which digest this is, as a service would name it.
    pub fn kind(&self) -> &'static str {
        match self {
            Digest::Sha1(_) => "sha1",
            Digest::Sha256(_) => "sha256",
            Digest::Sha512(_) => "sha512",
        }
    }

    /// The `sha1` of `bytes`.
    pub fn sha1(bytes: &[u8]) -> Digest {
        use sha1::Digest as _;
        Digest::Sha1(format!("{:x}", sha1::Sha1::digest(bytes)))
    }

    /// The `sha256` of `bytes`.
    pub fn sha256(bytes: &[u8]) -> Digest {
        use sha2::Digest as _;
        Digest::Sha256(format!("{:x}", sha2::Sha256::digest(bytes)))
    }

    /// The `sha512` of `bytes`.
    pub fn sha512(bytes: &[u8]) -> Digest {
        use sha2::Digest as _;
        Digest::Sha512(format!("{:x}", sha2::Sha512::digest(bytes)))
    }

    /// The same digest of `bytes`, computed with *this* digest's algorithm.
    ///
    /// What a caller holding "the digest the service published" needs in order
    /// to check bytes it holds: which of the three to compute is the published
    /// digest's business, and a caller that had to switch on the kind itself
    /// would be a caller that could switch wrongly.
    pub fn of(&self, bytes: &[u8]) -> Digest {
        match self {
            Digest::Sha1(_) => Digest::sha1(bytes),
            Digest::Sha256(_) => Digest::sha256(bytes),
            Digest::Sha512(_) => Digest::sha512(bytes),
        }
    }

    /// Whether `bytes` are what this digest names.
    pub fn matches(&self, bytes: &[u8]) -> bool {
        self.of(bytes).hex() == self.hex()
    }

    /// Check that the file at `path` really is these bytes.
    ///
    /// The file is read in chunks and never held whole. A missing file is an
    /// [`Error::Io`] like any other read that could not happen, and a file whose
    /// contents disagree with its name is [`Error::HashMismatch`] -- the same
    /// error a download's own digest check produces, so a caller that refuses a
    /// bad transfer refuses a bad stored file the same way.
    pub fn verify_file(&self, path: &Path) -> Result<(), Error> {
        let actual = match self {
            Digest::Sha1(_) => {
                use sha1::Digest as _;
                let mut hasher = sha1::Sha1::new();
                hash_file(path, |chunk| hasher.update(chunk))?;
                format!("{:x}", hasher.finalize())
            }
            Digest::Sha256(_) => {
                use sha2::Digest as _;
                let mut hasher = sha2::Sha256::new();
                hash_file(path, |chunk| hasher.update(chunk))?;
                format!("{:x}", hasher.finalize())
            }
            Digest::Sha512(_) => {
                use sha2::Digest as _;
                let mut hasher = sha2::Sha512::new();
                hash_file(path, |chunk| hasher.update(chunk))?;
                format!("{:x}", hasher.finalize())
            }
        };
        if actual != self.hex() {
            return Err(Error::hash_mismatch(path, self.hex(), actual));
        }
        Ok(())
    }
}

/// Whether a byte is a lowercase hex digit.
fn is_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

/// Feed a file's contents into `update` a chunk at a time.
fn hash_file(path: &Path, mut update: impl FnMut(&[u8])) -> Result<(), Error> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|error| Error::io(path, error))?;
    let mut buffer = vec![0u8; HASH_CHUNK];
    loop {
        let read = file.read(&mut buffer).map_err(|error| Error::io(path, error))?;
        if read == 0 {
            return Ok(());
        }
        update(&buffer[..read]);
    }
}

/// How a fetch into the store ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// The store already had it, verified nothing and asked for nothing. The
    /// cheap answer, and the reason a second install of the same version is
    /// mostly a disk read.
    AlreadyThere,
    /// It was fetched: how many bytes came over the wire.
    Fetched(u64),
}

/// A directory of files named by digest.
#[derive(Debug, Clone)]
pub struct ContentStore {
    dir: PathBuf,
}

impl ContentStore {
    /// A store rooted at `dir`.
    ///
    /// Nothing is created here: the directory appears when the first file is
    /// stored, so a launcher that only reads never writes.
    pub fn new(dir: impl Into<PathBuf>) -> ContentStore {
        ContentStore { dir: dir.into() }
    }

    /// Where the store lives.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where a digest's file lives: two characters of fan-out, then the digest.
    ///
    /// The fan-out is not decoration. A store with a file per library, asset and
    /// mod of a large modpack is tens of thousands of entries, and a single
    /// directory that size is slow to enumerate on Windows and unwieldy to look
    /// at anywhere. Two characters is 256 directories, which is what Prism,
    /// Cargo and the Modrinth App all settled on for the same reason.
    pub fn path(&self, digest: &Digest) -> PathBuf {
        let hex = digest.hex();
        // `hex` is at least 40 characters by construction -- `Digest` cannot be
        // built any other way -- so the slice is always in bounds.
        self.dir.join(&hex[..2]).join(hex)
    }

    /// Whether the store holds a file under this digest.
    ///
    /// A path lookup and nothing else: no hashing, no reading. This is the check
    /// that makes "download only what is missing" cheap enough to ask for every
    /// file in a version.
    pub fn has(&self, digest: &Digest) -> bool {
        self.path(digest).is_file()
    }

    /// Whether the store holds these bytes *and* they are what the name says.
    ///
    /// The expensive question, asked when a file is about to be used rather than
    /// listed: a jar goes to the classpath only after this says yes.
    pub fn verified(&self, digest: &Digest) -> bool {
        digest.verify_file(&self.path(digest)).is_ok()
    }

    /// The bytes for `digest`, verified before they are handed over.
    ///
    /// Verified rather than read-and-hope, because the alternative is a corrupt
    /// jar on a classpath, and the check is a hash of a file that is about to be
    /// read anyway.
    pub fn read(&self, digest: &Digest) -> Result<Vec<u8>, Error> {
        let path = self.path(digest);
        digest.verify_file(&path)?;
        std::fs::read(&path).map_err(|error| Error::io(&path, error))
    }

    /// Store `bytes` under the name their digest gives them.
    ///
    /// The bytes are hashed first: a store that filed whatever it was given
    /// would be a store whose names mean nothing, and the whole value of a
    /// digest-named directory is that the name can be believed without reading
    /// the file.
    pub fn put(&self, digest: &Digest, bytes: &[u8]) -> Result<PathBuf, Error> {
        self.check(digest, bytes)?;
        let path = self.path(digest);
        write_atomic(&path, bytes)?;
        Ok(path)
    }

    /// Take a verified copy of `source` into the store, returning where it went.
    ///
    /// This is the join with the downloader: a transfer writes a file somewhere
    /// temporary, and this is what turns it into a stored one -- after checking
    /// it. A file that does not verify is refused, so the store cannot come to
    /// hold a lie, and a file already stored is left alone rather than rewritten
    /// (which is what makes two installs racing over the same jar harmless).
    ///
    /// A rename is used first and a copy fallback second, because the temporary
    /// directory and the store can be on different volumes and a rename across
    /// volumes fails on Windows.
    pub fn adopt(&self, digest: &Digest, source: &Path) -> Result<PathBuf, Error> {
        digest.verify_file(source)?;
        let path = self.path(digest);
        if path.is_file() {
            return Ok(path);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
        if std::fs::rename(source, &path).is_ok() {
            return Ok(path);
        }
        std::fs::copy(source, &path).map_err(|error| Error::io(&path, error))?;
        std::fs::remove_file(source).map_err(|error| Error::io(source, error))?;
        Ok(path)
    }

    /// Forget one stored file, returning whether there was one.
    ///
    /// Nothing else is touched. The store is shared and content-addressed, so it
    /// has no way to know whether another instance still wants these bytes; the
    /// caller that can know is the one that deletes instances, and this is the
    /// verb it needs.
    pub fn remove(&self, digest: &Digest) -> bool {
        std::fs::remove_file(self.path(digest)).is_ok()
    }

    /// How many files the store holds, and how many bytes that is.
    ///
    /// Walks the two-character directories, so it is a settings page's number
    /// rather than one that can be asked per file.
    pub fn stats(&self) -> (usize, u64) {
        let mut files = 0usize;
        let mut bytes = 0u64;
        let Ok(shards) = std::fs::read_dir(&self.dir) else {
            return (files, bytes);
        };
        for shard in shards.flatten() {
            // Only the fan-out directories are the store: `staging` is a sibling
            // of them, and a half-transferred file is not a stored one.
            let name = shard.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.len() != 2 || !name.bytes().all(is_hex_digit) {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(shard.path()) else {
                continue;
            };
            for entry in entries.flatten() {
                if entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
                    files += 1;
                    bytes += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
                }
            }
        }
        (files, bytes)
    }

    /// Where a transfer writes its file before it is one of ours.
    ///
    /// Under the store, so a completed transfer is a rename rather than a copy,
    /// and named for the digest, so two transfers racing over the same file
    /// continue each other's `.part` instead of colliding. The `.part` suffix is
    /// `fetch_to_file`'s: it appends to `<name>.part` and renames on success, so
    /// this path is what makes an interrupted install resumable across runs.
    pub fn staging_path(&self, digest: &Digest) -> PathBuf {
        self.dir.join("staging").join(digest.hex())
    }

    /// Delete staging files nobody has touched for `older_than`, returning how
    /// many went.
    ///
    /// A crashed run leaves a finished transfer that was never adopted and a
    /// `.part` that was being resumed, and both are worth keeping for a while --
    /// the point of the `.part` is that the next run continues it. "A while" is
    /// the caller's number rather than this module's: a launcher that is about to
    /// start a large install wants a long one, and one running out of disk wants
    /// zero.
    pub fn sweep_staging(&self, older_than: Duration) -> usize {
        let dir = self.dir.join("staging");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return 0;
        };
        let mut swept = 0usize;
        for entry in entries.flatten() {
            let stale = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                .map(|age| age >= older_than)
                // A file whose age cannot be read is not one to delete blindly.
                .unwrap_or(false);
            if stale && std::fs::remove_file(entry.path()).is_ok() {
                swept += 1;
            }
        }
        swept
    }

    /// Fetch `url` into the store under `digest`, if it is not there already.
    ///
    /// The whole of "download only what is missing", in one call: a store that
    /// has the file answers without a request, and one that does not transfers it
    /// to [`ContentStore::staging_path`], verifies it, and files it. A transfer
    /// that fails its digest is deleted rather than kept, because a resume would
    /// otherwise append to a prefix that can never verify -- the same rule
    /// `fetch_to_file` already applies to a download carrying its own digest.
    pub fn fetch(
        &self,
        fetch: &dyn Fetch,
        url: &str,
        digest: &Digest,
        cancel: &Cancel,
        backoff: &Backoff,
        sleep: &mut dyn FnMut(Duration),
    ) -> Result<Stored, Error> {
        if self.has(digest) {
            return Ok(Stored::AlreadyThere);
        }
        let staging = self.staging_path(digest);
        // No digest on the `Download`: this store verifies with the caller's
        // kind, and `fetch_to_file`'s own check is sha256-only. Two checks that
        // disagree about the algorithm would be one check too many.
        let download = Download::new(url, &staging);
        let downloaded = fetch_to_file(fetch, &download, cancel, backoff, sleep)?;
        if let Err(error) = self.adopt(digest, &staging) {
            let _ = std::fs::remove_file(&staging);
            return Err(error);
        }
        Ok(Stored::Fetched(downloaded.bytes()))
    }

    /// The same, with real waiting.
    pub fn fetch_blocking(
        &self,
        fetch: &dyn Fetch,
        url: &str,
        digest: &Digest,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Stored, Error> {
        let mut sleep = |wait: Duration| std::thread::sleep(wait);
        self.fetch(fetch, url, digest, cancel, backoff, &mut sleep)
    }

    /// Refuse bytes that are not what their digest says.
    fn check(&self, digest: &Digest, bytes: &[u8]) -> Result<(), Error> {
        if digest.matches(bytes) {
            return Ok(());
        }
        Err(Error::hash_mismatch(self.path(digest), digest.hex(), digest.of(bytes).hex()))
    }
}

/// Write `bytes` to `path` through a sibling temporary file.
///
/// A reader must never see half a file under a name that promises its contents.
/// The name carries the process id so two launchers sharing a store cannot
/// overwrite each other's half-written file. (`palantir_core::util::atomic_write`
/// does the same work, but reports through `palantir_core`'s error type and this
/// crate's errors are its own; see the same helper in `cache.rs`.)
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
    }
    let temp = temp_path(path);
    std::fs::write(&temp, bytes).map_err(|error| Error::io(&temp, error))?;
    if let Err(error) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(Error::io(path, error));
    }
    Ok(())
}

/// A sibling of `path` to write before renaming it into place.
fn temp_path(path: &Path) -> PathBuf {
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(format!(".part-{}", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::request::{MapFetch, Route};

    /// The well-known digests of the empty string and of `abc`, so the hashes
    /// are checked against something other than themselves.
    const EMPTY_SHA1: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const ABC_SHA1: &str = "a9993e364706816aba3e25717850c26c9cd0d89d";
    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const ABC_SHA512: &str = "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f";

    fn store_in(name: &str) -> ContentStore {
        let root = std::env::temp_dir().join("palantirmc-engine-content").join(name);
        let _ = std::fs::remove_dir_all(&root);
        ContentStore::new(root)
    }

    #[test]
    fn a_digest_is_read_from_what_the_service_published() {
        let sha1 = Digest::parse(ABC_SHA1).expect("a sha1");
        assert_eq!(sha1, Digest::Sha1(ABC_SHA1.to_string()));
        assert_eq!(sha1.kind(), "sha1");
        assert_eq!(sha1.hex(), ABC_SHA1);
        // Case and padding are the service's business, not the caller's.
        assert_eq!(
            Digest::parse(&format!("  {}  ", ABC_SHA1.to_uppercase())).expect("the same digest"),
            sha1
        );
        assert_eq!(Digest::parse(EMPTY_SHA256).expect("a sha256").kind(), "sha256");
        assert_eq!(Digest::parse(ABC_SHA512).expect("a sha512").kind(), "sha512");
    }

    #[test]
    fn a_digest_of_an_unknown_shape_is_refused_rather_than_guessed_at() {
        // 32 characters is MD5, which this launcher does not verify with: a
        // caller who hands one over has a bug, and a store that guessed would
        // hide it behind a file that verifies as something else.
        let error = Digest::parse("0123456789abcdef0123456789abcdef").expect_err("md5");
        assert!(format!("{error}").contains("sha1, sha256 or sha512"), "{error}");
        assert!(Digest::parse("not hex at all, but long enough to be looked at once").is_err());
        assert!(Digest::parse("").is_err());
    }

    #[test]
    fn the_hashes_are_the_published_ones() {
        assert_eq!(Digest::sha1(b"").hex(), EMPTY_SHA1);
        assert_eq!(Digest::sha256(b"").hex(), EMPTY_SHA256);
        assert_eq!(Digest::sha1(b"abc").hex(), ABC_SHA1);
        assert_eq!(Digest::sha256(b"abc").hex(), ABC_SHA256);
        assert_eq!(Digest::sha512(b"abc").hex(), ABC_SHA512);
        // And computing one *from* a digest that was read from somewhere uses
        // that digest's algorithm rather than a favourite of this file's.
        for text in [ABC_SHA1, ABC_SHA256, ABC_SHA512] {
            let published = Digest::parse(text).expect("a digest");
            assert_eq!(published.of(b"abc").hex(), published.hex());
            assert!(published.matches(b"abc"));
            assert!(!published.matches(b"abd"));
            assert_eq!(published.of(b"abc").kind(), published.kind());
        }
    }

    #[test]
    fn a_file_is_checked_against_its_own_name() {
        let store = store_in("verify");
        let path = store.dir().join("something.jar");
        std::fs::create_dir_all(store.dir()).expect("a directory");
        std::fs::write(&path, b"abc").expect("the file");

        Digest::sha1(b"abc").verify_file(&path).expect("sha1 matches");
        Digest::sha256(b"abc").verify_file(&path).expect("sha256 matches");
        Digest::sha512(b"abc").verify_file(&path).expect("sha512 matches");

        let error = Digest::sha1(b"abd").verify_file(&path).expect_err("a different digest");
        match error {
            Error::HashMismatch { expected, actual, .. } => {
                assert_eq!(expected, Digest::sha1(b"abd").hex());
                assert_eq!(actual, ABC_SHA1);
            }
            other => panic!("expected a hash mismatch, got {other:?}"),
        }
        // A file that is not there is a read failure, not a mismatch.
        assert!(matches!(
            Digest::sha1(b"abc").verify_file(&store.dir().join("absent")),
            Err(Error::Io { .. })
        ));
    }

    #[test]
    fn a_stored_file_is_found_by_a_path_lookup_and_lives_under_its_fan_out() {
        let store = store_in("put");
        let digest = Digest::sha256(b"a library");
        let path = store.put(&digest, b"a library").expect("stored");
        assert_eq!(path, store.path(&digest));
        assert_eq!(
            path.parent().and_then(|dir| dir.file_name()).and_then(|name| name.to_str()),
            Some(&digest.hex()[..2]),
            "two characters of fan-out"
        );
        assert!(store.has(&digest));
        assert!(store.verified(&digest));
        assert_eq!(store.read(&digest).expect("the bytes"), b"a library");
        assert_eq!(store.stats(), (1, 9));
        assert!(!store.has(&Digest::sha256(b"something else")));
    }

    #[test]
    fn a_store_refuses_to_file_bytes_that_are_not_what_they_are_named() {
        // The rule that makes the name worth believing. Without it the store is
        // a directory of files with opinions in their names.
        let store = store_in("refuse");
        let digest = Digest::sha256(b"the real bytes");
        let error = store.put(&digest, b"different bytes").expect_err("a lie");
        assert!(matches!(error, Error::HashMismatch { .. }), "{error:?}");
        assert!(!store.has(&digest), "and nothing was written");
        assert_eq!(store.stats(), (0, 0));

        // A file that was stored and then damaged is caught where it is used.
        store.put(&digest, b"the real bytes").expect("stored");
        let path = store.path(&digest);
        std::fs::write(&path, b"the real bytes!").expect("damaged");
        assert!(store.has(&digest), "the name is still there");
        assert!(!store.verified(&digest), "but the contents are not");
        assert!(matches!(store.read(&digest), Err(Error::HashMismatch { .. })));
    }

    #[test]
    fn a_downloaded_file_is_adopted_only_after_it_verifies() {
        let store = store_in("adopt");
        let digest = Digest::sha256(b"a jar");
        let good = store.dir().join("incoming");
        std::fs::create_dir_all(store.dir()).expect("a directory");
        std::fs::write(&good, b"a jar").expect("the file");

        let path = store.adopt(&digest, &good).expect("adopted");
        assert_eq!(path, store.path(&digest));
        assert!(store.verified(&digest));
        assert!(!good.exists(), "it was moved, not copied");
        // Adopting the same bytes again leaves the stored file alone.
        let again = store.dir().join("incoming-2");
        std::fs::write(&again, b"a jar").expect("the file");
        assert_eq!(store.adopt(&digest, &again).expect("adopted again"), path);

        // A file that does not verify is refused and left where it was, because
        // the caller may still be able to resume it.
        let corrupt = store.dir().join("corrupt");
        std::fs::write(&corrupt, b"not a jar").expect("the file");
        let error = store.adopt(&Digest::sha256(b"a jar"), &corrupt).expect_err("a mismatch");
        assert!(matches!(error, Error::HashMismatch { .. }), "{error:?}");
        assert!(corrupt.exists());
    }

    #[test]
    fn fetching_only_asks_for_what_is_missing() {
        let store = store_in("fetch");
        let body = b"a whole library, more or less".to_vec();
        let digest = Digest::sha256(&body);
        let fetch = MapFetch::new().with_route("https://cdn.invalid/lib.jar", Route::body(body.clone()));

        let first = store
            .fetch_blocking(&fetch, "https://cdn.invalid/lib.jar", &digest, &Cancel::new(), &Backoff::with_attempts(1))
            .expect("fetched");
        assert_eq!(first, Stored::Fetched(body.len() as u64));
        assert_eq!(fetch.count(), 1);
        assert!(store.verified(&digest));
        assert_eq!(store.read(&digest).expect("the bytes"), body);

        // The second look is a path lookup: the fetcher is never asked, which is
        // what makes a reinstall of the same version mostly a disk read.
        let second = store
            .fetch_blocking(&fetch, "https://cdn.invalid/lib.jar", &digest, &Cancel::new(), &Backoff::with_attempts(1))
            .expect("not fetched");
        assert_eq!(second, Stored::AlreadyThere);
        assert_eq!(fetch.count(), 1);
        assert!(!store.staging_path(&digest).exists(), "and nothing was left staging");
    }

    #[test]
    fn a_transfer_that_does_not_verify_is_deleted_rather_than_filed() {
        // A server that answers with the wrong bytes: the file must not enter
        // the store, and the staging copy must not stay behind -- a resume would
        // append to a prefix that can never verify, and the next attempt would
        // resume onto it again.
        let store = store_in("bad-transfer");
        let digest = Digest::sha256(b"the expected bytes");
        let fetch = MapFetch::new().with_route("u", Route::text("the wrong bytes"));

        let error = store
            .fetch_blocking(&fetch, "u", &digest, &Cancel::new(), &Backoff::with_attempts(1))
            .expect_err("a mismatch");
        assert!(matches!(error, Error::HashMismatch { .. }), "{error:?}");
        assert!(!store.has(&digest));
        assert_eq!(store.stats(), (0, 0));
        assert!(!store.staging_path(&digest).exists(), "the staging file went with it");
    }

    #[test]
    fn an_interrupted_transfer_leaves_a_part_file_the_next_run_continues() {
        // The store's staging path is what makes resume survive a restart: the
        // `.part` is under the digest's own name, so a second run finds it
        // without being told what it was doing.
        let store = store_in("resume");
        let body = vec![b'z'; 4096];
        let digest = Digest::sha256(&body);
        let cancel = Cancel::new();
        let fetch = MapFetch::new().with_route(
            "u",
            Route::body(body.clone()).chunked(512).stopping_after(2, cancel.clone()),
        );

        let error = store
            .fetch_blocking(&fetch, "u", &digest, &cancel, &Backoff::with_attempts(1))
            .expect_err("cancelled part way");
        assert!(matches!(error, Error::Cancelled), "{error:?}");
        let staging = store.staging_path(&digest);
        let part = crate::download::part_path(&staging);
        assert_eq!(
            std::fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0),
            1024,
            "two chunks of the body are on disk"
        );

        // The next run continues from there and files the whole thing.
        let whole = MapFetch::new().with_route("u", Route::body(body.clone()));
        let stored = store
            .fetch_blocking(&whole, "u", &digest, &Cancel::new(), &Backoff::with_attempts(1))
            .expect("resumed");
        assert_eq!(stored, Stored::Fetched(4096 - 1024), "only the rest came over the wire");
        assert!(store.verified(&digest));
        assert!(!staging.exists());
    }

    #[test]
    fn staging_files_are_swept_by_age_and_a_part_file_is_kept_until_it_is_old() {
        let store = store_in("sweep");
        let digest = Digest::sha256(b"half of something");
        let staging = store.staging_path(&digest);
        std::fs::create_dir_all(staging.parent().expect("a staging directory")).expect("mkdir");
        std::fs::write(&staging, b"half").expect("the file");

        assert_eq!(store.sweep_staging(Duration::from_secs(3600)), 0, "it is fresh");
        assert!(staging.exists(), "a resume is worth keeping");
        assert_eq!(store.sweep_staging(Duration::ZERO), 1, "asked and answered");
        assert!(!staging.exists());
        // Nothing to sweep is not an error.
        assert_eq!(store.sweep_staging(Duration::ZERO), 0);
    }

    #[test]
    fn a_stored_file_can_be_forgotten_without_touching_the_rest() {
        let store = store_in("remove");
        let one = Digest::sha256(b"one");
        let two = Digest::sha256(b"two");
        store.put(&one, b"one").expect("stored");
        store.put(&two, b"two").expect("stored");
        assert_eq!(store.stats(), (2, 6));

        assert!(store.remove(&one));
        assert!(!store.remove(&one), "it was already gone");
        assert_eq!(store.stats(), (1, 3));
        assert!(store.has(&two));
        assert!(store.verified(&two));
    }
}
