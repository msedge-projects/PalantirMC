//! The language the interface is in: the chosen tag, the table behind it, the
//! fallback when the table has nothing to say, and the writing direction.
//!
//! The tables are [`crate::locale_gen`], compiled from the reference's own
//! locale trees by `tools/gen_locale.py`. This module is the runtime: which
//! table is in force, what a key is called in it, and -- the part that is not a
//! lookup -- which plural arm a count selects.
//!
//! ## Why the choice is ambient, and why it is per thread
//!
//! Every string the interface draws arrives through [`crate::text_gen::Key`], and
//! there are 3,846 of them at a call site that has no room for a parameter: a
//! page draws `Key::Foo.message()` because that is the sentence the reference
//! puts there. Threading a `Language` through all of them would be a rewrite of
//! every page to say what the interface already knows, so the answer is ambient
//! -- one window, one choice -- and [`set`] is what changes it.
//!
//! Ambient, and **per thread** rather than per process. iced runs an
//! `Application`'s `update` and `view` on the thread that started it, so one
//! window sees one value, and the interior mutability costs a `Cell` read.
//! A process-wide value would also be shared by every test, which is the real
//! argument: a test that puts German in force would put it in force for the
//! render test running beside it, and a suite that fails depending on the order a
//! runner happened to schedule two threads is worse than no test.
//!
//! ## The fallback
//!
//! A locale translates a subset of English's keys -- `ar-SA` carries 1,577 of
//! 3,846 -- and a key it does not carry renders English. That is not this
//! launcher's invention: the reference's own i18n config sets
//! `fallbackLocale: 'en-US'`, and [`lookup`] is that rule made concrete. It
//! returns `None` rather than English's string, so that the caller can take the
//! English path it already had -- which is what keeps English exactly as it was
//! before there was a language setting, rather than passing every string through
//! a second renderer that merely aims to agree.
//!
//! ## The offer is 32 of the 33 tables
//!
//! [`OFFERED`] is the reference's own `LOCALES`, quoted from
//! `ui/src/composables/i18n.ts`. It lists 32 codes and has `ar-SA` commented out
//! -- `Commented out as it's RTL - will enable when we have better RTL support`
//! -- so this launcher compiles that table (`locale_gen::ALL` has 33 entries) and
//! does not offer it, because offering a language the reference declines to
//! render is a promise about this launcher rather than a port of that one. The
//! English table has no `locale.ar-SA` name key either, which is the same
//! decision read from the other side.

use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::locale_gen;
use crate::text_gen::Key;

/// English, which is the default and the fallback.
pub const ENGLISH: &str = "en-US";

/// The languages the reference offers, in its own order.
///
/// Quoted from `LOCALES` in `ui/src/composables/i18n.ts`, minus the one it
/// comments out. The order is the reference's -- not sorted, so a list of
/// languages reads the way its own settings pane reads -- and it is the list the
/// language row draws while [`locale_gen::ALL`] is the list of tables.
pub const OFFERED: [&str; 32] = [
    "cs-CZ", "da-DK", "de-CH", "de-DE", "en-US", "es-419", "es-ES", "fi-FI", "fil-PH",
    "fr-FR", "he-IL", "hu-HU", "id-ID", "it-IT", "ja-JP", "ko-KR", "ms-MY", "nl-NL",
    "no-NO", "pl-PL", "pt-BR", "pt-PT", "ro-RO", "ru-RU", "sr-CS", "sv-SE", "th-TH",
    "tr-TR", "uk-UA", "vi-VN", "zh-CN", "zh-TW",
];

/// Which way the interface runs.
///
/// A property of the locale rather than of a page, because the reference keeps
/// it that way: `dir` is a field of a `LocaleDefinition`, and the CSS it lays
/// out with is flex order that a `dir="rtl"` reverses. [`crate::locale_gen`]
/// carries the flag per table so this cannot drift from the vendored list.
/// `#[allow(dead_code)]` because nothing lays out from it yet. G128 mirrors the
/// shell's rail, its panel and every row on this flag, and until that slice the
/// direction is a property of the vendored `LOCALES` data that this module keeps
/// and its tests read. Deleting it until it is painted would mean re-deriving it
/// from `i18n.ts` in the slice that needs it, which is exactly the drift the
/// generated table exists to prevent.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Left to right, which is every locale but two.
    Ltr,
    /// Right to left.
    Rtl,
}

#[allow(dead_code)]
impl Direction {
    /// Whether this is the right-to-left direction.
    pub const fn is_rtl(self) -> bool {
        matches!(self, Direction::Rtl)
    }

    /// The value the reference writes in `dir`, for a test to compare against.
    pub const fn as_str(self) -> &'static str {
        match self {
            Direction::Ltr => "ltr",
            Direction::Rtl => "rtl",
        }
    }
}

thread_local! {
    /// The index into [`locale_gen::ALL`] of the locale in force, **plus one**.
    ///
    /// Plus one because a `Cell` starts at zero and zero has to mean "nobody has
    /// chosen", which is English. A bare index would make zero `ar-SA` -- the
    /// first table by tag -- and the wrong default would only ever show up in
    /// Arabic.
    static ACTIVE: Cell<usize> = const { Cell::new(0) };
}

/// The position of a tag in the tables, if this build has one.
fn index_of(tag: &str) -> Option<usize> {
    locale_gen::ALL.iter().position(|locale| locale.tag == tag)
}

/// The position of English, which every fallback resolves to.
fn english_index() -> usize {
    index_of(ENGLISH).unwrap_or(0)
}

/// The locale in force.
pub fn active() -> &'static locale_gen::Locale {
    let chosen = ACTIVE.with(Cell::get);
    if chosen == 0 {
        return &locale_gen::ALL[english_index()];
    }
    // A stored index always came from `index_of`, so it is in range; the
    // fallback is for the shape of the code rather than for a case that happens.
    locale_gen::ALL.get(chosen - 1).unwrap_or(&locale_gen::ALL[english_index()])
}

/// The tag of the locale in force.
pub fn tag() -> &'static str {
    active().tag
}

/// Which way the interface runs in the locale in force.
#[allow(dead_code)]
pub fn direction() -> Direction {
    if active().rtl {
        Direction::Rtl
    } else {
        Direction::Ltr
    }
}

/// Whether the interface runs right to left.
#[allow(dead_code)]
pub fn is_rtl() -> bool {
    direction().is_rtl()
}

/// Put a language in force.
///
/// An unknown tag is English rather than an error, for the same reason
/// [`crate::color_theme::ColorTheme::from_id`] falls back: a preferences file
/// written by a newer build, or hand-edited, must still open. The empty string
/// is the same case -- "nothing chosen" -- and is what [`crate::prefs`] stores.
pub fn set(tag: &str) {
    ACTIVE.with(|active| active.set(resolve(tag).1 + 1));
}

/// The table a tag names, and its position, without putting it in force.
///
/// [`set`] is this plus a write, and a test that wants to read one language's
/// table while another is in force has only this to call.
fn resolve(tag: &str) -> (&'static locale_gen::Locale, usize) {
    let wanted = if tag.trim().is_empty() { ENGLISH } else { tag.trim() };
    let chosen = index_of(wanted).unwrap_or_else(english_index);
    (&locale_gen::ALL[chosen], chosen)
}

/// The table entry for an English key in the locale in force, if it has one.
///
/// `None` means *render English*, and it is returned for English itself as well
/// as for a key a locale does not carry. See the module doc for why that is a
/// `None` rather than English's own string.
pub fn lookup(key: Key) -> Option<&'static str> {
    translated(key as usize)
}

/// [`lookup`] by table position, which is how `text_gen`'s generated `message`
/// calls it without depending on anything but a number.
pub fn translated(index: usize) -> Option<&'static str> {
    translated_in(active(), index)
}

/// [`translated`] with the table named outright.
///
/// The split is what lets a test read German while English is in force: the
/// language is a parameter here and ambient in [`translated`].
pub fn translated_in(locale: &'static locale_gen::Locale, index: usize) -> Option<&'static str> {
    if locale.tag == ENGLISH {
        return None;
    }
    // `u16` because the English table is 3,846 entries and a table is a sparse
    // subset of it; see `tools/gen_locale.py` for why the position rather than
    // the key string is what a table holds.
    let wanted = u16::try_from(index).ok()?;
    let entries = locale.entries;
    let found = entries.binary_search_by_key(&wanted, |pair| pair.0).ok()?;
    Some(entries[found].1)
}

/// The reference's own name for a language, as a table key.
///
/// The position is resolved by `tools/gen_locale.py`, because a runtime scan for
/// a name would walk 3,846 keys 32 times to draw one list. Every one of the 32
/// offered codes has a name in the English locale, so `None` is a guard for a
/// tree that grows a code with no `locale.<tag>` in English -- which is what
/// [`label`] answers with the tag rather than with an invented name.
pub fn label_key(tag: &str) -> Option<Key> {
    let locale = locale_gen::ALL.iter().find(|locale| locale.tag == tag)?;
    let position = locale.label? as usize;
    crate::text_gen::ALL.get(position).copied()
}

/// What the reference calls a language, in the language in force when it can.
///
/// The label is the reference's own `locale.<tag>` message -- the same key its
/// `translatedName` names -- so a list of languages is the reference's words for
/// them rather than a second table of names kept beside the generated one, and a
/// label is itself translated once a language is in force. A code with no name in
/// English is answered with the tag itself, which is honest about a gap rather
/// than inventing a name this launcher would then have to translate.
pub fn label(tag: &str) -> String {
    match label_key(tag) {
        Some(key) => key.message().to_string(),
        None => tag.to_string(),
    }
}

/// What the reference calls a category, in the language in force.
///
/// `ui/src/utils/tag-messages.ts`'s `formatCategory`: the reference's own message
/// for the tag when it publishes one (`tag.category.kitchen-sink` is *Kitchen
/// Sink*), and `capitalizeString` when it does not.
///
/// Both halves are the reference's rule rather than a convenience. Capitalising a
/// tag that *has* a message would give *Gui* where the reference says *GUI*, and
/// the fallback exists because Modrinth publishes tags the message table has not
/// caught up with -- `formatCategory` is not a lookup that can fail, it is a
/// lookup with an arm for the gap.
pub fn category_label(name: &str) -> String {
    tag_label("tag.category.", name)
}

/// [`category_label`] over a category *header* -- `technical` is *Technical*, and
/// the header is what one of the browse sidebar's sections is named from.
pub fn category_header_label(header: &str) -> String {
    match header_key(header) {
        Some(key) => match crate::text_gen::from_name(key) {
            Some(found) => lookup(found).unwrap_or_else(|| found.message()).to_string(),
            None => capitalize(header),
        },
        None => capitalize(header),
    }
}

/// The reference's own `categoryHeaderMessages`, as a lookup.
///
/// **Keyed by the API's header names, not by the message ids**, because the two
/// differ: the API calls the header `categories` and the reference's table keys it
/// that way while its *message id* is `header.category.category` -- singular --
/// and the sentence it prints is *Category*. Building the id by pasting the header
/// into `header.category.` gets that one wrong and would print *Categories*.
///
/// Transcribed rather than derived, because the reference's table is eight
/// entries long and one of them is spelled differently from its id. A header the
/// table does not have falls back to [`capitalize`], which is the reference's own
/// fallback in `formatCategoryHeader`.
fn header_key(header: &str) -> Option<&'static str> {
    Some(match header {
        "resolutions" => "header.category.resolutions",
        "categories" => "header.category.category",
        "features" => "header.category.feature",
        "performance-impact" => "header.category.performance-impact",
        "minecraft_server_community" => "header.category.minecraft-server-community",
        "minecraft_server_features" => "header.category.minecraft-server-features",
        "minecraft_server_gameplay" => "header.category.minecraft-server-gameplay",
        "minecraft_server_meta" => "header.category.minecraft-server-meta",
        _ => return None,
    })
}

/// What the reference calls a loader, in the language in force.
///
/// `formatLoader`, which is [`category_label`] over `tag.loader.` -- the two
/// tables are separate in the reference because a name can be both a loader and
/// a category (`minecraft` is a loader for resource packs and a category for
/// mods), and the type is what says which one is meant.
pub fn loader_label(name: &str) -> String {
    tag_label("tag.loader.", name)
}

/// The one shape the three of them share: look the tag up as a message, and
/// capitalise it if the reference has no message for it.
///
/// [`text_gen::from_name`] is a binary search over the generated key table, and
/// the key is the reference's own (`Key::name` is it verbatim), so a tag this
/// launcher has never seen is still a lookup rather than a table of its own.
fn tag_label(prefix: &str, name: &str) -> String {
    match crate::text_gen::from_name(&format!("{prefix}{name}")) {
        Some(key) => lookup(key).unwrap_or_else(|| key.message()).to_string(),
        None => capitalize(name),
    }
}

/// `capitalizeString`: the first character up, the rest left alone.
///
/// Not a title case and not a sentence case -- *Mrpack*, *Modded*, *Gui* -- which
/// is why it is a helper rather than `to_uppercase`.
fn capitalize(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// How much of the interface a language carries, as the reference's own language
/// settings print it beside each name.
///
/// `language-settings-coverage.generated.ts` is written by a generator that counts
/// exactly what this counts -- a key the table carries is a key that language
/// translates -- and `CheckCircleButton` rows show the number with
/// `{percentage}% supported`. So the count is real rather than decorative: German
/// falls back for 54 of the 3,846 keys, `ar-SA` for 2,269.
///
/// One walk per language, and once: the table is walked 33 times the first time a
/// settings pane asks, which is 127 thousand binary searches and then no more.
/// Recomputing it on every repaint would be a settings dialog that walks its whole
/// corpus sixty times a second.
pub fn coverage(tag: &str) -> Option<u32> {
    static TABLE: std::sync::OnceLock<std::sync::Mutex<BTreeMap<&'static str, u32>>> =
        std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| std::sync::Mutex::new(BTreeMap::new()));
    let mut table = table.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // The table is keyed by the tag *it* carries rather than by the one that was
    // asked about, so the key is the reference's own `&'static str`.
    let Some(locale) = locale_gen::ALL.iter().find(|locale| locale.tag == tag) else {
        return None;
    };
    if let Some(found) = table.get(locale.tag) {
        return Some(*found);
    }
    let total = crate::text_gen::ALL.len();
    if total == 0 {
        return None;
    }
    // English is `None` for every key by the fallback's own shape, so it is
    // counted here rather than through the table: the language the interface
    // ships in carries all of it.
    let carried = crate::text_gen::ALL
        .into_iter()
        .filter(|key| locale.tag == ENGLISH || translated_in(locale, *key as usize).is_some())
        .count();
    let percent = ((carried * 100) / total) as u32;
    table.insert(locale.tag, percent);
    Some(percent)
}

/// The offered languages in the order the reference's own list draws them.
///
/// `language-settings-selector.vue` sorts what it builds --
/// `result.sort((a, b) => (b.coverage?.percentage ?? -1) - (a.coverage?.percentage ?? -1))`
/// -- so the pane reads most-covered first and the language in force is at the
/// top of it, which is where the capture finds it. A language with no coverage
/// sorts last, as `?? -1` says.
///
/// The sort is stable on both sides: `Array.prototype.sort` and [`slice::sort_by`]
/// keep equal elements in the order they arrived, so languages that tie keep
/// [`OFFERED`]'s order and two runs of this cannot disagree.
pub fn offered_by_coverage() -> Vec<&'static str> {
    // `?? -1` rather than a plain `Option` order, which would put a language the
    // corpus cannot measure *first*: `None` is below every number here.
    let mut offered: Vec<&'static str> = OFFERED.to_vec();
    offered.sort_by(|a, b| match (coverage(a), coverage(b)) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    offered
}

/// A language's own name in itself, which the reference prints beside the name in
/// the language in force when the two are not the same words.
///
/// The reference reads it as `loc.translatedName` -- a message per locale that is
/// rendered in *that* locale, so German reads "Deutsch" while an English reader
/// reads the same message as "German". It is the one string in the list that
/// differs by definition between the two readings, which is why it is `None`
/// rather than the same words twice.
pub fn translated_label(tag: &str) -> Option<String> {
    let key = label_key(tag)?;
    let locale = locale_gen::ALL.iter().find(|locale| locale.tag == tag)?;
    let in_itself = translated_in(locale, key as usize)?.to_string();
    let in_force = label(tag);
    (in_itself != in_force).then_some(in_itself)
}

/// The plural category a count falls into, for a language.///
/// This is CLDR's cardinal rule for the languages in this corpus, and it is the
/// runtime half of the table `tools/gen_locale.py` validates against -- that
/// tool's `CLDR_CATEGORIES` is the other half, and a Rust test in this module
/// compares the two so a language cannot be compiled with one set of categories
/// and rendered with another.
///
/// `v = 0` throughout, because every value that arrives here is an integer: the
/// reference's own counts are, and the one place it pluralizes something else --
/// `formatCompactNumberPlural`, which produces `"1.2K"` -- passes a *category*
/// that the caller has already decided, not a number. That is why a fractional
/// `many` for `cs-CZ` cannot be produced here, and the arms it has for one are
/// as unreachable in this launcher as they are in the reference, whose
/// `Intl.PluralRules` is being reproduced.
///
/// A language not named below is `other` alone, which is the rule for the
/// languages this corpus does not contain and the safe answer for one it grows:
/// `other` is the arm every message must have.
pub fn category(language: &str, value: u64) -> &'static str {
    let n = value;
    let i = value;
    let i10 = i % 10;
    let i100 = i % 100;
    match language {
        "ar" => match n {
            0 => "zero",
            1 => "one",
            2 => "two",
            _ if (3..=10).contains(&i100) => "few",
            _ if (11..=99).contains(&i100) => "many",
            _ => "other",
        },
        "cs" => {
            if i == 1 {
                "one"
            } else if (2..=4).contains(&i) {
                "few"
            } else {
                "other"
            }
        }
        "he" => {
            if i == 1 {
                "one"
            } else if i == 2 {
                "two"
            } else if n != 0 && i10 == 0 {
                "many"
            } else {
                "other"
            }
        }
        "pl" => {
            if i == 1 {
                "one"
            } else if (2..=4).contains(&i10) && !(12..=14).contains(&i100) {
                "few"
            } else if i10 <= 1 || (5..=9).contains(&i10) || (12..=14).contains(&i100) {
                "many"
            } else {
                "other"
            }
        }
        "ro" => {
            if i == 1 {
                "one"
            } else if n == 0 || (n % 100) <= 19 {
                "few"
            } else {
                "other"
            }
        }
        "ru" | "uk" => {
            if i10 == 1 && i100 != 11 {
                "one"
            } else if (2..=4).contains(&i10) && !(12..=14).contains(&i100) {
                "few"
            } else if i10 == 0 || (5..=9).contains(&i10) || (11..=14).contains(&i100) {
                "many"
            } else {
                "other"
            }
        }
        "sr" => {
            if i10 == 1 && i100 != 11 {
                "one"
            } else if (2..=4).contains(&i10) && !(12..=14).contains(&i100) {
                "few"
            } else {
                "other"
            }
        }
        // French and Portuguese count zero as `one`; the rest of the
        // one/other languages count only one.
        "fr" | "pt" => {
            if i <= 1 {
                "one"
            } else {
                "other"
            }
        }
        // `fil` is the rule with a hole in it: `one` is every count whose last
        // digit is not 4, 6 or 9, so 5 and 0 are `one` and 4 is not. Written as
        // the rule rather than as a list of exceptions, which is how CLDR states
        // it and the only way it stays right at 104.
        "fil" => {
            if !matches!(i10, 4 | 6 | 9) {
                "one"
            } else {
                "other"
            }
        }
        // Languages with a single form: `other`, always.
        "id" | "ja" | "ko" | "ms" | "th" | "vi" | "zh" => "other",
        // `de`, `en`, `es`, `fi`, `hu`, `it`, `nl`, `no`, `sv`, `tr` and
        // anything unnamed: one is exactly one.
        _ => {
            if i == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

/// The thousands separator a language groups with.
///
/// CLDR's `group` symbol for the languages in this corpus, which is what the
/// reference's `Intl.NumberFormat` uses for a `{count, number}` and for the `#`
/// inside a plural arm. Three values cover all 28: a full stop for most of
/// Europe and South-East Asia, a non-breaking space for the Slavic and Nordic
/// languages that separate hundreds from thousands with one, and a comma for the
/// rest. A language not named below is a comma, which is English's and the most
/// common answer.
///
/// **What is deliberately not here is digit substitution.**
/// `Intl.NumberFormat('ar-SA')` writes 1,234 in Arabic-Indic digits
/// (`١٬٢٣٤`), and this renderer writes Western ones. French is the other edge:
/// modern CLDR uses a *narrow* no-break space for its groups and this uses the
/// ordinary one, so a grouped French number is right except for that code point.
/// Both are recorded in `GATES.md` rather than approximated, because a second
/// table of digit shapes is a table nobody measured.
fn separator(language: &str) -> &'static str {
    match language {
        "da" | "de" | "es" | "id" | "it" | "ms" | "nl" | "pt" | "ro" | "sr" | "tr" | "vi" => ".",
        "cs" | "fi" | "fr" | "hu" | "no" | "pl" | "ru" | "sv" | "uk" => "\u{a0}",
        _ => ",",
    }
}

/// A count in the reference's compact form: `formatCompactNumber`.
///
/// `ui/src/composables/format-number.ts`'s three cases, with its own thresholds and
/// its own fraction digits:
///
/// | Count | Reference | This |
/// | --- | --- | --- |
/// | 9,999 | `9,999`, not compact at all | [`group`] |
/// | 12,345 | one digit, `12.3K` | the same |
/// | 999,999 | rounds up and promotes, `1M` | the same |
/// | 41,000,000 | two digits, `41M` | the same |
///
/// The million-and-up case is what a project card's own counts are, which is why
/// this exists: a card that wrote `41,000,000 downloads` beside a download icon is a
/// card three times the width of the reference's, and the reference puts the full
/// count in a tooltip this kit does not draw.
///
/// **The suffix and the fraction digit are English's**, and deliberately so rather
/// than by omission: `Intl.NumberFormat` abbreviates per language (`de` writes
/// `1,2\u{a0}Mio.`), and every count this launcher draws a number *into* is English
/// already -- the plural helpers in `text_gen` are generated from English's own
/// messages. A localized suffix table is a second table of strings nobody here has
/// measured, which is the one thing this module refuses to invent. The grouping
/// under ten thousand, which is not a suffix, is the language's own.
pub fn compact(language: &str, value: u64) -> String {
    // Under ten thousand the reference does not abbreviate: `9,999` is shorter than
    // `10.0K` and nobody reads `9.9K` for a count of 9,999.
    if value < 10_000 {
        return group(language, value);
    }
    // The unit, and how many fraction digits the reference allows under it.
    const UNITS: [(u64, &str, usize); 4] = [
        (1_000_000_000_000, "T", 2),
        (1_000_000_000, "B", 2),
        (1_000_000, "M", 2),
        (1_000, "K", 1),
    ];
    let mut chosen = UNITS[UNITS.len() - 1];
    for unit in UNITS {
        if value >= unit.0 {
            chosen = unit;
            break;
        }
    }
    let (mut scale, mut suffix, mut digits) = chosen;
    let mut mantissa = round(value as f64 / scale as f64, digits);
    // `Intl` rounds the mantissa and *then* promotes a unit that has rounded up to a
    // thousand: 999,999 is `1M` rather than `1000K`. One step is enough, because the
    // only way to reach a thousand is to be within a rounding of it -- and
    // `rposition`, because the table is in descending order and the step up is the
    // *smallest* unit bigger than the one in hand.
    if mantissa >= 1000.0 {
        if let Some(bigger) = UNITS.iter().rposition(|unit| unit.0 > scale) {
            scale = UNITS[bigger].0;
            suffix = UNITS[bigger].1;
            digits = UNITS[bigger].2;
            mantissa = round(value as f64 / scale as f64, digits);
        }
    }
    // `format!` rather than a hand-rolled trim: the trailing zeros `41.00` must go
    // and the fraction that survives must keep the digit it was rounded to, and
    // `format!`'s own `{:.*}` is the one place that rule already lives.
    let text = format!("{mantissa:.digits$}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed}{suffix}")
}

/// `value` rounded to `digits` decimal places.
fn round(value: f64, digits: usize) -> f64 {
    let factor = 10f64.powi(digits as i32);
    (value * factor).round() / factor
}

/// An integer with a language's own grouping.
///
/// The same arithmetic as [`crate::text::number`] -- a separator before every
/// third digit counting from the right, and never before the first -- with the
/// separator the language uses instead of a comma. English's own table is not
/// routed through here: the generated helpers format with `text::number`, which
/// is the code that ran before there was a language setting.
pub fn group(language: &str, value: u64) -> String {
    let digits = value.to_string();
    let separator = separator(language);
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push_str(separator);
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_is_named_by_the_reference_s_own_message_or_by_its_first_letter() {
        set("");
        // The message arm: `tag.category.kitchen-sink` is *Kitchen Sink* in the
        // reference, not *Kitchen-sink*.
        assert_eq!(category_label("kitchen-sink"), "Kitchen Sink");
        assert_eq!(category_label("optimization"), "Optimization");
        // And the message arm is not a capitalisation: `gui` has a message and it
        // is *GUI*.
        assert_eq!(category_label("gui"), "GUI");
        assert_eq!(category_label("pokemon"), "Pokémon");

        // The fallback arm: Modrinth publishes tags the message table has not
        // caught up with, and the reference capitalises those rather than
        // printing the slug.
        assert_eq!(category_label("not-a-real-category"), "Not-a-real-category");
        assert_eq!(category_label(""), "");
        assert_eq!(loader_label("not-a-real-loader"), "Not-a-real-loader");
        assert_eq!(loader_label("legacy-fabric"), "Legacy Fabric");

        // A header is its own table, and its own message. The live API calls the
        // one every project type has `categories`, while the reference spells its
        // message id in the singular -- so the sentence is *Category* and not
        // *Categories*, which is what a pasted-together id would have printed.
        assert_eq!(category_header_label("categories"), "Category");
        assert_eq!(category_header_label("technical"), "Technical");
        assert_eq!(category_header_label("performance-impact"), "Performance impact");
        assert_eq!(category_header_label("resolutions"), "Resolution");
        assert_eq!(category_header_label("features"), "Feature");
        assert_eq!(category_header_label("minecraft_server_gameplay"), "Gameplay");
        assert_eq!(category_header_label("game-mechanics"), "Game-mechanics");
    }

    #[test]
    fn english_is_the_default_and_an_unknown_tag_opens_as_it() {
        set("");
        assert_eq!(tag(), ENGLISH);
        assert_eq!(direction(), Direction::Ltr);
        set("kl-GL");
        assert_eq!(tag(), ENGLISH, "a language this build has never heard of is English");
        set("  ");
        assert_eq!(tag(), ENGLISH);
    }

    #[test]
    fn a_chosen_tag_survives_and_names_its_own_table() {
        set("de-DE");
        assert_eq!(tag(), "de-DE");
        assert_eq!(active().language, "de");
        assert_eq!(direction(), Direction::Ltr);
        set("he-IL");
        assert_eq!(tag(), "he-IL");
        assert!(is_rtl(), "the reference declares he-IL dir: 'rtl'");
        set("ar-SA");
        assert!(
            is_rtl(),
            "the table is compiled and its direction is recorded, offered or not"
        );
        set(ENGLISH);
        assert!(!is_rtl());
    }

    #[test]
    fn the_offer_is_the_reference_s_own_list_and_ar_sa_is_not_in_it() {
        assert_eq!(OFFERED.len(), 32);
        assert!(OFFERED.contains(&"he-IL"));
        assert!(
            !OFFERED.contains(&"ar-SA"),
            "LOCALES comments ar-SA out as RTL; the table exists and is not offered"
        );
        assert_eq!(locale_gen::ALL.len(), 33, "33 trees are compiled");
        // The offered list is a subset of the tables, so a label can always be
        // resolved against one.
        for offered in OFFERED {
            assert!(index_of(offered).is_some(), "{offered} has no table");
        }
        // And the two lists differ by exactly the one the reference excludes.
        let extra: Vec<&str> = locale_gen::ALL
            .iter()
            .map(|locale| locale.tag)
            .filter(|tag| !OFFERED.contains(tag))
            .collect();
        assert_eq!(extra, ["ar-SA"]);
    }

    #[test]
    fn the_pane_s_order_is_most_covered_first_and_keeps_the_offer_s_order_within_a_tie() {
        let offered = offered_by_coverage();
        assert_eq!(offered.len(), OFFERED.len(), "every offered language is still offered");
        assert_eq!(
            offered[0], ENGLISH,
            "the language the interface ships in carries every key, so it sorts first \
             and the pane opens on it -- which is where the capture finds it"
        );
        // Descending by the same number the row prints, and nothing dropped: a
        // sort that lost a language would be a list the search cannot find.
        let percentages: Vec<u32> = offered.iter().filter_map(|tag| coverage(tag)).collect();
        let mut descending = percentages.clone();
        descending.sort_by(|a, b| b.cmp(a));
        assert_eq!(percentages, descending);
        assert!(offered.contains(&"he-IL"), "the RTL language is offered and sorted");
        // `Array.prototype.sort` is stable, so two languages that measure the
        // same keep the order `LOCALES` lists them in.
        for pair in offered.windows(2) {
            if coverage(pair[0]) == coverage(pair[1]) {
                let first = OFFERED.iter().position(|tag| *tag == pair[0]);
                let second = OFFERED.iter().position(|tag| *tag == pair[1]);
                assert!(first < second, "{:?} and {:?} tie and are out of order", pair[0], pair[1]);
            }
        }
    }

    #[test]
    fn a_key_a_locale_does_not_carry_falls_back_and_a_key_it_does_does_not() {
        // English is `None` for everything -- that is the fallback's own shape.
        set(ENGLISH);
        assert_eq!(lookup(Key::AppActionBarDownloads), None);

        set("de-DE");
        // German carries this one; the reference's own wording, not a retyping.
        let downloads = lookup(Key::AppActionBarDownloads).expect("de-DE translates the rail");
        assert_eq!(downloads, "Downloads");
        // `de-DE` falls back for 54 keys, and any of them must be `None`.
        let missing = crate::text_gen::ALL
            .into_iter()
            .filter(|key| lookup(*key).is_none())
            .count();
        assert_eq!(missing, 54, "de-DE's own fallback count, from gen_locale --report");

        // And a locale with far less coverage falls back a lot more.
        set("ar-SA");
        let missing = crate::text_gen::ALL
            .into_iter()
            .filter(|key| lookup(*key).is_none())
            .count();
        assert_eq!(missing, 2269, "ar-SA translates 1,577 of 3,846 keys");
        set(ENGLISH);
    }

    #[test]
    fn a_translated_key_is_the_locale_s_own_sentence() {
        // The same key the English table's own quoted sample uses, read in German
        // and in a language that is not written with Latin letters.
        set("de-DE");
        assert_eq!(Key::SettingsDisplayThemeDark.message(), "Dunkel");
        set("ru-RU");
        assert_ne!(
            Key::SettingsDisplayThemeDark.message(),
            Key::SettingsDisplayThemeDark.name(),
            "a translated key must not fall back for a key the locale carries"
        );
        set(ENGLISH);
        assert_eq!(Key::SettingsDisplayThemeDark.message(), "Dark");
    }

    #[test]
    fn the_direction_is_the_reference_s_own_field() {
        // Every table's flag, against the two codes the reference marks.
        let rtl: Vec<&str> = locale_gen::ALL
            .iter()
            .filter(|locale| locale.rtl)
            .map(|locale| locale.tag)
            .collect();
        assert_eq!(rtl, ["ar-SA", "he-IL"]);
        // And `direction` reads that field rather than a second list.
        for tag in OFFERED {
            set(tag);
            assert_eq!(is_rtl(), tag == "he-IL", "{tag}");
            assert_eq!(direction().as_str(), if tag == "he-IL" { "rtl" } else { "ltr" });
        }
        set(ENGLISH);
    }

    #[test]
    fn the_plural_rule_is_each_language_s_own() {
        // Arabic has all six, and they are the numbers CLDR names them by.
        for (value, expected) in
            [(0u64, "zero"), (1, "one"), (2, "two"), (3, "few"), (10, "few"), (11, "many"), (99, "many"), (100, "other"), (103, "few")]
        {
            assert_eq!(category("ar", value), expected, "ar {value}");
        }
        // Russian and Polish are the same shape with a different one.
        for (value, expected) in
            [(1u64, "one"), (2, "few"), (5, "many"), (11, "many"), (21, "one"), (22, "few"), (25, "many")]
        {
            assert_eq!(category("ru", value), expected, "ru {value}");
        }
        assert_eq!(category("pl", 1), "one");
        assert_eq!(category("pl", 3), "few");
        assert_eq!(category("pl", 5), "many");
        assert_eq!(category("pl", 12), "many");
        assert_eq!(category("pl", 22), "few");
        // French counts zero as one; English does not.
        assert_eq!(category("fr", 0), "one");
        assert_eq!(category("en", 0), "other");
        assert_eq!(category("pt", 0), "one");
        assert_eq!(category("pt", 1), "one");
        assert_eq!(category("de", 1), "one");
        assert_eq!(category("de", 2), "other");
        // A single-form language is `other` for everything, including one.
        for language in ["ja", "ko", "zh", "th", "vi", "id", "ms"] {
            for value in [0u64, 1, 2, 11, 100] {
                assert_eq!(category(language, value), "other", "{language} {value}");
            }
        }
        // The rule with a hole in it.
        assert_eq!(category("fil", 3), "one");
        assert_eq!(category("fil", 4), "other");
        assert_eq!(category("fil", 5), "one");
        assert_eq!(category("fil", 9), "other");
        assert_eq!(category("fil", 10), "one");
    }

    /// The runtime rule and the generator's table have to agree, or a locale is
    /// compiled against one set of categories and rendered with another. The
    /// generator writes `CLDR_CATEGORIES` in Python; this is the Rust half.
    ///
    /// The comparison is **containment, not equality**, and the next test is why:
    /// a category set includes the arms of the *fractional* rule (`v != 0`), and
    /// every count this launcher hands to a rule is an integer, so a rule can
    /// legitimately produce fewer than its table lists. What it may never do is
    /// produce one the table does not have -- that would select an arm the copied
    /// locale was never compiled to be able to carry.
    #[test]
    fn every_category_the_rule_produces_is_one_the_generator_compiles_against() {
        // The languages the corpus is in, and the categories the tool's table
        // lists for them, as pairs a reader can disagree with.
        let declared: [(&str, &[&str]); 29] = [
            ("ar", &["zero", "one", "two", "few", "many", "other"]),
            ("cs", &["one", "few", "other"]),
            ("da", &["one", "other"]),
            ("de", &["one", "other"]),
            ("en", &["one", "other"]),
            ("es", &["one", "other"]),
            ("fi", &["one", "other"]),
            ("fil", &["one", "other"]),
            ("fr", &["one", "other"]),
            ("he", &["one", "two", "many", "other"]),
            ("hu", &["one", "other"]),
            ("id", &["other"]),
            ("it", &["one", "other"]),
            ("ja", &["other"]),
            ("ko", &["other"]),
            ("ms", &["other"]),
            ("nl", &["one", "other"]),
            ("no", &["one", "other"]),
            ("pl", &["one", "few", "many", "other"]),
            ("pt", &["one", "other"]),
            ("ro", &["one", "few", "other"]),
            ("ru", &["one", "few", "many", "other"]),
            ("sr", &["one", "few", "other"]),
            ("sv", &["one", "other"]),
            ("th", &["other"]),
            ("tr", &["one", "other"]),
            ("uk", &["one", "few", "many", "other"]),
            ("vi", &["other"]),
            ("zh", &["other"]),
        ];
        for (language, declared) in declared {
            let mut produced: Vec<&str> = (0..1000u64)
                .map(|value| category(language, value))
                .collect();
            produced.sort_unstable();
            produced.dedup();
            for found in produced {
                assert!(
                    declared.contains(&found),
                    "{language} produced {found:?}, which its table does not list"
                );
            }
        }
    }

    /// The categories a language's table lists but its *integer* rule can never
    /// produce, pinned rather than left to be rediscovered.
    ///
    /// CLDR gives `cs` a `many` arm and `pl` an `other` arm for `v != 0`-style
    /// fractional counts. Every value that reaches [`category`] is a number -- the
    /// reference's counts are, and its one non-numeric case,
    /// `formatCompactNumberPlural`, passes a *category* the caller already chose,
    /// which never goes through a rule at all -- so those two arms are dead here.
    /// They are dead in the reference for the same reason: `Intl.PluralRules` is
    /// handed the same integers.
    ///
    /// This is the integer-level version of what `gen_locale.py --report` prints,
    /// which compares against the *category set* and therefore does not catch
    /// these two. It is written down because a Czech `many` arm in the copy looks
    /// like a live translation and is not.
    #[test]
    fn a_fractional_category_is_not_produced_for_an_integer_count() {
        let polish: Vec<&str> = (0..1000u64).map(|value| category("pl", value)).collect();
        assert!(!polish.contains(&"other"), "Polish `other` is its fractional arm");
        assert!(polish.contains(&"one") && polish.contains(&"few") && polish.contains(&"many"));
        let czech: Vec<&str> = (0..1000u64).map(|value| category("cs", value)).collect();
        assert!(!czech.contains(&"many"), "Czech `many` is its fractional arm");
        assert!(czech.contains(&"one") && czech.contains(&"few"));
    }

    #[test]
    fn a_count_is_abbreviated_the_way_the_reference_abbreviates_one() {
        // `format-number.ts`'s three cases, at the boundaries rather than in the
        // middle: 9,999 is the last count it does not abbreviate and 10,000 the
        // first it does, and each unit keeps its own fraction digits -- one under a
        // million, two above it.
        assert_eq!(compact("en", 0), "0");
        assert_eq!(compact("en", 999), "999");
        assert_eq!(compact("en", 9_999), "9,999");
        assert_eq!(compact("en", 10_000), "10K");
        assert_eq!(compact("en", 12_345), "12.3K");
        assert_eq!(compact("en", 999_949), "999.9K");
        // Rounded up to a thousand, the unit is promoted: `Intl` writes 999,999 as
        // `1M`, not as `1000K`.
        assert_eq!(compact("en", 999_999), "1M");
        assert_eq!(compact("en", 1_000_000), "1M");
        assert_eq!(compact("en", 1_234_567), "1.23M");
        assert_eq!(compact("en", 41_000_000), "41M");
        assert_eq!(compact("en", 1_500_000_000), "1.5B");
        assert_eq!(compact("en", 2_000_000_000_000), "2T");
        // The arm under ten thousand is the *language's* grouping, which is the one
        // part of this rule that is not English's own.
        assert_eq!(compact("de", 9_999), "9.999");
        assert_eq!(compact("ru", 9_999), "9\u{a0}999");
    }

    #[test]
    fn numbers_are_grouped_the_way_each_language_groups_them() {
        // Four digits, so there is a separator to be wrong about.
        assert_eq!(group("en", 1234), "1,234");
        assert_eq!(group("de", 1234), "1.234");
        assert_eq!(group("pt", 1234567), "1.234.567");
        assert_eq!(group("ru", 1234), "1\u{a0}234");
        assert_eq!(group("pl", 12345), "12\u{a0}345");
        assert_eq!(group("ja", 1234), "1,234");
        // Below a thousand there is nothing to separate, in any language.
        for language in ["en", "de", "ru", "ar", "ja"] {
            assert_eq!(group(language, 999), "999");
            assert_eq!(group(language, 0), "0");
        }
    }

    #[test]
    fn the_language_in_force_is_the_one_the_rule_reads() {
        // `text::render` asks the rule by `active().language`, so the choice and
        // the rule have to be the same table's.
        set("ar-SA");
        assert_eq!(category(active().language, 0), "zero");
        assert_eq!(category(active().language, 2), "two");
        set("ja-JP");
        assert_eq!(category(active().language, 1), "other");
        set(ENGLISH);
        assert_eq!(category(active().language, 1), "one");
    }

    #[test]
    fn a_language_is_labelled_with_the_reference_s_own_name() {
        set(ENGLISH);
        assert_eq!(label("de-DE"), "German (Germany)");
        assert_eq!(label("he-IL"), "Hebrew");
        assert_eq!(label("en-US"), "English (United States)");
        // A code whose name is not a bare word: the tag itself has a digit in it,
        // and the key that names it is `locale.es-419`.
        assert_eq!(label("es-419"), "Spanish (Latin America)");
        // And a label is itself translated once a language is in force.
        set("de-DE");
        assert_eq!(label("he-IL"), "Hebräisch");
        set(ENGLISH);
    }
}
