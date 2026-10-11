use palantir_core::paths::DataRoot;
use palantir_net::download::sha1_bytes;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Loader {
    #[default]
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}
impl Loader {
    pub const ALL: [Self; 5] = [
        Self::Vanilla,
        Self::Fabric,
        Self::Quilt,
        Self::Forge,
        Self::NeoForge,
    ];
    pub fn desktop_supported(self) -> bool {
        matches!(self, Self::Vanilla | Self::Fabric | Self::Quilt)
    }
}
impl fmt::Display for Loader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Vanilla => "Vanilla",
                Self::Fabric => "Fabric",
                Self::Quilt => "Quilt",
                Self::Forge => "Forge",
                Self::NeoForge => "NeoForge",
            }
        )
    }
}
impl Loader {
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Vanilla => "minecraft",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub game_version: String,
    pub loader: Loader,
    pub installed_version: Option<String>,
    #[serde(default)]
    pub loader_version: Option<String>,
}

/// One local identity for offline play. A launcher holds more than one because
/// a shared machine has more than one player, and having to rename the single
/// offline name means a world is always owned by whoever renamed it last.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OfflineAccount {
    /// Stable key, derived from the name when the account is added. The visible
    /// name carries the meaning; the key only has to survive a list edit.
    pub id: String,
    pub name: String,
}
impl OfflineAccount {
    pub fn new(name: &str) -> Self {
        Self {
            id: offline_uuid(name),
            name: name.into(),
        }
    }
    pub fn uuid(&self) -> String {
        offline_uuid(&self.name)
    }
}
/// The launcher's offline identity for a player name: the derivation the single
/// offline player has always used, which is a SHA-1 over a launcher-owned
/// domain string formatted 8-4-4-4-12.
///
/// It is deliberately *not* normalised with version/variant nibbles and
/// deliberately not the vanilla `OfflinePlayer:` scheme. Either change would
/// give an existing name a different identifier, so a world or server that
/// already knows the old one would stop recognising the player -- for a cosmetic
/// gain in a string that is labelled offline everywhere it appears.
pub fn offline_uuid(name: &str) -> String {
    let digest = sha1_bytes(format!("PalantirOffline:{name}").as_bytes());
    format!(
        "{}-{}-{}-{}-{}",
        &digest[..8],
        &digest[8..12],
        &digest[12..16],
        &digest[16..20],
        &digest[20..32]
    )
}
/// The one name rule every offline identity shares: the game's own 1–16
/// character alphanumeric/underscore limit, refused at the door so a bad name
/// cannot reach a launch command or a stored descriptor.
pub fn validate_player_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 16
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err("Player name must contain 1–16 letters, digits, or underscores.".into());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub player_name: String,
    pub java: String,
    pub memory_mb: u32,
    pub language: String,
    pub confirm_delete: bool,
    pub accent: Accent,
    pub scale: Scale,
    pub show_activity: bool,
    pub reduce_motion: bool,
    pub remember_page: bool,
    pub last_page: String,
    /// Local identities beyond the legacy single name; absent means none saved.
    #[serde(default)]
    pub offline_accounts: Vec<OfflineAccount>,
    /// The selected offline account's id. `None` means use the fallback name.
    #[serde(default)]
    pub active_account: Option<String>,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Accent {
    #[default]
    Copper,
    Glacier,
    Lavender,
}
impl Accent {
    pub const ALL: [Self; 3] = [Self::Copper, Self::Glacier, Self::Lavender];
}
impl fmt::Display for Accent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Copper => "Copper",
                Self::Glacier => "Glacier",
                Self::Lavender => "Lavender",
            }
        )
    }
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Scale {
    Compact,
    #[default]
    Comfortable,
    Large,
}
impl Scale {
    pub const ALL: [Self; 3] = [Self::Compact, Self::Comfortable, Self::Large];
    pub fn factor(self) -> f32 {
        match self {
            Self::Compact => 0.9,
            Self::Comfortable => 1.0,
            Self::Large => 1.15,
        }
    }
}
impl fmt::Display for Scale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Compact => "Compact",
                Self::Comfortable => "Comfortable",
                Self::Large => "Large",
            }
        )
    }
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            player_name: "Player".into(),
            java: String::new(),
            memory_mb: 4096,
            language: "en".into(),
            confirm_delete: true,
            accent: Accent::Copper,
            scale: Scale::Comfortable,
            show_activity: true,
            reduce_motion: false,
            remember_page: true,
            last_page: "home".into(),
            offline_accounts: Vec::new(),
            active_account: None,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<()> {
        validate_player_name(&self.player_name)?;
        // A hand-edited file could carry an unusable saved identity; refusing it
        // here keeps every later launch and save honest about what it accepted.
        for account in &self.offline_accounts {
            validate_player_name(&account.name)?;
        }
        if !(512..=32768).contains(&self.memory_mb) {
            return Err("Memory must be between 512 and 32768 MiB.".into());
        }
        Ok(())
    }
    /// The identity Play launches with: the selected offline account, or the
    /// legacy single player name while the list is empty or nothing is selected.
    pub fn active_identity(&self) -> (String, String, String) {
        if let Some(account) = self
            .active_account
            .as_ref()
            .and_then(|id| self.offline_accounts.iter().find(|a| &a.id == id))
        {
            return (account.id.clone(), account.name.clone(), account.uuid());
        }
        (
            "offline".into(),
            self.player_name.clone(),
            offline_uuid(&self.player_name),
        )
    }
    /// Adding an identity is a request to use it, so the new account becomes
    /// active immediately; a player who only wanted to keep it can select
    /// another one back.
    pub fn add_offline_account(&mut self, name: &str) -> Result<()> {
        let name = name.trim();
        validate_player_name(name)?;
        // The id derives from the name, so a second row with the same name would
        // collide with the first one's key.
        if self
            .offline_accounts
            .iter()
            .any(|account| account.name.eq_ignore_ascii_case(name))
        {
            return Err(format!("{name} is already in the offline account list."));
        }
        let account = OfflineAccount::new(name);
        self.active_account = Some(account.id.clone());
        self.offline_accounts.push(account);
        Ok(())
    }
    pub fn select_offline_account(&mut self, id: &str) -> Result<()> {
        if !self.offline_accounts.iter().any(|account| account.id == id) {
            return Err("That offline account is no longer in the list.".into());
        }
        self.active_account = Some(id.into());
        Ok(())
    }
    /// Removing the selected account clears the selection rather than leaving a
    /// dangling id behind; `active_identity` then falls back to the legacy name.
    pub fn remove_offline_account(&mut self, id: &str) -> Result<()> {
        let before = self.offline_accounts.len();
        self.offline_accounts.retain(|account| account.id != id);
        if self.offline_accounts.len() == before {
            return Err("That offline account is no longer in the list.".into());
        }
        if self.active_account.as_deref() == Some(id) {
            self.active_account = None;
        }
        Ok(())
    }
}

pub fn safe_component(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        return Err(
            "Invalid identifier: only letters, digits, dots, dashes and underscores are allowed."
                .into(),
        );
    }
    Ok(())
}
pub fn load_instances(root: &DataRoot) -> Result<Vec<Instance>> {
    let mut items = Vec::new();
    if !root.instances_dir().exists() {
        return Ok(items);
    }
    for entry in std::fs::read_dir(root.instances_dir()).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        // Never traverse a symlink into another user's data directory.
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let path = entry.path().join("instance.json");
        if !path.exists() {
            continue;
        }
        let item: Instance = read_json(&path)?;
        safe_component(&item.id)?;
        safe_component(&item.game_version)?;
        if entry.file_name().to_str() != Some(&item.id) {
            return Err(format!(
                "Instance {} has a mismatched directory.",
                item.name
            ));
        }
        if let Some(version) = &item.installed_version {
            safe_component(version)?;
        }
        items.push(item);
    }
    items.sort_by_key(|item| item.name.to_lowercase());
    Ok(items)
}
pub fn create(root: &DataRoot, name: &str, game: &str, loader: Loader) -> Result<Instance> {
    let name = name.trim();
    let game = game.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err("Choose an instance name between 1 and 80 characters.".into());
    }
    safe_component(game)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let item = Instance {
        id: format!("world-{nonce}"),
        name: name.into(),
        game_version: game.into(),
        loader,
        installed_version: None,
        loader_version: None,
    };
    std::fs::create_dir(root.instances_dir())
        .or_else(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(error)
            }
        })
        .map_err(|e| e.to_string())?;
    std::fs::create_dir(root.instance_dir(&item.id)).map_err(|e| e.to_string())?;
    std::fs::create_dir(root.game_dir(&item.id)).map_err(|e| e.to_string())?;
    save_instance(root, &item)?;
    Ok(item)
}
pub fn save_instance(root: &DataRoot, item: &Instance) -> Result<()> {
    safe_component(&item.id)?;
    write_json(&root.instance_descriptor(&item.id), item)
}
pub fn load_preferences(root: &DataRoot) -> Result<Preferences> {
    let path = root.root.join("preferences.json");
    if !path.exists() && !path.with_extension("bak").exists() {
        return Ok(Preferences::default());
    }
    let preferences: Preferences = read_json(&path)?;
    preferences.validate()?;
    Ok(preferences)
}
pub fn save_preferences(root: &DataRoot, preferences: &Preferences) -> Result<()> {
    preferences.validate()?;
    write_json(&root.root.join("preferences.json"), preferences)
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let source = if path.exists() {
        path.to_path_buf()
    } else {
        path.with_extension("bak")
    };
    serde_json::from_slice(
        &std::fs::read(&source).map_err(|e| format!("{}: {e}", source.display()))?,
    )
    .map_err(|e| format!("{}: {e}", source.display()))
}
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let staging = path.with_extension("tmp");
    let backup = path.with_extension("bak");
    std::fs::write(&staging, bytes).map_err(|e| e.to_string())?;
    // Windows rename cannot replace a destination; preserve a recoverable previous file.
    if path.exists() {
        if backup.exists() {
            std::fs::remove_file(&backup).map_err(|e| e.to_string())?;
        }
        std::fs::rename(path, &backup).map_err(|e| e.to_string())?;
    }
    std::fs::rename(staging, path).map_err(|e| e.to_string())
}
pub fn instance_path(root: &DataRoot, id: &str) -> Result<PathBuf> {
    safe_component(id)?;
    let path = root.instance_dir(id);
    if std::fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Refusing a symbolic-link instance directory.".into());
    }
    Ok(path)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub project_type: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectDetail {
    pub title: String,
    pub description: String,
    pub body: String,
    pub downloads: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub license: ProjectLicense,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectLicense {
    #[serde(default)]
    pub id: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub changelog: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ReleaseFile>,
    #[serde(default)]
    pub dependencies: Vec<ReleaseDependency>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseFile {
    pub url: String,
    pub filename: String,
    pub size: u64,
    pub primary: bool,
    pub hashes: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseDependency {
    pub dependency_type: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub file_name: Option<String>,
}
impl Release {
    pub fn primary_file(&self) -> Option<&ReleaseFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
    pub fn compatible(&self, item: &Instance, kind: &str) -> bool {
        self.game_versions.contains(&item.game_version)
            && (kind != "mod" || self.loaders.iter().any(|l| l == item.loader.api_name()))
    }
    pub fn required_dependencies(&self) -> bool {
        self.dependencies
            .iter()
            .any(|d| d.dependency_type == "required")
    }
}
#[derive(Debug, Clone)]
pub struct GameChoice {
    pub id: String,
    pub kind: String,
}
impl fmt::Display for GameChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} · {}", self.id, self.kind)
    }
}
#[derive(Debug, Clone)]
pub struct ImportReview {
    pub source: String,
    pub name: String,
    pub game: String,
    pub loader: Loader,
    pub files: usize,
    pub bytes: u64,
    pub optional: Vec<String>,
    pub notes: Vec<String>,
    pub copy_supported: bool,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Authentication {
    Offline,
    Requesting,
    DeviceCode {
        code: String,
        url: String,
        expires_in: u64,
    },
    Verifying,
    Connected {
        player: String,
    },
    Expired,
    Failed(String),
    Cancelled,
}
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub directory: bool,
}
#[derive(Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<Project>,
    pub total_hits: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    Mod,
    Modpack,
    ResourcePack,
    Shader,
}
impl ProjectType {
    pub const ALL: [Self; 4] = [Self::Mod, Self::Modpack, Self::ResourcePack, Self::Shader];
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Modpack => "modpack",
            Self::ResourcePack => "resourcepack",
            Self::Shader => "shader",
        }
    }
}
impl fmt::Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Mod => "Mods",
                Self::Modpack => "Modpacks",
                Self::ResourcePack => "Resource packs",
                Self::Shader => "Shaders",
            }
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Relevance,
    Downloads,
    Updated,
    Newest,
}
impl Sort {
    pub const ALL: [Self; 4] = [
        Self::Relevance,
        Self::Downloads,
        Self::Updated,
        Self::Newest,
    ];
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Updated => "updated",
            Self::Newest => "newest",
        }
    }
}
impl fmt::Display for Sort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Relevance => "Relevance",
                Self::Downloads => "Most downloaded",
                Self::Updated => "Recently updated",
                Self::Newest => "Newest",
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn releases_keep_compatibility_and_dependency_contracts() {
        let mut release:Release=serde_json::from_str(r#"{"id":"release","project_id":"project","name":"Stable","version_number":"1.0","version_type":"release","game_versions":["1.20.1"],"loaders":["fabric"],"files":[{"url":"https://cdn.modrinth.com/test.jar","filename":"test.jar","size":42,"primary":true,"hashes":{"sha1":"abc"}}],"dependencies":[]}"#).unwrap();
        let mut item = Instance {
            id: "test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            installed_version: None,
            loader_version: None,
        };
        assert!(release.compatible(&item, "mod"));
        item.loader = Loader::Vanilla;
        assert!(!release.compatible(&item, "mod"));
        assert!(release.compatible(&item, "resourcepack"));
        release.dependencies.push(ReleaseDependency {
            dependency_type: "required".into(),
            project_id: Some("api".into()),
            version_id: None,
            file_name: None,
        });
        assert!(release.required_dependencies());
        assert_eq!(release.primary_file().unwrap().filename, "test.jar");
        item.game_version = "1.21".into();
        assert!(!release.compatible(&item, "resourcepack"));
        assert_eq!(Loader::ALL.len(), 5);
    }
    #[test]
    fn identifiers_cannot_escape_the_root() {
        for value in ["", "..", "../save", "C:\\save", "a/b", "a b"] {
            assert!(safe_component(value).is_err());
        }
        assert!(safe_component("1.20.1-fabric").is_ok());
    }
    #[test]
    fn preferences_validate_before_persistence() {
        let mut p = Preferences::default();
        assert!(p.validate().is_ok());
        p.player_name = "../../foo".into();
        assert!(p.validate().is_err());
    }
    #[test]
    fn offline_accounts_are_unique_selectable_and_removable() {
        let mut p = Preferences::default();
        p.add_offline_account("Quiet").unwrap();
        assert_eq!(p.offline_accounts.len(), 1);
        assert_eq!(p.active_identity().1, "Quiet");
        assert!(p.add_offline_account("quiet").is_err(), "duplicate name");
        assert!(p.add_offline_account("no spaces").is_err());
        assert!(p.add_offline_account("").is_err());
        p.add_offline_account("Second").unwrap();
        let first = p.offline_accounts[0].id.clone();
        p.select_offline_account(&first).unwrap();
        assert_eq!(p.active_identity().1, "Quiet");
        p.remove_offline_account(&first).unwrap();
        assert_eq!(p.active_account, None, "the selection cannot dangle");
        // The legacy name is the identity again once nothing is selected.
        assert_eq!(p.active_identity().1, p.player_name);
        assert!(p.remove_offline_account(&first).is_err());
    }
    #[test]
    fn offline_identity_keeps_the_shipped_shape() {
        let uuid = offline_uuid("Player");
        assert_eq!(uuid.len(), 36);
        let groups: Vec<usize> = uuid.split('-').map(|g| g.len()).collect();
        assert_eq!(groups, vec![8, 4, 4, 4, 12]);
        assert!(
            uuid.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c) || c == '-')
        );
        assert_eq!(uuid, offline_uuid("Player"), "the derivation is stable");
        assert_ne!(uuid, offline_uuid("player"), "names stay distinct");
        // An account's uuid is derived, never stored, so a rename cannot leave
        // a stale identifier in the file.
        let account = OfflineAccount::new("Player");
        assert_eq!(account.id, uuid);
        assert_eq!(account.uuid(), uuid);
    }
    #[test]
    fn older_preferences_files_load_without_account_fields() {
        let legacy = r#"{"player_name":"Legacy","java":"","memory_mb":2048,"language":"en","confirm_delete":true,"accent":"Glacier","scale":"Compact","show_activity":false,"reduce_motion":false,"remember_page":true,"last_page":"home"}"#;
        let p: Preferences = serde_json::from_str(legacy).unwrap();
        assert!(p.offline_accounts.is_empty());
        assert_eq!(p.active_account, None);
        assert_eq!(p.active_identity().1, "Legacy");
    }
    #[test]
    fn instance_and_preferences_round_trip() {
        let root = DataRoot::new(std::env::temp_dir().join(format!(
                "palantir-ui-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir_all(&root.root).unwrap();
        let item = create(&root, "Quiet valley", "1.20.1", Loader::Fabric).unwrap();
        assert_eq!(load_instances(&root).unwrap()[0].id, item.id);
        let mut prefs = Preferences::default();
        save_preferences(&root, &prefs).unwrap();
        prefs.memory_mb = 2048;
        save_preferences(&root, &prefs).unwrap();
        assert_eq!(load_preferences(&root).unwrap().memory_mb, 2048);
        std::fs::remove_dir_all(root.root).unwrap();
    }
}
