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
    search_url_parts_with_facets(query, project_type, index, limit, offset, &[])
}

/// The same URL, with the caller's own facet groups beside the project type's.
///
/// Modrinth's `facets` parameter is a list of *or* groups -- `[["a","b"],["c"]]`
/// matches a project tagged `a` or `b`, and also tagged `c` -- so a second
/// constraint is a second group rather than a second parameter. The reference
/// builds exactly that shape in `ui/src/utils/search.ts`, where every exclusion
/// lands in one group per field (`project_id NOT IN ["a","b"]` is one group, not
/// two), which is why the groups are taken whole and pre-spelled here: this is
/// the shape a caller means, not a field/value pair for this module to re-derive.
///
/// A group goes inside one pair of quotes as it stands, which is what makes the
/// exclusion syntax work at all: `project_id NOT IN ["AANobbMI"]` carries its own
/// quotes and has to. Escaping them is therefore the caller's -- and there is
/// nothing for this module to guess at, because a group is opaque on the way in
/// and on the way out.
pub fn search_url_parts_with_facets(
    query: &str,
    project_type: Option<&str>,
    index: Option<&str>,
    limit: u32,
    offset: u32,
    extra_facets: &[String],
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
    let mut groups: Vec<String> = Vec::new();
    if let Some(project_type) = project_type.filter(|kind| !kind.is_empty()) {
        groups.push(format!("project_type:{project_type}"));
    }
    groups.extend(extra_facets.iter().cloned());
    if !groups.is_empty() {
        // Every group is an array of its own: `facets` is a list of or-groups,
        // and `[["a"]]` is one group of one while `["a"]` is the group itself.
        let quoted: Vec<String> = groups.iter().map(|group| format!("[\"{group}\"]")).collect();
        let facets = format!("[{}]", quoted.join(","));
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

/// The URL of the category list: `GET /v2/tag/category`.
///
/// There is no `/tags`: the three lists a browse page's filters are built from --
/// categories, game versions and loaders -- are three routes, not one document,
/// which is why the reference's `get_categories`, `get_game_versions` and
/// `get_loaders` are three calls rather than three fields. Each is a *list*
/// document that only changes when Modrinth ships, so the engine holds them on
/// the slow clock rather than the search one.
pub fn tag_categories_url() -> String {
    format!("{MODRINTH_BASE_URL}/tag/category")
}

/// The URL of the game-version list: `GET /v2/tag/game_version`.
pub fn tag_game_versions_url() -> String {
    format!("{MODRINTH_BASE_URL}/tag/game_version")
}

/// The URL of the loader list: `GET /v2/tag/loader`.
pub fn tag_loaders_url() -> String {
    format!("{MODRINTH_BASE_URL}/tag/loader")
}

/// One game version Modrinth knows about.
///
/// `version` is what a filter option's `query_value` carries and `version_type`
/// is what decides whether it sits under the *Show all versions* toggle group
/// (`search.ts`: anything that is not a `release`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct GameVersionTag {
    /// The version number, e.g. `1.21.4`.
    pub version: String,
    /// `release`, `snapshot`, `beta` or `alpha`.
    pub version_type: String,
}

/// One loader Modrinth knows about.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct LoaderTag {
    /// The loader's name as the API spells it, e.g. `fabric`.
    pub name: String,
    /// Which project types this loader is offered for.
    ///
    /// Not decoration: `search.ts` reads it to decide which filter lists a loader
    /// belongs in, so `fabric` (mods) and `mrpack`'s loaders are different rows
    /// from the same list.
    pub supported_project_types: Vec<String>,
}

/// One category Modrinth knows about.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct CategoryTag {
    /// The category's name as the API spells it, e.g. `technology`.
    pub name: String,
    /// Which project type it describes.
    pub project_type: String,
    /// The header it is filed under, e.g. `technical` or `gameplay`.
    ///
    /// The header is what the browse sidebar's own sections are named from
    /// (`formatCategoryHeader`), so two categories sharing one are two rows of
    /// the same list.
    pub header: String,
}

/// `GET /v2/tags`, as the browse page's filters read it.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize)]
pub struct Tags {
    /// Every game version Modrinth lists.
    pub game_versions: Vec<GameVersionTag>,
    /// Every loader Modrinth lists.
    pub loaders: Vec<LoaderTag>,
    /// Every category Modrinth lists.
    pub categories: Vec<CategoryTag>,
}

impl Tags {
    /// The categories of one project type, filed under `header`.
    ///
    /// The reference builds one `FilterType` per `(project_type, header)` pair
    /// (`search.ts`'s `category_${project_type}_${header}`), so this is the query
    /// behind one of the sidebar's sections.
    pub fn categories_under(&self, project_type: &str, header: &str) -> Vec<&CategoryTag> {
        self.categories
            .iter()
            .filter(|category| category.project_type == project_type && category.header == header)
            .collect()
    }

    /// Every header one project type has categories under, in the order the API
    /// first lists them.
    pub fn headers(&self, project_type: &str) -> Vec<&str> {
        let mut headers: Vec<&str> = Vec::new();
        for category in &self.categories {
            if category.project_type == project_type && !headers.contains(&category.header.as_str())
            {
                headers.push(category.header.as_str());
            }
        }
        headers
    }

    /// The loaders offered for one project type.
    pub fn loaders_for(&self, project_type: &str) -> Vec<&LoaderTag> {
        self.loaders
            .iter()
            .filter(|loader| loader.supported_project_types.iter().any(|kind| kind == project_type))
            .collect()
    }

    /// The game versions of one release type, newest first as the API lists them.
    pub fn game_versions_of(&self, version_type: &str) -> Vec<&GameVersionTag> {
        self.game_versions
            .iter()
            .filter(|version| version.version_type == version_type)
            .collect()
    }
}

/// Build the version-list URL for a project id or slug.
///
/// Calls `GET /v2/project/{project}/version` (all loaders/game versions;
/// filtering happens client-side).
pub fn version_url(project: &str) -> String {
    format!("{}/project/{}/version", MODRINTH_BASE_URL, percent_encode_path(project))
}

/// Read a string field that the service is allowed to publish as `null`.
///
/// `#[serde(default)]` covers a field that is *absent* from a document; it does
/// not cover one that is present and `null`, and `null` is exactly how Modrinth
/// writes an account with no display name -- measured on `/v2/user/modrinth`,
/// whose `name` is `null` while its `username` is a string. Without this, one
/// unset word would fail the parse of the whole profile. A `null` read through
/// here becomes the empty string, which is what the reference draws anyway.
///
/// Only the fields known to be nullable use it. `username`, `id`, `bio` and the
/// dates are strings in the service's own schema, and defaulting them to empty on
/// a `null` would be this launcher inventing a shape the API does not have.
///
/// Four more fields joined that list because the service published the `null`
/// rather than omitting the field, which is the whole distinction: a gallery
/// image's caption (`title` and `description`), and a search hit's `icon_url`
/// and `latest_version`. The first pair is measured: `GET /v2/project/rQiXwLhB`
/// carries `"gallery":[{"url":"...","title":null,"description":null,...}]`,
/// and the parse of that *whole document* used to fail at the `null`'s last
/// byte -- the reader saw `line 1 column 9168` in place of the project.
/// `ModrinthUserProject::icon_url` had already met the same shape on an older
/// document; these are the fields that had not.
fn null_as_empty<'de, D>(reader: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(<Option<String> as serde::Deserialize>::deserialize(reader)?.unwrap_or_default())
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
    /// Icon URL, `null` for a project that has not uploaded one -- the same
    /// nullable shape [`ModrinthUserProject::icon_url`] already reads.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub icon_url: String,
    /// Newest version id, `null` for a project whose versions have all been
    /// withdrawn. One such hit used to fail the whole search page, because
    /// `default` covers an absent field and not a present `null`.
    #[serde(default, deserialize_with = "null_as_empty")]
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
    /// What changed in this version, in markdown.
    #[serde(default)]
    pub changelog: String,
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

/// Build the project URL for a project id or slug.
///
/// Calls `GET /v2/project/{project}` -- the document a project page's header is.
pub fn project_url(project: &str) -> String {
    format!("{}/project/{}", MODRINTH_BASE_URL, percent_encode_path(project))
}

/// Build the team-members URL for a project id or slug.
///
/// Calls `GET /v2/project/{project}/members`. A project document names the team
/// it belongs to but not the people on it, and Modrinth's own project page shows
/// one of them by name -- so the name is a second request, against this URL.
pub fn project_members_url(project: &str) -> String {
    format!("{}/project/{}/members", MODRINTH_BASE_URL, percent_encode_path(project))
}

/// One `GET /v2/project/{id}` response (subset; unknown fields ignored).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ModrinthProject {
    /// Project id.
    #[serde(default)]
    pub id: String,
    /// URL slug.
    #[serde(default)]
    pub slug: String,
    /// Modrinth's own project type (`mod`, `modpack`, `resourcepack`, ...).
    #[serde(default)]
    pub project_type: String,
    /// Title.
    #[serde(default)]
    pub title: String,
    /// The one-line summary.
    #[serde(default)]
    pub description: String,
    /// The long description, in markdown.
    #[serde(default)]
    pub body: String,
    /// Total downloads.
    #[serde(default)]
    pub downloads: u64,
    /// Followers.
    #[serde(default)]
    pub followers: u64,
    /// The game versions it has a version for.
    #[serde(default)]
    pub game_versions: Vec<String>,
    /// The loaders it runs on.
    #[serde(default)]
    pub loaders: Vec<String>,
    /// The gallery, in the order the author put it in.
    #[serde(default)]
    pub gallery: Vec<ModrinthGalleryImage>,
}

/// One entry of a project's `gallery` array.
///
/// An image is a URL and a caption; the caption has a title and a longer
/// description, and either may be *null* -- not merely absent, which is why
/// both read through [`null_as_empty`] -- and the page falls back to the URL
/// when it draws one.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ModrinthGalleryImage {
    /// Where the image is.
    #[serde(default)]
    pub url: String,
    /// The caption's title: `null` for an image the author captioned with none.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub title: String,
    /// The caption's description, `null` the same way and just as often.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub description: String,
}

/// One `GET /v2/project/{id}/members` entry (subset).
///
/// The entry wraps the *whole* user document, not a cut-down one: this tree once
/// had a second `ModrinthUser` holding nothing but `username`, which is the same
/// document as [`ModrinthUser`] below with every other field dropped. Two types
/// for one object is how a member's name and a profile's name end up parsed by
/// different code; the member keeps the same type and reads the one field it
/// draws.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ModrinthMember {
    /// The user on the team.
    #[serde(default)]
    pub user: ModrinthUser,
    /// Their role: `Owner`, `Member`, and so on.
    #[serde(default)]
    pub role: String,
}

/// Modrinth's news feed: the one Modrinth document this launcher reads that is
/// not the API.
///
/// `App.vue` fetches exactly this URL at startup and takes `res.articles`, so the
/// panel's news section is a publisher's own file rather than a scraped page. It
/// is not under `/v2` and it is not a project's metadata; a feed of
/// announcements, published as JSON for the news page and its readers.
pub const NEWS_URL: &str = "https://modrinth.com/news/feed/articles.json";

/// The news page the panel's *View all news* button opens: the reference's own
/// `href` in `App.vue`, spelled once here because the shell draws it and nothing
/// else in this launcher writes a URL by hand.
pub const NEWS_PAGE_URL: &str = "https://modrinth.com/news";

/// The feed's own envelope, `{ "articles": [...] }`.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
pub struct NewsFeed {
    /// The articles, newest first as the feed publishes them.
    #[serde(default)]
    pub articles: Vec<NewsArticle>,
}

/// One news article.
///
/// Five fields, measured off the live feed rather than guessed: the title, the
/// one-line summary, a thumbnail URL, an ISO-8601 date, and the article's own
/// link. Everything is defaulted, because a feed is a stranger's JSON and one
/// article missing a summary should cost the panel a paragraph rather than the
/// whole section.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
pub struct NewsArticle {
    /// The headline.
    #[serde(default)]
    pub title: String,
    /// One line under it.
    #[serde(default)]
    pub summary: String,
    /// The card's image, as a URL on Modrinth's own CDN.
    #[serde(default)]
    pub thumbnail: String,
    /// When it was published, ISO-8601 (`2026-09-07T19:00:00.000Z`).
    #[serde(default)]
    pub date: String,
    /// The article's own page on modrinth.com.
    #[serde(default)]
    pub link: String,
}

/// The month names the reference's `dateStyle: 'long'` uses, in its default
/// locale. Twelve entries rather than a date crate: this launcher draws one
/// format, and a dependency that can format any date is a dependency that has to
/// be kept current for a word.
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl NewsArticle {
    /// The publication date as the reference draws it: `September 7, 2026`.
    ///
    /// Read out of the ISO-8601 string's own first ten characters, which is the
    /// only part of it this needs; a time zone would move the *day* of an evening
    /// announcement, and the reference reads the same string the same way.
    ///
    /// A date that cannot be read is drawn as it was published rather than as
    /// nothing: a wrong-looking date is a bug report, a missing one is silence.
    pub fn date_label(&self) -> String {
        date_label(&self.date)
    }
}

/// The publication date as the reference draws it: `September 7, 2026`.
///
/// Read out of the ISO-8601 string's own first ten characters, which is the only
/// part of it this needs; a time zone would move the *day* of an evening
/// announcement, and the reference reads the same string the same way. A date that
/// cannot be read comes back as it arrived rather than as nothing: a wrong-looking
/// date is a bug report, a missing one is silence.
///
/// A free function because three documents now carry dates in the same shape -- an
/// article's publication, a user's `created`, and a project's `published` -- and one
/// formatter is what keeps them from drifting apart.
pub fn date_label(iso: &str) -> String {
    let date = iso.get(..10).unwrap_or_default();
    let mut parts = date.split('-');
    let year = parts.next().and_then(|part| part.parse::<i32>().ok());
    let month = parts.next().and_then(|part| part.parse::<usize>().ok());
    let day = parts.next().and_then(|part| part.parse::<u32>().ok());
    match (year, month, day) {
        (Some(year), Some(month), Some(day)) if (1..=12).contains(&month) => {
            format!("{} {day}, {year}", MONTHS[month - 1])
        }
        _ => iso.to_string(),
    }
}

/// One user's profile, as Modrinth's own document writes it.
///
/// The reference's page draws this header through `plugin:users|get_user_profile`,
/// which wraps Labrinth's *v3* user service -- a route outside Modrinth's published
/// API. What is here is the **published** `/v2/user/{username}` document instead: the
/// same account, the same fields, and the same URL the reference's own web client
/// uses, which is why this launcher can draw a real profile without a Modrinth
/// session. The v3-only halves of that page -- collections, organizations, anything
/// that needs the reader to be signed in -- are the parts that stay named as absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModrinthUser {
    /// The account's id, which is what the projects list is asked for by.
    #[serde(default)]
    pub id: String,
    /// The username, which is what `/user/:user` is matched against.
    #[serde(default)]
    pub username: String,
    /// The display name, which is `null` for an account that has not set one.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub name: String,
    /// Where the avatar is, if they have one.
    #[serde(default)]
    pub avatar_url: String,
    /// Their own sentence about themselves, if they wrote one.
    #[serde(default)]
    pub bio: String,
    /// When the account was created, in the API's ISO-8601 shape.
    #[serde(default)]
    pub created: String,
}

impl ModrinthUser {
    /// The name to draw: the display name when there is one, the username otherwise.
    ///
    /// The reference draws both -- `name` as the heading and `@username` under it --
    /// and an account with no display name has only the username to draw once.
    pub fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            &self.username
        } else {
            &self.name
        }
    }

    /// Whether the display name is a second name rather than the username repeated.
    ///
    /// A heading that is the username and a line under it that is the same string
    /// with an `@` in front is a line worth skipping.
    pub fn has_separate_username(&self) -> bool {
        !self.name.trim().is_empty() && !self.name.eq_ignore_ascii_case(&self.username)
    }

    /// When they joined, as the feed's dates are written.
    pub fn joined_label(&self) -> String {
        date_label(&self.created)
    }
}

/// One project a user owns, as `/v2/user/{id}/projects` writes them.
///
/// The same documents a search returns, with one difference that matters: the
/// project document keys its own id as `id`, where a search hit keys it as
/// `project_id`. That is why this is its own type rather than a second alias on
/// [`ModrinthSearchHit`] -- a struct that answers to two spellings of the same field
/// would accept a document that has neither.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModrinthUserProject {
    /// The project's id.
    #[serde(default)]
    pub id: String,
    /// Its URL slug, which is what a link to it is built from.
    #[serde(default)]
    pub slug: String,
    /// Its title.
    #[serde(default)]
    pub title: String,
    /// Its one-line summary.
    #[serde(default)]
    pub description: String,
    /// When it was published.
    #[serde(default)]
    pub published: String,
    /// Total downloads.
    #[serde(default)]
    pub downloads: u64,    /// Its icon, if it has one: `null` for a project that has not uploaded one,
    /// which is the one field of this document the service marks nullable.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub icon_url: String,


    /// Its type (`mod`, `modpack`, `resourcepack`, ...), as the document says.
    #[serde(default)]
    pub project_type: String,
}

/// The profile document for one user, by username or by id.
///
/// `/v2/user/{id|username}` answers either, which is what lets the projects list be
/// asked for by id afterwards: a display name can change between two requests, an id
/// cannot.
pub fn user_url(user: &str) -> String {
    format!("{MODRINTH_BASE_URL}/user/{}", percent_encode(user))
}

/// Every project one user owns.
pub fn user_projects_url(user: &str) -> String {
    format!("{MODRINTH_BASE_URL}/user/{}/projects", percent_encode(user))
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

    /// An article with just a date, which is all `date_label` reads.
    fn dated(date: &str) -> NewsArticle {
        NewsArticle { date: date.to_string(), ..NewsArticle::default() }
    }

    #[test]
    fn a_gallery_image_the_author_captioned_with_nothing_still_reads() {
        // `GET /v2/project/rQiXwLhB` as the API answers it, trimmed to the
        // gallery: the first image's `title` and `description` are `null`. Before
        // `null_as_empty` was on them this whole document failed to parse -- the
        // reader was shown `invalid type: null, expected a string at line 1
        // column 9168`, and that column is this `null`'s last byte, so the
        // project page drew the error instead of the project.
        let body = r##"{
            "id": "rQiXwLhB",
            "slug": "battlearmorytacz",
            "project_type": "modpack",
            "title": "BattleArmory TACZ",
            "description": "\u6218\u5730\u6b66\u5e93 TACZ",
            "body": "# BattleArmory",
            "downloads": 3507037,
            "followers": 87,
            "game_versions": ["1.20.1"],
            "loaders": ["forge"],
            "gallery": [{
                "url": "https://cdn.modrinth.com/data/rQiXwLhB/images/1f8cafea.jpeg",
                "raw_url": "https://cdn.modrinth.com/data/rQiXwLhB/images/1f8cafea.jpeg",
                "featured": false,
                "title": null,
                "description": null,
                "created": "2025-03-09T09:08:19.516923Z",
                "ordering": 0
            }]
        }"##;
        let project: ModrinthProject =
            serde_json::from_str(body).expect("the project document");
        assert_eq!(project.title, "BattleArmory TACZ");
        assert_eq!(project.downloads, 3_507_037);
        assert_eq!(project.gallery.len(), 1);
        // The image is still drawable: its caption is simply empty, which is the
        // fallback the page already had.
        let image = &project.gallery[0];
        assert!(image.url.ends_with("1f8cafea.jpeg"));
        assert_eq!(image.title, "");
        assert_eq!(image.description, "");
        // And a caption that *is* there is not lost to the tolerance.
        let captioned: ModrinthGalleryImage = serde_json::from_str(
            r#"{"url":"https://cdn/x.png","title":"Banner","description":"A shot"}"#,
        )
        .expect("a captioned image");
        assert_eq!(captioned.title, "Banner");
        assert_eq!(captioned.description, "A shot");
    }

    #[test]
    fn a_hit_with_no_icon_and_no_version_still_reads() {
        // The same two fields on a *search hit*: a project with no icon and no
        // published version has `null` in both, and one such hit would fail the
        // whole page of results rather than one card.
        let body = r#"{ "hits": [
            { "project_id": "rQiXwLhB", "slug": "battlearmorytacz",
              "title": "BattleArmory TACZ", "description": "\u6218\u5730\u6b66\u5e93",
              "author": "JZ_zhenmeng", "downloads": 3507037, "follows": 87,
              "icon_url": null, "latest_version": null,
              "versions": ["1.20.1"], "categories": ["forge"] },
            { "project_id": "sodium", "slug": "sodium", "title": "Sodium",
              "description": "A rendering engine", "author": "jellysquid3",
              "downloads": 1, "follows": 1,
              "icon_url": "https://cdn.modrinth.com/data/AANobbMI/icon.webp",
              "latest_version": "mc1.21.1-0.6.0" }
        ], "offset": 0, "limit": 20, "total_hits": 2 }"#;
        let response: ModrinthSearchResponse =
            serde_json::from_str(body).expect("the search response");
        assert_eq!(response.hits.len(), 2);
        assert_eq!(response.hits[0].project_ref(), "rQiXwLhB");
        assert_eq!(response.hits[0].icon_url, "");
        assert_eq!(response.hits[0].latest_version, "");
        assert!(response.hits[1].icon_url.ends_with("icon.webp"));
        assert_eq!(response.hits[1].latest_version, "mc1.21.1-0.6.0");
    }

    #[test]
    fn a_news_date_is_drawn_the_way_the_reference_draws_it() {
        // `dateStyle: 'long'` in the reference's own locale: `September 7, 2026`.
        // The day is not zero-padded, because the reference's formatter is not.
        assert_eq!(dated("2026-09-07T19:00:00.000Z").date_label(), "September 7, 2026");
        assert_eq!(dated("2026-01-31T00:00:00.000Z").date_label(), "January 31, 2026");
        assert_eq!(dated("2026-12-01").date_label(), "December 1, 2026");
        // A date this launcher cannot read is drawn as it was published rather
        // than as nothing: a wrong-looking date is a bug report, an empty one is
        // silence. A month of 13 is not a month.
        assert_eq!(dated("not a date").date_label(), "not a date");
        assert_eq!(dated("2026-13-01T00:00:00Z").date_label(), "2026-13-01T00:00:00Z");
        assert_eq!(dated("").date_label(), "");
    }

    #[test]
    fn the_feed_is_an_articles_envelope_and_nothing_else_is_required() {
        // The shape the live feed serves, measured: one top-level key, and every
        // article field optional so that one article missing a summary costs the
        // panel a paragraph rather than the whole section.
        let feed: NewsFeed = serde_json::from_str(
            r#"{"articles": [{"title": "A", "date": "2026-09-07T19:00:00.000Z"}]}"#,
        )
        .expect("the feed's own shape");
        assert_eq!(feed.articles.len(), 1);
        assert_eq!(feed.articles[0].title, "A");
        assert_eq!(feed.articles[0].summary, "");
        assert_eq!(feed.articles[0].link, "");
        // And an envelope with no `articles` at all is an empty feed rather than
        // a parse failure: the panel draws nothing for it either way.
        let empty: NewsFeed = serde_json::from_str("{}").expect("no articles key");
        assert!(empty.articles.is_empty());
    }

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
    fn a_search_url_carries_the_callers_own_facet_groups_beside_the_project_type() {
        // `facets` is a list of or-groups, so a project type and a caller's own
        // constraint are *two* groups -- both have to hold -- rather than two
        // values in one group, which would mean "a project of this type or a
        // project that is not this one".
        let url = search_url_parts_with_facets(
            "sodium",
            Some("modpack"),
            None,
            20,
            0,
            &[r#"project_id NOT IN ["AANobbMI","fabric-api"]"#.to_string()],
        );
        assert_eq!(
            url,
            "https://api.modrinth.com/v2/search?query=sodium&limit=20\
             &facets=%5B%5B%22project_type%3Amodpack%22%5D%2C%5B%22project_id%20NOT%20IN%20%5B%22AANobbMI%22%2C%22fabric-api%22%5D%22%5D%5D"
        );
        // A group with no project type beside it is the whole of `facets`.
        assert_eq!(
            search_url_parts_with_facets("sodium", None, None, 20, 0, &["categories = \"forge\"".into()]),
            "https://api.modrinth.com/v2/search?query=sodium&limit=20\
             &facets=%5B%5B%22categories%20%3D%20%22forge%22%22%5D%5D"
        );
        assert_eq!(
            search_url_parts_with_facets("sodium", None, None, 20, 0, &[]),
            search_url_parts("sodium", None, None, 20, 0),
            "no facets is the URL that had none"
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
            changelog: String::new(),
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
            changelog: String::new(),
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
    fn a_user_is_asked_for_by_name_and_their_projects_by_their_number() {
        // `/v2/user/{id|username}` takes either, which is the whole reason the
        // projects list can be asked for by the id the first answer gives: a
        // display name can change between two requests, an id cannot.
        assert_eq!(user_url("jellysquid3"), "https://api.modrinth.com/v2/user/jellysquid3");
        assert_eq!(
            user_projects_url("2REoufqX"),
            "https://api.modrinth.com/v2/user/2REoufqX/projects"
        );
        // A name with a space in it is one path segment, not two.
        assert!(user_url("a b").ends_with("/user/a%20b"));
    }

    #[test]
    fn a_user_s_own_document_is_read_field_by_field() {
        // The shape `GET /v2/user/Modrinth` answers with, trimmed: `name` is null
        // for an account that has not set one, which is the case the header has to
        // survive.
        let body = r#"{
            "id": "2REoufqX",
            "username": "Modrinth",
            "name": null,
            "avatar_url": "https://cdn.modrinth.com/data/2REoufqX/abc_96.webp",
            "bio": "An official user account of Modrinth.",
            "created": "2023-11-13T23:22:36.604990Z",
            "role": "admin",
            "badges": 1
        }"#;
        let user: ModrinthUser = serde_json::from_str(body).expect("the profile");
        assert_eq!(user.id, "2REoufqX");
        assert_eq!(user.username, "Modrinth");
        assert!(user.avatar_url.ends_with("abc_96.webp"));
        // A profile with no display name is drawn under its username, and there is
        // no second name to draw under that.
        assert_eq!(user.display_name(), "Modrinth");
        assert!(!user.has_separate_username());
        assert_eq!(user.joined_label(), "November 13, 2023");
        // A display name that *is* the username (in any case) is the same answer:
        // one name rather than the same string twice.
        let same: ModrinthUser =
            serde_json::from_str(r#"{"username":"jellysquid3","name":"JellySquid3"}"#)
                .expect("the profile");
        assert!(!same.has_separate_username());
        let named: ModrinthUser =
            serde_json::from_str(r#"{"username":"jellysquid3","name":"Jelly"}"#)
                .expect("the profile");
        assert_eq!(named.display_name(), "Jelly");
        assert!(named.has_separate_username());
        // A date that cannot be read is drawn as it arrived rather than as nothing.
        let odd: ModrinthUser = serde_json::from_str(r#"{"created":"sometime"}"#).expect("parsed");
        assert_eq!(odd.joined_label(), "sometime");
    }

    #[test]
    fn a_user_s_projects_are_the_documents_the_rest_of_the_api_writes() {
        // Two entries out of `GET /v2/user/jellysquid3/projects`, with the fields
        // this launcher draws and the key that makes them a type of their own: the
        // id is `id` here and `project_id` in a search hit.
        let body = r#"[
            {"id":"hEOCdOgW","slug":"phosphor","project_type":"mod","title":"Phosphor",
             "description":"No-compromises lighting engine optimization mod",
             "published":"2021-01-03T00:58:54.900351Z","downloads":865848,
             "icon_url":"https://cdn.modrinth.com/data/hEOCdOgW/abc.png"},
            {"id":"AANobbMI","slug":"sodium","project_type":"mod","title":"Sodium",
             "description":"Modern rendering engine","downloads":50000000}
        ]"#;
        let projects: Vec<ModrinthUserProject> = serde_json::from_str(body).expect("the list");
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].id, "hEOCdOgW");
        assert_eq!(projects[0].slug, "phosphor");
        assert_eq!(projects[0].downloads, 865848);
        assert_eq!(date_label(&projects[0].published), "January 3, 2021");
        // An entry with no icon is still an entry: the card says so rather than
        // dropping a project the reader owns.
        assert!(projects[1].icon_url.is_empty());
        assert_eq!(projects[1].title, "Sodium");
        assert!(serde_json::from_str::<Vec<ModrinthUserProject>>("[]").is_ok());
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
