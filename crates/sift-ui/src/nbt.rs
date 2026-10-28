//! Enough NBT to read a world's name.
//!
//! `level.dat` is a gzipped NBT document, and the only field this launcher needs from it is
//! `Data.LevelName` — the name the player gave the world, which `read_worlds` was showing as
//! the folder's name instead. Porting the whole format would be the usual mistake: what is
//! here reads the tags that can contain the field (compounds and strings) and skips every
//! other tag by its length, which is the part that cannot be skipped without losing the
//! reader's place.
//!
//! The format: <https://minecraft.wiki/w/NBT_format>. Every tag is `id, name, payload`; a
//! compound is a list of tags ended by TAG_End, a string is a `u16` length and that many
//! bytes of (modified) UTF-8, and the payload sizes of the rest are fixed or given.

use std::io::Read;
use std::path::Path;

/// The tag ids this reader names. The rest are skipped by size.
const TAG_END: u8 = 0;
const TAG_BYTE: u8 = 1;
const TAG_SHORT: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_LONG: u8 = 4;
const TAG_FLOAT: u8 = 5;
const TAG_DOUBLE: u8 = 6;
const TAG_BYTE_ARRAY: u8 = 7;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;
const TAG_INT_ARRAY: u8 = 11;
const TAG_LONG_ARRAY: u8 = 12;

/// Read a world's name from its folder, or `None` when there is no readable `level.dat`.
///
/// The name is whatever the player typed, which can be empty or only spaces; a name that says
/// nothing is no better than the folder, so it counts as missing.
pub fn world_name(world_dir: &Path) -> Option<String> {
    name_from_level_dat(&std::fs::read(world_dir.join("level.dat")).ok()?)
}

/// The same read over bytes already in hand: a `level.dat`'s contents, gzipped or not.
pub fn name_from_level_dat(bytes: &[u8]) -> Option<String> {
    let root = document(bytes)?;
    let name = find_string(&root, &["Data", "LevelName"])?;
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Decompress if the file is gzipped, then parse the root compound.
fn document(bytes: &[u8]) -> Option<Tag> {
    // A gzip member starts `1f 8b`; an uncompressed document starts with its root tag's id.
    let raw = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .ok()?;
        out
    } else {
        bytes.to_vec()
    };
    let mut reader = Reader { bytes: &raw, at: 0 };
    let id = reader.u8()?;
    if id != TAG_COMPOUND {
        return None;
    }
    let _name = reader.string()?;
    reader.compound()
}

/// One parsed tag. Only the shapes a `LevelName` can travel in are kept; everything else
/// parses to [`Tag::Other`] so the reader still knows where it is.
#[derive(Debug, Clone, PartialEq)]
enum Tag {
    Compound(Vec<(String, Tag)>),
    String(String),
    Other,
}

impl Tag {
    fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, tag)| tag),
            _ => None,
        }
    }
}

/// Walk a path of compound keys down to a string.
fn find_string<'a>(root: &'a Tag, path: &[&str]) -> Option<&'a str> {
    let mut here = root;
    for key in path {
        here = here.get(key)?;
    }
    match here {
        Tag::String(text) => Some(text.as_str()),
        _ => None,
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(count)?;
        let slice = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|slice| slice[0])
    }

    fn u16(&mut self) -> Option<u16> {
        self.take(2)
            .map(|slice| u16::from_be_bytes([slice[0], slice[1]]))
    }

    fn i32(&mut self) -> Option<i32> {
        self.take(4).map(|slice| {
            i32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]])
        })
    }

    /// A string: `u16` byte length, then that many bytes. NBT writes UTF-8, which is what
    /// `String::from_utf8` reads; invalid bytes are not a world name, so they fail here.
    fn string(&mut self) -> Option<String> {
        let length = self.u16()? as usize;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    /// A compound: tags until TAG_End. The reader's position is the contract, so an unknown
    /// tag id fails rather than guessing a length.
    fn compound(&mut self) -> Option<Tag> {
        let mut entries = Vec::new();
        loop {
            let id = self.u8()?;
            if id == TAG_END {
                return Some(Tag::Compound(entries));
            }
            let name = self.string()?;
            let tag = self.payload(id)?;
            entries.push((name, tag));
        }
    }

    /// A tag's payload, by id. Compounds and strings are kept; the rest are measured and
    /// dropped.
    fn payload(&mut self, id: u8) -> Option<Tag> {
        match id {
            TAG_BYTE => {
                self.take(1)?;
                Some(Tag::Other)
            }
            TAG_SHORT => {
                self.take(2)?;
                Some(Tag::Other)
            }
            TAG_INT => {
                self.take(4)?;
                Some(Tag::Other)
            }
            TAG_LONG => {
                self.take(8)?;
                Some(Tag::Other)
            }
            TAG_FLOAT => {
                self.take(4)?;
                Some(Tag::Other)
            }
            TAG_DOUBLE => {
                self.take(8)?;
                Some(Tag::Other)
            }
            TAG_BYTE_ARRAY => {
                let length = self.i32()?;
                self.take(usize::try_from(length).ok()?)?;
                Some(Tag::Other)
            }
            TAG_STRING => self.string().map(Tag::String),
            TAG_LIST => {
                // A list is an element id, a length, and that many payloads of that id.
                let element = self.u8()?;
                let length = self.i32()?;
                for _ in 0..length {
                    self.payload(element)?;
                }
                Some(Tag::Other)
            }
            TAG_COMPOUND => self.compound(),
            TAG_INT_ARRAY => {
                let length = self.i32()?;
                self.take(usize::try_from(length).ok()?.checked_mul(4)?)?;
                Some(Tag::Other)
            }
            TAG_LONG_ARRAY => {
                let length = self.i32()?;
                self.take(usize::try_from(length).ok()?.checked_mul(8)?)?;
                Some(Tag::Other)
            }
            // TAG_End inside a payload position has no payload of its own.
            TAG_END => Some(Tag::Other),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A document with `Data { LevelName: <name>, hardcore: 1 }`, hand-assembled so the test
    /// does not depend on the writer it is testing.
    fn document_with(name: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(TAG_COMPOUND); // root
        out.extend_from_slice(&0u16.to_be_bytes()); // root's name: empty
        out.push(TAG_COMPOUND);
        out.extend_from_slice(&4u16.to_be_bytes());
        out.extend_from_slice(b"Data");
        out.push(TAG_STRING);
        out.extend_from_slice(&9u16.to_be_bytes());
        out.extend_from_slice(b"LevelName");
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
        out.push(TAG_BYTE); // a tag the reader must skip by size
        out.extend_from_slice(&8u16.to_be_bytes()); // "hardcore" is eight bytes
        out.extend_from_slice(b"hardcore");
        out.push(1);
        out.push(TAG_END); // ends Data
        out.push(TAG_END); // ends the root
        out
    }

    #[test]
    fn reads_the_name_from_an_uncompressed_document() {
        assert_eq!(
            name_from_level_dat(&document_with("My World")).as_deref(),
            Some("My World")
        );
    }

    #[test]
    fn reads_the_name_through_gzip() {
        let mut encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&document_with("Compressed")).unwrap();
        let gzipped = encoder.finish().unwrap();
        assert!(gzipped.starts_with(&[0x1f, 0x8b]), "a real gzip member");
        assert_eq!(
            name_from_level_dat(&gzipped).as_deref(),
            Some("Compressed")
        );
    }

    #[test]
    fn a_blank_name_is_not_a_name() {
        assert_eq!(name_from_level_dat(&document_with("   ")), None);
    }

    #[test]
    fn a_missing_or_broken_file_is_no_name() {
        assert_eq!(
            name_from_level_dat(&document_with("Half a document")[..12]),
            None,
            "a truncated document is not a name"
        );
        assert_eq!(
            world_name(Path::new("this-folder-does-not-exist")),
            None,
            "no level.dat at all"
        );
    }
}
