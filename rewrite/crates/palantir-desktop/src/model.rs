use palantir_core::paths::DataRoot;
use palantir_net::download::sha1_bytes;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub type Result<T> = std::result::Result<T, String>;

/// Java collections are launcher-owned; Bedrock copies are discovered from
/// Windows, never persisted as Java descriptors or given a loader.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Edition {
    #[default]
    Java,
    Bedrock,
}
impl Edition {
    pub const ALL: [Self; 2] = [Self::Java, Self::Bedrock];
    pub fn name(self) -> &'static str {
        match self {
            Self::Java => "Java",
            Self::Bedrock => "Bedrock",
        }
    }
}

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
    /// The vendor installer that installs this loader, when it is a
    /// processor-based one. Fabric and Quilt publish a launcher profile and
    /// vanilla is the metadata itself, so they have no vendor jar; Forge and
    /// NeoForge do, and the desktop reaches them through
    /// `palantir_loader::forge` rather than through a second copy of the
    /// vendor facts here.
    pub fn installer_vendor(self) -> Option<palantir_loader::forge::Vendor> {
        match self {
            Self::Forge => Some(palantir_loader::forge::Vendor::Forge),
            Self::NeoForge => Some(palantir_loader::forge::Vendor::NeoForge),
            Self::Vanilla | Self::Fabric | Self::Quilt => None,
        }
    }
}

/// Which identity a launch uses.
///
/// The two are not interchangeable and must never be silently swapped: an
/// offline launch hands the game a launcher-derived identifier, and a
/// Microsoft launch hands it the session a real sign-in bought. A launcher that
/// quietly fell back from the second to the first would be reporting an
/// authenticated session it does not have.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LaunchMode {
    #[default]
    Offline,
    Microsoft,
}
impl LaunchMode {
    pub const ALL: [Self; 2] = [Self::Offline, Self::Microsoft];
}
impl fmt::Display for LaunchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Offline => "Offline identity",
            Self::Microsoft => "Microsoft account",
        })
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
    /// A window size this collection wants for its game, independent of the
    /// launcher preference. `None` is the common case: a game that is not
    /// asked for a size opens at its own default.
    #[serde(default)]
    pub window: Option<WindowOverride>,
    /// This collection's own memory budget in MiB. `None` follows the
    /// launcher preference.
    #[serde(default)]
    pub memory_mb: Option<u32>,
    /// This collection's own Java executable. `None` follows the launcher
    /// preference (and its blank means automatic management).
    #[serde(default)]
    pub java: Option<String>,
    /// Extra arguments, environment entries, and hooks this collection's
    /// launches carry. Empty means none.
    #[serde(default)]
    pub custom: palantir_loader::hooks::LaunchCustom,
}
/// The one memory rule both scopes share.
pub fn validate_memory_mb(memory_mb: u32) -> Result<()> {
    if !(512..=32768).contains(&memory_mb) {
        return Err("Memory must be between 512 and 32768 MiB.".into());
    }
    Ok(())
}
/// A runtime override names one executable path. It is executed directly and
/// never through a shell, so a control character in it has no innocent reading.
pub fn validate_java_override(path: &str) -> Result<()> {
    if path.is_empty() || path.chars().any(char::is_control) {
        return Err("Java runtime must be a plain executable path.".into());
    }
    Ok(())
}
/// Parse the per-collection launch-settings inputs: blank means "follow the
/// launcher" on that side, anything else must be valid on its own.
pub fn parse_runtime_overrides(memory: &str, java: &str) -> Result<(Option<u32>, Option<String>)> {
    let memory = memory.trim();
    let java = java.trim();
    let memory_mb = if memory.is_empty() {
        None
    } else {
        let parsed = memory
            .parse::<u32>()
            .map_err(|_| "Memory must be a whole number of MiB.".to_string())?;
        validate_memory_mb(parsed)?;
        Some(parsed)
    };
    let java = if java.is_empty() {
        None
    } else {
        validate_java_override(java)?;
        Some(java.to_string())
    };
    Ok((memory_mb, java))
}
/// The memory budget a launch should use: the collection's own, else the
/// launcher preference. Stored values are checked here too.
pub fn effective_memory(item: &Instance, preferences: &Preferences) -> Result<u32> {
    let memory_mb = item.memory_mb.unwrap_or(preferences.memory_mb);
    validate_memory_mb(memory_mb)?;
    Ok(memory_mb)
}
/// The runtime a launch should execute: the collection's own path, else the
/// launcher preference, else `None` meaning automatic management -- the
/// runtime named by the game's own metadata.
pub fn effective_java(item: &Instance, preferences: &Preferences) -> Result<Option<String>> {
    if let Some(java) = &item.java {
        validate_java_override(java)?;
        return Ok(Some(java.clone()));
    }
    let java = preferences.java.trim();
    Ok((!java.is_empty()).then(|| java.to_string()))
}
/// Parse the launch-settings texts into their stored shapes: arguments and
/// environment entries are shell-quoted words (environment split at the
/// *first* `=`, so a value can contain one), and a hook that is present must
/// parse before it is stored.
pub fn parse_launch_custom(
    args: &str,
    env: &str,
    pre_launch: &str,
    wrapper: &str,
    post_exit: &str,
) -> Result<palantir_loader::hooks::LaunchCustom> {
    use palantir_loader::hooks::{Hooks, LaunchCustom, command_words, env_pairs};
    let hook = |text: &str| -> Option<String> {
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    };
    // The engine's error type crosses into the desktop's String-based one
    // here, at the one boundary that parses player text.
    let custom = LaunchCustom {
        extra_args: command_words(args).map_err(|e| e.to_string())?,
        env: env_pairs(&command_words(env).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
        hooks: Hooks {
            pre_launch: hook(pre_launch),
            wrapper: hook(wrapper),
            post_exit: hook(post_exit),
        },
    };
    custom.validate().map_err(|e| e.to_string())?;
    Ok(custom)
}
/// The stored launch customs as the edit fields show them.
pub fn launch_custom_text(
    custom: &palantir_loader::hooks::LaunchCustom,
) -> (String, String, String, String, String) {
    use palantir_loader::hooks::quote_words;
    let env_words: Vec<String> = custom
        .env
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    (
        quote_words(&custom.extra_args),
        quote_words(&env_words),
        custom.hooks.pre_launch.clone().unwrap_or_default(),
        custom.hooks.wrapper.clone().unwrap_or_default(),
        custom.hooks.post_exit.clone().unwrap_or_default(),
    )
}
/// One collection's fixed game window size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowOverride {
    pub width: u32,
    pub height: u32,
}
impl WindowOverride {
    pub fn validate(&self) -> Result<()> {
        validate_resolution(self.width, self.height)
    }
}
/// One rule for every window size the launcher stores: both sides zero
/// (unset), or both in a plausible screen range. A half-filled pair is
/// refused rather than guessed at, because a launch command must never carry
/// a size the player did not actually choose.
pub fn validate_resolution(width: u32, height: u32) -> Result<()> {
    if width == 0 && height == 0 {
        return Ok(());
    }
    if !(320..=16384).contains(&width) || !(320..=16384).contains(&height) {
        return Err("Window size must be 320–16384 pixels per side, or left unset.".into());
    }
    Ok(())
}
/// Parse the window-size inputs: both blank means unset, anything else must
/// be a complete in-range pair.
pub fn parse_window_size(width: &str, height: &str) -> Result<(u32, u32)> {
    let width = width.trim();
    let height = height.trim();
    if width.is_empty() && height.is_empty() {
        return Ok((0, 0));
    }
    match (width.parse::<u32>(), height.parse::<u32>()) {
        (Ok(width), Ok(height)) => {
            validate_resolution(width, height)?;
            Ok((width, height))
        }
        _ => Err("Set both width and height, or leave both blank.".into()),
    }
}
/// The window size a launch should request: the collection's own override,
/// else the launcher preference, else `None` for the game's own default.
/// Stored values are checked here too -- a hand-edited descriptor must not
/// reach a launch command unchecked.
pub fn effective_resolution(
    item: &Instance,
    preferences: &Preferences,
) -> Result<Option<(u32, u32)>> {
    let (width, height) = match item.window {
        Some(window) => {
            window.validate()?;
            (window.width, window.height)
        }
        None => (preferences.window_width, preferences.window_height),
    };
    validate_resolution(width, height)?;
    Ok(if width == 0 {
        None
    } else {
        Some((width, height))
    })
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
    /// The settings schema that wrote this file. Zero is a file from before
    /// the field existed; `migrate` upgrades it on load and refuses a file
    /// from a *newer* build rather than guessing at fields it cannot know.
    #[serde(default = "legacy_settings_version")]
    pub version: u32,
    pub player_name: String,
    pub java: String,
    pub memory_mb: u32,
    /// The game window size launches request; both zero lets the game choose.
    pub window_width: u32,
    pub window_height: u32,
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
    /// Which collection tabs the app shows.
    #[serde(default)]
    pub features: Features,
    /// The Java runtimes the player has added to the roster. Selecting one
    /// copies its path into `java`; the roster is what makes a runtime
    /// findable again and what "remove" removes it from.
    #[serde(default)]
    pub java_installs: Vec<JavaInstall>,
}
/// One managed Java runtime in the roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaInstall {
    /// What the runtime reported for itself (`java -version`'s first line).
    pub label: String,
    /// The executable path.
    pub path: String,
}
/// A runtime that was asked for must still be there. Checked by name at
/// launch: a removed or renamed executable is a clear refusal naming the
/// path, not a spawn error that points nowhere.
pub fn ensure_java(path: &str) -> Result<()> {
    if !std::path::Path::new(path).exists() {
        return Err(format!(
            "The Java runtime {path} is gone. Choose another in Preferences, or clear this collection's override in Manage."
        ));
    }
    Ok(())
}
/// The settings schema this build writes.
pub const SETTINGS_VERSION: u32 = 1;
/// A file without a `version` key predates the schema field.
fn legacy_settings_version() -> u32 {
    0
}
/// Which collection tabs the app shows. Tabs we have not built yet (files,
/// screenshots) join this struct under the same `show_*` naming when their
/// surfaces exist -- no switch here promises a tab that is not built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Features {
    pub show_overview: bool,
    pub show_content: bool,
    pub show_worlds: bool,
    pub show_logs: bool,
    pub show_manage: bool,
}
impl Default for Features {
    fn default() -> Self {
        // Default-on: the default install shows every tab it has.
        Self {
            show_overview: true,
            show_content: true,
            show_worlds: true,
            show_logs: true,
            show_manage: true,
        }
    }
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
            version: SETTINGS_VERSION,
            player_name: "Player".into(),
            java: String::new(),
            memory_mb: 4096,
            window_width: 0,
            window_height: 0,
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
            features: Features::default(),
            java_installs: Vec::new(),
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
        validate_memory_mb(self.memory_mb)?;
        validate_resolution(self.window_width, self.window_height)?;
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
        window: None,
        memory_mb: None,
        java: None,
        custom: Default::default(),
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
/// Copy a collection to a new one: descriptor, game directory, worlds and
/// content, byte for byte. The source is only ever read. Shared engine
/// downloads are not copied -- they are already shared, which is the whole
/// point of sharing them.
pub fn duplicate(root: &DataRoot, id: &str, name: &str) -> Result<Instance> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err("Choose a name between 1 and 80 characters.".to_string());
    }
    let source = instance_path(root, id)?;
    let original: Instance = read_json(&source.join("instance.json"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let copy = Instance {
        id: format!("world-{nonce}"),
        name: name.into(),
        ..original
    };
    let target = root.instance_dir(&copy.id);
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    copy_tree(&source, &target)?;
    // The copied tree carried the original's descriptor; the copy writes its
    // own, naming itself.
    save_instance(root, &copy)?;
    Ok(copy)
}
/// Recursive copy of one collection directory. Refuses to follow symlinks:
/// a duplicate copies what the collection is, not where a link points.
fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        if kind.is_symlink() {
            return Err(format!(
                "{} contains a symbolic link; refusing to duplicate it.",
                from.display()
            ));
        }
        if kind.is_dir() {
            std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
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
    let mut preferences: Preferences = read_json(&path)?;
    migrate(&mut preferences)?;
    preferences.validate()?;
    Ok(preferences)
}
/// Bring a stored settings file up to this build's schema.
///
/// Version 0 is a file from before the schema field existed; every field it
/// lacks arrived through `serde(default)`, so migrating is declaring the
/// version, not rewriting values. A file from a newer build is refused: its
/// fields are not ours to reinterpret.
pub fn migrate(preferences: &mut Preferences) -> Result<()> {
    if preferences.version > SETTINGS_VERSION {
        return Err(format!(
            "preferences.json uses settings schema {}, but this build understands {}. Move the file aside or update the launcher.",
            preferences.version, SETTINGS_VERSION
        ));
    }
    preferences.version = SETTINGS_VERSION;
    Ok(())
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
/// A worker-resolved review receipt. Confirmation carries this exact plan;
/// the worker resolves again and refuses a changed set before fetching bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPlan {
    pub instance_id: String,
    pub instance_name: String,
    pub project_id: String,
    pub release_id: String,
    pub game_version: String,
    pub loader: Loader,
    pub files: Vec<ContentFile>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFile {
    pub url: String,
    pub file_name: String,
    pub sha1: String,
    pub size: Option<u64>,
    pub folder: String,
    /// Only true after the existing regular file was hash/size verified.
    pub already_present: bool,
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
    /// Archive bytes reviewed by the worker; confirmation must match them.
    pub archive_sha1: Option<String>,
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
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
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
    #[test]
    fn settings_round_trip_preserves_every_field() {
        let root = DataRoot::new(std::env::temp_dir().join(format!(
            "palantir-settings-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
        std::fs::create_dir_all(&root.root).unwrap();
        // Every field set away from its default, so a round trip that loses
        // any one of them fails here rather than in a player's file.
        let mut saved = Preferences {
            player_name: "RoundTrip".into(),
            java: "/opt/java/bin/java".into(),
            memory_mb: 8192,
            window_width: 1600,
            window_height: 900,
            confirm_delete: false,
            accent: Accent::Glacier,
            scale: Scale::Large,
            show_activity: false,
            reduce_motion: true,
            remember_page: false,
            last_page: "library".into(),
            java_installs: vec![JavaInstall {
                label: "openjdk 21".into(),
                path: "/opt/jdk/bin/java".into(),
            }],
            ..Preferences::default()
        };
        saved.add_offline_account("RoundTrip").unwrap();
        saved.features.show_worlds = false;
        saved.features.show_logs = false;
        save_preferences(&root, &saved).unwrap();
        assert_eq!(load_preferences(&root).unwrap(), saved);
        std::fs::remove_dir_all(root.root).unwrap();
    }
    #[test]
    fn schema_version_migrates_old_files_and_refuses_newer_ones() {
        let legacy = r#"{"player_name":"Legacy","java":"","memory_mb":2048,"language":"en","confirm_delete":true,"accent":"Glacier","scale":"Compact","show_activity":false,"reduce_motion":false,"remember_page":true,"last_page":"home"}"#;
        let mut p: Preferences = serde_json::from_str(legacy).unwrap();
        assert_eq!(p.version, 0, "a file without the key parses as pre-schema");
        assert!(
            p.features.show_worlds,
            "missing Features defaults to showing tabs"
        );
        migrate(&mut p).unwrap();
        assert_eq!(p.version, SETTINGS_VERSION);
        let mut future = Preferences {
            version: SETTINGS_VERSION + 1,
            ..Preferences::default()
        };
        assert!(
            migrate(&mut future).is_err(),
            "a newer schema must not be guessed at"
        );
    }
    fn sample_instance() -> Instance {
        Instance {
            id: "world-test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        }
    }
    #[test]
    fn duplicating_copies_every_byte_and_leaves_the_source_alone() {
        let root = DataRoot::new(std::env::temp_dir().join(format!(
            "palantir-duplicate-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
        std::fs::create_dir_all(&root.root).unwrap();
        let item = create(&root, "Original", "1.20.1", Loader::Fabric).unwrap();
        let game = root.game_dir(&item.id);
        std::fs::create_dir_all(game.join("saves/World")).unwrap();
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("saves/World/level.dat"), b"world bytes").unwrap();
        std::fs::write(game.join("mods/mod.jar"), b"mod bytes").unwrap();
        let copy = duplicate(&root, &item.id, "Copy of Original").unwrap();
        assert_ne!(copy.id, item.id);
        assert_eq!(copy.name, "Copy of Original");
        assert_eq!(copy.game_version, item.game_version);
        let copied = root.game_dir(&copy.id);
        for path in ["saves/World/level.dat", "mods/mod.jar"] {
            assert_eq!(
                std::fs::read(copied.join(path)).unwrap(),
                std::fs::read(game.join(path)).unwrap(),
                "{path} did not survive the copy byte for byte"
            );
        }
        // The source is untouched and both collections are ordinary
        // collections afterwards.
        assert_eq!(
            std::fs::read(game.join("mods/mod.jar")).unwrap(),
            b"mod bytes"
        );
        assert_eq!(load_instances(&root).unwrap().len(), 2);
        // Nonsense is refused before anything is written.
        assert!(duplicate(&root, "absent", "X").is_err());
        assert!(duplicate(&root, &item.id, "  ").is_err());
        std::fs::remove_dir_all(root.root).unwrap();
    }
    #[test]
    fn window_sizes_follow_the_collection_then_the_launcher() {
        let mut preferences = Preferences::default();
        let item = sample_instance();
        // Neither set: the game opens at its own default size.
        assert_eq!(effective_resolution(&item, &preferences).unwrap(), None);
        preferences.window_width = 1280;
        preferences.window_height = 720;
        assert_eq!(
            effective_resolution(&item, &preferences).unwrap(),
            Some((1280, 720))
        );
        // A collection's own window size wins over the launcher preference.
        let item = Instance {
            window: Some(WindowOverride {
                width: 800,
                height: 600,
            }),
            ..item
        };
        assert_eq!(
            effective_resolution(&item, &preferences).unwrap(),
            Some((800, 600))
        );
    }
    #[test]
    fn half_filled_or_impossible_window_sizes_are_refused() {
        assert!(validate_resolution(0, 0).is_ok());
        assert!(validate_resolution(1280, 720).is_ok());
        assert!(validate_resolution(0, 720).is_err());
        assert!(validate_resolution(1280, 0).is_err());
        assert!(validate_resolution(12, 12).is_err());
        assert!(validate_resolution(20000, 720).is_err());
        assert_eq!(parse_window_size("  ", "").unwrap(), (0, 0));
        assert!(parse_window_size("1280", "").is_err());
        assert!(parse_window_size("wide", "720").is_err());
        // A hand-edited descriptor cannot smuggle an impossible size past
        // the launch that would read it.
        let item = Instance {
            window: Some(WindowOverride {
                width: 12,
                height: 12,
            }),
            ..sample_instance()
        };
        assert!(effective_resolution(&item, &Preferences::default()).is_err());
    }
    #[test]
    fn older_files_gain_window_defaults() {
        let legacy = r#"{"id":"x","name":"X","game_version":"1.20.1","loader":"Vanilla","installed_version":null}"#;
        let item: Instance = serde_json::from_str(legacy).unwrap();
        assert_eq!(item.window, None);
        let legacy_prefs = r#"{"player_name":"Legacy","java":"","memory_mb":2048,"language":"en","confirm_delete":true,"accent":"Glacier","scale":"Compact","show_activity":false,"reduce_motion":false,"remember_page":true,"last_page":"home"}"#;
        let p: Preferences = serde_json::from_str(legacy_prefs).unwrap();
        assert_eq!((p.window_width, p.window_height), (0, 0));
        assert_eq!(effective_resolution(&item, &p).unwrap(), None);
        assert_eq!(item.memory_mb, None);
        assert_eq!(item.java, None);
        assert!(p.java_installs.is_empty());
    }
    #[test]
    fn a_removed_runtime_is_refused_by_name() {
        let dir = std::env::temp_dir().join(format!(
            "palantir-java-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let present = dir.join("java");
        std::fs::write(&present, b"stub").unwrap();
        assert!(ensure_java(&present.display().to_string()).is_ok());
        std::fs::remove_file(&present).unwrap();
        let error = ensure_java(&present.display().to_string()).unwrap_err();
        assert!(
            error.contains(&present.display().to_string()),
            "the refusal must name the missing runtime: {error}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn collection_runtime_overrides_win_but_stay_validated() {
        let mut preferences = Preferences::default();
        let mut item = sample_instance();
        // Nothing overridden: the launcher's budget, and managed runtime.
        assert_eq!(effective_memory(&item, &preferences).unwrap(), 4096);
        assert_eq!(effective_java(&item, &preferences).unwrap(), None);
        item.memory_mb = Some(2048);
        preferences.java = "/opt/launcher/java".into();
        assert_eq!(effective_memory(&item, &preferences).unwrap(), 2048);
        assert_eq!(
            effective_java(&item, &preferences).unwrap().as_deref(),
            Some("/opt/launcher/java")
        );
        // A collection's own runtime wins over the launcher's.
        item.java = Some("/opt/collection/java".into());
        assert_eq!(
            effective_java(&item, &preferences).unwrap().as_deref(),
            Some("/opt/collection/java")
        );
        // Stored nonsense must not reach a launch command, whichever scope it
        // hides in.
        item.memory_mb = Some(12);
        assert!(effective_memory(&item, &preferences).is_err());
        item.memory_mb = None;
        item.java = Some(String::new());
        assert!(effective_java(&item, &preferences).is_err());
    }
    #[test]
    fn runtime_override_inputs_treat_blank_as_follow_the_launcher() {
        assert_eq!(parse_runtime_overrides("", "").unwrap(), (None, None));
        assert_eq!(
            parse_runtime_overrides("2048", "").unwrap(),
            (Some(2048), None)
        );
        assert!(parse_runtime_overrides("12", "").is_err());
        assert!(parse_runtime_overrides("wide", "").is_err());
        assert!(parse_runtime_overrides("", "bad\npath").is_err());
        assert_eq!(
            parse_runtime_overrides(" 2048 ", " /opt/java ").unwrap(),
            (Some(2048), Some("/opt/java".into()))
        );
    }
}
