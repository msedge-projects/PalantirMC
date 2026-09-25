//! Modrinth's API v2, read through the engine's cache.
//!
//! `crate::modrinth` is the vocabulary -- the URL shapes and the types a search
//! response deserializes into -- and this is the part that *asks*: one cache, one
//! pool, one retry policy, the same three the rest of the engine uses. The split
//! matters because the desktop crate already has a browser (`browse.rs`) that
//! talks to this API with a `reqwest` client of its own, and a second client is a
//! second connection pool, a second `User-Agent`, and a second answer to "how
//! many requests is this launcher making".
//!
//! ## Two TTLs, because a search and a project age differently
//!
//! [`SEARCH_TTL`] is five minutes. A search is not a document, it is a *question*
//! -- "what mods match this, most downloaded first" -- and the answer moves as
//! projects are published and adopted. Half an hour of a stale Discover page is a
//! launcher that looks broken to somebody who just watched a video about a new
//! mod; five minutes is long enough that typing and backspacing in the search
//! field does not become one request per keystroke.
//!
//! A project's version list is believed for [`crate::engine::cache::DEFAULT_TTL`]
//! instead. It changes when the author publishes a release, which is a much
//! slower clock than a search ranking, and it is the list an install reads.
//!
//! ## What is not here
//!
//! Nothing verifies a *digest* of these responses, and nothing can: Modrinth
//! publishes digests for the *files* a version carries, not for its JSON. Those
//! digests are in the response ([`ModrinthVersionFile::sha1`] and `sha512`) and
//! they are checked where the file is: by the content store, at the point the
//! bytes are adopted. A cache entry here is a document that parsed, which is the
//! most a JSON API can promise.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::engine::cache::MetadataCache;
use crate::engine::cancel::Cancel;
use crate::engine::request::Fetch;
use crate::engine::retry::Backoff;
use crate::modrinth::{
    search_url_parts, version_url, ModrinthProjectVersion, ModrinthSearchResponse,
};
use crate::Error;

/// How long a search result is believed.
///
/// Short, and deliberately shorter than the metadata default: a search is a
/// question about what people are using right now, and the answer to it changes
/// on a scale of days at the slowest. See the module documentation.
pub const SEARCH_TTL: Duration = Duration::from_secs(5 * 60);

/// One search, as a browsing interface asks for it.
///
/// A value rather than five arguments, because the URL *is* the cache key: a
/// caller that assembled one by hand would be assembling its own key, and two
/// pages that built the same search slightly differently would be two entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    /// What the user typed.
    pub query: String,
    /// One of Modrinth's project types (`mod`, `modpack`, `resourcepack`,
    /// `shader`, `datapack`), or `None` for all of them.
    pub project_type: Option<String>,
    /// Modrinth's own sort name (`relevance`, `downloads`, `follows`, `newest`,
    /// `updated`), or `None` for the API's default.
    pub index: Option<String>,
    /// How many rows to ask for.
    pub limit: u32,
    /// How far into the result set to start.
    pub offset: u32,
}

impl Search {
    /// A search for `query`, with the page size the reference's Discover page
    /// uses.
    pub fn new(query: impl Into<String>) -> Search {
        Search { query: query.into(), project_type: None, index: None, limit: 50, offset: 0 }
    }

    /// The same search, constrained to one project type.
    pub fn of_type(mut self, project_type: impl Into<String>) -> Search {
        self.project_type = Some(project_type.into());
        self
    }

    /// The same search, in one of Modrinth's own sort orders.
    pub fn sorted_by(mut self, index: impl Into<String>) -> Search {
        self.index = Some(index.into());
        self
    }

    /// The same search, starting `offset` rows in.
    pub fn from_row(mut self, offset: u32) -> Search {
        self.offset = offset;
        self
    }

    /// The same search, asking for `limit` rows.
    pub fn with_limit(mut self, limit: u32) -> Search {
        self.limit = limit;
        self
    }

    /// The URL this search is, which is also the key it is cached under.
    pub fn url(&self) -> String {
        search_url_parts(
            &self.query,
            self.project_type.as_deref(),
            self.index.as_deref(),
            self.limit,
            self.offset,
        )
    }
}

/// Modrinth's API, read through the engine.
pub struct ModrinthApi {
    /// The search handle: believed for [`SEARCH_TTL`].
    searches: MetadataCache,
    /// The project handle, over the same directory: believed for the metadata
    /// default.
    projects: MetadataCache,
    fetch: Arc<dyn Fetch>,
}

impl std::fmt::Debug for ModrinthApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModrinthApi")
            .field("dir", &self.searches.dir())
            .finish_non_exhaustive()
    }
}

impl ModrinthApi {
    /// Modrinth over `cache`'s directory, fetching through `fetch`.
    pub fn new(cache: MetadataCache, fetch: Arc<dyn Fetch>) -> ModrinthApi {
        let searches = cache.clone().with_ttl(SEARCH_TTL);
        ModrinthApi { searches, projects: cache, fetch }
    }

    /// The same API with a different search TTL.
    pub fn with_search_ttl(mut self, ttl: Duration) -> ModrinthApi {
        self.searches = self.searches.with_ttl(ttl);
        self
    }

    /// The same API with a different project TTL.
    pub fn with_project_ttl(mut self, ttl: Duration) -> ModrinthApi {
        self.projects = self.projects.with_ttl(ttl);
        self
    }

    /// Where the responses are cached.
    pub fn cache_dir(&self) -> &Path {
        self.searches.dir()
    }

    /// Run a search, from the cache when it is fresh.
    pub fn search(
        &self,
        search: &Search,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<ModrinthSearchResponse, Error> {
        let url = search.url();
        let held = self.searches.get(&url, self.fetch.as_ref(), cancel, backoff)?;
        serde_json::from_slice(&held.body).map_err(|error| Error::json(url, error.to_string()))
    }

    /// Every version of one project, newest first as Modrinth lists them.
    ///
    /// Unfiltered on purpose: filtering by game version and loader happens in the
    /// caller, where the instance's own settings are, and an unfiltered list is
    /// the one that can be cached once and answered for every instance.
    pub fn versions(
        &self,
        project: &str,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Vec<ModrinthProjectVersion>, Error> {
        let url = version_url(project);
        let held = self.projects.get(&url, self.fetch.as_ref(), cancel, backoff)?;
        serde_json::from_slice(&held.body).map_err(|error| Error::json(url, error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::cache::DEFAULT_TTL;
    use crate::engine::request::{MapFetch, Route};

    /// A search body with one hit in it.
    const SEARCH_BODY: &str = r#"{
        "hits": [{
            "project_id": "AANobbMI", "slug": "sodium", "title": "Sodium",
            "description": "Modern rendering engine", "author": "jellysquid",
            "downloads": 41000000, "follows": 9000,
            "icon_url": "https://cdn.modrinth.com/icon.png", "latest_version": "mc1.21.4-0.6.5"
        }],
        "offset": 0, "limit": 50, "total_hits": 214
    }"#;

    /// A version list with one version and one file.
    const VERSIONS_BODY: &str = r#"[{
        "id": "abc123", "project_id": "AANobbMI", "name": "Sodium 0.6.5",
        "version_number": "mc1.21.4-0.6.5", "version_type": "release", "downloads": 1234,
        "game_versions": ["1.21.4"], "loaders": ["fabric"],
        "files": [{
            "url": "https://cdn.modrinth.com/data/AANobbMI/versions/abc123/sodium.jar",
            "filename": "sodium.jar", "primary": true, "size": 1048576,
            "hashes": { "sha512": "aa", "sha1": "bb" }
        }],
        "dependencies": []
    }]"#;

    /// An API over a scratch directory whose only routes are the ones a test
    /// scripts, so a test that forgets one fails instead of dialling out.
    fn api(name: &str, ttl: Duration) -> (ModrinthApi, Arc<MapFetch>) {
        let root = std::env::temp_dir().join("palantirmc-engine-modrinth").join(name);
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        let api = ModrinthApi::new(MetadataCache::new(root, ttl), fetch.clone());
        (api, fetch)
    }

    fn run(api: &ModrinthApi, search: &Search) -> Result<ModrinthSearchResponse, Error> {
        api.search(search, &Cancel::new(), &Backoff::with_attempts(1))
    }

    #[test]
    fn a_search_reads_the_api_shape_and_is_cached_under_its_own_url() {
        let (api, fetch) = api("search", DEFAULT_TTL);
        let search = Search::new("sodium").of_type("mod").sorted_by("downloads");
        fetch.set_route(&search.url(), Route::text(SEARCH_BODY));

        let response = run(&api, &search).expect("a response");
        assert_eq!(response.total_hits, 214);
        assert_eq!(response.hits.len(), 1);
        assert_eq!(response.hits[0].title, "Sodium");
        assert_eq!(response.hits[0].project_ref(), "AANobbMI");
        assert!(api.searches.cached(&search.url()).is_some(), "the response was kept, with an age");
        assert!(api.cache_dir().exists());

        // The key is the URL, so the request that was made is the one the search
        // describes -- facets and sort included, percent-encoded.
        let asked = fetch.requests();
        assert_eq!(asked.len(), 1);
        assert!(asked[0].url.contains("facets=%5B%5B%22project_type%3Amod%22%5D%5D"), "{}", asked[0].url);
        assert!(asked[0].url.ends_with("&index=downloads"), "{}", asked[0].url);
    }

    #[test]
    fn the_same_search_twice_costs_one_request_and_a_different_one_costs_two() {
        let (api, fetch) = api("distinct", DEFAULT_TTL);
        let sodium = Search::new("sodium").of_type("mod");
        let lithium = Search::new("lithium").of_type("mod");
        let newest = sodium.clone().sorted_by("newest");
        for search in [&sodium, &lithium, &newest] {
            fetch.set_route(&search.url(), Route::text(SEARCH_BODY));
        }

        run(&api, &sodium).expect("the first");
        run(&api, &sodium).expect("again, from the cache");
        assert_eq!(fetch.count(), 1, "a fresh search is not asked for again");

        run(&api, &lithium).expect("a different question");
        assert_eq!(fetch.count(), 2);
        // And sorting is part of the question, because it is part of the URL: a
        // cache that ignored it would answer "most downloads" for "newest".
        run(&api, &newest).expect("the same query, sorted");
        assert_eq!(fetch.count(), 3);
        assert!(fetch.requests()[2].url.contains("&index=newest"));
        run(&api, &newest).expect("sorted again, from the cache");
        assert_eq!(fetch.count(), 3, "and the sorted answer is cached separately");
    }

    #[test]
    fn a_search_is_believed_for_minutes_rather_than_for_half_an_hour() {
        // The TTL is short because a search is about what people are using
        // *now*; zero is the way to reach the expired path without waiting, and
        // both handles are over the same directory so a project's versions keep
        // their own belief.
        let (api, fetch) = api("ttls", DEFAULT_TTL);
        let search = Search::new("sodium");
        fetch.set_route(&search.url(), Route::text(SEARCH_BODY));
        let api = api.with_search_ttl(Duration::ZERO);

        run(&api, &search).expect("the first");
        run(&api, &search).expect("the second");
        assert_eq!(fetch.count(), 2, "a zero TTL means always ask");
        assert_eq!(api.projects.ttl(), DEFAULT_TTL, "the other handle is unchanged");
        assert_eq!(api.searches.ttl(), Duration::ZERO);
        assert_eq!(ModrinthApi::new(MetadataCache::new(".", DEFAULT_TTL), fetch).searches.ttl(), SEARCH_TTL);
    }

    #[test]
    fn a_project_version_list_is_read_and_believed_for_the_metadata_default() {
        let (api, fetch) = api("versions", DEFAULT_TTL);
        fetch.set_route(&version_url("AANobbMI"), Route::text(VERSIONS_BODY));

        let versions = api
            .versions("AANobbMI", &Cancel::new(), &Backoff::with_attempts(1))
            .expect("versions");
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version_number, "mc1.21.4-0.6.5");
        assert_eq!(versions[0].loaders, vec!["fabric"]);
        assert_eq!(versions[0].game_versions, vec!["1.21.4"]);
        let primary = versions[0].primary_file().expect("a file");
        assert_eq!(primary.filename, "sodium.jar");
        assert_eq!(primary.sha1(), Some("bb"));
        assert_eq!(primary.sha512(), Some("aa"));

        // Believed: the second call reads the cache, and the URL is one a slug
        // can be used in as well as an id.
        api.versions("AANobbMI", &Cancel::new(), &Backoff::with_attempts(1)).expect("again");
        assert_eq!(fetch.count(), 1);
        api.versions("sodium", &Cancel::new(), &Backoff::with_attempts(1)).expect_err("no route");
        assert_eq!(fetch.count(), 2, "a slug is a different URL and so a different cache entry");
    }

    #[test]
    fn a_failure_is_reported_as_it_came_and_nothing_is_stored() {
        let (api, fetch) = api("failure", DEFAULT_TTL);
        let search = Search::new("sodium");
        fetch.set_route(&search.url(), Route::text(SEARCH_BODY).failing(1, 503));

        let error = run(&api, &search).expect_err("a 503");
        assert!(matches!(error, Error::Http { status: Some(503), .. }), "{error:?}");
        assert_eq!(fetch.count(), 1);
        assert!(api.searches.cached(&search.url()).is_none(), "nothing was stored");

        // And the API answers once the service does.
        let response = run(&api, &search).expect("the second attempt");
        assert_eq!(response.total_hits, 214);
    }

    #[test]
    fn a_body_that_is_not_the_shape_it_claims_is_reported_against_its_url() {
        let (api, fetch) = api("bad-json", DEFAULT_TTL);
        let search = Search::new("sodium");
        fetch.set_route(&search.url(), Route::text("<html>not json</html>"));

        let error = run(&api, &search).expect_err("an HTML error page");
        assert!(matches!(error, Error::Json { .. }), "{error:?}");
        let message = format!("{error}");
        assert!(message.contains("api.modrinth.com"), "it names the URL: {message}");
    }

    #[test]
    fn a_cancelled_search_stops_before_the_body_is_read() {
        let (api, fetch) = api("cancelled", DEFAULT_TTL);
        let search = Search::new("sodium");
        // Chunked, so the cancellation lands between chunks rather than before
        // the request: the case a progress line has to handle.
        fetch.set_route(&search.url(), Route::body(SEARCH_BODY.as_bytes()).chunked(8));
        let cancel = Cancel::new();
        cancel.cancel();

        let error = api
            .search(&search, &cancel, &Backoff::with_attempts(1))
            .expect_err("cancelled");
        assert!(matches!(error, Error::Cancelled), "{error:?}");
        assert!(api.searches.cached(&search.url()).is_none());
    }
}
