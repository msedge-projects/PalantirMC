//! Java runtime handling — port of `java/JavaVersion.cpp` plus the
//! file-based part of `java/JavaUtils.cpp` (install scanning and
//! `release` file parsing).
//!
//! Running `java` to probe unknown runtimes is a phase-2 (`prism-net`) /
//! launch-step concern and is intentionally out of scope here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A parsed Java version string (`1.8.0_412`, `17.0.2`, `21`, ...).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JavaVersion {
    /// Raw string.
    pub raw: String,
    major_raw: i64,
    minor: i64,
    security: i64,
    update: i64,
    prerelease: String,
    parseable: bool,
}

fn parse_int(s: &str) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    s.parse::<i64>().ok()
}

impl JavaVersion {
    /// Parse a version string, mirroring `JavaVersion::operator=` in
    /// `java/JavaVersion.cpp`. Unknown segments read as 0 (matching the
    /// `toInt()` fallbacks in the C++). `1.x` versions are normalized so
    /// `1.8.0_412` reports major 8, minor 0, security/update 412.
    /// A `-suffix` (e.g. `21-ea`) is a prerelease and still yields major 21.
    pub fn parse(s: &str) -> JavaVersion {
        let mut v = JavaVersion { raw: s.to_string(), ..Default::default() };
        // Split off `-prerelease` first (Prism only recognizes `-`, but also
        // tolerate `+` build metadata by ignoring it for the numeric parts).
        let (no_dash, dash_pre) = match s.split_once('-') {
            Some((h, t)) => (h, t),
            None => (s, ""),
        };
        // Prerelease is the leading alphanumeric run after `-`.
        let pre: String = dash_pre.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        v.prerelease = pre;
        // Ignore `+build` metadata for the numeric parse.
        let no_build = no_dash.split('+').next().unwrap_or(no_dash);
        if let Some(rest) = no_build.strip_prefix("1.") {
            // Legacy `1.<major>.<minor>_<security>` form.
            let (head, tail) = match rest.split_once('_') {
                Some((h, t)) => (h, t),
                None => (rest, ""),
            };
            // Tail may still carry a stray `+`/`-`; take the numeric prefix.
            let tail_num = tail.split(['-', '+']).next().unwrap_or(tail);
            let upd = parse_int(tail_num).unwrap_or(0);
            v.update = upd;
            v.security = upd;
            let mut it = head.split('.');
            let major_str = it.next().unwrap_or("");
            v.major_raw = parse_int(major_str).unwrap_or(0);
            v.minor = it.next().and_then(parse_int).unwrap_or(0);
            // Legacy has no third dot component; any extra is ignored.
            v.parseable = parse_int(major_str).is_some();
        } else {
            // Modern `<major>.<minor>.<security>` form.
            let (head, tail) = match no_build.split_once('_') {
                Some((h, t)) => (h, t),
                None => (no_build, ""),
            };
            let tail_num = tail.split(['-', '+']).next().unwrap_or(tail);
            v.update = parse_int(tail_num).unwrap_or(0);
            let mut it = head.split('.');
            let major_str = it.next().unwrap_or("");
            v.major_raw = parse_int(major_str).unwrap_or(0);
            v.minor = it.next().and_then(parse_int).unwrap_or(0);
            v.security = it.next().and_then(parse_int).unwrap_or(0);
            v.parseable = parse_int(major_str).is_some();
        }
        v
    }

    /// Major version with legacy `1.x` normalization (`JavaVersion::major`):
    /// `1.8.0_412` reports 8. Parsing already normalizes, so this is the
    /// stored major.
    pub fn major(&self) -> i64 {
        self.major_raw
    }

    /// Java 7 or older needs `-XX:PermSize` (`requiresPermGen`).
    pub fn requires_perm_gen(&self) -> bool {
        !self.parseable || self.major() < 8
    }

    /// Java 9+ is modular (`isModular`).
    pub fn is_modular(&self) -> bool {
        self.parseable && self.major() >= 9
    }

    /// Ordered comparison matching Prism `operator<`/`operator==`: numeric
    /// (major, minor, security, update), then prerelease (a prerelease sorts
    /// before the release; two prereleases compare lexically). Unparseable
    /// versions fall back to raw string comparison.
    pub fn compare(&self, other: &JavaVersion) -> std::cmp::Ordering {
        if self.parseable && other.parseable {
            let ord = (self.major(), self.minor, self.security, self.update)
                .cmp(&(other.major(), other.minor, other.security, other.update));
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
            match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => self.prerelease.cmp(&other.prerelease),
            }
        } else {
            self.raw.cmp(&other.raw)
        }
    }
}

impl PartialOrd for JavaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JavaVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.compare(other)
    }
}

/// One detected Java installation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JavaInstall {
    /// JAVA_HOME style directory.
    pub home: PathBuf,
    /// Parsed version, when the `release` file could be read.
    pub version: Option<JavaVersion>,
    /// `JAVA_VENDOR` from the release file.
    pub vendor: Option<String>,
    /// `OS_ARCH` from the release file.
    pub architecture: Option<String>,
}

impl JavaInstall {
    /// Real architecture, normalized like `RuntimeContext`
    /// (`amd64` -> `x86_64` etc.).
    pub fn real_architecture(&self) -> Option<String> {
        self.architecture.as_deref().map(|a| match a {
            "amd64" => "x86_64".to_string(),
            "i386" | "i686" => "x86".to_string(),
            "aarch64" => "arm64".to_string(),
            "arm" | "armhf" => "arm32".to_string(),
            other => other.to_string(),
        })
    }
}

/// Parse a `release` file's key/value lines into a map. Handles the UTF-16LE
/// encoding used by JDKs on Windows.
pub fn parse_release_file(bytes: &[u8]) -> BTreeMap<String, String> {
    let text = if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        let value = value.strip_prefix('"').unwrap_or(value);
        let value = value.strip_suffix('"').unwrap_or(value);
        map.insert(key.trim().to_string(), value.to_string());
    }
    map
}

/// Probe a candidate JAVA_HOME directory: requires an executable in
/// `bin/` and best-effort parses the `release` file.
pub fn probe_home(dir: &Path) -> Option<JavaInstall> {
    let exe = if cfg!(windows) { "bin/java.exe" } else { "bin/java" };
    if !dir.join(exe).exists() {
        return None;
    }
    let mut install = JavaInstall { home: dir.to_path_buf(), ..Default::default() };
    if let Ok(bytes) = std::fs::read(dir.join("release")) {
        let map = parse_release_file(&bytes);
        if let Some(v) = map.get("JAVA_VERSION") {
            install.version = Some(JavaVersion::parse(v));
        }
        install.vendor = map.get("JAVA_VENDOR").cloned();
        install.architecture = map.get("OS_ARCH").cloned();
    }
    Some(install)
}

/// Common parent directories to scan for Java installs per platform
/// (`JavaUtils` defaults, without registry/`/usr/libexec/java_home` lookups,
/// which arrive with the phase-2 network/system work).
pub fn scan_roots(system: crate::paths::System) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    match system {
        crate::paths::System::Windows => {
            for var in ["ProgramFiles", "ProgramFiles(x86)"] {
                if let Some(pf) = std::env::var_os(var) {
                    let pf = PathBuf::from(pf);
                    roots.push(pf.join("Java"));
                    roots.push(pf.join("Eclipse Adoptium"));
                    roots.push(pf.join("Microsoft"));
                }
            }
            if let Some(lappdata) = std::env::var_os("LOCALAPPDATA") {
                roots.push(PathBuf::from(lappdata).join("Programs"));
            }
        }
        crate::paths::System::Linux => {
            roots.push(PathBuf::from("/usr/lib/jvm"));
            roots.push(PathBuf::from("/usr/java"));
        }
        crate::paths::System::MacOS => {
            for base in [
                PathBuf::from("/Library/Java/JavaVirtualMachines"),
                dirs_home().join("Library/Java/JavaVirtualMachines"),
            ] {
                roots.push(base);
            }
        }
    }
    roots.retain(|p| p.is_dir());
    roots
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Scan the standard roots for Java homes (directory-level probe only).
pub fn scan_installs(system: crate::paths::System) -> Vec<JavaInstall> {
    let mut out = Vec::new();
    for root in scan_roots(system) {
        let entries = match std::fs::read_dir(&root) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let home = if path.join("Contents/Home").exists() {
                path.join("Contents/Home")
            } else {
                path.clone()
            };
            if let Some(install) = probe_home(&home) {
                out.push(install);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::System;
    use std::cmp::Ordering;

    #[test]
    fn parse_legacy_and_modern_versions() {
        let v = JavaVersion::parse("1.8.0_412");
        assert_eq!(v.major(), 8);
        assert_eq!(v.update, 412);
        assert!(!v.requires_perm_gen()); // PermGen was removed in Java 8
    }

    #[test]
    fn permgen_and_modular_boundaries() {
        assert!(JavaVersion::parse("1.7.0_80").requires_perm_gen());
        assert!(!JavaVersion::parse("1.8.0_412").requires_perm_gen());
        assert!(!JavaVersion::parse("8").requires_perm_gen());
        assert!(JavaVersion::parse("9").is_modular());
        assert!(JavaVersion::parse("17.0.2").is_modular());
        assert!(!JavaVersion::parse("1.8.0_412").is_modular());
    }

    #[test]
    fn version_ordering() {
        assert_eq!(JavaVersion::parse("17.0.2").compare(&JavaVersion::parse("17.0.1")), Ordering::Greater);
        assert_eq!(JavaVersion::parse("1.8.0_412").compare(&JavaVersion::parse("1.8.0_282")), Ordering::Greater);
        assert_eq!(JavaVersion::parse("8").compare(&JavaVersion::parse("1.8.0")), Ordering::Equal);
        assert_eq!(JavaVersion::parse("21").compare(&JavaVersion::parse("17.0.9")), Ordering::Greater);
    }

    #[test]
    fn garbage_segments_read_as_zero() {
        let v = JavaVersion::parse("21-ea");
        assert_eq!(v.major(), 21);
        let v = JavaVersion::parse("");
        assert_eq!(v.major(), 0);
        assert!(!v.is_modular());
    }

    #[test]
    fn release_file_parsing_utf8_and_utf16() {
        let utf8 = b"JAVA_VERSION=\"17.0.2\"\nJAVA_VENDOR=\"Eclipse Adoptium\"\nOS_ARCH=\"x86_64\"\n";
        let map = parse_release_file(utf8);
        assert_eq!(map["JAVA_VERSION"], "17.0.2");
        assert_eq!(map["OS_ARCH"], "x86_64");
        let mut utf16 = vec![0xFF, 0xFE];
        for unit in "JAVA_VERSION=\"21\"\n".encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        let map = parse_release_file(&utf16);
        assert_eq!(map["JAVA_VERSION"], "21");
    }

    #[test]
    fn probe_home_requires_executable_and_reads_release() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("jdk-17");
        std::fs::create_dir_all(home.join("bin")).unwrap();
        assert!(probe_home(&home).is_none()); // no binary yet
        #[cfg(windows)]
        let exe = home.join("bin/java.exe");
        #[cfg(not(windows))]
        let exe = home.join("bin/java");
        std::fs::write(&exe, b"").unwrap();
        std::fs::write(home.join("release"), b"JAVA_VERSION=\"17.0.2\"\nOS_ARCH=\"aarch64\"\n").unwrap();
        let install = probe_home(&home).unwrap();
        assert_eq!(install.version.as_ref().unwrap().major(), 17);
        assert_eq!(install.real_architecture().as_deref(), Some("arm64"));
    }

    #[test]
    fn scan_roots_only_return_existing_dirs() {
        // The host may or may not have JVM dirs; the contract is just that
        // every returned path exists and probing does not panic.
        for root in scan_roots(System::current()) {
            assert!(root.is_dir(), "{root:?} reported but missing");
        }
    }
}
