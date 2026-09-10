//! Maven/Gradle coordinate parsing — port of `minecraft/GradleSpecifier.h`.
//!
//! Format: `group:artifact:version[:classifier][@extension]`.

/// A parsed Maven coordinate. Invalid input is retained (`invalid_value`)
/// with `valid == false`, mirroring the C++ type, so broken library names
/// can be reported as problems instead of failing the whole file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GradleSpecifier {
    group: String,
    artifact: String,
    version: String,
    classifier: String,
    extension: Option<String>,
    valid: bool,
    invalid_value: String,
}

impl GradleSpecifier {
    /// Parse `group:artifact:version[:classifier][@extension]`.
    ///
    /// Faithful to the anchored regex
    /// `([^:@]+):([^:@]+):([^:@]+)(?::([^:@]+))?(?:@([^:@]+))?`: every
    /// component must be non-empty and free of `@`, and the `@extension`
    /// suffix may hang off the version (3rd) or classifier (4th) component.
    pub fn parse(value: &str) -> GradleSpecifier {
        let mut parts: Vec<&str> = value.split(':').collect();
        if parts.len() < 3 || parts.len() > 4 {
            return GradleSpecifier { invalid_value: value.to_string(), valid: false, ..Default::default() };
        }
        // The `@extension` suffix may hang off the last colon-component (the
        // classifier when present, otherwise the version), per the anchored
        // regex `([^:@]+):([^:@]+):([^:@]+)(?::([^:@]+))?(?:@([^:@]+))?`.
        let last_index = parts.len() - 1;
        let (core, extension) = match parts[last_index].split_once('@') {
            Some((a, b)) => (a, Some(b.to_string())),
            None => (parts[last_index], None),
        };
        parts[last_index] = core;
        let bad_ext = extension.as_ref().is_some_and(|e| e.is_empty() || e.contains('@'));
        let bad_core = parts.iter().any(|p| p.is_empty() || p.contains('@'));
        if bad_ext || bad_core {
            return GradleSpecifier { invalid_value: value.to_string(), valid: false, ..Default::default() };
        }
        GradleSpecifier {
            group: parts[0].to_string(),
            artifact: parts[1].to_string(),
            version: parts[2].to_string(),
            classifier: parts.get(3).map(|s| (*s).to_string()).unwrap_or_default(),
            extension,
            valid: true,
            invalid_value: String::new(),
        }
    }

    /// Whether parsing succeeded.
    pub fn valid(&self) -> bool {
        self.valid
    }

    /// Group id.
    pub fn group(&self) -> &str {
        &self.group
    }

    /// Artifact id.
    pub fn artifact(&self) -> &str {
        &self.artifact
    }

    /// Version string.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Classifier (empty when none).
    pub fn classifier(&self) -> &str {
        &self.classifier
    }

    /// Extension when explicitly specified (`@jar`).
    pub fn extension(&self) -> Option<&str> {
        self.extension.as_deref()
    }

    /// Override the classifier (native classifier substitution).
    pub fn set_classifier(&mut self, classifier: &str) {
        self.classifier = classifier.to_string();
    }

    /// Replace the version (used when reconstructing main jars).
    pub fn set_version(&mut self, version: &str) {
        self.version = version.to_string();
    }

    /// `serialize()` — round-trips the coordinate; invalid values yield the
    /// original raw string.
    pub fn serialize(&self) -> String {
        if !self.valid {
            return self.invalid_value.clone();
        }
        let mut out = format!("{}:{}:{}", self.group, self.artifact, self.version);
        if !self.classifier.is_empty() {
            out.push(':');
            out.push_str(&self.classifier);
        }
        if let Some(ext) = &self.extension {
            out.push('@');
            out.push_str(ext);
        }
        out
    }

    /// File name: `artifact-version[-classifier].<ext|jar>`.
    pub fn file_name(&self) -> String {
        if !self.valid {
            return String::new();
        }
        let mut name = format!("{}-{}", self.artifact, self.version);
        if !self.classifier.is_empty() {
            name.push('-');
            name.push_str(&self.classifier);
        }
        format!("{}.{}", name, self.extension.as_deref().unwrap_or("jar"))
    }

    /// Maven-relative path: `group/path/version/<file>`, optionally with a
    /// file-name override (`toPath`).
    pub fn to_path(&self, filename_override: &str) -> String {
        if !self.valid {
            return String::new();
        }
        let file = if filename_override.is_empty() { self.file_name() } else { filename_override.to_string() };
        format!("{}/{}/{}/{}", self.group.replace('.', "/"), self.artifact, self.version, file)
    }

    /// Identity match ignoring version (`matchName`): group + artifact +
    /// classifier.
    pub fn match_name(&self, other: &GradleSpecifier) -> bool {
        self.group == other.group && self.artifact == other.artifact && self.classifier == other.classifier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_coordinate_with_extension() {
        let g = GradleSpecifier::parse("org.gradle.test.classifiers:service:1.0:jdk15@jar");
        assert!(g.valid());
        assert_eq!(g.group(), "org.gradle.test.classifiers");
        assert_eq!(g.artifact(), "service");
        assert_eq!(g.version(), "1.0");
        assert_eq!(g.classifier(), "jdk15");
        assert_eq!(g.extension(), Some("jar"));
        assert_eq!(g.serialize(), "org.gradle.test.classifiers:service:1.0:jdk15@jar");
    }

    #[test]
    fn parse_plain_coordinate_defaults_to_jar() {
        let g = GradleSpecifier::parse("com.mojang:minecraft:1.21.1:client");
        assert!(g.valid());
        assert_eq!(g.extension(), None);
        assert_eq!(g.file_name(), "minecraft-1.21.1-client.jar");
        let plain = GradleSpecifier::parse("org.lwjgl3:lwjgl:3.3.3");
        assert_eq!(plain.file_name(), "lwjgl-3.3.3.jar");
        assert_eq!(plain.classifier(), "");
    }

    #[test]
    fn to_path_builds_maven_layout() {
        let g = GradleSpecifier::parse("net.minecraftforge:forge:1.20.1-47.2.0:universal");
        assert_eq!(
            g.to_path(""),
            "net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-universal.jar"
        );
        assert_eq!(g.to_path("my.jar"), "net/minecraftforge/forge/1.20.1-47.2.0/my.jar");
    }

    #[test]
    fn invalid_coordinates_stay_invalid() {
        for bad in ["", "a:b", "a:b:c:d:e", "onlyone", "a::c", "a:b:c:", "a:b:c:@", "a@x:b:c", "a:b:c:d@e@f"] {
            let g = GradleSpecifier::parse(bad);
            assert!(!g.valid(), "{bad} should be invalid");
            assert_eq!(g.serialize(), bad);
            assert_eq!(g.file_name(), "");
            assert_eq!(g.to_path(""), "");
        }
    }

    #[test]
    fn extension_may_attach_to_version_component() {
        let g = GradleSpecifier::parse("a:b:c@zip");
        assert!(g.valid());
        assert_eq!(g.version(), "c");
        assert_eq!(g.extension(), Some("zip"));
        assert_eq!(g.classifier(), "");
        assert_eq!(g.file_name(), "b-c.zip");
    }

    #[test]
    fn match_name_ignores_version_but_not_classifier() {
        let a = GradleSpecifier::parse("org.lwjgl3:lwjgl:3.2.2");
        let b = GradleSpecifier::parse("org.lwjgl3:lwjgl:3.3.1");
        assert!(a.match_name(&b));
        let nat = GradleSpecifier::parse("org.lwjgl3:lwjgl:3.3.1:natives-linux");
        assert!(!a.match_name(&nat)); // classifier differs
        assert!(nat.match_name(&GradleSpecifier::parse("org.lwjgl3:lwjgl:9.9.9:natives-linux")));
    }
}
