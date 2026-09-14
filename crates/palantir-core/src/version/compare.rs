//! Version string comparison — port of `launcher/Version.cpp` (develop).
//!
//! Strings split into sections that are `Numeric`, `Textual` or
//! `PreRelease`; `+` starts an ignored appendix; `-`/` ` begin a
//! pre-release section when followed by a non-digit. Numeric sections strip
//! leading zeros and compare by length then digits (no overflow).
//! Section order: `PreRelease < Null < Textual`, so `1.0-pre1 < 1.0` and
//! `1.0 < 1.0.1`.
//!
//! Prism `Version.cpp` ground truth notes (`Section::operator<=>` + `parse`):
//! * A missing section (`Null`) is less than any present section except
//!   `PreRelease` (`Null` vs `PreRelease` is `Greater`), so a trailing zero
//!   section still counts: `"1.20" < "1.20.0"` because `Null` <
//!   `Numeric("")` (`"0"` stripped to `""` by `removeLeadingZeros`). The
//!   `numeric_...` test therefore expects `Greater`; the old `Equal`
//!   expectation contradicted `zero_section_vs_null_matches_qt`.
//! * Parsing splits on digit/non-digit runs, so `"1.0a"` is
//!   `[Numeric("1"), Textual("."), Numeric(""), Textual("a")]`. Vs `"1.1"`
//!   (`[Numeric("1"), Textual("."), Numeric("1")]`) the decision happens at
//!   index 2 (`Numeric("")` < `Numeric("1")`), hence `"1.0a" < "1.1"`.
//!   A genuine differing-kind codepoint case is `"1.a" > "1.1"`
//!   (`'a'` (97) > `'1'` (49)).

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Numeric,
    Textual,
    PreRelease,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Section {
    kind: Kind,
    value: String,
}

impl Section {
    /// Numeric-vs-numeric comparison: by length then digits (leading zeros
    /// are stripped at parse time, so this equals numeric comparison
    /// without overflow).
    fn compare(&self, other: &Section) -> Ordering {
        if self.kind == Kind::Numeric && other.kind == Kind::Numeric {
            return self.value.len().cmp(&other.value.len()).then_with(|| self.value.cmp(&other.value));
        }
        // Textual comparison (differing kinds, or both textual/pre-release).
        // Note: a fully-zero numeric section keeps its empty value here
        // (`removeLeadingZeros` in Version.cpp), which then compares by
        // codepoint/length like any other textual operand.
        let mut a = self.value.chars();
        let mut b = other.value.chars();
        loop {
            match (a.next(), b.next()) {
                (Some(x), Some(y)) => {
                    if x != y {
                        return (x as u32).cmp(&(y as u32));
                    }
                }
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
            }
        }
    }
}

/// Missing-section ordering (`Section::Null`): greater than PreRelease,
/// less than anything else.
fn null_vs(other: &Section) -> Ordering {
    if other.kind == Kind::PreRelease {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

/// A parsed version string with Prism-compatible ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalantirVersion {
    raw: String,
    sections: Vec<Section>,
}

impl PalantirVersion {
    /// Parse a version string.
    pub fn parse(s: &str) -> PalantirVersion {
        let mut sections: Vec<Section> = Vec::new();
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();
        let mut i = 0usize;
        while i < len {
            let mut cur = Section { kind: Kind::Textual, value: String::new() };
            let c = chars[i];
            if c == '+' {
                break; // appendix ignored
            }
            if c == '-' || c == ' ' {
                cur.value.push(c);
                i += 1;
                if i < len && !chars[i].is_ascii_digit() {
                    cur.kind = Kind::PreRelease;
                }
            } else if c.is_ascii_digit() {
                cur.kind = Kind::Numeric;
            }
            while i < len {
                let r = chars[i];
                let numeric = cur.kind == Kind::Numeric;
                if r.is_ascii_digit() != numeric
                    || (r == ' ' && numeric)
                    || (r == '-' && cur.kind != Kind::PreRelease)
                    || r == '+'
                {
                    break;
                }
                cur.value.push(r);
                i += 1;
            }
            if !cur.value.is_empty() {
                if cur.kind == Kind::Numeric {
                    // removeLeadingZeros: "00" -> "" (kept as an empty numeric)
                    cur.value = cur.value.trim_start_matches('0').to_string();
                }
                sections.push(cur);
            }
        }
        PalantirVersion { raw: s.to_string(), sections }
    }

    /// The original string.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Three-way comparison against another version.
    pub fn compare(&self, other: &PalantirVersion) -> Ordering {
        let len = self.sections.len().max(other.sections.len());
        for i in 0..len {
            let cmp = match (self.sections.get(i), other.sections.get(i)) {
                (None, None) => Ordering::Equal,
                (None, Some(b)) => null_vs(b),
                (Some(a), None) => null_vs(a).reverse(),
                (Some(a), Some(b)) => a.compare(b),
            };
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for PalantirVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PalantirVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::*;

    fn cmp(a: &str, b: &str) -> Ordering {
        PalantirVersion::parse(a).compare(&PalantirVersion::parse(b))
    }

    #[test]
    fn numeric_comparison_is_not_lexicographic() {
        assert_eq!(cmp("1.20.1", "1.20.10"), Less);
        assert_eq!(cmp("1.20.10", "1.20.9"), Greater);
        assert_eq!(cmp("1.7.10", "1.7.9"), Greater);
        assert_eq!(cmp("1.20", "1.20.1"), Less);
        // Prism truth: trailing zero section counts — Null < Numeric("") so
        // "1.20" < "1.20.0". Keeps `zero_section_vs_null_matches_qt` as ground
        // truth (the old `Equal` here contradicted it).
        assert_eq!(cmp("1.20.0", "1.20"), Greater);
    }

    #[test]
    fn zero_section_vs_null_matches_qt() {
        // "1.20.0" vs "1.20": sections [1, ".", 20, ".", Numeric("")] vs
        // [1, ".", 20]; "0" strips to "" (empty numeric), Null vs Numeric ->
        // Less means "1.20" < "1.20.0".
        assert_eq!(cmp("1.20", "1.20.0"), Less);
    }

    #[test]
    fn pre_release_sorts_before_release() {
        assert_eq!(cmp("1.0-pre1", "1.0"), Less);
        assert_eq!(cmp("1.0-pre2", "1.0-pre1"), Greater);
        assert_eq!(cmp("1.0", "1.0.1"), Less);
        // space-separated names (Modrinth style)
        assert_eq!(cmp("1.20 Pre-Release 1", "1.20"), Less);
    }

    #[test]
    fn appendix_after_plus_is_ignored() {
        assert_eq!(cmp("1.0+build.5", "1.0"), Equal);
        assert_eq!(cmp("1.0+build.5", "1.0+build.9"), Equal);
    }

    #[test]
    fn textual_sections_compare_by_codepoint() {
        assert_eq!(cmp("1.0a", "1.0b"), Less);
        // "1.0a" splits into [1, ".", Numeric(""), Textual("a")]; vs "1.1"
        // ([1, ".", Numeric("1")]) the decision is Numeric("") < Numeric("1")
        // at index 2, so Less — the trailing "a" is never reached. This matches
        // Version.cpp (numeric-vs-numeric by length/digits, else codepoint).
        assert_eq!(cmp("1.0a", "1.1"), Less);
        // Genuine differing-kind codepoint path: Textual("a") vs Numeric("1")
        // at the same index falls into the textual path, 'a' (97) > '1' (49).
        assert_eq!(cmp("1.a", "1.1"), Greater);
    }

    #[test]
    fn leading_zeros_do_not_confuse_ordering() {
        assert_eq!(cmp("1.08", "1.8"), Equal);
        assert_eq!(cmp("1.010", "1.9"), Greater);
    }

    #[test]
    fn raw_string_is_preserved() {
        let v = PalantirVersion::parse("1.20-pre1");
        assert_eq!(v.as_str(), "1.20-pre1");
    }
}
