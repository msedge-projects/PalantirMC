//! What a self-update would have to be, and the one part of it a test can run.
//!
//! **Nothing calls this yet.** The launcher has no updater, and the decision
//! recorded in `NOTES.md` is that writing one is a change with risks that a
//! session without a Windows host cannot close. What is here is the piece that is
//! pure arithmetic -- whether an offered version is newer than the running one --
//! because that is the only part of the design that can be held by a test offline,
//! and the alternative (the whole design in prose, no code) leaves the one
//! falsifiable claim unwritten.
//!
//! ## The shape an updater would have
//!
//! 1. **Ask** a signed manifest for the newest version: `{ version, url, sha256,
//!    notes }`, fetched over HTTPS from a host the launcher trusts.
//! 2. **Compare** with `env!("CARGO_PKG_VERSION")` -- [`is_newer`] -- and do
//!    nothing if it is not newer.
//! 3. **Download** the installer to a temporary file beside its destination, and
//!    verify **both** the manifest's `sha256` and the file's size before anything
//!    is run. A truncated download that is executed is the failure mode that
//!    matters: an installer half on disk can still update the registry and then
//!    fail mid-copy.
//! 4. **Run** it with the launcher closed, then exit. Windows will not let a
//!    running executable be replaced, so an updater that overwrites in place is
//!    an updater that does nothing; the installer already knows how to replace an
//!    installed copy.
//!
//! ## The risks, written down where they cannot be forgotten
//!
//! * **Trust.** An updater turns "the launcher is running" into "whatever this
//!   server says runs with the user's privileges". The manifest must be signed
//!   with a key compiled into the binary, and the signature must cover the
//!   version, the URL and the digest together -- otherwise a host that serves one
//!   field can redirect the download.
//! * **The rename.** `installer/sift.nsi` installs the exe as
//!   `Sift Launcher.exe` from a payload called `SiftLauncher.exe`
//!   (`File /oname=`). An updater that looks for `SiftLauncher.exe` in the install
//!   folder updates nothing, because that name is the *payload's*, not the
//!   installed file's. Either the build adopts the installed name or the updater
//!   is told it.
//! * **Rollback.** The previous executable has to survive until the new one has
//!   started once, or a bad release is a launcher that cannot be opened to fetch
//!   the good one.
//! * **No host to try it on.** Every step after the comparison needs a Windows
//!   machine with an installed copy, an older version to replace and network
//!   access to a server. None of that exists here, which is why the comparison is
//!   the only part that ships.

/// Whether `offered` names a version later than `current`.
///
/// Dotted numbers, compared field by field as numbers rather than as strings --
/// `0.1.10` is later than `0.1.9`, and a string comparison says the opposite.
/// Missing fields are zero, so `0.2` and `0.2.0` are the same version and neither
/// is newer than the other. A leading `v` is accepted on either side because tags
/// and manifests both use it; anything that is not a dotted number is not a
/// version, and an unparseable pair answers `false` -- a launcher must never
/// decide to replace itself on a string it could not read.
pub fn is_newer(offered: &str, current: &str) -> bool {
    let (Some(offered), Some(current)) = (parts(offered), parts(current)) else {
        return false;
    };
    let fields = offered.len().max(current.len());
    for index in 0..fields {
        let offered = offered.get(index).copied().unwrap_or(0);
        let current = current.get(index).copied().unwrap_or(0);
        if offered != current {
            return offered > current;
        }
    }
    false
}

/// A version's numeric fields, or `None` when the string is not one.
///
/// Four fields at most: a dotted number with more of them is not a version this
/// launcher ships, and a longer one is refused rather than half-read.
fn parts(version: &str) -> Option<Vec<u64>> {    let version = version.trim();
    let version = version.strip_prefix('v').unwrap_or(version);
    if version.is_empty() || version.len() > 32 {
        return None;
    }
    let fields: Vec<u64> = version
        .split('.')
        .map(|field| field.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()?;
    if fields.len() > 4 {
        return None;
    }
    Some(fields)
}

/// One release, as a manifest would describe it.
///
/// Four fields and no more: what the launcher needs in order to decide whether to
/// fetch a file, and then to check that the file it fetched is the one described.
/// The notes a release carries are not here because nothing would draw them yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The version the release is.
    pub version: String,
    /// Where its bytes are.
    pub url: String,
    /// The lowercase hex `sha256` of those bytes.
    pub sha256: String,
    /// How many bytes there are, so a truncated transfer is caught by a number
    /// rather than by hashing half an installer.
    pub size: u64,
}

/// Why `offer` should not be acted on, or `None` when it may be.
///
/// The four tests are the ones a manifest has to pass *before* the launcher fetches
/// anything, and each one exists because of a way an updater goes wrong rather than
/// because a field was in the way:
///
/// * a version that is not newer is nothing to do, and re-installing the same
///   release would be a loop;
/// * a URL that is not `https` is a download a network in the middle can rewrite --
///   the digest would catch it, but only after fetching whatever was served;
/// * a digest that is not 64 hex characters cannot catch anything, and an updater
///   with no digest is an updater that runs whatever it was handed;
/// * a size of zero means the manifest did not say, and the cheapest check there is
///   -- the byte count -- would be skipped.
pub fn offer_problem(offer: &Offer, current: &str) -> Option<String> {
    if !is_newer(&offer.version, current) {
        return Some(format!(
            "{} is not newer than {current}",
            offer.version.trim()
        ));
    }
    if !offer.url.starts_with("https://") {
        return Some(format!("{} is not an https address", offer.url));
    }
    let digest = offer.sha256.trim();
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Some("the manifest's sha256 is not 64 hex characters".to_string());
    }
    if offer.size == 0 {
        return Some("the manifest gives no size for the download".to_string());
    }
    None
}

/// Check a downloaded file against what the manifest said: size, then digest.
///
/// **Size first**, because it is one `stat` and a truncated transfer is the common
/// way a download goes wrong -- hashing a half file to discover it is half a file
/// spends the disk read for an answer already available.
///
/// The digest is `sift_net::download`'s, which streams the file in fixed chunks
/// rather than reading it into memory, and compares case-insensitively. Nothing
/// here writes, and nothing here runs anything: this is the check that stands
/// between a download and the installer that would replace this launcher.
pub fn verify_download(path: &std::path::Path, size: u64, sha256: &str) -> Result<(), String> {
    let found = std::fs::metadata(path)
        .map_err(|error| format!("{} could not be read: {error}", path.display()))?
        .len();
    if found != size {
        return Err(format!(
            "{} is {found} bytes where the manifest said {size}",
            path.display()
        ));
    }
    sift_net::download::verify_sha256(path, sha256).map_err(|error| {
        format!(
            "{} is not the file the manifest described: {error}",
            path.display()
        )
    })
}

/// Move a verified download onto `target`, or refuse and leave `target` alone.
///
/// The order is the whole function: **verify, then rename**. A rename that happened
/// first would have replaced the installed launcher with a file nobody checked, and
/// the check would then be checking the wrong copy.
///
/// `std::fs::rename` is atomic within one volume, which is why the download belongs
/// beside its destination rather than in a temporary directory: a rename across
/// volumes is a copy, and a copy that fails half way leaves a file where the
/// launcher used to be. This function cannot promise the caller downloaded beside
/// the target -- that is the caller's rule -- but it can promise nothing is moved
/// before it is checked.
pub fn install_after_verify(
    temp: &std::path::Path,
    target: &std::path::Path,
    size: u64,
    sha256: &str,
) -> Result<(), String> {
    verify_download(temp, size, sha256)?;
    std::fs::rename(temp, target).map_err(|error| {
        format!(
            "{} was checked but could not be moved to {}: {error}",
            temp.display(),
            target.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_dotted_number_is_newer_and_an_earlier_one_is_not() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("0.1.0", "0.1.0"), "the same version is not newer");
    }

    /// The reason this is numbers and not strings: lexicographic order puts
    /// `10` before `9`, and a launcher that offered nothing on the tenth release
    /// would be quietly stuck.
    #[test]
    fn the_fields_are_compared_as_numbers() {
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.9", "0.1.10"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
    }

    #[test]
    fn a_missing_field_is_a_zero_and_a_deeper_one_is_later() {
        assert!(!is_newer("0.2", "0.2.0"));
        assert!(!is_newer("0.2.0", "0.2"));
        assert!(is_newer("0.2.1", "0.2"));
        assert!(is_newer("0.2.0.1", "0.2"));
    }

    #[test]
    fn a_tag_prefix_is_accepted() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.2.0", "v0.1.0"));
        assert!(!is_newer("v0.1.0", "v0.1.0"));
    }

    /// A version the launcher cannot read is never grounds for replacing itself.
    #[test]
    fn what_is_not_a_version_is_never_newer() {
        for junk in ["", " ", "latest", "0.1.x", "0..1", "0.1.-1", "0.1.0.0.0.1"] {
            assert!(!is_newer(junk, "0.1.0"), "{junk:?}");
            assert!(!is_newer("0.2.0", junk), "{junk:?}");
        }
        // And the length ceiling refuses a string that is all digits but absurd.
        assert!(!is_newer(&"1".repeat(33), "0.1.0"));
    }

    /// A complete manifest for a newer release, which each test then breaks once.
    fn offer() -> Offer {
        Offer {
            version: "0.2.0".to_string(),
            url: "https://example.invalid/sift-setup-0.2.0.exe".to_string(),
            sha256: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08".to_string(),
            size: 12_345,
        }
    }

    #[test]
    fn a_complete_offer_for_a_newer_release_has_no_problem() {
        assert_eq!(offer_problem(&offer(), "0.1.0"), None);
        // The `v` a tag carries is accepted on the manifest's side too.
        let tagged = Offer {
            version: "v0.2.0".to_string(),
            ..offer()
        };
        assert_eq!(offer_problem(&tagged, "0.1.0"), None);
    }

    /// Each of the four refusals, one field at a time.
    #[test]
    fn an_offer_is_refused_for_each_thing_that_would_make_it_unsafe() {
        let same = offer_problem(&offer(), "0.2.0").expect("the same version is nothing to do");
        assert!(same.contains("not newer"), "{same}");
        let older = offer_problem(&offer(), "0.3.0").expect("an older release is nothing to do");
        assert!(older.contains("not newer"), "{older}");

        let plaintext = Offer {
            url: "http://example.invalid/x.exe".to_string(),
            ..offer()
        };
        let problem = offer_problem(&plaintext, "0.1.0").expect("http is refused");
        assert!(problem.contains("https"), "{problem}");

        for digest in ["", "abc", &"z".repeat(64), &"a".repeat(63)] {
            let bad = Offer {
                sha256: digest.to_string(),
                ..offer()
            };
            assert!(
                offer_problem(&bad, "0.1.0").is_some(),
                "{digest:?} must not pass as a digest"
            );
        }

        let sizeless = Offer {
            size: 0,
            ..offer()
        };
        assert!(offer_problem(&sizeless, "0.1.0").is_some(), "no size");
    }

    /// A scratch file holding `bytes`, in a directory of its own.
    fn scratch(name: &str, bytes: &[u8]) -> (std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir()
            .join("sift-update-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        let path = root.join("download.exe");
        std::fs::write(&path, bytes).expect("a scratch file");
        (root, path)
    }

    #[test]
    fn a_download_that_matches_its_size_and_digest_is_accepted() {
        let bytes = b"an installer";
        let (root, path) = scratch("verify-ok", bytes);
        let digest = sift_net::download::sha256_hex(bytes);
        assert_eq!(verify_download(&path, bytes.len() as u64, &digest), Ok(()));
        // The manifest's own case does not matter, and neither does its whitespace:
        // `verify_sha256` trims and lowercases, and this must not undo that.
        assert_eq!(
            verify_download(&path, bytes.len() as u64, &digest.to_uppercase()),
            Ok(())
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The size is checked before the digest, and its message says both numbers.
    #[test]
    fn a_truncated_download_is_refused_by_size_and_says_so() {
        let bytes = b"an installer";
        let (root, path) = scratch("verify-short", bytes);
        let digest = sift_net::download::sha256_hex(bytes);
        let expected = bytes.len() as u64 + 10;
        let problem =
            verify_download(&path, expected, &digest).expect_err("a short file must be refused");
        assert!(
            problem.contains(&format!("{expected}")),
            "the message names what the manifest said: {problem}"
        );
        assert!(
            problem.contains(&format!("{} bytes", bytes.len())),
            "and what was actually found: {problem}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_download_of_the_right_size_and_the_wrong_bytes_is_refused() {
        let (root, path) = scratch("verify-wrong", b"an installer");
        let other = sift_net::download::sha256_hex(b"a different installer");
        let problem = verify_download(&path, 12, &other).expect_err("a wrong digest must be refused");
        assert!(problem.contains("not the file"), "{problem}");
        // And a digest that is not a digest at all is a refusal rather than a pass.
        assert!(verify_download(&path, 12, "not-a-digest").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Verified, then moved: the target appears only after the check, and a bad
    /// download leaves the target exactly as it was.
    #[test]
    fn a_verified_download_is_moved_and_an_unverified_one_is_not() {
        let bytes = b"an installer";
        let digest = sift_net::download::sha256_hex(bytes);
        let (root, temp) = scratch("install-ok", bytes);
        let target = root.join("SiftLauncher.exe");
        std::fs::write(&target, b"the old launcher").expect("a stand-in launcher");

        install_after_verify(&temp, &target, bytes.len() as u64, &digest).expect("a good download");
        assert_eq!(
            std::fs::read(&target).expect("the target"),
            bytes,
            "the new bytes are in place"
        );
        assert!(!temp.exists(), "the temporary file was moved, not copied");

        // The same thing with a corrupt download: the installed file is untouched.
        let (root, temp) = scratch("install-bad", b"a corrupted installer!");
        let target = root.join("SiftLauncher.exe");
        std::fs::write(&target, b"the old launcher").expect("a stand-in launcher");
        assert!(install_after_verify(&temp, &target, bytes.len() as u64, &digest).is_err());
        assert_eq!(
            std::fs::read(&target).expect("the target"),
            b"the old launcher",
            "a download that fails its check must not replace anything"
        );
        assert!(temp.exists(), "the refused download is left for inspection");
        let _ = std::fs::remove_dir_all(&root);
    }
}
