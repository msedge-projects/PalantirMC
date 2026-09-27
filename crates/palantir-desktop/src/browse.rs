//! Modrinth browsing: search projects and install the right file into an
//! instance.
//!
//! This is the "Browse" page's engine. It talks to the public Modrinth API
//! (`api.modrinth.com/v2`) with a proper `User-Agent`, picks the newest file
//! that actually matches the target instance's game version and loader, then
//! downloads it straight into `<instance>/mods/` after verifying the file's
//! `sha1` (or, when the API omits it, its byte size).
//!
//! URL building is shared with `palantir-net::modrinth`; parsing and selection
//! are pure so the matching rules are unit-tested without a network.

// What the binary no longer needs is marked rather than deleted: the module's
// request building and pack reading are what its tests cover, and the shell's
// own Discover page asks `palantir-net` directly since the old shell went.
//
// The pack installer's transfers joined them with the launch path's (G93): the
// files a pack lists go over the launcher's one wire, so nothing here builds a
// client, a fetcher or a thread count of its own, and the `User-Agent`
// Modrinth's guidelines ask for is the one `engine::http` sets on the client
// every request already goes through.
#[cfg(test)]
use serde::Deserialize;

/// Content tabs exposed by Modrinth's public project types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg(test)]
pub enum ContentType {
    /// Modrinth modpacks.
    ///
    /// First, and the default, because that is where the reference opens its
    /// Discover page: its tab strip was measured as Modpacks, Mods, Resource
    /// Packs, Data Packs, Shaders, with the Modpacks pill filled. It also has a
    /// sixth tab, Servers, which this shell does not carry -- `project_type:server`
    /// answers 0 hits through the public search API, so a tab for it could only
    /// ever be empty, and a dead tab is worse than an absent one. `REFERENCE.md`
    /// records the difference.
    #[default]
    Modpacks,
    /// Java/Fabric/Forge/Quilt mods.
    Mods,
    /// Client-side resource packs.
    ResourcePacks,
    /// World/data packs.
    DataPacks,
    /// Shader packs.
    Shaders,
}

/// The label is what `pick_list` draws in the closed control and beside each
/// entry, so the two can never disagree about what an order is called.
#[cfg(test)]
impl ContentType {
    /// Label shown in the Browse tab strip.
    ///
    /// "Packs" is capitalised in both entries because that is how the reference
    /// sets them; read off its tab strip, where OCR returned "Data Packs" whole
    /// and merged "ResourcePacks" out of the same line.
    #[cfg(test)]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Modpacks => "Modpacks",
            Self::Mods => "Mods",
            Self::ResourcePacks => "Resource Packs",
            Self::DataPacks => "Data Packs",
            Self::Shaders => "Shaders",
        }
    }

    /// Modrinth's `project_type` facet value.
    #[cfg(test)]
    pub const fn api_value(self) -> &'static str {
        match self {
            Self::Mods => "mod",
            Self::ResourcePacks => "resourcepack",
            Self::DataPacks => "datapack",
            Self::Shaders => "shader",
            Self::Modpacks => "modpack",
        }
    }

    /// All supported tabs, in the order the reference draws them.
    pub const fn all() -> [Self; 5] {
        [Self::Modpacks, Self::Mods, Self::ResourcePacks, Self::DataPacks, Self::Shaders]
    }

    /// Whether the target folder is a loader-specific mod folder.
    pub const fn needs_loader(self) -> bool {
        matches!(self, Self::Mods)
    }
}

/// One search result (a superset of what the UI paints).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg(test)]
pub struct Hit {
    /// Project id (stable).
    #[serde(default)]
    pub project_id: String,
    /// URL slug.
    #[serde(default)]
    pub slug: String,
    /// Display title.
    #[serde(default)]
    pub title: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Author username.
    #[serde(default)]
    pub author: String,
    /// Total downloads.
    #[serde(default)]
    pub downloads: u64,
    /// Icon URL (may be empty).
    #[serde(default)]
    pub icon_url: String,
    /// `mod`, `modpack`, `resourcepack`, ...
    #[serde(default)]
    pub project_type: String,
}

#[cfg(test)]
impl Hit {
    /// Stable identifier for follow-up calls.
    #[cfg(test)]
    pub fn project_ref(&self) -> &str {
        if self.project_id.is_empty() {
            &self.slug
        } else {
            &self.project_id
        }
    }

    /// `1.2M downloads` / `12,345 downloads`, compact for cards.
    #[cfg(test)]
    pub fn downloads_label(&self) -> String {
        let n = self.downloads;
        if n >= 1_000_000_000 {
            format!("{:.1}B downloads", n as f64 / 1_000_000_000.0)
        } else if n >= 1_000_000 {
            format!("{:.1}M downloads", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}K downloads", n as f64 / 1_000.0)
        } else {
            format!("{n} downloads")
        }
    }

    /// `by author · 1.2M downloads`.
    pub fn byline(&self) -> String {
        if self.author.is_empty() {
            self.downloads_label()
        } else {
            format!("by {} · {}", self.author, self.downloads_label())
        }
    }
}

/// Search response envelope.
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg(test)]
struct SearchEnvelope {
    #[serde(default)]
    hits: Vec<Hit>,
}

/// Parse a `GET /v2/search` body (pure; used by the tests).
#[cfg(test)]
pub fn parse_search(body: &str) -> Result<Vec<Hit>, String> {
    let envelope: SearchEnvelope =
        serde_json::from_str(body).map_err(|error| format!("unexpected search response: {error}"))?;
    Ok(envelope.hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The tab strip's own rule, and the one assertion that outlived it: a pack
    /// is installed as an instance of its own rather than into a folder, which
    /// is why it is the one tab whose content has no `mods` to go in. The folder
    /// table itself is `route::ProjectType::target_folder` now -- the install
    /// path is live and this module is not.
    #[test]
    fn a_modpack_targets_an_instance_of_its_own() {
        assert!(!ContentType::Modpacks.needs_loader());
        assert_eq!(ContentType::Modpacks.api_value(), "modpack");
        assert!(ContentType::Mods.needs_loader());
    }

    /// The strip's order and its opening tab are the reference's, measured off
    /// its own Discover page (`REFERENCE.md`): Modpacks first and selected.
    #[test]
    fn tabs_lead_with_modpacks() {
        assert_eq!(ContentType::default(), ContentType::Modpacks);
        let labels: Vec<&str> = ContentType::all().iter().map(|k| k.label()).collect();
        assert_eq!(
            labels,
            ["Modpacks", "Mods", "Resource Packs", "Data Packs", "Shaders"]
        );
        // Every tab still maps to a project type the search API answers for;
        // the reference's sixth tab (Servers) is deliberately absent, because
        // `project_type:server` returns nothing.
        for kind in ContentType::all() {
            assert!(!kind.api_value().is_empty());
        }
    }

    #[test]
    fn search_parsing_reads_the_api_shape() {
        let body = r#"{"hits":[{
            "project_id":"AANobbMI","slug":"sodium","title":"Sodium",
            "description":"Fast rendering","author":"jellysquid3","downloads":12345678,
            "icon_url":"https://x/icon.png","project_type":"mod","unexpected":true
        }],"offset":0,"limit":20,"total_hits":1}"#;
        let hits = parse_search(body).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].project_ref(), "AANobbMI");
        assert_eq!(hits[0].byline(), "by jellysquid3 · 12.3M downloads");
        assert_eq!(hits[0].downloads_label(), "12.3M downloads");
        assert!(parse_search("not json").is_err());
        assert!(parse_search("{}").unwrap().is_empty());
    }

    #[test]
    fn download_labels_scale() {
        let mut hit = Hit {
            project_id: "p".into(),
            slug: "s".into(),
            title: "t".into(),
            description: String::new(),
            author: String::new(),
            downloads: 12,
            icon_url: String::new(),
            project_type: "mod".into(),
        };
        assert_eq!(hit.downloads_label(), "12 downloads");
        assert_eq!(hit.byline(), "12 downloads");
        hit.downloads = 1_234;
        assert_eq!(hit.downloads_label(), "1.2K downloads");
        hit.downloads = 2_500_000_000;
        assert_eq!(hit.downloads_label(), "2.5B downloads");
        // A hit with no project id falls back to its slug.
        hit.project_id.clear();
        assert_eq!(hit.project_ref(), "s");
    }

}
