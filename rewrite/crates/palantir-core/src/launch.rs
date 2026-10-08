//! Turning a version's metadata into an actual command line.
//!
//! The metadata says what to run; this module fills in who is running it.
//! Two shapes exist and both end in the same [`LaunchPlan`]:
//!
//! - the 2018+ shape: `arguments.jvm` and `arguments.game`, with rule-gated
//!   entries, plus (since 2026) `arguments.default-user-jvm` -- JVM tuning
//!   the metadata offers as *defaults*, which a launcher replaces when the
//!   person has configured memory or flags of their own. The plan keeps that
//!   group separate for exactly that reason;
//! - the older shape: `minecraftArguments`, one whitespace-separated string
//!   of game arguments. It carries no JVM list at all, so the launcher
//!   supplies the two JVM arguments the format leaves to it: the natives
//!   directory and the classpath, written in the format's own placeholders.
//!
//! Placeholder names are the format's (`${auth_player_name}` and friends);
//! their values are ours. A placeholder this module does not know is an
//! error naming it -- format growth must fail loudly here, not leak a
//! literal `${...}` into a running game.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::error::{Error, Result};
use crate::rules::{Platform, rules_allow};
use crate::version::{Argument, Version};

/// Everything the metadata cannot know: who is running this, and from where.
///
/// Every value is supplied by the caller; the defaults exist so a test (or a
/// future offline mode) can fill in only what it cares about.
#[derive(Debug, Clone, Default)]
pub struct LaunchContext {
    pub player_name: String,
    pub player_uuid: String,
    pub access_token: String,
    /// The Microsoft account's Xbox user id; empty offline.
    pub xuid: String,
    /// The application (client) id; empty offline.
    pub client_id: String,
    /// `msa` or `legacy`; the format passes it through to the game.
    pub user_type: String,
    /// The legacy account-properties string (a JSON object as text).
    pub user_properties: String,
    pub game_dir: PathBuf,
    pub assets_root: PathBuf,
    pub assets_index_name: String,
    pub version_name: String,
    pub version_type: String,
    pub natives_dir: PathBuf,
    pub launcher_name: String,
    pub launcher_version: String,
    pub library_dir: PathBuf,
    /// The classpath jars in order; joined with the host's separator.
    pub classpath: Vec<PathBuf>,
    /// `--width`/`--height`, when the caller has them (pair with the
    /// `has_custom_resolution` feature on the [`Platform`]).
    pub resolution: Option<(u32, u32)>,
    /// Quick-play targets (a world, server or realm to join at once).
    pub quick_play: QuickPlay,
    /// Where the logging configuration was downloaded, if it was.
    pub log_config_path: Option<PathBuf>,
}

/// The format's four quick-play slots; absent slots expand to empty strings,
/// which is how the game is told there is nothing to join.
#[derive(Debug, Clone, Default)]
pub struct QuickPlay {
    pub path: Option<String>,
    pub singleplayer: Option<String>,
    pub multiplayer: Option<String>,
    pub realms: Option<String>,
}

/// The finished pieces of a launch, already expanded.
///
/// The command assembles as: the java executable, then `default_jvm_args`
/// (the metadata's tuning defaults -- skip them when the person has their
/// own), then `jvm_args`, then `main_class`, then `game_args`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub default_jvm_args: Vec<String>,
    pub jvm_args: Vec<String>,
    pub main_class: String,
    pub game_args: Vec<String>,
}

impl LaunchPlan {
    /// The full command line for a given java executable.
    pub fn command(&self, java: &Path) -> Vec<String> {
        let mut command = Vec::with_capacity(
            2 + self.default_jvm_args.len() + self.jvm_args.len() + self.game_args.len(),
        );
        command.push(java.to_string_lossy().into_owned());
        command.extend(self.default_jvm_args.iter().cloned());
        command.extend(self.jvm_args.iter().cloned());
        command.push(self.main_class.clone());
        command.extend(self.game_args.iter().cloned());
        command
    }
}

/// The classpath separator the running OS expects (`;` on Windows, `:`
/// elsewhere) -- a fact of the platform's java, not of the metadata.
pub fn classpath_separator() -> &'static str {
    if cfg!(target_os = "windows") {
        ";"
    } else {
        ":"
    }
}

/// Resolve one version into a launch plan for this platform and context.
pub fn build_launch_plan(
    version: &Version,
    platform: &Platform,
    context: &LaunchContext,
) -> Result<LaunchPlan> {
    let vars = context.vars();
    let mut plan = LaunchPlan {
        default_jvm_args: Vec::new(),
        jvm_args: Vec::new(),
        main_class: version.main_class.clone(),
        game_args: Vec::new(),
    };

    if let Some(arguments) = &version.arguments {
        expand_list(
            &arguments.default_user_jvm,
            platform,
            &vars,
            &mut plan.default_jvm_args,
        )?;
        expand_list(&arguments.jvm, platform, &vars, &mut plan.jvm_args)?;
        expand_list(&arguments.game, platform, &vars, &mut plan.game_args)?;
    } else if let Some(legacy) = &version.minecraft_arguments {
        // The pre-2018 shape: the two JVM arguments the format leaves to
        // the launcher, then its one string of game arguments, split on
        // whitespace (the format quotes nothing).
        plan.jvm_args = vec![
            "-Djava.library.path=${natives_directory}".to_string(),
            "-cp".to_string(),
            "${classpath}".to_string(),
        ];
        plan.jvm_args = expand_all(&plan.jvm_args, &vars)?;
        plan.game_args = expand_all(
            &legacy
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>(),
            &vars,
        )?;
    } else {
        return Err(Error::Invalid {
            what: "version metadata",
            why: format!("{} carries neither argument shape", version.id),
        });
    }

    // The logging configuration is a JVM argument of its own, and its
    // `${path}` is the file the launcher downloaded.
    if let Some(logging) = version.logging.get("client") {
        if let Some(path) = &context.log_config_path {
            let mut with_path = vars.clone();
            with_path.insert("path".to_string(), path.to_string_lossy().into_owned());
            plan.jvm_args
                .insert(0, expand(&logging.argument, &with_path)?);
        }
    }

    Ok(plan)
}

impl LaunchContext {
    /// The placeholder table: the format's names to this context's values.
    fn vars(&self) -> BTreeMap<String, String> {
        let mut vars = BTreeMap::new();
        let mut put = |key: &str, value: String| {
            vars.insert(key.to_string(), value);
        };
        put("auth_player_name", self.player_name.clone());
        put("auth_uuid", self.player_uuid.clone());
        put("auth_access_token", self.access_token.clone());
        // The pre-2014 name for the same token (the 1.5.2 sample asks for
        // it); keeping both names on one value is what the format means.
        put("auth_session", self.access_token.clone());
        put("auth_xuid", self.xuid.clone());
        put("clientid", self.client_id.clone());
        put("user_type", self.user_type.clone());
        put("user_properties", self.user_properties.clone());
        put(
            "game_directory",
            self.game_dir.to_string_lossy().into_owned(),
        );
        put(
            "assets_root",
            self.assets_root.to_string_lossy().into_owned(),
        );
        // Pre-1.6 versions read assets from a `resources/` tree in the game
        // directory -- exactly what `map_to_resources` indexes populate --
        // and are handed that directory as their assets dir.
        put(
            "game_assets",
            self.game_dir
                .join("resources")
                .to_string_lossy()
                .into_owned(),
        );
        put("assets_index_name", self.assets_index_name.clone());
        put("version_name", self.version_name.clone());
        put("version_type", self.version_type.clone());
        put(
            "natives_directory",
            self.natives_dir.to_string_lossy().into_owned(),
        );
        put("launcher_name", self.launcher_name.clone());
        put("launcher_version", self.launcher_version.clone());
        put(
            "library_directory",
            self.library_dir.to_string_lossy().into_owned(),
        );
        let classpath: Vec<String> = self
            .classpath
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        put("classpath", classpath.join(classpath_separator()));
        put("classpath_separator", classpath_separator().to_string());
        let (width, height) = self.resolution.unwrap_or((0, 0));
        put("resolution_width", width.to_string());
        put("resolution_height", height.to_string());
        put(
            "quickPlayPath",
            self.quick_play.path.clone().unwrap_or_default(),
        );
        put(
            "quickPlaySingleplayer",
            self.quick_play.singleplayer.clone().unwrap_or_default(),
        );
        put(
            "quickPlayMultiplayer",
            self.quick_play.multiplayer.clone().unwrap_or_default(),
        );
        put(
            "quickPlayRealms",
            self.quick_play.realms.clone().unwrap_or_default(),
        );
        vars
    }
}

/// Expand one argument list in order, honoring each entry's rules.
fn expand_list(
    list: &[Argument],
    platform: &Platform,
    vars: &BTreeMap<String, String>,
    out: &mut Vec<String>,
) -> Result<()> {
    for argument in list {
        match argument {
            Argument::Plain(text) => out.push(expand(text, vars)?),
            Argument::Conditional { rules, value } => {
                if !rules_allow(rules, platform)? {
                    continue;
                }
                for text in value.strings() {
                    out.push(expand(text, vars)?);
                }
            }
        }
    }
    Ok(())
}

/// Expand every string in a list.
fn expand_all(list: &[String], vars: &BTreeMap<String, String>) -> Result<Vec<String>> {
    list.iter().map(|text| expand(text, vars)).collect()
}

/// Substitute `${name}` placeholders. An unknown name is an error naming it:
/// a placeholder leaking into a running game is worse than no launch at all.
fn expand(text: &str, vars: &BTreeMap<String, String>) -> Result<String> {
    let pattern = Regex::new(r"\$\{([a-zA-Z_][a-zA-Z_0-9]*)\}").map_err(|_| Error::Invalid {
        what: "launch argument",
        why: "the placeholder pattern itself failed to compile".to_string(),
    })?;
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for whole in pattern.find_iter(text) {
        out.push_str(&text[last..whole.start()]);
        let name = &text[whole.start() + 2..whole.end() - 1];
        let value = vars.get(name).ok_or_else(|| Error::Invalid {
            what: "launch argument",
            why: format!("unknown placeholder ${{{name}}} in {text:?}"),
        })?;
        out.push_str(value);
        last = whole.end();
    }
    out.push_str(&text[last..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> LaunchContext {
        LaunchContext {
            player_name: "Steve".to_string(),
            player_uuid: "uuid-1".to_string(),
            version_name: "1.0".to_string(),
            version_type: "release".to_string(),
            ..LaunchContext::default()
        }
    }

    fn platform() -> Platform {
        Platform::new(crate::rules::Os::Linux, "6.8.0", crate::rules::Arch::X86_64)
    }

    fn version(json: &str) -> Version {
        Version::parse(json).unwrap()
    }

    #[test]
    fn every_known_placeholder_substitutes() {
        let vars = context().vars();
        for key in [
            "auth_player_name",
            "auth_uuid",
            "auth_access_token",
            "auth_session",
            "game_assets",
            "auth_xuid",
            "clientid",
            "user_type",
            "user_properties",
            "game_directory",
            "assets_root",
            "assets_index_name",
            "version_name",
            "version_type",
            "natives_directory",
            "launcher_name",
            "launcher_version",
            "library_directory",
            "classpath",
            "classpath_separator",
            "resolution_width",
            "resolution_height",
            "quickPlayPath",
            "quickPlaySingleplayer",
            "quickPlayMultiplayer",
            "quickPlayRealms",
        ] {
            assert!(vars.contains_key(key), "no value wired for ${{{key}}}");
            let expanded = expand(&format!("${{{key}}}"), &vars).unwrap();
            assert!(!expanded.contains("${"), "{key} did not expand");
        }
    }

    #[test]
    fn an_unknown_placeholder_is_an_error_naming_it() {
        let vars = context().vars();
        let err = expand("--token ${from_the_future}", &vars).unwrap_err();
        assert!(err.to_string().contains("from_the_future"), "{err}");
    }

    #[test]
    fn modern_lists_expand_with_rules() {
        let version = version(
            r#"{"id": "1.0", "type": "release", "mainClass": "game.Main",
                "time": "t", "releaseTime": "r",
                "arguments": {
                    "jvm": ["-cp", "${classpath}"],
                    "game": ["--username", "${auth_player_name}",
                             {"rules": [{"action": "allow", "os": {"name": "osx"}}],
                              "value": "--mac-only"}]}}"#,
        );
        let plan = build_launch_plan(&version, &platform(), &context()).unwrap();
        assert_eq!(plan.main_class, "game.Main");
        assert_eq!(plan.jvm_args, vec!["-cp", ""]);
        assert_eq!(plan.game_args, vec!["--username", "Steve"]);
    }

    #[test]
    fn legacy_strings_split_and_expand() {
        let version = version(
            r#"{"id": "1.0", "type": "release", "mainClass": "game.Main",
                "time": "t", "releaseTime": "r",
                "minecraftArguments": "--username ${auth_player_name} --version ${version_name}"}"#,
        );
        let plan = build_launch_plan(&version, &platform(), &context()).unwrap();
        // The two JVM arguments the legacy format leaves to the launcher.
        assert_eq!(plan.jvm_args.len(), 3);
        assert_eq!(plan.jvm_args[1], "-cp");
        assert_eq!(
            plan.game_args,
            vec!["--username", "Steve", "--version", "1.0"]
        );
    }

    #[test]
    fn a_version_with_no_arguments_at_all_is_refused() {
        let version = version(
            r#"{"id": "1.0", "type": "release", "mainClass": "game.Main",
                "time": "t", "releaseTime": "r"}"#,
        );
        assert!(build_launch_plan(&version, &platform(), &context()).is_err());
    }

    #[test]
    fn the_logging_argument_gets_its_path() {
        let version = version(
            r#"{"id": "1.0", "type": "release", "mainClass": "game.Main",
                "time": "t", "releaseTime": "r",
                "arguments": {"jvm": ["-cp", "${classpath}"]},
                "logging": {"client": {
                    "argument": "-Dlog4j.configurationFile=${path}",
                    "file": {"id": "log.xml"},
                    "type": "log4j2-xml"}}}"#,
        );
        let mut context = context();
        context.log_config_path = Some(PathBuf::from("/data/log.xml"));
        let plan = build_launch_plan(&version, &platform(), &context).unwrap();
        assert_eq!(plan.jvm_args[0], "-Dlog4j.configurationFile=/data/log.xml");
    }

    #[test]
    fn command_orders_tuning_jvm_main_game() {
        let plan = LaunchPlan {
            default_jvm_args: vec!["-Xmx4G".to_string()],
            jvm_args: vec!["-cp".to_string(), "c".to_string()],
            main_class: "game.Main".to_string(),
            game_args: vec!["--demo".to_string()],
        };
        assert_eq!(
            plan.command(Path::new("/usr/bin/java")),
            vec!["/usr/bin/java", "-Xmx4G", "-cp", "c", "game.Main", "--demo"]
        );
    }
}
