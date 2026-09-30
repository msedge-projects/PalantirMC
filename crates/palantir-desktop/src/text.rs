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
//! ## The chosen language
//!
//! The strings come from the reference's locales, and *which* locale is a
//! setting: [`crate::locale`] owns it, and [`render`] is how a generated helper
//! reads it. A helper asks for its own key in the language in force and renders
//! the locale's template if the language has one; if it does not -- or if the
//! language is English, or the template asks for something this renderer cannot
//! produce -- the helper runs the English code the generator wrote, which is the
//! same code it ran before there was a language setting. That is the whole design
//! in one sentence: **English is not rendered through here**, so 3,846 sentences
//! cannot quietly change meaning the day a second language is offered.
//!
//! The ICU subset is the one `tools/gen_text.py` accepts -- `{name}`,
//! `{name, number}`, `{name, plural, ..}` with `#`, and `{name, select, ..}` --
//! and this renderer is the second half of that contract: the generator refuses a
//! locale it cannot parse, so every template that reaches [`render`] is one of
//! those four shapes. Anything else returns `None`, which is a fallback to
//! English rather than a wrong sentence.

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

/// The six CLDR categories, which is the set a template's arms are drawn from.
const CATEGORIES: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

impl<'a> Plural<'a> {
    /// The count, when this value is one.
    ///
    /// `None` for a category the caller decided, which is the case that has no
    /// arithmetic in it: the reference passes `"1.2K"`, and a string is not a
    /// number to pluralize.
    pub fn count(&self) -> Option<u64> {
        match self {
            Plural::Number(number) => Some(*number),
            Plural::Category(_) => None,
        }
    }

    /// The category this value selects under `language`, or `other`.
    ///
    /// A number goes through the language's CLDR rule. A category string is its
    /// own answer when it names a category -- that is how the reference passes an
    /// already-compacted count -- and is `other` when it does not, which is what
    /// `Intl.PluralRules` does with a string it cannot parse.
    fn category(&self, language: &str) -> &'static str {
        match self {
            Plural::Number(number) => crate::locale::category(language, *number),
            Plural::Category(named) => CATEGORIES
                .iter()
                .find(|category| **category == *named)
                .copied()
                .unwrap_or("other"),
        }
    }

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

    /// What `#` and `{name, number}` render as in English: the number, grouped.
    ///
    /// Both go through `Intl.NumberFormat` in the reference, and a category string
    /// is already formatted, so it comes back unchanged. English's own grouping is
    /// a comma, which is what the generated English code has always used; the
    /// renderer that reads another language groups with *that* language's
    /// separator, which is [`Plural::grouped_in`] and [`crate::locale::group`].
    pub fn grouped(&self) -> String {
        self.grouped_in("en")
    }

    /// [`Plural::grouped`] in a named language.
    pub fn grouped_in(&self, language: &str) -> String {
        match self {
            // Bound as `value` rather than `number`: naming it after the function
            // below would shadow it, and the mistake reads as a call of a `&u64`.
            Plural::Number(value) => crate::locale::group(language, *value),
            Plural::Category(category) => (*category).to_string(),
        }
    }
}

/// A value a generated helper hands to [`render`].
///
/// The helper's parameters are typed -- a count is a `u64`, a choice key is a
/// `&str` -- and a locale's template may use an argument differently from
/// English's. The two cases that matter are both real in this corpus: `pt-BR`
/// writes `{count, number}` where English writes `{count}` in the same sentence,
/// and `ar-SA` adds `two`/`few`/`many` arms to a plural English spells with
/// `one`/`other`. Carrying the value by kind is what lets the same helper render
/// either locale without the generator having to guess.
#[derive(Debug, Clone, Copy)]
pub enum Value<'a> {
    /// A string argument: a name, a choice key, a version like `"21"`.
    Text(&'a str),
    /// A count that is not pluralized.
    Number(u64),
    /// A plural value, which is a count or a category the caller already chose.
    Plural(Plural<'a>),
}

impl<'a> Value<'a> {
    /// A string argument.
    pub fn text(value: &'a str) -> Value<'a> {
        Value::Text(value)
    }

    /// A count that is not pluralized.
    pub fn number(value: u64) -> Value<'a> {
        Value::Number(value)
    }

    /// A plural value.
    pub fn plural(value: Plural<'a>) -> Value<'a> {
        Value::Plural(value)
    }

    /// What a bare `{name}` renders as: the value, as it was given.
    fn bare(&self) -> String {
        match self {
            Value::Text(text) => (*text).to_string(),
            Value::Number(number) => number.to_string(),
            Value::Plural(plural) => plural.bare(),
        }
    }

    /// What `{name, number}` and `#` render as in a language: the number, grouped.
    ///
    /// A string argument is already formatted, so it comes back unchanged; a
    /// count that is not a plural is grouped with the language's own separator,
    /// which is the `Intl.NumberFormat` the reference's typed arguments go
    /// through.
    fn grouped(&self, language: &str) -> String {
        match self {
            Value::Text(text) => (*text).to_string(),
            Value::Number(value) => crate::locale::group(language, *value),
            Value::Plural(plural) => plural.grouped_in(language),
        }
    }
}

/// The arguments a template may name, by the reference's own names for them.
struct Arguments<'a> {
    language: &'static str,
    values: &'a [(&'a str, Value<'a>)],
}

impl<'a> Arguments<'a> {
    fn get(&self, name: &str) -> Option<&'a Value<'a>> {
        self.values
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, value)| value)
    }
}

/// Render a message from the language in force, or `None` to render English.
///
/// Called by every generated helper that has ICU markers in it, before the
/// English body the generator wrote. `None` is the answer in four cases, and
/// every one of them is "use the English path": the language in force is
/// English; the locale does not carry this key; the locale's template is not one
/// this renderer understands; or it names an argument the helper does not have.
/// The last two should not happen -- the generator refuses a locale it cannot
/// render -- and returning `None` rather than a guess is what makes them a
/// fallback instead of a wrong sentence if they ever do.
pub fn render<'a>(
    key: crate::text_gen::Key,
    args: &'a [(&'a str, Value<'a>)],
) -> Option<String> {
    let template = crate::locale::lookup(key)?;
    let arguments = Arguments { language: crate::locale::active().language, values: args };
    let mut out = String::new();
    walk(template, &arguments, None, &mut out)?;
    Some(out)
}

/// Append `message` to `out`, or `None` if it is not a template we can render.
///
/// A single recursive pass rather than a tree: an arm's body is a slice of the
/// same string, so the walk is the whole parser. `plural` is the value the `#`
/// marker stands for while an arm is being drawn, and it is `None` outside one
/// -- a `#` in a message's own text is refused by the generator, so a `#` here
/// with no plural above it is a locale this renderer cannot read.
fn walk(
    message: &str,
    arguments: &Arguments,
    plural: Option<Value<'_>>,
    out: &mut String,
) -> Option<()> {
    let chars: Vec<char> = message.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '#' => {
                out.push_str(&plural?.grouped(arguments.language));
                index += 1;
            }
            '\'' => {
                // `''` is one literal apostrophe. A single apostrophe before a
                // brace opens ICU's quoting, which the generator refuses, so it
                // means this template is not one this renderer was built for.
                if chars.get(index + 1) == Some(&'\'') {
                    out.push('\'');
                    index += 2;
                } else if chars.get(index + 1) == Some(&'{') {
                    return None;
                } else {
                    out.push('\'');
                    index += 1;
                }
            }
            '{' => {
                let (inner, next) = braces(&chars, index)?;
                placeholder(&inner, arguments, out)?;
                index = next;
            }
            character => {
                out.push(character);
                index += 1;
            }
        }
    }
    Some(())
}

/// The contents of the `{...}` opening at `start`, and the index after it.
fn braces(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut depth = 0;
    for (offset, character) in chars[start..].iter().enumerate() {
        if *character == '{' {
            depth += 1;
        } else if *character == '}' {
            depth -= 1;
            if depth == 0 {
                let inner: String = chars[start + 1..start + offset].iter().collect();
                return Some((inner, start + offset + 1));
            }
        }
    }
    None
}

/// Split on commas that are not inside braces.
fn split_top_level(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for character in text.chars() {
        match character {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if character == ',' && depth == 0 {
            parts.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(character);
        }
    }
    parts.push(current.trim().to_string());
    parts
}

/// Render one `{...}` into `out`.
fn placeholder(inner: &str, arguments: &Arguments, out: &mut String) -> Option<()> {
    let parts = split_top_level(inner);
    let name = parts.first()?.as_str();
    if name.is_empty() {
        return None;
    }
    let kind = parts.get(1).map(String::as_str).unwrap_or("");
    if kind.is_empty() {
        out.push_str(&arguments.get(name)?.bare());
        return Some(());
    }
    match kind {
        "number" => {
            out.push_str(&arguments.get(name)?.grouped(arguments.language));
            Some(())
        }
        "plural" => {
            let value = *arguments.get(name)?;
            let arms = arms(parts.get(2..).unwrap_or(&[]), "plural")?;
            let count = match value {
                Value::Plural(plural) => plural.count(),
                Value::Number(number) => Some(number),
                Value::Text(_) => return None,
            };
            let category = match value {
                Value::Plural(plural) => plural.category(arguments.language),
                // A `{count, plural, ..}` where English had a bare `{count}`:
                // the argument is a count in that case, and a count pluralizes.
                Value::Number(number) => crate::locale::category(arguments.language, number),
                Value::Text(_) => return None,
            };
            let (body, fallback) = choose(&arms, category, count);
            let body = body.or(fallback)?;
            walk(&body, arguments, Some(value), out)
        }
        "select" => {
            let value = *arguments.get(name)?;
            let text = match value {
                Value::Text(text) => text,
                _ => return None,
            };
            let arms = arms(parts.get(2..).unwrap_or(&[]), "select")?;
            let (body, fallback) = choose(&arms, text, None);
            let body = body.or(fallback)?;
            walk(&body, arguments, None, out)
        }
        // `date`, `time`, `list`, `duration`, `selectordinal` and anything else
        // the generator refuses. Reaching one means the table and the generator
        // disagree, and English is the safe answer.
        _ => None,
    }
}

/// The arms of a `plural` or a `select`, as `(key, body)`.
///
/// `kind` only changes what a missing `other` costs: ICU requires one, and the
/// generator refuses a message without one, so an arm list that has none is a
/// template this renderer cannot read.
fn arms(parts: &[String], _kind: &str) -> Option<Vec<(String, String)>> {
    let joined = parts.join(",");
    let chars: Vec<char> = joined.chars().collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        let brace = chars[index..].iter().position(|character| *character == '{')? + index;
        let key: String = chars[index..brace].iter().collect();
        let key = key.trim().to_string();
        if key.is_empty() {
            return None;
        }
        let (body, next) = braces(&chars, brace)?;
        result.push((key, body));
        index = next;
    }
    if result.is_empty() || result.iter().all(|(key, _)| key != "other") {
        return None;
    }
    Some(result)
}

/// The arm a value selects, and the `other` arm to fall back on.
///
/// ICU's order: an exact `=N` match wins, then the category, then `other`. The
/// generated tables keep the reference's own arm order, so walking them and
/// taking the first match is that rule -- `other` is skipped in the walk and
/// returned separately, because it matches everything and would otherwise shadow
/// every category below it.
fn choose(arms: &[(String, String)], wanted: &str, exact: Option<u64>) -> (Option<String>, Option<String>) {
    let mut other = None;
    for (key, body) in arms {
        if key == "other" {
            other = Some(body.clone());
            continue;
        }
        if let Some(number) = key.strip_prefix('=') {
            if let (Ok(number), Some(exact)) = (number.trim().parse::<u64>(), exact) {
                if number == exact {
                    return (Some(body.clone()), other);
                }
            }
            continue;
        }
        if key == wanted {
            return (Some(body.clone()), other);
        }
    }
    (None, other)
}

/// A message's `<tag>...</tag>` slot, split out of the sentence around it.
///
/// The reference's copy marks the part of a sentence that is a *control* with a
/// tag: `friends.sign-in-to-add-friends` is
/// `"<link>Sign in to a Modrinth account</link> to add friends and see what
/// they're playing!"`, and the component renders what is between the tags as a
/// `text-brand cursor-pointer` span with the sign-in behind it rather than as
/// prose. The generated table keeps the markup verbatim -- it has to, because the
/// tag's *name* is the slot the component fills -- so a caller that draws a
/// tagged message has to split it, and this is that split: the text before the
/// slot, the slot's own words, and the text after.
///
/// `None` when the message has no such tag, or has one that is never closed --
/// half a sentence is worse than a whole one, so an unclosed tag is not a slot but
/// a message with no slot. Most of the copy has no tag at all, and a caller that
/// asks for one and gets `None` should draw the sentence rather than invent a
/// control for it.
pub fn tagged<'a>(message: &'a str, tag: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = message.find(&open)?;
    let rest = start + open.len();
    let end = rest + message.get(rest..)?.find(&close)?;
    Some((&message[..start], &message[rest..end], &message[end + close.len()..]))
}

/// A message's `{name}` placeholder, and the sentence on either side of it.
///
/// The reference's copy marks a word that is a *control* with an ICU placeholder
/// rather than a tag when the component fills the slot itself:
/// `app.skins.ears-feature-notice` is
/// `"This skin uses features from the {ears} mod"`, and its component substitutes a
/// sentinel for the placeholder, splits on the sentinel and draws what is between the
/// halves as a link. The generated table keeps the placeholder verbatim -- it has to,
/// because the placeholder's *name* is the slot -- so the split is what a caller draws
/// around, and this is that split: the text before the slot and the text after it.
///
/// `None` when the message has no such placeholder, which is the honest answer a
/// caller should draw the sentence for rather than inventing a control in the middle
/// of it.
pub fn placeholder_parts<'a>(message: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
    let slot = format!("{{{name}}}");
    let at = message.find(&slot)?;
    Some((&message[..at], &message[at + slot.len()..]))
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
    fn a_tagged_message_splits_around_its_slot() {
        // The reference's own sentence, tag and all, out of the generated table.
        let sentence = "<link>Sign in to a Modrinth account</link> to add friends and see what they're playing!";
        let (before, slot, after) = tagged(sentence, "link").expect("a slot");
        assert_eq!(before, "");
        assert_eq!(slot, "Sign in to a Modrinth account");
        assert_eq!(after, " to add friends and see what they're playing!");
        // Text on both sides, which is the shape most tagged copy has.
        assert_eq!(tagged("a <b>bold</b> word", "b"), Some(("a ", "bold", " word")));
        // A tag nobody asked for is not a slot, and neither is a tag that was
        // opened and never closed: half a sentence is worse than none.
        assert_eq!(tagged("a <b>bold</b> word", "link"), None);
        assert_eq!(tagged("<link>unclosed", "link"), None);
        assert_eq!(tagged("no tags at all", "link"), None);
        assert_eq!(tagged("", "link"), None);
        // The first slot is the one that is taken, and a tag inside the slot is
        // not the closing tag.
        assert_eq!(
            tagged("<b>one</b> then <b>two</b>", "b"),
            Some(("", "one", " then <b>two</b>"))
        );
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
    fn english_never_goes_through_the_locale_renderer() {
        // The single most important property of this module: with English in
        // force, every helper is the code the generator wrote, so no sentence can
        // change meaning the day a second language is offered.
        crate::locale::set(crate::locale::ENGLISH);
        let key = crate::text_gen::Key::AppScreenshotsSelectionDeleteDescription;
        assert_eq!(render(key, &[]), None);
        assert_eq!(
            render(key, &[("count", Value::plural(Plural::Number(3)))]),
            None,
            "even with arguments, English is not rendered through here"
        );
    }

    #[test]
    fn a_key_a_locale_does_not_carry_renders_english() {
        // `de-DE` translates 3,792 of 3,846 keys, so this is not hypothetical.
        crate::locale::set("de-DE");
        let missing = crate::text_gen::ALL
            .into_iter()
            .find(|key| crate::locale::lookup(*key).is_none())
            .expect("a key de-DE falls back on");
        assert_eq!(render(missing, &[]), None, "{}", missing.name());
        crate::locale::set(crate::locale::ENGLISH);
    }

    #[test]
    fn a_locale_s_own_plural_arms_are_the_ones_that_render() {
        // The sentence this launcher's Arabic table carries, arm by arm: `zero`
        // and `two` are categories English's rule cannot produce at all, and the
        // count `3` falls in `few`, which is the third arm. In English the same
        // two calls are the generator's own code, which is why this test names
        // the language rather than relying on a default.
        let key = crate::text_gen::Key::AppScreenshotsSelectionDeleteDescription;
        let render_count = |count: u64| {
            render(key, &[("count", Value::plural(Plural::Number(count)))])
                .expect("ar-SA carries this key")
        };
        crate::locale::set("ar-SA");
        let zero = render_count(0);
        let two = render_count(2);
        let three = render_count(3);
        assert_ne!(zero, two, "zero and two are different arms");
        assert_ne!(two, three, "two and few are different arms");
        assert!(three.contains('3'), "`#` is the count it belongs to: {three}");
        assert!(zero.contains("لقطة"), "the Arabic arm: {zero}");

        // Russian has one/few/many, and the digit is there in each.
        crate::locale::set("ru-RU");
        let one = render_count(1);
        let five = render_count(5);
        assert_ne!(one, five);
        assert!(one.contains('1') && five.contains('5'));
        crate::locale::set(crate::locale::ENGLISH);
    }

    #[test]
    fn a_category_the_caller_chose_is_not_pluralized_again() {
        // What the reference does for a compacted count: `formatCompactNumberPlural`
        // hands over `"1.2K"` and a category of its own choosing, and the arm is
        // that choice rather than anything derived from the string.
        let key = crate::text_gen::Key::ProjectOnlinePlayerCountTooltip;
        crate::locale::set("de-DE");
        let one = render(
            key,
            &[
                ("count", Value::text("1.2K")),
                ("countPlural", Value::plural(Plural::Category("one"))),
            ],
        )
        .expect("de-DE carries this key");
        let many = render(
            key,
            &[
                ("count", Value::text("3.4K")),
                ("countPlural", Value::plural(Plural::Category("other"))),
            ],
        )
        .expect("de-DE carries this key");
        // The category `"one"` is the arm, not something re-derived from the
        // string `"1.2K"` -- which, pluralized as a number, would be `other`.
        assert_eq!(one, "1.2K Spieler online");
        assert_eq!(many, "3.4K Spieler online");
        crate::locale::set(crate::locale::ENGLISH);
    }

    #[test]
    fn every_offered_language_renders_every_shape_without_a_brace_left_in_it() {
        // The gate's "a drawer that still draws in every locale": every language
        // the reference offers, through each of the four ICU shapes, and nothing
        // that still looks like a template. A `{` left in a drawn sentence is the
        // failure this is here to catch -- it is what the Home screen shipped
        // with until `text::tagged` was used, and it is invisible until somebody
        // reads the language.
        use crate::text_gen;
        for tag in crate::locale::OFFERED {
            crate::locale::set(tag);
            let drawn = [
                // A bare argument.
                text_gen::app_action_bar_downloading_java("21"),
                // A plural, at a count each language has an arm for.
                text_gen::app_screenshots_selection_delete_description(1u64),
                text_gen::app_screenshots_selection_delete_description(2u64),
                text_gen::app_screenshots_selection_delete_description(0u64),
                // A typed number.
                text_gen::project_server_ping_ms(1234u64),
                // A select with a plural nested inside it.
                text_gen::time_frame_picker_last_timeframe(3u64, "hours"),
                // An argument used both bare and as a plural.
                text_gen::app_instance_confirm_delete_instances_label(2u64),
                // And a plain string, which is the 3,493 keys a table holds
                // verbatim.
                text_gen::Key::AppActionBarDownloads.message().to_string(),
            ];
            for sentence in &drawn {
                assert!(!sentence.is_empty(), "{tag} drew nothing");
                assert!(
                    !sentence.contains('{') && !sentence.contains('}'),
                    "{tag} left a template in the drawn sentence: {sentence}"
                );
            }
            assert!(
                !drawn[7].is_empty(),
                "{tag}: a plain message is never empty"
            );
        }
        crate::locale::set(crate::locale::ENGLISH);
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

    #[test]
    fn a_placeholder_is_split_out_of_the_sentence_around_it() {
        let notice = "This skin uses features from the {ears} mod";
        assert_eq!(
            placeholder_parts(notice, "ears"),
            Some(("This skin uses features from the ", " mod"))
        );
        // A sentence with no such slot, and one that merely mentions the word.
        assert_eq!(placeholder_parts("nothing to fill", "ears"), None);
        assert_eq!(placeholder_parts("braces but no slot: {}", "ears"), None);
    }
}
