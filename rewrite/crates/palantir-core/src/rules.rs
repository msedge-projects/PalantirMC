//! Deciding what applies on this machine.
//!
//! Metadata gates arguments and libraries behind rules. The semantics, read
//! off how the format's own documents use them:
//!
//! - a rule matches when every condition it states matches (a condition it
//!   does not state is satisfied);
//! - a list of rules resolves to the **last matching** rule's action;
//! - a list nothing matches resolves to **disallow**. (That is why real
//!   lists wanting a platform exclusion open with an unconditional
//!   `{"action": "allow"}` -- the `lwjgl` entries in the 1.12.2 sample -- and
//!   allow-lists like `{"action": "allow", "os": {"name": "osx"}}` need no
//!   companion rule.)
//!
//! OS version conditions come in two shapes: `version`, a regular expression
//! matched anywhere in the OS version string, and `versionRange`, with `min`
//! inclusive and `max` exclusive. The 26.3 sample's boundary pair -- ZGC at
//! `min: 10.0.17134`, G1 at `max: 10.0.17134` -- partitions cleanly only
//! with an exclusive max, which is how this reads it.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use regex::Regex;

use crate::error::{Error, Result};
use crate::version::{OsRule, Rule, RuleAction, VersionRange};

/// The machine a rule is asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    /// Whatever the OS reports as its version (Windows: `10.0.19045`), the
    /// string regex and range rules are matched against.
    pub os_version: String,
    pub arch: Arch,
    /// Launcher-level switches metadata can gate on (`is_demo_user`,
    /// `has_custom_resolution`, ...).
    pub features: BTreeMap<String, bool>,
}

impl Platform {
    /// A platform description with no features enabled.
    pub fn new(os: Os, os_version: impl Into<String>, arch: Arch) -> Self {
        Self {
            os,
            os_version: os_version.into(),
            arch,
            features: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
    /// Anything else: matches no OS-named condition.
    Other,
}

impl Os {
    /// The format's name for this OS.
    pub fn mojang_name(self) -> Option<&'static str> {
        match self {
            Os::Windows => Some("windows"),
            Os::MacOs => Some("osx"),
            Os::Linux => Some("linux"),
            Os::Other => None,
        }
    }

    /// The running system.
    pub fn host() -> Self {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            Os::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86,
    X86_64,
    Aarch64,
    Other,
}

impl Arch {
    /// The format names only 32-bit x86, in rules gating 32-bit Java.
    pub fn mojang_name(self) -> Option<&'static str> {
        match self {
            Arch::X86 => Some("x86"),
            _ => None,
        }
    }

    /// What `${arch}` becomes in a natives classifier: `32` or `64`.
    pub fn natives_suffix(self) -> &'static str {
        match self {
            Arch::X86 => "32",
            _ => "64",
        }
    }

    /// The running system.
    pub fn host() -> Self {
        if cfg!(target_pointer_width = "32") {
            Arch::X86
        } else if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else if cfg!(target_pointer_width = "64") {
            Arch::X86_64
        } else {
            Arch::Other
        }
    }
}

/// Resolve a rule list: last match wins, nothing matches means disallow,
/// and an empty list allows (it is what an absent `rules` key means).
pub fn rules_allow(rules: &[Rule], platform: &Platform) -> Result<bool> {
    if rules.is_empty() {
        return Ok(true);
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, platform)? {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    Ok(allowed)
}

/// Does one rule apply here? Every condition it states must match.
pub fn rule_matches(rule: &Rule, platform: &Platform) -> Result<bool> {
    if let Some(os) = &rule.os {
        if !os_matches(os, platform)? {
            return Ok(false);
        }
    }
    if let Some(features) = &rule.features {
        for (name, wanted) in features {
            if platform.features.get(name) != Some(wanted) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn os_matches(os: &OsRule, platform: &Platform) -> Result<bool> {
    if let Some(name) = &os.name {
        if platform.os.mojang_name() != Some(name.as_str()) {
            return Ok(false);
        }
    }
    if let Some(pattern) = &os.version {
        // The format says regex; unanchored, so it is a search. A pattern the
        // format authors wrote badly must fail loudly, not match nothing.
        let regex = Regex::new(pattern).map_err(|_| Error::Invalid {
            what: "rule os.version",
            why: format!("{pattern:?} is not a valid regular expression"),
        })?;
        if !regex.is_match(&platform.os_version) {
            return Ok(false);
        }
    }
    if let Some(arch) = &os.arch {
        if platform.arch.mojang_name() != Some(arch.as_str()) {
            return Ok(false);
        }
    }
    if let Some(range) = &os.version_range {
        if !version_in_range(&platform.os_version, range) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `min` inclusive, `max` exclusive, comparing dot-separated components
/// numerically (missing trailing components count as zero).
fn version_in_range(version: &str, range: &VersionRange) -> bool {
    if let Some(min) = &range.min {
        if compare_versions(version, min) == Ordering::Less {
            return false;
        }
    }
    if let Some(max) = &range.max {
        if compare_versions(version, max) != Ordering::Less {
            return false;
        }
    }
    true
}

/// Compare two dotted version strings component by component. Components
/// that parse as numbers compare numerically (so `17134` > `9`), anything
/// else lexically; a missing component counts as zero (`10` == `10.0`).
fn compare_versions(a: &str, b: &str) -> Ordering {
    let mut left = a.split('.');
    let mut right = b.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (l, r) => {
                let l = l.unwrap_or("0");
                let r = r.unwrap_or("0");
                let order = match (l.parse::<u64>(), r.parse::<u64>()) {
                    (Ok(l), Ok(r)) => l.cmp(&r),
                    _ => l.cmp(r),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::VersionRange;

    fn windows(version: &str) -> Platform {
        Platform::new(Os::Windows, version, Arch::X86_64)
    }

    fn rule(json: &str) -> Rule {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn last_match_wins() {
        // The 1.12.2 lwjgl shape: everywhere except macOS, expressed as an
        // unconditional allow followed by a macOS disallow.
        let rules = vec![
            rule(r#"{"action": "allow"}"#),
            rule(r#"{"action": "disallow", "os": {"name": "osx"}}"#),
        ];
        assert!(rules_allow(&rules, &windows("10.0.19045")).unwrap());
        let mac = Platform::new(Os::MacOs, "14.0", Arch::X86_64);
        assert!(!rules_allow(&rules, &mac).unwrap());
    }

    #[test]
    fn a_list_nothing_matches_disallows() {
        // An allow-list with no companion rule: only macOS, nobody else.
        let rules = vec![rule(r#"{"action": "allow", "os": {"name": "osx"}}"#)];
        assert!(!rules_allow(&rules, &windows("10.0.19045")).unwrap());
    }

    #[test]
    fn empty_rules_allow_everything() {
        assert!(rules_allow(&[], &windows("10.0.19045")).unwrap());
    }

    #[test]
    fn features_must_agree() {
        let rules = vec![rule(
            r#"{"action": "allow", "features": {"has_custom_resolution": true}}"#,
        )];
        assert!(!rules_allow(&rules, &windows("10.0")).unwrap());
        let mut platform = windows("10.0");
        platform
            .features
            .insert("has_custom_resolution".to_string(), true);
        assert!(rules_allow(&rules, &platform).unwrap());
        platform
            .features
            .insert("has_custom_resolution".to_string(), false);
        assert!(!rules_allow(&rules, &platform).unwrap());
    }

    #[test]
    fn version_regex_searches_the_os_version() {
        // `\\.` in the JSON so the pattern is the regex `^10\.`.
        let rules = vec![rule(r#"{"action": "allow", "os": {"version": "^10\\."}}"#)];
        assert!(rules_allow(&rules, &windows("10.0.19045")).unwrap());
        assert!(!rules_allow(&rules, &windows("6.1.7601")).unwrap());
    }

    #[test]
    fn a_broken_regex_is_an_error_not_a_silent_no_match() {
        let rules = vec![rule(r#"{"action": "allow", "os": {"version": "["}}"#)];
        assert!(rules_allow(&rules, &windows("10.0")).is_err());
    }

    #[test]
    fn version_range_is_min_inclusive_max_exclusive() {
        // The 26.3 sample's boundary: ZGC at min 10.0.17134, G1 at max
        // 10.0.17134. Exactly at the boundary only the min side may match,
        // or both would claim it.
        let min = VersionRange {
            min: Some("10.0.17134".to_string()),
            ..VersionRange::default()
        };
        let max = VersionRange {
            max: Some("10.0.17134".to_string()),
            ..VersionRange::default()
        };
        assert!(version_in_range("10.0.17134", &min));
        assert!(!version_in_range("10.0.17134", &max));
        assert!(!version_in_range("10.0.17133", &min));
        assert!(version_in_range("10.0.17133", &max));
        assert!(version_in_range("10.0.19045", &min));
        assert!(!version_in_range("10.0.19045", &max));
    }

    #[test]
    fn versions_compare_by_components_not_text() {
        assert_eq!(compare_versions("10.0.9", "10.0.17134"), Ordering::Less);
        assert_eq!(compare_versions("10.0", "10"), Ordering::Equal);
        assert_eq!(compare_versions("6.1.7601", "10.0"), Ordering::Less);
    }

    #[test]
    fn arch_gates_32_bit_only() {
        let rules = vec![rule(r#"{"action": "allow", "os": {"arch": "x86"}}"#)];
        assert!(!rules_allow(&rules, &windows("10.0")).unwrap());
        let x86 = Platform::new(Os::Windows, "10.0", Arch::X86);
        assert!(rules_allow(&rules, &x86).unwrap());
    }
}
