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
    format!("{}/search?query={}&limit=50", MODRINTH_BASE_URL, percent_encode(query))
}

/// Build a Modrinth search URL constrained to one project type.
///
/// Modrinth's `facets` query parameter is JSON (`[["project_type:mod"]]`), not
/// a bespoke query-string flag. Keeping that encoding here prevents each GUI
/// from hand-rolling subtly different filters and means resource-pack searches
/// never return mods that the install path cannot handle.
pub fn search_url_with_project_type(query: &str, project_type: &str) -> String {
    let facets = format!(r#"[["project_type:{project_type}"]]"#);
    format!(
        "{}&facets={}",
        search_url(query),
        percent_encode(&facets)
    )
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
}

impl ModrinthSearchHit {
    /// Return the stable project identifier, preferring `project_id` and
    /// falling back to `slug` when the id is empty.
    pub fn project_ref(&self) -> &str {
        if self.project_id.is_empty() { &self.slug } else { &self.project_id }
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
    fn search_url_can_filter_to_a_modrinth_project_type() {
        assert_eq!(
            search_url_with_project_type("faithful", "resourcepack"),
            "https://api.modrinth.com/v2/search?query=faithful&limit=50&facets=%5B%5B%22project_type%3Aresourcepack%22%5D%5D"
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
