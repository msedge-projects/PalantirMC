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
//! **The file is the other launcher's, not ours.** It sits at the data root
//! beside the instances, and Prism reads it on every start, so this launcher
//! reads *and writes* Prism's shape — `formatVersion` 3:
//!
//! ```json
//! { "accounts": [ { "type": "MSA", "active": true,
//!                   "profile": { "id": "<32 hex>", "name": "Steve" },
//!                   "ygg": { "token": "<game token>", "exp": 1789405992 },
//!                   "msa": { "refresh_token": "…" },
//!                   "entitlement": { "ownsMinecraft": true, "canPlayMinecraft": true } } ],
//!   "formatVersion": 3 }
//! ```
//!
//! Two reasons, and the second is the serious one: an account already in that
//! file signs the user in on the first run, which is most of what makes adopting
//! an existing install worth anything — and a launcher that wrote *its own*
//! shape over that file would sign the user out of the other launcher and drop
//! the token chain it needs. So the parsed document is kept whole and only the
//! fields this launcher owns are patched, which leaves capes, skins, the Xbox
//! chain and the per-account client id exactly as they were.
//!
//! A file written by an earlier build of *this* launcher (`{selected, list[]}`)
//! still loads — its entries come back as offline accounts, tokens and all — and
//! is rewritten in the shared shape, so nobody loses a sign-in to the change.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
    #[cfg(test)]
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
    #[cfg(test)]
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
    #[cfg(test)]
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
    #[cfg(test)]
    pub fn needs_refresh(&self, now_ms: i64) -> bool {
        needs_refresh(self.kind, self.refresh_token.as_deref(), self.expires_at_ms, now_ms)
    }

    /// The short line under the name in the accounts list.
    #[cfg(test)]
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

/// The `formatVersion` this launcher writes: the other launcher's MSA-era file.
pub const ACCOUNTS_FORMAT_VERSION: i64 = 3;

/// A fresh document in the shared shape.
fn empty_document() -> Value {
    json!({ "accounts": [], "formatVersion": ACCOUNTS_FORMAT_VERSION })
}

/// One account out of the file, whichever of the two shapes it came from.
///
/// Returns `None` for an object with no name at all: there is nothing to show
/// and nothing to sign in as, and inventing one would put a blank row in the
/// accounts list.
fn entry_from_object(object: &Value) -> Option<AccountEntry> {
    let text = |value: &Value| {
        value.as_str().map(str::to_string).filter(|text| !text.trim().is_empty())
    };
    let kind = match object["type"].as_str() {
        Some(kind) if kind.eq_ignore_ascii_case("msa") => AccountKind::Msa,
        // The shape this launcher wrote before it read the shared one.
        Some(kind) if kind.eq_ignore_ascii_case("offline") => AccountKind::Offline,
        // A file carrying the token blocks but no usable `type` is an MSA
        // account as far as anything here is concerned: only an online account
        // has an `msa`/`ygg` block to carry.
        _ if object.get("msa").is_some() || object.get("ygg").is_some() => AccountKind::Msa,
        _ => AccountKind::Offline,
    };
    let kind = match object["kind"].as_str() {
        Some(legacy) if legacy.eq_ignore_ascii_case("msa") => AccountKind::Msa,
        Some(legacy) if legacy.eq_ignore_ascii_case("offline") => AccountKind::Offline,
        _ => kind,
    };
    let username = text(&object["profile"]["name"])
        .or_else(|| text(&object["ygg"]["extra"]["userName"]))
        .or_else(|| text(&object["username"]))?;
    // `profile.id` is the profile UUID as 32 hex digits with no dashes, which is
    // the form this launcher uses everywhere else.
    let uuid = text(&object["profile"]["id"])
        .or_else(|| text(&object["uuid"]))
        .unwrap_or_default();
    Some(AccountEntry {
        username,
        uuid: uuid.to_lowercase(),
        kind,
        access_token: text(&object["ygg"]["token"]).or_else(|| text(&object["access_token"])),
        refresh_token: text(&object["msa"]["refresh_token"])
            .or_else(|| text(&object["refresh_token"])),
        // `ygg.exp` is in **seconds**; every expiry this launcher holds is in
        // milliseconds.
        expires_at_ms: object["ygg"]["exp"]
            .as_i64()
            .map(|seconds| seconds.saturating_mul(1000))
            .or_else(|| object["expires_at_ms"].as_i64()),
        entitled: object["entitlement"]["ownsMinecraft"]
            .as_bool()
            .or_else(|| object["entitled"].as_bool()),
    })
}

/// The accounts in a parsed shared document, and which one is active.
fn entries_from_document(document: &Value) -> (Option<String>, Vec<AccountEntry>) {
    let mut list: Vec<AccountEntry> = Vec::new();
    let mut selected: Option<String> = None;
    for object in document["accounts"].as_array().into_iter().flatten() {
        let Some(entry) = entry_from_object(object) else {
            continue;
        };
        if object["active"].as_bool().unwrap_or(false) && selected.is_none() {
            selected = Some(entry.uuid.clone());
        }
        list.push(entry);
    }
    (selected, list)
}

/// The accounts in the `{selected, list[]}` document an earlier build of *this*
/// launcher wrote. Read, never written.
fn entries_from_legacy_document(document: &Value) -> (Option<String>, Vec<AccountEntry>) {
    let selected = document["selected"].as_str().map(str::to_string);
    let list: Vec<AccountEntry> = document["list"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(entry_from_object)
        .collect();
    (selected, list)
}

/// Whether an object in the file is `entry`.
///
/// The profile id decides; the name is the fallback, for an account the user
/// signed into a build that wrote no id.
fn same_account(object: &Value, entry: &AccountEntry) -> bool {
    let id = object["profile"]["id"].as_str().or_else(|| object["uuid"].as_str());
    if let Some(id) = id {
        if id.eq_ignore_ascii_case(&entry.uuid) {
            return true;
        }
    }
    object["profile"]["name"].as_str().or_else(|| object["username"].as_str())
        == Some(entry.username.as_str())
}

/// Write the fields this launcher owns onto an account object, in place.
///
/// Everything else in the object is left exactly as it was — that is the whole
/// point: the object belongs to the other launcher, and this one is only
/// updating the tokens it just minted.
fn write_entry(object: &mut Value, entry: &AccountEntry, active: bool) {
    let Some(map) = object.as_object_mut() else {
        return;
    };
    map.insert(
        "type".to_string(),
        json!(if entry.is_microsoft() { "MSA" } else { "Offline" }),
    );
    map.insert("active".to_string(), json!(active));

    // `profile` is patched rather than replaced, so a cape, a skin and its
    // `data` blob survive.
    {
        let profile = map.entry("profile".to_string()).or_insert_with(|| json!({}));
        if !profile.is_object() {
            *profile = json!({});
        }
        if let Some(profile) = profile.as_object_mut() {
            profile.insert("id".to_string(), json!(entry.uuid));
            profile.insert("name".to_string(), json!(entry.username));
        }
    }

    match entry.access_token.as_deref() {
        Some(token) => {
            let ygg = map.entry("ygg".to_string()).or_insert_with(|| json!({}));
            if !ygg.is_object() {
                *ygg = json!({});
            }
            if let Some(ygg) = ygg.as_object_mut() {
                ygg.insert("token".to_string(), json!(token));
                match entry.expires_at_ms {
                    // Seconds on disk, milliseconds here: the file's shape is the
                    // other launcher's and is not negotiable.
                    Some(millis) => {
                        ygg.insert("exp".to_string(), json!(millis / 1000));
                    }
                    None => {
                        ygg.remove("exp");
                    }
                }
            }
        }
        None => {
            map.remove("ygg");
        }
    }

    if let Some(refresh) = entry.refresh_token.as_deref() {
        let msa = map.entry("msa".to_string()).or_insert_with(|| json!({}));
        if !msa.is_object() {
            *msa = json!({});
        }
        if let Some(msa) = msa.as_object_mut() {
            msa.insert("refresh_token".to_string(), json!(refresh));
        }
        // Which Azure application the refresh token belongs to is what the
        // launcher reading this needs in order to renew it. The tokens this one
        // mints come from the same public client id the reference uses, so that
        // is what is recorded — and an account that already names one keeps it.
        map.entry("msa-client-id".to_string())
            .or_insert_with(|| json!(palantir_net::DEFAULT_MICROSOFT_CLIENT_ID));
    }

    if entry.is_microsoft() {
        match entry.entitled {
            Some(entitled) => {
                map.insert(
                    "entitlement".to_string(),
                    json!({ "ownsMinecraft": entitled, "canPlayMinecraft": entitled }),
                );
            }
            // Not answered is not "no": saying it here would tell the other
            // launcher the user does not own the game.
            None => {
                map.remove("entitlement");
            }
        }
    } else {
        map.remove("entitlement");
    }

    // Keys this launcher's own earlier shape used, so a rewritten file does not
    // carry two names for one thing.
    for legacy in [
        "username",
        "uuid",
        "kind",
        "access_token",
        "refresh_token",
        "expires_at_ms",
        "entitled",
    ] {
        map.remove(legacy);
    }
}

/// In-memory store bound to one `accounts.json` path.
#[derive(Debug, Clone)]
pub struct AccountsStore {
    path: PathBuf,
    selected: Option<String>,
    list: Vec<AccountEntry>,
    /// The file as it was parsed, so a save keeps every field this launcher does
    /// not model.
    document: Value,
}

impl AccountsStore {
    /// Load from `path`. A missing file yields an empty store without a
    /// warning; a corrupt file yields an empty store plus a warning message
    /// (never an error: the launcher must keep working).
    pub fn load_with_report(path: &Path) -> (Self, Option<String>) {
        let empty = |document: Value| AccountsStore {
            path: path.to_path_buf(),
            selected: None,
            list: Vec::new(),
            document,
        };
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => return (empty(empty_document()), None),
        };
        let parsed: Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(error) => {
                return (
                    empty(empty_document()),
                    Some(format!("accounts file is corrupt, starting empty: {error}")),
                )
            }
        };
        if parsed.get("accounts").is_some() || parsed.get("formatVersion").is_some() {
            let (selected, list) = entries_from_document(&parsed);
            (
                AccountsStore { path: path.to_path_buf(), selected, list, document: parsed },
                None,
            )
        } else if parsed.get("list").is_some() {
            // An earlier build of this launcher. Its accounts are read, and the
            // document starts empty so the next save writes the shared shape.
            let (selected, list) = entries_from_legacy_document(&parsed);
            (
                AccountsStore {
                    path: path.to_path_buf(),
                    selected,
                    list,
                    document: empty_document(),
                },
                None,
            )
        } else {
            (
                empty(empty_document()),
                Some("accounts file does not look like an accounts file, starting empty".to_string()),
            )
        }
    }

    /// Load from `path`, discarding any warning.
    ///
    /// Test-only: every caller in the shell needs the warning, because a file
    /// it could not parse is a thing the user has to be told about.
    #[cfg(test)]
    pub fn load(path: &Path) -> Self {
        Self::load_with_report(path).0
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
    ///
    /// Test-only: the shell reaches an account through
    /// [`Self::selected_account`], because the file's own selection is the one
    /// that decides which account a launch signs in as.
    #[cfg(test)]
    pub fn account(&self, uuid: &str) -> Option<&AccountEntry> {
        self.list.iter().find(|a| a.uuid == uuid)
    }

    /// Persist to disk in the shared shape (creating parent directories as
    /// needed).
    ///
    /// Every account object already in the file is carried over field for field
    /// and only what this launcher owns is written onto it, so signing in here
    /// does not sign the user out there.
    pub fn save(&self) -> Result<(), String> {
        let mut document = if self.document.is_object() {
            self.document.clone()
        } else {
            empty_document()
        };
        let existing: Vec<Value> = document["accounts"].as_array().cloned().unwrap_or_default();
        let mut accounts: Vec<Value> = Vec::with_capacity(self.list.len());
        for entry in &self.list {
            let mut object = existing
                .iter()
                .find(|object| same_account(object, entry))
                .cloned()
                .unwrap_or_else(|| json!({}));
            if !object.is_object() {
                object = json!({});
            }
            write_entry(&mut object, entry, self.selected.as_deref() == Some(entry.uuid.as_str()));
            accounts.push(object);
        }
        document["accounts"] = Value::Array(accounts);
        document["formatVersion"] = json!(ACCOUNTS_FORMAT_VERSION);
        let text = match serde_json::to_string_pretty(&document) {
            Ok(text) => text,
            Err(error) => return Err(format!("serializing accounts: {error}")),
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
    #[cfg(test)]
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
    #[cfg(test)]
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

        // The raw document is the *shared* shape, and says which account is
        // active the way the other launcher reads it.
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["formatVersion"], serde_json::json!(ACCOUNTS_FORMAT_VERSION));
        assert_eq!(raw["accounts"].as_array().unwrap().len(), 2);
        assert_eq!(raw["accounts"][0]["profile"]["name"], serde_json::json!("Steve"));
        assert_eq!(raw["accounts"][1]["profile"]["name"], serde_json::json!("Alex"));
        assert_eq!(raw["accounts"][0]["active"], serde_json::json!(false));
        assert_eq!(raw["accounts"][1]["active"], serde_json::json!(true));
        assert_eq!(raw["accounts"][1]["type"], serde_json::json!("Offline"));
        assert!(
            raw["accounts"][1]["profile"]["id"].as_str().is_some(),
            "an account is named by its profile id: {raw}"
        );
    }

    /// A file in the shape the other launcher writes, including the parts this
    /// launcher does not model.
    fn prism_accounts_file() -> serde_json::Value {
        serde_json::json!({
            "accounts": [
                {
                    "active": true,
                    "entitlement": {"canPlayMinecraft": true, "ownsMinecraft": true},
                    "msa": {
                        "exp": 1789323188,
                        "extra": {"ext_expires_in": 3599},
                        "iat": 1789319589,
                        "refresh_token": "M.C555_refresh"
                    },
                    "msa-client-id": "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb",
                    "profile": {
                        "cape": "b9d4f2e0-6109-43f7-97aa-84250ce3c1dd",
                        "id": "119bed4a98cd44b399434000dacd244c",
                        "name": "WSedge",
                        "skin": {"id": "s1", "url": "http://textures/skin", "variant": "classic"}
                    },
                    "type": "MSA",
                    "xrp-mc": {"exp": 1789377191, "iat": 1789319591, "token": "xbox-token"},
                    "ygg": {"exp": 1789405992, "iat": 1789319592, "token": "game-token"}
                },
                {
                    "active": false,
                    "profile": {"id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "name": "Second"},
                    "type": "Offline"
                }
            ],
            "formatVersion": 3
        })
    }

    #[test]
    fn an_account_the_other_launcher_saved_signs_the_user_in_here() {
        // The whole point of sharing the file: somebody who already has an
        // account is signed in on the first run of this launcher, with the
        // Minecraft token and the refresh token and the expiry in seconds the
        // other launcher wrote.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        std::fs::write(&path, prism_accounts_file().to_string()).unwrap();

        let (store, warn) = AccountsStore::load_with_report(&path);
        assert!(warn.is_none(), "{warn:?}");
        assert_eq!(store.list().len(), 2);
        let account = store.selected_account().expect("the active account is selected");
        assert_eq!(account.username, "WSedge");
        assert_eq!(account.uuid, "119bed4a98cd44b399434000dacd244c");
        assert!(account.is_microsoft());
        assert_eq!(account.access_token.as_deref(), Some("game-token"));
        assert_eq!(account.refresh_token.as_deref(), Some("M.C555_refresh"));
        assert_eq!(account.expires_at_ms, Some(1_789_405_992_000), "seconds became milliseconds");
        assert_eq!(account.entitled, Some(true));
        // `active` is the selection, not `active` plus a separate key.
        assert_eq!(store.selected_uuid(), Some("119bed4a98cd44b399434000dacd244c"));
        assert!(!store.list()[1].is_microsoft());
        // The expiry came out of the file in seconds and is the renewal rule's
        // input: outside the twelve-hour window nothing is renewed, inside it is.
        let expiry = 1_789_405_992_000;
        assert!(
            !account.needs_refresh(expiry - REFRESH_WINDOW_MS - 1),
            "a session with more than twelve hours left is used as it is"
        );
        assert!(account.needs_refresh(expiry), "and one that has run out is renewed");
    }

    #[test]
    fn saving_keeps_every_field_this_launcher_does_not_model() {
        // The failure this prevents is the serious one: rewriting the file into
        // this launcher's own shape signs the user out of the *other* launcher
        // and throws away the token chain it needs to get back in.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        std::fs::write(&path, prism_accounts_file().to_string()).unwrap();

        let (mut store, _) = AccountsStore::load_with_report(&path);
        store
            .update_tokens("119bed4a98cd44b399434000dacd244c", "new-game-token", None, 1_800_000_000_000)
            .unwrap();
        store.save().unwrap();

        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let first = &raw["accounts"][0];
        assert_eq!(first["ygg"]["token"], serde_json::json!("new-game-token"));
        assert_eq!(first["ygg"]["exp"], serde_json::json!(1_800_000_000));
        // Untouched, field for field.
        assert_eq!(first["xrp-mc"]["token"], serde_json::json!("xbox-token"));
        assert_eq!(first["msa-client-id"], serde_json::json!("c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb"));
        assert_eq!(first["profile"]["cape"], serde_json::json!("b9d4f2e0-6109-43f7-97aa-84250ce3c1dd"));
        assert_eq!(first["profile"]["skin"]["url"], serde_json::json!("http://textures/skin"));
        assert_eq!(first["msa"]["extra"]["ext_expires_in"], serde_json::json!(3599));
        assert_eq!(first["entitlement"]["ownsMinecraft"], serde_json::json!(true));
        // The second account is still there, still not active.
        assert_eq!(raw["accounts"][1]["profile"]["name"], serde_json::json!("Second"));
        assert_eq!(raw["accounts"][1]["active"], serde_json::json!(false));
        // And it reads back the same way.
        let (back, _) = AccountsStore::load_with_report(&path);
        assert_eq!(back.list().len(), 2);
        assert_eq!(back.selected_account().unwrap().access_token.as_deref(), Some("new-game-token"));
    }

    #[test]
    fn signing_in_here_does_not_erase_an_account_saved_there() {
        // A second Microsoft account added by this launcher must not remove the
        // one the other launcher saved, and both must be in the file afterwards.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        std::fs::write(&path, prism_accounts_file().to_string()).unwrap();
        let (mut store, _) = AccountsStore::load_with_report(&path);
        store
            .upsert_microsoft(AccountEntry::microsoft(
                "NewPlayer",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "token-b",
                "refresh-b",
                1_900_000_000_000,
                Some(true),
            ))
            .unwrap();
        store.save().unwrap();

        let (back, _) = AccountsStore::load_with_report(&path);
        let mut names: Vec<&str> = back.list().iter().map(|a| a.username.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["NewPlayer", "Second", "WSedge"]);
        assert_eq!(back.selected_uuid(), Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        // The one that was active before is still an account, now inactive.
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["accounts"][0]["active"], serde_json::json!(false));
        assert_eq!(raw["accounts"][0]["msa"]["refresh_token"], serde_json::json!("M.C555_refresh"));
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
        let entry = &raw["accounts"][0];
        assert_eq!(entry["type"], serde_json::json!("Offline"));
        assert_eq!(entry["active"], serde_json::json!(true));
        assert!(entry.get("ygg").is_none(), "no token block for a name-only account");
        assert!(entry.get("msa").is_none());
        assert!(entry.get("entitlement").is_none(), "and nothing to claim about owning the game");
        // None of this launcher's own old keys leak into the shared shape.
        for legacy in ["kind", "access_token", "refresh_token", "expires_at_ms", "entitled"] {
            assert!(entry.get(legacy).is_none(), "{legacy} should not be written");
        }
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
