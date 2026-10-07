//! Instance discovery, summaries and creation.
//!
//! The GUI needs more than a folder name: every card shows the game version,
//! the mod loader and its build, playtime, mod counts and the memory ceiling,
//! and the Create Instance dialog must produce an instance that really has the
//! chosen loader registered as a versioned `mmc-pack.json` component (see
//! [`register_loader`]; its libraries and entry point come from the same
//! metadata as Minecraft's), not just a renamed folder.
//!
//! All of it is plain filesystem work on `palantir-core`, so the whole module is
//! unit-tested against temp dirs — no window, no network.

use std::path::{Path, PathBuf};

use palantir_core::instance::Instance;
use palantir_core::pack::PackProfile;
use palantir_core::paths::PalantirPaths;
use palantir_core::settings::defaults;
use crate::model::InstanceEntry;

use crate::catalog::LoaderKind;
use crate::mods::list_mods;

/// Everything a card and the detail sidebar need about one instance.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceCard {
    /// Folder id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// `iconKey`.
    pub icon: String,
    /// The instance's own icon, already read off disk: the PNG that
    /// `<icons_dir>/<key>.png` names, or `None` when the key resolves to no
    /// file -- a builtin key, an instance nobody has given an icon, or a read
    /// that failed.
    ///
    /// Read with the listing rather than where the tile is drawn, because a view
    /// gets no disk and no frame may open a file: `crate::pages::instance`'s note
    /// on reading a listing once is the same rule one page over. `Arc<[u8]>`
    /// rather than `Vec<u8>` because a card is cloned per frame and this is the
    /// one field that is not a few bytes.
    pub icon_png: Option<std::sync::Arc<[u8]>>,
    /// Owning group, if any.
    pub group: Option<String>,
    /// `net.minecraft` component version (`""` when the pack file is unreadable).
    pub mc_version: String,
    /// Mod loader, `Vanilla` when there is none.
    pub loader: LoaderKind,
    /// Loader build for the loader component (`""` for vanilla).
    pub loader_version: String,
    /// Seconds played, from `totalTimePlayed`.
    pub playtime_secs: i64,
    /// Milliseconds since the epoch at the last launch, from `lastLaunchTime`.
    ///
    /// The instance header's third fact: `page-header/index.vue` draws a
    /// `PageHeaderMetadataTimeItem` over `instance.last_played` and falls back to
    /// `neverPlayed` when there is none, so the header needs the *time* and not
    /// only the *duration*. Zero is "never launched", which is what a pack that
    /// has never been started reads as.
    pub last_launch_millis: i64,
    /// Mod files present in `mods/`.
    pub mods_total: usize,
    /// Of those, the enabled ones.
    pub mods_enabled: usize,
    /// `MaxMemAlloc` from `instance.cfg`.
    pub max_mem_mb: i64,
    /// Why the instance could not be fully summarized, if anything went wrong.
    pub problem: Option<String>,
}

impl InstanceCard {
    /// `Fabric 0.19.5 · 26.2`, or just the game version for vanilla.
    pub fn subtitle(&self) -> String {
        if self.loader == LoaderKind::Vanilla || self.loader_version.is_empty() {
            self.mc_version.clone()
        } else {
            format!("{} {} · {}", self.loader.label(), self.loader_version, self.mc_version)
        }
    }

    /// The header's own second fact: the total playtime, in words.
    ///
    /// `pages/instance/components/page-header/index.vue` counts the seconds down
    /// and says the largest unit it reaches, spelled out rather than abbreviated:
    /// `3 hours`, `1 hour`, `45 minutes`, `1 minute`, `30 seconds`, `1 second`.
    /// An instance with no playtime has no label at all -- the header's `v-if="...
    /// && playtimeLabel"` skips the fact rather than showing a zero -- so the one
    /// word that reaches the header when the seconds are zero is `neverPlayed`,
    /// which is the same word its clock arm says.
    pub fn playtime_label(&self) -> String {
        let secs = self.playtime_secs.max(0);
        if secs == 0 {
            return "Never played".to_string();
        }
        let plural = |count: i64, unit: &str| {
            if count == 1 {
                format!("1 {unit}")
            } else {
                format!("{count} {unit}s")
            }
        };
        let hours = secs / 3600;
        if hours >= 1 {
            return plural(hours, "hour");
        }
        let minutes = secs / 60;
        if minutes >= 1 {
            return plural(minutes, "minute");
        }
        plural(secs, "second")
    }

    /// `loaderLabel`: the loader's display name and the game version, one space
    /// apart.
    ///
    /// The header's first fact is `[loaderDisplayName, game_version].filter(Boolean)
    /// .join(' ')`, and `formatLoaderLabel` is the loader's *name* -- "Fabric",
    /// "Vanilla" -- with no build on it. A header that put the loader's build here
    /// would be saying something the reference does not say; the build is what the
    /// instance's own Content tab lists.
    pub fn loader_label(&self) -> String {
        let mut label = self.loader.label().to_string();
        if !self.mc_version.is_empty() {
            label.push(' ');
            label.push_str(&self.mc_version);
        }
        label
    }

    /// Whether a mod loader is installed.
    #[cfg(test)]
    pub fn has_loader(&self) -> bool {
        self.loader.loads_mods() && !self.loader_version.is_empty()
    }
}

/// When the instance was last launched, in the header's third fact.
///
/// `page-header-metadata-time-item.vue` renders `useRelativeTime`, which is
/// `Intl.RelativeTimeFormat` over dayjs's thresholds, and joins it to its own
/// `label` prop: *Last played 2 hours ago*. The header only reaches for that arm
/// when `instance.last_played` is set, and its `v-else` is the same clock icon
/// over `neverPlayed` -- so the two arms are one function, and a stamp of zero is
/// the second of them.
///
/// The thresholds are dayjs's, which are the same numbers `Intl` ships with
/// behind the default locale: 44 seconds is "a few seconds", 45 is "a minute",
/// 90 seconds is two minutes, 45 minutes is an hour, 22 hours is a day, 26 days
/// is a month, 26 more is a year. A future stamp is not a thing an instance
/// carries -- a clock that has moved backwards would be -- so it reads as
/// "just now" rather than as a negative age.
pub fn last_played_label(millis: i64, now_millis: i64) -> String {
    if millis <= 0 {
        return "Never played".to_string();
    }
    let seconds = (now_millis - millis).max(0) / 1000;
    let ago = |count: i64, unit: &str| {
        if count == 1 {
            format!("1 {unit} ago")
        } else {
            format!("{count} {unit}s ago")
        }
    };
    let relative = if seconds <= 44 {
        "a few seconds ago".to_string()
    } else if seconds <= 89 {
        "a minute ago".to_string()
    } else if seconds <= 44 * MINUTE {
        ago((seconds / MINUTE).max(1), "minute")
    } else if seconds <= 89 * MINUTE {
        "an hour ago".to_string()
    } else if seconds <= 21 * HOUR {
        ago(seconds / HOUR, "hour")
    } else if seconds <= 35 * HOUR {
        "a day ago".to_string()
    } else if seconds <= 25 * DAY {
        ago(seconds / DAY, "day")
    } else if seconds <= 45 * DAY {
        "a month ago".to_string()
    } else if seconds <= 319 * DAY {
        ago(seconds / (30 * DAY), "month")
    } else if seconds <= 547 * DAY {
        "a year ago".to_string()
    } else {
        ago(seconds / (365 * DAY), "year")
    };
    format!("Last played {relative}")
}

/// The three units dayjs's thresholds are counted in. They are named so the
/// ladder above reads as the one in `Intl.RelativeTimeFormat` -- `s`, `m`, `mm`,
/// `h`, `hh`, `d`, `dd`, `M`, `MM`, `y` -- rather than as a wall of digit runs.
const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// Result of a full background scan: the cards, and the directory they came
/// from.
///
/// `selected` and `status` were the old shell's summary strip — the tests are
/// their only reader now, so they are gated rather than deleted: they are what
/// pins the scan's outcome.
#[derive(Debug, Clone, Default)]
pub struct LoadedInstances {
    /// One summary per discovered instance, name-sorted.
    pub cards: Vec<InstanceCard>,
    /// Pre-selected instance id (`InstanceDir` override applied).
    #[cfg(test)]
    pub selected: Option<String>,
    /// Resolved instances directory.
    pub instances_dir: PathBuf,
    /// One-line outcome for the status strip.
    #[cfg(test)]
    pub status: String,
}

/// Read one instance's loader, version, mods and memory ceiling.
pub fn summarize(instances_dir: &Path, entry: &InstanceEntry) -> InstanceCard {
    let mut card = InstanceCard {
        id: entry.id.clone(),
        name: entry.name.clone(),
        icon: entry.icon.clone(),
        // Filled by [`load`], which is the only reader with the `icons/`
        // directory in hand.
        icon_png: None,
        group: entry.group.clone(),
        mc_version: String::new(),
        loader: LoaderKind::Vanilla,
        loader_version: String::new(),
        playtime_secs: entry.playtime_secs,
        last_launch_millis: 0,
        mods_total: 0,
        mods_enabled: 0,
        max_mem_mb: defaults::MAX_MEM_ALLOC,
        problem: None,
    };
    let instance = match Instance::open(&instances_dir.join(&entry.id)) {
        Ok(instance) => instance,
        Err(error) => {
            card.problem = Some(format!("cannot open instance: {error}"));
            return card;
        }
    };
    card.max_mem_mb = instance.settings().get_i64("MaxMemAlloc", defaults::MAX_MEM_ALLOC);
    card.last_launch_millis = instance.last_launch_millis().max(0);
    match PackProfile::load(&instance.mmc_pack_path()) {
        Ok(profile) => {
            for component in profile.components() {
                if component.uid == "net.minecraft" {
                    card.mc_version = component.version.clone();
                    continue;
                }
                if let Some(kind) = LoaderKind::from_uid(&component.uid) {
                    if component.is_enabled() {
                        card.loader = kind;
                        card.loader_version = component.version.clone();
                    }
                }
            }
        }
        Err(error) => card.problem = Some(format!("cannot read mmc-pack.json: {error}")),
    }
    let mods = list_mods(&instance.mods_dir());
    card.mods_total = mods.len();
    card.mods_enabled = mods.iter().filter(|entry| entry.enabled).count();
    card
}

/// Scan every instance under `paths` and summarize it. Failures degrade to an
/// empty list plus a status line, never a crash.
pub fn load(paths: &PalantirPaths) -> LoadedInstances {
    let instances_dir = paths.configured_instances_dir();
    let model = match crate::model::InstanceListModel::load(paths) {
        Ok(model) => model,
        Err(_error) => {
            return LoadedInstances {
                instances_dir: instances_dir.clone(),
                // The message goes only to the tests' status field — the binary
                // has no strip to put it on, so the binding is named for the
                // build that does not use it.
                #[cfg(test)]
                status: format!("listing instances failed: {_error}"),
                ..Default::default()
            };
        }
    };
    let mut cards: Vec<InstanceCard> =
        model.entries().iter().map(|entry| summarize(&instances_dir, entry)).collect();
    // Each card's own icon, read once with the listing rather than per frame.
    for card in &mut cards {
        card.icon_png = instance_icon(paths, &card.icon);
    }
    #[cfg(test)]
    let selected = resolve_selected_id(paths, &cards);
    #[cfg(test)]
    let status = if cards.is_empty() {
        format!("No instances yet — press N to create one (looked in {}).", instances_dir.display())
    } else {
        format!("{} instance(s) ready", cards.len())
    };
    LoadedInstances {
        cards,
        instances_dir,
        #[cfg(test)]
        selected,
        #[cfg(test)]
        status,
    }
}

/// Pre-selection for startup: `SelectedInstance` when it still exists.
#[cfg(test)]
pub fn resolve_selected_id(paths: &PalantirPaths, cards: &[InstanceCard]) -> Option<String> {
    let want = paths.selected_instance_id()?;
    if want.trim().is_empty() {
        return None;
    }
    cards.iter().any(|card| card.id == want).then_some(want)
}

// ---- Creating ----------------------------------------------------------

/// What the Create Instance dialog collected.
#[derive(Debug, Clone, PartialEq)]
pub struct NewInstance {
    /// Display name (also the folder id, uniquified on collision).
    pub name: String,
    /// Chosen loader.
    pub loader: LoaderKind,
    /// Chosen game version.
    pub game: String,
    /// Chosen loader build (required unless `loader` is vanilla).
    pub loader_build: Option<String>,
    /// `iconKey` to store (`icon_source` wins when both are set).
    pub icon_key: Option<String>,
    /// A PNG on disk to install as a custom icon (drag & drop / upload).
    pub icon_source: Option<PathBuf>,
    /// `MaxMemAlloc` to set (enables the `OverrideMemory` gate when `Some`).
    pub max_mem_mb: Option<i64>,
    /// `JavaPath` to set (enables the `OverrideJavaLocation` gate when `Some`).
    ///
    /// This is the Synced settings "Java binary" the Create dialog starts from,
    /// written down rather than looked up later: the instance settings page then
    /// shows the Java it runs with, the same way it shows the heap. A launch
    /// still falls back to that default when the path here is gone or names a
    /// major the version cannot run on, so pinning one at creation cannot make
    /// an instance unlaunchable.
    pub java_path: Option<String>,
}

impl NewInstance {
    /// A vanilla instance for `game`.
    pub fn vanilla(name: &str, game: &str) -> NewInstance {
        NewInstance {
            name: name.to_string(),
            loader: LoaderKind::Vanilla,
            game: game.to_string(),
            loader_build: None,
            icon_key: None,
            icon_source: None,
            max_mem_mb: None,
            java_path: None,
        }
    }
}

/// A created instance plus anything that did not go perfectly.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatedInstance {
    /// New folder id.
    pub id: String,
    /// Non-fatal problems (the instance exists either way).
    pub warnings: Vec<String>,
}

/// Create an instance and record the requested loader for real.
///
/// Order matters: `Instance::create` writes `instance.cfg` + `mmc-pack.json`,
/// then the loader component is registered in that profile, then
/// settings/icon, then a single save.
pub fn create(paths: &PalantirPaths, spec: &NewInstance) -> Result<CreatedInstance, String> {
    let name = spec.name.trim();
    if name.is_empty() {
        return Err("give the instance a name".to_string());
    }
    if spec.game.trim().is_empty() {
        return Err("pick a game version".to_string());
    }
    let instances_dir = paths.configured_instances_dir();
    let mut warnings = Vec::new();
    let mut instance = Instance::create(&instances_dir, name, spec.game.trim())
        .map_err(|error| format!("creating the instance folder failed: {error}"))?;
    let id = instance.id();

    if spec.loader.loads_mods() {
        match spec.loader_build.as_deref().map(str::trim) {
            Some(build) if !build.is_empty() => {
                let uid = spec.loader.uid().unwrap_or_default();
                // The loader becomes a *component with a version*, which is what
                // Prism writes, and its version file then comes from the same
                // metadata service as Minecraft's.
                //
                // What this used to do is write a synthesized
                // `patches/<uid>.json`. A patch does not add to a component, it
                // *replaces* it, and the synthesized file carried a main class
                // and no libraries at all: the loader jar, the ASM stack it
                // loads and Forge's ForgeWrapper never reached the classpath, so
                // the game stopped on the first missing class. The invented
                // entry points were wrong too -- the metadata then started Forge
                // and NeoForge through ForgeWrapper, not through the
                // launchwrapper or bootstraplauncher names that were written
                // here, and the loader's own installer has since replaced that
                // rewrite entirely (G119).
                if let Err(error) = register_loader(&instance, uid, build) {
                    warnings.push(format!(
                        "{} {} was not installed: {error}",
                        spec.loader.label(),
                        build
                    ));
                }
            }
            _ => warnings.push(format!(
                "no {} build was selected, so the instance is vanilla for now",
                spec.loader.label()
            )),
        }
    }

    if let Some(mem) = spec.max_mem_mb {
        instance.settings_mut().set_bool("OverrideMemory", true);
        instance.settings_mut().set_i64("MaxMemAlloc", mem);
    }

    // The Synced settings default, written the way the instance settings page
    // writes it: the gate plus the path, because Prism reads `JavaPath` only
    // behind `OverrideJavaLocation`. A blank path is "nothing was chosen", so
    // it leaves the instance with no opinion rather than an empty one.
    if let Some(java) = spec.java_path.as_deref().map(str::trim).filter(|path| !path.is_empty()) {
        instance.settings_mut().set_bool("OverrideJavaLocation", true);
        instance.settings_mut().set_str("JavaPath", java);
    }

    if let Some(source) = spec.icon_source.as_deref() {
        match import_icon_file(paths, source, &id) {
            Ok(key) => instance.set_icon_key(&key),
            Err(error) => warnings.push(error),
        }
    } else if let Some(key) = spec.icon_key.as_deref().filter(|key| !key.is_empty()) {
        instance.set_icon_key(key);
    }

    if let Err(error) = instance.save() {
        return Err(format!("saving '{id}' failed: {error}"));
    }
    // Prism creates these on launch; making them now gives the pages
    // something to open straight away.
    for dir in [instance.mods_dir(), instance.game_root().join("saves")] {
        let _ = std::fs::create_dir_all(dir);
    }
    Ok(CreatedInstance { id, warnings })
}

/// Register a mod loader as a component of `instance`'s `mmc-pack.json`.
///
/// The version is the loader build the user picked; everything else about the
/// loader (its libraries, its entry point, what it requires) belongs to the
/// metadata for that uid/version pair, exactly as it does for Minecraft.
///
/// Refuses to write a component that could not be resolved: an empty uid or
/// build would leave a slot with nothing to load, which is precisely the state
/// that used to make a freshly created instance unlaunchable.
fn register_loader(instance: &Instance, uid: &str, build: &str) -> Result<(), String> {
    let uid = uid.trim();
    let build = build.trim();
    if uid.is_empty() {
        return Err("this loader has no component uid".to_string());
    }
    if build.is_empty() {
        return Err("no build version was given".to_string());
    }
    let path = instance.mmc_pack_path();
    let mut profile = PackProfile::load(&path)
        .map_err(|error| format!("reading {} failed: {error}", path.display()))?;
    profile.set_version(uid, build, true);
    profile
        .save(&path)
        .map_err(|error| format!("writing {} failed: {error}", path.display()))
}

// ---- Icons -------------------------------------------------------------

/// PNG magic: custom icons must really be PNGs (Prism only reads PNG icons).
const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The icon an `iconKey` names, read out of the launcher's own `icons/`
/// directory.
///
/// The rule on disk is Prism's, and it is the one [`import_icon_file`] writes
/// against: a key that is not a builtin resolves to `<icons_dir>/<key>.png`.
///
/// Every way this can come up empty is `None` rather than an error -- an empty
/// key, a key naming no file, a file that is not a PNG, a read that failed. A
/// card with no picture is a card drawn with an empty box, which is the
/// reference's own placeholder, and an instance whose icon cannot be read is not
/// an instance that failed to load.
pub fn instance_icon(paths: &PalantirPaths, key: &str) -> Option<std::sync::Arc<[u8]>> {
    if key.is_empty() {
        return None;
    }
    let bytes = std::fs::read(paths.icons_dir().join(format!("{key}.png"))).ok()?;
    if bytes.len() < PNG_MAGIC.len() || bytes[..PNG_MAGIC.len()] != PNG_MAGIC {
        return None;
    }
    Some(std::sync::Arc::from(bytes.into_boxed_slice()))
}

/// Install a PNG as `id`'s custom icon, returning the `iconKey` to store.
///
/// Prism resolves non-builtin keys to `<icons_dir>/<key>.png`, so the file is
/// copied there under a sanitized name.
pub fn import_icon_file(paths: &PalantirPaths, source: &Path, id: &str) -> Result<String, String> {
    let bytes = std::fs::read(source)
        .map_err(|error| format!("reading icon '{}' failed: {error}", source.display()))?;
    if bytes.len() < PNG_MAGIC.len() || bytes[..PNG_MAGIC.len()] != PNG_MAGIC {
        return Err(format!("'{}' is not a PNG image", source.display()));
    }
    let stem = match source.file_stem().and_then(|stem| stem.to_str()) {
        Some(stem) if !stem.is_empty() => stem,
        _ => id,
    };
    let key = palantir_core::util::sanitize_dir_name(stem);
    let key = if key.is_empty() { id.to_string() } else { key };
    let icons_dir = paths.icons_dir();
    palantir_core::util::ensure_dir(&icons_dir)
        .map_err(|error| format!("creating {} failed: {error}", icons_dir.display()))?;
    let dest = icons_dir.join(format!("{key}.png"));
    palantir_core::util::atomic_write(&dest, &bytes)
        .map_err(|error| format!("writing {} failed: {error}", dest.display()))?;
    Ok(key)
}

/// Install an icon file onto an existing instance and persist the key.
#[cfg(test)]
pub fn set_instance_icon(paths: &PalantirPaths, id: &str, source: &Path) -> Result<String, String> {
    let key = import_icon_file(paths, source, id)?;
    let mut instance = Instance::open(&paths.configured_instances_dir().join(id))
        .map_err(|error| format!("cannot open '{id}': {error}"))?;
    instance.set_icon_key(&key);
    instance.save().map_err(|error| format!("saving '{id}' failed: {error}"))?;
    Ok(key)
}

// ---- Importing from other launchers ------------------------------------

/// One instance found in another launcher's data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCandidate {
    /// Instance name (folder name).
    pub name: String,
    /// Source folder to copy.
    pub source: PathBuf,
    /// Which launcher it came from.
    pub origin: &'static str,
}

/// Instance directories of other launchers, as `(instances dir, origin)`.
///
/// Prism Launcher's on-disk format is exactly what this workspace reads, so
/// importing is a copy. (The Modrinth App keeps its profiles in its own
/// database format, which would need a converter — out of scope, and the
/// dialog does not pretend otherwise.)
pub fn candidate_roots() -> Vec<(PathBuf, &'static str)> {
    let mut roots = Vec::new();
    let mut push = |dir: PathBuf, origin: &'static str| {
        if dir.is_dir() {
            roots.push((dir, origin));
        }
    };
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        push(appdata.join("PrismLauncher").join("instances"), "Prism Launcher");
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        push(home.join(".local/share/PrismLauncher/instances"), "Prism Launcher");
        push(
            home.join("Library/Application Support/PrismLauncher/instances"),
            "Prism Launcher",
        );
    }
    roots
}

/// Scan one instances directory for importable instances.
pub fn scan_root(instances_dir: &Path, origin: &'static str) -> Vec<ImportCandidate> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(instances_dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // A real instance has an instance.cfg; skip stray folders.
        if !path.join("instance.cfg").is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        out.push(ImportCandidate { name, source: path, origin });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.source.cmp(&b.source)));
    out
}

/// Drop candidates whose id would collide with an instance we already have.
pub fn skip_existing(
    candidates: Vec<ImportCandidate>,
    existing: &[String],
) -> Vec<ImportCandidate> {
    candidates
        .into_iter()
        .filter(|candidate| !existing.iter().any(|id| id == &candidate.name))
        .collect()
}

/// Ids of the instances already in `instances_dir`.
pub fn existing_ids(instances_dir: &Path) -> Vec<String> {
    Instance::discover(instances_dir)
        .unwrap_or_default()
        .iter()
        .filter_map(|dir| dir.file_name().map(|name| name.to_string_lossy().into_owned()))
        .collect()
}

/// Every importable instance across the known launchers, excluding ids that
/// already exist in `paths`.
pub fn find_importable(paths: &PalantirPaths) -> Vec<ImportCandidate> {
    let instances_dir = paths.configured_instances_dir();
    let mut found = Vec::new();
    for (root, origin) in candidate_roots() {
        // Our own folder is what `load` scans; offering it back as an import
        // would only ever collide with itself.
        if root == instances_dir {
            continue;
        }
        found.extend(scan_root(&root, origin));
    }
    skip_existing(found, &existing_ids(&instances_dir))
}

/// Copy an instance folder into `paths`, uniquifying the id on collision.
pub fn import_instance(paths: &PalantirPaths, source: &Path) -> Result<String, String> {
    let instances_dir = paths.configured_instances_dir();
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("'{}' has no folder name", source.display()))?;
    let id = palantir_core::util::unique_dir_name(&instances_dir, &name)
        .map_err(|error| format!("picking a folder name failed: {error}"))?;
    let dest = instances_dir.join(&id);
    copy_dir_recursive(source, &dest)?;
    Ok(id)
}

/// Recursively copy a directory tree (files and directories; symlinks and
/// special files are skipped, which is all Prism ever writes in an instance).
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    let source = std::fs::canonicalize(src)
        .map_err(|error| format!("reading '{}': {error}", src.display()))?;
    // Resolve the nearest existing ancestor, including symlinks, before
    // creating anything. Importing into the source itself would otherwise
    // recursively copy the newly created destination until disk exhaustion.
    let mut ancestor = dst.to_path_buf();
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        if let Some(name) = ancestor.file_name() {
            suffix.push(name.to_owned());
        }
        if !ancestor.pop() || ancestor.as_os_str().is_empty() {
            ancestor = PathBuf::from(".");
            break;
        }
    }
    let mut destination = std::fs::canonicalize(&ancestor)
        .map_err(|error| format!("resolving '{}': {error}", dst.display()))?;
    for part in suffix.iter().rev() {
        destination.push(part);
    }
    if destination.starts_with(&source) {
        return Err("cannot copy an instance into itself or one of its subfolders".into());
    }
    copy_tree(&source, &destination)
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(src).map_err(|e| format!("reading '{}': {e}", src.display()))?;
    std::fs::create_dir_all(dst).map_err(|e| format!("creating '{}': {e}", dst.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("listing '{}': {e}", src.display()))?;
        let kind = entry
            .file_type()
            .map_err(|e| format!("stating '{}': {e}", entry.path().display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&from, &to)?;
        } else if kind.is_file() {
            std::fs::copy(&from, &to).map_err(|e| format!("copying '{}': {e}", from.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths() -> (tempfile::TempDir, PalantirPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        (dir, paths)
    }

    fn fabric_spec(name: &str, game: &str, build: &str) -> NewInstance {
        NewInstance {
            name: name.to_string(),
            loader: LoaderKind::Fabric,
            game: game.to_string(),
            loader_build: Some(build.to_string()),
            ..NewInstance::vanilla(name, game)
        }
    }

    #[test]
    fn creating_a_fabric_instance_installs_the_loader_for_real() {
        let (_dir, paths) = test_paths();
        let created = create(&paths, &fabric_spec("Fabric Dreams", "1.21.1", "0.19.5")).unwrap();
        assert_eq!(created.id, "Fabric Dreams");
        assert!(created.warnings.is_empty(), "warnings: {:?}", created.warnings);

        let instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        let profile = PackProfile::load(&instance.mmc_pack_path()).unwrap();
        assert_eq!(profile.get("net.fabricmc.fabric-loader").unwrap().version, "0.19.5");
        assert_eq!(profile.get("net.minecraft").unwrap().version, "1.21.1");
        assert!(instance.mods_dir().is_dir(), "mods folder is created up front");

        // The loader is a component of the profile, and *only* that: a patch
        // file here would replace the metadata's version file for the loader
        // and take its libraries with it, which is how every loader used to end
        // up missing its jars.
        assert!(
            !instance
                .patches_dir()
                .join("net.fabricmc.fabric-loader.json")
                .exists(),
            "the loader must come from the metadata, not from an invented patch"
        );

        // …and the card agrees with the disk.
        let model = crate::model::InstanceListModel::load(&paths).unwrap();
        let card = summarize(&paths.instances_dir(), &model.entries()[0]);
        assert_eq!(card.loader, LoaderKind::Fabric);
        assert_eq!(card.loader_version, "0.19.5");
        assert_eq!(card.mc_version, "1.21.1");
        assert_eq!(card.subtitle(), "Fabric 0.19.5 · 1.21.1");
        assert!(card.has_loader());
    }

    #[test]
    fn creating_a_vanilla_instance_adds_no_loader() {
        let (_dir, paths) = test_paths();
        let created = create(&paths, &NewInstance::vanilla("Plain", "26.2")).unwrap();
        let instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        let profile = PackProfile::load(&instance.mmc_pack_path()).unwrap();
        // One component. The `org.lwjgl3` slot Prism writes beside it is not
        // needed against Mojang's own file, which carries the LWJGL libraries
        // itself, so a new instance does not name it.
        assert_eq!(profile.components().len(), 1);
        assert_eq!(profile.get("net.minecraft").unwrap().version, "26.2");
        assert!(
            !profile
                .components()
                .iter()
                .any(|component| LoaderKind::from_uid(&component.uid).is_some()),
            "vanilla must not carry a loader component"
        );
        assert!(!instance.patches_dir().join("net.fabricmc.fabric-loader.json").exists());

        // …and the card agrees with the disk.
        let model = crate::model::InstanceListModel::load(&paths).unwrap();
        let card = summarize(&paths.instances_dir(), &model.entries()[0]);
        assert_eq!(card.loader, LoaderKind::Vanilla);
        assert_eq!(card.subtitle(), "26.2");
        assert!(!card.has_loader());
    }

    #[test]
    fn a_created_instance_records_the_java_it_was_given() {
        let (_dir, paths) = test_paths();
        let spec = NewInstance {
            java_path: Some("  C:/jdk21/bin/javaw.exe  ".to_string()),
            ..NewInstance::vanilla("Pinned", "1.21.1")
        };
        let created = create(&paths, &spec).unwrap();
        let instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        // Prism reads `JavaPath` only behind `OverrideJavaLocation`, so the gate
        // is what makes the path mean anything at all — writing one without the
        // other is how a setting ends up stored and ignored.
        assert!(instance.settings().get_bool("OverrideJavaLocation", false));
        assert_eq!(instance.settings().get_str("JavaPath", ""), "C:/jdk21/bin/javaw.exe");

        // Nothing chosen leaves the instance with no opinion rather than an
        // empty path the settings page would show as a choice.
        let plain = create(&paths, &NewInstance::vanilla("Unpinned", "1.21.1")).unwrap();
        let plain = Instance::open(&paths.instances_dir().join(&plain.id)).unwrap();
        assert!(!plain.settings().get_bool("OverrideJavaLocation", false));
        assert_eq!(plain.settings().get_str("JavaPath", ""), "");

        // A blank path is "nothing was chosen" too: the create dialog passes the
        // default straight through, and an empty one is ordinary.
        let blank = NewInstance {
            java_path: Some("   ".to_string()),
            ..NewInstance::vanilla("Blank", "1.21.1")
        };
        let blank = create(&paths, &blank).unwrap();
        let blank = Instance::open(&paths.instances_dir().join(&blank.id)).unwrap();
        assert!(!blank.settings().get_bool("OverrideJavaLocation", false));
    }

    #[test]
    fn loader_without_a_build_warns_but_still_creates() {
        let (_dir, paths) = test_paths();
        let spec = NewInstance {
            loader_build: None,
            ..fabric_spec("Half Loaded", "1.21.1", "0.19.5")
        };
        let created = create(&paths, &spec).unwrap();
        assert_eq!(created.warnings.len(), 1);
        assert!(created.warnings[0].contains("no Fabric build"));
        let model = crate::model::InstanceListModel::load(&paths).unwrap();
        let card = summarize(&paths.instances_dir(), &model.entries()[0]);
        assert_eq!(card.loader, LoaderKind::Vanilla);
        assert_eq!(card.subtitle(), "1.21.1");
    }

    #[test]
    fn creation_validates_name_and_game_version() {
        let (_dir, paths) = test_paths();
        let blank = NewInstance::vanilla("   ", "26.2");
        assert!(create(&paths, &blank).unwrap_err().contains("name"));
        let no_game = NewInstance::vanilla("Fine", " ");
        assert!(create(&paths, &no_game).unwrap_err().contains("game version"));
    }

    #[test]
    fn duplicate_names_get_unique_folders() {
        let (_dir, paths) = test_paths();
        let first = create(&paths, &NewInstance::vanilla("Same", "26.2")).unwrap();
        let second = create(&paths, &NewInstance::vanilla("Same", "26.2")).unwrap();
        assert_eq!(first.id, "Same");
        assert_ne!(second.id, first.id);
        assert!(paths.instances_dir().join(&second.id).is_dir());
    }

    #[test]
    fn custom_icons_are_copied_into_the_icons_dir() {
        let (dir, paths) = test_paths();
        let png = dir.path().join("my pack.png");
        std::fs::write(&png, [&PNG_MAGIC[..], b"pretend pixels"].concat()).unwrap();
        let created = create(
            &paths,
            &NewInstance { icon_source: Some(png.clone()), ..NewInstance::vanilla("Iconed", "26.2") },
        )
        .unwrap();
        assert!(created.warnings.is_empty(), "warnings: {:?}", created.warnings);
        let icon = paths.icons_dir().join("my pack.png");
        assert!(icon.is_file());
        let instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        assert_eq!(instance.icon_key(), "my pack");

        // Non-PNG uploads are refused with a reason.
        let bogus = dir.path().join("bogus.png");
        std::fs::write(&bogus, b"not an image").unwrap();
        let err = set_instance_icon(&paths, &created.id, &bogus).unwrap_err();
        assert!(err.contains("not a PNG"), "error: {err}");
        assert!(import_icon_file(&paths, &dir.path().join("missing.png"), "x").is_err());
    }

    #[test]
    fn an_instance_s_icon_is_read_with_the_listing_and_a_missing_one_is_an_empty_box() {
        let (dir, paths) = test_paths();
        let png = dir.path().join("my pack.png");
        std::fs::write(&png, [&PNG_MAGIC[..], b"pretend pixels"].concat()).unwrap();
        let created = create(
            &paths,
            &NewInstance { icon_source: Some(png), ..NewInstance::vanilla("Iconed", "26.2") },
        )
        .unwrap();

        // The key the instance carries resolves to the file the icon was copied
        // to, and the card the *listing* builds carries its bytes -- read once
        // with the scan rather than by whichever frame happens to draw the tile.
        let loaded = load(&paths);
        let card = loaded.cards.iter().find(|card| card.id == created.id).expect("the new instance");
        let bytes = card.icon_png.as_deref().expect("an icon was installed");
        assert_eq!(&bytes[..PNG_MAGIC.len()], &PNG_MAGIC[..]);

        // Every way this can come up empty is an empty box rather than a failure:
        // a builtin key, an empty key, and a file that is not a PNG at all.
        assert!(instance_icon(&paths, "default").is_none());
        assert!(instance_icon(&paths, "").is_none());
        std::fs::write(paths.icons_dir().join("bogus.png"), b"not an image").unwrap();
        assert!(instance_icon(&paths, "bogus").is_none());
    }

    #[test]
    fn memory_override_is_written_when_requested() {
        let (_dir, paths) = test_paths();
        let spec = NewInstance { max_mem_mb: Some(6144), ..NewInstance::vanilla("Memory", "26.2") };
        let created = create(&paths, &spec).unwrap();
        let instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        assert_eq!(instance.settings().get_i64("MaxMemAlloc", 0), 6144);
        assert!(instance.settings().get_bool("OverrideMemory", false));
        let model = crate::model::InstanceListModel::load(&paths).unwrap();
        assert_eq!(summarize(&paths.instances_dir(), &model.entries()[0]).max_mem_mb, 6144);
    }

    #[test]
    fn load_reports_cards_and_preselection() {
        let (_dir, paths) = test_paths();
        create(&paths, &fabric_spec("Beta", "1.21.1", "0.19.5")).unwrap();
        create(&paths, &NewInstance::vanilla("alpha", "26.2")).unwrap();
        std::fs::write(paths.global_config(), "[General]\nConfigVersion=1.3\nSelectedInstance=alpha\n")
            .unwrap();

        let loaded = load(&paths);
        let names: Vec<&str> = loaded.cards.iter().map(|card| card.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "Beta"]);
        assert_eq!(loaded.selected.as_deref(), Some("alpha"));
        assert!(loaded.status.contains("2 instance"));
        assert!(loaded.cards[0].problem.is_none());

        // A stale pre-selection is ignored.
        std::fs::write(paths.global_config(), "[General]\nConfigVersion=1.3\nSelectedInstance=Gone\n")
            .unwrap();
        assert!(load(&paths).selected.is_none());
    }

    #[test]
    fn the_header_s_three_facts_are_the_reference_s_own_words() {
        let mut card = InstanceCard {
            id: "x".into(),
            name: "x".into(),
            icon: "default".into(),
            icon_png: None,
            group: None,
            mc_version: "26.2".into(),
            loader: LoaderKind::Vanilla,
            loader_version: String::new(),
            playtime_secs: 0,
            last_launch_millis: 0,
            mods_total: 0,
            mods_enabled: 0,
            max_mem_mb: 4096,
            problem: None,
        };

        // The loader's *name*, then the game version: `loaderLabel` is
        // `[loaderDisplayName, game_version].filter(Boolean).join(' ')`, and no
        // loader build is in it.
        assert_eq!(card.loader_label(), "Vanilla 26.2");
        card.loader = LoaderKind::Fabric;
        card.loader_version = "0.19.5".into();
        assert_eq!(card.loader_label(), "Fabric 26.2", "the build is not the header's fact");
        card.mc_version.clear();
        assert_eq!(card.loader_label(), "Fabric", "an empty half is dropped, not padded");

        // The playtime counts down and says the largest unit it reaches, in words.
        assert_eq!(card.playtime_label(), "Never played");
        card.playtime_secs = 1;
        assert_eq!(card.playtime_label(), "1 second");
        card.playtime_secs = 90;
        assert_eq!(card.playtime_label(), "1 minute");
        card.playtime_secs = 3 * 3600 + 12 * 60;
        assert_eq!(card.playtime_label(), "3 hours", "and not 3 hours 12 minutes");

        // The clock has two arms: the relative age, or the one word.
        let now = 1_700_000_000_000;
        assert_eq!(last_played_label(0, now), "Never played");
        assert_eq!(last_played_label(now - 30_000, now), "Last played a few seconds ago");
        assert_eq!(last_played_label(now - 90_000, now), "Last played 1 minute ago");
        assert_eq!(last_played_label(now - 3 * 3_600_000, now), "Last played 3 hours ago");
        assert_eq!(last_played_label(now + 60_000, now), "Last played a few seconds ago");
    }

    #[test]
    fn the_header_s_clock_reads_the_last_launch_stamp() {
        let (_dir, paths) = test_paths();
        let created = create(&paths, &fabric_spec("Stamped", "1.21.1", "0.19.5")).unwrap();
        let model = crate::model::InstanceListModel::load(&paths).unwrap();
        let card = summarize(&paths.instances_dir(), &model.entries()[0]);
        assert_eq!(card.last_launch_millis, 0, "a pack that has never run has no stamp");

        let mut instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        instance.set_last_launch_millis(1_700_000_000_000);
        instance.save().unwrap();
        let card = summarize(&paths.instances_dir(), &model.entries()[0]);
        assert_eq!(card.last_launch_millis, 1_700_000_000_000);
    }

    #[test]
    fn scan_root_only_accepts_real_instances() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Real")).unwrap();
        std::fs::write(dir.path().join("Real").join("instance.cfg"), b"[General]\n").unwrap();
        std::fs::create_dir_all(dir.path().join("NotAnInstance")).unwrap();
        std::fs::write(dir.path().join("loose.txt"), b"x").unwrap();

        let found = scan_root(dir.path(), "Prism Launcher");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Real");
        assert_eq!(found[0].origin, "Prism Launcher");
        assert!(scan_root(&dir.path().join("missing"), "Prism Launcher").is_empty());
    }

    #[test]
    fn importing_copies_the_tree_and_uniquifies_the_id() {
        let (dir, paths) = test_paths();
        let source = dir.path().join("Borrowed");
        std::fs::create_dir_all(source.join("mods")).unwrap();
        std::fs::write(source.join("instance.cfg"), b"[General]\nname=Borrowed\n").unwrap();
        std::fs::write(source.join("mods").join("sodium.jar"), b"jar").unwrap();

        let id = import_instance(&paths, &source).unwrap();
        assert_eq!(id, "Borrowed");
        assert!(paths.instances_dir().join("Borrowed").join("mods").join("sodium.jar").is_file());
        let again = import_instance(&paths, &source).unwrap();
        assert_ne!(again, id);
        assert!(import_instance(&paths, &dir.path().join("nope")).is_err());
    }

    #[test]
    fn copying_into_the_source_is_refused_before_creating_directories() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        std::fs::create_dir_all(&source).unwrap();
        assert!(copy_dir_recursive(&source, &source).is_err());
        let nested = source.join("new/deep/copy");
        assert!(copy_dir_recursive(&source, &nested).is_err());
        assert!(!source.join("new").exists());
    }

    #[test]
    fn copy_dir_recursive_copies_trees() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"a").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), b"b").unwrap();
        let dst = dir.path().join("dst");
        copy_dir_recursive(&src, &dst).unwrap();
        assert_eq!(std::fs::read(dst.join("a.txt")).unwrap(), b"a");
        assert_eq!(std::fs::read(dst.join("sub").join("b.txt")).unwrap(), b"b");
        assert!(copy_dir_recursive(&dir.path().join("missing"), &dst).is_err());
    }

    #[test]
    fn find_importable_skips_ids_that_already_exist() {
        let (dir, paths) = test_paths();
        // A foreign launcher root holding one real instance and one stray
        // folder (no `instance.cfg`).
        let root = dir.path().join("BorrowedLauncher");
        let borrowed = root.join("Borrowed");
        std::fs::create_dir_all(&borrowed).unwrap();
        std::fs::write(borrowed.join("instance.cfg"), b"[General]\n").unwrap();
        std::fs::create_dir_all(root.join("stray-folder")).unwrap();

        let found = scan_root(&root, "Prism Launcher");
        assert_eq!(found.len(), 1, "a real instance is a folder with instance.cfg");
        assert_eq!(found[0].name, "Borrowed");
        assert_eq!(found[0].origin, "Prism Launcher");
        assert_eq!(scan_root(&root.join("missing"), "Prism Launcher"), Vec::new());

        let kept = skip_existing(found.clone(), &["SomethingElse".to_string()]);
        assert_eq!(kept.len(), 1);
        let skipped = skip_existing(found, &["Borrowed".to_string()]);
        assert!(skipped.is_empty(), "an id we already have is not offered again");

        // The machine-wide scan is a thin wrapper over the two above.
        let _ = find_importable(&paths);
    }
}
