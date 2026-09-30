//! The skins the reader has added, in the launcher's own store.
//!
//! The reference keeps this itself: its `minecraft-skins` plugin holds a texture
//! for every skin a reader adds, `helpers/skins.ts` names the operations
//! (`set_custom_skin_order`, `remove_custom_skin`, `save_custom_skin`) and labels
//! each row with a `source` of `'default' | 'custom' | 'custom_external'` against
//! Mojang's own. This launcher has no plugin, so the store is a folder under its
//! **own** directory -- the one [`PalantirPaths::home`] resolves, which is where
//! its preferences already live and which is deliberately not the data root:
//!
//! ```text
//! <home>/skins/index.json     what the reader has, in the reader's order
//! <home>/skins/<key>.png      one 64x64 texture per row, named for its digest
//! ```
//!
//! Three decisions are worth naming, because each is a place this could have been
//! built differently.
//!
//! **A row is keyed by the digest of its pixels.** The reference's key is the
//! plugin's `texture_key`, which for a skin the reader picked is the digest of the
//! normalised texture; computing the same thing here means the same file added
//! twice is one row rather than two, and it means the row's identity survives the
//! file being renamed. It also means the store can find a row without being told
//! which one: [`key_of`] is a function of the bytes alone.
//!
//! **Every write is atomic, and a broken index is read as empty.** The same rule
//! `prefs` follows, for the same reason: a half-written index is read by the next
//! run, and a reader's saved skins are not something to lose to a crash. A file
//! that will not parse is left where it is rather than overwritten -- what is
//! destroyed by a write is always the thing that was already wrong.
//!
//! **This store holds the reader's own skins, and only those.** The reference's
//! list is *three* sources at once, because its plugin answers for the skins it
//! holds and for the ones the account holds; here the account's own skins are read
//! from Minecraft's document every time the page is drawn (G104) and are not copied
//! into this file, so what a row's [`Source`] says is *how it arrived*: a file the
//! reader picked (`custom`) or a texture the account already owned that the reader
//! chose to keep (`custom_external`, the reference's own word for it). The
//! consequence is named where it can be seen: the same skin can be a row here and a
//! row in the account's own list, because this launcher has no way to ask the
//! account's document whether a texture it lists is one this store holds without
//! fetching every texture it lists.
//!
//! The textures are stored **already normalised** -- 64x64, which is what
//! [`crate::skin::prepare`] hands this module -- so a legacy 64x32 file the reader
//! picked is stored in the shape the service takes and a row never has to be padded
//! twice. That is the reference's behaviour too (its `normalize_skin_texture` runs
//! before `save_custom_skin`), and it is what makes [`texture`]'s bytes safe to
//! hand straight to `SkinChange::Upload`.

use std::path::PathBuf;

use palantir_core::paths::PalantirPaths;
use serde::{Deserialize, Serialize};
use sha1::Digest;

/// The folder the store lives in, under the launcher's own directory.
pub const STORE_DIR: &str = "skins";

/// The index inside that folder.
pub const INDEX_FILE: &str = "index.json";

/// Where a row's skin came from.
///
/// The reference's `SkinSource` has three values and this has two, and the missing
/// one is the point: `'default'` is a skin Minecraft ships, which its plugin holds
/// a copy of for its bundles. This launcher has no such bundle, so a row here is
/// always one of the reader's own -- either a file they picked or a texture their
/// account already had.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A file the reader picked, and this launcher uploaded.
    Custom,
    /// A texture the account already owned, kept here so it can be edited.
    CustomExternal,
}

/// `#[allow(dead_code)]` because nothing in the binary asks for the word yet: the
/// index carries it through serde's own rename, and this reads it back for the gate
/// and for a caller that has to *name* a source -- which is the next thing this
/// store wants. Kept beside the enum so the reference's three words and this
/// launcher's two cannot drift (`locale.rs`'s `Direction` is the same arrangement).
#[allow(dead_code)]
impl Source {
    /// The word the index and the page use, which is the reference's own.
    pub const fn as_str(self) -> &'static str {
        match self {
            Source::Custom => "custom",
            Source::CustomExternal => "custom_external",
        }
    }
}

/// One skin the reader has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The digest of the stored texture, which is this row's identity.
    pub key: String,
    /// What the row is called. The reader's own file name when they picked a file,
    /// and the account's own alias when the row was imported.
    pub name: String,
    /// The arm style, in the service's own words (`CLASSIC` or `SLIM`).
    pub variant: String,
    /// The cape this row asks for, by the id the document gave it. Empty when it
    /// asks for none -- and therefore not written, which is `prefs`' rule for a
    /// field whose default is the empty value.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cape: String,
    /// How the row arrived.
    pub source: Source,
    /// The PNG inside the store folder, relative to it: `<key>.png`.
    pub file: String,
}

/// Everything the reader has, in the reader's order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Index {
    /// The stored keys, most recently added first, in the order the reader has put
    /// them in. The reference's `set_custom_skin_order` is this list.
    pub order: Vec<String>,
    /// The rows themselves. Every key in `order` is a row here, and a row whose key
    /// is missing from `order` is drawn after the ones that are -- a partly
    /// reordered file is not a file that loses skins.
    pub skins: Vec<Entry>,
}

impl Index {
    /// Whether this row is known.
    pub fn has(&self, key: &str) -> bool {
        self.skins.iter().any(|entry| entry.key == key)
    }

    /// One row, by key.
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.skins.iter().find(|entry| entry.key == key)
    }

    /// The rows in the reader's order.
    ///
    /// Not `skins` itself: the file's own order is the order rows were written in,
    /// and what the page draws is the order the reader chose.
    pub fn ordered(&self) -> Vec<&Entry> {
        let mut rows: Vec<&Entry> = self.order.iter().filter_map(|key| self.get(key)).collect();
        for entry in &self.skins {
            if !self.order.iter().any(|key| key == &entry.key) {
                rows.push(entry);
            }
        }
        rows
    }

    /// Move a row to the front, keeping every other row in its place.
    fn promote(&mut self, key: &str) {
        self.order.retain(|known| known != key);
        self.order.insert(0, key.to_string());
    }
}

/// The index's path: `<home>/skins/index.json`.
pub fn index_path(home: &PalantirPaths) -> PathBuf {
    home.root.join(STORE_DIR).join(INDEX_FILE)
}

/// The store folder itself.
pub fn dir(home: &PalantirPaths) -> PathBuf {
    home.root.join(STORE_DIR)
}

/// One row's texture: `<home>/skins/<key>.png`.
pub fn texture_path(home: &PalantirPaths, entry: &Entry) -> PathBuf {
    dir(home).join(&entry.file)
}

/// The identity of a texture: the hex SHA-1 of its bytes.
///
/// SHA-1 because the rest of this launcher already speaks it -- Mojang publishes
/// one per library, Modrinth publishes one per file, and both are checked with it
/// -- and because nothing here is a security boundary: the digest's job is to be
/// the same for the same pixels and different for different ones.
pub fn key_of(texture: &[u8]) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(texture);
    format!("{:x}", hasher.finalize())
}

/// Read the index, falling back to *nothing*.
///
/// A missing file is a reader with no saved skins, which is the normal first run;
/// a file that will not parse is one this launcher cannot honour, and it is left in
/// place -- the same rule `prefs` follows, for the same reason.
pub fn load(home: &PalantirPaths) -> Index {
    std::fs::read_to_string(index_path(home))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write the index, atomically.
pub fn save(home: &PalantirPaths, index: &Index) -> Result<(), String> {
    std::fs::create_dir_all(dir(home))
        .map_err(|error| format!("could not make the skin store: {error}"))?;
    let text = serde_json::to_string_pretty(index).map_err(|error| error.to_string())?;
    let mut bytes = text.into_bytes();
    bytes.push(b'\n');
    palantir_core::util::atomic_write(&index_path(home), &bytes)
        .map_err(|error| format!("could not write the skin store: {error}"))
}

/// Read one row's texture back.
pub fn texture(home: &PalantirPaths, entry: &Entry) -> Result<Vec<u8>, String> {
    std::fs::read(texture_path(home, entry))
        .map_err(|error| format!("could not read the stored texture for {}: {error}", entry.name))
}

/// Keep a texture, and remember it.
///
/// `name` is what the row is called, `variant` is the arm style in the service's
/// own words, and `cape` is the cape the row asks for (empty for none). The bytes
/// have to be the 64x64 shape the service takes -- padding a legacy texture is
/// [`crate::skin::prepare`]'s job, one caller up, and a second opinion about the
/// format here would be a second place to be wrong about it.
///
/// A texture that is *already* stored is not stored twice: the row that is already
/// there is brought to the front and takes the arm style and name from this call,
/// because those are the two things a reader can change about a file they are
/// adding again. Nothing is written for it -- the pixels are already on disk under
/// the same name.
pub fn add(
    home: &PalantirPaths,
    texture: &[u8],
    name: &str,
    variant: &str,
    cape: &str,
    source: Source,
) -> Result<Entry, String> {
    let key = key_of(texture);
    let mut index = load(home);
    let entry = Entry {
        key: key.clone(),
        name: name.to_string(),
        variant: variant.to_string(),
        cape: cape.to_string(),
        source,
        file: format!("{key}.png"),
    };
    if index.has(&key) {
        let mut updated = entry;
        if let Some(existing) = index.skins.iter_mut().find(|row| row.key == key) {
            existing.name = updated.name.clone();
            existing.variant = updated.variant.clone();
            updated = existing.clone();
        }
        index.promote(&key);
        save(home, &index)?;
        return Ok(updated);
    }
    std::fs::create_dir_all(dir(home))
        .map_err(|error| format!("could not make the skin store: {error}"))?;
    std::fs::write(texture_path(home, &entry), texture)
        .map_err(|error| format!("could not store the texture for {name}: {error}"))?;
    index.skins.push(entry.clone());
    index.promote(&key);
    save(home, &index)?;
    Ok(entry)
}

/// Forget a row, and its pixels with it -- the reference's `remove_custom_skin`.
///
/// Answers whether there was one: a row already gone is not a failure, it is a
/// press on a page that has not been redrawn yet.
pub fn forget(home: &PalantirPaths, key: &str) -> Result<bool, String> {
    let mut index = load(home);
    let Some(position) = index.skins.iter().position(|row| row.key == key) else {
        return Ok(false);
    };
    let entry = index.skins.remove(position);
    index.order.retain(|known| known != key);
    let path = texture_path(home, &entry);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|error| {
            format!("could not remove the stored texture for {}: {error}", entry.name)
        })?;
    }
    save(home, &index)?;
    Ok(true)
}

/// Write the reader's order -- the reference's `set_custom_skin_order`.
///
/// Keys this store does not hold are ignored, and rows the caller left out keep
/// their place at the end, because the caller is a page drawing a list and a list
/// that arrived short is not an instruction to delete anything.
///
/// `#[allow(dead_code)]` because this launcher has no drag-and-drop list to call it
/// from yet: `Index::order` is what a reorder would write, `add` maintains it by
/// promoting a row to the front and `forget` prunes it, and the reference's own
/// `set_custom_skin_order` is the vocabulary this keeps so the slice that draws the
/// list does not have to re-derive it. `locale.rs`'s `Direction` is the same
/// arrangement, for the same reason.
#[allow(dead_code)]
pub fn reorder(home: &PalantirPaths, ordered: &[String]) -> Result<(), String> {
    let mut index = load(home);
    let mut want: Vec<String> = Vec::with_capacity(index.skins.len());
    for key in ordered {
        if index.has(key) && !want.iter().any(|known| known == key) {
            want.push(key.clone());
        }
    }
    for entry in &index.skins {
        if !want.iter().any(|known| known == &entry.key) {
            want.push(entry.key.clone());
        }
    }
    index.order = want;
    save(home, &index)
}

/// Change what a row asks for: its arm style and its cape.
///
/// The two fields the edit modal can write, and nothing else: a row's name comes
/// from the file it was added from and its pixels are its identity, so a modal that
/// could write either would be a modal that can lose the row's key.
pub fn update(home: &PalantirPaths, key: &str, variant: &str, cape: &str) -> Result<bool, String> {
    let mut index = load(home);
    let Some(entry) = index.skins.iter_mut().find(|row| row.key == key) else {
        return Ok(false);
    };
    entry.variant = variant.to_string();
    entry.cape = cape.to_string();
    save(home, &index)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> (tempfile::TempDir, PalantirPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path());
        (dir, paths)
    }

    /// 64x64 RGBA, one value everywhere: a real texture as far as the store is
    /// concerned, which is a sequence of bytes with a digest.
    fn pixels(byte: u8) -> Vec<u8> {
        vec![byte; 64 * 64 * 4]
    }

    #[test]
    fn nothing_stored_yet_reads_as_nothing_and_a_broken_index_is_left_alone() {
        let (_dir, home) = home();
        assert!(load(&home).ordered().is_empty());
        std::fs::create_dir_all(dir(&home)).unwrap();
        std::fs::write(index_path(&home), b"{ this is not json").unwrap();

        assert!(load(&home).ordered().is_empty());
        // The file that would not parse is still there, and still the reader's.
        assert_eq!(std::fs::read(index_path(&home)).unwrap(), b"{ this is not json");
    }

    #[test]
    fn what_is_added_is_read_back_with_its_arm_style_and_its_pixels() {
        let (_dir, home) = home();
        let entry = add(&home, &pixels(7), "hero.png", "SLIM", "", Source::Custom).unwrap();

        let index = load(&home);
        assert_eq!(index.ordered().len(), 1);
        assert_eq!(index.ordered()[0].name, "hero.png");
        assert_eq!(index.ordered()[0].variant, "SLIM");
        assert_eq!(index.ordered()[0].source, Source::Custom);
        assert_eq!(index.ordered()[0].source.as_str(), "custom");
        assert_eq!(index.ordered()[0].key, key_of(&pixels(7)));
        assert_eq!(super::texture(&home, &entry).unwrap(), pixels(7));
    }

    #[test]
    fn adding_the_same_pixels_twice_is_one_row_and_brings_it_to_the_front() {
        let (_dir, home) = home();
        add(&home, &pixels(1), "first.png", "CLASSIC", "", Source::Custom).unwrap();
        add(&home, &pixels(2), "second.png", "CLASSIC", "", Source::Custom).unwrap();
        add(&home, &pixels(1), "first-again.png", "SLIM", "", Source::Custom).unwrap();

        let index = load(&home);
        assert_eq!(index.ordered().len(), 2);
        assert_eq!(index.ordered()[0].name, "first-again.png");
        assert_eq!(index.ordered()[0].variant, "SLIM");
        assert_eq!(index.ordered()[1].name, "second.png");
    }

    #[test]
    fn forgetting_removes_the_row_and_its_pixels() {
        let (_dir, home) = home();
        let entry = add(&home, &pixels(3), "gone.png", "CLASSIC", "", Source::Custom).unwrap();
        let path = texture_path(&home, &entry);

        assert_eq!(forget(&home, &entry.key), Ok(true));
        assert!(load(&home).ordered().is_empty());
        assert!(!path.exists(), "the pixels went with the row");
        assert_eq!(forget(&home, &entry.key), Ok(false), "twice is not a failure");
    }

    #[test]
    fn a_reorder_keeps_the_rows_it_was_not_told_about() {
        let (_dir, home) = home();
        let one = add(&home, &pixels(1), "one.png", "CLASSIC", "", Source::Custom).unwrap();
        let two = add(&home, &pixels(2), "two.png", "CLASSIC", "", Source::Custom).unwrap();
        let three = add(&home, &pixels(3), "three.png", "CLASSIC", "", Source::Custom).unwrap();

        // A list that names two rows, in the other order, and a key nobody has.
        reorder(&home, &[two.key.clone(), "not-ours".to_string(), one.key.clone()]).unwrap();
        let names: Vec<String> = load(&home).ordered().iter().map(|row| row.name.clone()).collect();
        assert_eq!(names, vec!["two.png", "one.png", "three.png"]);
        assert_eq!(load(&home).ordered()[2].key, three.key);
    }

    #[test]
    fn what_the_modal_writes_is_what_the_next_read_draws() {
        let (_dir, home) = home();
        let entry = add(&home, &pixels(9), "edit.png", "CLASSIC", "", Source::CustomExternal).unwrap();

        assert_eq!(update(&home, &entry.key, "SLIM", "cape-id"), Ok(true));
        let index = load(&home);
        let row = &index.ordered()[0];
        assert_eq!(row.variant, "SLIM");
        assert_eq!(row.cape, "cape-id");
        assert_eq!(row.source, Source::CustomExternal);
        assert_eq!(row.source.as_str(), "custom_external");
        assert_eq!(update(&home, "not-a-key", "SLIM", ""), Ok(false));
    }

    #[test]
    fn a_row_with_no_cape_writes_no_cape_and_a_row_that_asks_for_one_names_it() {
        let (_dir, home) = home();
        add(&home, &pixels(4), "plain.png", "CLASSIC", "", Source::Custom).unwrap();
        let text = std::fs::read_to_string(index_path(&home)).unwrap();
        assert!(!text.contains("cape"), "{text}");

        let entry = add(&home, &pixels(5), "wants.png", "CLASSIC", "Migrator", Source::Custom).unwrap();
        assert_eq!(entry.cape, "Migrator");
        assert!(std::fs::read_to_string(index_path(&home)).unwrap().contains("Migrator"));
    }
}
