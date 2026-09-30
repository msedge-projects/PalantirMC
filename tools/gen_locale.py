#!/usr/bin/env python3
"""Compile the reference client's *other* locales into a Rust table.

`tools/gen_text.py` compiles the English locale into `text_gen.rs`, and this is
the same argument carried to the other 32: the reference is the only authority on
what its own interface says in German, and a translation retyped by hand is a
translation that drifts. The two tools share the ICU parser rather than each
having one -- [`gen_text`] is imported, not copied -- because a locale the
generator accepts and a locale the runtime renders have to agree about what a
message means.

    python tools/gen_locale.py            # write crates/palantir-desktop/src/locale_gen.rs
    python tools/gen_locale.py --check    # fail if that file is not what this emits
    python tools/gen_locale.py --report   # what each locale carries, and what it is missing
    python tools/gen_locale.py --only de-DE,pt-BR,few   # a subset, for the size measurement

## The two trees, and the 33 files that are not 33 languages

The reference merges two message trees for one locale -- `app-frontend/src/locales`
and `ui/src/locales` -- exactly as it does for English, and they share no key. All
33 tags exist in both trees, so 33 is how many *trees* there are.

It is **not** how many languages the reference offers. `LOCALES`, in
`ui/src/composables/i18n.ts`, lists 32 codes and `buildLocaleMessages` drops any
tree whose tag is not in it: `ar-SA` is present as files and **commented out in
that list**, with the comment `Commented out as it's RTL - will enable when we
have better RTL support`. So this tool compiles 33 tables -- the data is here and
refusing to compile it would be refusing a measurement -- while the offered list
is the reference's own 32, and `ar-SA` is compiled but not offered. That
distinction is `locale.rs`'s to keep and `GATES.md`'s to record.

## What a table is, and why it is sparse

A locale translates a subset of English's 3846 keys: 89,177 leaves across the 33
trees against 33 x 3846 = 126,918 slots. So a table is the reference's own sparse
shape -- `(index into text_gen::ALL, template)` -- sorted by index so a lookup is
a binary search, and a key that is not in it renders English. That row is what
`ui/src/composables/i18n.ts` sets as `fallbackLocale: 'en-US'` for the reference
itself, and it is the only honest statement of how translated a locale looks:
`ar-SA` carries 1,577 of 3,846 keys and falls back for the other 2,269.

The index rather than the key string is not a micro-optimisation. Every locale's
keys are a subset of English's (verified here, and refused if a tree ever grows
one that is not), so the key is already in the binary once; repeating 89,177 of
them would be 2.5 MB of duplicate text. The `--report` output prints both costs.

## The plural rules, and what is refused

`gen_text.py` refuses a plural category English has no rule for. That is the
wrong test for the other 32: `zero`, `two`, `few` and `many` are real arms in this
corpus -- 13 locales use `few`, 6 use `many`, `ar-SA` uses all six -- and a
language whose copy has them is not malformed ICU. So this tool compiles a locale
with **the whole CLDR category set** as legal arms, and separately records which
of a language's legal arms its CLDR rule can never select (Japanese copy carries
`one` arms; `Intl.PluralRules('ja').select(1)` is `other`, so they are dead in the
reference too). A dead arm is a measurement, not an error.

What is still refused is what is *genuinely* unsupported: an arm key that is not a
CLDR category and not `=N`, the `date`, `time`, `list`, `duration` and
`selectordinal` types, and ICU's single-apostrophe quoting. Any of those stops the
tool with the key's name, because a message rendered wrongly is worse than one
that visibly failed to compile.

`CLDR_CATEGORIES` below is the cardinal category set per language, and it is the
one place a rule lives on this side of the fence: the runtime's rule is
`crate::locale`, and a Rust test asserts the two agree about which categories a
language can produce.
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import sys

import gen_text

ROOT = gen_text.ROOT

# The two trees, in the order the reference merges them.
TREES = [
    pathlib.Path("vendor/modrinth-app/app-frontend/src/locales"),
    pathlib.Path("vendor/modrinth-app/ui/src/locales"),
]
OUTPUT = pathlib.Path("crates/palantir-desktop/src/locale_gen.rs")

# The CLDR cardinal category set per primary language subtag, for the languages
# this corpus is in. `en` is here because the runtime's fallback is English, and
# the two tables have to answer the same question about the same language.
#
# This is the *set a rule can produce*, which is not the same as the set the copy
# uses: Japanese can only produce `other`, and its copy still carries `one` arms.
# The difference is reported rather than refused -- a dead arm in a translation is
# upstream's fact, and the reference renders it as `other` too.
CLDR_CATEGORIES = {
    "ar": {"zero", "one", "two", "few", "many", "other"},
    "cs": {"one", "few", "many", "other"},
    "da": {"one", "other"},
    "de": {"one", "other"},
    "en": {"one", "other"},
    "es": {"one", "many", "other"},
    "fi": {"one", "other"},
    "fil": {"one", "other"},
    "fr": {"one", "many", "other"},
    "he": {"one", "two", "many", "other"},
    "hu": {"one", "other"},
    "id": {"other"},
    "it": {"one", "many", "other"},
    "ja": {"other"},
    "ko": {"other"},
    "ms": {"other"},
    "nl": {"one", "other"},
    "no": {"one", "other"},
    "pl": {"one", "few", "many", "other"},
    "pt": {"one", "many", "other"},
    "ro": {"one", "few", "other"},
    "ru": {"one", "few", "many", "other"},
    "sr": {"one", "few", "other"},
    "sv": {"one", "other"},
    "th": {"other"},
    "tr": {"one", "other"},
    "uk": {"one", "few", "many", "other"},
    "vi": {"other"},
    "zh": {"other"},
}

# The reference's own `dir: 'rtl'` entries, quoted from `LOCALES`. `ar-SA` is
# there because a tree is compiled even when it is not offered; `he-IL` is the
# only one of the 32 that is both offered and RTL.
RTL = {"ar-SA", "he-IL"}

# The order the reference's `LOCALES` lists its 32 codes in. Not used for the
# tables -- those are emitted by tag -- but `--report` prints coverage in it, so a
# reader comparing this output against `i18n.ts` is comparing like with like.
REFERENCE_OFFERED = [
    "cs-CZ", "da-DK", "de-CH", "de-DE", "en-US", "es-419", "es-ES", "fi-FI",
    "fil-PH", "fr-FR", "he-IL", "hu-HU", "id-ID", "it-IT", "ja-JP", "ko-KR",
    "ms-MY", "nl-NL", "no-NO", "pl-PL", "pt-BR", "pt-PT", "ro-RO", "ru-RU",
    "sr-CS", "sv-SE", "th-TH", "tr-TR", "uk-UA", "vi-VN", "zh-CN", "zh-TW",
]


def language_of(tag: str) -> str:
    """The primary subtag, which is what a plural rule is chosen by."""
    return tag.split("-")[0].lower()


def tags() -> list[str]:
    """Every locale tree on disk, in tag order."""
    return sorted(path.name for path in (ROOT / TREES[0]).iterdir() if path.is_dir())


def load_locale(tag: str) -> dict:
    """One locale's messages, both trees merged.

    Same refusal as `gen_text.load_all`: the reference spreads the two objects
    into one bag and does not expect a key in both, so a collision is a fact
    about upstream rather than something to paper over.
    """
    messages: dict = {}
    for tree in TREES:
        path = ROOT / tree / tag / "index.json"
        if not path.exists():
            raise gen_text.Refused(f"{tag}: {tree}/index.json is missing")
        for key, text in gen_text.leaves(json.loads(path.read_text(encoding="utf-8"))):
            if key in messages:
                raise gen_text.Refused(f"{tag}: {key} is in more than one tree")
            messages[key] = text
    return messages


def arms_used(messages: dict) -> collections.Counter:
    """Every plural arm key in a locale, by re-reading the templates.

    A second reading with a predicate that shares nothing with the parser: the
    parser says a locale is *legal*, and this says what it actually carries. The
    two disagreeing is the bug either one could have.
    """
    counter: collections.Counter = collections.Counter()
    for text in messages.values():
        if ", plural," not in text:
            continue
        index = 0
        while True:
            found = text.find(", plural,", index)
            if found < 0:
                break
            cursor = found + len(", plural,")
            depth = 0
            while cursor < len(text):
                if text[cursor] == "{":
                    depth += 1
                elif text[cursor] == "}":
                    if depth == 0:
                        break
                    depth -= 1
                cursor += 1
            block = text[found + len(", plural,"):cursor]
            inner = 0
            current = ""
            for character in block:
                if character == "{":
                    if inner == 0:
                        key = current.strip()
                        if key:
                            counter[key] += 1
                    inner += 1
                    current = ""
                elif character == "}":
                    inner -= 1
                    current = ""
                elif inner == 0:
                    current += character
            index = cursor
    return counter


def validate(tag: str, messages: dict, counts: gen_text.Counts) -> dict:
    """Parse every message, stopping with the key's name on anything unsupported.

    The allowed arms are the whole CLDR set rather than the language's own: an arm
    a language's rule cannot select is a dead arm in a translation, which is
    upstream's to have, not a construct this tool may refuse. What it may refuse
    is a key that is not a category at all, or an ICU type the reference's copy is
    not supposed to use -- see the module docstring.
    """
    parsed = {}
    for key in sorted(messages):
        try:
            parsed[key] = gen_text.parse_message(
                counts, messages[key], gen_text.ALL_PLURAL_CATEGORIES
            )
        except gen_text.Refused as refusal:
            raise gen_text.Refused(f"{tag}: {key}: {refusal}") from None
    return parsed


def build_lines(tag_sources, english_keys):
    index = {key: position for position, key in enumerate(english_keys)}
    body: list = []
    total_entries = 0
    total_value_bytes = 0
    for tag, messages in tag_sources:
        position = index
        pairs = []
        for key in sorted(messages):
            if key not in position:
                raise gen_text.Refused(
                    f"{tag}: {key} is not a key in the English table; a locale cannot "
                    "index a key English does not have"
                )
            pairs.append((position[key], messages[key]))
        pairs.sort()
        language = language_of(tag)
        rtl = "true" if tag in RTL else "false"
        entries = len(pairs)
        value_bytes = sum(len(text.encode("utf-8")) for _, text in pairs)
        total_entries += entries
        total_value_bytes += value_bytes
        constant = tag.replace("-", "_").upper()
        body.append(f"/// `{tag}`: {entries} of {len(english_keys)} keys, "
                    f"{value_bytes:,} bytes of translated text.")
        body.append(f"static {constant}: [(u16, &str); {entries}] = [")
        for key_index, text in pairs:
            body.append(f"    ({key_index}, {gen_text.escape(text)}),")
        body.append("];")
        body.append("")
    return body, total_entries, total_value_bytes


def emit(tag_sources, english_keys, counts_by_tag, refused):
    """The generated `locale_gen.rs`."""
    index = {key: position for position, key in enumerate(english_keys)}
    body, total_entries, total_value_bytes = build_lines(tag_sources, english_keys)

    header: list = []
    header.append("//! The reference client's other locales, compiled from its vendored locale")
    header.append("//! trees.")
    header.append("//!")
    header.append("//! Generated by `tools/gen_locale.py` -- do not edit by hand. Run")
    header.append("//! `python tools/gen_locale.py` to regenerate, and `--check` to see whether this")
    header.append("//! file is what the tool emits. CI runs the check, so a vendored locale and this")
    header.append("//! table cannot drift apart.")
    header.append("//!")
    header.append("//! Each table is the reference's own sparse shape: `(index, template)` pairs,")
    header.append("//! sorted by index, where the index is a position in [`crate::text_gen::ALL`].")
    header.append("//! A key a locale does not carry is not in its table and falls back to English,")
    header.append("//! which is what the reference itself does (`fallbackLocale: 'en-US'` in")
    header.append("//! `app-frontend/src/i18n.config.ts`).")
    header.append("//!")
    header.append("//! The templates are the reference's own ICU, verbatim, including the arm")
    header.append("//! keywords `Intl.PluralRules` selects by: `zero`, `one`, `two`, `few`, `many`")
    header.append("//! and `other`, plus `=N` exact selectors. [`crate::locale`] fills one in.")
    header.append("//!")
    header.append("//! Every locale's keys are a subset of English's, which is why a table stores a")
    header.append("//! position rather than a key string: the key is already in the binary once, in")
    header.append("//! [`crate::text_gen::NAMES`]. The whole set is "
                  f"{total_entries:,} entries, {total_value_bytes:,} bytes of")
    header.append("//! translated text.")
    header.append("//!")
    header.append("//! `ar-SA` is here and is **not** offered: the reference's own `LOCALES`")
    header.append("//! (`ui/src/composables/i18n.ts`) lists 32 codes with this one commented out as")
    header.append("//! RTL. It is compiled because the data exists and a measurement is worth more")
    header.append("//! than an omission; [`crate::locale::OFFERED`] is the list the language row")
    header.append("//! draws, and it is the reference's.")
    header.append("")
    header.append("// A table is data: the entries are read by the setting that is in force, so")
    header.append("// most of them are unreachable from any one build of the shell. Refusing to")
    header.append("// compile them would make the tables depend on what the interface happens to")
    header.append("// draw, which is the drift this generator exists to prevent.")
    header.append("#![allow(dead_code)]")
    header.append("")
    header.append("/// One locale's own strings.")
    header.append("///")
    header.append("/// `entries` is sorted by its first element, so a lookup is a binary search and")
    header.append("/// two locales' tables can be compared by walking them together.")
    header.append("pub struct Locale {")
    header.append("    /// The BCP-47 tag the reference's `LOCALES` spells it with.")
    header.append("    pub tag: &'static str,")
    header.append("    /// The primary language subtag, which selects the plural rule.")
    header.append("    pub language: &'static str,")
    header.append("    /// Whether the reference declares this locale `dir: 'rtl'`.")
    header.append("    pub rtl: bool,")
    header.append("    /// The position in [`crate::text_gen::ALL`] of the reference's own name")
    header.append("    /// for this language (`locale.<tag>`).")
    header.append("    ///")
    header.append("    /// Resolved here rather than at runtime because the lookup is a scan of")
    header.append("    /// 3,846 keys and the label is drawn for every offered language at once.")
    header.append("    /// It resolves for every one of the 32 offered codes today -- the English")
    header.append("    /// locale carries all 32 `locale.*` names -- so `None` is a guard for a")
    header.append("    /// tree that grows a code upstream has not named yet, and a name that is")
    header.append("    /// missing is answered with the tag rather than with one this launcher")
    header.append("    /// invents.")
    header.append("    pub label: Option<u16>,")
    header.append("    /// `(position in text_gen::ALL, this locale's template)`, sorted by position.")
    header.append("    pub entries: &'static [(u16, &'static str)],")
    header.append("}")
    header.append("")
    table_rows = []
    for tag, _messages in tag_sources:
        constant = tag.replace("-", "_").upper()
        language = language_of(tag)
        rtl = "true" if tag in RTL else "false"
        # The reference's own name for the language, which every one of the 32
        # offered codes has -- `locale.<tag>` is a key in the English locale, so
        # this is a position rather than a string. A tag with no name would be
        # `None`, which is a guard rather than a case today.
        named = index.get(f"locale.{tag}")
        label = f"Some({named})" if named is not None else "None"
        table_rows.append(
            f'    Locale {{ tag: "{tag}", language: "{language}", rtl: {rtl}, '
            f"label: {label}, entries: &{constant} }},"
        )
    header.append("/// Every locale tree on disk, by tag.")
    header.append("///")
    header.append("/// The order is the tag order, which is a stable order rather than the")
    header.append("/// reference's: `LOCALES`' own order is [`crate::locale::OFFERED`], and the two")
    header.append("/// are different lists for a reason -- this one is data, that one is the offer.")
    header.append(f"pub static ALL: [Locale; {len(table_rows)}] = [")
    header.extend(table_rows)
    header.append("];")
    header.append("")
    header.append("/// The locale a tag names, if this build has one.")
    header.append("///")
    header.append("/// A scan, not a binary search: the list is 33 long and is walked once when the")
    header.append("/// setting is applied, not per lookup.")
    header.append("pub fn find(tag: &str) -> Option<&'static Locale> {")
    header.append("    ALL.iter().find(|locale| locale.tag == tag)")
    header.append("}")
    header.append("")
    header.append("#[cfg(test)]")
    header.append("mod tests {")
    header.append("    use super::*;")
    header.append("")
    header.append("    #[test]")
    header.append("    fn every_table_is_sorted_so_a_lookup_can_be_a_binary_search() {")
    header.append("        for locale in &ALL {")
    header.append("            for pair in locale.entries.windows(2) {")
    header.append("                assert!(pair[0].0 < pair[1].0, \"{} is out of order\", locale.tag);")
    header.append("            }")
    header.append("            assert!(!locale.entries.is_empty(), \"{}\", locale.tag);")
    header.append("        }")
    header.append("    }")
    header.append("")
    header.append("    #[test]")
    header.append("    fn no_index_is_past_the_english_table() {")
    header.append("        for locale in &ALL {")
    header.append("            for (position, _) in locale.entries {")
    header.append("                assert!(")
    header.append("                    (*position as usize) < crate::text_gen::ALL.len(),")
    header.append("                    \"{} names position {position}\",")
    header.append("                    locale.tag,")
    header.append("                );")
    header.append("            }")
    header.append("        }")
    header.append("    }")
    header.append("")
    header.append("    #[test]")
    header.append("    fn the_tags_are_unique_and_find_agrees_with_the_list() {")
    header.append("        let mut tags: Vec<&str> = ALL.iter().map(|locale| locale.tag).collect();")
    header.append("        let count = tags.len();")
    header.append("        tags.sort_unstable();")
    header.append("        tags.dedup();")
    header.append("        assert_eq!(count, tags.len(), \"two locales share a tag\");")
    header.append("        for locale in &ALL {")
    header.append("            assert_eq!(find(locale.tag).map(|found| found.tag), Some(locale.tag));")
    header.append("        }")
    header.append("        assert!(find(\"xx-XX\").is_none());")
    header.append("    }")
    header.append("}")
    header.append("")

    # The tests go after the tables, not with the types that describe them:
    # `clippy::items_after_test_module` is right that an item below a `#[cfg(test)]`
    # module is easy to miss, and 89,000 entries are exactly the kind of thing that
    # would hide below one.
    split = header.index("#[cfg(test)]")
    return "\n".join(header[:split] + body + header[split:])


def report(tag_sources, english_keys, counts_by_tag, refused) -> str:
    out: list = []
    english = set(english_keys)
    names = len(english_keys)
    pairs = len(tag_sources) * names
    out.append(f"{'tag':8} {'keys':>6} {'cover':>6} {'fallback':>8} {'plural':>6} "
               f"{'select':>6} {'number':>6} {'#':>4} arms")
    total_leaves = total_bytes = total_fallback = 0
    # The extremes are over the trees that are *translations*: English is the
    # source, so counting it would report 100% as the fullest tree every run.
    sparsest: tuple = ("", names)
    fullest: tuple = ("", 0)
    # The keys every translation carries: the rest of English is reached by a
    # fallback in at least one language, and the complement of this set is the
    # English a translation never replaces anywhere.
    everywhere = None
    for tag, messages in tag_sources:
        counts = counts_by_tag[tag]
        # One leaf per key: an arm is a branch of the same template, not a table
        # entry of its own, so `len(messages)` *is* the key count. The column that
        # used to sit beside it printed this number a second time, which is what
        # "keys" is for and what the coverage share needs.
        leaves = len(messages)
        arms = arms_used(messages)
        keys = set(messages)
        fallback = len(english - keys)
        value_bytes = sum(len(text.encode("utf-8")) for text in messages.values())
        total_leaves += leaves
        total_bytes += value_bytes
        total_fallback += fallback
        if tag != "en-US":
            if leaves < sparsest[1]:
                sparsest = (tag, leaves)
            if leaves > fullest[1]:
                fullest = (tag, leaves)
            everywhere = keys if everywhere is None else (everywhere & keys)
        plural = sum(1 for text in messages.values() if ", plural," in text)
        select = sum(1 for text in messages.values() if ", select," in text)
        number = sum(1 for text in messages.values() if ", number" in text)
        hashes = sum(
            1 for text in messages.values() if ", plural," in text and "#" in text
        )
        # `=0(2)` rather than `=0=2`: an explicit arm's key already carries a `=`.
        shown = ", ".join(f"{key}({count})" for key, count in sorted(arms.items()))
        out.append(f"{tag:8} {leaves:>6} {100 * leaves / names:>5.1f}% {fallback:>8} "
                   f"{counts.nodes['plural']:>6} {counts.nodes['select']:>6} "
                   f"{counts.nodes['number']:>6} {counts.hashes:>4} {shown}")
    out.append("")
    out.append(f"trees            {len(tag_sources)}: the reference's own 32 offered codes, "
               "plus ar-SA, which its list comments out")
    out.append(f"keys             {names:,} English names; the trees carry {total_leaves:,} "
               f"of a possible {pairs:,} ({100 * total_leaves / pairs:.1f}% translated)")
    out.append(f"coverage         over the translations: sparsest {sparsest[0]} at "
               f"{sparsest[1]:,} ({100 * sparsest[1] / names:.1f}%), fullest "
               f"{fullest[0]} at {fullest[1]:,} ({100 * fullest[1] / names:.1f}%)")
    out.append(f"fallback pairs   {total_fallback:,} of {pairs:,} locale-key pairs "
               f"({100 * total_fallback / pairs:.1f}%) read English's sentence")
    out.append(f"names in all 32 {len(everywhere or ()):,} of the {names:,}; the other "
               f"{names - len(everywhere or ()):,} are missing from at least one translation")
    out.append(f"value bytes      {total_bytes:,} of translated text")
    out.append(f"index bytes      {total_leaves * 2:,} (a u16 per entry)")
    if refused:
        out.append("")
        # The category *set* rather than the integer rule: this catches an arm a
        # language has no rule for at all, and it does not catch the two arms that
        # belong to a language's *fractional* rule -- Czech `many` and Polish
        # `other` -- because those categories are legitimately in the set. The
        # integer-level version of this measurement is a test in `crate::locale`,
        # which is where the rule lives, and the two belong together.
        out.append(
            "arms outside the language's CLDR category set (unselectable for every count):"
        )
        for tag, dead in refused:
            out.append(f"  {tag:8} {', '.join(dead)}")
    return "\n".join(out) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="fail if the output is stale")
    parser.add_argument("--report", action="store_true", help="print what each locale carries")
    parser.add_argument("--only", default="", metavar="TAG,TAG",
                        help="compile only these locales (for the size measurement)")
    arguments = parser.parse_args()

    english = gen_text.load_all()
    english_keys = sorted(english)
    wanted = [tag for tag in arguments.only.split(",") if tag]
    available = tags()
    sources: list = []
    counts_by_tag: dict = {}
    refused: list = []
    try:
        for tag in available:
            if wanted and tag not in wanted:
                continue
            messages = load_locale(tag)
            counts = gen_text.Counts()
            validate(tag, messages, counts)
            counts_by_tag[tag] = counts
            sources.append((tag, messages))
            language = language_of(tag)
            legal = CLDR_CATEGORIES.get(language)
            if legal is None:
                raise gen_text.Refused(f"{tag}: no CLDR rule for the language {language!r}")
            dead = sorted(
                key for key in arms_used(messages)
                if not key.startswith("=") and key not in legal
            )
            if dead:
                refused.append((tag, dead))
    except gen_text.Refused as refusal:
        print(f"gen_locale: refused {refusal}", file=sys.stderr)
        return 1

    missing = [tag for tag in wanted if tag not in {t for t, _ in sources}]
    if missing:
        print(f"gen_locale: no such locale: {', '.join(missing)}", file=sys.stderr)
        return 1

    generated = emit(sources, english_keys, counts_by_tag, refused)

    if arguments.report:
        sys.stdout.write(report(sources, english_keys, counts_by_tag, refused))

    path = ROOT / OUTPUT
    if arguments.check:
        current = path.read_text(encoding="utf-8") if path.exists() else ""
        if current != generated:
            print(
                f"gen_locale: {OUTPUT} is not what this tool emits; "
                "run `python tools/gen_locale.py`",
                file=sys.stderr,
            )
            return 1
        print("locale generation is byte-identical")
        return 0

    path.write_text(generated, encoding="utf-8", newline="\n")
    print(f"gen_locale: wrote {OUTPUT} ({len(generated.splitlines())} lines)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
