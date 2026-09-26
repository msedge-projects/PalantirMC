//! Minimal Modrinth API v2 client types and URL builders.
//!
//! Prism parity: Prism's `ModrinthAPI` talks to `https://api.modrinth.com/v2`
//! with a `PrismLauncher/<version>` user agent. Facet filters are omitted here
//! (search takes a plain query string); only the endpoints needed for browsing
//! (`search`) and version listing (`project/{id}/version`) are modeled.
//!
//! No network calls live in this module (keeping tests offline); pair
//! [`search_url`]/[`version_url`] with a [`crate::meta::Fetcher`] in a later
//! phase to fetch, then `serde_json::from_str` into the structs below.

use std::collections::HashMap;

/// Modrinth API v2 base URL.
pub const MODRINTH_BASE_URL: &str = "https://api.modrinth.com/v2";

/// Build the project-search URL for `query`.
///
/// The query is percent-encoded and a default `limit=50` is appended to match
/// Prism's browsing page size. Use [`search_url_with_project_type`] when the
/// caller is browsing one of Modrinth's content tabs.
pub fn search_url(query: &str) -> String {
    search_url_parts(query, None, None, 50, 0)
}

/// Build a search URL from the parts a browsing interface actually has, each of
/// them optional except the query.
///
/// This is the one place the parameters are spelled out, and the builders around
/// it are this function with a field filled in: a caller that knows it wants a
/// project type should not have to write `None` for a sort order, and two
/// encoders for the same query string is how two pages come to send subtly
/// different ones.
///
/// The parts that are present are emitted in the order the API documents, and an
/// absent one is left out rather than sent empty: `facets=` with nothing after it
/// asks Modrinth to match a project type called `""`, which returns nothing and
/// looks like a broken search rather than a missing filter.
pub fn search_url_parts(
    query: &str,
    project_type: Option<&str>,
    index: Option<&str>,
    limit: u32,
    offset: u32,
) -> String {
    let mut url = format!(
        "{}/search?query={}&limit={}",
        MODRINTH_BASE_URL,
        percent_encode(query),
        limit.max(1)
    );
    if offset > 0 {
        url.push_str(&format!("&offset={offset}"));
    }
    if let Some(project_type) = project_type.filter(|kind| !kind.is_empty()) {
        let facets = format!(r#"[["project_type:{project_type}"]]"#);
        url.push_str(&format!("&facets={}", percent_encode(&facets)));
    }
    if let Some(index) = index.filter(|order| !order.is_empty()) {
        url.push_str(&format!("&index={}", percent_encode(index)));
    }
    url
}

/// Build a Modrinth search URL constrained to one project type.
///
/// Modrinth's `facets` query parameter is JSON (`[["project_type:mod"]]`), not
/// a bespoke query-string flag. Keeping that encoding here prevents each GUI
/// from hand-rolling subtly different filters and means resource-pack searches
/// never return mods that the install path cannot handle.
pub fn search_url_with_project_type(query: &str, project_type: &str) -> String {
    search_url_parts(query, Some(project_type), None, 50, 0)
}

/// Build a Modrinth search URL constrained to one project type *and* sorted by
/// one of Modrinth's own `index` values.
///
/// `index` is the API's name for the sort order (`relevance`, `downloads`,
/// `follows`, `newest`, `updated`), and it is spelled the same way here as it is
/// on the wire so the caller has one vocabulary rather than two. The reference's
/// Discover page offers the same list, in the same order, behind its Sort
/// control.
pub fn search_url_sorted(query: &str, project_type: &str, index: &str) -> String {
    search_url_parts(query, Some(project_type), Some(index), 50, 0)
}

/// Build the version-list URL for a project id or slug.
///
/// Calls `GET /v2/project/{project}/version` (all loaders/game versions;
/// filtering happens client-side).
pub fn version_url(project: &str) -> String {
    format!("{}/project/{}/version", MODRINTH_BASE_URL, percent_encode_path(project))
}

/// A `GET /v2/search` response body (subset; unknown fields ignored).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ModrinthSearchResponse {
    /// Matching projects for this page.
    #[serde(default)]
    pub hits: Vec<ModrinthSearchHit>,
    /// Echoed page offset.
    #[serde(default)]
    pub offset: u32,
    /// Echoed page limit.
    #[serde(default)]
    pub limit: u32,
    /// Total hits across all pages.
    #[serde(default)]
    pub total_hits: u32,
}

/// One entry of [`ModrinthSearchResponse::hits`] (subset).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ModrinthSearchHit {
    /// `project_id` (e.g. `"P7dR8mSH"`).
    #[serde(default)]
    pub project_id: String,
    /// URL slug (e.g. `"sodium"`).
    #[serde(default)]
    pub slug: String,
    /// Human-readable title.
    #[serde(default)]
    pub title: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Author username.
    #[serde(default)]
    pub author: String,
    /// Total download count.
    #[serde(default)]
    pub downloads: u64,
    /// Follower count.
    #[serde(default)]
    pub follows: u64,
    /// Icon URL, if any.
    #[serde(default)]
    pub icon_url: String,
    /// Newest version id, if any.
    #[serde(default)]
    pub latest_version: String,
    /// The game versions the project supports, as the search API lists them.
    ///
    /// On a *search hit* rather than on the project document, and that is the
    /// API's shape rather than a choice: a browsing card shows these tags without
    /// a second request per card, which is the whole reason the field is on the
    /// hit at all.
    #[serde(default)]
    pub versions: Vec<String>,
    /// The project's categories, which is also where a hit carries its loaders
    /// (`fabric`, `quilt`, `forge`, `neoforge`, ...): Modrinth tags them in the
    /// same list, and its own cards draw both from it.
    #[serde(default)]
    pub categories: Vec<String>,
}

impl ModrinthSearchHit {
    /// Return the stable project identifier, preferring `project_id` and
    /// falling back to `slug` when the id is empty.
    pub fn project_ref(&self) -> &str {
        if self.project_id.is_empty() { &self.slug } else { &self.project_id }
    }
}

/// One dependency a version declares.
///
/// Modrinth publishes these on every version: a mod that needs Fabric API says
/// so here, and an installer that ignores the field hands the user a game that
/// crashes on startup with a stack trace they cannot act on.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
pub struct ModrinthDependency {
    /// Version id, when the dependency pins one.
    #[serde(default)]
    pub version_id: Option<String>,
    /// Project id, when the dependency is another Modrinth project.
    #[serde(default)]
    pub project_id: Option<String>,
    /// File name, for a dependency that is not hosted on Modrinth.
    #[serde(default)]
    pub file_name: Option<String>,
    /// `required` / `optional` / `incompatible` / `embedded`.
    #[serde(default)]
    pub dependency_type: String,
}

impl ModrinthDependency {
    /// Whether this dependency has to be installed for the parent to work.
    pub fn is_required(&self) -> bool {
        self.dependency_type == "required"
    }

    /// The project to install, when the dependency names one.
    pub fn project(&self) -> Option<&str> {
        self.project_id.as_deref().filter(|id| !id.is_empty())
    }
}

/// One entry of `GET /v2/project/{id}/version` (subset).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ModrinthProjectVersion {
    /// Version id.
    #[serde(default)]
    pub id: String,
    /// Parent project id.
    #[serde(default)]
    pub project_id: String,
    /// Human-readable name.
    #[serde(default)]
    pub name: String,
    /// Machine version number.
    #[serde(default)]
    pub version_number: String,
    /// `release` / `beta` / `alpha`.
    #[serde(default)]
    pub version_type: String,
    /// Download count.
    #[serde(default)]
    pub downloads: u64,
    /// Target game versions (e.g. `["1.20.4"]`).
    #[serde(default)]
    pub game_versions: Vec<String>,
    /// Target loaders (e.g. `["fabric"]`).
    #[serde(default)]
    pub loaders: Vec<String>,
    /// Downloadable files.
    #[serde(default)]
    pub files: Vec<ModrinthVersionFile>,
    /// Other projects this version depends on.
    #[serde(default)]
    pub dependencies: Vec<ModrinthDependency>,
}

impl ModrinthProjectVersion {
    /// Return the primary file, or the first file when none is flagged
    /// primary, or `None` when [`Self::files`] is empty.
    pub fn primary_file(&self) -> Option<&ModrinthVersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

/// One downloadable file of a [`ModrinthProjectVersion`].
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ModrinthVersionFile {
    /// Direct download URL.
    #[serde(default)]
    pub url: String,
    /// File name.
    #[serde(default)]
    pub filename: String,
    /// Whether this is the primary file of the version.
    #[serde(default)]
    pub primary: bool,
    /// File size in bytes.
    #[serde(default)]
    pub size: u64,
    /// Hash map (`sha512`/`sha1` keys when present).
    #[serde(default)]
    pub hashes: HashMap<String, String>,
}

impl ModrinthVersionFile {
    /// Return the `sha512` hex digest when the server provided one.
    pub fn sha512(&self) -> Option<&str> {
        self.hashes.get("sha512").map(String::as_str)
    }

    /// Return the `sha1` hex digest when the server provided one.
    pub fn sha1(&self) -> Option<&str> {
        self.hashes.get("sha1").map(String::as_str)
    }
}

/// Percent-encode a query string (RFC 3986 unreserved set left intact).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        if matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~') {
            out.push(*b as char);
        } else {
            out.push('%');
            out.push(hex_nibble(b >> 4));
            out.push(hex_nibble(b & 0x0F));
        }
    }
    out
}

/// Percent-encode a single URL path segment (same rules as [`percent_encode`]).
fn percent_encode_path(s: &str) -> String {
    percent_encode(s)
}

/// Format the low 4 bits of `v` as an uppercase hex char.
fn hex_nibble(v: u8) -> char {
    match v & 0x0F {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        9 => '9',
        10 => 'A',
        11 => 'B',
        12 => 'C',
        13 => 'D',
        14 => 'E',
        _ => 'F',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_url_encodes_query_and_limit() {
        assert_eq!(
            search_url("sodium"),
            "https://api.modrinth.com/v2/search?query=sodium&limit=50"
        );
    }

    #[test]
    fn search_url_percent_encodes_specials() {
        assert_eq!(
            search_url("a b+c"),
            "https://api.modrinth.com/v2/search?query=a%20b%2Bc&limit=50"
        );
    }

    #[test]
    fn search_url_empty_query() {
        assert_eq!(search_url(""), "https://api.modrinth.com/v2/search?query=&limit=50");
    }

    #[test]
    fn a_sorted_search_carries_both_the_type_and_the_order() {
        // The facet is JSON and has to be percent-encoded, and `index` is the
        // API's own name for the order -- one parameter each, in the order the
        // API documents them.
        assert_eq!(
            search_url_sorted("sodium", "mod", "downloads"),
            "https://api.modrinth.com/v2/search?query=sodium&limit=50\
             &facets=%5B%5B%22project_type%3Amod%22%5D%5D&index=downloads"
        );
    }

    #[test]
    fn search_url_can_filter_to_a_modrinth_project_type() {
        assert_eq!(
            search_url_with_project_type("faithful", "resourcepack"),
            "https://api.modrinth.com/v2/search?query=faithful&limit=50&facets=%5B%5B%22project_type%3Aresourcepack%22%5D%5D"
        );
    }

    #[test]
    fn a_search_url_leaves_out_the_parts_it_does_not_have() {
        // The three builders above are this one with fields filled in, and the
        // two rules that matter are here: an absent part is *omitted* rather than
        // sent empty (`facets=` with nothing after it asks for a project type
        // called `""` and returns nothing, which reads as a broken search), and
        // the two optional parts keep the order the API documents.
        assert_eq!(
            search_url_parts("sodium", None, None, 20, 0),
            "https://api.modrinth.com/v2/search?query=sodium&limit=20"
        );
        assert_eq!(
            search_url_parts("sodium", Some("mod"), None, 20, 40),
            "https://api.modrinth.com/v2/search?query=sodium&limit=20&offset=40\
             &facets=%5B%5B%22project_type%3Amod%22%5D%5D"
        );
        // An empty string for a filter is an absent filter, not a filter that
        // matches nothing.
        assert_eq!(
            search_url_parts("sodium", Some(""), Some(""), 20, 0),
            "https://api.modrinth.com/v2/search?query=sodium&limit=20"
        );
        // And a limit of zero would be a page with no rows in it.
        assert_eq!(
            search_url_parts("sodium", None, Some("newest"), 0, 0),
            "https://api.modrinth.com/v2/search?query=sodium&limit=1&index=newest"
        );
    }

    #[test]
    fn version_url_builds_project_endpoint() {
        assert_eq!(
            version_url("sodium"),
            "https://api.modrinth.com/v2/project/sodium/version"
        );
    }

    #[test]
    fn version_url_encodes_segments() {
        assert_eq!(
            version_url("a b"),
            "https://api.modrinth.com/v2/project/a%20b/version"
        );
    }

    #[test]
    fn search_response_deserializes_fixture() {
        let body = r#"{
            "hits": [{
                "project_id": "P7dR8mSH",
                "slug": "sodium",
                "title": "Sodium",
                "description": "Fast rendering",
                "author": "jellysquid",
                "downloads": 123,
                "follows": 45,
                "icon_url": "https://x/icon.png",
                "latest_version": "v1"
            }],
            "offset": 0, "limit": 50, "total_hits": 1
        }"#;
        let resp: ModrinthSearchResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.total_hits, 1);
        assert_eq!(resp.hits.len(), 1);
        assert_eq!(resp.hits[0].title, "Sodium");
        assert_eq!(resp.hits[0].project_ref(), "P7dR8mSH");
    }

    #[test]
    fn search_hit_project_ref_falls_back_to_slug() {
        let hit = ModrinthSearchHit {
            project_id: String::new(),
            slug: "sodium".to_string(),
            title: String::new(),
            description: String::new(),
            author: String::new(),
            downloads: 0,
            follows: 0,
            icon_url: String::new(),
            latest_version: String::new(),
            versions: Vec::new(),
            categories: Vec::new(),
        };
        assert_eq!(hit.project_ref(), "sodium");
    }

    #[test]
    fn search_hit_project_ref_prefers_id() {
        let hit = ModrinthSearchHit {
            project_id: "ID".to_string(),
            slug: "slug".to_string(),
            title: String::new(),
            description: String::new(),
            author: String::new(),
            downloads: 0,
            follows: 0,
            icon_url: String::new(),
            latest_version: String::new(),
            versions: Vec::new(),
            categories: Vec::new(),
        };
        assert_eq!(hit.project_ref(), "ID");
    }

    #[test]
    fn project_version_deserializes_fixture() {
        let body = r#"[{
            "id": "v1",
            "project_id": "P7dR8mSH",
            "name": "Sodium 1.0",
            "version_number": "1.0",
            "version_type": "release",
            "downloads": 10,
            "game_versions": ["1.20.4"],
            "loaders": ["fabric"],
            "files": [{
                "url": "https://x/sodium.jar",
                "filename": "sodium.jar",
                "primary": true,
                "size": 42,
                "hashes": {"sha512": "aa", "sha1": "bb"}
            }]
        }]"#;
        let versions: Vec<ModrinthProjectVersion> = serde_json::from_str(body).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version_number, "1.0");
        assert_eq!(versions[0].game_versions, vec!["1.20.4"]);
        let primary = versions[0].primary_file().unwrap();
        assert_eq!(primary.filename, "sodium.jar");
        assert_eq!(primary.sha512(), Some("aa"));
        assert_eq!(primary.sha1(), Some("bb"));
    }

    #[test]
    fn a_version_reads_the_dependencies_it_declares() {
        // The real shape from `GET /v2/project/{id}/version`: a required
        // dependency names a project, an optional one may name only a file, and
        // `embedded` means "already inside this file".
        let json = r#"[{
            "id": "v1", "project_id": "sodium", "name": "Sodium 0.6",
            "version_number": "0.6", "version_type": "release", "downloads": 1,
            "game_versions": ["1.21.1"], "loaders": ["fabric"],
            "files": [{"url": "https://cdn/x.jar", "filename": "x.jar", "primary": true,
                       "size": 10, "hashes": {"sha1": "aa"}}],
            "dependencies": [
                {"version_id": "fab", "project_id": "fabric-api", "file_name": null,
                 "dependency_type": "required"},
                {"project_id": null, "file_name": "libfoo.jar", "dependency_type": "required"},
                {"project_id": "iris", "dependency_type": "optional"},
                {"project_id": "sodium", "dependency_type": "embedded"}
            ]
        }]"#;
        let versions: Vec<ModrinthProjectVersion> = serde_json::from_str(json).unwrap();
        let deps = &versions[0].dependencies;
        assert_eq!(deps.len(), 4);
        let required: Vec<&ModrinthDependency> =
            deps.iter().filter(|d| d.is_required()).collect();
        assert_eq!(required.len(), 2);
        assert_eq!(required[0].project(), Some("fabric-api"));
        // A dependency that is not on Modrinth has no project to install.
        assert_eq!(required[1].project(), None);
        assert_eq!(required[1].file_name.as_deref(), Some("libfoo.jar"));
        // An empty project id is not a project either.
        let empty = ModrinthDependency {
            project_id: Some(String::new()),
            dependency_type: "required".to_string(),
            ..ModrinthDependency::default()
        };
        assert_eq!(empty.project(), None);
    }

    #[test]
    fn primary_file_returns_none_when_empty() {
        let v = ModrinthProjectVersion {
            id: String::new(),
            project_id: String::new(),
            name: String::new(),
            version_number: String::new(),
            version_type: String::new(),
            downloads: 0,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            files: Vec::new(),
            dependencies: Vec::new(),
        };
        assert!(v.primary_file().is_none());
    }

    #[test]
    fn primary_file_falls_back_to_first() {
        let mk = |primary: bool, name: &str| ModrinthVersionFile {
            url: String::new(),
            filename: name.to_string(),
            primary,
            size: 0,
            hashes: HashMap::new(),
        };
        let v = ModrinthProjectVersion {
            id: String::new(),
            project_id: String::new(),
            name: String::new(),
            version_number: String::new(),
            version_type: String::new(),
            downloads: 0,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            files: vec![mk(false, "a.jar"), mk(false, "b.jar")],
            dependencies: Vec::new(),
        };
        assert_eq!(v.primary_file().unwrap().filename, "a.jar");
    }

    #[test]
    fn version_file_hashes_missing_yield_none() {
        let f = ModrinthVersionFile {
            url: String::new(),
            filename: "a.jar".to_string(),
            primary: false,
            size: 0,
            hashes: HashMap::new(),
        };
        assert_eq!(f.sha512(), None);
        assert_eq!(f.sha1(), None);
    }

    #[test]
    fn version_file_sha_accessors() {
        let mut hashes = HashMap::new();
        hashes.insert("sha512".to_string(), "AA".to_string());
        hashes.insert("sha1".to_string(), "BB".to_string());
        let f = ModrinthVersionFile {
            url: String::new(),
            filename: "a".to_string(),
            primary: true,
            size: 1,
            hashes,
        };
        assert_eq!(f.sha512(), Some("AA"));
        assert_eq!(f.sha1(), Some("BB"));
    }
}
