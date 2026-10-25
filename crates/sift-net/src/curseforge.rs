//! CurseForge's Core API, as the launcher asks it.
//!
//! The second catalogue beside Modrinth's, and a deliberately thinner one: what
//! is here is the vocabulary (URLs, the documents they answer with, the rules
//! about which files may be fetched) and one function per request. Nothing here
//! caches, and that is not an oversight -- see [`fetch_search`].
//!
//! ## Two clauses in Overwolf's terms decide the shape of this module
//!
//! * *"Developer shall not … save or cache any data obtained through the API or
//!   SDK."* So a CurseForge answer goes straight to the page that asked and is
//!   never written to [`crate::engine::cache::MetadataCache`] the way a Modrinth
//!   search is. The engine's cache is still used for everything else; this
//!   module simply does not route through it. `sift-desktop`'s own store is
//!   where the request is *made*, and it keeps no copy for the same reason.
//! * *"…in order build any product or service that competes, directly or
//!   indirectly, with CF."* The launcher browses and installs; it does not
//!   mirror the catalogue, bulk-export it, or offer itself as a second
//!   CurseForge. Nothing in this module pages through the whole index on its
//!   own: every request is one page of one question a reader typed.
//!
//! ## Which files may be fetched from a third-party client
//!
//! A mod carries `allowModDistribution`. `Some(false)` means the author refused
//! distribution outside CurseForge's own client, and [`CfMod::distribution_allowed`]
//! is the only correct reading of it: absent and `null` are read as *allowed*,
//! which is the API's own default and the answer CurseForge gives for a project
//! that never touched the setting. A file whose `downloadUrl` is null is the
//! second half of the same rule: the URL is withheld until the file is asked
//! for by id, and until then a client has nothing to fetch.
//!
//! ## What is deliberately absent
//!
//! No data-pack or plugin class id. CurseForge publishes several class ids per
//! game and the two this launcher does not browse were never measured against
//! the live service, so they are left out rather than guessed: a wrong `classId`
//! is not an error, it is a silently empty page. The four that are here are the
//! ones a reader of the CurseForge site can be pointed at (`/minecraft/modpacks`
//! and friends) and were each confirmed by the category each search answers.

use std::path::Path;

use crate::engine::cancel::Cancel;
use crate::engine::request::{Fetch, Request};
use crate::Error;

/// CurseForge's API root.
pub const CURSEFORGE_BASE_URL: &str = "https://api.curseforge.com/v1";

/// The game id CurseForge files Minecraft under.
///
/// A constant rather than a parameter: this launcher has no other game, and a
/// call site that could pass the wrong one is a call site that eventually does.
pub const MINECRAFT_GAME_ID: u32 = 432;

/// The header the API key travels in.
pub const API_KEY_HEADER: &str = "x-api-key";

/// How many results one page may hold.
///
/// The service accepts 50 and answers 400 above it; asking for more than it
/// allows is a request that fails rather than a page that is bigger.
pub const MAX_PAGE_SIZE: u32 = 50;

/// One browsable kind of project, as the API's `classId` names it.
///
/// The ids are the ones CurseForge's own site uses for Minecraft; the comment on
/// each is the page it is the id of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CfClass {
    /// Mods (`/minecraft/mc-mods`).
    #[default]
    Mod = 6,
    /// Resource packs (`/minecraft/texture-packs`).
    ResourcePack = 12,
    /// Worlds (`/minecraft/worlds`).
    World = 17,
    /// Modpacks (`/minecraft/modpacks`).
    Modpack = 4471,
    /// Shaders (`/minecraft/shaders`).
    Shader = 6552,
}

impl CfClass {
    /// The `classId` the search request carries.
    pub fn id(self) -> u32 {
        self as u32
    }

    /// Every kind this launcher browses.
    pub fn all() -> [CfClass; 5] {
        [
            CfClass::Modpack,
            CfClass::Mod,
            CfClass::ResourcePack,
            CfClass::World,
            CfClass::Shader,
        ]
    }

    /// The label a tab strip draws.
    pub fn label(self) -> &'static str {
        match self {
            CfClass::Modpack => "Modpacks",
            CfClass::Mod => "Mods",
            CfClass::ResourcePack => "Resource Packs",
            CfClass::World => "Worlds",
            CfClass::Shader => "Shaders",
        }
    }
}

/// How a page of results is ordered.
///
/// Only the three a browse page offers; the API's full list also has name,
/// author and date created, which sort the catalogue rather than help choose
/// from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CfSort {
    /// `sortField=2`.
    #[default]
    Popularity,
    /// `sortField=6`.
    Downloads,
    /// `sortField=3`.
    Updated,
    /// `sortField=11`.
    Released,
}

impl CfSort {
    /// The `sortField` value.
    pub fn field(self) -> u32 {
        match self {
            CfSort::Popularity => 2,
            CfSort::Downloads => 6,
            CfSort::Updated => 3,
            CfSort::Released => 11,
        }
    }
}

/// The mod loader a search is narrowed to.
///
/// `Any` is sent as no parameter at all: a request carrying `modLoaderType=0`
/// and one carrying none mean the same thing to the service, and the second is
/// the one its own site sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CfLoader {
    /// Every loader.
    #[default]
    Any,
    /// Forge.
    Forge,
    /// Fabric.
    Fabric,
    /// Quilt.
    Quilt,
    /// NeoForge.
    NeoForge,
}

impl CfLoader {
    /// The `modLoaderType` value, or `None` for [`CfLoader::Any`].
    pub fn id(self) -> Option<u32> {
        match self {
            CfLoader::Any => None,
            CfLoader::Forge => Some(1),
            CfLoader::Fabric => Some(4),
            CfLoader::Quilt => Some(5),
            CfLoader::NeoForge => Some(6),
        }
    }

    /// The loader name the desktop crate already uses for its instance filters,
    /// so one loader is spelled the same in a search and in an instance.
    pub fn from_loader_name(name: &str) -> CfLoader {
        match name {
            "forge" => CfLoader::Forge,
            "fabric" => CfLoader::Fabric,
            "quilt" => CfLoader::Quilt,
            "neoforge" => CfLoader::NeoForge,
            _ => CfLoader::Any,
        }
    }
}

/// One question: what to search for, and how to narrow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfQuery {
    /// Free text, sent as `searchFilter`. Empty means "everything in `class`".
    pub text: String,
    /// Which kind of project.
    pub class: CfClass,
    /// A Minecraft version to require, as `gameVersion`.
    pub game_version: String,
    /// A loader to require.
    pub loader: CfLoader,
    /// How the page is ordered.
    pub sort: CfSort,
    /// Where the page starts.
    pub index: u32,
    /// How long the page is. Clamped to [`MAX_PAGE_SIZE`] by [`CfQuery::url`].
    pub page_size: u32,
}

impl CfQuery {
    /// A first page of `class`, most popular first.
    pub fn new(class: CfClass) -> CfQuery {
        CfQuery {
            text: String::new(),
            class,
            game_version: String::new(),
            loader: CfLoader::Any,
            sort: CfSort::Popularity,
            index: 0,
            page_size: 20,
        }
    }

    /// The same question with free text.
    pub fn text(mut self, text: impl Into<String>) -> CfQuery {
        self.text = text.into();
        self
    }

    /// The same question narrowed to a game version.
    pub fn game_version(mut self, version: impl Into<String>) -> CfQuery {
        self.game_version = version.into();
        self
    }

    /// The same question narrowed to a loader.
    pub fn loader(mut self, loader: CfLoader) -> CfQuery {
        self.loader = loader;
        self
    }

    /// The same question ordered differently.
    pub fn sorted(mut self, sort: CfSort) -> CfQuery {
        self.sort = sort;
        self
    }

    /// The same page length.
    pub fn page_size(mut self, page_size: u32) -> CfQuery {
        self.page_size = page_size;
        self
    }

    /// The search URL this question is asked at.
    ///
    /// Every optional narrowing is *absent* rather than empty when it is not
    /// set: `gameVersion=` asks the service for a version literally named "",
    /// which is no version at all and answers nothing, while omitting the
    /// parameter asks for every version. That distinction is the one thing this
    /// builder has to get right.
    pub fn url(&self) -> String {
        let mut url = format!(
            "{CURSEFORGE_BASE_URL}/mods/search?gameId={MINECRAFT_GAME_ID}&classId={}&index={}&pageSize={}&sortField={}&sortOrder=desc",
            self.class.id(),
            self.index,
            self.page_size.clamp(1, MAX_PAGE_SIZE),
            self.sort.field(),
        );
        if !self.text.trim().is_empty() {
            url.push_str("&searchFilter=");
            url.push_str(&encode(self.text.trim()));
        }
        if !self.game_version.trim().is_empty() {
            url.push_str("&gameVersion=");
            url.push_str(&encode(self.game_version.trim()));
        }
        if let Some(loader) = self.loader.id() {
            url.push_str("&modLoaderType=");
            url.push_str(&loader.to_string());
        }
        url
    }
}

/// Percent-encode one query value (RFC 3986 unreserved set left alone).
///
/// Written out rather than pulled in: `crate::modrinth`'s encoder is private to
/// that module, and the only values that reach here are a search phrase, a game
/// version and two integers.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// One project, as a search or a lookup writes it.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfMod {
    /// Project id.
    #[serde(default)]
    pub id: u32,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// URL slug, which is what a reader recognises.
    #[serde(default)]
    pub slug: String,
    /// One-line description.
    #[serde(default)]
    pub summary: String,
    /// Total downloads.
    #[serde(default)]
    pub download_count: u64,
    /// `classId`.
    #[serde(default)]
    pub class_id: u32,
    /// The project's pictures.
    #[serde(default)]
    pub logo: Option<CfLogo>,
    /// Authors, in the order the API lists them.
    #[serde(default)]
    pub authors: Vec<CfAuthor>,
    /// The categories the project is filed under.
    #[serde(default)]
    pub categories: Vec<CfCategory>,
    /// Whether a third-party client may fetch this project's files.
    ///
    /// `Option` because the field is nullable and its absence is meaningful:
    /// see [`CfMod::distribution_allowed`].
    #[serde(default, rename = "allowModDistribution")]
    pub allow_mod_distribution: Option<bool>,
    /// The files the project has, when the answer carried them.
    #[serde(default)]
    pub latest_files: Vec<CfFile>,
}

impl CfMod {
    /// The id as the string the rest of the launcher keys projects by.
    pub fn project_ref(&self) -> String {
        self.id.to_string()
    }

    /// The name to draw, which is the slug when the project has no display name.
    pub fn title(&self) -> &str {
        if self.name.is_empty() {
            &self.slug
        } else {
            &self.name
        }
    }

    /// The first author's name, or an empty string when the API named none.
    pub fn author(&self) -> &str {
        self.authors
            .first()
            .map(|author| author.name.as_str())
            .unwrap_or_default()
    }

    /// The icon to draw, when the project has one.
    pub fn icon_url(&self) -> &str {
        self.logo
            .as_ref()
            .map(|logo| logo.thumbnail_url.as_str())
            .unwrap_or_default()
    }

    /// The project's own page on curseforge.com.
    ///
    /// The id form rather than `minecraft/<kind>/<slug>`: the id address is
    /// canonical, it redirects to whatever slug the author is using today, and it
    /// needs neither the class table nor a slug that may have been renamed. It is
    /// the address a reader who cannot install from here should be sent to, which
    /// is the same instruction [`CfMod::distribution_refusal`] gives.
    pub fn web_url(&self) -> String {
        format!("https://www.curseforge.com/projects/{}", self.id)
    }

    /// Whether this project's files may be fetched by a client that is not
    /// CurseForge's own.
    ///
    /// Only an explicit `false` refuses. `null` and an absent field both mean the
    /// author never restricted distribution, which is what the API's own
    /// documentation says and what its client does.
    pub fn distribution_allowed(&self) -> bool {
        self.allow_mod_distribution != Some(false)
    }

    /// The sentence a refusal shows, naming the project rather than the rule.
    pub fn distribution_refusal(&self) -> String {
        format!(
            "{} does not allow its files to be downloaded outside CurseForge. Open it on curseforge.com and install it there.",
            self.title()
        )
    }
}

/// A project's logo, in the two sizes the API publishes.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfLogo {
    /// Small square, what a card draws.
    #[serde(default)]
    pub thumbnail_url: String,
    /// Full size.
    #[serde(default)]
    pub url: String,
}

/// One author of a project.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfAuthor {
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Profile URL on curseforge.com.
    #[serde(default)]
    pub url: String,
}

/// One category a project is filed under.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfCategory {
    /// Category id.
    #[serde(default)]
    pub id: u32,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// URL slug.
    #[serde(default)]
    pub slug: String,
}

/// One file of a project.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFile {
    /// File id.
    #[serde(default)]
    pub id: u32,
    /// The project this file belongs to.
    #[serde(default)]
    pub mod_id: u32,
    /// The name the author gave the release.
    #[serde(default)]
    pub display_name: String,
    /// The name of the file on disk.
    #[serde(default)]
    pub file_name: String,
    /// 1 release, 2 beta, 3 alpha.
    #[serde(default)]
    pub release_type: u32,
    /// The hashes the API publishes for the file.
    #[serde(default)]
    pub hashes: Vec<CfHash>,
    /// When it was published, ISO-8601.
    #[serde(default)]
    pub file_date: String,
    /// Size in bytes.
    #[serde(default)]
    pub file_length: u64,
    /// Download URL, or `None` when the API withholds it.
    #[serde(default)]
    pub download_url: Option<String>,
    /// The game versions the file is for, as free-form strings.
    #[serde(default)]
    pub game_versions: Vec<String>,
    /// The project's other files this one depends on.
    #[serde(default)]
    pub dependencies: Vec<CfDependency>,
}

impl CfFile {
    /// The SHA-1 the API published, when it published one (`algo` 1).
    pub fn sha1(&self) -> Option<&str> {
        self.hashes
            .iter()
            .find(|hash| hash.algo == 1 && !hash.value.is_empty())
            .map(|hash| hash.value.as_str())
    }

    /// The URL to fetch the bytes from, when the answer carried one.
    ///
    /// `None` is the "ask for the download URL by id" case: the API omits
    /// `downloadUrl` on every file of a project whose author restricted
    /// distribution, and on some answers where it simply chose not to inline it.
    pub fn direct_url(&self) -> Option<&str> {
        self.download_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
    }

    /// Whether this is a release rather than a beta or an alpha.
    pub fn is_release(&self) -> bool {
        self.release_type == 1
    }

    /// The version a reader should see: the display name when there is one.
    pub fn version_label(&self) -> &str {
        if self.display_name.is_empty() {
            &self.file_name
        } else {
            &self.display_name
        }
    }
}

/// One hash of a file.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfHash {
    /// The digest, hex.
    #[serde(default)]
    pub value: String,
    /// 1 = sha1, 2 = md5.
    #[serde(default)]
    pub algo: u32,
}

/// One dependency of a file.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfDependency {
    /// The project depended on.
    #[serde(default)]
    pub mod_id: u32,
    /// 1 = embedded, 2 = optional, 3 = required, 4 = tool, 5 = incompatible,
    /// 6 = include.
    #[serde(default)]
    pub relation_type: u32,
}

impl CfDependency {
    /// Whether the dependency must be installed for the file to work.
    ///
    /// `3` is required and `1` embedded: both mean the pack cannot run without
    /// it. `2` optional and `4` tool are the two a reader can decline.
    pub fn is_required(&self) -> bool {
        matches!(self.relation_type, 1 | 3)
    }
}

/// A page of results, with the count the service made of the whole question.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfPage<T> {
    /// The rows.
    #[serde(default)]
    pub data: Vec<T>,
    /// Where this page starts and what the whole answer holds.
    #[serde(default)]
    pub pagination: CfPagination,
}

impl<T> CfPage<T> {
    /// How many rows the whole question matched, not just this page.
    pub fn total(&self) -> u64 {
        self.pagination.total_count
    }

    /// Whether another page exists after this one.
    pub fn has_more(&self) -> bool {
        (self.pagination.index as u64 + self.data.len() as u64)
            < self.pagination.total_count.max(self.data.len() as u64)
    }
}

/// Where a page sits in a longer answer.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfPagination {
    /// First row of this page.
    #[serde(default)]
    pub index: u32,
    /// Rows in this page.
    #[serde(default)]
    pub page_size: u32,
    /// Rows the service counted.
    #[serde(default)]
    pub result_count: u32,
    /// Rows the whole question matched.
    #[serde(default)]
    pub total_count: u64,
}

/// Parse a search body.
pub fn parse_search(body: &[u8]) -> Result<CfPage<CfMod>, Error> {
    serde_json::from_slice(body)
        .map_err(|error| Error::json(Path::new("curseforge:mods/search"), error.to_string()))
}

/// Parse a single-project body.
///
/// The API wraps one document in `data` exactly as it wraps a list, so a caller
/// that read the raw object would be reading a shape the service does not send.
pub fn parse_mod(body: &[u8]) -> Result<CfMod, Error> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        #[serde(default)]
        data: Option<CfMod>,
    }
    let envelope: Envelope = serde_json::from_slice(body)
        .map_err(|error| Error::json(Path::new("curseforge:mod"), error.to_string()))?;
    envelope
        .data
        .ok_or_else(|| Error::format(Path::new("curseforge:mod"), "no data in the answer"))
}

/// Parse a files body.
pub fn parse_files(body: &[u8]) -> Result<CfPage<CfFile>, Error> {
    serde_json::from_slice(body)
        .map_err(|error| Error::json(Path::new("curseforge:mods/files"), error.to_string()))
}

/// The URL of one project.
pub fn mod_url(id: u32) -> String {
    format!("{CURSEFORGE_BASE_URL}/mods/{id}")
}

/// The URL of one project's file list.
///
/// The two narrowings are the ones the API documents on this route; both are
/// omitted when unset, for [`CfQuery::url`]'s reason.
pub fn files_url(id: u32, game_version: &str, loader: CfLoader) -> String {
    let mut url = format!("{CURSEFORGE_BASE_URL}/mods/{id}/files");
    let mut first = true;
    if !game_version.trim().is_empty() {
        url.push_str("?gameVersion=");
        url.push_str(&encode(game_version.trim()));
        first = false;
    }
    if let Some(loader) = loader.id() {
        url.push_str(if first { "?" } else { "&" });
        url.push_str("modLoaderType=");
        url.push_str(&loader.to_string());
    }
    url
}

/// The URL of one file.
pub fn file_url(mod_id: u32, file_id: u32) -> String {
    format!("{CURSEFORGE_BASE_URL}/mods/{mod_id}/files/{file_id}")
}

/// The URL that answers a file's actual download URL.
///
/// Asked when [`CfFile::direct_url`] is `None` and the project allows
/// distribution at all. The answer is `{"data": "https://…"}`.
pub fn download_url_url(mod_id: u32, file_id: u32) -> String {
    format!("{CURSEFORGE_BASE_URL}/mods/{mod_id}/files/{file_id}/download-url")
}

/// Parse the body [`download_url_url`] answers with.
pub fn parse_download_url(body: &[u8]) -> Result<String, Error> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        #[serde(default)]
        data: Option<String>,
    }
    let envelope: Envelope = serde_json::from_slice(body)
        .map_err(|error| Error::json(Path::new("curseforge:download-url"), error.to_string()))?;
    match envelope.data {
        Some(url) if !url.trim().is_empty() => Ok(url),
        _ => Err(Error::format(
            Path::new("curseforge:download-url"),
            "CurseForge did not return a download URL for this file",
        )),
    }
}

/// The request a CurseForge call is made as: the URL, its key, and JSON.
///
/// Built as a value rather than sent here so the same function serves the real
/// pool and a `MapFetch` in a test, and so a test can assert the key travelled.
pub fn request(url: impl Into<String>, key: &str) -> Request {
    Request::get(url)
        .header(API_KEY_HEADER, key)
        .header("Accept", "application/json")
}

/// Read one body through `fetch`, with the key the launcher holds.
///
/// **Uncached on purpose.** Overwolf's terms forbid storing what the API
/// answers, so this goes to `Fetch` directly and never through
/// [`crate::engine::cache::MetadataCache`]: a page that asks twice asks twice.
pub fn fetch_body(fetch: &dyn Fetch, url: &str, key: &str) -> Result<Vec<u8>, Error> {
    if key.trim().is_empty() {
        return Err(Error::format(
            Path::new(url),
            "no CurseForge API key is configured, so the catalogue cannot be read",
        ));
    }
    fetch.get(&request(url, key), &Cancel::new())
}

/// Run one search, uncached.
pub fn fetch_search(fetch: &dyn Fetch, key: &str, query: &CfQuery) -> Result<CfPage<CfMod>, Error> {
    let url = query.url();
    let body = fetch_body(fetch, &url, key)?;
    parse_search(&body)
}

/// Read one project, uncached.
pub fn fetch_mod(fetch: &dyn Fetch, key: &str, id: u32) -> Result<CfMod, Error> {
    let url = mod_url(id);
    let body = fetch_body(fetch, &url, key)?;
    parse_mod(&body)
}

/// Read one project's files, uncached.
pub fn fetch_files(
    fetch: &dyn Fetch,
    key: &str,
    id: u32,
    game_version: &str,
    loader: CfLoader,
) -> Result<Vec<CfFile>, Error> {
    let url = files_url(id, game_version, loader);
    let body = fetch_body(fetch, &url, key)?;
    Ok(parse_files(&body)?.data)
}

/// Resolve a file's download URL: the one its document carries, or the one the
/// API answers for it by id.
///
/// `Err` when the URL is withheld and cannot be asked for, which is the case a
/// page reports as "install it from curseforge.com" rather than as a failure.
pub fn resolve_download(
    fetch: &dyn Fetch,
    key: &str,
    file: &CfFile,
    distribution_allowed: bool,
) -> Result<String, Error> {
    if let Some(url) = file.direct_url() {
        return Ok(url.to_string());
    }
    if !distribution_allowed {
        return Err(Error::format(
            Path::new("curseforge:download-url"),
            "the project does not allow downloads outside CurseForge",
        ));
    }
    let url = download_url_url(file.mod_id, file.id);
    let body = fetch_body(fetch, &url, key)?;
    parse_download_url(&body)
}

/// A CurseForge modpack's `manifest.json`, as the archive carries it.
///
/// The format is the pack's own and not the API's: a `.zip` from curseforge.com
/// holds this file plus an `overrides/` tree, and installing it means resolving
/// every `files[]` row into a download. Parsing lives here rather than in
/// `sift-loader` because it is the half of the format this crate already has
/// the vocabulary for (project ids, file ids), and the loader keeps the half
/// that touches an instance.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfManifest {
    /// `minecraftModpack` in every real pack; kept so a wrong file is refused
    /// by name rather than installed as an empty instance.
    #[serde(default, rename = "manifestType")]
    pub manifest_type: String,
    /// `1` in every real pack.
    #[serde(default, rename = "manifestVersion")]
    pub manifest_version: u32,
    /// The pack's name.
    #[serde(default)]
    pub name: String,
    /// The pack's version string.
    #[serde(default)]
    pub version: String,
    /// The pack author's name.
    #[serde(default)]
    pub author: String,
    /// The game and loader the pack is built for.
    #[serde(default)]
    pub minecraft: CfManifestMinecraft,
    /// Every file to install.
    #[serde(default)]
    pub files: Vec<CfManifestFile>,
    /// The folder inside the archive that holds the pack's own files.
    #[serde(default = "default_overrides")]
    pub overrides: String,
}

fn default_overrides() -> String {
    String::from("overrides")
}

impl CfManifest {
    /// Parse a `manifest.json`.
    ///
    /// Refuses anything that is not a CurseForge modpack manifest, naming what it
    /// found: a `.zip` from another launcher opens with a different format, and
    /// "this is not a CurseForge pack" is a sentence a reader can act on while
    /// "invalid JSON" is not.
    pub fn parse(bytes: &[u8]) -> Result<CfManifest, Error> {
        let manifest: CfManifest = serde_json::from_slice(bytes).map_err(|error| {
            Error::json(Path::new("curseforge:manifest.json"), error.to_string())
        })?;
        if !manifest.manifest_type.is_empty() && manifest.manifest_type != "minecraftModpack" {
            return Err(Error::format(
                Path::new("curseforge:manifest.json"),
                format!(
                    "not a CurseForge modpack: manifestType is {:?}",
                    manifest.manifest_type
                ),
            ));
        }
        if manifest.files.is_empty() {
            return Err(Error::format(
                Path::new("curseforge:manifest.json"),
                "the pack lists no files to install",
            ));
        }
        if manifest.minecraft.version.trim().is_empty() {
            return Err(Error::format(
                Path::new("curseforge:manifest.json"),
                "the pack names no Minecraft version",
            ));
        }
        Ok(manifest)
    }

    /// The loader id the manifest names, as Minecraft's version JSON spells it
    /// (`forge-47.2.0` → `forge-47.2.0`), with its `primary` flag honoured.
    ///
    /// `None` means the pack is vanilla: a manifest with no `modLoaders` is a
    /// valid pack, just one with nothing to install beyond the game.
    pub fn loader_id(&self) -> Option<&str> {
        self.minecraft
            .mod_loaders
            .iter()
            .find(|loader| loader.primary)
            .or_else(|| self.minecraft.mod_loaders.first())
            .map(|loader| loader.id.as_str())
            .filter(|id| !id.trim().is_empty())
    }
}

/// The `minecraft` block of a pack manifest.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfManifestMinecraft {
    /// The game version the pack is built for.
    #[serde(default)]
    pub version: String,
    /// The loaders it needs.
    #[serde(default, rename = "modLoaders")]
    pub mod_loaders: Vec<CfManifestLoader>,
}

/// One loader a pack needs.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfManifestLoader {
    /// `forge-47.2.0`, `fabric-0.15.7`, `neoforge-21.1.66`.
    #[serde(default)]
    pub id: String,
    /// Whether this is the loader the pack is built on.
    #[serde(default)]
    pub primary: bool,
}

/// One file a pack names.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfManifestFile {
    /// The project the file belongs to.
    #[serde(default, rename = "projectID")]
    pub project_id: u32,
    /// The file itself.
    #[serde(default, rename = "fileID")]
    pub file_id: u32,
    /// Whether the pack cannot run without it.
    #[serde(default)]
    pub required: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::request::{MapFetch, Route};

    #[test]
    fn a_search_url_omits_what_was_not_asked_for() {
        let bare = CfQuery::new(CfClass::Modpack).url();
        assert_eq!(
            bare,
            "https://api.curseforge.com/v1/mods/search?gameId=432&classId=4471&index=0&pageSize=20&sortField=2&sortOrder=desc"
        );
        assert!(
            !bare.contains("gameVersion="),
            "an empty version is not asked"
        );
        assert!(!bare.contains("modLoaderType="), "any loader is not sent");

        let narrow = CfQuery::new(CfClass::Mod)
            .text("jei & co")
            .game_version("1.21.1")
            .loader(CfLoader::NeoForge)
            .sorted(CfSort::Downloads)
            .page_size(50)
            .url();
        assert!(narrow.contains("classId=6"), "{narrow}");
        assert!(narrow.contains("searchFilter=jei%20%26%20co"), "{narrow}");
        assert!(narrow.contains("gameVersion=1.21.1"), "{narrow}");
        assert!(narrow.contains("modLoaderType=6"), "{narrow}");
        assert!(narrow.contains("sortField=6"), "{narrow}");
        assert!(narrow.contains("pageSize=50"), "{narrow}");
    }

    #[test]
    fn a_page_size_is_clamped_to_what_the_service_accepts() {
        let url = CfQuery::new(CfClass::Mod).page_size(500).url();
        assert!(url.contains("pageSize=50"), "{url}");
        let url = CfQuery::new(CfClass::Mod).page_size(0).url();
        assert!(url.contains("pageSize=1"), "{url}");
    }

    #[test]
    fn class_ids_are_the_sites_own() {
        assert_eq!(CfClass::Mod.id(), 6);
        assert_eq!(CfClass::ResourcePack.id(), 12);
        assert_eq!(CfClass::World.id(), 17);
        assert_eq!(CfClass::Modpack.id(), 4471);
        assert_eq!(CfClass::Shader.id(), 6552);
        // The default is what a Discover page opens on.
        assert_eq!(CfClass::default(), CfClass::Mod);
        assert_eq!(CfClass::all()[0], CfClass::Modpack);
    }

    #[test]
    fn a_search_body_parses_into_rows_and_a_count() {
        let body = br#"{"data":[{
            "id":238222,"name":"Just Enough Items","slug":"jei","summary":"View items and recipes",
            "downloadCount":1234567,"classId":6,
            "logo":{"thumbnailUrl":"https://media.forgecdn.net/thumb.png","url":"https://media.forgecdn.net/full.png"},
            "authors":[{"name":"mezz","url":"https://curseforge.com/members/mezz"}],
            "categories":[{"id":423,"name":"Map and Information","slug":"map-information"}],
            "allowModDistribution":true
        }],"pagination":{"index":0,"pageSize":20,"resultCount":1,"totalCount":418}}"#;
        let page = parse_search(body).expect("a page");
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.total(), 418);
        let item = &page.data[0];
        assert_eq!(item.project_ref(), "238222");
        assert_eq!(item.title(), "Just Enough Items");
        assert_eq!(item.author(), "mezz");
        assert_eq!(item.icon_url(), "https://media.forgecdn.net/thumb.png");
        assert!(item.distribution_allowed());
        // One row shown of 418 matched: another page exists.
        assert!(page.has_more());
        let last = CfPage {
            data: vec![CfMod::default()],
            pagination: CfPagination {
                index: 417,
                page_size: 20,
                result_count: 1,
                total_count: 418,
            },
        };
        assert!(!last.has_more(), "the last row of 418 is the end");
        assert!(parse_search(b"not json").is_err());
    }

    #[test]
    fn an_absent_distribution_flag_allows_and_an_explicit_false_refuses() {
        let absent: CfMod = serde_json::from_str(r#"{"id":1,"name":"A"}"#).expect("a mod");
        assert!(absent.distribution_allowed());
        let null: CfMod =
            serde_json::from_str(r#"{"id":1,"name":"A","allowModDistribution":null}"#)
                .expect("a mod");
        assert!(
            null.distribution_allowed(),
            "null is the API's default, not a refusal"
        );
        let refused: CfMod =
            serde_json::from_str(r#"{"id":1,"name":"A","allowModDistribution":false}"#)
                .expect("a mod");
        assert!(!refused.distribution_allowed());
        // The refusal names the project, not the rule.
        assert!(refused.distribution_refusal().contains('A'));
    }

    #[test]
    fn a_file_picks_its_sha1_and_says_whether_it_can_be_fetched() {
        let file: CfFile = serde_json::from_str(
            r#"{"id":4825,"modId":238222,"displayName":"jei-1.21.1-19.21.0.247.jar",
                "fileName":"jei.jar","releaseType":1,"fileLength":1234,
                "hashes":[{"value":"deadbeef","algo":2},{"value":"abc123","algo":1}],
                "downloadUrl":"https://edge.forgecdn.net/files/4825/jei.jar",
                "gameVersions":["1.21.1","NeoForge"],
                "dependencies":[{"modId":1,"relationType":3},{"modId":2,"relationType":2}]}"#,
        )
        .expect("a file");
        assert_eq!(file.sha1(), Some("abc123"));
        assert_eq!(
            file.direct_url(),
            Some("https://edge.forgecdn.net/files/4825/jei.jar")
        );
        assert!(file.is_release());
        assert_eq!(file.version_label(), "jei-1.21.1-19.21.0.247.jar");
        assert!(file.dependencies[0].is_required());
        assert!(!file.dependencies[1].is_required());

        // A withheld URL and an empty one are the same fact.
        let withheld: CfFile =
            serde_json::from_str(r#"{"id":1,"modId":2,"downloadUrl":null,"hashes":[]}"#)
                .expect("a file");
        assert_eq!(withheld.direct_url(), None);
        assert_eq!(withheld.sha1(), None);
        let blank: CfFile =
            serde_json::from_str(r#"{"id":1,"modId":2,"downloadUrl":""}"#).expect("a file");
        assert_eq!(blank.direct_url(), None);
    }

    #[test]
    fn the_file_list_url_carries_its_narrowings_correctly() {
        assert_eq!(
            files_url(238222, "", CfLoader::Any),
            "https://api.curseforge.com/v1/mods/238222/files"
        );
        assert_eq!(
            files_url(238222, "1.21.1", CfLoader::Any),
            "https://api.curseforge.com/v1/mods/238222/files?gameVersion=1.21.1"
        );
        assert_eq!(
            files_url(238222, "1.21.1", CfLoader::NeoForge),
            "https://api.curseforge.com/v1/mods/238222/files?gameVersion=1.21.1&modLoaderType=6"
        );
        assert_eq!(
            files_url(238222, "", CfLoader::Fabric),
            "https://api.curseforge.com/v1/mods/238222/files?modLoaderType=4"
        );
        assert_eq!(
            file_url(238222, 4825),
            "https://api.curseforge.com/v1/mods/238222/files/4825"
        );
        assert_eq!(
            download_url_url(238222, 4825),
            "https://api.curseforge.com/v1/mods/238222/files/4825/download-url"
        );
    }

    #[test]
    fn a_request_carries_the_key_and_never_a_body() {
        let built = request("https://api.curseforge.com/v1/mods/1", "secret");
        assert_eq!(built.header_value(API_KEY_HEADER), Some("secret"));
        assert_eq!(built.header_value("Accept"), Some("application/json"));
        assert_eq!(built.offset, None, "a metadata read never resumes");
    }

    #[test]
    fn a_search_goes_out_uncached_and_with_the_key() {
        let query = CfQuery::new(CfClass::Modpack).text("atm");
        let fetch = MapFetch::new().with_route(
            query.url(),
            Route::text(r#"{"data":[],"pagination":{"totalCount":0}}"#),
        );
        let page = fetch_search(&fetch, "k", &query).expect("an answer");
        assert!(page.data.is_empty());
        let requests = fetch.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].header_value(API_KEY_HEADER), Some("k"));
        // Asked again is asked again: nothing here believes the first answer
        // still stands, because the terms do not allow keeping it.
        let _ = fetch_search(&fetch, "k", &query).expect("an answer");
        assert_eq!(fetch.count(), 2);
    }

    #[test]
    fn no_key_is_refused_by_name_instead_of_asked_with_an_empty_header() {
        let query = CfQuery::new(CfClass::Mod);
        let fetch = MapFetch::new().with_route(query.url(), Route::text(r#"{"data":[]}"#));
        let error = fetch_search(&fetch, "  ", &query).expect_err("no key");
        assert!(error.to_string().contains("API key"), "{error}");
        assert_eq!(fetch.count(), 0, "nothing was sent");
    }

    #[test]
    fn a_download_url_is_resolved_from_the_document_or_asked_for_by_id() {
        let with_url: CfFile = serde_json::from_str(
            r#"{"id":7,"modId":9,"downloadUrl":"https://edge.forgecdn.net/x.jar"}"#,
        )
        .expect("a file");
        let fetch = MapFetch::new();
        assert_eq!(
            resolve_download(&fetch, "k", &with_url, true).expect("a url"),
            "https://edge.forgecdn.net/x.jar"
        );
        assert_eq!(fetch.count(), 0, "the document already said it");

        // Withheld: the API is asked, once, by file id.
        let withheld: CfFile = serde_json::from_str(r#"{"id":7,"modId":9}"#).expect("a file");
        let asked = MapFetch::new().with_route(
            download_url_url(9, 7),
            Route::text(r#"{"data":"https://edge.forgecdn.net/y.jar"}"#),
        );
        assert_eq!(
            resolve_download(&asked, "k", &withheld, true).expect("a url"),
            "https://edge.forgecdn.net/y.jar"
        );
        assert_eq!(asked.requests()[0].url, download_url_url(9, 7));

        // Withheld *and* forbidden: refused without a request at all.
        let refused = MapFetch::new();
        let error = resolve_download(&refused, "k", &withheld, false).expect_err("refused");
        assert!(error.to_string().contains("does not allow"), "{error}");
        assert_eq!(refused.count(), 0);
    }

    #[test]
    fn a_manifest_parses_and_names_its_loader() {
        let body = br#"{
            "minecraft":{"version":"1.21.1","modLoaders":[
                {"id":"neoforge-21.1.66","primary":false},
                {"id":"neoforge-21.1.72","primary":true}]},
            "manifestType":"minecraftModpack","manifestVersion":1,
            "name":"All the Mods 10","version":"3.1.0","author":"ATMTeam",
            "files":[
                {"projectID":238222,"fileID":4825,"required":true},
                {"projectID":306612,"fileID":5512,"required":false}],
            "overrides":"overrides"
        }"#;
        let manifest = CfManifest::parse(body).expect("a manifest");
        assert_eq!(manifest.name, "All the Mods 10");
        assert_eq!(manifest.minecraft.version, "1.21.1");
        // `primary` wins over list order.
        assert_eq!(manifest.loader_id(), Some("neoforge-21.1.72"));
        assert_eq!(manifest.files[0].project_id, 238222);
        assert_eq!(manifest.files[0].file_id, 4825);
        assert!(manifest.files[0].required);
        assert_eq!(manifest.overrides, "overrides");
    }

    #[test]
    fn a_manifest_is_refused_by_name_when_it_is_not_one() {
        // Wrong format.
        let wrong = br#"{"manifestType":"modrinth","minecraft":{"version":"1.21"},"files":[{"projectID":1,"fileID":2}]}"#;
        let error = CfManifest::parse(wrong).expect_err("wrong format");
        assert!(
            error.to_string().contains("not a CurseForge modpack"),
            "{error}"
        );
        // Right format, nothing to install.
        let empty =
            br#"{"manifestType":"minecraftModpack","minecraft":{"version":"1.21"},"files":[]}"#;
        assert!(CfManifest::parse(empty).is_err());
        // Right format, no game version.
        let vague =
            br#"{"manifestType":"minecraftModpack","minecraft":{},"files":[{"projectID":1,"fileID":2}]}"#;
        assert!(CfManifest::parse(vague).is_err());
        assert!(CfManifest::parse(b"\x00\x01").is_err());
        // A vanilla pack is legal: no loaders at all.
        let vanilla = br#"{"manifestType":"minecraftModpack","minecraft":{"version":"1.20.1"},"files":[{"projectID":1,"fileID":2}]}"#;
        assert_eq!(
            CfManifest::parse(vanilla).expect("vanilla").loader_id(),
            None
        );
    }

    #[test]
    fn an_empty_loader_list_is_not_a_loader_named_nothing() {
        let manifest = CfManifest::parse(
            br#"{"minecraft":{"version":"1.21","modLoaders":[{"id":"  "}]},"files":[{"projectID":1,"fileID":2}]}"#,
        )
        .expect("a manifest");
        assert_eq!(manifest.loader_id(), None);
    }
}
