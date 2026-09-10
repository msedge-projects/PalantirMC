//! Tiny local offline-accounts store.
//!
//! The desktop GUI keeps `<data_root>/accounts.json` shaped as
//! `{selected, list[{username, uuid}]}` where `selected` is the uuid of the
//! chosen account (if any). This is intentionally minimal: offline accounts
//! only. Microsoft login is out of scope and the UI says so honestly.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One offline account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountEntry {
    /// Player name.
    #[serde(default)]
    pub username: String,
    /// Account id (32 lowercase hex digits, no dashes).
    #[serde(default)]
    pub uuid: String,
}

/// On-disk document shape.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
struct AccountsDoc {
    /// Uuid of the selected account, if any.
    #[serde(default)]
    selected: Option<String>,
    /// Known accounts.
    #[serde(default)]
    list: Vec<AccountEntry>,
}

/// In-memory store bound to one `accounts.json` path.
#[derive(Debug, Clone)]
pub struct AccountsStore {
    path: PathBuf,
    selected: Option<String>,
    list: Vec<AccountEntry>,
}

impl AccountsStore {
    /// Load from `path`. A missing file yields an empty store without a
    /// warning; a corrupt file yields an empty store plus a warning message
    /// (never an error: the launcher must keep working).
    pub fn load_with_report(path: &Path) -> (Self, Option<String>) {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => {
                return (
                    AccountsStore { path: path.to_path_buf(), selected: None, list: Vec::new() },
                    None,
                );
            }
        };
        match serde_json::from_slice::<AccountsDoc>(&bytes) {
            Ok(doc) => (
                AccountsStore { path: path.to_path_buf(), selected: doc.selected, list: doc.list },
                None,
            ),
            Err(e) => (
                AccountsStore { path: path.to_path_buf(), selected: None, list: Vec::new() },
                Some(format!("accounts file is corrupt, starting empty: {e}")),
            ),
        }
    }

    /// Load from `path`, discarding any warning.
    #[allow(dead_code)]
    pub fn load(path: &Path) -> Self {
        Self::load_with_report(path).0
    }

    /// File this store is bound to.
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// All accounts in file order.
    pub fn list(&self) -> &[AccountEntry] {
        &self.list
    }

    /// Uuid of the selected account, if any.
    pub fn selected_uuid(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// The selected account, if it still exists.
    pub fn selected_account(&self) -> Option<&AccountEntry> {
        match self.selected.as_deref() {
            Some(uuid) => self.list.iter().find(|a| a.uuid == uuid),
            None => None,
        }
    }

    /// Persist to disk (creating parent directories as needed).
    pub fn save(&self) -> Result<(), String> {
        let doc = AccountsDoc { selected: self.selected.clone(), list: self.list.clone() };
        let text = match serde_json::to_string_pretty(&doc) {
            Ok(text) => text,
            Err(e) => return Err(format!("serializing accounts: {e}")),
        };
        if let Some(parent) = self.path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(format!("creating accounts dir: {e}"));
            }
        }
        match std::fs::write(&self.path, text) {
            Ok(()) => Ok(()),
            Err(e) => Err(format!("writing accounts file: {e}")),
        }
    }

    /// Add an offline account. Names must be non-blank and unique.
    pub fn add(&mut self, username: &str) -> Result<(), String> {
        let name = username.trim();
        if name.is_empty() {
            return Err("username is empty".to_string());
        }
        if self.list.iter().any(|a| a.username == name) {
            return Err(format!("account '{name}' already exists"));
        }
        let uuid = uuid::Uuid::new_v4().simple().to_string();
        self.list.push(AccountEntry { username: name.to_string(), uuid: uuid.clone() });
        if self.selected.is_none() {
            self.selected = Some(uuid);
        }
        Ok(())
    }

    /// Select the account with `uuid`.
    pub fn select(&mut self, uuid: &str) -> Result<(), String> {
        if self.list.iter().any(|a| a.uuid == uuid) {
            self.selected = Some(uuid.to_string());
            Ok(())
        } else {
            Err(format!("no account with id '{uuid}'"))
        }
    }

    /// Remove the account with `uuid` (clearing the selection if needed).
    /// Returns whether anything was removed.
    pub fn remove(&mut self, uuid: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|a| a.uuid != uuid);
        let removed = self.list.len() != before;
        if removed && self.selected.as_deref() == Some(uuid) {
            self.selected = None;
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("accounts.json");
        (dir, path)
    }

    #[test]
    fn round_trip_preserves_selection_and_order() {
        let (_dir, path) = tmp_file();
        let (mut store, warn) = AccountsStore::load_with_report(&path);
        assert!(warn.is_none());
        assert!(store.list().is_empty());

        store.add("Steve").unwrap();
        store.add("Alex").unwrap();
        let alex = store.list().iter().find(|a| a.username == "Alex").unwrap().uuid.clone();
        // First add auto-selects.
        assert_eq!(store.selected_account().map(|a| a.username.as_str()), Some("Steve"));
        store.select(&alex).unwrap();
        store.save().unwrap();

        let (back, warn) = AccountsStore::load_with_report(&path);
        assert!(warn.is_none());
        assert_eq!(back.list().len(), 2);
        assert_eq!(back.list()[0].username, "Steve");
        assert_eq!(back.selected_uuid(), Some(alex.as_str()));
        assert_eq!(back.selected_account().map(|a| a.username.as_str()), Some("Alex"));

        // Raw document has the required shape.
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(raw.get("selected").is_some());
        assert_eq!(raw["list"].as_array().unwrap().len(), 2);
        assert!(raw["list"][0].get("username").is_some());
        assert!(raw["list"][0].get("uuid").is_some());
    }

    #[test]
    fn corrupt_file_starts_empty_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        std::fs::write(&path, b"{not json").unwrap();
        let (store, warn) = AccountsStore::load_with_report(&path);
        assert!(store.list().is_empty());
        assert!(warn.is_some());
    }

    #[test]
    fn add_rejects_blanks_and_duplicates() {
        let (_dir, path) = tmp_file();
        let mut store = AccountsStore::load(&path);
        assert!(store.add("   ").is_err());
        assert!(store.add("Steve").is_ok());
        assert_eq!(store.list()[0].uuid.len(), 32);
        assert!(store.add("Steve").is_err());
        assert!(store.select("nope").is_err());
    }

    #[test]
    fn remove_clears_selection_only_when_needed() {
        let (_dir, path) = tmp_file();
        let mut store = AccountsStore::load(&path);
        store.add("Steve").unwrap();
        store.add("Alex").unwrap();
        let steve = store.list()[0].uuid.clone();
        let alex = store.list()[1].uuid.clone();
        assert!(!store.remove("missing"));
        assert!(store.remove(&alex));
        // Selection (Steve) untouched.
        assert_eq!(store.selected_uuid(), Some(steve.as_str()));
        assert!(store.remove(&steve));
        assert!(store.selected_uuid().is_none());
        assert!(store.list().is_empty());
    }
}
