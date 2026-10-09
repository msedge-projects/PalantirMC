//! Which libraries this machine gets, and where each one lands.
//!
//! A metadata `Library` is a possibility space: a jar, a natives jar, or
//! both, gated by rules and named by Maven coordinate. Resolution turns one
//! into exactly what this platform needs: the classpath jar and the natives
//! jar to fetch and extract when the library names a classifier for this OS.
//!
//! A `downloads` block is the complete list of what exists to fetch: a
//! library whose block carries only classifiers (`lwjgl-platform`,
//! `jinput-platform`) has no base jar at all, and the repository answers 404
//! for one. Only name-only documents -- older and mod-loader ones -- derive
//! the Maven layout from the coordinate.
//!
//! The `${arch}` in a classifier template (`natives-windows-${arch}`) is the
//! bitness the running Java will use -- `32` or `64`.

use std::collections::BTreeMap;

use crate::error::Result;
use crate::maven::MavenCoord;
use crate::rules::{Platform, rules_allow};
use crate::version::{Artifact, Library, LibraryDownloads};

/// The repository metadata names its library downloads from. Documents that
/// give a name without a download record are served from here by convention.
pub const DEFAULT_LIBRARY_URL: &str = "https://libraries.minecraft.net/";

/// One file to download, and where it lands under the library root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRef {
    /// Path under the library root (the Maven layout).
    pub rel_path: String,
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// A library resolved for one platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLibrary {
    pub coord: MavenCoord,
    /// The classpath jar, when the library has one (natives-only entries
    /// may not).
    pub artifact: Option<DownloadRef>,
    /// The natives jar to fetch and extract, when this platform has one.
    pub natives: Option<DownloadRef>,
    /// Paths to skip when extracting the natives jar.
    pub extract_exclude: Vec<String>,
}

impl Library {
    /// Resolve against a platform. `None` when the rules say this library is
    /// not for this machine.
    pub fn resolve(&self, platform: &Platform) -> Result<Option<ResolvedLibrary>> {
        if let Some(rules) = &self.rules {
            if !rules_allow(rules, platform)? {
                return Ok(None);
            }
        }
        let coord = MavenCoord::parse(&self.name)?;

        let artifact = match self.downloads.as_ref() {
            Some(downloads) => downloads.artifact.as_ref().map(from_artifact),
            // No download record at all: derive the layout from the
            // coordinate and the repository base (older and mod-loader
            // documents). A document that has records but omits the base
            // artifact is classifier-only, and gets none.
            None => Some(DownloadRef {
                rel_path: coord.rel_path(),
                url: Some(join_url(self.repo_base(), &coord.rel_path())),
                sha1: None,
                size: None,
            }),
        };

        let natives = self.resolve_natives(platform, &coord)?;

        Ok(Some(ResolvedLibrary {
            coord,
            artifact,
            natives,
            extract_exclude: self
                .extract
                .as_ref()
                .map(|e| e.exclude.clone())
                .unwrap_or_default(),
        }))
    }

    /// The classifier jar for this platform, if the library names one here.
    fn resolve_natives(
        &self,
        platform: &Platform,
        coord: &MavenCoord,
    ) -> Result<Option<DownloadRef>> {
        let Some(classifier) = self.natives_classifier(platform) else {
            return Ok(None);
        };
        let jar = match self
            .downloads
            .as_ref()
            .and_then(|d: &LibraryDownloads| d.classifiers.as_ref())
            .and_then(|c: &BTreeMap<String, Artifact>| c.get(&classifier))
        {
            Some(artifact) => from_artifact(artifact),
            None => {
                // The `downloads` block is authoritative: a classifier it
                // does not name is not on the repository (and asking for one
                // is a 404 -- 1.5.2's lwjgl-platform names natives for three
                // OSes but serves one). Only name-only documents derive the
                // classifier jar from the coordinate.
                if self.downloads.is_some() {
                    return Ok(None);
                }
                let mut coord = coord.clone();
                coord.classifier = Some(classifier.clone());
                let rel_path = coord.rel_path();
                DownloadRef {
                    url: Some(join_url(self.repo_base(), &rel_path)),
                    rel_path,
                    sha1: None,
                    size: None,
                }
            }
        };
        Ok(Some(jar))
    }

    /// The classifier template for this OS, with `${arch}` filled in.
    fn natives_classifier(&self, platform: &Platform) -> Option<String> {
        let template = self.natives.as_ref()?.get(platform.os.mojang_name()?)?;
        Some(template.replace("${arch}", platform.arch.natives_suffix()))
    }

    fn repo_base(&self) -> &str {
        self.url.as_deref().unwrap_or(DEFAULT_LIBRARY_URL)
    }
}

fn from_artifact(artifact: &Artifact) -> DownloadRef {
    DownloadRef {
        rel_path: artifact.path.clone(),
        url: artifact.url.clone(),
        sha1: artifact.sha1.clone(),
        size: artifact.size,
    }
}

fn join_url(base: &str, rel_path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), rel_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Arch, Os};

    fn library(json: &str) -> Library {
        serde_json::from_str(json).unwrap()
    }

    fn platform() -> Platform {
        Platform::new(Os::Windows, "10.0.19045", Arch::X86_64)
    }

    #[test]
    fn a_plain_library_resolves_to_its_download_record() {
        let lib = library(
            r#"{"name": "com.example:thing:1.0",
                "downloads": {"artifact": {
                    "path": "com/example/thing/1.0/thing-1.0.jar",
                    "sha1": "abc", "size": 5,
                    "url": "https://libraries.minecraft.net/com/example/thing/1.0/thing-1.0.jar"}}}"#,
        );
        let resolved = lib.resolve(&platform()).unwrap().unwrap();
        assert_eq!(
            resolved.artifact.unwrap().rel_path,
            "com/example/thing/1.0/thing-1.0.jar"
        );
        assert!(resolved.natives.is_none());
    }

    #[test]
    fn a_classifier_only_library_has_no_base_artifact() {
        // The real 1.5.2 shape: `downloads` carries classifiers and nothing
        // else, and the repository has no base jar to serve.
        let lib = library(
            r#"{"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.0",
                "natives": {"windows": "natives-windows"},
                "downloads": {"classifiers": {
                    "natives-windows": {
                        "path": "org/lwjgl/lwjgl/lwjgl-platform/2.9.0/lwjgl-platform-2.9.0-natives-windows.jar",
                        "sha1": "def", "size": 9,
                        "url": "https://libraries.minecraft.net/n.jar"}}}}"#,
        );
        let resolved = lib.resolve(&platform()).unwrap().unwrap();
        assert!(resolved.artifact.is_none());
        assert!(
            resolved
                .natives
                .unwrap()
                .rel_path
                .ends_with("natives-windows.jar")
        );
    }

    #[test]
    fn a_downloads_block_is_authoritative_about_missing_classifiers() {
        // Also real 1.5.2: this one names natives for three OSes but ships
        // one classifier. The other two do not exist and must not be asked for.
        let lib = library(
            r#"{"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.1-nightly-20130708-debug3",
                "natives": {"windows": "natives-windows", "osx": "natives-osx"},
                "downloads": {"classifiers": {
                    "natives-osx": {
                        "path": "p/lwjgl-platform-2.9.1-nightly-20130708-debug3-natives-osx.jar",
                        "sha1": "abc", "size": 9,
                        "url": "https://libraries.minecraft.net/o.jar"}}}}"#,
        );
        let resolved = lib.resolve(&platform()).unwrap().unwrap();
        assert!(resolved.artifact.is_none());
        assert!(resolved.natives.is_none());
    }

    #[test]
    fn a_name_only_library_derives_the_maven_layout() {
        let lib = library(r#"{"name": "com.example:thing:1.0"}"#);
        let resolved = lib.resolve(&platform()).unwrap().unwrap();
        let artifact = resolved.artifact.unwrap();
        assert_eq!(artifact.rel_path, "com/example/thing/1.0/thing-1.0.jar");
        assert_eq!(
            artifact.url.as_deref(),
            Some("https://libraries.minecraft.net/com/example/thing/1.0/thing-1.0.jar")
        );
    }

    #[test]
    fn a_name_only_library_can_name_its_own_repository() {
        let lib =
            library(r#"{"name": "com.example:thing:1.0", "url": "https://maven.example.com/"}"#);
        let resolved = lib.resolve(&platform()).unwrap().unwrap();
        assert_eq!(
            resolved.artifact.unwrap().url.as_deref(),
            Some("https://maven.example.com/com/example/thing/1.0/thing-1.0.jar")
        );
    }

    #[test]
    fn natives_expand_arch_and_pick_the_platform_classifier() {
        let lib = library(
            r#"{"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
                "natives": {"windows": "natives-windows-${arch}"},
                "downloads": {"classifiers": {
                    "natives-windows-64": {
                        "path": "org/lwjgl/lwjgl/lwjgl-platform/2.9.4/lwjgl-platform-2.9.4-natives-windows-64.jar",
                        "sha1": "def", "size": 9,
                        "url": "https://libraries.minecraft.net/x.jar"}}}}"#,
        );
        let resolved = lib.resolve(&platform()).unwrap().unwrap();
        let natives = resolved.natives.unwrap();
        assert!(natives.rel_path.ends_with("natives-windows-64.jar"));
    }

    #[test]
    fn no_classifier_for_this_os_means_no_natives() {
        let lib = library(
            r#"{"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
                "natives": {"linux": "natives-linux"}}"#,
        );
        let resolved = lib.resolve(&platform()).unwrap().unwrap();
        assert!(resolved.natives.is_none());
    }

    #[test]
    fn rules_can_drop_a_library_entirely() {
        let lib = library(
            r#"{"name": "com.example:mac-thing:1.0",
                "rules": [{"action": "allow", "os": {"name": "osx"}}]}"#,
        );
        assert!(lib.resolve(&platform()).unwrap().is_none());
    }

    #[test]
    fn extract_excludes_travel_with_the_natives() {
        let lib = library(
            r#"{"name": "com.example:nat:1.0",
                "extract": {"exclude": ["META-INF/"]},
                "natives": {"windows": "natives-windows"}}"#,
        );
        let resolved = lib.resolve(&platform()).unwrap().unwrap();
        assert_eq!(resolved.extract_exclude, vec!["META-INF/".to_string()]);
    }
}
