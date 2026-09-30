//! The generated vocabulary, checked against the sheets it was generated from.
//!
//! `tools/gen_tokens.py` writes [`crate::theme_tokens`]: every token the
//! reference declares, per mode, after the reference's own cascade, with
//! `var()` chains followed. What makes that a copy rather than a transcription
//! is this module: the sheets are read a *second* time, here, with a parser
//! that shares nothing with the generator, and the two readings are compared
//! key by key. When the generated file has drifted — an upstream pull the
//! generator was not re-run after, an edit by hand — the failure names the
//! file and line the reference states the token on, which is also the fix:
//! re-run the generator and commit what it writes.
//!
//! This reader is deliberately small. It must parse exactly what
//! `theme_tokens.rs` claims and nothing more: the moment it grows features the
//! generator lacks (or lacks what the generator has), "the two agree" stops
//! meaning anything. When the sheets change shape, both readers have to move,
//! and a change that moves only one of them fails here. That cost is the
//! point — it is what makes the generator safe to trust.
//!
//! The same standard is applied to the *scoped* half of the copy — the tokens a
//! component, a page or `classes.scss` sets for one selector, which have no mode
//! to resolve in. Those are re-derived here too, from the files themselves, and
//! compared against the generated table.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::reference_tokens::vendored_tree;
use crate::theme_tokens::{self, Kind};

/// One reading of the sheets: every mode's every token, as
/// `(mode, token) -> file|line|kind|value`.
///
/// Keyed rather than ordered because the comparison has to be indifferent to
/// everything except content — the first version zipped two sorted lists and
/// any count difference shifted every pairing into a wall of wrong rows that
/// named the right tokens at the wrong values.
type Vocabulary = BTreeMap<(String, String), String>;

fn sheets_vocabulary(tree: &Path) -> Option<Vocabulary> {
    let variables_text = std::fs::read_to_string(tree.join("assets/styles/variables.scss")).ok()?;
    let defaults_text = std::fs::read_to_string(tree.join("assets/styles/defaults.scss")).ok()?;

    // A declaration: name, raw value, line, and the sheet it is on.
    type Decl = (String, String, usize, &'static str);
    // A block: its selector, its `@extend` parent if it states one, and
    // the declarations it carries.
    type Block = (String, Option<String>, Vec<Decl>);

    let mut blocks: Vec<Block> = Vec::new();

    let mut walk = |text: &str, file: &'static str| {
        let mut current: Option<Block> = None;
        let mut pending: Option<(String, String, usize)> = None;
        // A selector can run over several lines — `.dark-mode,` / `.dark,`
        // / `:root[data-theme='dark'] {` — so lines accumulate until the
        // `{`. Joining with a space is what makes the accumulated form
        // equal the single-line form the lookups below use.
        let mut selector = String::new();
        let mut depth = 0usize;
        for (index, raw) in text.lines().enumerate() {
            let line_no = index + 1;
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            if let Some((name, value, at)) = pending.take() {
                let joined = format!("{value} {trimmed}");
                if joined.contains(';') {
                    if let Some(block) = current.as_mut() {
                        block.2.push((name, joined, at, file));
                    }
                } else {
                    pending = Some((name, joined, at));
                }
                continue;
            }
            if trimmed.ends_with('{') {
                selector.push_str(trimmed.trim_end_matches('{').trim());
                let name = selector.trim().to_string();
                selector.clear();
                if depth == 0 && !name.starts_with('@') {
                    // A top-level block starts here. `@media` and friends
                    // only raise the depth: their inner blocks are not the
                    // cascade's, and their declarations are not tokens.
                    current = Some((name, None, Vec::new()));
                }
                depth += 1;
                continue;
            }
            if trimmed == "}" {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    if let Some(block) = current.take() {
                        blocks.push(block);
                    }
                }
                continue;
            }
            if depth == 0 {
                // Part of a selector that has not met its `{` yet.
                selector.push_str(trimmed);
                selector.push(' ');
                continue;
            }
            if depth == 1 {
                if let Some(rest) = trimmed.strip_prefix("@extend ") {
                    if let Some(block) = current.as_mut() {
                        block.1 = Some(rest.trim_end_matches(';').trim().to_string());
                    }
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("--") {
                    if let Some((name, value)) = rest.split_once(':') {
                        let name = name.trim().to_string();
                        let value = value.trim().to_string();
                        if value.contains(';') {
                            if let Some(block) = current.as_mut() {
                                block.2.push((name, value, line_no, file));
                            }
                        } else {
                            pending = Some((name, value, line_no));
                        }
                    }
                }
            }
        }
    };
    walk(&variables_text, "variables.scss");
    walk(&defaults_text, "defaults.scss");

    // A block is found by any one of its selector's comma-separated parts,
    // which is how `.dark-mode, .dark, :root[data-theme='dark']` is found
    // by its first part alone.
    let find = |name: &str| -> Option<&Block> {
        blocks.iter().find(|(selector, _, _)| {
            selector.split(',').any(|part| part.trim() == name)
        })
    };
    let light = find(".light-properties")?;
    let html = find("html")?;
    let dark = find(".dark-mode")?;
    let oled = find(".oled-mode")?;
    let retro = find(".retro-mode")?;
    let body = find("body")?;

    // The cascade's shape, asserted rather than assumed — the same three
    // facts the generator fails on, because if upstream re-parents a block
    // both readers have to be told, not just one.
    assert_eq!(html.1.as_deref(), Some(".light-properties"), "`html` no longer extends light");
    assert_eq!(oled.1.as_deref(), Some(".dark-mode"), "`.oled-mode` no longer extends dark");
    assert_eq!(retro.1.as_deref(), Some(".dark-mode"), "`.retro-mode` no longer extends dark");

    // The cascade: base, then dark, then the two modes that extend dark.
    // Later blocks win, which is what `@extend` means.
    let table = |parts: Vec<&Block>| -> BTreeMap<String, (String, usize, &'static str)> {
        let mut out = BTreeMap::new();
        for (_, _, decls) in parts {
            for (name, value, line, file) in decls {
                out.insert(name.clone(), (value.clone(), *line, *file));
            }
        }
        out
    };
    let base = table(vec![light, html, body]);
    let mut dark_all = base.clone();
    for (name, entry) in table(vec![dark]) {
        dark_all.insert(name, entry);
    }
    let mut oled_all = dark_all.clone();
    for (name, entry) in table(vec![oled]) {
        oled_all.insert(name, entry);
    }
    let mut retro_all = dark_all.clone();
    for (name, entry) in table(vec![retro]) {
        retro_all.insert(name, entry);
    }

    let mut vocabulary: Vocabulary = BTreeMap::new();
    for (mode, merged) in [
        ("LIGHT", &base),
        ("DARK", &dark_all),
        ("OLED", &oled_all),
        ("RETRO", &retro_all),
    ] {
        for (name, (raw, _, _)) in merged {
            let key = (mode.to_string(), name.clone());
            let value = render_row(mode, name, raw, merged);
            vocabulary.insert(key, value);
        }
    }
    Some(vocabulary)
}

/// The declaration's value, without its `;`, its comment, or `!important`.
fn cut(raw: &str) -> String {
    let text = raw.split(';').next().unwrap_or(raw).trim();
    let text = text.split("//").next().unwrap_or(text).trim();
    text.strip_suffix("!important").unwrap_or(text).trim().to_string()
}

/// Render one row of the sheets' reading: `file|line|kind|value`.
///
/// The value is cleaned *before* the chains are followed — `;`, a `//`
/// comment, and `!important` — because the reference writes
/// `--color-link: var(--color-blue) !important;`, and a follower that saw the
/// `!important` would stop one hop early and report text where the generator
/// reports a colour. The line reported is the referent's: the value is written
/// where the chain ends, and that is where a reader is sent to check it, which
/// is the same rule the generator applies.
fn render_row(
    _mode: &str,
    token: &str,
    raw: &str,
    merged: &BTreeMap<String, (String, usize, &'static str)>,
) -> String {
    // `var(--x)` or `var(--x, fallback)`, and nothing else — the shape the
    // generator accepts (`^var\(\s*(--[\w-]+)\s*(?:,.*)?\)$`). The name's own
    // character check is what rejects a composite (`var(--x) var(--y)`), which
    // is the shape that would otherwise render as a missing token.
    let bare_var = |text: &str| -> Option<String> {
        let inner = text.strip_prefix("var(")?.strip_suffix(')')?;
        let name = inner.split(',').next()?.trim();
        let plain = name.strip_prefix("--").is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        });
        plain.then(|| name.to_string())
    };

    let mut current = token.to_string();
    let mut value = cut(raw);
    for _ in 0..8 {
        match bare_var(&value) {
            Some(next) => {
                // The walk keys tokens without the `--` it strips, so a lookup
                // with one on would miss every hop of every chain and report
                // the token it never reached instead of the value it ends on.
                let name = next.trim_start_matches("--").to_string();
                match merged.get(&name) {
                    Some((raw, _, _)) => {
                        value = cut(raw);
                        current = name;
                    }
                    // A chain off the edge of the sheets: render it so the
                    // comparison fails loudly rather than quietly matching
                    // nothing. The generator cannot emit this shape, so this
                    // row can only ever be a failure.
                    None => return format!("|0|Text|__missing:{next}"),
                }
            }
            None => break,
        }
    }

    let (file, line) = merged
        .get(&current)
        .map(|(_, line, file)| ((*file).to_string(), *line as u32))
        .unwrap_or((String::new(), 0));
    let (kind, encoded) = classify(&value);
    format!("{file}|{line}|{kind}|{encoded}")
}

/// The generator's classifier, again, from a resolved value.
///
/// The order matters as much as the arms: gradient, then colour, then lengths,
/// then numbers, then text. A value on a boundary (`2rem` is a length and also
/// parses as a number) classifies the same way here as it did there only if
/// the arms are tried in the same order.
fn classify(value: &str) -> (String, String) {
    if value.starts_with("linear-gradient(") || value.starts_with("radial-gradient(") {
        // A stop neither reader can encode — `--loading-bar-gradient` has
        // `var(--color-brand) 0%` — makes the whole value text, which is what
        // the generator does with it: a claim about a gradient whose stops are
        // not readable stays a claim that fails loudly.
        return match encode_gradient(value) {
            Some(stops) => ("Gradient".to_string(), stops),
            None => ("Text".to_string(), value.to_string()),
        };
    }
    if let Some(colour) = encode_colour(value) {
        return ("Color".to_string(), colour);
    }
    for (suffix, rem) in [("rem", true), ("px", false)] {
        if let Some(number) = value.strip_suffix(suffix) {
            if let Ok(parsed) = number.trim().parse::<f64>() {
                let px = if rem { parsed * 16.0 } else { parsed };
                return ("Length".to_string(), format_number(px));
            }
        }
    }
    if let Ok(parsed) = value.parse::<f64>() {
        return ("Number".to_string(), format_number(parsed));
    }
    ("Text".to_string(), value.to_string())
}

/// Numbers, the way both readers write them: integral values without a
/// fractional part, otherwise up to four decimals with trailing zeros cut.
fn format_number(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    let mut text = format!("{value:.4}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, or a CSS colour keyword.
fn encode_colour(text: &str) -> Option<String> {
    let channels = parse_colour(text.trim())?;
    let (red, green, blue, alpha) = channels;
    // The alpha decides the width, in one place: `#rrggbbaa` ending `ff` comes
    // back out six digits, because it *is* opaque — the same rule the
    // generator's `encode_color` applies, and the reason its output and this
    // one are comparable at all.
    if alpha >= 0.999 {
        return Some(format!("#{red:02x}{green:02x}{blue:02x}"));
    }
    // Half up, added and truncated rather than rounded: Python's `round` is
    // half-to-even, and on `rgba(27, 217, 106, 0.7)` -- 178.5 -- the two
    // differed by one byte and every brand-shadow row failed. `+ 0.5` then
    // truncate is the one rule both languages state identically.
    Some(format!(
        "#{red:02x}{green:02x}{blue:02x}{:02x}",
        (alpha * 255.0 + 0.5) as u8
    ))
}

/// A colour as channels and an alpha, before either reader decides how to
/// write it: `(red, green, blue, alpha)`.
fn parse_colour(text: &str) -> Option<(u8, u8, u8, f64)> {
    if let Some(digits) = text.strip_prefix('#') {
        let channel = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
        return match digits.len() {
            3 => {
                let mut channels = [0u8; 3];
                for (index, slot) in channels.iter_mut().enumerate() {
                    let digit = digits.get(index..index + 1)?;
                    *slot = u8::from_str_radix(&format!("{digit}{digit}"), 16).ok()?;
                }
                Some((channels[0], channels[1], channels[2], 1.0))
            }
            6 => Some((channel(0)?, channel(2)?, channel(4)?, 1.0)),
            8 => Some((
                channel(0)?,
                channel(2)?,
                channel(4)?,
                f64::from(channel(6)?) / 255.0,
            )),
            _ => None,
        };
    }
    for (prefix, count) in [("rgba(", 4usize), ("rgb(", 3)] {
        if let Some(inner) = text.strip_prefix(prefix).and_then(|rest| rest.strip_suffix(')')) {
            let parts: Vec<&str> = inner.split(',').map(|part| part.trim()).collect();
            if parts.len() != count {
                return None;
            }
            let numbers: Vec<f64> = parts.iter().filter_map(|part| part.parse::<f64>().ok()).collect();
            if numbers.len() != count {
                return None;
            }
            let bytes: Vec<u8> = numbers[..3].iter().map(|n| (*n / 255.0 * 255.0 + 0.5) as u8).collect();
            let alpha = if count == 4 { numbers[3] } else { 1.0 };
            return Some((bytes[0], bytes[1], bytes[2], alpha));
        }
    }
    match text {
        "white" => Some((255, 255, 255, 1.0)),
        "black" => Some((0, 0, 0, 1.0)),
        "transparent" => Some((0, 0, 0, 0.0)),
        _ => None,
    }
}

/// A gradient's stops, as `#rrggbbaa@position` joined with commas — the
/// generator's encoding — or nothing when the stops cannot be read as stops
/// (`var()` inside one, a non-percent position), which makes the row text.
fn encode_gradient(text: &str) -> Option<String> {
    let open = text.find('(')?;
    let close = text.rfind(')').unwrap_or(text.len());
    let inner = &text[open + 1..close];

    let mut parts: Vec<&str> = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, character) in inner.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&inner[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&inner[start..]);

    let mut stops = Vec::new();
    for part in parts {
        let part = part.trim();
        if part.starts_with("to ") || part.starts_with("from ") || part.ends_with("deg") {
            continue;
        }
        // The position is the last space-separated piece, so a stop whose
        // colour itself contains a space (`rgba(1, 2, 3, 0.5) 40%`) survives.
        let (colour, position) = part.rsplit_once(' ')?;
        let position: f64 = position.strip_suffix('%')?.parse().ok()?;
        let colour = encode_colour(colour.trim())?;
        stops.push(format!("{colour}@{}", format_number(position)));
    }
    if stops.len() < 2 {
        return None;
    }
    Some(stops.join(","))
}

/// How both readers write a kind, so the two spellings cannot drift.
fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Color => "Color",
        Kind::Length => "Length",
        Kind::Number => "Number",
        Kind::Gradient => "Gradient",
        Kind::Text => "Text",
    }
}

/// The generated file's rows, keyed the same way the sheets' are.
fn generated_rows() -> BTreeMap<(String, String), String> {
    let mut out = BTreeMap::new();
    for (mode, table) in [
        ("LIGHT", theme_tokens::LIGHT),
        ("DARK", theme_tokens::DARK),
        ("OLED", theme_tokens::OLED),
        ("RETRO", theme_tokens::RETRO),
    ] {
        for row in table {
            // Keyed without the `--`, as the sheets' walk names them — the
            // same normalization both sides can be read by.
            let key = (mode.to_string(), row.token.trim_start_matches("--").to_string());
            out.insert(
                key,
                format!("{}|{}|{}|{}", row.file, row.line, kind_name(row.kind), row.value),
            );
        }
    }
    out
}

// ---- The scoped half of the vocabulary ---------------------------------
//
// Everything outside the two global sheets: a token a component, a page or
// `classes.scss` sets for one selector. Read by two parsers that share no code
// — this one and `tools/gen_tokens.py` — and compared row by row, keyed by the
// file and line the declaration is on.

/// A token declaration can only live in a stylesheet or an SFC style block. The
/// list matches the generator's, including the suffixes this tree does not use
/// yet: a `.sass` file appearing upstream is then *walked* and fails as a stale
/// copy, rather than being invisible to both readers and passing.
const STYLE_SUFFIXES: [&str; 6] = ["scss", "css", "sass", "less", "styl", "vue"];

/// The two sheets the mode tables are built from; the scoped walk skips them.
const GLOBAL_SHEETS: [&str; 2] = ["assets/styles/variables.scss", "assets/styles/defaults.scss"];

/// Every style file under the tree, sorted, with `node_modules` left alone.
fn style_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                if entry.file_name() != "node_modules" {
                    stack.push(path);
                }
                continue;
            }
            let suffix = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default();
            if STYLE_SUFFIXES.contains(&suffix) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// A file's path as the copy writes it: relative to the tree, forward slashes,
/// so the same rows read the same on whichever machine generated them.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The declarations one file holds, as `(selector, token, raw value, line)`.
///
/// The selector is the innermost *rule* the declaration sits in: at-rules are
/// transparent, so a token set inside `@media` is reported against the rule the
/// query wraps, and a selector that runs over several lines is joined with
/// single spaces. The generator states the same rule, and the comparison below
/// only means something while both of them do.
fn scoped_declarations(text: &str) -> Vec<(String, String, String, usize)> {
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut pending: Option<(String, String, String, usize)> = None;

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("<style") || line.starts_with("</style") {
            // A component can carry more than one style block, and the rules of
            // one are not the rules of the next.
            stack.clear();
            pending = None;
            continue;
        }
        if let Some((selector, name, value, at)) = pending.take() {
            let joined = format!("{value} {line}");
            if joined.contains(';') {
                out.push((selector, name, joined, at));
            } else {
                pending = Some((selector, name, joined, at));
            }
            continue;
        }
        if let Some(head) = line.strip_suffix('{') {
            stack.push(head.split_whitespace().collect::<Vec<_>>().join(" "));
            continue;
        }
        if line.starts_with('}') {
            stack.pop();
            continue;
        }
        // A declaration starts with `--` *and* holds a colon: a continuation of
        // the property above it can start with `--` without being one
        // (`transition: --_top-fade-height 0.05s linear,` in ScrollablePanel.vue),
        // and reading that as a declaration invents a token. The name keeps its
        // `--`, which is how the mode tables spell a token too.
        if line.starts_with("--") {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let selector = stack
                .iter()
                .rev()
                .find(|rule| !rule.starts_with('@'))
                .cloned()
                .unwrap_or_default();
            let (name, value) = (name.trim().to_string(), value.trim().to_string());
            if value.contains(';') {
                out.push((selector, name, value, index + 1));
            } else {
                pending = Some((selector, name, value, index + 1));
            }
        }
    }
    out
}

/// The scoped half of the sheets' reading, keyed `path:line` and valued
/// `selector|token|kind|value` — the rendering the generated table gets too.
fn sheets_scoped(root: &Path) -> Option<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for path in style_files(root) {
        let file = relative(root, &path);
        if GLOBAL_SHEETS.contains(&file.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        for (selector, token, raw, line) in scoped_declarations(&text) {
            let (kind, value) = classify(&cut(&raw));
            out.insert(
                format!("{file}:{line}"),
                format!("{selector}|{token}|{kind}|{value}"),
            );
        }
    }
    Some(out)
}

/// The generated scoped table, keyed the same way.
fn generated_scoped() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for row in theme_tokens::SCOPED {
        out.insert(
            format!("{}:{}", row.file, row.line),
            format!(
                "{}|{}|{}|{}",
                row.selector,
                row.token,
                kind_name(row.kind),
                row.value
            ),
        );
    }
    out
}

/// A comparison's value, said the way a reader checks it.
fn describe(detail: &str) -> String {
    let fields: Vec<&str> = detail.split('|').collect();
    match fields.as_slice() {
        [selector, token, kind, value] => {
            format!("{selector} sets {token} to `{value}` ({kind})")
        }
        _ => detail.to_string(),
    }
}

#[test]
fn the_generated_vocabulary_is_the_sheets() {
    let Some(vocabulary) = sheets_vocabulary(&vendored_tree()) else {
        return;
    };
    let generated = generated_rows();

    let missing: Vec<String> = vocabulary
        .keys()
        .filter(|key| !generated.contains_key(*key))
        .map(|(mode, token)| format!("{mode}: --{token} is in the sheets but not in the copy"))
        .collect();
    let invented: Vec<String> = generated
        .keys()
        .filter(|key| !vocabulary.contains_key(*key))
        .map(|(mode, token)| format!("{mode}: --{token} is in the copy but the sheets do not declare it"))
        .collect();

    let mut problems = missing;
    problems.extend(invented);
    for ((mode, token), theirs) in &vocabulary {
        match generated.get(&(mode.clone(), token.clone())) {
            Some(ours) if ours == theirs => {}
            Some(ours) => {
                let fields: Vec<&str> = ours.split('|').collect();
                let file = fields.first().copied().unwrap_or("?");
                let line = fields.get(1).copied().unwrap_or("?");
                problems.push(format!(
                    "--{token} [{mode}]: generated `{ours}`, sheets `{theirs}`\n    \
                     vendor/modrinth-app/assets/styles/{file}:{line}  --{token}"
                ));
            }
            None => {}
        }
    }

    assert!(
        problems.is_empty(),
        "\ntheme_tokens.rs has drifted from the sheets ({} row(s)):\n\n{}\n\n\
         fix: run `python tools/gen_tokens.py` and commit what it writes\n",
        problems.len(),
        problems.join("\n\n")
    );
}

#[test]
fn the_generated_vocabulary_covers_every_declared_token() {
    // The second half of the standard `theme_tokens.rs` is held to: it is the
    // shell's *copy* of the vocabulary, so it must be complete as well as
    // correct — every `--token` either sheet declares, in every mode's table.
    // The declared set is collected by a line scan rather than by either
    // parser, because a parser is exactly the thing whose blind spot this test
    // exists to catch: whatever shape a block takes, a declaration is a line
    // that starts with `--`.
    let declared = || -> Option<std::collections::BTreeSet<String>> {
        let mut names = std::collections::BTreeSet::new();
        for file in ["assets/styles/variables.scss", "assets/styles/defaults.scss"] {
            let text = std::fs::read_to_string(vendored_tree().join(file)).ok()?;
            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("--") {
                    if let Some((name, _)) = rest.split_once(':') {
                        names.insert(name.trim().to_string());
                    }
                }
            }
        }
        Some(names)
    }();
    let Some(declared) = declared else {
        return;
    };
    assert!(!declared.is_empty(), "the sheets declare nothing: the scan broke");

    let mut problems = Vec::new();
    for (mode, table) in [
        ("LIGHT", theme_tokens::LIGHT),
        ("DARK", theme_tokens::DARK),
        ("OLED", theme_tokens::OLED),
        ("RETRO", theme_tokens::RETRO),
    ] {
        let copied: std::collections::BTreeSet<&str> =
            table.iter().map(|row| row.token.trim_start_matches("--")).collect();
        for name in &declared {
            if !copied.contains(name.as_str()) {
                problems.push(format!("{mode}: --{name} is declared in the sheets but absent from the copy"));
            }
        }
        for row in table {
            let name = row.token.trim_start_matches("--");
            if !declared.contains(name) {
                problems.push(format!(
                    "{mode}: --{name} is in the copy but no sheet declares it\n    \
                     vendor/modrinth-app/assets/styles/{}:{}",
                    row.file, row.line
                ));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "\n{}\n\nfix: run `python tools/gen_tokens.py` and commit what it writes\n",
        problems.join("\n")
    );
}

#[test]
fn the_generated_vocabulary_holds_the_tokens_the_palette_paints() {
    // The point of the copy: every value the transcribed-token gate checks the
    // palette against must also be *in* the generated vocabulary, so the shell
    // holds the token it paints from. A palette that drifts from
    // `theme_tokens` while both agree with the sheets would mean the palette
    // stopped being transcribed from the reference, which is exactly what this
    // exists to catch.
    if crate::reference_tokens::Sheets::load().is_none() {
        return;
    }

    for (ours, palette) in [
        ("bg", crate::theme::Palette::dark().bg),
        ("surface", crate::theme::Palette::dark().surface),
        ("text", crate::theme::Palette::dark().text),
        ("danger", crate::theme::Palette::dark().danger),
        ("bg", crate::theme::Palette::light().bg),
        ("surface", crate::theme::Palette::light().surface),
        ("text", crate::theme::Palette::light().text),
        // `accent` is deliberately absent: it is a declared deviation (the
        // measured #00da75 against the sheet's green-500), so its value must
        // NOT be in the vocabulary — its being there would mean the deviation
        // had been silently transcribed.
        ("on_accent", crate::theme::Palette::dark().on_accent),
        ("border", crate::theme::Palette::dark().border),
    ] {
        let red = (palette.r * 255.0).round() as u8;
        let green = (palette.g * 255.0).round() as u8;
        let blue = (palette.b * 255.0).round() as u8;
        let hex = format!("#{red:02x}{green:02x}{blue:02x}");
        // `starts_with` rather than equality because a value the reference
        // writes with an alpha (`--color-button-border` is
        // `rgba(161, 161, 161, 0.35)`) is encoded `#rrggbbaa`, and the rgb
        // prefix is still the token's own.
        assert!(
            theme_tokens::DARK.iter().any(|row| row.value.starts_with(&hex))
                || theme_tokens::LIGHT.iter().any(|row| row.value.starts_with(&hex)),
            "palette.{ours} paints {hex} and the generated vocabulary holds no such value: \
             the palette has stopped being transcribed from the reference"
        );
    }
}

#[test]
fn the_generated_scoped_vocabulary_is_the_sheets() {
    let Some(theirs) = sheets_scoped(&vendored_tree()) else {
        return;
    };
    let ours = generated_scoped();

    // One problem per row that only one reader found. Keyed by file and line
    // rather than zipped in order: two declarations on the same token at
    // different lines are two rows, and an ordered comparison would report the
    // one that shifted instead of the one that differs.
    let mut problems: Vec<String> = Vec::new();
    for (key, detail) in &theirs {
        match ours.get(key) {
            Some(mine) if mine == detail => {}
            Some(mine) => problems.push(format!(
                "{key}\n    the sheets say {}, and the copy {}",
                describe(detail),
                describe(mine)
            )),
            None => problems.push(format!(
                "{key}\n    the sheets say {}, and the copy has no row for it",
                describe(detail)
            )),
        }
    }
    for (key, detail) in &ours {
        if !theirs.contains_key(key) {
            problems.push(format!(
                "{key}\n    the copy says {}, and no sheet declares it",
                describe(detail)
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "\nthe scoped half of theme_tokens.rs has drifted from the sheets ({} row(s) of {}):\n\n\
         {}\n\nfix: run `python tools/gen_tokens.py` and commit what it writes\n",
        problems.len(),
        theirs.len(),
        problems
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

#[test]
fn the_generated_scoped_vocabulary_covers_every_declared_token() {
    // The copy has to be complete as well as correct, and the count comes from a
    // line scan rather than from either parser: a declaration is a line that
    // starts with `--` and holds a colon. A walker that skipped a file, or lost
    // track of a block and swallowed the declarations in it, fails here instead
    // of shipping a copy with a hole in it.
    if crate::reference_tokens::Sheets::load().is_none() {
        return;
    }
    let root = vendored_tree();

    let mut per_file: BTreeMap<&str, usize> = BTreeMap::new();
    for row in theme_tokens::SCOPED {
        *per_file.entry(row.file).or_default() += 1;
    }
    assert!(
        !per_file.is_empty(),
        "the scoped table is empty: the generator or this scan broke"
    );

    let mut problems = Vec::new();
    for path in style_files(&root) {
        let file = relative(&root, &path);
        if GLOBAL_SHEETS.contains(&file.as_str()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let declared = text
            .lines()
            .filter(|line| {
                let line = line.trim();
                line.starts_with("--") && line.contains(':')
            })
            .count();
        let copied = per_file.get(file.as_str()).copied().unwrap_or(0);
        if declared != copied {
            problems.push(format!(
                "vendor/modrinth-app/{file}: the sheets declare {declared} token(s) on lines of \
                 their own, and the copy holds {copied}"
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "\n{}\n\nfix: run `python tools/gen_tokens.py` and commit what it writes\n",
        problems.join("\n")
    );
}

#[test]
fn the_chrome_is_the_scoped_copy() {
    // The shell's chrome numbers -- the top bar's height, the rail's width, the
    // right panel's -- are the reference's own scoped tokens, declared on
    // `.app-contents` in `App.vue`, and they were transcribed by hand before the
    // copy existed. A constant whose only receipt is a comment saying where it
    // came from is exactly the shape of claim this module retires: the row is
    // read and the two are compared. `SCOPED` is compiled in, so this holds
    // without the vendored tree too; when the tree moves and the generator is
    // re-run, this is the test that says the chrome moved with it. The shell
    // these rows name is the only one there is now: the old shell's own copies
    // of the three went with it.
    for (token, ours, name) in [
        ("--top-bar-height", crate::shell::BAR, "shell::BAR"),
        ("--left-bar-width", crate::shell::RAIL, "shell::RAIL"),
        ("--right-bar-width", crate::shell::PANEL, "shell::PANEL"),
    ] {
        let Some(row) = theme_tokens::SCOPED
            .iter()
            .find(|row| row.token == token && row.file == "app-frontend/src/App.vue")
        else {
            panic!("the copy no longer holds {token}: the chrome has nothing to be transcribed from");
        };
        if row.kind != Kind::Length {
            panic!("{token} is a {:?}, not a length: the copy changed shape", row.kind);
        }
        let px = row.value;
        let Ok(theirs) = px.parse::<f32>() else {
            panic!("{token} is `{}`, which is not a length in px", row.value);
        };
        assert!(
            (ours - theirs).abs() < 0.01,
            "{name} is {ours}px and the reference's {token} is {theirs}px\n    \
             vendor/modrinth-app/{}:{}  {token}",
            row.file,
            row.line
        );
    }
}
