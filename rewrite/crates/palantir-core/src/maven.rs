//! Maven coordinates, because a library's `name` is one.
//!
//! `group:artifact:version[:classifier][@extension]`, and the library tree on
//! disk is the Maven layout derived from it. Deriving the layout is how a
//! launcher computes a library's path when the metadata gives a name but no
//! download record -- older and mod-loader documents do exactly that.

use crate::error::{Error, Result};

/// A parsed library name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenCoord {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl MavenCoord {
    /// Parse `group:artifact:version[:classifier][@ext]`.
    pub fn parse(name: &str) -> Result<Self> {
        let (coords, extension) = match name.split_once('@') {
            Some((coords, ext)) => (coords, ext.to_string()),
            None => (name, "jar".to_string()),
        };
        let parts: Vec<&str> = coords.split(':').collect();
        let (group, artifact, version, classifier) = match parts.as_slice() {
            [group, artifact, version] => (*group, *artifact, *version, None),
            [group, artifact, version, classifier] => {
                (*group, *artifact, *version, Some(*classifier))
            }
            _ => {
                return Err(Error::Invalid {
                    what: "library name",
                    why: format!("{name:?} is not group:artifact:version[:classifier][@ext]"),
                });
            }
        };
        Ok(Self {
            group: group.to_string(),
            artifact: artifact.to_string(),
            version: version.to_string(),
            classifier: classifier.map(str::to_string),
            extension,
        })
    }

    /// This coordinate's path under the library root: the Maven layout,
    /// `group/with/slashes/artifact/version/artifact-version[-classifier].ext`.
    pub fn rel_path(&self) -> String {
        let classifier = match &self.classifier {
            Some(classifier) => format!("-{classifier}"),
            None => String::new(),
        };
        format!(
            "{}/{}/{}/{}-{}{}.{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            self.artifact,
            self.version,
            classifier,
            self.extension
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_part_names_land_in_the_maven_layout() {
        let coord = MavenCoord::parse("org.lwjgl.lwjgl:lwjgl:2.9.4").unwrap();
        assert_eq!(
            coord.rel_path(),
            "org/lwjgl/lwjgl/lwjgl/2.9.4/lwjgl-2.9.4.jar"
        );
    }

    #[test]
    fn classifiers_and_extensions_extend_the_file_name() {
        let coord =
            MavenCoord::parse("org.lwjgl.lwjgl:lwjgl-platform:2.9.4:natives-windows@zip").unwrap();
        assert_eq!(
            coord.rel_path(),
            "org/lwjgl/lwjgl/lwjgl-platform/2.9.4/lwjgl-platform-2.9.4-natives-windows.zip"
        );
    }

    #[test]
    fn a_name_that_is_not_a_coordinate_is_refused() {
        assert!(MavenCoord::parse("just-a-name").is_err());
        assert!(MavenCoord::parse("a:b:c:d:e").is_err());
    }

    #[test]
    fn nightly_versions_keep_their_underscores() {
        let coord = MavenCoord::parse("org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209").unwrap();
        assert_eq!(
            coord.rel_path(),
            "org/lwjgl/lwjgl/lwjgl/2.9.4-nightly-20150209/lwjgl-2.9.4-nightly-20150209.jar"
        );
    }
}
