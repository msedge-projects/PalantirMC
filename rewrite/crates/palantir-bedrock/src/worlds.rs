//! The worlds a player already has: listed, read, and exported -- never opened.
//!
//! A Bedrock world is a folder whose heart is a modified LevelDB under `db/`,
//! and LevelDB is a log-structured store with a live write-ahead log: opening
//! it while the game holds it is how a launcher corrupts somebody's world. So
//! this module does the two things that are safe -- read the flat `level.dat`
//! the game itself reads, and copy the folder's bytes into a `.mcworld` archive
//! -- and refuses everything else. Nothing here creates a folder, writes into a
//! world, or touches the database; the export copies bytes verbatim precisely
//! so that a database mid-write is never interpreted by us.
//!
//! `level.dat` is gzipped NBT, so reading a world's name means reading NBT.
//! The reader is written here rather than pulled in because the crate has no
//! dependency on a general NBT library, and the subset a launcher needs is
//! small -- but it still walks *every* tag, because a reader that guesses a
//! tag's length desynchronises the stream and reports nonsense from then on.
//! A file on disk is untrusted input like any other, so nesting depth and
//! declared lengths are bounded and refused loudly.

use std::io::Read as _;
#[cfg(test)]
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// What a world folder looks like from outside, without opening its database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSummary {
    /// The folder name under `minecraftWorlds`, which is also the world's
    /// identity to the game.
    pub folder: String,
    /// The name the player gave it, or the folder name when `level.dat` could
    /// not be read.
    pub name: String,
    pub last_played: Option<i64>,
    pub game_version: Option<String>,
    /// False when `level.dat` exists but could not be parsed. Such a world is
    /// still listed (see [`list_worlds`]) and is not treated as damaged beyond
    /// what was actually observed.
    pub readable: bool,
}

/// The fields of a `level.dat` a launcher has any business knowing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LevelData {
    pub name: Option<String>,
    /// Unix milliseconds, as the game stores it.
    pub last_played: Option<i64>,
    pub version_name: Option<String>,
    pub storage_version: Option<i32>,
    pub day_time: Option<i64>,
}

/// What an export actually wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldExport {
    pub entries: usize,
    pub bytes: u64,
}

/// The nesting a `level.dat` may reach. The real documents are three or four
/// levels deep; anything past this is a hostile file trying to exhaust the
/// stack, and a launcher that panics has told the player nothing.
const MAX_DEPTH: usize = 64;

/// The largest decompressed `level.dat` accepted. The real file is a few
/// kilobytes; the bound is here so that a compressed input cannot expand
/// without limit.
const MAX_LEVEL_DAT: u64 = 64 * 1024 * 1024;

const TAG_END: u8 = 0;
const TAG_COMPOUND: u8 = 10;

/// Read one world's `level.dat`.
///
/// Accepts both spellings of the file: gzipped, which is what the game writes,
/// and a plain NBT stream, which is what a hand-edited or third-party world
/// sometimes carries. Which one it is, is decided by the gzip magic rather than
/// by the name, because the name is the one thing a file cannot be trusted on.
pub fn read_level_dat(path: &Path) -> Result<LevelData, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .take(MAX_LEVEL_DAT + 9)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_LEVEL_DAT + 8 {
        return Err("level.dat exceeds 64 MiB.".into());
    }
    let raw = if bytes.starts_with(&[0x1f, 0x8b]) {
        inflate(&bytes, path)?
    } else {
        bytes
    };
    // Bedrock writes an eight-byte little-endian version/length header followed
    // by little-endian NBT. Also keep the Java-style streams accepted by the
    // original reader for third-party exports and existing fixtures.
    let has_bedrock_header = raw.len() > 8 && raw[1..4] == [0, 0, 0] && raw[8] == TAG_COMPOUND;
    let fields = if has_bedrock_header || (raw.first() != Some(&TAG_COMPOUND) && raw.len() >= 8) {
        let length = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]) as usize;
        if length != raw.len() - 8 {
            return Err("Bedrock level.dat header length does not match its payload.".into());
        }
        parse_root_in(&raw[8..], true)
    } else {
        parse_root(&raw)
    }
    .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(LevelData::from_fields(&fields))
}

/// Decompress a gzipped `level.dat`, refusing one that expands past the bound
/// instead of reading until the memory runs out.
fn inflate(bytes: &[u8], path: &Path) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_LEVEL_DAT + 1)
        .read_to_end(&mut out)
        .map_err(|error| format!("{}: it is not readable gzip: {error}", path.display()))?;
    if out.len() as u64 > MAX_LEVEL_DAT {
        return Err(format!(
            "{}: it expands past the {MAX_LEVEL_DAT} byte limit",
            path.display()
        ));
    }
    Ok(out)
}

/// List the worlds under a `minecraftWorlds` folder, newest first.
///
/// A folder is a world when it holds a `level.dat`, which is the same rule the
/// game uses. Nothing else is inspected: `db/` is never opened, so a world the
/// game currently has open lists exactly like one it does not.
///
/// A world whose `level.dat` cannot be parsed is still listed, with the folder
/// as its display name and `readable: false`. Hiding it would be worse than
/// showing it: the player would conclude their world had been deleted when the
/// launcher had merely failed to read one file -- and the folder is still
/// there, still playable, still theirs.
///
/// A folder that does not exist yields no worlds rather than an error: a copy
/// of Bedrock that has never been opened has no worlds, which is a state to
/// show, not a failure to report.
pub fn list_worlds(minecraft_worlds: &Path) -> Result<Vec<WorldSummary>, String> {
    let entries = match std::fs::read_dir(minecraft_worlds) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{}: {error}", minecraft_worlds.display())),
    };
    let mut worlds = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", minecraft_worlds.display()))?;
        // `file_type` does not follow links, which is what makes this the place
        // to refuse them: a world directory that is a link points wherever it
        // likes, and this launcher does not list somebody else's folder as a
        // world of ours.
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        if !kind.is_dir() || kind.is_symlink() {
            continue;
        }
        let dir = entry.path();
        if !is_world_dir(&dir) {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().into_owned();
        match read_level_dat(&dir.join("level.dat")) {
            Ok(level) => worlds.push(WorldSummary {
                name: level.name.clone().unwrap_or_else(|| folder.clone()),
                last_played: level.last_played,
                game_version: level.version_name,
                folder,
                readable: true,
            }),
            Err(_) => worlds.push(WorldSummary {
                name: folder.clone(),
                last_played: None,
                game_version: None,
                folder,
                readable: false,
            }),
        }
    }
    // Newest first, and a world we could not date goes last rather than
    // pretending to be from 1970.
    worlds.sort_by(|left, right| {
        right
            .last_played
            .cmp(&left.last_played)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(worlds)
}

/// Is this folder a world? The game's own rule: it holds a `level.dat`.
pub fn is_world_dir(dir: &Path) -> bool {
    let is_real_dir = std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir());
    is_real_dir && dir.join("level.dat").is_file()
}

/// Write a copy of `world_dir` as a `.mcworld` archive.
///
/// The archive holds the world's own files at its root with their relative
/// paths, which is the layout the game's own importer expects. Every entry is
/// copied byte for byte: `db/` is never parsed, so a LevelDB caught mid-write
/// is copied as it is rather than reinterpreted, and nothing in this function
/// ever writes into `world_dir` -- `label` is the player-facing name that goes
/// in the archive's comment.
///
/// It refuses rather than half-exports: an existing destination, a symlinked
/// source or entry, a name that would leave the archive, and a source that is
/// not a world at all. A partial archive left behind by a failure is removed,
/// because a truncated `.mcworld` imports as a corrupt world.
pub fn export_world(
    world_dir: &Path,
    destination: &Path,
    label: &str,
) -> Result<WorldExport, String> {
    if label.trim().is_empty() || label.chars().any(char::is_control) {
        return Err("An export needs a name between 1 and 80 characters.".into());
    }
    let source = std::fs::symlink_metadata(world_dir)
        .map_err(|error| format!("{}: {error}", world_dir.display()))?;
    if source.file_type().is_symlink() || !source.is_dir() {
        return Err(format!(
            "{} is not a folder this launcher will export.",
            world_dir.display()
        ));
    }
    if !is_world_dir(world_dir) {
        return Err(format!(
            "{} holds no level.dat, so it is not a world.",
            world_dir.display()
        ));
    }
    if std::fs::symlink_metadata(destination).is_ok() {
        return Err(format!(
            "{} already exists; an export never overwrites one.",
            destination.display()
        ));
    }
    // Best effort, and named as such: `absolute` does not resolve `..`, so this
    // catches the ordinary mistake of naming a destination inside the world
    // rather than every way of expressing one.
    let source_path = std::path::absolute(world_dir).map_err(|error| error.to_string())?;
    let destination_path = std::path::absolute(destination).map_err(|error| error.to_string())?;
    if destination_path.starts_with(&source_path) {
        return Err("The export cannot be written inside the world it copies.".into());
    }
    let mut files = Vec::new();
    let mut bytes = 0u64;
    collect_files(world_dir, "", &mut files, &mut bytes)?;
    // A stable order makes the same world export to the same archive.
    files.sort_by(|left, right| left.0.cmp(&right.0));

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    // Reserve the output exclusively before writing: a file created after the
    // review must not be truncated, nor removed by failed-export cleanup.
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| format!("{}: {error}", destination.display()))?;
    let written = write_archive(output, destination, &files, label);
    if written.is_err() {
        let _ = std::fs::remove_file(destination);
    }
    written?;
    Ok(WorldExport {
        entries: files.len(),
        bytes,
    })
}

fn write_archive(
    file: std::fs::File,
    destination: &Path,
    files: &[(String, PathBuf, u64)],
    label: &str,
) -> Result<(), String> {
    let mut archive = zip::ZipWriter::new(file);
    // The archive's comment is the only place a `.mcworld` carries a title, and
    // a player who exports three worlds of the same name deserves to be able to
    // tell the files apart.
    archive.set_comment(label);
    for (name, path, _) in files {
        // LevelDB is already compressed and mostly immutable; re-deflating it
        // costs time and buys nothing, while `level.dat` and the small json
        // files beside it are worth compressing.
        let method = if name.starts_with("db/") {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        };
        let options = zip::write::SimpleFileOptions::default().compression_method(method);
        archive
            .start_file(name.as_str(), options)
            .map_err(|error| format!("{name}: {error}"))?;
        let mut contents =
            std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
        std::io::copy(&mut contents, &mut archive)
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    archive
        .finish()
        .map_err(|error| format!("{}: {error}", destination.display()))?;
    Ok(())
}

/// Every file under `dir`, as (path inside the archive, path on disk, size).
///
/// Symlinks are refused rather than followed: a link in a world folder is a
/// link out of it, and an archive that followed one would carry bytes the
/// player never agreed to share. Entries that are neither files nor folders
/// (a socket, a device) are skipped -- copying them would hang.
fn collect_files(
    dir: &Path,
    relative: &str,
    out: &mut Vec<(String, PathBuf, u64)>,
    bytes: &mut u64,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", dir.display()))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        if kind.is_symlink() {
            return Err(format!(
                "{} is a link; an export copies a world's own bytes.",
                entry.path().display()
            ));
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
            return Err(format!("{name:?} is not a usable name inside a world."));
        }
        let inside = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        if kind.is_dir() {
            collect_files(&entry.path(), &inside, out, bytes)?;
        } else if kind.is_file() {
            let size = entry
                .metadata()
                .map_err(|error| format!("{}: {error}", entry.path().display()))?
                .len();
            *bytes += size;
            out.push((inside, entry.path(), size));
        }
    }
    Ok(())
}

impl LevelData {
    /// Read the fields a launcher knows out of the root compound.
    fn from_fields(fields: &[(String, Value)]) -> Self {
        let nested = |compound: &str, key: &str| {
            fields
                .iter()
                .find(|(name, _)| name == compound)
                .and_then(|(_, value)| match value {
                    Value::Compound(inner) => inner
                        .iter()
                        .find(|(name, _)| name == key)
                        .and_then(|(_, value)| value.as_string())
                        .map(str::to_string),
                    _ => None,
                })
        };
        Self {
            name: field(fields, "LevelName")
                .and_then(Value::as_string)
                .map(str::to_string),
            last_played: field(fields, "LastPlayed").and_then(Value::as_long),
            // The nested spelling is what the game writes; the flat one is what
            // some third-party tools write, and reading only one of them would
            // report a version for half the worlds on a machine.
            version_name: nested("Version", "Name").or_else(|| {
                field(fields, "VersionName")
                    .and_then(Value::as_string)
                    .map(str::to_string)
            }),
            storage_version: field(fields, "StorageVersion").and_then(Value::as_int),
            day_time: field(fields, "DayTime").and_then(Value::as_long),
        }
    }
}

fn field<'a>(fields: &'a [(String, Value)], name: &str) -> Option<&'a Value> {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

/// The root compound's fields. A `level.dat` whose root is not a compound is
/// refused rather than guessed at.
fn parse_root(bytes: &[u8]) -> Result<Vec<(String, Value)>, String> {
    parse_root_in(bytes, false)
}
fn parse_root_in(bytes: &[u8], little_endian: bool) -> Result<Vec<(String, Value)>, String> {
    let mut reader = Reader::new(bytes);
    reader.little_endian = little_endian;
    let kind = reader.u8()?;
    if kind != TAG_COMPOUND {
        return Err(format!("its root tag is {kind}, not a compound"));
    }
    let _root_name = reader.string()?;
    reader.compound_body(0)
}

/// One NBT value, reduced to what this crate reads.
///
/// The payloads a launcher never looks at -- floats, arrays, lists -- are still
/// *walked* when they are parsed, but they are not kept: a `level.dat` holds
/// megabytes of terrain metadata we have no business holding a second copy of.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    /// A tag that was read and walked, and means nothing to this crate.
    Skipped,
    /// Every integer width the format defines, widened to one type; the
    /// distinction matters to the game and not to a list of worlds.
    Number(i64),
    Text(String),
    Compound(Vec<(String, Value)>),
}

impl Value {
    fn as_string(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }
    fn as_long(&self) -> Option<i64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }
    fn as_int(&self) -> Option<i32> {
        match self {
            Self::Number(value) => i32::try_from(*value).ok(),
            _ => None,
        }
    }
}

/// A bounds-checked cursor over the tag stream. Every read is checked, so a
/// truncated or hostile file becomes an error message instead of a panic.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    little_endian: bool,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            at: 0,
            little_endian: false,
        }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(count)
            .ok_or_else(|| "a declared length overflowed".to_string())?;
        if end > self.bytes.len() {
            return Err(format!(
                "it claims {count} more byte(s) at offset {} but only {} remain",
                self.at,
                self.bytes.len().saturating_sub(self.at)
            ));
        }
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let bytes = self.take(2)?;
        Ok(if self.little_endian {
            u16::from_le_bytes([bytes[0], bytes[1]])
        } else {
            u16::from_be_bytes([bytes[0], bytes[1]])
        })
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(self.u16()? as i16)
    }
    fn i32(&mut self) -> Result<i32, String> {
        let bytes = self.take(4)?;
        let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
        Ok(if self.little_endian {
            i32::from_le_bytes(bytes)
        } else {
            i32::from_be_bytes(bytes)
        })
    }
    fn i64(&mut self) -> Result<i64, String> {
        let bytes = self.take(8)?;
        let bytes = [
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ];
        Ok(if self.little_endian {
            i64::from_le_bytes(bytes)
        } else {
            i64::from_be_bytes(bytes)
        })
    }
    fn f32(&mut self) -> Result<f32, String> {
        let bytes = self.take(4)?;
        Ok(f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    fn f64(&mut self) -> Result<f64, String> {
        let bytes = self.take(8)?;
        Ok(f64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }
    /// An NBT string: a big-endian length, then that many UTF-8 bytes.
    fn string(&mut self) -> Result<String, String> {
        let length = self.u16()? as usize;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| "a name is not valid UTF-8".to_string())
    }
    /// A compound body: named tags until `TAG_End`. Every tag is walked, so a
    /// tag this crate does not use still has to be parsed correctly.
    fn compound_body(&mut self, depth: usize) -> Result<Vec<(String, Value)>, String> {
        if depth > MAX_DEPTH {
            return Err(format!("it nests past {MAX_DEPTH} levels"));
        }
        let mut fields = Vec::new();
        loop {
            let kind = self.u8()?;
            if kind == TAG_END {
                return Ok(fields);
            }
            let name = self.string()?;
            let value = self.payload(kind, depth + 1)?;
            fields.push((name, value));
        }
    }
    /// A list's elements, walked for their own lengths and dropped.
    ///
    /// The list still has to be parsed element by element: the stream has no
    /// element count in bytes, so skipping it without reading would leave the
    /// cursor somewhere the next tag does not start.
    fn list_body(&mut self, depth: usize) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err(format!("it nests past {MAX_DEPTH} levels"));
        }
        let kind = self.u8()?;
        let count = self.i32()?;
        if count < 0 {
            return Err(format!("a list declares {count} entries"));
        }
        // The format allows an empty list to name `TAG_End` as its element.
        if kind == TAG_END {
            return Ok(());
        }
        for _ in 0..count {
            self.payload(kind, depth + 1)?;
        }
        Ok(())
    }
    fn payload(&mut self, kind: u8, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(format!("it nests past {MAX_DEPTH} levels"));
        }
        Ok(match kind {
            1 => Value::Number(i64::from(self.u8()? as i8)),
            2 => Value::Number(i64::from(self.i16()?)),
            3 => Value::Number(i64::from(self.i32()?)),
            4 => Value::Number(self.i64()?),
            // Floats, arrays and lists are walked for their length and then
            // dropped: nothing a launcher shows lives in them.
            5 => {
                self.f32()?;
                Value::Skipped
            }
            6 => {
                self.f64()?;
                Value::Skipped
            }
            7 => {
                let count = self.i32()?;
                if count < 0 {
                    return Err(format!("a byte array declares {count} bytes"));
                }
                self.take(count as usize)?;
                Value::Skipped
            }
            8 => Value::Text(self.string()?),
            9 => {
                self.list_body(depth)?;
                Value::Skipped
            }
            10 => Value::Compound(self.compound_body(depth)?),
            11 | 12 => {
                let count = self.i32()?;
                if count < 0 {
                    return Err(format!("an array declares {count} entries"));
                }
                let width = if kind == 11 { 4 } else { 8 };
                self.take((count as usize).saturating_mul(width))?;
                Value::Skipped
            }
            other => return Err(format!("tag type {other} is not one the format defines")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temp folder of this test's own, cleaned up by the caller.
    fn temp_root(tag: &str) -> PathBuf {
        let name = format!(
            "palantirmc-worlds-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // ---- a minimal NBT encoder, test-only ----
    //
    // The library reads NBT and never writes it; a writer in the crate would be
    // unused code that a linter would rightly question. These helpers exist to
    // build the bytes a real `level.dat` has.

    fn push_string(out: &mut Vec<u8>, text: &str) {
        out.extend((text.len() as u16).to_be_bytes());
        out.extend(text.as_bytes());
    }
    fn named(out: &mut Vec<u8>, kind: u8, name: &str, payload: &[u8]) {
        out.push(kind);
        push_string(out, name);
        out.extend(payload);
    }
    fn string_tag(name: &str, value: &str) -> Vec<u8> {
        let mut payload = Vec::new();
        push_string(&mut payload, value);
        let mut out = Vec::new();
        named(&mut out, 8, name, &payload);
        out
    }
    fn long_tag(name: &str, value: i64) -> Vec<u8> {
        let mut out = Vec::new();
        named(&mut out, 4, name, &value.to_be_bytes());
        out
    }
    fn int_tag(name: &str, value: i32) -> Vec<u8> {
        let mut out = Vec::new();
        named(&mut out, 3, name, &value.to_be_bytes());
        out
    }
    fn compound_tag(name: &str, children: &[Vec<u8>]) -> Vec<u8> {
        let mut body = Vec::new();
        for child in children {
            body.extend(child.iter());
        }
        body.push(TAG_END);
        let mut out = Vec::new();
        named(&mut out, TAG_COMPOUND, name, &body);
        out
    }
    /// A whole `level.dat`: root compound, unrooted name, the fields below.
    fn level_dat_bytes(nested_version: bool) -> Vec<u8> {
        level_dat_played_at(1_700_000_000_000, nested_version)
    }
    fn level_dat_played_at(last_played: i64, nested_version: bool) -> Vec<u8> {
        let mut children = vec![
            string_tag("LevelName", "Quiet valley"),
            long_tag("LastPlayed", last_played),
            int_tag("StorageVersion", 10),
            long_tag("DayTime", 4_321),
        ];
        if nested_version {
            children.push(compound_tag(
                "Version",
                &[int_tag("Id", 42), string_tag("Name", "1.21.120")],
            ));
        }
        let mut root = Vec::new();
        root.push(TAG_COMPOUND);
        push_string(&mut root, "");
        for child in &children {
            root.extend(child.iter());
        }
        root.push(TAG_END);
        root
    }
    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn bedrocks_header_and_little_endian_nbt_read_the_real_title() {
        let root = temp_root("bedrock-header");
        let mut nbt = vec![10, 0, 0, 8];
        nbt.extend_from_slice(&9u16.to_le_bytes());
        nbt.extend_from_slice(b"LevelName");
        nbt.extend_from_slice(&12u16.to_le_bytes());
        nbt.extend_from_slice(b"Bedrock home");
        nbt.push(4);
        nbt.extend_from_slice(&10u16.to_le_bytes());
        nbt.extend_from_slice(b"LastPlayed");
        nbt.extend_from_slice(&123456789i64.to_le_bytes());
        nbt.push(0);
        let mut file = 10u32.to_le_bytes().to_vec();
        file.extend_from_slice(&(nbt.len() as u32).to_le_bytes());
        file.extend_from_slice(&nbt);
        let path = root.join("level.dat");
        std::fs::write(&path, &file).unwrap();
        let level = read_level_dat(&path).unwrap();
        assert_eq!(level.name.as_deref(), Some("Bedrock home"));
        assert_eq!(level.last_played, Some(123456789));
        file[4] += 1;
        std::fs::write(&path, file).unwrap();
        assert!(read_level_dat(&path).unwrap_err().contains("length"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn a_gzipped_level_dat_reads_its_name_and_dates() {
        let root = temp_root("read");
        let path = root.join("level.dat");
        std::fs::write(&path, gzip(&level_dat_bytes(true))).unwrap();
        let level = read_level_dat(&path).unwrap();
        assert_eq!(level.name.as_deref(), Some("Quiet valley"));
        assert_eq!(level.last_played, Some(1_700_000_000_000));
        assert_eq!(level.storage_version, Some(10));
        assert_eq!(level.day_time, Some(4_321));
        assert_eq!(level.version_name.as_deref(), Some("1.21.120"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_uncompressed_level_dat_reads_the_same_way() {
        let root = temp_root("raw");
        let path = root.join("level.dat");
        std::fs::write(&path, level_dat_bytes(true)).unwrap();
        let level = read_level_dat(&path).unwrap();
        assert_eq!(level.name.as_deref(), Some("Quiet valley"));
        assert_eq!(level.version_name.as_deref(), Some("1.21.120"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_flat_version_spelling_is_still_read() {
        let root = temp_root("flat");
        let path = root.join("level.dat");
        let mut bytes = level_dat_bytes(false);
        // Replace the root's end tag with one more field, keeping the stream
        // valid: some tools write the version as a plain string instead of the
        // nested compound the game writes.
        bytes.pop();
        bytes.extend(string_tag("VersionName", "1.20.1"));
        bytes.push(TAG_END);
        std::fs::write(&path, bytes).unwrap();
        let level = read_level_dat(&path).unwrap();
        assert_eq!(level.version_name.as_deref(), Some("1.20.1"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_truncated_stream_is_an_error_not_a_panic() {
        let root = temp_root("truncated");
        let path = root.join("level.dat");
        let bytes = level_dat_bytes(true);
        std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        let error = read_level_dat(&path).unwrap_err();
        assert!(
            error.contains("remain") || error.contains("not readable gzip"),
            "{error}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_length_larger_than_the_file_is_refused() {
        let mut bytes = Vec::new();
        bytes.push(TAG_COMPOUND);
        push_string(&mut bytes, "");
        // A string claiming 5000 bytes in a file that holds none of them.
        bytes.push(8);
        push_string(&mut bytes, "LevelName");
        bytes.extend(5000u16.to_be_bytes());
        bytes.extend(b"short");
        bytes.push(TAG_END);
        let error = parse_root(&bytes).unwrap_err();
        assert!(error.contains("claims"), "{error}");
    }

    #[test]
    fn a_deeply_nested_file_is_refused_instead_of_overflowing() {
        let mut inner = vec![TAG_END];
        for _ in 0..MAX_DEPTH + 8 {
            let mut next = Vec::new();
            named(&mut next, TAG_COMPOUND, "Level", &inner);
            next.push(TAG_END);
            inner = next;
        }
        let mut bytes = Vec::new();
        bytes.push(TAG_COMPOUND);
        push_string(&mut bytes, "");
        bytes.extend(inner);
        let error = parse_root(&bytes).unwrap_err();
        assert!(error.contains("nests past"), "{error}");
    }

    #[test]
    fn an_unknown_tag_type_is_reported_rather_than_skipped() {
        let mut bytes = Vec::new();
        bytes.push(TAG_COMPOUND);
        push_string(&mut bytes, "");
        bytes.push(99);
        push_string(&mut bytes, "Mystery");
        let error = parse_root(&bytes).unwrap_err();
        assert!(error.contains("99"), "{error}");
    }

    #[test]
    fn worlds_list_newest_first_and_skip_everything_else() {
        let root = temp_root("list");
        let worlds = root.join("minecraftWorlds");
        std::fs::create_dir_all(worlds.join("younger")).unwrap();
        std::fs::create_dir_all(worlds.join("older")).unwrap();
        std::fs::create_dir_all(worlds.join("damaged")).unwrap();
        std::fs::create_dir_all(worlds.join("not-a-world")).unwrap();
        std::fs::write(
            worlds.join("younger/level.dat"),
            gzip(&level_dat_played_at(1_700_000_000_000, true)),
        )
        .unwrap();
        std::fs::write(
            worlds.join("older/level.dat"),
            gzip(&level_dat_played_at(1_600_000_000_000, true)),
        )
        .unwrap();
        // A world whose level.dat is unreadable is still a world; a folder with
        // no level.dat and a loose file are not.
        std::fs::write(worlds.join("damaged/level.dat"), b"garbage").unwrap();
        std::fs::write(worlds.join("loose.dat"), b"x").unwrap();
        let listed = list_worlds(&worlds).unwrap();
        let folders: Vec<&str> = listed.iter().map(|world| world.folder.as_str()).collect();
        assert_eq!(folders, ["younger", "older", "damaged"]);
        assert!(listed[0].readable && listed[1].readable);
        assert!(!listed[2].readable);
        assert_eq!(listed[0].last_played, Some(1_700_000_000_000));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_unreadable_world_is_listed_rather_than_hidden() {
        let root = temp_root("damaged");
        let worlds = root.join("minecraftWorlds");
        std::fs::create_dir_all(worlds.join("half-a-world")).unwrap();
        std::fs::write(worlds.join("half-a-world/level.dat"), b"garbage").unwrap();
        let listed = list_worlds(&worlds).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "half-a-world");
        assert!(!listed[0].readable);
        assert_eq!(listed[0].last_played, None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn worlds_never_exist_before_bedrock_has_run() {
        let root = temp_root("absent");
        let listed = list_worlds(&root.join("minecraftWorlds")).unwrap();
        assert!(listed.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_folder_is_not_a_world_of_ours() {
        let root = temp_root("linked");
        let worlds = root.join("minecraftWorlds");
        std::fs::create_dir_all(&worlds).unwrap();
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("level.dat"), gzip(&level_dat_bytes(true))).unwrap();
        std::os::unix::fs::symlink(&elsewhere, worlds.join("link")).unwrap();
        assert!(list_worlds(&worlds).unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    fn world_with_files(root: &Path) -> PathBuf {
        let world = root.join("minecraftWorlds/valley");
        std::fs::create_dir_all(world.join("db")).unwrap();
        std::fs::write(world.join("level.dat"), gzip(&level_dat_bytes(true))).unwrap();
        std::fs::write(world.join("db/MANIFEST-000001"), b"leveldb bytes").unwrap();
        std::fs::write(world.join("levelname.txt"), b"Quiet valley").unwrap();
        world
    }

    fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(next) = stack.pop() {
            for entry in std::fs::read_dir(&next).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    stack.push(entry.path());
                } else {
                    files.push((entry.path(), std::fs::read(entry.path()).unwrap()));
                }
            }
        }
        files.sort();
        files
    }

    #[test]
    fn exporting_copies_the_world_without_touching_it() {
        let root = temp_root("export");
        let world = world_with_files(&root);
        let before = snapshot(&world);
        let destination = root.join("valley.mcworld");
        let report = export_world(&world, &destination, "Quiet valley").unwrap();
        assert_eq!(report.entries, 3);
        assert!(report.bytes > 0);
        let file = std::fs::File::open(&destination).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut names: Vec<String> = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["db/MANIFEST-000001", "level.dat", "levelname.txt"]);
        assert_eq!(snapshot(&world), before, "the world must be untouched");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exporting_refuses_an_existing_destination() {
        let root = temp_root("exists");
        let world = world_with_files(&root);
        let destination = root.join("valley.mcworld");
        std::fs::write(&destination, b"already here").unwrap();
        let error = export_world(&world, &destination, "Quiet valley").unwrap_err();
        assert!(error.contains("already exists"), "{error}");
        assert_eq!(std::fs::read(&destination).unwrap(), b"already here");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exporting_refuses_a_folder_that_is_not_a_world() {
        let root = temp_root("notworld");
        let plain = root.join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let error = export_world(&plain, &root.join("plain.mcworld"), "Plain").unwrap_err();
        assert!(error.contains("level.dat"), "{error}");
        assert!(!root.join("plain.mcworld").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_export_inside_the_world_is_refused() {
        let root = temp_root("inside");
        let world = world_with_files(&root);
        let error =
            export_world(&world, &world.join("valley.mcworld"), "Quiet valley").unwrap_err();
        assert!(error.contains("inside the world"), "{error}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn exporting_refuses_a_linked_source_or_entry() {
        let root = temp_root("links");
        let world = world_with_files(&root);
        let linked = root.join("linked-world");
        std::os::unix::fs::symlink(&world, &linked).unwrap();
        let error = export_world(&linked, &root.join("a.mcworld"), "Linked").unwrap_err();
        assert!(error.contains("not a folder"), "{error}");
        std::os::unix::fs::symlink(root.join("elsewhere.bin"), world.join("db/LINKED")).unwrap();
        let error = export_world(&world, &root.join("b.mcworld"), "Quiet valley").unwrap_err();
        assert!(error.contains("is a link"), "{error}");
        assert!(
            !root.join("b.mcworld").exists(),
            "a failed export leaves nothing"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_world_folder_is_recognised_by_its_level_dat() {
        let root = temp_root("isworld");
        let world = world_with_files(&root);
        assert!(is_world_dir(&world));
        assert!(!is_world_dir(&root));
        std::fs::remove_dir_all(root).unwrap();
    }
}
