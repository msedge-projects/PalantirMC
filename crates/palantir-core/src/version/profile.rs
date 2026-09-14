//! Launch profile merge — port of `minecraft/LaunchProfile.cpp` driven by
//! `VersionFile::applyTo`.
//!
//! Merge rules (verified against the C++):
//! * Only `net.minecraft` sets the Minecraft version, type and assets.
//! * Strings (`mainClass`, `appletClass`, `minecraftArguments`, version
//!   type...) are **replaced** when the incoming value is non-empty.
//! * Tweakers: existing tweakers named by the incoming list are removed
//!   first, then the incoming list is appended (dedup + move-later).
//! * Libraries split into regular vs native lists; entries match by
//!   group+artifact+classifier, and an existing entry is replaced only when
//!   the incoming version compares greater (`Version` ordering). An
//!   *ambiguous* match (multiple entries with the same specifier) behaves
//!   like "not found" and appends — a quirk faithfully ported.
//! * Maven files never dedupe; agents skip natives.
//! * The main jar lands last on the classpath (or `bin/minecraft.jar` when
//!   jar mods are present), native libraries after it, then the
//!   `${arch}`-split 32/64 native jars by `javaArchitecture`.

use super::library::{ApplicableFiles, Library};
use super::rules::RuntimeContext;
use super::{Agent, AssetIndexInfo, ProblemSeverity, PalantirVersion, VersionFile};
use std::collections::BTreeSet;
use std::path::Path;

/// The fully merged launch profile.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LaunchProfile {
    /// Minecraft version (from `net.minecraft` only).
    pub minecraft_version: String,
    /// Minecraft version type.
    pub minecraft_version_type: String,
    /// Asset index (from `net.minecraft` only; `legacy` fallback).
    pub minecraft_assets: Option<AssetIndexInfo>,
    /// Legacy `minecraftArguments` string.
    pub minecraft_arguments: String,
    /// Additional JVM arguments (`+jvmArgs`).
    pub addn_jvm_arguments: Vec<String>,
    /// Tweaker classes, in final application order.
    pub tweakers: Vec<String>,
    /// Main class (last non-empty wins).
    pub main_class: String,
    /// Legacy applet class.
    pub applet_class: String,
    /// Regular libraries.
    pub libraries: Vec<Library>,
    /// Native libraries.
    pub native_libraries: Vec<Library>,
    /// Maven files (no dedup).
    pub maven_files: Vec<Library>,
    /// Java agents.
    pub agents: Vec<Agent>,
    /// Jar mods.
    pub jar_mods: Vec<Library>,
    /// Applied mods (dedup by specifier + version).
    pub mods: Vec<Library>,
    /// Union of all `+traits`.
    pub traits: BTreeSet<String>,
    /// Main jar override.
    pub main_jar: Option<Library>,
    /// Compatible Java major versions.
    pub compatible_java_majors: Vec<i64>,
    /// Compatible Java name.
    pub compatible_java_name: String,
    /// Highest problem severity seen while merging.
    pub problem_severity: ProblemSeverity,
}

/// Result of a specifier lookup in a library list.
enum Match {
    /// Index of the single match.
    Found(usize),
    /// Multiple matches (behaves like "not found" in the C++).
    Ambiguous,
    /// No match.
    None,
}

fn find_library_index(list: &[Library], needle: &super::GradleSpecifier) -> Match {
    let mut found: Option<usize> = None;
    for (i, lib) in list.iter().enumerate() {
        if lib.name.match_name(needle) {
            if found.is_some() {
                return Match::Ambiguous;
            }
            found = Some(i);
        }
    }
    match found {
        Some(i) => Match::Found(i),
        None => Match::None,
    }
}

fn apply_string(from: &str, to: &mut String) {
    if !from.is_empty() {
        *to = from.to_string();
    }
}

impl LaunchProfile {
    /// Reset to the initial state (`LaunchProfile::clear`).
    pub fn clear(&mut self) {
        *self = LaunchProfile::default();
    }

    /// Apply one version file (`VersionFile::applyTo`).
    pub fn apply_version_file(&mut self, file: &VersionFile, ctx: &RuntimeContext) {
        if file.uid == "net.minecraft" {
            apply_string(&file.version, &mut self.minecraft_version);
            apply_string(&file.type_, &mut self.minecraft_version_type);
            if let Some(ai) = &file.asset_index {
                self.minecraft_assets = Some(ai.clone());
            }
        }
        if let Some(jar) = &file.main_jar {
            self.main_jar = Some(jar.clone());
        }
        apply_string(&file.main_class, &mut self.main_class);
        apply_string(&file.applet_class, &mut self.applet_class);
        apply_string(&file.minecraft_arguments, &mut self.minecraft_arguments);
        self.addn_jvm_arguments.extend(file.addn_jvm_arguments.iter().cloned());
        self.apply_tweakers(&file.add_tweakers);
        self.jar_mods.extend(file.jar_mods.iter().cloned());
        self.apply_mods(&file.mods);
        self.traits.extend(file.traits.iter().cloned());
        self.compatible_java_majors.extend(file.compatible_java_majors.iter().copied());
        apply_string(&file.compatible_java_name, &mut self.compatible_java_name);
        for library in &file.libraries {
            self.apply_library(library, ctx);
        }
        for maven in &file.maven_files {
            self.apply_maven_file(maven, ctx);
        }
        for agent in &file.agents {
            self.apply_agent(agent, ctx);
        }
        self.apply_problem_severity(file.problem_severity());
    }

    /// Tweaker merge: drop existing tweakers that the incoming list also
    /// names (moving them later), then append the incoming list.
    pub fn apply_tweakers(&mut self, incoming: &[String]) {
        let mut kept: Vec<String> = self
            .tweakers
            .iter()
            .filter(|t| !incoming.contains(t))
            .cloned()
            .collect();
        kept.extend(incoming.iter().cloned());
        self.tweakers = kept;
    }

    /// Mod merge (dedup by specifier, greater version replaces).
    pub fn apply_mods(&mut self, incoming: &[Library]) {
        for module in incoming {
            match find_library_index(&self.mods, &module.name) {
                Match::Found(i) => {
                    if PalantirVersion::parse(module.name.version())
                        > PalantirVersion::parse(self.mods[i].name.version())
                    {
                        self.mods[i] = module.clone();
                    }
                }
                Match::Ambiguous | Match::None => self.mods.push(module.clone()),
            }
        }
    }

    /// Library merge (`LaunchProfile::applyLibrary`).
    pub fn apply_library(&mut self, library: &Library, ctx: &RuntimeContext) {
        if !library.is_active(ctx) {
            return;
        }
        let list = if library.is_native() { &mut self.native_libraries } else { &mut self.libraries };
        match find_library_index(list, &library.name) {
            Match::Found(i) => {
                if PalantirVersion::parse(library.name.version())
                    > PalantirVersion::parse(list[i].name.version())
                {
                    list[i] = library.clone();
                }
            }
            Match::Ambiguous | Match::None => list.push(library.clone()),
        }
    }

    /// Maven file merge (`applyMavenFile`): active, non-native, never deduped.
    pub fn apply_maven_file(&mut self, maven: &Library, ctx: &RuntimeContext) {
        if !maven.is_active(ctx) || maven.is_native() {
            return;
        }
        self.maven_files.push(maven.clone());
    }

    /// Agent merge (`applyAgent`): active, non-native.
    pub fn apply_agent(&mut self, agent: &Agent, ctx: &RuntimeContext) {
        if !agent.library.is_active(ctx) || agent.library.is_native() {
            return;
        }
        self.agents.push(agent.clone());
    }

    /// Raise the problem severity (`applyProblemSeverity`).
    pub fn apply_problem_severity(&mut self, severity: ProblemSeverity) {
        if self.problem_severity < severity {
            self.problem_severity = severity;
        }
    }

    /// Asset index with the `legacy` fallback (`getMinecraftAssets`).
    pub fn assets_or_default(&self) -> AssetIndexInfo {
        self.minecraft_assets.clone().unwrap_or_else(|| AssetIndexInfo::bare("legacy"))
    }

    /// Whether a trait is present (`hasTrait`).
    pub fn has_trait(&self, trait_name: &str) -> bool {
        self.traits.contains(trait_name)
    }

    /// Whether the instance needs the legacy launcher
    /// (`BaseInstance::isLegacy`: traits `legacyLaunch`/`alphaLaunch`).
    pub fn is_legacy(&self) -> bool {
        self.has_trait("legacyLaunch") || self.has_trait("alphaLaunch")
    }

    /// Classpath and native jar lists (`getLibraryFiles`). When jar mods
    /// exist and `add_jar_mods` is set, the (rebuilt) main jar is expected
    /// at `<temp_path>/minecraft.jar`.
    pub fn get_library_files(
        &self,
        ctx: &RuntimeContext,
        override_path: Option<&Path>,
        temp_path: &Path,
        add_jar_mods: bool,
    ) -> ApplicableFiles {
        let mut files = ApplicableFiles::default();
        for lib in &self.libraries {
            let f = lib.applicable_files(ctx, override_path);
            files.jar.extend(f.jar);
            files.native.extend(f.native);
            files.native32.extend(f.native32);
            files.native64.extend(f.native64);
        }
        // NOTE: order is important here — main jar last on the classpath.
        if let Some(main) = &self.main_jar {
            if !self.jar_mods.is_empty() && add_jar_mods {
                files.jar.push(temp_path.join("minecraft.jar").to_string_lossy().into_owned());
            } else {
                let f = main.applicable_files(ctx, override_path);
                files.jar.extend(f.jar);
                files.native.extend(f.native);
                files.native32.extend(f.native32);
                files.native64.extend(f.native64);
            }
        }
        for lib in &self.native_libraries {
            let f = lib.applicable_files(ctx, override_path);
            files.jar.extend(f.jar);
            files.native.extend(f.native);
            files.native32.extend(f.native32);
            files.native64.extend(f.native64);
        }
        match self.java_architecture(ctx) {
            "32" => files.native.extend(files.native32.clone()),
            "64" => files.native.extend(files.native64.clone()),
            _ => {}
        }
        files
    }

    fn java_architecture<'a>(&self, ctx: &'a RuntimeContext) -> &'a str {
        &ctx.java_architecture
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::version_file::VersionFile;
    use serde_json::json;
    use std::path::PathBuf;

    fn ctx() -> RuntimeContext {
        RuntimeContext {
            java_architecture: "64".into(),
            java_real_architecture: "x86_64".into(),
            system: "linux".into(),
        }
    }

    fn vf(v: serde_json::Value) -> VersionFile {
        VersionFile::parse(&v, Path::new("t"), false).unwrap()
    }

    #[test]
    fn only_net_minecraft_sets_version_type_and_assets() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        p.apply_version_file(
            &vf(json!({"uid": "net.minecraft", "version": "1.20.4", "type": "release", "assets": "17"})),
            &c,
        );
        // an imposter tries to override
        p.apply_version_file(
            &vf(json!({"uid": "fake.minecraft", "version": "9.9.9", "type": "old", "assets": "1"})),
            &c,
        );
        assert_eq!(p.minecraft_version, "1.20.4");
        assert_eq!(p.minecraft_version_type, "release");
        assert_eq!(p.minecraft_assets.as_ref().unwrap().id, "17");
    }

    #[test]
    fn main_class_and_arguments_replaced_by_last_non_empty() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        p.apply_version_file(&vf(json!({"uid": "a", "mainClass": "first.Main", "minecraftArguments": "one"})), &c);
        p.apply_version_file(&vf(json!({"uid": "b", "mainClass": "second.Main", "minecraftArguments": "two"})), &c);
        p.apply_version_file(&vf(json!({"uid": "c", "minecraftArguments": ""})), &c);
        assert_eq!(p.main_class, "second.Main");
        assert_eq!(p.minecraft_arguments, "two");
    }

    #[test]
    fn tweakers_dedupe_and_move_later() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        p.apply_version_file(&vf(json!({"uid": "a", "+tweakers": ["t1", "t2"]})), &c);
        p.apply_version_file(&vf(json!({"uid": "b", "+tweakers": ["t2", "t3"]})), &c);
        assert_eq!(p.tweakers, vec!["t1", "t2", "t3"]);
    }

    #[test]
    fn libraries_dedupe_by_specifier_and_greater_version_replaces() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        // NOTE: the second entry must carry a `natives` map to be a true
        // Prism native (`Library::isNative` checks the natives dict, not the
        // coordinate classifier). That keeps Prism semantics intact: regular
        // vs native lists split by `isNative`, match by
        // group+artifact+classifier (`matchName`), greater version replaces.
        p.apply_version_file(
            &vf(json!({"uid": "a", "libraries": [
                {"name": "org.lwjgl3:lwjgl:3.2.2"},
                {"name": "org.lwjgl3:lwjgl:3.2.2:natives-linux", "natives": {"linux": "natives-linux"}}
            ]})),
            &c,
        );
        p.apply_version_file(
            &vf(json!({"uid": "b", "libraries": [{"name": "org.lwjgl3:lwjgl:3.3.1"}]})),
            &c,
        );
        // older version does not replace
        p.apply_version_file(
            &vf(json!({"uid": "c", "libraries": [{"name": "org.lwjgl3:lwjgl:3.1.0"}]})),
            &c,
        );
        assert_eq!(p.libraries.len(), 1);
        assert_eq!(p.libraries[0].name.version(), "3.3.1");
        assert_eq!(p.native_libraries.len(), 1);
    }

    #[test]
    fn inactive_libraries_are_skipped() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        p.apply_version_file(
            &vf(json!({"uid": "a", "libraries": [
                {"name": "win.only:lib:1", "rules": [{"action": "allow", "os": {"name": "windows"}}]}
            ]})),
            &c,
        );
        assert!(p.libraries.is_empty());
    }

    #[test]
    fn classpath_order_jars_main_jar_then_natives() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        p.apply_version_file(
            &vf(json!({
                "uid": "net.minecraft",
                "version": "1.20.4",
                "libraries": [{"name": "a:b:1"}],
                "mainJar": {"name": "com.mojang:minecraft:1.20.4:client"}
            })),
            &c,
        );
        p.apply_version_file(
            &vf(json!({"uid": "n", "libraries": [{"name": "nat:lib:1", "natives": {"linux": "natives-linux"}}]})),
            &c,
        );
        let files = p.get_library_files(&c, None, &PathBuf::from("/tmp/bin"), true);
        assert_eq!(
            files.jar,
            vec!["libraries/a/b/1/b-1.jar", "libraries/com/mojang/minecraft/1.20.4/minecraft-1.20.4-client.jar"]
        );
        assert_eq!(files.native, vec!["libraries/nat/lib/1/lib-1-natives-linux.jar"]);
    }

    #[test]
    fn jar_mods_redirect_main_jar_to_rebuilt_temp_jar() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        p.apply_version_file(
            &vf(json!({"uid": "mc", "mainJar": {"name": "com.mojang:minecraft:1.7.10:client"}})),
            &c,
        );
        p.apply_version_file(&vf(json!({"uid": "jm", "jarMods": [{"name": "mod.jar"}]})), &c);
        // Platform-independent: build the expectation with the same
        // `Path::join` the impl uses instead of hard-coding `/` separators
        // (Windows yields `...\minecraft.jar` via `to_string_lossy`).
        let tmp = PathBuf::from("/tmp/bin");
        let files = p.get_library_files(&c, None, &tmp, true);
        assert_eq!(files.jar, vec![tmp.join("minecraft.jar").to_string_lossy().into_owned()]);
        // without add_jar_mods the real main jar is used
        let files = p.get_library_files(&c, None, &PathBuf::from("/tmp/bin"), false);
        assert!(files.jar[0].contains("minecraft-1.7.10-client.jar"));
    }

    #[test]
    fn arch_specific_natives_selected_by_java_architecture() {
        let mut p = LaunchProfile::default();
        // NOTE: Prism filters by `isActive` at apply time, so the apply and
        // query contexts must share the OS. What this test exercises is the
        // `${arch}` -> 32/64 split in `getApplicableFiles` plus the
        // `javaArchitecture` selection in `get_library_files`.
        let win = RuntimeContext {
            java_architecture: "64".into(),
            java_real_architecture: "amd64".into(),
            system: "windows".into(),
        };
        p.apply_version_file(
            &vf(json!({"uid": "n", "libraries": [
                {"name": "nat:lib:1", "natives": {"windows": "natives-windows-${arch}"}}
            ]})),
            &win,
        );
        let files = p.get_library_files(&win, None, &PathBuf::from("/b"), false);
        assert_eq!(files.native, vec!["libraries/nat/lib/1/lib-1-natives-windows-64.jar"]);
        // 32-bit Java picks the 32 variant.
        let win32 = RuntimeContext {
            java_architecture: "32".into(),
            java_real_architecture: "x86".into(),
            system: "windows".into(),
        };
        let files32 = p.get_library_files(&win32, None, &PathBuf::from("/b"), false);
        assert_eq!(files32.native, vec!["libraries/nat/lib/1/lib-1-natives-windows-32.jar"]);
    }

    #[test]
    fn assets_fall_back_to_legacy() {
        let p = LaunchProfile::default();
        assert_eq!(p.assets_or_default().id, "legacy");
    }

    #[test]
    fn traits_union_and_legacy_detection() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        p.apply_version_file(&vf(json!({"uid": "f", "+traits": ["legacyLaunch", "first"] })), &c);
        p.apply_version_file(&vf(json!({"uid": "g", "+traits": ["first", "second"]})), &c);
        assert_eq!(p.traits, BTreeSet::from(["legacyLaunch".to_string(), "first".into(), "second".into()]));
        assert!(p.is_legacy());
        assert!(p.has_trait("first"));
        assert!(!p.has_trait("nope"));
    }

    #[test]
    fn problem_severity_propagates() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        p.apply_version_file(&vf(json!({"uid": "bad", "tweakers": ["banned"]})), &c);
        assert_eq!(p.problem_severity, ProblemSeverity::Error);
    }

    #[test]
    fn maven_files_and_agents_skip_natives_and_inactive() {
        let mut p = LaunchProfile::default();
        let c = ctx();
        p.apply_version_file(
            &vf(json!({"uid": "x",
                "mavenFiles": [
                    {"name": "m:f:1"},
                    {"name": "m:off:1", "rules": [{"action": "disallow"}]}
                ],
                "+agents": [{"name": "ag:ent:1", "argument": "opt"}]
            })),
            &c,
        );
        assert_eq!(p.maven_files.len(), 1);
        assert_eq!(p.agents.len(), 1);
        assert_eq!(p.agents[0].argument, "opt");
    }
}
