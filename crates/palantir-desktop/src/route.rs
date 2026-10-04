//! Where the shell can be: the reference's navigation, as a table this shell
//! can walk.
//!
//! This mirrors `vendor/modrinth-app/app-frontend/src/routes.js` route for
//! route and name for name, because everything else in the reference's chrome
//! is written against those names and their nesting: the rail's own highlight
//! rules (`App.vue`'s `is-primary` / `is-subpage`), the breadcrumbs, the
//! instance layout that pages sit inside, and `router.push({ name: ... })` in
//! dozens of call sites. A port that keeps the geometry but flattens the
//! nesting is the mistake this module exists to undo: `REFERENCE.md`'s "Still
//! to port" records that the old rail had Mods, Worlds, Logs, Settings,
//! Accounts and About as *top-level pages*, where the reference keeps the first
//! four inside an instance (`/instance/:id/...`) and Settings as a modal.
//!
//! Three things are therefore absent on purpose, and each is a fact about the
//! reference rather than an omission here:
//!
//! * There is no `Route::Settings`. `App.vue`'s rail hands Settings an
//!   `IconButton` whose `to` is a function that opens
//!   `AppSettingsModal` -- it is not addressable, so it cannot be a route.
//! * There is no `Route::Accounts`. Modrinth accounts live in the rail's
//!   profile slot as an overflow menu, and Minecraft accounts live in
//!   `AppSettingsModal`.
//! * There is no `Route::Mods` or `Route::Logs`: both are instance tabs
//!   (`InstanceTab::Content`, `InstanceTab::Logs`).
//!
//! What is here is a *fragment* of the reference's router: the parts a
//! desktop shell navigates in-process. `vue-router`'s history, suspense and
//! scroll behaviour are the browser's, and the shell's own equivalents are the
//! shell's business.

use std::fmt;

/// One kind of thing Modrinth hosts.
///
/// The route tokens are the reference's own: `routes.js` narrows the legacy
/// `/:projectType/:id` redirect with `(mod|plugin|datapack|resourcepack|
/// shader|modpack)`, and `Browse.vue`'s tab list adds `server`, which is
/// browsable (`/browse/server`) but is not a project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProjectType {
    /// `/browse/modpack`
    Modpack,
    /// `/browse/mod`
    Mod,
    /// `/browse/plugin` -- in `routes.js`'s redirect and in the messages, but
    /// not one of Discover's tabs.
    Plugin,
    /// `/browse/resourcepack`
    ResourcePack,
    /// `/browse/datapack`
    Datapack,
    /// `/browse/shader`
    Shader,
    /// `/browse/server`
    Server,
}

impl ProjectType {
    /// Discover's tabs, in the order `Browse.vue` builds them.
    ///
    /// The order is the reference's, not alphabetical, and it is what the tab
    /// strip draws: Modpacks first because they are what a launcher is for.
    /// `Browse.vue` marks several `shown:` and hides them in context (Mods and
    /// Datapacks only inside an instance, Servers only outside one); those
    /// conditions belong to the page that draws them.
    pub const TABS: &'static [ProjectType] = &[
        ProjectType::Modpack,
        ProjectType::Mod,
        ProjectType::ResourcePack,
        ProjectType::Datapack,
        ProjectType::Shader,
        ProjectType::Server,
    ];

    /// Every kind, tabs first, in the order `ProjectType::TABS` declares.
    pub const ALL: &'static [ProjectType] = &[
        ProjectType::Modpack,
        ProjectType::Mod,
        ProjectType::Plugin,
        ProjectType::ResourcePack,
        ProjectType::Datapack,
        ProjectType::Shader,
        ProjectType::Server,
    ];

    /// The order a *profile's* filter strip sorts the types a user has into.
    ///
    /// A third order rather than a reuse of the two above, because the reference
    /// has three and they disagree: `PROJECT_TYPE_ORDER` in
    /// `ui/src/utils/project-types.ts` -- which is what a profile's `NavTabs` sorts
    /// by (`sortProjectTypes`) -- puts mods first and modpacks fifth, where
    /// Discover's `TABS` puts modpacks first. Its `collection` entry is not here: a
    /// collection is not a project type in this tree, and the page that would offer
    /// it cannot read one (see `pages::user`).
    pub const PROFILE_ORDER: &'static [ProjectType] = &[
        ProjectType::Mod,
        ProjectType::ResourcePack,
        ProjectType::Datapack,
        ProjectType::Shader,
        ProjectType::Modpack,
        ProjectType::Plugin,
        ProjectType::Server,
    ];

    /// The token this kind is spelled with in a path.
    pub fn token(self) -> &'static str {
        match self {
            ProjectType::Modpack => "modpack",
            ProjectType::Mod => "mod",
            ProjectType::Plugin => "plugin",
            ProjectType::ResourcePack => "resourcepack",
            ProjectType::Datapack => "datapack",
            ProjectType::Shader => "shader",
            ProjectType::Server => "server",
        }
    }

    /// The kind's name as a heading: the category message from
    /// `ui/src/utils/common-messages.ts`.
    pub fn label(self) -> &'static str {
        match self {
            ProjectType::Modpack => "Modpacks",
            ProjectType::Mod => "Mods",
            ProjectType::Plugin => "Plugins",
            ProjectType::ResourcePack => "Resource Packs",
            ProjectType::Datapack => "Data Packs",
            ProjectType::Shader => "Shaders",
            ProjectType::Server => "Servers",
        }
    }

    /// The kind's name mid-sentence, singular and plural: the sentence message
    /// from the same file. `Browse.vue`'s breadcrumb reads `Discover
    /// {projectType}`, and the placeholder is this.
    pub fn sentence(self, count: usize) -> &'static str {
        match (self, count) {
            (ProjectType::Modpack, 1) => "modpack",
            (ProjectType::Modpack, _) => "modpacks",
            (ProjectType::Mod, 1) => "mod",
            (ProjectType::Mod, _) => "mods",
            (ProjectType::Plugin, 1) => "plugin",
            (ProjectType::Plugin, _) => "plugins",
            (ProjectType::ResourcePack, 1) => "resource pack",
            (ProjectType::ResourcePack, _) => "resource packs",
            (ProjectType::Datapack, 1) => "data pack",
            (ProjectType::Datapack, _) => "data packs",
            (ProjectType::Shader, 1) => "shader",
            (ProjectType::Shader, _) => "shaders",
            (ProjectType::Server, 1) => "server",
            (ProjectType::Server, _) => "servers",
        }
    }

    /// The folder inside an instance this kind installs into.
    ///
    /// `None` for the two kinds that are not a folder inside somebody's
    /// instance: a pack *is* an instance (that is why the option exists at all),
    /// and a server is a process this launcher does not host. A caller that gets
    /// `None` has to say so rather than guess a folder.
    ///
    /// The four Minecraft ones are the names the old shell's table carried
    /// (`browse::ContentType::target_folder`, measured then and unchanged);
    /// their live home is here now, next to the tokens and labels of the same
    /// vocabulary. `plugins` is the fifth, and it is not read by the client: it
    /// is where a server's own loader looks, which is what the reference's
    /// content kinds name (`server-panel-sync.ts`).
    pub fn target_folder(self) -> Option<&'static str> {
        match self {
            ProjectType::Modpack | ProjectType::Server => None,
            ProjectType::Mod => Some("mods"),
            ProjectType::Plugin => Some("plugins"),
            ProjectType::ResourcePack => Some("resourcepacks"),
            ProjectType::Datapack => Some("datapacks"),
            ProjectType::Shader => Some("shaderpacks"),
        }
    }

    /// The kind a path segment names, aliases included.
    ///
    /// The two aliases are the reference's own: `PROJECT_TYPE_ALIASES` in
    /// `common-messages.ts` rewrites `shaderpack` and `minecraft_java_server`
    /// before anything reads them, because the API returns the long spellings
    /// and every label is keyed on the short ones.
    pub fn from_token(token: &str) -> Option<ProjectType> {
        match token {
            "shaderpack" => return Some(ProjectType::Shader),
            "minecraft_java_server" => return Some(ProjectType::Server),
            _ => {}
        }
        ProjectType::ALL.iter().copied().find(|kind| kind.token() == token)
    }

    /// The token a *profile's* filter strip spells this kind with: `mods`,
    /// `resourcepacks`, and so on.
    ///
    /// Plural, because that is how the reference writes its own profile links:
    /// `UserProfilePageLayout.vue`'s `navLinks` build each `href` as
    /// `` `${profilePath}/${projectType}s` ``. A link this launcher writes has to be
    /// the one the reference writes -- a reader who copies a profile address out of
    /// the reference and pastes it here should land on the same list -- so the
    /// spelling lives here beside the singular [`Self::token`] rather than at the
    /// call site, and [`Self::from_profile_token`] is its other half.
    pub fn profile_token(self) -> String {
        format!("{}s", self.token())
    }

    /// The kind a profile address's third segment names: either spelling.
    ///
    /// Both, because both exist in the reference's own tree: its profile strip
    /// links to `/user/:user/mods` and its `routes.js` reads a project type out of
    /// that segment with the singular parser. Accepting only one of the two would
    /// make either this launcher's own links or the reference's addresses fail to
    /// navigate, and the failure would be a link that does nothing.
    ///
    /// `collections` is refused rather than guessed at: it is a fourth link in the
    /// reference's strip, and collections are not a project type here (see
    /// [`Self::PROFILE_ORDER`]). Refusing the *token* is not refusing the *address*:
    /// `/user/jelly/collections` resolves, through [`ProfileTab::from_profile_token`],
    /// which is this function's other half for that one link.
    pub fn from_profile_token(token: &str) -> Option<ProjectType> {
        Self::from_token(token)
            .or_else(|| token.strip_suffix('s').and_then(Self::from_token))
    }
}

impl fmt::Display for ProjectType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.token())
    }
}

/// Which tab of a project page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectTab {
    /// `''`
    Description,
    /// `versions`
    Versions,
    /// `version/:version`
    Version(String),
    /// `gallery`
    Gallery,
}

/// Which tab of an instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstanceTab {
    /// `''`
    Content,
    /// `projects/:type` -- the same page, with the content list filtered.
    /// Keeping the filter in the address is what makes the filter shareable
    /// and what makes back button out of it do the obvious thing.
    ContentFilter(ProjectType),
    /// `files`
    Files,
    /// `worlds`
    Worlds,
    /// `screenshots`
    Screenshots,
    /// `logs`
    Logs,
    /// `share`
    Share,
}

/// What a profile address's third segment names.
///
/// The fourth of the shape [`ServerTab`], [`ProjectTab`] and [`InstanceTab`] take
/// -- an enum beside [`Route`] rather than another `ProjectType` variant, and for
/// the reason `ProjectType::from_profile_token` gives: a collection is not a kind
/// of project, so a variant would have to be answered by every place that derives
/// something from a type. `target_folder` would have to say where a collection
/// installs, `sentence` would have to put "collections" in a sentence about a
/// downloaded thing, `TABS` and `ALL` would have to decide whether Discover
/// offers one -- and the reference offers none of those, because it has no such
/// concept either. The reference keeps `'collection'` as a *string* that only the
/// profile's own filter reads: `layout.vue:762` pushes it into `projectTypes` after
/// `catalogProjectTypes` has run, and `parseProjectTypeRouteParam`
/// (`ui/src/utils/v3-projects.ts:75`) turns it back into the selection.
///
/// No `All` variant, and that is the reference's shape rather than an omission:
/// `routes.js:66` spells the segment `:projectType?`, and *All* is what an address
/// with no segment is (`layout.vue:771-773` builds its `href` as `profilePath`
/// alone). So [`Route::User`] carries an `Option` and `None` is *All* -- the
/// optional parameter, not a tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileTab {
    /// `mods`, `datapacks` and the rest: one project type.
    Projects(ProjectType),
    /// `collections`, which is the strip's fourth link and no kind of project.
    Collections,
}

impl ProfileTab {
    /// The third segment this tab's address spells, as the reference's own links do.
    ///
    /// `layout.vue:779` builds every one of them the same way --
    /// `` `${profilePath}/${projectType}s` `` -- from the *singular* string, which is
    /// why a type's is [`ProjectType::profile_token`] and why `'collection'` becomes
    /// `collections`. There is no singular to append here, and the one segment that
    /// resolves is the plural.
    pub fn profile_token(self) -> String {
        match self {
            ProfileTab::Projects(kind) => kind.profile_token(),
            ProfileTab::Collections => "collections".to_string(),
        }
    }

    /// What a profile address's third segment names, or `None` for one that names
    /// nothing this tree has a page for.
    ///
    /// `parseProjectTypeRouteParam`, term for term: both spellings of the
    /// collections token are read (`v3-projects.ts:81-83` tests `'collection'` and
    /// `'collections'` and answers `'collection'` for either), and everything else
    /// goes through [`ProjectType::from_profile_token`] -- the singular parser with
    /// the trailing `s` stripped, which is what the reference's line above it does
    /// before `normalizeProjectType`.
    ///
    /// `collection` is deliberately *not* reachable through
    /// [`ProjectType::from_profile_token`] and this is its other half rather than a
    /// contradiction: a collection is not a project type, so no type is handed back
    /// for the token, and the address is still a real one.
    pub fn from_profile_token(token: &str) -> Option<ProfileTab> {
        if token == "collections" || token == "collection" {
            return Some(ProfileTab::Collections);
        }
        ProjectType::from_profile_token(token).map(ProfileTab::Projects)
    }
}

/// Which tab of a hosted server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerTab {
    /// `''`
    Overview,
    /// `content`
    Content,
    /// `files`
    Files,
    /// `backups`
    Backups,
    /// `access`
    Access,
}

/// One addressable page.
///
/// Flat, with the ids and the tab as payloads, rather than a tree of nested
/// enums: the reference's router is flat too (its names are unique across the
/// whole tree, which is what makes `{ name: 'InstanceWorlds' }` a complete
/// address), and the nesting it does express is exactly the tab payloads here.
/// `parent` restores the hierarchy where a caller needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// `/` -- the instance list and its play buttons.
    Home,
    /// `/hosting/manage/`
    Servers,
    /// `/hosting/manage/:id`
    Server { id: String, tab: ServerTab },
    /// `/browse/:projectType`
    Discover { project_type: ProjectType },
    /// `/skins`
    Skins,
    /// `/screenshots`
    Screenshots,
    /// `/user/:user/:projectType?`
    User {
        /// `:user` -- whose profile.
        user: String,
        /// `:projectType`, read the way the reference reads it -- as a
        /// [`ProfileTab`], or `None` for the *All* tab's own empty path.
        project_type: Option<ProfileTab>,
    },
    /// `/project/:id`
    Project { id: String, tab: ProjectTab },
    /// `/instance/:id`
    Instance { id: String, tab: InstanceTab },
}

impl Route {
    /// The router name, as `routes.js` spells it.
    ///
    /// Worth keeping exactly, spaces and case included: the reference's own
    /// code says `{ name: 'Discover content' }`, and a port that renames its
    /// routes makes every upstream fix harder to follow.
    #[cfg(test)]
    pub fn name(&self) -> &'static str {
        match self {
            Route::Home => "Home",
            Route::Servers => "Servers",
            Route::Server { tab, .. } => match tab {
                ServerTab::Overview => "ServerManageOverview",
                ServerTab::Content => "ServerManageContent",
                ServerTab::Files => "ServerManageFiles",
                ServerTab::Backups => "ServerManageBackups",
                ServerTab::Access => "ServerManageAccess",
            },
            Route::Discover { .. } => "Discover content",
            Route::Skins => "Skin selector",
            Route::Screenshots => "Screenshots",
            Route::User { .. } => "User",
            Route::Project { tab, .. } => match tab {
                ProjectTab::Description => "Description",
                ProjectTab::Versions => "Versions",
                ProjectTab::Version(_) => "Version",
                ProjectTab::Gallery => "Gallery",
            },
            Route::Instance { tab, .. } => match tab {
                InstanceTab::Content => "InstanceContent",
                InstanceTab::ContentFilter(_) => "InstanceContentFilter",
                InstanceTab::Files => "InstanceFiles",
                InstanceTab::Worlds => "InstanceWorlds",
                InstanceTab::Screenshots => "InstanceScreenshots",
                InstanceTab::Logs => "InstanceLogs",
                InstanceTab::Share => "InstanceShare",
            },
        }
    }

    /// The page this one is a tab of, where it is a tab.
    ///
    /// `ProjectTab::Description` and `InstanceTab::Content` are their own
    /// parent: the reference routes the bare path to the first tab, so going
    /// "up" from it is already done.
    #[cfg(test)]
    pub fn parent(&self) -> Option<Route> {
        match self {
            Route::Server { id, tab } if *tab != ServerTab::Overview => Some(Route::Server {
                id: id.clone(),
                tab: ServerTab::Overview,
            }),
            Route::Project { id, tab } if *tab != ProjectTab::Description => Some(Route::Project {
                id: id.clone(),
                tab: ProjectTab::Description,
            }),
            Route::Instance { id, tab } if *tab != InstanceTab::Content => Some(Route::Instance {
                id: id.clone(),
                tab: InstanceTab::Content,
            }),
            _ => None,
        }
    }

    /// Whether this page needs the right panel on screen whatever the user's
    /// `toggle_sidebar` setting says.
    ///
    /// `App.vue`'s `forceSidebar`: Discover, Project and User pages are
    /// search-and-install pages, and the panel is where the install goes.
    pub fn forces_sidebar(&self) -> bool {
        matches!(self, Route::Discover { .. } | Route::Project { .. } | Route::User { .. })
    }

    /// The route a path names, or `None` if the path is not one of them.
    ///
    /// One path is accepted that `routes.js` does not have a route for: the
    /// legacy `/:projectType/:id/:rest*`, which the reference registers as a
    /// redirect to `/project/:id/:rest`. Both the pattern and the destination
    /// are the reference's; the indirection was only ever there because the
    /// site had those URLs first.
    fn from_segments(segments: &[String]) -> Option<Route> {
        let first = segments.first().map(String::as_str)?;
        // The legacy form resolves to Project before anything else looks at
        // it. `plugin` is in the alternation upstream and therefore here, even
        // though Discover has no tab for it.
        if segments.len() >= 2 && ProjectType::from_token(first).is_some() {
            return Route::from_segments(
                &[vec!["project".to_string()], segments[1..].to_vec()].concat(),
            );
        }
        match segments {
            [one] if one == "skins" => Some(Route::Skins),
            [one] if one == "screenshots" => Some(Route::Screenshots),
            [hosting, manage] if hosting == "hosting" && manage == "manage" => {
                Some(Route::Servers)
            }
            [hosting, manage, id] if hosting == "hosting" && manage == "manage" => {
                Some(Route::Server { id: id.clone(), tab: ServerTab::Overview })
            }
            [hosting, manage, id, tab] if hosting == "hosting" && manage == "manage" => {
                // A three-segment tail is the *id* plus a tab: `/hosting/manage/
                // content` names a server called `content`, not the content tab.
                let tab = match tab.as_str() {
                    "content" => ServerTab::Content,
                    "files" => ServerTab::Files,
                    "backups" => ServerTab::Backups,
                    "access" => ServerTab::Access,
                    _ => return None,
                };
                Some(Route::Server { id: id.clone(), tab })
            }
            [browse, kind] if browse == "browse" => Some(Route::Discover {
                project_type: ProjectType::from_token(kind)?,
            }),
            [user, name] if user == "user" => {
                Some(Route::User { user: name.clone(), project_type: None })
            }
            [user, name, kind] if user == "user" => Some(Route::User {
                user: name.clone(),
                project_type: Some(ProfileTab::from_profile_token(kind)?),
            }),
            [project, id] if project == "project" => Some(Route::Project {
                id: id.clone(),
                tab: ProjectTab::Description,
            }),
            [project, id, tab] if project == "project" => {
                let tab = match tab.as_str() {
                    "versions" => ProjectTab::Versions,
                    "gallery" => ProjectTab::Gallery,
                    _ => return None,
                };
                Some(Route::Project { id: id.clone(), tab })
            }
            [project, id, version, value] if project == "project" && version == "version" => {
                Some(Route::Project { id: id.clone(), tab: ProjectTab::Version(value.clone()) })
            }
            [instance, id] if instance == "instance" => Some(Route::Instance {
                id: id.clone(),
                tab: InstanceTab::Content,
            }),
            [instance, id, tab] if instance == "instance" => {
                let tab = match tab.as_str() {
                    "files" => InstanceTab::Files,
                    "worlds" => InstanceTab::Worlds,
                    "screenshots" => InstanceTab::Screenshots,
                    "logs" => InstanceTab::Logs,
                    "share" => InstanceTab::Share,
                    _ => return None,
                };
                Some(Route::Instance { id: id.clone(), tab })
            }
            [instance, id, projects, kind] if instance == "instance" && projects == "projects" => {
                Some(Route::Instance {
                    id: id.clone(),
                    tab: InstanceTab::ContentFilter(ProjectType::from_token(kind)?),
                })
            }
            _ => None,
        }
    }
}

/// The query a page carries: which instance or server it is being browsed
/// *for*, and where the user came from.
///
/// This is not decoration. In the reference, opening Discover from inside an
/// instance sets `?i=<id>` and turns the page into an install-into-that-
/// instance flow -- it changes the breadcrumb, the install button's target,
/// which tabs are offered, and the rail's highlight. `Browse.vue` keeps the
/// five parameters below and carries them onto every tab link it builds, which
/// is the shape this mirrors.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// `?i=` -- the instance this page is being browsed for.
    pub instance: Option<String>,
    /// `?ai=` -- the instance a new instance is being added to.
    pub add_to_instance: Option<String>,
    /// `?from=` -- where the page was opened from. `worlds` is the one value
    /// the reference tests for by name.
    pub from: Option<String>,
    /// `?sid=` -- the Modrinth Hosting server this page is being browsed for.
    pub server: Option<String>,
    /// `?wid=` -- the world being played, for the world-to-server flow.
    pub world: Option<String>,
}

impl Context {
    /// Whether this page is inside an instance.
    ///
    /// `App.vue`'s `is-subpage` predicates test exactly this (`route.query.i`)
    /// and nothing else, which is why `add_to_instance` is not folded in.
    pub fn in_instance(&self) -> bool {
        self.instance.is_some()
    }

    /// Whether this page is inside a hosted server.
    pub fn in_server(&self) -> bool {
        self.server.is_some()
    }

    /// Read the five parameters out of a query string, with or without its
    /// leading `?`.
    fn parse(query: &str) -> Context {
        let mut context = Context::default();
        for pair in query.trim_start_matches('?').split('&') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            let value = decode(value);
            match key {
                "i" => context.instance = Some(value),
                "ai" => context.add_to_instance = Some(value),
                "from" => context.from = Some(value),
                "sid" => context.server = Some(value),
                "wid" => context.world = Some(value),
                _ => {}
            }
        }
        context
    }
}

/// A route plus the context it was opened in: one complete address.
///
/// The two are separate because the *page* is decided by the path alone while
/// the *mark on the rail* is decided by both, which is precisely how `App.vue`
/// splits them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    pub route: Route,
    pub context: Context,
}

impl Address {
    /// A top-level page with no context.
    pub fn at(route: Route) -> Address {
        Address { route, context: Context::default() }
    }

    /// Read an address out of a path, fragment and query included.
    ///
    /// A fragment is dropped rather than refused: the reference's own legacy
    /// redirect carries `to.hash` through to the project URL, and nothing the
    /// shell draws depends on it.
    pub fn parse(path: &str) -> Option<Address> {
        let path = path.trim();
        // Order matters: a fragment sits *after* the query, so the `#` has to
        // come off first or the fragment's tail would be read as part of the
        // last query value. It is dropped rather than refused because the
        // reference's legacy redirect carries `to.hash` through, and nothing
        // the shell draws depends on it.
        let path = match path.split_once('#') {
            Some((before, _fragment)) => before,
            None => path,
        };
        let (path, query) = match path.split_once('?') {
            Some((path, query)) => (path, query),
            None => (path, ""),
        };
        let segments: Vec<String> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(decode)
            .collect();
        let route = if segments.is_empty() { Route::Home } else { Route::from_segments(&segments)? };
        Some(Address { route, context: Context::parse(query) })
    }

    /// Which rail slot this address lights up, and how.
    ///
    /// The rules are lifted from `App.vue`'s `:is-primary` and `:is-subpage`
    /// predicates and are genuinely conditional: `/browse` is Discover, but
    /// `/browse/mod?i=sodium` is *inside* an instance and is marked on Home
    /// instead, which is how the reference shows where the user is within a
    /// nesting without a second rail. Two things are deliberately absent:
    /// `User` marks nothing (the profile slot is an overflow menu with a
    /// function `to`, so the reference's own link never acquires an active
    /// class), and Settings, Create new instance and the profile slot can
    /// never be marked at all because they are buttons rather than routes.
    pub fn marks(&self) -> Vec<(Rail, Mark)> {
        let inside = self.context.in_instance();
        let hosting = self.context.in_server();
        match &self.route {
            Route::Home => vec![(Rail::Home, Mark::Primary)],
            // Browse inside an instance is the instance's page as much as it
            // is Discover's: the reference marks Home as a subpage there.
            Route::Discover { .. } if inside => vec![(Rail::Home, Mark::Subpage)],
            Route::Discover { .. } if hosting => vec![(Rail::Servers, Mark::Subpage)],
            Route::Discover { .. } => vec![(Rail::Discover, Mark::Primary)],
            Route::Project { .. } if inside => vec![(Rail::Home, Mark::Subpage)],
            Route::Project { .. } if hosting => vec![(Rail::Servers, Mark::Subpage)],
            Route::Project { .. } => vec![(Rail::Discover, Mark::Subpage)],
            Route::Servers => vec![(Rail::Servers, Mark::Primary)],
            Route::Server { .. } => vec![(Rail::Servers, Mark::Subpage)],
            Route::Skins => vec![(Rail::Skins, Mark::Primary)],
            Route::Screenshots => vec![(Rail::Screenshots, Mark::Primary)],
            // An instance page marks nothing at all, which looks like an
            // oversight upstream and is reproduced anyway: `App.vue` gives
            // predicates to exactly three buttons (Home, Discover, Servers),
            // and none of them tests for `/instance`. The reference reaches
            // this page from the install flow rather than from the rail, and
            // the rail it leaves unmarked is what the shell draws.
            Route::Instance { .. } => Vec::new(),
            Route::User { .. } => Vec::new(),
        }
    }
}

/// The rail's slots, top to bottom as `App.vue` declares them.
///
/// The three at the end are buttons rather than links and the profile slot is
/// an overflow menu, but they are slots of the same rail and the shell draws
/// them the same size, so they are named here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rail {
    /// `PlayIcon` -- `/`
    Home,
    /// `CompassIcon` -- `/browse/modpack`
    Discover,
    /// `ShirtIcon` -- `/skins`, shown only when `show_skin_selector_in_sidebar`
    Skins,
    /// `ImageIcon` -- `/screenshots`, shown only when screenshots are synced
    Screenshots,
    /// `ServerStackIcon` -- `/hosting/manage`
    Servers,
    /// `PlusIcon` -- opens the creation flow; not addressable
    CreateInstance,
    /// `SettingsIcon` -- opens `AppSettingsModal`; not addressable
    Settings,
    /// `Avatar` / `LogInIcon` / `SpinnerIcon` -- the account overflow menu
    Profile,
}

impl Rail {
    /// Every slot, in the order `App.vue` draws them.
    pub const ALL: &'static [Rail] = &[
        Rail::Home,
        Rail::Discover,
        Rail::Skins,
        Rail::Screenshots,
        Rail::Servers,
        Rail::CreateInstance,
        Rail::Settings,
        Rail::Profile,
    ];
}

impl fmt::Display for Rail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Rail::Home => "home",
            Rail::Discover => "discover",
            Rail::Skins => "skins",
            Rail::Screenshots => "screenshots",
            Rail::Servers => "servers",
            Rail::CreateInstance => "create",
            Rail::Settings => "settings",
            Rail::Profile => "profile",
        };
        formatter.write_str(name)
    }
}

/// How a rail slot is marked.
///
/// `Primary` is `NavButton`'s `router-link-active`: the 48px circular plate at
/// full opacity with the selected text colour. `Subpage` is its
/// `subpage-active`: the same plate at `--color-button-bg`, with the icon in
/// `--color-contrast` and a drop shadow, which is the quieter mark the
/// reference uses to say "you are somewhere below this".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Primary,
    Subpage,
}

/// `encodeURIComponent`'s safe set: letters, digits and `-_.!~*'()`.
///
/// Taken from the reference's own behaviour rather than from RFC 3986, which
/// leaves `!*'()` out; `encodeURIComponent` is what every `router.push` in the
/// reference percent-encodes with.
#[cfg(test)]
fn is_component_safe(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte)
}

/// Percent-encode one path segment or query value.
#[cfg(test)]
fn encode(raw: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        if is_component_safe(byte) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

/// One hexadecimal digit, or `None`.
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Percent-decode one path segment or query value.
///
/// Lenient on purpose: a stray `%` is a literal `%`, not an error, which is
/// what `decodeURIComponent` does when the caller has already caught, and what
/// keeps a malformed link from taking the shell to a blank page.
fn decode(raw: &str) -> String {
    if !raw.contains('%') {
        return raw.to_string();
    }
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escape = if bytes[index] == b'%' && index + 2 < bytes.len() {
            match (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2])) {
                (Some(high), Some(low)) => Some((high << 4) | low),
                _ => None,
            }
        } else {
            None
        };
        match escape {
            Some(byte) => {
                out.push(byte);
                index += 3;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every route the shell can be on, one of each shape.
    fn sample() -> Vec<Route> {
        vec![
            Route::Home,
            Route::Servers,
            Route::Server { id: "srv".into(), tab: ServerTab::Overview },
            Route::Server { id: "srv".into(), tab: ServerTab::Content },
            Route::Server { id: "srv".into(), tab: ServerTab::Files },
            Route::Server { id: "srv".into(), tab: ServerTab::Backups },
            Route::Server { id: "srv".into(), tab: ServerTab::Access },
            Route::Discover { project_type: ProjectType::Modpack },
            Route::Discover { project_type: ProjectType::Mod },
            Route::Discover { project_type: ProjectType::ResourcePack },
            Route::Discover { project_type: ProjectType::Datapack },
            Route::Discover { project_type: ProjectType::Shader },
            Route::Discover { project_type: ProjectType::Server },
            Route::Skins,
            Route::Screenshots,
            Route::User { user: "jelly".into(), project_type: None },
            Route::User {
                user: "jelly".into(),
                project_type: Some(ProfileTab::Projects(ProjectType::Mod)),
            },
            Route::User {
                user: "jelly".into(),
                project_type: Some(ProfileTab::Collections),
            },
            Route::Project { id: "sodium".into(), tab: ProjectTab::Description },
            Route::Project { id: "sodium".into(), tab: ProjectTab::Versions },
            Route::Project { id: "sodium".into(), tab: ProjectTab::Version("mc1.21-0.5".into()) },
            Route::Project { id: "sodium".into(), tab: ProjectTab::Gallery },
            Route::Instance { id: "All the Mods 10".into(), tab: InstanceTab::Content },
            Route::Instance {
                id: "All the Mods 10".into(),
                tab: InstanceTab::ContentFilter(ProjectType::Mod),
            },
            Route::Instance { id: "a/b".into(), tab: InstanceTab::Files },
            Route::Instance { id: "a/b".into(), tab: InstanceTab::Worlds },
            Route::Instance { id: "a/b".into(), tab: InstanceTab::Screenshots },
            Route::Instance { id: "a/b".into(), tab: InstanceTab::Logs },
            Route::Instance { id: "a/b".into(), tab: InstanceTab::Share },
        ]
    }

    #[test]
    fn the_names_are_the_references_names() {
        // Straight out of `routes.js`. A rename here would make every later
        // port of a page harder to line up with upstream.
        assert_eq!(Route::Home.name(), "Home");
        assert_eq!(Route::Discover { project_type: ProjectType::Modpack }.name(), "Discover content");
        assert_eq!(Route::Skins.name(), "Skin selector");
        assert_eq!(Route::Servers.name(), "Servers");
        assert_eq!(
            Route::Instance { id: "x".into(), tab: InstanceTab::ContentFilter(ProjectType::Mod) }
                .name(),
            "InstanceContentFilter"
        );
        assert_eq!(
            Route::Project { id: "x".into(), tab: ProjectTab::Version("v".into()) }.name(),
            "Version"
        );
        assert_eq!(
            Route::Server { id: "x".into(), tab: ServerTab::Backups }.name(),
            "ServerManageBackups"
        );
        assert_eq!(ProjectType::ALL.len(), 7, "the alias table is keyed on these");
    }

    #[test]
    fn the_legacy_project_urls_redirect_the_way_the_reference_redirects_them() {
        // `routes.js`: `/:projectType(mod|plugin|datapack|resourcepack|shader|
        // modpack)/:id/:rest(.*)*` -> `/project/:id/:rest`.
        for path in ["/mod/sodium", "/plugin/bukkit", "/datapack/foo/versions"] {
            let parsed = Address::parse(path).expect(path);
            assert!(
                matches!(parsed.route, Route::Project { .. }),
                "{path} did not redirect to a project page"
            );
        }
        let versions = Address::parse("/mod/sodium/versions").expect("legacy versions");
        assert_eq!(versions.route, Route::Project {
            id: "sodium".into(),
            tab: ProjectTab::Versions,
        });
        // A single segment is not the legacy form: `/mod` alone is not a
        // project id, and routes.js has no route for it either.
        assert!(Address::parse("/mod").is_none());
    }

    #[test]
    fn a_three_segment_hosting_path_names_a_server_called_content() {
        // `/hosting/manage/:id` nests `''`, `content`, `files`, `backups` and
        // `access` under the *id*, so the ambiguity is real and the reference
        // resolves it by segment count. Getting this backwards would make a
        // server's Overview unreachable whenever it happened to be named after
        // one of the tabs.
        assert_eq!(
            Address::parse("/hosting/manage/content").map(|address| address.route),
            Some(Route::Server { id: "content".into(), tab: ServerTab::Overview })
        );
        assert_eq!(
            Address::parse("/hosting/manage/content/content").map(|address| address.route),
            Some(Route::Server { id: "content".into(), tab: ServerTab::Content })
        );
    }

    #[test]
    fn both_spellings_of_the_servers_page_are_the_servers_page() {
        // `routes.js` declares `/hosting/manage/` with a trailing slash and
        // `App.vue` links to `/hosting/manage` without one, and its
        // `is-primary` predicate compares against both.
        assert_eq!(Address::parse("/hosting/manage/").map(|a| a.route), Some(Route::Servers));
        assert_eq!(Address::parse("/hosting/manage").map(|a| a.route), Some(Route::Servers));
    }

    #[test]
    fn unknown_paths_are_refused_rather_than_guessed_at() {
        for path in ["/mods", "/worlds", "/logs", "/settings", "/accounts", "/about", "/nonsense"] {
            assert!(Address::parse(path).is_none(), "{path} should not resolve");
        }
    }

    #[test]
    fn a_fragment_after_the_query_does_not_become_part_of_the_path() {
        // The legacy redirect carries `to.hash` through to the project URL, so
        // `/mod/sodium#gallery` is a real shape the reference produces.
        let address = Address::parse("/project/sodium/versions?i=x#gallery").expect("fragment");
        assert_eq!(address.route, Route::Project {
            id: "sodium".into(),
            tab: ProjectTab::Versions,
        });
        assert_eq!(address.context.instance.as_deref(), Some("x"));
    }

    #[test]
    fn the_rail_marks_the_page_the_reference_marks() {
        let marks = |path: &str| Address::parse(path).expect(path).marks();
        assert_eq!(marks("/"), vec![(Rail::Home, Mark::Primary)]);
        assert_eq!(marks("/browse/modpack"), vec![(Rail::Discover, Mark::Primary)]);
        assert_eq!(marks("/project/sodium"), vec![(Rail::Discover, Mark::Subpage)]);
        assert_eq!(
            marks("/project/sodium?i=atm10"),
            vec![(Rail::Home, Mark::Subpage)],
            "a project opened inside an instance belongs to the instance"
        );
        assert_eq!(marks("/browse/mod?i=atm10"), vec![(Rail::Home, Mark::Subpage)]);
        assert_eq!(marks("/browse/server?sid=abc"), vec![(Rail::Servers, Mark::Subpage)]);
        assert_eq!(marks("/hosting/manage/"), vec![(Rail::Servers, Mark::Primary)]);
        assert_eq!(marks("/hosting/manage/srv/backups"), vec![(Rail::Servers, Mark::Subpage)]);
        assert_eq!(marks("/skins"), vec![(Rail::Skins, Mark::Primary)]);
        assert_eq!(marks("/screenshots"), vec![(Rail::Screenshots, Mark::Primary)]);
        assert!(
            marks("/user/jelly").is_empty(),
            "the profile slot is an overflow menu, so the reference never marks it"
        );
        assert!(
            marks("/instance/atm10").is_empty(),
            "none of the three rail buttons with predicates tests for /instance"
        );
        assert!(
            marks("/instance/atm10/worlds").is_empty(),
            "an instance tab is still an instance page"
        );
    }

    #[test]
    fn the_rail_has_eight_slots_and_three_of_them_are_not_routes() {
        assert_eq!(Rail::ALL.len(), 8);
        let addressable: Vec<Rail> = sample()
            .iter()
            .flat_map(|route| Address::at(route.clone()).marks())
            .map(|(rail, _)| rail)
            .collect();
        for slot in [Rail::CreateInstance, Rail::Settings, Rail::Profile] {
            assert!(
                !addressable.contains(&slot),
                "{slot} is a button in the reference, not a route"
            );
        }
    }

    #[test]
    fn only_the_search_pages_force_the_right_panel_open() {
        // `App.vue`'s `forceSidebar`: `/browse`, `/project` and `/user`.
        assert!(Route::Discover { project_type: ProjectType::Mod }.forces_sidebar());
        assert!(Route::Project { id: "x".into(), tab: ProjectTab::Gallery }.forces_sidebar());
        assert!(Route::User { user: "jelly".into(), project_type: None }.forces_sidebar());
        assert!(!Route::Home.forces_sidebar());
        assert!(!Route::Skins.forces_sidebar());
        assert!(!Route::Screenshots.forces_sidebar());
        assert!(!Route::Servers.forces_sidebar());
        assert!(!Route::Instance { id: "x".into(), tab: InstanceTab::Content }.forces_sidebar());
    }

    #[test]
    fn a_tab_knows_which_page_it_is_a_tab_of() {
        assert_eq!(
            Route::Instance { id: "x".into(), tab: InstanceTab::ContentFilter(ProjectType::Mod) }
                .parent(),
            Some(Route::Instance { id: "x".into(), tab: InstanceTab::Content })
        );
        assert_eq!(
            Route::Project { id: "x".into(), tab: ProjectTab::Gallery }.parent(),
            Some(Route::Project { id: "x".into(), tab: ProjectTab::Description })
        );
        assert_eq!(
            Route::Server { id: "x".into(), tab: ServerTab::Access }.parent(),
            Some(Route::Server { id: "x".into(), tab: ServerTab::Overview })
        );
        // A page is not its own child.
        assert_eq!(
            Route::Instance { id: "x".into(), tab: InstanceTab::Content }.parent(),
            None
        );
        assert_eq!(Route::Home.parent(), None);
    }

    #[test]
    fn project_type_tokens_are_the_route_tokens_and_the_aliases_resolve() {
        for kind in ProjectType::ALL {
            assert_eq!(ProjectType::from_token(kind.token()), Some(*kind));
        }
        // `PROJECT_TYPE_ALIASES`, from `common-messages.ts`.
        assert_eq!(ProjectType::from_token("shaderpack"), Some(ProjectType::Shader));
        assert_eq!(
            ProjectType::from_token("minecraft_java_server"),
            Some(ProjectType::Server)
        );        assert_eq!(ProjectType::from_token("shaders"), None);
        assert_eq!(ProjectType::from_token(""), None);
    }

    #[test]
    fn a_profile_address_takes_the_plural_the_reference_links_with() {
        // The reference's own strip href, spelled `mods`; the singular is this
        // tree's older spelling and still parses, because an address somebody
        // already has must not stop working.
        for kind in ProjectType::ALL {
            assert_eq!(ProjectType::from_profile_token(&kind.profile_token()), Some(*kind));
            assert_eq!(ProjectType::from_profile_token(kind.token()), Some(*kind));
        }
        assert_eq!(ProjectType::from_token("mods"), None, "the API's own spelling is singular");
        // The reference's fourth link is *not* a project type, and this assertion is
        // the reason it never becomes one: the token resolves to no type at all, so
        // nothing that derives from a type -- `target_folder`, `sentence`, `TABS` --
        // has an arm to answer for. What changed is not this line but the address
        // below: `/user/jelly/collections` is a real one, through `ProfileTab`.
        assert_eq!(ProjectType::from_profile_token("collections"), None);
        assert_eq!(ProjectType::from_profile_token("collection"), None);
        assert_eq!(ProjectType::from_profile_token(""), None);
        assert_eq!(ProjectType::from_profile_token("s"), None);
        // And the addresses themselves, both ways round.
        for path in [
            "/user/jelly",
            "/user/jelly/mods",
            "/user/jelly/mod",
            "/user/jelly/resourcepacks",
            "/user/jelly/modpacks",
        ] {
            let address = Address::parse(path).unwrap_or_else(|| panic!("{path} is not a route"));
            assert!(matches!(address.route, Route::User { .. }), "{path}");
        }
        assert_eq!(
            Address::parse("/user/jelly/mods").map(|address| address.route),
            Address::parse("/user/jelly/mod").map(|address| address.route),
            "the two spellings are one page"
        );
        // The collections address, which used to be `is_none()` here and was the
        // reason the strip's fourth tab was a dead end: `layout.vue:779` builds it
        // out of the same template that builds `mods`, so it is an address, and it
        // names a profile tab rather than a project type.
        assert_eq!(
            Address::parse("/user/jelly/collections").map(|address| address.route),
            Some(Route::User {
                user: "jelly".into(),
                project_type: Some(ProfileTab::Collections),
            })
        );
        // `parseProjectTypeRouteParam` accepts the singular too
        // (`v3-projects.ts:81`), so both of the reference's spellings land here --
        // and on the same tab, which is the point of the pair.
        assert_eq!(
            Address::parse("/user/jelly/collection").map(|address| address.route),
            Address::parse("/user/jelly/collections").map(|address| address.route)
        );
    }

    #[test]
    fn the_collections_address_round_trips_and_a_third_segment_that_is_nothing_is_refused() {
        // Read, written, read: `ProfileTab::profile_token` is the writer the shell
        // uses for every one of the strip's four tabs, so the four addresses and
        // the four tabs are one list and this walks all of it.
        let strip = [
            (None, "/user/FlameFire"),
            (Some(ProfileTab::Projects(ProjectType::Datapack)), "/user/FlameFire/datapacks"),
            (Some(ProfileTab::Projects(ProjectType::Modpack)), "/user/FlameFire/modpacks"),
            (Some(ProfileTab::Collections), "/user/FlameFire/collections"),
        ];
        for (tab, path) in strip {
            let written = match tab {
                Some(tab) => format!("/user/FlameFire/{}", tab.profile_token()),
                None => "/user/FlameFire".to_string(),
            };
            assert_eq!(written, path, "{tab:?} is not spelled the reference's way");
            let parsed = Address::parse(path).unwrap_or_else(|| panic!("{path} is not a route"));
            assert_eq!(parsed.route, Route::User { user: "FlameFire".into(), project_type: tab });
            assert_eq!(parsed.context, Context::default());
            // And back again.
            assert_eq!(
                Address::parse(&written).map(|address| address.route),
                Some(parsed.route),
                "{written} did not survive a round trip"
            );
        }
        // The four addresses are four tabs: none of them is another one, and none is
        // the bare profile.
        let routes: Vec<Route> = strip
            .iter()
            .map(|(_, path)| Address::parse(path).expect("a route").route)
            .collect();
        for (index, route) in routes.iter().enumerate() {
            for (other, earlier) in routes.iter().enumerate() {
                assert_eq!(index == other, route == earlier, "two of the four are one address");
            }
        }
        // A third segment that names nothing is still refused, and refused as a
        // whole rather than read as *All*: `Address::parse` answers `None` so the
        // shell falls back rather than landing the reader on a page that says
        // something else.
        for path in ["/user/jelly/nonsense", "/user/jelly/collectionss", "/user/jelly/s"] {
            assert!(Address::parse(path).is_none(), "{path} should not resolve");
        }
        // A user called `collections` is a name, not a tab: the first segment is
        // read as the user wherever it sits, and `/collections` is the strip's token
        // rather than a top-level page.
        assert!(Address::parse("/collections").is_none());
        assert!(matches!(
            Address::parse("/user/collections/mods").map(|address| address.route),
            Some(Route::User {
                user,
                project_type: Some(ProfileTab::Projects(ProjectType::Mod)),
            }) if user == "collections"
        ));
    }

    #[test]
    fn a_collection_is_a_profile_tab_and_not_a_project_type() {
        // The one assertion that keeps the two apart, and the reason
        // `ProfileTab` is its own enum: a collection is selected by an address and
        // filters nothing.
        assert_eq!(
            ProfileTab::from_profile_token("collections"),
            Some(ProfileTab::Collections)
        );
        assert_eq!(ProfileTab::from_profile_token("collection"), Some(ProfileTab::Collections));
        assert_eq!(
            ProfileTab::from_profile_token("mods"),
            Some(ProfileTab::Projects(ProjectType::Mod))
        );
        assert_eq!(ProfileTab::from_profile_token(""), None);
        // No type is handed back for the collections token, and the tab is not one
        // of the types any of the orderings carry.
        assert_eq!(ProjectType::from_profile_token("collections"), None);
        assert_eq!(ProjectType::from_profile_token("collection"), None);
        for kind in ProjectType::ALL {
            assert_ne!(ProfileTab::Projects(*kind), ProfileTab::Collections);
        }
        // It is not in the list the strip sorts, and `PROJECT_TYPE_ORDER`'s own
        // `collection` entry is not read as one: that order ends with `collection`
        // (`ui/src/utils/project-types.ts:1-10`) purely to put the tab last, which
        // `pages::user` does by appending rather than sorting.
        assert_eq!(ProjectType::PROFILE_ORDER.len(), 7);
        assert_eq!(ProjectType::ALL.len(), 7, "still seven: no variant was added");
        assert_eq!(ProjectType::TABS.len(), 6, "and Discover still offers six");
        // The one address a collection has is its own, and it is the one the
        // reference's `navLinks` builds for it.
        assert_eq!(ProfileTab::Collections.profile_token(), "collections");
        for kind in ProjectType::ALL {
            assert_ne!(kind.profile_token(), "collections");
        }
    }

    #[test]
    fn discover_offers_six_tabs_in_the_order_the_reference_draws_them() {
        // `Browse.vue`'s `tabs` computed, in order.
        assert_eq!(
            ProjectType::TABS.iter().map(|kind| kind.token()).collect::<Vec<_>>(),
            vec!["modpack", "mod", "resourcepack", "datapack", "shader", "server"]
        );
        // Plugin exists in `routes.js`'s redirect and in the messages but is
        // not one of the tabs, so it is in `ALL` and not in `TABS`.
        assert!(ProjectType::ALL.contains(&ProjectType::Plugin));
        assert!(!ProjectType::TABS.contains(&ProjectType::Plugin));
    }

    /// The install table, from the API's own spelling of a project type through
    /// to the folder under an instance root.
    #[test]
    fn a_kind_installs_into_the_folder_it_names() {
        let folder = |token: &str| {
            ProjectType::from_token(token).and_then(|kind| kind.target_folder())
        };
        assert_eq!(folder("mod"), Some("mods"));
        assert_eq!(folder("resourcepack"), Some("resourcepacks"));
        assert_eq!(folder("datapack"), Some("datapacks"));
        // The API's long spelling, which is why this goes through `from_token`
        // rather than comparing the string to a token.
        assert_eq!(folder("shaderpack"), Some("shaderpacks"));
        assert_eq!(folder("plugin"), Some("plugins"));
        assert_eq!(
            folder("modpack"),
            None,
            "a pack is an instance, not a folder inside one"
        );
        assert_eq!(folder("minecraft_java_server"), None);
        assert_eq!(folder("not a type"), None);
    }

    #[test]
    fn the_labels_are_the_messages_the_reference_ships() {
        assert_eq!(ProjectType::Modpack.label(), "Modpacks");
        assert_eq!(ProjectType::ResourcePack.label(), "Resource Packs");
        assert_eq!(ProjectType::Datapack.label(), "Data Packs");
        assert_eq!(ProjectType::Mod.sentence(1), "mod");
        assert_eq!(ProjectType::Mod.sentence(4), "mods");
        assert_eq!(ProjectType::ResourcePack.sentence(1), "resource pack");
        assert_eq!(ProjectType::Server.sentence(2), "servers");
    }

    #[test]
    fn escaping_round_trips_including_bytes_outside_ascii() {
        for raw in ["plain", "with space", "a/b", "café", "100%", "a+b", "q?uery", "emoji 🎮"] {
            assert_eq!(decode(&encode(raw)), raw, "{raw} did not survive");
        }
        // `encodeURIComponent`'s safe set, spot-checked: these five are the
        // ones RFC 3986 would have escaped instead.
        assert_eq!(encode("!*'()"), "!*'()");
        assert_eq!(encode("a b+c&d=e"), "a%20b%2Bc%26d%3De");
        // A stray `%` is a literal, which is what `decodeURIComponent`-style
        // leniency means: a malformed link must not take the shell down.
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
    }
}
