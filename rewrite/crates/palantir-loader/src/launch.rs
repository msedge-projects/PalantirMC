//! Turning a resolved version into a running game: extract natives jars and
//! spawn the Java process with the classpath, JVM flags, and game args the
//! version names.
//!
//! The plan is built from the version document and a `LaunchContext` that
//! names where the install landed its files (the client jar, the library
//! root, the natives tree, the game and asset directories) and who is
//! playing. Building one resolves every argument the *format* leaves as a
//! placeholder (`${auth_player_name}`, `${classpath}`, ...) into the strings
//! the process will actually see -- so the invariant holds: once a plan is
//! built, no placeholder survives into the command.
//!
//! What is not here: the process supervision itself. `LaunchPlan::command`
//! hands the caller a `std::process::Command` it may spawn, log or inspect;
//! windowing and lifetime are `palantir-desktop`'s business.

use std::path::{Path, PathBuf};

use palantir_core::launch::{client_download, resolve_args};
use palantir_core::maven::MavenCoord;
use palantir_core::rules::{Os, Platform};
use palantir_core::version::Version;
use palantir_net::download::sha1_file;

use crate::error::{Error, Result};
use crate::installer::PlannedProcessor;

/// What the process needs to start.
#[derive(Debug, Clone)]
pub struct LaunchPlan {
    /// The Java binary to run.
    pub java: PathBuf,
    /// Working directory the process starts in (the game dir).
    pub dir: PathBuf,
    /// The main class the version names.
    pub main_class: String,
    /// The game (client) jar, the first entry on the classpath.
    pub client: PathBuf,
    /// The extracted natives tree this platform's version names.
    pub natives: Option<PathBuf>,
    /// The classpath: client jar first, then the resolved library jars in
    /// version order (loader shadows parent, per the merge rules).
    pub classpath: Vec<PathBuf>,
    /// The JVM arguments, placeholders already expanded.
    pub jvm_args: Vec<String>,
    /// The game arguments, placeholders already expanded.
    pub game_args: Vec<String>,
}

impl LaunchPlan {
    /// The command line, ready to spawn: java, the JVM args, the main class,
    /// the game args. Two things the *launcher* historically supplied by
    /// hand are supplied here when the version's own JVM list carries none:
    /// the classpath (`-cp`, every pre-2018 version) and where the natives
    /// live (`-Djava.library.path`, every pre-2018 version). A document
    /// that names either keeps its own value -- this never doubles up.
    pub fn command(&self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.java);
        command.current_dir(&self.dir);
        command.args(&self.jvm_args);
        if !self
            .jvm_args
            .iter()
            .any(|arg| arg == "-cp" || arg == "-classpath")
        {
            command.arg("-cp");
            command.arg(joined(&self.classpath, host_separator()));
        }
        if let Some(natives) = &self.natives {
            if !self
                .jvm_args
                .iter()
                .any(|arg| arg.starts_with("-Djava.library.path"))
            {
                command.arg(format!("-Djava.library.path={}", display(natives)));
            }
        }
        command.arg(&self.main_class);
        command.args(&self.game_args);
        command
    }
}

/// Where the install landed its files, and who is playing.
#[derive(Debug, Clone)]
pub struct LaunchContext {
    pub player_name: String,
    pub player_uuid: String,
    pub access_token: String,
    pub clientid: String,
    pub auth_xuid: String,
    /// `msa`, `mojang` or `legacy` -- the format passes it as `--userType`.
    pub user_type: String,
    pub launcher_name: String,
    pub launcher_version: String,
    /// Values for the feature-gated placeholders (`${resolution_width}`,
    /// `${quickPlayPath}`, ...): the format names them, the caller's settings
    /// fill the ones its enabled features actually use.
    pub placeholders: std::collections::BTreeMap<String, String>,
    /// The Java runtime binary to spawn.
    pub java: PathBuf,
    pub game_dir: PathBuf,
    pub assets_root: PathBuf,
    pub natives_dir: PathBuf,
    pub library_root: PathBuf,
    pub client_jar: PathBuf,
    /// Where a pre-2016 document's `${game_assets}` points: the asset root
    /// the game actually reads for this install -- the object store for a
    /// modern index, the `resources/` tree a `map_to_resources` index
    /// writes into, or a virtual index's tree. The document only names the
    /// placeholder; the caller knows the layout the install produced.
    pub game_assets: PathBuf,
}

impl Default for LaunchContext {
    fn default() -> Self {
        Self {
            player_name: "Player".to_string(),
            player_uuid: String::new(),
            access_token: String::new(),
            clientid: String::new(),
            auth_xuid: String::new(),
            user_type: "msa".to_string(),
            launcher_name: "palantir".to_string(),
            launcher_version: env!("CARGO_PKG_VERSION").to_string(),
            placeholders: std::collections::BTreeMap::new(),
            java: PathBuf::from("java"),
            game_dir: PathBuf::from(".minecraft"),
            assets_root: PathBuf::from(".minecraft/assets"),
            natives_dir: PathBuf::from(".minecraft/bin/natives"),
            library_root: PathBuf::from(".minecraft/libraries"),
            client_jar: PathBuf::from(".minecraft/versions/client/client.jar"),
            // The map_to_resources shape the pre-1.6 era itself used; an
            // install with another layout sets this when it builds context.
            game_assets: PathBuf::from(".minecraft/resources"),
        }
    }
}

/// Build the plan from a resolved version document and the on-disk locations
/// the install produced.
///
/// The classpath is derived from the version itself: its `libraries`
/// resolved for this platform (rules and all), each jar at its Maven layout
/// under the library root. The caller does not recount what the version
/// already says.
pub fn build_launch_plan(
    version: &Version,
    platform: &Platform,
    context: &LaunchContext,
) -> Result<LaunchPlan> {
    // A runnable version names a client download for this platform; where the
    // jar lives on disk is what the install decided, and the context knows it.
    client_download(version, platform).ok_or_else(|| Error::Invalid {
        what: "version downloads",
        why: "no client download for this platform".to_string(),
    })?;

    let mut classpath = vec![context.client_jar.clone()];
    for library in &version.libraries {
        let Some(resolved) = library.resolve(platform)? else {
            continue;
        };
        if let Some(artifact) = resolved.artifact {
            let entry = context.library_root.join(artifact.rel_path);
            // Documents repeat entries (overlays re-list their game's
            // libraries); one file belongs on the classpath once, exactly
            // as the syncer's jobs absorb the repetition on the way in.
            if !classpath.contains(&entry) {
                classpath.push(entry);
            }
        }
    }

    let (jvm, game) = argument_lists(version, platform);
    let separator = if platform.os == Os::Windows { ';' } else { ':' };
    let classpath_string = joined(&classpath, separator);

    let expand =
        |text: &str| expand_placeholders(text, version, context, &classpath_string, separator);

    Ok(LaunchPlan {
        java: context.java.clone(),
        dir: context.game_dir.clone(),
        main_class: version.main_class.clone(),
        client: context.client_jar.clone(),
        natives: Some(context.natives_dir.clone()),
        classpath,
        jvm_args: jvm.iter().map(|arg| expand(arg)).collect(),
        game_args: game.iter().map(|arg| expand(arg)).collect(),
    })
}

/// Extract one natives jar into `dest`, skipping the paths the library's
/// `extract` block names (META-INF and friends never belong in a natives
/// tree).
pub fn extract_natives(jar: &Path, dest: &Path, exclude: &[String]) -> Result<PathBuf> {
    std::fs::create_dir_all(dest).map_err(|source| Error::Io {
        path: dest.to_path_buf(),
        source,
    })?;

    let file = std::fs::File::open(jar).map_err(|source| Error::Io {
        path: jar.to_path_buf(),
        source,
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|source| Error::Io {
        path: jar.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
    })?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|source| Error::Io {
            path: jar.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
        })?;
        let name = entry.name().to_string();
        if excluded(&name, exclude) {
            continue;
        }
        let out = dest.join(&name);
        if name.ends_with('/') {
            std::fs::create_dir_all(&out).map_err(|source| Error::Io {
                path: out.clone(),
                source,
            })?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let mut file = std::fs::File::create(&out).map_err(|source| Error::Io {
            path: out.clone(),
            source,
        })?;
        std::io::copy(&mut entry, &mut file).map_err(|source| Error::Io { path: out, source })?;
    }

    Ok(dest.to_path_buf())
}

/// The argument lists as strings, rules already applied. A pre-2018 version
/// carries no JVM list at all and one whitespace-separated game string; the
/// format's own split is the one wanted.
fn argument_lists(version: &Version, platform: &Platform) -> (Vec<String>, Vec<String>) {
    if version.arguments.is_none() {
        let game = version
            .minecraft_arguments
            .as_deref()
            .map(|text| text.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        return (Vec::new(), game);
    }
    let (jvm, game, _default_user_jvm) = resolve_args(version, platform).strings();
    (jvm, game)
}

/// Replace the placeholders the format defines with the values this launch
/// provides. A placeholder this launch cannot know (the optional feature
/// flags' dimensions, say) is left alone; anything the process needs is in
/// the table.
fn expand_placeholders(
    text: &str,
    version: &Version,
    context: &LaunchContext,
    classpath: &str,
    separator: char,
) -> String {
    let index_name = version
        .asset_index
        .as_ref()
        .map(|index| index.id.clone())
        .or_else(|| version.assets.clone())
        .unwrap_or_default();

    let table: [(&str, String); 19] = [
        ("${natives_directory}", display(&context.natives_dir)),
        ("${launcher_name}", context.launcher_name.clone()),
        ("${launcher_version}", context.launcher_version.clone()),
        ("${library_directory}", display(&context.library_root)),
        ("${classpath}", classpath.to_string()),
        ("${auth_player_name}", context.player_name.clone()),
        ("${auth_uuid}", context.player_uuid.clone()),
        ("${auth_access_token}", context.access_token.clone()),
        ("${clientid}", context.clientid.clone()),
        ("${auth_xuid}", context.auth_xuid.clone()),
        ("${user_type}", context.user_type.clone()),
        ("${version_name}", version.id.clone()),
        ("${version_type}", version.kind.clone()),
        ("${game_directory}", display(&context.game_dir)),
        ("${assets_root}", display(&context.assets_root)),
        ("${assets_index_name}", index_name),
        // The format joins path lists with the platform's own separator;
        // the Forge family's module path (`-p`) is what uses it.
        ("${classpath_separator}", separator.to_string()),
        // The pre-2016 spellings, from the single-string era: `--session`
        // carried the same token later passed as `accessToken`, and
        // `--assetsDir ${game_assets}` names the asset root this install
        // presents to the game.
        ("${auth_session}", context.access_token.clone()),
        ("${game_assets}", display(&context.game_assets)),
    ];

    let mut out = text.to_string();
    for (placeholder, value) in table {
        out = out.replace(placeholder, &value);
    }
    for (name, value) in &context.placeholders {
        out = out.replace(&format!("${{{name}}}"), value);
    }
    out
}

fn excluded(name: &str, exclude: &[String]) -> bool {
    let stripped = name.strip_suffix('/').unwrap_or(name);
    exclude.iter().any(|pattern| {
        stripped == pattern
            || stripped.starts_with(&format!("{pattern}/"))
            || stripped.starts_with(&format!("{pattern}\\"))
    })
}

fn joined(paths: &[PathBuf], separator: char) -> String {
    paths
        .iter()
        .map(|path| display(path))
        .collect::<Vec<_>>()
        .join(&separator.to_string())
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
fn host_separator() -> char {
    if cfg!(windows) { ';' } else { ':' }
}

// ---- Forge-family processors ----
//
// A Forge or NeoForge install is not finished when its files land: its
// install profile names processors -- plain executable jars -- that patch
// the game jar and must run before the version is launchable. `installer.rs`
// plans them; running the plan is this half of the launch slice.

/// What a processor run did.
#[derive(Debug, Default)]
pub struct ProcessorReport {
    /// Jar coordinates that ran, in plan order.
    pub ran: Vec<String>,
    /// Jar coordinates skipped: every promised artifact was already at its
    /// promised hash, so the processor had run before.
    pub skipped: Vec<String>,
}

/// The command headless Java will be asked to run for one processor: its jar
/// and classpath on `-cp`, the jar's manifest `Main-Class`, then the plan's
/// already-expanded args.
pub fn processor_command(
    java: &Path,
    library_dir: &Path,
    planned: &PlannedProcessor,
) -> Result<std::process::Command> {
    let jar = artifact_path(library_dir, &planned.jar)?;
    let mut classpath = vec![jar.clone()];
    for coordinate in &planned.classpath {
        classpath.push(artifact_path(library_dir, coordinate)?);
    }
    let joined = std::env::join_paths(&classpath).map_err(|source| Error::Invalid {
        what: "processor classpath",
        why: source.to_string(),
    })?;

    let mut command = std::process::Command::new(java);
    command
        .arg("-cp")
        .arg(joined)
        .arg(jar_main_class(&jar)?)
        .args(&planned.args);
    Ok(command)
}

/// Run the planned processors in order with `java`. A processor whose
/// outputs already sit at their promised hashes has run before and is
/// skipped; one that runs must leave its promised artifacts behind, or the
/// install would fail much later, somewhere else.
pub fn run_processors(
    java: &Path,
    library_dir: &Path,
    plan: &[PlannedProcessor],
) -> Result<ProcessorReport> {
    let mut report = ProcessorReport::default();
    for planned in plan {
        if !planned.outputs.is_empty() && receipts_hold(&planned.outputs)? {
            report.skipped.push(planned.jar.clone());
            continue;
        }
        let status = processor_command(java, library_dir, planned)?
            .status()
            .map_err(|source| Error::Io {
                path: java.to_path_buf(),
                source,
            })?;
        if !status.success() {
            return Err(Error::Processor {
                jar: planned.jar.clone(),
                why: format!("headless Java exited with {status}"),
            });
        }
        if !receipts_hold(&planned.outputs)? {
            return Err(Error::Processor {
                jar: planned.jar.clone(),
                why: "its promised artifacts are not there at their promised hashes".to_string(),
            });
        }
        report.ran.push(planned.jar.clone());
    }
    Ok(report)
}

/// Do the promised artifacts sit at their promised hashes? The install
/// profile calls these the skip receipts: when they hold, the processor has
/// already run.
fn receipts_hold(outputs: &[(PathBuf, String)]) -> Result<bool> {
    for (artifact, promised) in outputs {
        if !artifact.is_file() {
            return Ok(false);
        }
        let actual = sha1_file(artifact).map_err(|source| Error::Net { source })?;
        if !actual.eq_ignore_ascii_case(promised) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// One Maven coordinate's file under the library root.
fn artifact_path(library_dir: &Path, coordinate: &str) -> Result<PathBuf> {
    Ok(library_dir.join(MavenCoord::parse(coordinate)?.rel_path()))
}

/// The entry point a processor jar names in its manifest. Manifest values
/// wrap at 72 bytes with a leading space on the continuation line, and a
/// vendor jar may well wrap this one.
fn jar_main_class(jar: &Path) -> Result<String> {
    let file = std::fs::File::open(jar).map_err(|source| Error::Io {
        path: jar.to_path_buf(),
        source,
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|source| Error::Io {
        path: jar.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
    })?;
    let mut manifest = archive
        .by_name("META-INF/MANIFEST.MF")
        .map_err(|source| Error::Io {
            path: jar.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
        })?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut manifest, &mut text).map_err(|source| Error::Io {
        path: jar.to_path_buf(),
        source,
    })?;

    let unfolded = unfold_manifest(&text);
    for line in unfolded.lines() {
        if let Some(value) = line.strip_prefix("Main-Class:") {
            return Ok(value.trim().to_string());
        }
    }
    Err(Error::Invalid {
        what: "processor jar",
        why: format!("{} names no Main-Class", jar.display()),
    })
}

/// Join manifest continuation lines -- a leading space continues the
/// previous logical line -- so one logical line is one string.
fn unfold_manifest(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(' ') {
            out.push_str(rest);
        } else {
            out.push('\n');
            out.push_str(line);
        }
    }
    out
}
