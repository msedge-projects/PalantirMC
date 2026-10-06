//! INI codec for `instance.cfg` and `prismlauncher.cfg`.
//!
//! Mirrors `launcher/settings/INIFile.cpp` of Prism Launcher (develop):
//!
//! * **Current format** (`ConfigVersion=1.3`): Qt `QSettings::IniFormat`,
//!   ported byte-for-byte from Qt's `qsettings.cpp` (`readIniFile`,
//!   `readIniSection`, `writeIniFile`, `iniEscapedKey`, `iniEscapedString`,
//!   `iniUnescapedStringList`). Sections use the `\`-prefixed-key form Qt
//!   actually writes (`[General]` header plus keys like `\foo\bar=x`);
//!   keys are written sorted (QMap, case-insensitive `operator<`), values
//!   are quoted when they contain `;` `,` `=` or start/end with a space,
//!   and a missing `ConfigVersion` key is injected on save.
//! * **Legacy format** (no `ConfigVersion`): the hand-rolled parser of
//!   `parseOldFileFormat` — `#` comments (escapable via `\#`), first-`=`
//!   split, `\n`/`\t`/`\#` unescaping, conditional dequoting — is ported
//!   including its quirks, and keys are migrated the same way before
//!   `ConfigVersion=1.3` is added.
//!
//! Documented deviations: lines without `=` are skipped instead of failing
//! the whole file (Prism only logs a format error); `#` is not an inline
//! comment in the Qt flavor.

use crate::error::Result;
use crate::util::{atomic_write, read_text};
use std::path::Path;

/// Case-insensitive key/value map preserving first-seen key case.
///
/// Mirrors `QSettings` semantics: `get("name")` and `get("NAME")` hit the
/// same entry, the last write wins, and the display case of the first
/// occurrence is written back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IniMap {
    entries: Vec<(String, String)>,
}

impl IniMap {
    /// Create an empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Case-insensitive lookup.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// Case-insensitive containment check.
    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Set a value (case-insensitive key match; last value wins, display
    /// case of the existing entry is preserved).
    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
            slot.1 = value;
        } else {
            self.entries.push((key.to_string(), value));
        }
    }

    /// Remove a key case-insensitively. Returns true when it existed.
    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| !k.eq_ignore_ascii_case(key));
        self.entries.len() != before
    }

    /// All entries in insertion order.
    pub fn entries(&self) -> &[(String, String)] {
        &self.entries
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---- Qt IniFormat port ------------------------------------------------------

/// `QSettingsKeyLess`: case-insensitive comparison, tie-broken by the
/// original case (`QSettingsPrivate::normalizedKey` + `QString::compare`).
fn key_less(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

/// `iniEscapedKey`: `/` becomes `\`, letters/digits/`_-.` pass through,
/// everything else becomes `%XX` (or `%UXXXX` above U+00FF).
fn escape_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 2);
    for c in s.chars() {
        let ch = c as u32;
        if c == '/' {
            out.push('\\');
        } else if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            out.push(c);
        } else if ch <= 0xFF {
            out.push('%');
            out.push_str(&format!("{:02X}", ch));
        } else {
            out.push_str(&format!("%U{:04X}", ch));
        }
    }
    out
}

/// `iniUnescapedKey`: `\` becomes `/`, `%XX` / `%UXXXX` decode; invalid
/// escapes pass through as literal `%`.
fn unescape_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == '\\' {
            out.push('/');
            i += 1;
            continue;
        }
        if ch != '%' || i == bytes.len() - 1 {
            out.push(ch);
            i += 1;
            continue;
        }
        let mut num_digits = 2usize;
        let mut first_digit = i + 1;
        if bytes[first_digit] == 'U' {
            first_digit += 1;
            num_digits = 4;
        }
        if first_digit + num_digits > bytes.len() {
            out.push('%');
            i += 1;
            continue;
        }
        let hex: String = bytes[first_digit..first_digit + num_digits].iter().collect();
        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
            Some(c) => {
                out.push(c);
                i = first_digit + num_digits;
            }
            None => {
                out.push('%');
                i += 1;
            }
        }
    }
    out
}

const fn hex_digit_val(c: u8) -> i32 {
    match c {
        b'0'..=b'9' => (c - b'0') as i32,
        b'a'..=b'f' => (c - b'a' + 10) as i32,
        b'A'..=b'F' => (c - b'A' + 10) as i32,
        _ => -1,
    }
}

/// `iniEscapedString`: escapes control characters and `\`/`"`, and quotes
/// the value when it contains `;` `,` `=` or starts/ends with a space.
fn escape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    let mut needs_quotes = false;
    for c in v.chars() {
        let ch = c as u32;
        if matches!(c, ';' | ',' | '=') {
            needs_quotes = true;
        }
        match c {
            '\0' => out.push_str("\\0"),
            '\u{7}' => out.push_str("\\a"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ if ch <= 0x1F || ch == 0x7F => out.push_str(&format!("\\x{:x}", ch)),
            c => out.push(c),
        }
    }
    if needs_quotes || v.starts_with(' ') || v.ends_with(' ') {
        out.insert(0, '"');
        out.push('"');
    }
    out
}

/// `iniUnescapedStringList` for the single-string case: handles quoted
/// segments, escape codes (`\a \b \f \n \r \t \v \" ? \' \\`, `\xHH`,
/// octal), line continuations after a backslash, and comma-separated
/// string lists (only the first element is kept, matching
/// `QSettings::value` on a list-valued key).
fn unescape_value(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(n);
    let mut i = 0usize;
    let mut in_quotes = false;
    let mut current_value_is_quoted = false;
    // Skip leading spaces (StSkipSpaces on entry).
    while i < n && (chars[i] == ' ' || chars[i] == '\t') {
        i += 1;
    }
    let mut chop_limit = out.len();
    while i < n {
        match chars[i] {
            '\\' => {
                i += 1;
                if i >= n {
                    break;
                }
                let c = chars[i];
                i += 1;
                match c {
                    'a' => out.push('\u{7}'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'v' => out.push('\u{b}'),
                    '"' => out.push('"'),
                    '?' => out.push('?'),
                    '\'' => out.push('\''),
                    '\\' => out.push('\\'),
                    'x' => {
                        if i < n && hex_digit_val(chars[i] as u8) >= 0 {
                            let mut val: u32 = 0;
                            let mut digits = 0;
                            while i < n && digits < 4 {
                                let d = hex_digit_val(chars[i] as u8);
                                if d < 0 {
                                    break;
                                }
                                val = val * 16 + d as u32;
                                i += 1;
                                digits += 1;
                            }
                            if let Some(c) = char::from_u32(val) {
                                out.push(c);
                            }
                        }
                    }
                    c if ('0'..='7').contains(&c) => {
                        let mut val = c as u32 - '0' as u32;
                        let mut digits = 1;
                        while i < n && digits < 3 {
                            let d = chars[i];
                            if !('0'..='7').contains(&d) {
                                break;
                            }
                            val = val * 8 + (d as u32 - '0' as u32);
                            i += 1;
                            digits += 1;
                        }
                        if let Some(c) = char::from_u32(val) {
                            out.push(c);
                        }
                    }
                    '\n' | '\r'
                        // line continuation: skip a paired \r\n / \n\r
                        if i < n && (chars[i] == '\n' || chars[i] == '\r') && chars[i] != c => {
                            i += 1;
                        }
                    _ => {} // skipped, like Qt
                }
                chop_limit = out.len();
            }
            '"' => {
                i += 1;
                current_value_is_quoted = true;
                in_quotes = !in_quotes;
                if !in_quotes {
                    // StSkipSpaces after a closing quote
                    while i < n && (chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                }
            }
            ',' if !in_quotes => {
                // string list: QSettings::value() reads only the first item.
                // Qt chops trailing spaces when the value was not quoted.
                if !current_value_is_quoted {
                    chop_trailing_spaces(&mut out, chop_limit);
                }
                break;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    if !current_value_is_quoted {
        chop_trailing_spaces(&mut out, chop_limit);
    }
    out
}

/// Trim trailing spaces/tabs within `limit` (`iniChopTrailingSpaces`).
fn chop_trailing_spaces(s: &mut String, limit: usize) {
    while s.len() > limit && (s.ends_with(' ') || s.ends_with('\t')) {
        s.pop();
    }
}

/// A parsed INI line: none for blank/comment lines, some for entries.
struct RawLine {
    /// Line content without the terminator (quotes intact).
    text: String,
    /// Relative byte offset of the first unquoted/uncommented `=` in `text`, if any.
    equals: Option<usize>,
    /// Bytes consumed in `data` up to the end of `text` (terminator excluded).
    consumed: usize,
}

/// `readIniLine`: extracts one logical line, honoring quoted strings,
/// backslash escapes (including escaped line terminators) and `;`
/// comments. Returns `None` at end of input.
fn read_ini_line(data: &str) -> Option<RawLine> {
    let bytes = data.as_bytes();
    let n = bytes.len();
    let mut line_start = 0usize;
    // skip leading whitespace
    while line_start < n
        && (bytes[line_start] == b' '
            || bytes[line_start] == b'\t'
            || bytes[line_start] == b'\n'
            || bytes[line_start] == b'\r')
    {
        line_start += 1;
    }
    if line_start >= n {
        return None;
    }
    let mut i = line_start;
    let mut in_quotes = false;
    let mut equals: Option<usize> = None;
    loop {
        if i >= n {
            break;
        }
        let ch = bytes[i];
        if ch != b'=' && ch != b'\n' && ch != b'\r' && ch != b'\\' && ch != b'"' && ch != b';' {
            i += 1;
            continue;
        }
        i += 1;
        match ch {
            b'=' => {
                if !in_quotes && equals.is_none() {
                    equals = Some(i - 1);
                }
            }
            b'\n' | b'\r' => {
                if i == line_start + 1 {
                    // blank line: advance past it (Qt: ++lineStart)
                    line_start += 1;
                    // The new start may still be whitespace (e.g. "\r\n");
                    // skip it so `text` never includes skipped blanks.
                    while line_start < n
                        && (bytes[line_start] == b' '
                            || bytes[line_start] == b'\t'
                            || bytes[line_start] == b'\n'
                            || bytes[line_start] == b'\r')
                    {
                        // Keep `i` in sync when we skip ahead.
                        line_start += 1;
                    }
                    if line_start >= n {
                        return None;
                    }
                    i = line_start;
                    in_quotes = false;
                    equals = None;
                    continue;
                }
                if !in_quotes {
                    i -= 1;
                    break;
                }
            }
            b'\\' => {
                if i < n {
                    let c1 = bytes[i];
                    i += 1;
                    if i < n {
                        let c2 = bytes[i];
                        if (c1 == b'\n' && c2 == b'\r') || (c1 == b'\r' && c2 == b'\n') {
                            i += 1;
                        }
                    }
                }
            }
            b'"' => in_quotes = !in_quotes,
            b';' => {
                if i == line_start + 1 {
                    while i < n && bytes[i] != b'\n' && bytes[i] != b'\r' {
                        i += 1;
                    }
                    while i < n
                        && (bytes[i] == b' '
                            || bytes[i] == b'\t'
                            || bytes[i] == b'\n'
                            || bytes[i] == b'\r')
                    {
                        i += 1;
                    }
                    if i >= n {
                        return None;
                    }
                    line_start = i;
                    in_quotes = false;
                    equals = None;
                    continue;
                }
                if !in_quotes {
                    i -= 1;
                    break;
                }
            }
            _ => unreachable!(),
        }
    }
    let end = i.min(n);
    if end <= line_start {
        return None;
    }
    let text = data.get(line_start..end).unwrap_or("").to_string();
    let consumed = end;
    // Convert absolute `=` offset to a relative byte offset into `text`.
    let equals = equals.filter(|&e| e < end).map(|e| e - line_start);
    Some(RawLine {
        text,
        equals,
        consumed,
    })
}

/// Parse INI text in the current (QSettings) flavor.
///
/// `path` is only used for error context.
pub fn parse_qt(text: &str, path: &Path) -> Result<IniMap> {
    let _ = path;
    let mut map = IniMap::new();
    let mut section = String::new();
    let mut rest = text;
    while let Some(line) = read_ini_line(rest) {
        // Advance by byte offset (Qt `dataPos`), not `find()`: `find()` rewinds
        // when `line.text` reappears earlier in `rest` (duplicate values).
        rest = rest.get(line.consumed..).unwrap_or("");
        let trimmed = line.text.trim();
        if trimmed.starts_with('[') {
            // section header; `]` missing is tolerated (Qt sets the error
            // flag but keeps the prefix)
            let body = match trimmed.find(']') {
                Some(end) => trimmed.get(1..end).unwrap_or("").trim(),
                None => trimmed.get(1..).unwrap_or("").trim(),
            };
            if body.eq_ignore_ascii_case("general") {
                section.clear();
            } else if body.eq_ignore_ascii_case("%general") {
                // Qt: "[%General]" is the escaped real section "General/",
                // not the root. Preserve the case after '%' without unescaping
                // (matches `readIniFile`: `currentSection = iniSection+1`).
                let mut s = body.get(1..).unwrap_or("").to_string();
                s.push('/');
                section = s;
            } else {
                let body = body.strip_prefix('%').unwrap_or(body);
                section = unescape_key(body);
                section.push('/');
            }
            continue;
        }
        let Some(eq) = line.equals else {
            continue; // skipped; Prism logs a format error instead
        };
        let raw_key = line.text.get(..eq).unwrap_or("").trim();
        let key = unescape_key(raw_key);
        // Qt `\-prefixed-key` form: sectioned keys are written with a leading
        // `\` (e.g. `[UI]` + `\Foo\Bar`), which unescapes to a leading `/`.
        // `QSettingsKey::normalizedKey` strips leading/trailing/duplicate `/`,
        // so strip leading `/` here to emulate normalization.
        let key = key.trim_start_matches('/');
        let full_key = format!("{section}{key}");
        let value_raw = line.text.get(eq + 1..).unwrap_or("").trim_start();
        let value = unescape_value(value_raw);
        map.set(&full_key, value);
    }
    Ok(map)
}

/// Parse INI text in the legacy (pre-`ConfigVersion`) flavor.
///
/// Exact port of `parseOldFileFormat` in `INIFile.cpp`, quirks included.
pub fn parse_legacy(text: &str, path: &Path) -> Result<IniMap> {
    let mut map = IniMap::new();
    for line in text.lines() {
        let bytes = line.as_bytes();
        // Qt searches for '#' starting at index 1; a '#' at column 0 alone
        // never triggers truncation (see comment in parseOldFileFormat).
        let mut search = 1usize;
        let mut cut: Option<usize> = None;
        while let Some(pos) = bytes[search.min(bytes.len())..]
            .iter()
            .position(|&b| b == b'#')
            .map(|p| p + search.min(bytes.len()))
        {
            if pos > 0 && bytes[pos - 1] == b'\\' {
                search = pos + 1;
                continue;
            }
            cut = Some(line.find('#').unwrap_or(pos));
            break;
        }
        let mut line: &str = match cut {
            Some(at) => &line[..at],
            None => line,
        };
        line = line.trim();
        let Some(eq) = line.find('=') else { continue };
        let key = line[..eq].trim().to_string();
        let value = line[eq + 1..].trim();
        let value = unquote(legacy_unescape(value));
        if !key.is_empty() {
            map.set(&key, value);
        }
    }
    let _ = path;
    Ok(map)
}

/// Load an INI file, auto-detecting the format exactly like
/// `INIFile::loadFile`: QSettings parse first; when `ConfigVersion` is
/// absent or `1.1`/`1.2`, apply the migration steps and normalize to `1.3`.
pub fn load_ini(text: &str, path: &Path) -> Result<IniMap> {
    let qt = parse_qt(text, path)?;
    match qt.get("ConfigVersion") {
        Some("1.1") => {
            let mut map = parse_qt(text, path)?;
            migrate_keys_1_1(&mut map);
            map.set("ConfigVersion", "1.3");
            Ok(map)
        }
        Some("1.2") => {
            let mut map = parse_qt(text, path)?;
            migrate_keys(&mut map);
            map.set("ConfigVersion", "1.3");
            Ok(map)
        }
        Some(_) => Ok(qt),
        None => {
            let mut map = parse_legacy(text, path)?;
            migrate_keys(&mut map);
            map.set("ConfigVersion", "1.3");
            Ok(map)
        }
    }
}

/// Load an INI file from disk (UTF-8, lossy on invalid bytes).
pub fn load_ini_file(path: &Path) -> Result<IniMap> {
    let text = read_text(path)?;
    load_ini(&text, path)
}

/// Serialize a map in the current QSettings flavor, injecting
/// `ConfigVersion=1.3` when absent (mirrors `INIFile::saveFile` +
/// `QConfFileSettingsPrivate::writeIniFile`).
///
/// Layout: root keys first under an implicit `[General]`, then one section
/// per first path component, both groups sorted case-insensitively (tie
/// break on the original case, `QSettingsKeyLess`); keys inside a section
/// keep their full-path sort order. Blank line before every section header.
pub fn save_ini(map: &IniMap) -> String {
    let mut map = map.clone();
    if !map.contains("ConfigVersion") {
        map.set("ConfigVersion", "1.3");
    }
    let mut sorted: Vec<(String, String)> = map.entries().to_vec();
    sorted.sort_by(|a, b| key_less(&a.0, &b.0));

    // Group by the first path component, keeping the global sort order.
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (k, v) in sorted {
        let (section, key) = match k.find('/') {
            Some(at) => (k[..at].to_string(), k[at + 1..].to_string()),
            None => (String::new(), k),
        };
        match groups.last_mut() {
            Some((s, entries)) if *s == section => entries.push((key, v)),
            _ => groups.push((section, vec![(key, v)])),
        }
    }

    let mut out = String::new();
    let mut first = true;
    for (section, entries) in &groups {
        if !first {
            out.push('\n');
        }
        out.push('[');
        if section.is_empty() {
            out.push_str("General");
        } else if section.eq_ignore_ascii_case("general") {
            out.push_str("%General");
        } else {
            out.push_str(&escape_key(section));
        }
        out.push_str("]\n");
        for (k, v) in entries {
            // Qt `\-prefixed-key` form: keys inside a named section are written
            // with a leading `\` (e.g. `[UI]` + `\Foo\Bar=x` for
            // `UI/Foo/Bar`). Deeper `/` separators are escaped like any other
            // key character (`/` -> `\`).
            if !section.is_empty() {
                out.push('\\');
            }
            out.push_str(&escape_key(k));
            out.push('=');
            out.push_str(&escape_value(v));
            out.push('\n');
        }
        first = false;
    }
    out
}

/// Write an INI file to disk atomically.
pub fn save_ini_file(path: &Path, map: &IniMap) -> Result<()> {
    atomic_write(path, save_ini(map).as_bytes())
}

/// Legacy `\n` `\t` `\#` (and generic `\c` -> `c`) unescaping; a trailing
/// lone backslash is dropped, as in `INIFile::unescape`.
fn legacy_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            match c {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                '#' => out.push('#'),
                other => out.push(other),
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// Legacy conditional dequoting (`INIFile::unquote`).
fn unquote(s: String) -> String {
    let contains_special = s.contains([';', '=', ',']);
    if contains_special && s.len() >= 2 && s.starts_with('\"') && s.ends_with('\"') {
        s[1..s.len() - 1].to_string()
    } else {
        s
    }
}

/// Apply the `WideBarVisibility_*` / `UI/*_Page/Columns` / `linkedInstances`
/// / `Env` key migrations shared by all legacy loads (`migrateQByteArrayToBase64`).
fn migrate_keys(map: &mut IniMap) {
    let entries: Vec<(String, String)> = map.entries().to_vec();
    for (key, value) in entries {
        if key.starts_with("WideBarVisibility_")
            || (key.starts_with("UI/") && key.ends_with("_Page/Columns"))
        {
            map.set(&key, base64_encode(value.as_bytes()));
        } else if key == "linkedInstances" {
            map.set(&key, serde_json::json!([value]).to_string());
        } else if key == "Env" {
            map.set(&key, "{}");
        }
    }
}

/// Extra step applied for `ConfigVersion=1.1` files: re-unquote values that
/// still carry their old-style quoting.
fn migrate_keys_1_1(map: &mut IniMap) {
    migrate_keys(map);
    let entries: Vec<(String, String)> = map.entries().to_vec();
    for (key, value) in entries {
        let has_special = value.contains([';', '=', ',']);
        if has_special && value.len() >= 2 && value.starts_with('\"') && value.ends_with('\"') {
            map.set(&key, &value[1..value.len() - 1]);
        }
    }
}

/// Minimal standard base64 encoder (avoids a dedicated dependency).
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn qt_parse_basic_and_case_insensitive_last_wins() {
        let map = parse_qt("name=Alpha\nNAME=Beta\niconKey=default\n", Path::new("t")).unwrap();
        assert_eq!(map.get("name"), Some("Beta")); // last wins, case-insensitive
        assert_eq!(map.get("ICONKEY"), Some("default"));
        assert!(map.contains("Name"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn qt_parse_quoted_values_escapes_and_inline_comment() {
        let map = parse_qt(
            "notes=\"line1\\nline2 with ; and =\"\nplain=val ; comment\nnocomment=val;ue\n",
            Path::new("t"),
        )
        .unwrap();
        assert_eq!(map.get("notes"), Some("line1\nline2 with ; and ="));
        assert_eq!(map.get("plain"), Some("val"));
        // ';' outside quotes starts a comment (readIniLine)
        assert_eq!(map.get("noComment"), Some("val"));
    }

    #[test]
    fn qt_parse_hash_is_not_a_comment_in_qt_flavor() {
        let map = parse_qt("color=#ffcc00\n", Path::new("t")).unwrap();
        assert_eq!(map.get("color"), Some("#ffcc00"));
    }

    #[test]
    fn qt_parse_sections_map_to_slash_keys() {
        // Qt's actual encoding: [General] header + \-prefixed keys.
        let map = parse_qt("[General]\n\\UI\\Foo_Page\\Columns=x\nplain=1\n", Path::new("t")).unwrap();
        assert_eq!(map.get("UI/Foo_Page/Columns"), Some("x"));
        assert_eq!(map.get("plain"), Some("1"));
    }

    #[test]
    fn qt_parse_named_section_and_general_equivalents() {
        // Qt requires section headers alone on a line; keys follow on later
        // lines. "[%General]" is the escaped real section "General/" (Qt
        // `readIniFile`: "%general" -> section, not root), while "[general]"
        // (case-insensitive "general") is the root.
        let map = parse_qt("[UI]\n\\a=1\n[%General]\n\\b=2\n[general]\n\\c=3\n", Path::new("t")).unwrap();
        assert_eq!(map.get("UI/a"), Some("1"));
        assert_eq!(map.get("General/b"), Some("2"));
        assert_eq!(map.get("c"), Some("3"));
    }

    #[test]
    fn qt_parse_skips_lines_without_equals() {
        // "garbage" (no '=') is skipped; "[unclosed" (missing ']' tolerated
        // per Qt/header) sets section "unclosed/", so "bad2=x" becomes
        // "unclosed/bad2".
        let map = parse_qt("good=1\ngarbage\n\n[unclosed\nbad2=x\n", Path::new("t")).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("good"), Some("1"));
        assert_eq!(map.get("unclosed/bad2"), Some("x"));
    }

    #[test]
    fn qt_parse_percent_escaped_keys() {
        let map = parse_qt("%41%42%43=escaped\n", Path::new("t")).unwrap();
        assert_eq!(map.get("ABC"), Some("escaped"));
    }

    #[test]
    fn legacy_parse_comments_escapes_and_unquote() {
        let map = parse_legacy(
            "# full comment\nkey=value # trailing\nesc=\\#notcomment\nmulti\nquoted=\"a;b=c,d\"\nnoquote=\"simple\"\n",
            Path::new("t"),
        )
        .unwrap();
        assert_eq!(map.get("key"), Some("value"));
        assert_eq!(map.get("esc"), Some("#notcomment"));
        assert_eq!(map.get("quoted"), Some("a;b=c,d"));
        assert_eq!(map.get("noquote"), Some("\"simple\""));
    }

    #[test]
    fn legacy_parse_hash_at_column_zero_and_escaped_hash() {
        let map = parse_legacy("#comment\n", Path::new("t")).unwrap();
        assert!(map.is_empty());
        let map = parse_legacy("k=v#rest\n", Path::new("t")).unwrap();
        assert_eq!(map.get("k"), Some("v"));
    }

    #[test]
    fn legacy_load_migrates_and_injects_config_version() {
        let text = "linkedInstances=one,Wide\nWideBarVisibility_Main=AAAA\nname=Old\n";
        let map = load_ini(text, Path::new("t")).unwrap();
        assert_eq!(map.get("ConfigVersion"), Some("1.3"));
        assert_eq!(map.get("linkedInstances"), Some("[\"one,Wide\"]"));
        assert_eq!(map.get("WideBarVisibility_Main"), Some(base64_encode(b"AAAA").as_str()));
        assert_eq!(map.get("name"), Some("Old"));
    }

    #[test]
    fn save_sorted_case_insensitive_and_injects_config_version() {
        let mut map = IniMap::new();
        map.set("iconKey", "default");
        map.set("ConfigVersion", "1.3");
        map.set("InstanceType", "OneSix");
        map.set("name", "Test");
        let out = save_ini(&map);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines,
            vec!["[General]", "ConfigVersion=1.3", "iconKey=default", "InstanceType=OneSix", "name=Test"]
        );
    }

    #[test]
    fn save_escapes_specials_and_empty_values() {
        let mut map = IniMap::new();
        map.set("notes", "multi\nline; with = and ,");
        map.set("empty", "");
        map.set("plain", "true");
        let out = save_ini(&map);
        assert!(out.contains("notes=\"multi\\nline; with = and ,\"\n"));
        assert!(out.contains("empty=\n"));
        assert!(out.contains("plain=true\n"));
    }

    #[test]
    fn save_sections_round_trip() {
        let mut map = IniMap::new();
        map.set("UI/Foo_Page/Columns", "x,y");
        map.set("root", "1");
        let text = save_ini(&map);
        // [General] first, then the UI section with the \-encoded key.
        // `save_ini` injects `ConfigVersion=1.3` by design.
        let expected = "[General]\nConfigVersion=1.3\nroot=1\n\n[UI]\n\\Foo_Page\\Columns=\"x,y\"\n";
        assert_eq!(text, expected);
        let back = parse_qt(&text, Path::new("t")).unwrap();
        assert_eq!(back.get("UI/Foo_Page/Columns"), Some("x,y"));
        assert_eq!(back.get("root"), Some("1"));
    }

    #[test]
    fn round_trip_exotic_values_is_lossless() {
        let mut map = IniMap::new();
        for (i, v) in [
            "simple",
            "with space",
            " semi; colon",
            "comma,slice",
            "eq=sign",
            "quote\"inside",
            "back\\slash",
            "new\nline",
            "tab\there",
            "\u{7}bell",
            "",
        ]
        .into_iter()
        .enumerate()
        {
            map.set(&format!("k{i}"), v);
        }
        let text = save_ini(&map);
        let back = parse_qt(&text, Path::new("t")).unwrap();
        for (i, v) in [
            "simple",
            "with space",
            " semi; colon",
            "comma,slice",
            "eq=sign",
            "quote\"inside",
            "back\\slash",
            "new\nline",
            "tab\there",
            "\u{7}bell",
            "",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(back.get(&format!("k{i}")), Some(v), "round trip failed for {v:?}");
        }
    }

    #[test]
    fn case_insensitive_sort_and_first_case_preserved() {
        let mut map = IniMap::new();
        map.set("Zebra", "last");
        map.set("apple", "first");
        map.set("ZEBRA", "overwritten");
        assert_eq!(map.get("zebra"), Some("overwritten"));
        let text = save_ini(&map);
        let apple = text.find("apple").unwrap();
        let zebra = text.find("Zebra").unwrap();
        assert!(apple < zebra);
    }

    #[test]
    fn load_ini_1_2_migrates_keys() {
        let text = "ConfigVersion=1.2\nWideBarVisibility_Main=AAAA\n";
        let map = load_ini(text, Path::new("t")).unwrap();
        assert_eq!(map.get("ConfigVersion"), Some("1.3"));
        assert_eq!(map.get("WideBarVisibility_Main"), Some(base64_encode(b"AAAA").as_str()));
    }
}