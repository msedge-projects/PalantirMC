//! Pure launch-plan builders — port of the argument/script generation in
//! `MinecraftInstance.cpp` (+ `Commandline::splitArgs`). No process is
//! spawned here; `palantir-net`/the GUI turn a [`LaunchArgs`] into a command.

use crate::version::LaunchProfile;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Authentication data injected into the launch arguments
/// (`AuthSession` subset used by argument generation).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthSession {
    /// Player name (`auth_player_name`).
    pub player_name: String,
    /// Player UUID (`auth_uuid`).
    pub uuid: String,
    /// Access token (`auth_access_token`).
    pub access_token: String,
    /// Legacy session id (`auth_session`).
    pub session: String,
    /// `user_type` (`msa` / `legacy` / ...).
    pub user_type: String,
    /// Serialized user properties (`user_properties`).
    pub user_properties: String,
    /// Launch in demo mode (`--demo`).
    pub demo: bool,
}

/// Server/world to join at launch (`MinecraftTarget`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchTarget {
    /// Server address (empty = no server join).
    pub address: String,
    /// Server port.
    pub port: u16,
    /// Quick-play world name (empty = none).
    pub world: String,
}

/// Window parameters for the launch script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowParams {
    /// Window width.
    pub width: i64,
    /// Window height.
    pub height: i64,
    /// Launch maximized (legacy instances print `maximized`).
    pub maximized: bool,
}

/// Inputs for [`java_arguments`], mirroring the settings/traits read in
/// `MinecraftInstance::javaArguments` and `extraArguments`.
#[derive(Debug, Clone)]
pub struct JavaArgsOptions {
    /// Profile traits (checked for `FirstThreadOnMacOS`).
    pub traits: BTreeSet<String>,
    /// `JvmArgs` setting (split with [`split_command_args`]).
    pub jvm_args: String,
    /// `MinMemAlloc` in MiB.
    pub min_mem: i64,
    /// `MaxMemAlloc` in MiB.
    pub max_mem: i64,
    /// `PermGen` in MiB.
    pub perm_gen: i64,
    /// Whether jar mods are present (adds the FML ignore flags).
    pub jar_mods_present: bool,
    /// Profile `+jvmArgs` (token-replaced).
    pub addn_jvm_arguments: Vec<String>,
    /// Agents as `(jar_path, argument)` pairs.
    pub agents: Vec<(String, String)>,
    /// Major version of the selected Java runtime.
    pub java_major: i64,
    /// Whether the `legacyServices` online-fixes flag applies.
    pub online_fixes: bool,
    /// Host platform.
    pub platform: crate::paths::System,
    /// Linux: whether `xrandr` exists (disables the LWJGL2 workaround).
    pub xrandr_available: bool,
    /// Linux: whether the `org.lwjgl` (LWJGL2) component is present.
    pub has_lwjgl2: bool,
    /// Custom/system OpenAL path (macOS/Linux native workarounds).
    pub native_openal: Option<String>,
    /// Custom/system GLFW path.
    pub native_glfw: Option<String>,
    /// Custom/system SDL path.
    pub native_sdl: Option<String>,
    /// Instance window title (macOS dock name).
    pub window_title: String,
    /// Token map for `+jvmArgs` substitution.
    pub token_map: BTreeMap<String, String>,
}

/// Substitute `${token}` occurrences. Unknown tokens are kept verbatim
/// (`replaceTokensIn`); matching is non-greedy, i.e. `${a}${b}` resolves
/// both separately.
pub fn replace_tokens(text: &str, map: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                // Non-greedy nesting: if the key itself contains a nested
                // `${`, the outer `${` is not a real token start. Emit it
                // literally and rescan so the inner `${b}` resolves first.
                // `${a${b}}` -> `${` + `a` + `B` + `}` = `${aB}`.
                if key.contains("${") {
                    out.push_str("${");
                    rest = after;
                    continue;
                }
                match map.get(key) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push_str("${");
                        out.push_str(key);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str("${");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Shell-like argument splitting (`Commandline::splitArgs`): double quotes
/// toggle quoting and are removed; whitespace outside quotes splits.
pub fn split_command_args(args: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quoted = false;
    for c in args.chars() {
        if c == '"' {
            in_quoted = !in_quoted;
        } else if !in_quoted && c.is_whitespace() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// The profile variable mapping (`makeProfileVarMapping`).
pub fn profile_var_map(
    profile: &LaunchProfile,
    instance_name: &str,
    instance_id: &str,
    _instance_root: &Path,
    game_root: &Path,
    game_assets: &Path,
    assets_root: &Path,
    library_directory: &Path,
) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("profile_name".into(), instance_name.to_string());
    m.insert("version_name".into(), profile.minecraft_version.clone());
    m.insert("version_type".into(), profile.minecraft_version_type.clone());
    m.insert("game_directory".into(), game_root.to_string_lossy().into_owned());
    m.insert("game_assets".into(), game_assets.to_string_lossy().into_owned());
    m.insert("assets_root".into(), assets_root.to_string_lossy().into_owned());
    m.insert("assets_index_name".into(), profile.assets_or_default().id);
    m.insert("library_directory".into(), library_directory.to_string_lossy().into_owned());
    let _ = instance_id; // kept in the signature to mirror the C++ context
    m
}

/// `--username ${auth_player_name} ...` style argument list
/// (`processMinecraftArgs`). `vars` must already contain the profile
/// mapping; session tokens are added here.
pub fn process_minecraft_args(
    profile: &LaunchProfile,
    session: Option<&AuthSession>,
    target: Option<&LaunchTarget>,
    vars: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut args: Vec<String> = profile
        .minecraft_arguments
        .split(' ')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    for tweaker in &profile.tweakers {
        args.push("--tweakClass".into());
        args.push(tweaker.clone());
    }
    if let Some(target) = target {
        if !target.address.is_empty() {
            if profile.has_trait("feature:is_quick_play_multiplayer") {
                args.push("--quickPlayMultiplayer".into());
                args.push(format!("{}:{}", target.address, target.port));
            } else {
                args.push("--server".into());
                args.push(target.address.clone());
                args.push("--port".into());
                args.push(target.port.to_string());
            }
        } else if !target.world.is_empty() && profile.has_trait("feature:is_quick_play_singleplayer") {
            args.push("--quickPlaySingleplayer".into());
            args.push(target.world.clone());
        }
    }
    let mut map = vars.clone();
    if let Some(session) = session {
        map.insert("auth_session".into(), session.session.clone());
        map.insert("auth_access_token".into(), session.access_token.clone());
        map.insert("auth_player_name".into(), session.player_name.clone());
        map.insert("auth_uuid".into(), session.uuid.clone());
        map.insert("user_properties".into(), session.user_properties.clone());
        map.insert("user_type".into(), session.user_type.clone());
        if session.demo {
            args.push("--demo".into());
        }
    }
    args.iter().map(|a| replace_tokens(a, &map)).collect()
}

/// The `INST_*` environment variables Prism exports
/// (`MinecraftInstance::getVariables`).
pub fn instance_env_vars(
    instance_name: &str,
    instance_id: &str,
    instance_root: &Path,
    game_root: &Path,
    java_path: &Path,
    java_args_joined: &str,
) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("INST_NAME".into(), instance_name.to_string());
    m.insert("INST_ID".into(), instance_id.to_string());
    m.insert("INST_DIR".into(), instance_root.to_string_lossy().into_owned());
    m.insert("INST_MC_DIR".into(), game_root.to_string_lossy().into_owned());
    m.insert("INST_JAVA".into(), java_path.to_string_lossy().into_owned());
    m.insert("INST_JAVA_ARGS".into(), java_args_joined.to_string());
    m.insert("NO_COLOR".into(), "1".to_string());
    m
}

/// Build the JVM argument list (`javaArguments` + `extraArguments`).
/// Argument order matches the C++ exactly.
pub fn java_arguments(opts: &JavaArgsOptions) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    args.push("-Duser.language=en".into());
    // extraArguments():
    args.extend(split_command_args(&opts.jvm_args));
    if opts.jar_mods_present {
        args.push("-Dfml.ignoreInvalidMinecraftCertificates=true".into());
        args.push("-Dfml.ignorePatchDiscrepancies=true".into());
    }
    for arg in &opts.addn_jvm_arguments {
        args.push(replace_tokens(arg, &opts.token_map));
    }
    for (jar, arg) in &opts.agents {
        let mut a = format!("-javaagent:{jar}");
        if !arg.is_empty() {
            a.push('=');
            a.push_str(arg);
        }
        args.push(a);
    }
    for (flag, path) in [
        ("-Dorg.lwjgl.openal.libname=", &opts.native_openal),
        ("-Dorg.lwjgl.glfw.libname=", &opts.native_glfw),
        ("-Dorg.lwjgl.sdl.libname=", &opts.native_sdl),
    ] {
        if let Some(p) = path {
            if !p.is_empty() {
                args.push(format!("{flag}{p}"));
            }
        }
    }
    // javaArguments() platform workarounds:
    if opts.platform == crate::paths::System::MacOS {
        args.push("-Xdock:icon=icon.png".into());
        args.push(format!("-Xdock:name=\"{}\"", opts.window_title));
        if opts.traits.contains("FirstThreadOnMacOS") {
            args.push("-XstartOnFirstThread".into());
        }
    }
    if opts.platform == crate::paths::System::Windows {
        args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".into());
    }
    if opts.platform == crate::paths::System::Linux && opts.has_lwjgl2 && !opts.xrandr_available {
        args.push("-DLWJGL_DISABLE_XRANDR=true".into());
    }
    let (min, max) = (opts.min_mem, opts.max_mem);
    if min < max {
        args.push(format!("-Xms{min}m"));
        args.push(format!("-Xmx{max}m"));
    } else {
        args.push(format!("-Xms{max}m"));
        args.push(format!("-Xmx{min}m"));
    }
    if opts.java_major < 8 {
        // requiresPermGen; the default (64) emits no flag
        if opts.perm_gen != 64 {
            args.push(format!("-XX:PermSize={}m", opts.perm_gen));
        }
    }
    if opts.java_major >= 9 && opts.online_fixes {
        args.push("--add-opens".into());
        args.push("java.base/java.net=ALL-UNNAMED".into());
    }
    args
}

/// Generate the Prism launch-script text
/// (`MinecraftInstance::createLaunchScript`): a line-based protocol consumed
/// by the external launcher parts.
pub fn create_launch_script(
    profile: &LaunchProfile,
    session: Option<&AuthSession>,
    target: Option<&LaunchTarget>,
    mc_args: &[String],
    window: WindowParams,
    launcher_brand: &str,
    launcher_version: &str,
    instance_name: &str,
) -> String {
    let mut out = String::new();
    if !profile.main_class.is_empty() {
        out.push_str(&format!("mainClass {}\n", profile.main_class));
    }
    if !profile.applet_class.is_empty() {
        out.push_str(&format!("appletClass {}\n", profile.applet_class));
    }
    if let Some(target) = target {
        if !target.address.is_empty() {
            out.push_str(&format!("serverAddress {}\n", target.address));
            out.push_str(&format!("serverPort {}\n", target.port));
        } else if !target.world.is_empty() {
            out.push_str(&format!("worldName {}\n", target.world));
        }
    }
    for param in mc_args {
        out.push_str(&format!("param {param}\n"));
    }
    out.push_str(&format!("windowTitle {launcher_brand}: {instance_name}\n"));
    let window_params = if window.maximized && profile.is_legacy() {
        "maximized".to_string()
    } else {
        format!("{}x{}", window.width, window.height)
    };
    out.push_str(&format!("windowParams {window_params}\n"));
    out.push_str(&format!("launcherBrand {launcher_brand}\n"));
    out.push_str(&format!("launcherVersion {launcher_version}\n"));
    out.push_str(&format!("instanceName {instance_name}\n"));
    // Quirk faithfully ported: the C++ writes the instance *name* here too.
    out.push_str(&format!("instanceIconKey {instance_name}\n"));
    out.push_str("instanceIconPath icon.png\n");
    if let Some(session) = session {
        out.push_str(&format!("userName {}\n", session.player_name));
        out.push_str(&format!("sessionId {}\n", session.session));
    }
    for t in &profile.traits {
        out.push_str(&format!("traits {t}\n"));
    }
    if profile.has_trait("legacyServices") {
        out.push_str("onlineFixes true\n");
    }
    let launcher = if profile.is_legacy() { "legacy" } else { "standard" };
    out.push_str(&format!("launcher {launcher}\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::{AssetIndexInfo, Library};

    fn profile() -> LaunchProfile {
        LaunchProfile {
            minecraft_version: "1.20.4".into(),
            minecraft_version_type: "release".into(),
            minecraft_assets: Some(AssetIndexInfo::bare("17")),
            minecraft_arguments: "--username ${auth_player_name} --version ${version_name} --gameDir ${game_directory}".into(),
            main_class: "net.minecraft.client.main.Main".into(),
            ..Default::default()
        }
    }

    fn vars() -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("version_name".to_string(), "1.20.4".into());
        m.insert("game_directory".to_string(), "/inst/minecraft".into());
        m
    }

    #[test]
    fn token_replacement_is_non_greedy_and_keeps_unknowns() {
        let mut map = BTreeMap::new();
        map.insert("a".to_string(), "A".into());
        map.insert("b".to_string(), "B".into());
        assert_eq!(replace_tokens("${a}${b}", &map), "AB");
        assert_eq!(replace_tokens("x${nope}y", &map), "x${nope}y");
        assert_eq!(replace_tokens("unclosed ${a", &map), "unclosed ${a");
        assert_eq!(replace_tokens("plain", &map), "plain");
        assert_eq!(replace_tokens("${a}", &map), "A");
        assert_eq!(replace_tokens("${a${b}}", &map), "${aB}"); // first } closes
    }

    #[test]
    fn split_args_handles_quotes_like_commandline() {
        assert_eq!(split_command_args("-Xmx1G -Dfoo=bar"), vec!["-Xmx1G", "-Dfoo=bar"]);
        assert_eq!(split_command_args("-Xms\"1 2G\" -Xmx4G"), vec!["-Xms1 2G", "-Xmx4G"]);
        assert_eq!(split_command_args("   "), Vec::<String>::new());
        assert_eq!(split_command_args("\"a b\"c"), vec!["a bc"]);
        assert_eq!(split_command_args("-X\"m\"x"), vec!["-Xmx"]);
    }

    #[test]
    fn minecraft_args_expand_tokens_tweakers_and_join_target() {
        let p = profile();
        let session = AuthSession {
            player_name: "Steve".into(),
            uuid: "uuid-1".into(),
            access_token: "tok".into(),
            session: "sess".into(),
            user_type: "msa".into(),
            user_properties: "{}".into(),
            demo: false,
        };
        let target = LaunchTarget { address: "mc.example.com".into(), port: 25565, world: String::new() };
        let args = process_minecraft_args(&p, Some(&session), Some(&target), &vars());
        assert!(args.contains(&"--username".to_string()) && args.contains(&"Steve".to_string()));
        assert!(args.contains(&"1.20.4".to_string()));
        assert!(args.contains(&"/inst/minecraft".to_string()));
        assert_eq!(*args.iter().rev().nth(3).unwrap(), "--server");
        assert_eq!(*args.last().unwrap(), "25565");
    }

    #[test]
    fn quick_play_used_when_trait_present() {
        let mut p = profile();
        p.traits.insert("feature:is_quick_play_multiplayer".into());
        let target = LaunchTarget { address: "s".into(), port: 1234, world: String::new() };
        let args = process_minecraft_args(&p, None, Some(&target), &vars());
        assert!(args.windows(2).any(|w| w[0] == "--quickPlayMultiplayer" && w[1] == "s:1234"));
        let mut p = profile();
        p.traits.insert("feature:is_quick_play_singleplayer".into());
        let target = LaunchTarget { address: String::new(), port: 0, world: "World".into() };
        let args = process_minecraft_args(&p, None, Some(&target), &vars());
        assert!(args.windows(2).any(|w| w[0] == "--quickPlaySingleplayer" && w[1] == "World"));
    }

    #[test]
    fn demo_flag_added_for_demo_sessions() {
        let session = AuthSession { demo: true, ..Default::default() };
        let args = process_minecraft_args(&profile(), Some(&session), None, &vars());
        assert!(args.contains(&"--demo".to_string()));
    }

    #[test]
    fn jvm_args_match_prism_order_and_defaults() {
        let opts = JavaArgsOptions {
            traits: BTreeSet::new(),
            jvm_args: "-XX:+UseG1GC".into(),
            min_mem: 128,
            max_mem: 4096,
            perm_gen: 64,
            jar_mods_present: false,
            addn_jvm_arguments: vec!["-Dcustom=${version_name}".into()],
            agents: vec![("/lib/agent.jar".into(), "opt".into())],
            java_major: 17,
            online_fixes: false,
            platform: crate::paths::System::Linux,
            xrandr_available: true,
            has_lwjgl2: true,
            native_openal: None,
            native_glfw: None,
            native_sdl: None,
            window_title: "PalantirMC: T".into(),
            token_map: vars(),
        };
        let args = java_arguments(&opts);
        let expected = vec![
            "-Duser.language=en",
            "-XX:+UseG1GC",
            "-Dcustom=1.20.4",
            "-javaagent:/lib/agent.jar=opt",
            "-Xms128m",
            "-Xmx4096m",
        ];
        assert_eq!(args, expected);
    }

    #[test]
    fn jvm_memory_swaps_when_min_ge_max() {
        let mut opts = base_opts(crate::paths::System::Linux);
        opts.min_mem = 4096;
        opts.max_mem = 4096;
        let args = java_arguments(&opts);
        assert!(args.contains(&"-Xms4096m".to_string()) && args.contains(&"-Xmx4096m".to_string()));
        opts.min_mem = 8192;
        opts.max_mem = 1024;
        let args = java_arguments(&opts);
        assert!(args.contains(&"-Xms1024m".to_string()) && args.contains(&"-Xmx8192m".to_string()));
    }

    fn base_opts(platform: crate::paths::System) -> JavaArgsOptions {
        JavaArgsOptions {
            traits: BTreeSet::new(),
            jvm_args: String::new(),
            min_mem: 128,
            max_mem: 4096,
            perm_gen: 64,
            jar_mods_present: false,
            addn_jvm_arguments: vec![],
            agents: vec![],
            java_major: 17,
            online_fixes: false,
            platform,
            xrandr_available: true,
            has_lwjgl2: false,
            native_openal: None,
            native_glfw: None,
            native_sdl: None,
            window_title: "T".into(),
            token_map: BTreeMap::new(),
        }
    }

    #[test]
    fn platform_workarounds_applied_conditionally() {
        let mut mac = base_opts(crate::paths::System::MacOS);
        mac.traits.insert("FirstThreadOnMacOS".into());
        let mac_args = java_arguments(&mac);
        assert!(mac_args.contains(&"-Xdock:icon=icon.png".to_string()));
        assert!(mac_args.contains(&"-Xdock:name=\"T\"".to_string()));
        assert!(mac_args.contains(&"-XstartOnFirstThread".to_string()));
        // trait absent -> no flag
        let mut mac2 = base_opts(crate::paths::System::MacOS);
        mac2.window_title = "W".into();
        assert!(!java_arguments(&mac2).contains(&"-XstartOnFirstThread".to_string()));

        let win = java_arguments(&base_opts(crate::paths::System::Windows));
        assert!(win.iter().any(|a| a.starts_with("-XX:HeapDumpPath=MojangTricks")));

        let mut linux = base_opts(crate::paths::System::Linux);
        linux.has_lwjgl2 = true;
        linux.xrandr_available = false;
        assert!(java_arguments(&linux).contains(&"-DLWJGL_DISABLE_XRANDR=true".to_string()));
        linux.xrandr_available = true;
        assert!(!java_arguments(&linux).contains(&"-DLWJGL_DISABLE_XRANDR=true".to_string()));
    }

    #[test]
    fn perm_gen_emitted_only_for_old_java_and_non_default() {
        let mut old = base_opts(crate::paths::System::Linux);
        old.java_major = 7;
        assert!(!java_arguments(&old).iter().any(|a| a.starts_with("-XX:PermSize"))); // default 64
        old.perm_gen = 128;
        assert!(java_arguments(&old).contains(&"-XX:PermSize=128m".to_string()));
        old.java_major = 8;
        assert!(!java_arguments(&old).iter().any(|a| a.starts_with("-XX:PermSize")));
    }

    #[test]
    fn online_fixes_add_opens_on_modular_java() {
        let mut opts = base_opts(crate::paths::System::Linux);
        opts.online_fixes = true;
        opts.java_major = 17;
        let a = java_arguments(&opts);
        assert!(a.windows(2).any(|w| w[0] == "--add-opens" && w[1] == "java.base/java.net=ALL-UNNAMED"));
        opts.java_major = 7;
        assert!(!java_arguments(&opts).contains(&"--add-opens".to_string()));
    }

    #[test]
    fn jar_mod_flags_and_native_paths() {
        let mut opts = base_opts(crate::paths::System::Linux);
        opts.jar_mods_present = true;
        opts.native_openal = Some("/usr/lib/libopenal.so".into());
        let a = java_arguments(&opts);
        assert!(a.contains(&"-Dfml.ignoreInvalidMinecraftCertificates=true".to_string()));
        assert!(a.contains(&"-Dfml.ignorePatchDiscrepancies=true".to_string()));
        assert!(a.contains(&"-Dorg.lwjgl.openal.libname=/usr/lib/libopenal.so".to_string()));
    }

    #[test]
    fn launch_script_matches_line_protocol() {
        let mut p = profile();
        p.traits.insert("first".into());
        p.traits.insert("second".into());
        let session = AuthSession { player_name: "Steve".into(), session: "sess".into(), ..Default::default() };
        let mc_args = process_minecraft_args(&p, Some(&session), None, &vars());
        let script = create_launch_script(
            &p,
            Some(&session),
            None,
            &mc_args,
            WindowParams { width: 854, height: 480, maximized: false },
            crate::PRODUCT_NAME,
            "9.0",
            "My Inst",
        );
        let lines: Vec<&str> = script.lines().collect();
        assert_eq!(lines[0], "mainClass net.minecraft.client.main.Main");
        assert!(script.contains("param --username\nparam Steve\n"));
        // The title carries the *writer's* name: a launcher that labels the
        // window it started with somebody else's brand is mislabelling it.
        assert!(script.contains("windowTitle PalantirMC: My Inst\n"));
        assert!(script.contains("windowParams 854x480\n"));
        assert!(script.contains("launcherBrand PalantirMC\n"));
        assert!(script.contains("launcherVersion 9.0\n"));
        assert!(script.contains("instanceName My Inst\n"));
        assert!(script.contains("instanceIconKey My Inst\n")); // quirk: name, not icon key
        assert!(script.contains("instanceIconPath icon.png\n"));
        assert!(script.contains("userName Steve\n"));
        assert!(script.contains("sessionId sess\n"));
        assert!(script.contains("traits first\n"));
        assert!(script.contains("launcher standard\n"));
        assert!(!script.contains("onlineFixes"));
    }

    #[test]
    fn launch_script_legacy_traits_switch_launcher() {
        let mut p = profile();
        p.traits.insert("legacyLaunch".into());
        let script = create_launch_script(
            &p, None, None, &[], WindowParams { width: 854, height: 480, maximized: true }, "B", "1", "L",
        );
        assert!(script.contains("windowParams maximized\n"));
        assert!(script.contains("launcher legacy\n"));
    }

    #[test]
    fn env_vars_match_getvariables() {
        let v = instance_env_vars(
            "Name",
            "id",
            Path::new("/i"),
            Path::new("/i/minecraft"),
            Path::new("/usr/bin/java"),
            "-Xmx1G",
        );
        assert_eq!(v["INST_NAME"], "Name");
        assert_eq!(v["INST_ID"], "id");
        assert_eq!(v["INST_MC_DIR"], "/i/minecraft");
        assert_eq!(v["INST_JAVA_ARGS"], "-Xmx1G");
        assert_eq!(v["NO_COLOR"], "1");
    }

    #[test]
    fn profile_var_map_contains_prism_tokens() {
        let mut p = profile();
        p.libraries.push(Library::default());
        let m = profile_var_map(
            &p,
            "Name",
            "id",
            Path::new("/i"),
            Path::new("/i/minecraft"),
            Path::new("/assets/17"),
            Path::new("/assets"),
            Path::new("/libraries"),
        );
        assert_eq!(m["profile_name"], "Name");
        assert_eq!(m["version_name"], "1.20.4");
        assert_eq!(m["version_type"], "release");
        assert_eq!(m["game_directory"], "/i/minecraft");
        assert_eq!(m["game_assets"], "/assets/17");
        assert_eq!(m["assets_index_name"], "17");
        assert_eq!(m["library_directory"], "/libraries");
    }
}
