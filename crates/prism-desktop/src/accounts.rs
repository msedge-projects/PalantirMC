//! The account store: `<data_root>/accounts.json`.
//!
//! Two kinds of account live here.
//!
//! * **Offline** accounts are a name and nothing else. Their UUID is derived
//!   from the name exactly the way Java's `UUID.nameUUIDFromBytes` derives it
//!   (`md5("OfflinePlayer:" + name)` with the version/variant bits rewritten),
//!   which is what Prism and the vanilla launcher do — so the same name means
//!   the same player in every launcher, and a world started here is not a
//!   stranger to a world started there.
//! * **Microsoft** accounts carry the name and UUID from the Minecraft profile
//!   plus the tokens the login chain produced: a game access token, its expiry,
//!   and the Microsoft refresh token that mints a new one. Storing the refresh
//!   token is what makes sign-in stick across restarts, and the expiry is what
//!   lets [`AccountEntry::needs_refresh`] decide — before a launch, not during
//!   one — that the session has to be renewed.
//!
//! The on-disk shape is `{selected, list[]}`. Fields added after the first
//! release all carry `#[serde(default)]`, so an `accounts.json` written by an
//! earlier build still loads (its entries are simply offline accounts) and a
//! file written by this one remains readable by that build, which ignores the
//! keys it does not know.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How an account authenticates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    /// Name-only account, no network.
    #[default]
    Offline,
    /// A signed-in Microsoft account.
    Msa,
}

impl AccountKind {
    /// Label for the accounts list.
    pub fn label(self) -> &'static str {
        match self {
            AccountKind::Offline => "Offline",
            AccountKind::Msa => "Microsoft",
        }
    }

    /// Whether this kind can join online-mode servers.
    pub fn is_online(self) -> bool {
        matches!(self, AccountKind::Msa)
    }
}

/// One account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountEntry {
    /// Player name.
    #[serde(default)]
    pub username: String,
    /// Account id (32 lowercase hex digits, no dashes).
    #[serde(default)]
    pub uuid: String,
    /// Offline name-only, or a signed-in Microsoft account.
    #[serde(default)]
    pub kind: AccountKind,
    /// Game access token (`auth_access_token`) for a Microsoft account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// Microsoft refresh token, used to mint a new access token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Unix milliseconds at which `access_token` stops working.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<i64>,
    /// Whether the last login found a game entitlement. `None` means the
    /// entitlements call was not answered, which is not the same as "no".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entitled: Option<bool>,
}

/// How long before expiry a Microsoft token is renewed.
///
/// Prism's rule (`MinecraftAccount::shouldRefresh`): a token is refreshed once
/// it would expire within twelve hours, so a session never dies halfway through
/// a play session.
pub const REFRESH_WINDOW_MS: i64 = 12 * 60 * 60 * 1000;

impl AccountEntry {
    /// A new offline account for `username`.
    pub fn offline(username: &str) -> AccountEntry {
        let name = username.trim().to_string();
        AccountEntry {
            uuid: offline_uuid(&name),
            username: name,
            kind: AccountKind::Offline,
            access_token: None,
            refresh_token: None,
            expires_at_ms: None,
            entitled: None,
        }
    }

    /// A signed-in Microsoft account.
    pub fn microsoft(
        username: &str,
        uuid: &str,
        access_token: &str,
        refresh_token: &str,
        expires_at_ms: i64,
        entitled: Option<bool>,
    ) -> AccountEntry {
        AccountEntry {
            username: username.to_string(),
            uuid: uuid.to_lowercase(),
            kind: AccountKind::Msa,
            access_token: Some(access_token.to_string()),
            refresh_token: Some(refresh_token.to_string()),
            expires_at_ms: Some(expires_at_ms),
            entitled,
        }
    }

    /// Whether this is a signed-in Microsoft account.
    pub fn is_microsoft(&self) -> bool {
        self.kind == AccountKind::Msa
    }

    /// Whether the stored Microsoft token has to be renewed before use.
    ///
    /// A Microsoft account with no refresh token always "needs" one: there is
    /// nothing to renew *with*, and the caller's answer is to send the user
    /// through the device-code flow again, which is the correct outcome.
    pub fn needs_refresh(&self, now_ms: i64) -> bool {
        needs_refresh(self.kind, self.refresh_token.as_deref(), self.expires_at_ms, now_ms)
    }

    /// The short line under the name in the accounts list.
    pub fn detail(&self) -> String {
        match self.kind {
            AccountKind::Offline => format!("Offline · {}", self.uuid),
            AccountKind::Msa => {
                let ownership = match self.entitled {
                    Some(true) => "owns Minecraft",
                    Some(false) => "no game entitlement",
                    None => "ownership unknown",
                };
                format!("Microsoft · {ownership} · {}", self.uuid)
            }
        }
    }
}

/// The shared refresh rule, so an account entry and a launch reference cannot
/// disagree about when a session is stale.
///
/// See [`REFRESH_WINDOW_MS`]: a token inside the window (or with no recorded
/// expiry) is renewed, and a Microsoft account with no refresh token always
/// needs the interactive flow again because there is nothing to renew with.
pub fn needs_refresh(
    kind: AccountKind,
    refresh_token: Option<&str>,
    expires_at_ms: Option<i64>,
    now_ms: i64,
) -> bool {
    if !kind.is_online() {
        return false;
    }
    if refresh_token.unwrap_or("").is_empty() {
        return true;
    }
    match expires_at_ms {
        Some(expires_at) => expires_at - now_ms < REFRESH_WINDOW_MS,
        None => true,
    }
}

/// Java's `UUID.nameUUIDFromBytes` over `"OfflinePlayer:<name>"`.
///
/// Prism derives offline UUIDs this way (`MinecraftAccount::uuidFromUsername`),
/// and so does every other launcher, which is why the same name is the same
/// player everywhere. Returns 32 lowercase hex digits without dashes.
pub fn offline_uuid(username: &str) -> String {
    let digest = md5_bytes(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes = digest;
    // Version 3 (name-based, MD5) in the high nibble of byte 6, and the IETF
    // variant in the top bits of byte 8 — the two edits Java's implementation
    // makes after hashing.
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// MD5 of `data`.
///
/// The `md-5` crate rather than a hand-rolled compression function: this digest
/// is a naming scheme, not a security boundary, but a wrong MD5 still means
/// wrong UUIDs, and a tested implementation is the only kind worth shipping.
fn md5_bytes(data: &[u8]) -> [u8; 16] {
    use md5::Digest;
    let mut hasher = md5::Md5::new();
    hasher.update(data);
    hasher.finalize().into()
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

    /// The account with `uuid`, if it exists.
    pub fn account(&self, uuid: &str) -> Option<&AccountEntry> {
        self.list.iter().find(|a| a.uuid == uuid)
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
        let entry = AccountEntry::offline(name);
        let uuid = entry.uuid.clone();
        self.list.push(entry);
        if self.selected.is_none() {
            self.selected = Some(uuid);
        }
        Ok(())
    }

    /// Add (or update) a Microsoft account and select it.
    ///
    /// Signing in again is not a second account: the profile UUID is stable, so
    /// a second sign-in replaces the tokens on the existing entry. The same
    /// method therefore serves both "sign in" and "renew", and the selection
    /// follows the account the user just proved they own.
    pub fn upsert_microsoft(&mut self, entry: AccountEntry) -> Result<(), String> {
        if entry.username.trim().is_empty() || entry.uuid.trim().is_empty() {
            return Err("Microsoft account is missing its profile name or id".to_string());
        }
        let uuid = entry.uuid.clone();
        let mut replaced = false;
        for existing in self.list.iter_mut() {
            if existing.uuid == uuid {
                *existing = entry.clone();
                replaced = true;
                break;
            }
        }
        if !replaced {
            // A leftover offline account with the same name would be confusing
            // next to the real one; the Microsoft entry wins.
            self.list.retain(|account| !(account.username == entry.username
                && account.kind == AccountKind::Offline));
            self.list.push(entry);
        }
        self.selected = Some(uuid);
        Ok(())
    }

    /// Replace the tokens of an existing account (a refresh, not a new login).
    pub fn update_tokens(
        &mut self,
        uuid: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_at_ms: i64,
    ) -> Result<(), String> {
        let account = self
            .list
            .iter_mut()
            .find(|account| account.uuid == uuid)
            .ok_or_else(|| format!("no account with id '{uuid}'"))?;
        account.access_token = Some(access_token.to_string());
        if let Some(refresh) = refresh_token.filter(|token| !token.is_empty()) {
            account.refresh_token = Some(refresh.to_string());
        }
        account.expires_at_ms = Some(expires_at_ms);
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

    fn msa_entry() -> AccountEntry {
        AccountEntry::microsoft(
            "Steve",
            "1A2B3C4D5E6F708192A3B4C5D6E7F809",
            "game-token",
            "refresh-token",
            1_000_000,
            Some(true),
        )
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

    // ---- offline UUIDs ----------------------------------------------------

    #[test]
    fn offline_uuid_matches_javas_name_uuid_from_bytes() {
        // Derived independently (Python's hashlib + the version/variant edit)
        // rather than from this implementation, so the test is a real oracle:
        // java.util.UUID.nameUUIDFromBytes("OfflinePlayer:" + name) for each.
        assert_eq!(offline_uuid("Steve"), "5627dd98e6be3c21b8a8e92344183641");
        assert_eq!(offline_uuid("Player"), "a01e3843e5213998958af459800e4d11");
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
        assert_eq!(offline_uuid("alex"), "bf20048ca55a322ca1005493e7b87286");
    }

    #[test]
    fn offline_uuids_are_stable_and_case_sensitive_like_prism() {
        assert_eq!(offline_uuid("Steve"), offline_uuid("Steve"));
        assert_ne!(offline_uuid("Steve"), offline_uuid("steve"));
        assert_eq!(offline_uuid("Steve").len(), 32);
        assert!(offline_uuid("Steve").chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn a_new_offline_account_gets_the_derived_uuid() {
        let (_dir, path) = tmp_file();
        let mut store = AccountsStore::load(&path);
        store.add("Steve").unwrap();
        assert_eq!(store.list()[0].uuid, offline_uuid("Steve"));
        assert!(!store.list()[0].is_microsoft());
    }

    // ---- Microsoft accounts ----------------------------------------------

    #[test]
    fn a_microsoft_account_round_trips_with_its_tokens() {
        let (_dir, path) = tmp_file();
        let (mut store, _) = AccountsStore::load_with_report(&path);
        store.upsert_microsoft(msa_entry()).unwrap();
        store.save().unwrap();

        let (back, _) = AccountsStore::load_with_report(&path);
        let account = back.selected_account().unwrap();
        assert_eq!(account.username, "Steve");
        assert_eq!(account.uuid, "1a2b3c4d5e6f708192a3b4c5d6e7f809");
        assert_eq!(account.kind, AccountKind::Msa);
        assert_eq!(account.access_token.as_deref(), Some("game-token"));
        assert_eq!(account.refresh_token.as_deref(), Some("refresh-token"));
        assert_eq!(account.expires_at_ms, Some(1_000_000));
        assert_eq!(account.entitled, Some(true));
        assert!(account.detail().contains("owns Minecraft"));
    }

    #[test]
    fn an_accounts_file_from_an_older_build_still_loads() {
        // Exactly what the previous release wrote: name + uuid, nothing else.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        std::fs::write(
            &path,
            br#"{"selected":"abc","list":[{"username":"Steve","uuid":"abc"}]}"#,
        )
        .unwrap();
        let (store, warn) = AccountsStore::load_with_report(&path);
        assert!(warn.is_none());
        let account = store.selected_account().unwrap();
        assert_eq!(account.username, "Steve");
        assert_eq!(account.kind, AccountKind::Offline, "an unknown kind defaults to offline");
        assert!(account.access_token.is_none());
        assert!(!account.needs_refresh(0));
    }

    #[test]
    fn an_offline_account_serializes_without_the_token_keys() {
        let (_dir, path) = tmp_file();
        let mut store = AccountsStore::load(&path);
        store.add("Steve").unwrap();
        store.save().unwrap();
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let entry = &raw["list"][0];
        assert!(entry.get("kind").is_some());
        assert!(entry.get("access_token").is_none(), "no empty token keys");
        assert!(entry.get("refresh_token").is_none());
    }

    #[test]
    fn signing_in_again_updates_the_same_account() {
        let (_dir, path) = tmp_file();
        let mut store = AccountsStore::load(&path);
        store.upsert_microsoft(msa_entry()).unwrap();
        let mut second = msa_entry();
        second.access_token = Some("fresh-token".into());
        second.expires_at_ms = Some(2_000_000);
        store.upsert_microsoft(second).unwrap();

        assert_eq!(store.list().len(), 1, "one profile is one account");
        assert_eq!(store.list()[0].access_token.as_deref(), Some("fresh-token"));
        assert_eq!(store.list()[0].expires_at_ms, Some(2_000_000));
        assert_eq!(store.selected_uuid(), Some("1a2b3c4d5e6f708192a3b4c5d6e7f809"));
    }

    #[test]
    fn signing_in_replaces_a_same_named_offline_account() {
        let (_dir, path) = tmp_file();
        let mut store = AccountsStore::load(&path);
        store.add("Steve").unwrap();
        assert!(store.selected_account().unwrap().is_microsoft() == false);
        store.upsert_microsoft(msa_entry()).unwrap();
        assert_eq!(store.list().len(), 1, "the offline placeholder is gone");
        assert!(store.list()[0].is_microsoft());
        assert_eq!(store.selected_account().unwrap().username, "Steve");
    }

    #[test]
    fn a_microsoft_entry_must_carry_a_profile() {
        let (_dir, path) = tmp_file();
        let mut store = AccountsStore::load(&path);
        let mut broken = msa_entry();
        broken.uuid = String::new();
        assert!(store.upsert_microsoft(broken).is_err());
        assert!(store.list().is_empty());
    }

    #[test]
    fn a_refresh_window_starts_twelve_hours_before_expiry() {
        let now = 1_000_000i64;
        let mut account = msa_entry();
        account.expires_at_ms = Some(now + REFRESH_WINDOW_MS + 1);
        assert!(!account.needs_refresh(now), "still more than twelve hours left");
        account.expires_at_ms = Some(now + REFRESH_WINDOW_MS - 1);
        assert!(account.needs_refresh(now), "inside the window");
        account.expires_at_ms = Some(now - 1);
        assert!(account.needs_refresh(now), "already expired");
        // No expiry recorded at all: nothing to trust, so renew.
        account.expires_at_ms = None;
        assert!(account.needs_refresh(now));
        // Offline accounts never need a network refresh.
        account.kind = AccountKind::Offline;
        assert!(!account.needs_refresh(now));
    }

    #[test]
    fn an_account_without_a_refresh_token_always_needs_signing_in() {
        let mut account = msa_entry();
        account.refresh_token = None;
        assert!(account.needs_refresh(0), "there is nothing to renew with");
        account.refresh_token = Some(String::new());
        assert!(account.needs_refresh(0));
    }

    #[test]
    fn refreshing_tokens_keeps_the_account_and_its_identity() {
        let (_dir, path) = tmp_file();
        let mut store = AccountsStore::load(&path);
        store.upsert_microsoft(msa_entry()).unwrap();
        store
            .update_tokens("1a2b3c4d5e6f708192a3b4c5d6e7f809", "new-token", None, 5_000)
            .unwrap();
        let account = store.account("1a2b3c4d5e6f708192a3b4c5d6e7f809").unwrap();
        assert_eq!(account.access_token.as_deref(), Some("new-token"));
        assert_eq!(account.refresh_token.as_deref(), Some("refresh-token"), "kept when not sent");
        assert_eq!(account.expires_at_ms, Some(5_000));
        assert!(account.entitled == Some(true), "ownership is not lost on a refresh");
        assert!(store.update_tokens("missing", "x", None, 1).is_err());
    }

    #[test]
    fn account_kind_labels_read_as_the_list_shows_them() {
        assert_eq!(AccountKind::Offline.label(), "Offline");
        assert_eq!(AccountKind::Msa.label(), "Microsoft");
        assert!(!AccountKind::Offline.is_online());
        assert!(AccountKind::Msa.is_online());
        assert_eq!(AccountKind::default(), AccountKind::Offline);
    }
}
