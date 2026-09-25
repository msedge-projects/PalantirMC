//! The runtime behind [`crate::text_gen`]: a plural value, the English plural
//! rule, and the number formatting the reference's copy depends on.
//!
//! The strings themselves are generated -- the reference's own locale, compiled by
//! `tools/gen_text.py` -- and this module is the small part a generator should not
//! be trusted to invent: the rules a message is filled in by. It is hand-written
//! because the rules are judgement, and it is short because the reference's copy
//! only ever needs four of ICU's constructs (see the generator for which, and for
//! why anything else is refused rather than approximated).
//!
//! Two behaviours are reproduced exactly, because the reference's copy leans on
//! both and both are invisible when wrong:
//!
//! * **`{count}` and `{count, number}` are not the same string.** The typed one
//!   goes through `Intl.NumberFormat`, so 1200 is `1,200`; the bare one is
//!   interpolated as the value, so it stays `1200`. [`Plural::bare`] and
//!   [`Plural::grouped`] are those two renderings, and the generated helper picks
//!   the one the message asked for.
//! * **A plural's value may be a number or a category.** `{count, plural, one
//!   {# project} other {# projects}}` is called with a count;
//!   `{countPlural, plural, one {player} other {players}}` is called with
//!   `formatCompactNumberPlural(...)`, a string such as `1.2K`. Both are calls the
//!   reference's own source makes, so both are what [`Plural`] holds.
//!
//! English is the only language here: the reference's own locale list is vendored
//! and its other 32 languages are not, so the plural rule is the CLDR rule for
//! `en` and nothing else is offered. `tools/gen_text.py` refuses any other plural
//! category for the same reason.

/// A plural's value: the number itself, or a category the caller already chose.
///
/// The reference passes both, and the two are not interchangeable: a number is
/// pluralised by the language's rule, while a string is taken as the answer. That
/// is why this is an enum rather than a `u64` with an escape hatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plural<'a> {
    /// A count, pluralised by the English rule.
    Number(u64),
    /// A category the caller has already decided on: `"one"`, or a compacted
    /// count such as `"1.2K"` that the reference computes itself.
    Category(&'a str),
}

impl<'a> From<u64> for Plural<'a> {
    fn from(number: u64) -> Plural<'a> {
        Plural::Number(number)
    }
}

impl<'a> From<&'a str> for Plural<'a> {
    fn from(category: &'a str) -> Plural<'a> {
        Plural::Category(category)
    }
}

impl Plural<'_> {
    /// Whether this value is the plural category `one`.
    ///
    /// The English rule, as CLDR states it: exactly one is `one`, and everything
    /// else -- including zero, and including 1.5 -- is `other`. A category string
    /// is its own answer, so `"one"` is `one` and `"1.2K"` is not.
    fn is_one(&self) -> bool {
        match self {
            Plural::Number(number) => *number == 1,
            Plural::Category(category) => *category == "one",
        }
    }

    /// Whether `arm` is the arm this value selects.
    ///
    /// `=N` is an exact match on the number, `one` and `other` are the English
    /// categories, and anything else is read as a category string the caller
    /// supplied -- which is how a generated helper never has to guess what a
    /// caller meant by passing a string.
    pub fn is(&self, arm: &str) -> bool {
        match arm {
            "other" => !self.is_one(),
            "one" => self.is_one(),
            exact if exact.starts_with('=') => match exact[1..].parse::<u64>() {
                Ok(wanted) => match self {
                    // The number, where there is one.
                    Plural::Number(number) => *number == wanted,
                    // A category that happens to be written as a digit, which the
                    // reference's own compacted counts are not but a future caller
                    // could pass.
                    Plural::Category(category) => category.parse::<u64>().ok() == Some(wanted),
                },
                // An arm that is not a number names nothing. Comparing two
                // unparsable arms would make every malformed arm match every
                // category, because `Err(_) == Err(_)`.
                Err(_) => false,
            },
            named => matches!(self, Plural::Category(category) if *category == named),
        }
    }

    /// What a bare `{name}` renders as: the value, as it was given.
    ///
    /// No grouping, because the reference's compiler interpolates a bare argument
    /// instead of formatting it.
    pub fn bare(&self) -> String {
        match self {
            Plural::Number(number) => number.to_string(),
            Plural::Category(category) => (*category).to_string(),
        }
    }

    /// What `#` and `{name, number}` render as: the number, grouped.
    ///
    /// Both go through `Intl.NumberFormat` in the reference, and a category string
    /// is already formatted, so it comes back unchanged.
    pub fn grouped(&self) -> String {
        match self {
            // Bound as `value` rather than `number`: naming it after the function
            // below would shadow it, and the mistake reads as a call of a `&u64`.
            Plural::Number(value) => number(*value),
            Plural::Category(category) => (*category).to_string(),
        }
    }
}

/// An integer as `Intl.NumberFormat` writes it in English: thousands separated.
///
/// Hand-rolled rather than reaching for a formatting crate: the reference
/// publishes one English locale, the separator is a comma, and a dependency whose
/// whole job is one comma is a dependency this build does not need.
pub fn number(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        // A comma before every third digit counting from the right, and never
        // before the first.
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_grouped_by_threes_from_the_right() {
        assert_eq!(number(0), "0");
        assert_eq!(number(1), "1");
        assert_eq!(number(999), "999");
        assert_eq!(number(1000), "1,000");
        assert_eq!(number(12345), "12,345");
        assert_eq!(number(123456), "123,456");
        assert_eq!(number(1234567), "1,234,567");
        // The longest thing a u64 can be, so an off-by-one in the boundary count
        // shows up here rather than in a live count of downloads.
        assert_eq!(number(u64::MAX), "18,446,744,073,709,551,615");
    }

    #[test]
    fn the_english_rule_is_one_for_one_and_other_for_everything_else() {
        for (value, one) in [(0u64, false), (1, true), (2, false), (11, false), (100, false)] {
            let plural = Plural::Number(value);
            assert_eq!(plural.is("one"), one, "{value}");
            assert_eq!(plural.is("other"), !one, "{value}");
        }
    }

    #[test]
    fn a_category_is_its_own_answer() {
        // What the reference does for a compacted count: the category arrives as
        // a string, and the string is not re-derived from a number.
        let compacted = Plural::Category("1.2K");
        assert!(!compacted.is("one"));
        assert!(compacted.is("other"));
        assert_eq!(compacted.bare(), "1.2K");
        assert_eq!(compacted.grouped(), "1.2K", "a category is already formatted");
        let one = Plural::Category("one");
        assert!(one.is("one"));
        assert!(!one.is("other"));
        // And a category no arm names is not silently the first arm.
        assert!(!Plural::Category("few").is("one"));
        assert!(Plural::Category("few").is("few"));
    }

    #[test]
    fn an_exact_arm_matches_the_number_and_nothing_else() {
        // `=0` is how the reference says "No languages match".
        assert!(Plural::Number(0).is("=0"));
        assert!(!Plural::Number(1).is("=0"));
        assert!(!Plural::Number(2).is("=0"));
        assert!(Plural::Number(0).is("=0"));
        // An arm that is not a number is not an exact match either way, rather
        // than matching everything.
        assert!(!Plural::Number(7).is("=x"));
        assert!(!Plural::Category("one").is("=x"));
        // A category written as digits still matches its own number.
        assert!(Plural::Category("0").is("=0"));
        assert!(!Plural::Category("0").is("=1"));
    }

    #[test]
    fn the_two_renderings_differ_for_a_number_and_agree_for_a_category() {
        // The distinction the whole module exists for: `{count}` versus
        // `{count, number}` on the same value.
        let value = Plural::Number(1200);
        assert_eq!(value.bare(), "1200");
        assert_eq!(value.grouped(), "1,200");
        assert_eq!(Plural::from(1200u64), value);
        assert_eq!(Plural::from("1.2K"), Plural::Category("1.2K"));
    }
}
