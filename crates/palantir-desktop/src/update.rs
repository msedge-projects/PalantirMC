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
//!   `Sift Launcher.exe` from a payload called `PalantirMC.exe`
//!   (`File /oname=`). An updater that looks for `PalantirMC.exe` in the install
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
fn parts(version: &str) -> Option<Vec<u64>> {
    let version = version.trim();
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
}
