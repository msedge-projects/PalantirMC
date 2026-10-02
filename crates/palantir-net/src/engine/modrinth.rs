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
    project_members_url, project_url, search_url_parts_with_facets, tag_categories_url,
    tag_game_versions_url, tag_loaders_url, user_projects_url, user_url, version_url,
    CategoryTag, GameVersionTag, LoaderTag, ModrinthMember, ModrinthProject, ModrinthProjectVersion,
    ModrinthSearchResponse, ModrinthUser, ModrinthUserProject, NewsArticle, NewsFeed, NEWS_URL,
};
use crate::Error;

/// Read one of the tag lists: the same cache, the same error reporting, and a
/// `Vec` of whatever the route publishes.
///
/// Three routes with one body shape between them, which is the only thing they
/// share -- each one's own document is a different type, and the caller joins
/// them. Factored here rather than written three times because a fourth tag route
/// is a thing Modrinth could add without this file noticing.
fn read_tag_list<T: serde::de::DeserializeOwned>(
    url: String,
    cache: &MetadataCache,
    fetch: &dyn Fetch,
    cancel: &Cancel,
    backoff: &Backoff,
) -> Result<Vec<T>, Error> {
    let held = cache.get(&url, fetch, cancel, backoff)?;
    serde_json::from_slice(&held.body).map_err(|error| Error::json(url, error.to_string()))
}

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
    /// Facet groups to put on the request beside the project type's.
    ///
    /// Empty for most searches, and the shape is the API's rather than this
    /// module's: one or more already-spelled or-groups, sent inside the one
    /// `facets=` parameter ([`search_url_parts_with_facets`]). They are part of
    /// the search rather than a decoration of it, which is why they travel in
    /// the same value and the URL is still the whole cache key.
    pub facets: Vec<String>,
}

impl Search {
    /// A search for `query`, with the page size the reference's Discover page
    /// uses.
    pub fn new(query: impl Into<String>) -> Search {
        Search {
            query: query.into(),
            project_type: None,
            index: None,
            limit: 50,
            offset: 0,
            facets: Vec::new(),
        }
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

    /// The same search, carrying `facets` as or-groups beside the project type's.
    ///
    /// One group per element: `["project_id NOT IN [\"AANobbMI\"]"]` asks for
    /// everything except one project, and a caller that wants two exclusions in
    /// one request spells both in one group rather than passing two.
    pub fn with_facets(mut self, facets: Vec<String>) -> Search {
        self.facets = facets;
        self
    }

    /// The URL this search is, which is also the key it is cached under.
    pub fn url(&self) -> String {
        search_url_parts_with_facets(
            &self.query,
            self.project_type.as_deref(),
            self.index.as_deref(),
            self.limit,
            self.offset,
            &self.facets,
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

    /// One project's own document.
    ///
    /// The header a project page draws: the title, the summary, the long
    /// description, the counts, the game versions and loaders it claims, and its
    /// gallery. Believed for the project TTL, which is the metadata default -- a
    /// description changes when an author edits it, on the same slow clock as a
    /// version list.
    pub fn project(
        &self,
        project: &str,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<ModrinthProject, Error> {
        let url = project_url(project);
        let held = self.projects.get(&url, self.fetch.as_ref(), cancel, backoff)?;
        serde_json::from_slice(&held.body).map_err(|error| Error::json(url, error.to_string()))
    }

    /// The people on one project's team, in the API's order.
    ///
    /// A second request, because the project document names a team and not a
    /// person: Modrinth's own page draws the owner's name under the title, and
    /// that name lives here. Cached under its own URL, so a page that only wants
    /// the description never pays for it.
    pub fn members(
        &self,
        project: &str,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Vec<ModrinthMember>, Error> {
        let url = project_members_url(project);
        let held = self.projects.get(&url, self.fetch.as_ref(), cancel, backoff)?;
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

    /// One user's own profile document.
    ///
    /// The published API's half of what the reference's profile page draws -- its
    /// plugin wraps Labrinth's v3 user service instead, which is a route outside the
    /// published API. Believed for the project TTL: a bio and an avatar are edited
    /// by hand on the same slow clock as a project description.
    pub fn user(
        &self,
        user: &str,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<ModrinthUser, Error> {
        let url = user_url(user);
        let held = self.projects.get(&url, self.fetch.as_ref(), cancel, backoff)?;
        serde_json::from_slice(&held.body).map_err(|error| Error::json(url, error.to_string()))
    }

    /// Every project one user owns.
    ///
    /// A second request under its own URL, so a page that only draws the header never
    /// pays for the list -- the same split [`Self::members`] makes for a project.
    pub fn user_projects(
        &self,
        user: &str,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Vec<ModrinthUserProject>, Error> {
        let url = user_projects_url(user);
        let held = self.projects.get(&url, self.fetch.as_ref(), cancel, backoff)?;
        serde_json::from_slice(&held.body).map_err(|error| Error::json(url, error.to_string()))
    }/// Every category Modrinth knows, as `GET /v2/tag/category` lists them.
    ///
    /// One of three tag routes -- the other two are [`Self::tag_game_versions`]
    /// and [`Self::tag_loaders`] -- and what a browse page's filter sections are
    /// built from, which is what this launcher had no use for until Discover grew
    /// a sidebar.
    ///
    /// Believed for the project TTL: the list changes when Modrinth ships, not
    /// when a release happens, so the search clock would be a request per tab
    /// change for an answer that was the same all afternoon.
    pub fn tag_categories(
        &self,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Vec<CategoryTag>, Error> {
        read_tag_list(tag_categories_url(), &self.projects, self.fetch.as_ref(), cancel, backoff)
    }

    /// Every game version Modrinth knows, newest first as the API lists them.
    pub fn tag_game_versions(
        &self,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Vec<GameVersionTag>, Error> {
        read_tag_list(tag_game_versions_url(), &self.projects, self.fetch.as_ref(), cancel, backoff)
    }

    /// Every loader Modrinth knows.
    pub fn tag_loaders(
        &self,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Vec<LoaderTag>, Error> {
        read_tag_list(tag_loaders_url(), &self.projects, self.fetch.as_ref(), cancel, backoff)
    }

    /// Modrinth's news feed, newest first.
    ///
    /// The one document here that is not the API: the panel's news section is
    /// drawn from it, and the reference fetches the same URL itself. Believed for
    /// the project TTL -- the metadata default -- rather than [`SEARCH_TTL`]: a
    /// feed of announcements is not a question about what people are using right
    /// now, and an article half an hour old is still the article under the
    /// reader's nose.
    ///
    /// The whole feed is returned and the *panel* takes the first four it draws,
    /// which is the reference's own split: its fetch slices four for the sidebar
    /// and its news page reads the rest of the same response.
    pub fn news(&self, cancel: &Cancel, backoff: &Backoff) -> Result<Vec<NewsArticle>, Error> {
        let held = self.projects.get(NEWS_URL, self.fetch.as_ref(), cancel, backoff)?;
        let feed: NewsFeed = serde_json::from_slice(&held.body)
            .map_err(|error| Error::json(NEWS_URL, error.to_string()))?;
        Ok(feed.articles)
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
            "icon_url": "https://cdn.modrinth.com/icon.png", "latest_version": "mc1.21.4-0.6.5",
            "versions": ["1.21.4", "1.21.3"], "categories": ["fabric", "optimization"]
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

    /// One `GET /v2/project/{id}` body, in the API's own shape. A two-hash raw
    /// string, because a markdown body's heading opens with `"#`.
    const PROJECT_BODY: &str = r##"{
        "id": "AANobbMI", "slug": "sodium", "project_type": "mod",
        "title": "Sodium", "description": "Modern rendering engine",
        "body": "# Sodium\n\nFaster.\n",
        "downloads": 41000000, "followers": 9000,
        "game_versions": ["1.21.4", "1.21.3"], "loaders": ["fabric"],
        "gallery": [{
            "url": "https://cdn.modrinth.com/shot.png", "title": "In the nether",
            "description": "A cave", "featured": true
        }]
    }"##;

    /// One `GET /v2/project/{id}/members` body: an owner and a member, the way
    /// Modrinth orders them.
    const MEMBERS_BODY: &str = r#"[
        {"team_id": "t1", "role": "Owner", "ordering": 0,
         "user": {"id": "u1", "username": "jellysquid3"}},
        {"team_id": "t1", "role": "Member", "ordering": 1,
         "user": {"id": "u2", "username": "embeddedt"}}
    ]"#;

    /// Two articles, in the feed's own shape and its own order, as the live feed
    /// publishes them (`2026-09-07T19:00:00.000Z` is a real entry's date).
    const NEWS_BODY: &str = r#"{
        "articles": [
            {"title": "Sync settings across instances",
             "summary": "Keep game options the same across your instances.",
             "thumbnail": "https://modrinth.com/news/article/sync-settings/thumbnail.webp",
             "date": "2026-09-07T19:00:00.000Z",
             "link": "https://modrinth.com/news/article/sync-settings"},
            {"title": "A second article", "date": "2026-08-01T10:00:00.000Z",
             "link": "https://modrinth.com/news/article/second"}
        ]
    }"#;

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
    fn the_news_feed_reads_five_fields_and_is_cached_like_a_document() {
        let (api, fetch) = api("news", DEFAULT_TTL);
        fetch.set_route(NEWS_URL, Route::text(NEWS_BODY));

        let news = api.news(&Cancel::new(), &Backoff::with_attempts(1)).expect("the feed");
        assert_eq!(news.len(), 2, "the whole feed, in the order it was published");
        assert_eq!(news[0].title, "Sync settings across instances");
        assert_eq!(news[0].summary, "Keep game options the same across your instances.");
        assert!(news[0].date.ends_with('Z'), "the date travels as published");
        assert_eq!(news[0].link, "https://modrinth.com/news/article/sync-settings");
        assert!(news[0].thumbnail.ends_with("thumbnail.webp"));
        // An article that publishes no summary is a card with one paragraph fewer,
        // not a feed that fails to parse.
        assert_eq!(news[1].summary, "");
        assert_eq!(news[1].thumbnail, "");

        api.news(&Cancel::new(), &Backoff::with_attempts(1)).expect("again");
        assert_eq!(fetch.count(), 1, "a feed inside its belief costs nothing");
        assert_eq!(fetch.requests()[0].url, NEWS_URL);
    }

    /// The profile shape `GET /v2/user/{name}` answers with, and the two projects
    /// `GET /v2/user/{id}/projects` answers for that account.
    /// Three tag bodies, trimmed to the shapes the filter lists read: two release
    /// versions and one snapshot (the split *Show all versions* makes), two
    /// loaders with different project-type support, and four categories across two
    /// headers of modpacks and one of mods.
    ///
    /// There is no `/tags`: these are three routes, which is why the reference's
    /// `get_categories`, `get_game_versions` and `get_loaders` are three calls.
    const TAGS_CATEGORY_BODY: &str = r#"[
        {"icon": "", "name": "technology", "project_type": "mod", "header": "technology"},
        {"icon": "", "name": "kitchen-sink", "project_type": "modpack", "header": "technical"},
        {"icon": "", "name": "adventure", "project_type": "modpack", "header": "gameplay"},
        {"icon": "", "name": "multiplayer", "project_type": "modpack", "header": "gameplay"}
    ]"#;
    const TAGS_GAME_VERSION_BODY: &str = r#"[
        {"version": "1.21.4", "version_type": "release", "date": "2026-01-01T00:00:00Z", "major": false},
        {"version": "1.21.1", "version_type": "release", "date": "2025-01-01T00:00:00Z", "major": false},
        {"version": "24w14potato", "version_type": "snapshot", "date": "2024-04-01T00:00:00Z", "major": false}
    ]"#;
    const TAGS_LOADER_BODY: &str = r#"[
        {"icon": "<svg/>", "name": "fabric", "supported_project_types": ["mod", "modpack"]},
        {"icon": "<svg/>", "name": "forge", "supported_project_types": ["mod"]}
    ]"#;

    const USER_BODY: &str = r#"{
        "id": "2REoufqX",
        "username": "Modrinth",
        "name": null,
        "avatar_url": "https://cdn.modrinth.com/data/2REoufqX/abc_96.webp",
        "bio": "An official user account of Modrinth.",
        "created": "2023-11-13T23:22:36.604990Z"
    }"#;

    const USER_PROJECTS_BODY: &str = r#"[
        {"id":"hEOCdOgW","slug":"phosphor","project_type":"mod","title":"Phosphor",
         "description":"No-compromises lighting engine optimization mod","downloads":865848,
         "icon_url":"https://cdn.modrinth.com/data/hEOCdOgW/abc.png"}
    ]"#;

    #[test]
    fn a_profile_and_its_projects_are_two_cached_documents() {
        let (api, fetch) = api("user", DEFAULT_TTL);
        fetch.set_route(&user_url("2REoufqX"), Route::text(USER_BODY));
        fetch.set_route(&user_projects_url("2REoufqX"), Route::text(USER_PROJECTS_BODY));

        let user = api.user("2REoufqX", &Cancel::new(), &Backoff::with_attempts(1)).expect("the profile");
        assert_eq!(user.username, "Modrinth");
        assert_eq!(user.display_name(), "Modrinth", "no display name means the username");
        assert!(user.avatar_url.ends_with("abc_96.webp"));
        assert_eq!(user.joined_label(), "November 13, 2023");
        let projects = api
            .user_projects("2REoufqX", &Cancel::new(), &Backoff::with_attempts(1))
            .expect("the list");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].slug, "phosphor");
        assert_eq!(projects[0].downloads, 865848);

        // Two URLs, so a page that only draws the header never pays for the list.
        // Asked twice, both come from the cache.
        api.user("2REoufqX", &Cancel::new(), &Backoff::with_attempts(1)).expect("again");
        api.user_projects("2REoufqX", &Cancel::new(), &Backoff::with_attempts(1)).expect("again");
        assert_eq!(fetch.count(), 2, "one request each, and neither twice");
        let urls: Vec<String> = fetch.requests().iter().map(|r| r.url.clone()).collect();
        assert!(urls.contains(&user_url("2REoufqX")));
        assert!(urls.contains(&user_projects_url("2REoufqX")));
    }

    #[test]
    fn the_three_tag_routes_are_asked_separately_and_the_filter_lists_are_queries_on_them() {
        let (api, fetch) = api("tags", DEFAULT_TTL);
        fetch.set_route(&tag_categories_url(), Route::text(TAGS_CATEGORY_BODY));
        fetch.set_route(&tag_game_versions_url(), Route::text(TAGS_GAME_VERSION_BODY));
        fetch.set_route(&tag_loaders_url(), Route::text(TAGS_LOADER_BODY));

        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);
        let tags = crate::Tags {
            categories: api.tag_categories(&cancel, &backoff).expect("the categories"),
            game_versions: api.tag_game_versions(&cancel, &backoff).expect("the versions"),
            loaders: api.tag_loaders(&cancel, &backoff).expect("the loaders"),
        };
        assert_eq!(tags.categories.len(), 4);
        assert_eq!(tags.game_versions.len(), 3);
        assert_eq!(tags.loaders.len(), 2);

        // A browse sidebar's section is one (project type, header) pair, and its
        // `FilterType` id is spelled out of the two -- which is why `headers`
        // exists and why it is the API's order rather than an alphabetical one.
        assert_eq!(tags.headers("modpack"), vec!["technical", "gameplay"]);
        assert_eq!(tags.headers("mod"), vec!["technology"]);
        let technical: Vec<&str> = tags
            .categories_under("modpack", "technical")
            .iter()
            .map(|category| category.name.as_str())
            .collect();
        assert_eq!(technical, vec!["kitchen-sink"]);

        // And the loaders are filtered by the project types they are offered for,
        // because `fabric` and the modpack loaders are rows of different lists.
        let modpack: Vec<&str> =
            tags.loaders_for("modpack").iter().map(|loader| loader.name.as_str()).collect();
        assert_eq!(modpack, vec!["fabric"]);
        let mods: Vec<&str> =
            tags.loaders_for("mod").iter().map(|loader| loader.name.as_str()).collect();
        assert_eq!(mods, vec!["fabric", "forge"]);

        // A version's type is what puts it under *Show all versions*, so it has to
        // survive the read rather than being flattened to a string.
        let releases: Vec<&str> = tags
            .game_versions_of("release")
            .iter()
            .map(|version| version.version.as_str())
            .collect();
        assert_eq!(releases, vec!["1.21.4", "1.21.1"]);

        // Asked again, all three come from the cache.
        api.tag_categories(&cancel, &backoff).expect("again");
        api.tag_game_versions(&cancel, &backoff).expect("again");
        api.tag_loaders(&cancel, &backoff).expect("again");
        assert_eq!(fetch.count(), 3, "one request each, and not a second for the second read");
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
        // The two fields a card draws tags from are the hit's own, not a second
        // request per card: the game versions and the categories (loaders among
        // them) come back with the search.
        assert_eq!(response.hits[0].versions, vec!["1.21.4", "1.21.3"]);
        assert_eq!(response.hits[0].categories, vec!["fabric", "optimization"]);
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
    fn a_project_and_its_team_are_two_cached_documents() {
        let (api, fetch) = api("project", DEFAULT_TTL);
        fetch.set_route(&project_url("AANobbMI"), Route::text(PROJECT_BODY));
        fetch.set_route(&project_members_url("AANobbMI"), Route::text(MEMBERS_BODY));

        let project = api
            .project("AANobbMI", &Cancel::new(), &Backoff::with_attempts(1))
            .expect("the project");
        assert_eq!(project.title, "Sodium");
        assert_eq!(project.project_type, "mod");
        assert_eq!(project.downloads, 41_000_000);
        assert_eq!(project.game_versions, vec!["1.21.4", "1.21.3"]);
        assert_eq!(project.loaders, vec!["fabric"]);
        assert_eq!(project.gallery.len(), 1);
        assert_eq!(project.gallery[0].title, "In the nether");
        assert!(project.body.contains("# Sodium"), "{}", project.body);

        let members = api
            .members("AANobbMI", &Cancel::new(), &Backoff::with_attempts(1))
            .expect("the team");
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].role, "Owner");
        assert_eq!(members[0].user.username, "jellysquid3");
        assert_eq!(members[1].user.username, "embeddedt");

        // Both are cached under their own URL: asked again, no request at all,
        // and the two handles are separate entries rather than one document.
        api.project("AANobbMI", &Cancel::new(), &Backoff::with_attempts(1)).expect("again");
        api.members("AANobbMI", &Cancel::new(), &Backoff::with_attempts(1)).expect("again");
        assert_eq!(fetch.count(), 2, "a second read of each is free");
        assert!(api.projects.cached(&project_url("AANobbMI")).is_some());
        assert!(api.projects.cached(&project_members_url("AANobbMI")).is_some());
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
