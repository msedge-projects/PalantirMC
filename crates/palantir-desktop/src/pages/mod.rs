//! The pages, and the one place they are turned into an `Element`.
//!
//! One module per page, each with its own `State`, its own `Message` and its own
//! tests -- the reference's own arrangement, where a route renders a component
//! that owns its data. [`Screen`] is which of them the shell is showing, and the
//! three functions below are the whole of the shell's knowledge about them: build
//! one from an address, keep what can be kept when the address changes, and draw.
//!
//! Why the pages are not in `shell.rs`: nineteen routes in one file is how the
//! interface it replaces ended up 10,000 lines long and impossible to gate. A page
//! here is answerable on its own -- its states, its filtering, its keys -- and the
//! shell's own file stays the size of the frame it draws.

pub mod discover;
pub mod home;
pub mod instance;
pub mod overlay;
pub mod project;
pub mod screenshots;
pub mod servers;
pub mod skins;
pub mod user;

use iced::Element;

use crate::route::{Address, Route};
use crate::store::Store;
use crate::theme_gen::Theme as Gen;

/// Which page is on screen.
///
/// One variant per page, each holding that page's own state, so the variants are
/// not the same size and clippy says so. Boxing the largest would put an
/// allocation in the way of every match to save a few bytes in a value the shell
/// holds exactly one of, which is a trade the lint cannot see and this comment
/// exists to record.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Screen {
    /// The library, at `/`.
    Home(home::State),
    /// Discover, at `/browse/:projectType`.
    Discover(discover::State),
    /// A project, at `/project/:id/:tab?`.
    Project(project::State),
    /// One instance, at `/instance/:id/:tab?`.
    Instance(instance::State),
    /// The skin selector.
    Skins(skins::State),
    /// The screenshot library.
    Screenshots(screenshots::State),
    /// Modrinth Hosting.
    Servers(servers::State),
    /// A user's profile.
    User(user::State),
}

/// Everything the pages can be told.
///
/// One variant per page rather than one per control: a page's own messages belong
/// to it, so adding a control to a page cannot change the shell's type. The shell
/// wraps the whole family in one variant of its own.
#[derive(Debug, Clone)]
pub enum Message {
    /// The library.
    Home(home::Message),
    /// Discover.
    Discover(discover::Message),
    /// A project.
    Project(project::Message),
    /// An instance.
    Instance(instance::Message),
    /// The skin selector.
    Skins(skins::Message),
    /// The screenshots.
    Screenshots(screenshots::Message),
    /// Modrinth Hosting.
    Servers(servers::Message),
    /// A profile.
    User(user::Message),
}

/// A navigation a page asked for.
///
/// Opening a thing is not page state: it changes which page is on screen, and the
/// history. A page therefore *reports* it rather than performing it, and the shell
/// is the one caller that acts on it -- which is also why the return of
/// [`Screen::update`] is an `Option` rather than another message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Open {
    /// Show an instance, at `/instance/:id`.
    Instance(String),
    /// Show a project, at `/project/:id`.
    Project(String),
    /// Show a profile, at `/user/:user/:projectType?`.
    ///
    /// The only navigation whose *address* carries a choice rather than a name: a
    /// profile's filter strip is links in the reference, so choosing one is a move
    /// to `/user/:user/:type` rather than a change of state, and the address stays
    /// the one record of what is on screen.
    User {
        /// Whose profile.
        user: String,
        /// The third segment the address names, if it names one.
        ///
        /// A [`crate::route::ProfileTab`] rather than a `ProjectType` because the
        /// strip's fourth link is not a kind of project: the reference's own
        /// `navLinks` (`layout.vue:766-780`) builds `collections` out of the same
        /// template that builds `mods`, so all four tabs are one navigation and this
        /// field is what all four of them carry. `None` is the *All* tab.
        project_type: Option<crate::route::ProfileTab>,
    },
}

/// Something a page asked for that only the shell can do.
///
/// The second kind is a *request*: a page describes what to ask for and the shell
/// is the one that can ask it, because asking is blocking and a page is drawn on
/// the frame thread (see [`crate::store`]). Neither kind is page state, and both
/// come back to the page as a message of its own -- a navigation by being applied
/// to the address, an answer by [`Message::search_result`] or
/// [`Message::project_result`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// Navigate to a thing.
    Open(Open),
    /// Ask the store, through the engine, for search results.
    Search(discover::Asked),
    /// Ask the store for the icons of a page of results: one fetch per URL, and one
    /// decode per picture, neither of them on the frame thread.
    ///
    /// A request of its own rather than part of [`Ask::Search`], because it is made
    /// at a different moment: the search has to land before the page knows which
    /// URLs it is missing, and the cards have to be drawable while the pictures are
    /// still coming. That is what makes this the one ask whose answer is decoration
    /// -- the page reserves the box either way ([`crate::ui::icon_box`]).
    Icons(discover::Icons),
    /// Ask the store, through the engine, for the tag lists: every category, game
    /// version and loader Modrinth knows.
    ///
    /// Not decoration and not decoration-adjacent either: the browse sidebar's
    /// filter sections are *made* of them, and a section with no options is not a
    /// section that drew badly -- it is a section with nothing in it. It is asked
    /// before the first search rather than beside it because the shell's
    /// `opening` hands back one request at a time, and these are the ones whose
    /// answer the page cannot draw anything without.
    Tags,
    /// Ask the store, through the engine, for one project: its own document, the
    /// people on its team, and its version list.
    ///
    /// One request rather than three because a project page draws all three at
    /// once, and three `Load`s for one page is three ways to be half drawn.
    Project(project::Asked),
    /// Ask the store, through the engine, for one user's profile: their own
    /// document and the projects they own.
    ///
    /// The same shape as [`Ask::Project`], and the same reason for one request
    /// rather than two: the page draws the header and the list together, and a page
    /// that could draw one of them first would draw a name over somebody else's
    /// projects.
    User(user::Asked),
    /// Read the account's own appearance for the Skins page: what it owns, and the
    /// skin in force drawn.
    ///
    /// The same shape as the two above, and the one where *which account* is most
    /// clearly the shell's: a page has never seen an account file, a selection or a
    /// token, and the shell reads all three from the launcher's own store.
    Skins(skins::Asked),
    /// Change what the account is wearing: put one of its own skins or capes on,
    /// or take it off.
    ///
    /// The write half of [`Ask::Skins`] and the same shape, because it is the same
    /// secret: *which* account and *which* token the change is made with are the
    /// shell's, and a page that could make it would have to hold both.
    Wear(skins::Wear),
    /// Add a skin from a file: open the launcher's own picker, read what it returns,
    /// and upload it as the account's skin.
    ///
    /// The first ask that is neither a question nor a change the page can describe, and
    /// the reason it exists as its own shape: what a reader picks is a *path*, and the
    /// bytes behind it are not a page's to hold -- a page has never read a file in this
    /// tree, for the same reason it has never held a token. So the page asks for the
    /// dialog and reports the round; the file, the padding and the upload are the
    /// shell's, and `crate::pick` says why the dialog has to be opened on the thread the
    /// window lives on.
    AddSkin(skins::Add),
    /// Do what the Skins page's edit modal asked for: write a stored row's arm
    /// style and cape and put it on, forget it, or take the account's skin off.
    ///
    /// Three actions in one ask because they are three presses of one modal, and
    /// because two of them are the launcher's *own* files rather than the
    /// account's: only the shell knows where this launcher's skin store lives
    /// (`crate::saved_skins`), and only the shell holds the token the third and
    /// the first need. The description travels -- the row's key, the chosen arm
    /// style and cape, and which of the three -- and the pixels are read here,
    /// because a page has never read a file in this tree.
    EditSkin(skins::Edit),
    /// Write the Skins page's saved rows in the order the reader put them in.
    ///
    /// A write with no account in it at all: the order is `crate::saved_skins`'
    /// own index, which is a file under this product's directory and therefore the
    /// shell's -- a page has never written a file in this tree -- and the reference
    /// reaches the same write through its own store's `set_custom_skin_order`.
    Reorder(skins::Reorder),
    /// Put a project into one of the launcher's instances.
    ///
    /// The same shape as the other asks, pointed at a file instead of a page: the
    /// *button* is the project page's, and which instances exist, which version
    /// fits one and where its folder is are all the shell's -- a page that
    /// installed a mod would have to know the whole of [`crate::store`].
    Install(project::Install),
    /// Open the creation flow.
    ///
    /// The third kind, and the same shape as the other two: the *button* is the
    /// page's, and the dialog, the request and the folder are not. A page that
    /// could create an instance would have to know where instances live, which
    /// is what [`crate::store`] is for.
    Create,
    /// Open one instance's own settings.
    ///
    /// [`Ask::Create`]'s shape pointed at a file that already exists: the button
    /// is the instance page's header, and the form, the read and the write are the
    /// shell's -- a page that could write an instance's own settings would have to
    /// know [`crate::store`] and the override gates behind it.
    InstanceSettings(String),
    /// Open the creation flow's import step, for the same reasons and off the same
    /// button on the welcome screen.
    Import,
    /// Run this instance.
    ///
    /// The fourth kind, and the one with the most behind it: the process, the
    /// account to sign in with, the memory and Java the launch is for, and the
    /// subscription that streams it are all the shell's, and a page that could
    /// start a game would have to know every one of them.
    Play(String),
    /// Stop the instance that is running.
    ///
    /// Deliberately the twin of [`Ask::Play`] rather than the same message with a
    /// flag: the reference's header emits `play` and `stop` as two events, and a
    /// page that cannot tell them apart cannot draw two buttons.
    Stop(String),
    /// Read one tab's own listing: an instance's mods, its files, its worlds, its
    /// screenshots, or the tail of its newest log.
    ///
    /// Not a document and not a request to a service -- the fifth kind, and the
    /// one that is about a page's own *cost*: these reads belong where they can
    /// happen once per tab rather than once per frame, and the frame thread is the
    /// one place they cannot. `crate::scale` has what reading them there cost.
    Instance(instance::Asked),
}

impl Message {
    /// The message that carries a search answer back to the page that asked.
    ///
    /// The shell builds this and never names a page's own messages: the seam
    /// between the engine and a page is this one function wide, so moving a search
    /// to another page is a change here rather than everywhere the shell runs a
    /// request.
    pub fn search_result(asked: &discover::Asked, result: Result<Vec<discover::Hit>, String>) -> Message {
        Message::Discover(discover::Message::Found { round: asked.round, result })
    }

    /// The message that carries a page of icons back to Discover.
    ///
    /// [`Message::search_result`]'s twin, and the second half of one answer: a
    /// search brings the results, and this brings the pictures those results name.
    /// It is a separate message because it is a separate request -- the results are
    /// on screen before it is made.
    ///
    /// No request is named here, unlike its siblings, because there is no round to
    /// check an icon against: an answer is keyed by the URL it carries, so an icon
    /// for a project the page has stopped showing is one nothing ever looks up.
    pub fn search_icons(arrived: Vec<crate::avatar::Fetched>) -> Message {
        Message::Discover(discover::Message::Icons { arrived })
    }

    /// The message that carries the tag lists back to Discover.
    ///
    /// Named here rather than built in the shell for the same reason the two above
    /// are: the answer's type is the page's, and the shell has never seen a
    /// `FilterType` in its life.
    pub fn tags(result: Result<palantir_net::Tags, String>) -> Message {
        Message::Discover(discover::Message::Tags { result })
    }

    /// The message that carries one tab's listing back to the instance page.
    ///
    /// The same seam as [`Message::search_result`], and the answer's own type is
    /// the page's for the same reason: the shell has never seen a listing, and the
    /// round it was read in travels with it so the page can drop an answer to a tab
    /// the reader has left.
    pub fn instance_result(
        asked: &instance::Asked,
        listing: Result<crate::store::Listing, String>,
    ) -> Message {
        Message::Instance(instance::Message::Listed { round: asked.round, listing })
    }

    /// The message that carries a project answer back to the page that asked.
    ///
    /// The same seam as [`Message::search_result`], and the reason it is a
    /// function rather than a variant the shell names: the answer's own type is
    /// the page's, and the shell has never seen one.
    pub fn project_result(asked: &project::Asked, result: Result<project::Project, String>) -> Message {
        // Boxed here, at the one crossing between the store's answer and a page's
        // message, so the page's own arms never box anything (see
        // `project::Message::Found`).
        Message::Project(project::Message::Found {
            round: asked.round,
            result: result.map(Box::new),
        })
    }

    /// The message that carries a profile back to the page that asked for it.
    ///
    /// The same seam as [`Message::project_result`], for the same reason: the
    /// answer's own type is the page's -- a profile is a document, a list and a
    /// picture that the shell has never seen as a value -- so the shell hands it
    /// over without naming any of it.
    pub fn user_result(asked: &user::Asked, result: Result<user::Profile, String>) -> Message {
        // Boxed here, at the one crossing between the store's answer and a page's
        // message, so the page's own arms never box anything.
        Message::User(user::Message::Found {
            round: asked.round,
            result: result.map(Box::new),
        })
    }

    /// The message that carries an account's appearance back to the Skins page.
    ///
    /// The same seam as [`Message::project_result`], and the reason it is a
    /// function: the answer's own type is the page's, and the shell has never seen
    /// one. It hands over what the store said and names nothing else.
    pub fn skins_result(asked: &skins::Asked, result: Result<skins::Loaded, String>) -> Message {
        Message::Skins(skins::Message::Found {
            round: asked.round,
            result: result.map(Box::new),
        })
    }

    /// The message that carries a change's outcome back to the Skins page.
    ///
    /// [`Message::skins_result`]'s twin, and the one place a page's *write* is
    /// answered: the same seam, handed a `Result` with nothing in it on success,
    /// because what changed is the document the page reloads rather than a value
    /// this carries.
    pub fn skin_worn(worn: &skins::Wear, result: Result<(), String>) -> Message {
        Message::Skins(skins::Message::Applied { round: worn.round, result })
    }

    /// The message that carries the file picker's outcome back to the Skins page.
    ///
    /// [`Message::skin_worn`]'s twin, and the one crossing where the answer is not a
    /// `Result`: a reader who closed the dialog chose nothing rather than failed at
    /// something, and `skins::Picked` is the type that says so.
    pub fn skin_added(add: &skins::Add, picked: skins::Picked) -> Message {
        Message::Skins(skins::Message::Added { round: add.round, picked })
    }

    /// The message that carries the edit modal's outcome back to the Skins page.
    ///
    /// [`Message::skin_worn`]'s twin, and the same sentence on failure: what a
    /// row's write changed is the row or the account, and the page reloads one
    /// and redraws the other rather than being told about it.
    pub fn skin_saved(edit: &skins::Edit, result: Result<(), String>) -> Message {
        Message::Skins(skins::Message::Edited { round: edit.round, result })
    }

    /// The message that carries a reorder's outcome back to the Skins page.
    ///
    /// [`Message::skin_saved`]'s twin, and the same sentence on failure: what an
    /// order change rewrites is the store the page reads, so a success reloads it
    /// and the reader sees the order they asked for rather than a sentence about it.
    pub fn skin_reordered(order: &skins::Reorder, result: Result<(), String>) -> Message {
        Message::Skins(skins::Message::Reordered { round: order.round, result })
    }

    /// The message that carries an install's outcome back to the page whose button
    /// asked for it.
    ///
    /// A sentence rather than the installed file: `store::Installed` has already
    /// been turned into one by the time it gets here -- the file name, the version
    /// and the instance's own name are the shell's to know, because the page asked
    /// for a transfer rather than for a path -- and a failure is a sentence in the
    /// same slot, which is what the reference's own error cards are.
    pub fn install_result(result: Result<String, String>) -> Message {
        Message::Project(project::Message::Noted(match result {
            Ok(line) => line,
            Err(reason) => reason,
        }))
    }
}

impl Screen {
    /// The page an address is drawn by.
    pub fn at(address: &Address) -> Screen {
        match &address.route {
            Route::Home => Screen::Home(home::State::default()),
            Route::Discover { project_type } => Screen::Discover(discover::State::new(*project_type)),
            Route::Project { id, tab } => Screen::Project(project::State::new(id.clone(), tab.clone())),
            Route::Instance { id, tab } => {
                Screen::Instance(instance::State::new(id.clone(), tab.clone()))
            }
            Route::Skins => Screen::Skins(skins::State::default()),
            Route::Screenshots => Screen::Screenshots(screenshots::State::default()),
            Route::Servers | Route::Server { .. } => Screen::Servers(servers::State::default()),
            // The two parts of a profile's address are the page's own state rather
            // than something the shell hands it at drawing time: a page that could
            // not name the user it is about could not ask for their profile, and
            // `update` sees no address.
            Route::User { user, project_type } => {
                Screen::User(user::State::new(user.clone(), *project_type))
            }
        }
    }

    /// Whether the page on screen is the page `route` names, so that pointing it
    /// somewhere new reuses it rather than building another.
    ///
    /// This is the question [`Screen::retarget`] answers by acting, asked
    /// separately because two callers need to know the answer *before* they act:
    /// the shell, to decide whether the page it is about to swap should fade in.
    /// Asked afterwards it is always true, because by then both pages are the
    /// same one.
    ///
    /// The rule is the one `retarget` already follows: a tab or a filter within a
    /// page is the same page, and a different project, instance, profile or route
    /// is not. `Route::Server` is deliberately absent from the `Servers` arm: the
    /// reference's `/hosting/manage/:id` is a different view from its list, and
    /// `retarget` answers it by building a new page, so this must say no.
    pub fn serves(&self, route: &crate::route::Route) -> bool {
        use crate::route::Route;
        match (self, route) {
            (Screen::Home(_), Route::Home) => true,
            (Screen::Discover(_), Route::Discover { .. }) => true,
            (Screen::Skins(_), Route::Skins) => true,
            (Screen::Screenshots(_), Route::Screenshots) => true,
            (Screen::Servers(_), Route::Servers) => true,
            (Screen::Project(state), Route::Project { id, .. }) => state.id == *id,
            (Screen::Instance(state), Route::Instance { id, .. }) => state.id == *id,
            (Screen::User(state), Route::User { user, .. }) => state.user == *user,
            _ => false,
        }
    }

    /// Point a page at a new address, keeping what the new address still means.
    ///
    /// A tab change is not a new page: pressing Files on an instance must not
    /// throw away the search the user typed on Content, and the reference keeps a
    /// component alive across its own tab routes for the same reason. A *different*
    /// instance or project is a new page, and gets one.
    pub fn retarget(&mut self, address: &Address) {
        match (&mut *self, &address.route) {
            (Screen::Discover(state), Route::Discover { project_type }) => {
                state.update(discover::Message::ProjectType(*project_type));
            }
            (Screen::Project(state), Route::Project { id, tab }) => {
                if state.id != *id {
                    *state = project::State::new(id.clone(), tab.clone());
                } else {
                    state.update(project::Message::Tab(tab.clone()));
                }
            }
            (Screen::User(state), Route::User { user, project_type }) => {
                if state.user != *user {
                    *state = user::State::new(user.clone(), *project_type);
                } else {
                    // A filter is not a new document: the profile the page is
                    // holding is the answer either way, so the strip's tabs move the
                    // page without a request. A *different* user is a new page, and
                    // is built above.
                    state.filter(*project_type);
                }
            }
            (Screen::Instance(state), Route::Instance { id, tab }) => {
                if state.id != *id {
                    *state = instance::State::new(id.clone(), tab.clone());
                } else {
                    // A tab is not something only the shell can do, so there is
                    // nothing to report -- and the store is not read: a tab
                    // press does not touch the machine.
                    let _ = state.update(instance::Message::Tab(tab.clone()), &Store::default());
                }
            }
            // Any other kind is another page, and the one on screen had its
            // chance to keep something.
            _ => *self = Screen::at(address),
        }
    }

    /// Apply a message, and report anything only the shell can do.
    ///
    /// The store is handed in because the pages act on the machine through it
    /// rather than on themselves: the Content tab toggles a mod file, and a search
    /// is a request the store makes.
    pub fn update(&mut self, message: Message, store: &Store) -> Option<Ask> {
        match (self, message) {
            // A card press is navigation, and is reported rather than applied: the
            // page does not know what else has to change when the pane does.
            (Screen::Home(_), Message::Home(home::Message::Open(id))) => {
                return Some(Ask::Open(Open::Instance(id)))
            }
            (Screen::Discover(_), Message::Discover(discover::Message::Open(id))) => {
                return Some(Ask::Open(Open::Project(id)))
            }
            // The library's create button is reported rather than applied: what
            // makes an instance is the store, and where the flow goes afterwards
            // is the shell's.
            (Screen::Home(_), Message::Home(home::Message::CreateInstance)) => {
                return Some(Ask::Create)
            }
            (Screen::Home(_), Message::Home(home::Message::ImportFromLauncher)) => {
                return Some(Ask::Import)
            }
            (Screen::Home(state), Message::Home(message)) => state.update(message),
            (Screen::Discover(state), Message::Discover(message)) => {
                // The first page that asked for anything. The request it reports
                // is a value: the shell runs it, and the answer comes back as a
                // message rather than as a return value, because an answer
                // arrives turns later.
                //
                // It answers with an `Ask` of its own now rather than with the
                // page's `Asked`, for the project page's reason: this page has a
                // second kind of request -- the icons of the results it is drawing
                // -- and only one of the two is a search.
                if let Some(asked) = state.update(message) {
                    return Some(asked);
                }
            }
            (Screen::Project(state), Message::Project(message)) => {
                // The second page that asks for something: the shell runs it and
                // the answer comes back turns later, exactly as Discover's does.
                // This one answers with an `Ask` of its own rather than with the
                // page's `Asked`, because a project page asks for two kinds of
                // thing now -- read me again, and put me somewhere.
                if let Some(asked) = state.update(message) {
                    return Some(asked);
                }
            }
            (Screen::Instance(state), Message::Instance(message)) => {
                // The one page that can ask for something other than a
                // navigation yet: an instance's Play and Stop are the shell's to
                // perform, for the same reason a search is.
                if let Some(asked) = state.update(message, store) {
                    return Some(asked);
                }
            }
            (Screen::Skins(state), Message::Skins(message)) => {
                // The third page that asks for something, and the first whose
                // question is about the *reader* rather than about a document: the
                // shell runs it and the answer comes back turns later, exactly as
                // Discover's and the project page's do. It answers with an `Ask` of
                // its own for the project page's reason: this page has two kinds of
                // request now -- read the account's appearance, and change what it
                // wears -- and only one of them is a question.
                if let Some(asked) = state.update(message) {
                    return Some(asked);
                }
            }
            // The profile page's two navigations, reported rather than applied:
            // a project row opens a project, and a filter tab moves the address the
            // list is filtered by. The page keeps nothing of either -- what it keeps
            // is the *answer*, which is why a filter moves it without a request.
            (Screen::User(_), Message::User(user::Message::Project(id))) => {
                return Some(Ask::Open(Open::Project(id)))
            }
            // A card's *Install*, reported rather than performed, and turned into
            // the same ask the project page's own button makes -- so a profile
            // installs a project by the one route rather than a second one.
            (Screen::User(_), Message::User(user::Message::Install(id, title, pack))) => {
                return Some(Ask::Install(project::Install { id, title, pack }))
            }
            (Screen::User(state), Message::User(user::Message::Filter(project_type))) => {
                return Some(Ask::Open(Open::User {
                    user: state.user.clone(),
                    project_type,
                }))
            }
            (Screen::User(state), Message::User(message)) => {
                if let Some(asked) = state.update(message) {
                    return Some(Ask::User(asked));
                }
            }
            (Screen::Screenshots(state), Message::Screenshots(message)) => state.update(message),
            (Screen::Servers(state), Message::Servers(message)) => state.update(message),
            // A message for a page that is not on screen is dropped rather than
            // applied to the wrong one: the shell routes by what it is showing.
            _ => {}
        }
        None
    }

    /// The request the page on screen owes because nothing has been asked for yet.
    ///
    /// `/browse/modpack` shows results rather than an invitation to press Search,
    /// which is what the reference does, so the shell asks on behalf of a page as
    /// soon as that page is drawn -- including the page a window opens on, which
    /// has never had a message of its own.
    pub fn opening(&mut self) -> Option<Ask> {
        match self {
            Screen::Discover(state) => state.opening(),
            Screen::Project(state) => state.opening().map(Ask::Project),
            // The page a window can open straight on -- `/skins` is on the rail --
            // so a reader who never presses anything still gets their own skin.
            Screen::Skins(state) => state.opening().map(Ask::Skins),
            // A profile is a document the reader arrived at, so it is owed on
            // arrival the same way a project's is: `/user/jelly` is a name and a
            // list this page has nothing of until it asks.
            Screen::User(state) => state.opening().map(Ask::User),
            // An instance's tab is a listing rather than a document: what the
            // reader arrived at is a folder, and the page has nothing of it until
            // it asks. A tab change lands here too -- `retarget` marks the listing
            // idle and the shell asks on the same turn -- which is why this is the
            // page's own `opening` rather than a request the tab press sends.
            Screen::Instance(state) => state.opening().map(Ask::Instance),
            _ => None,
        }
    }

    /// The Skins page's editor, when one is open: the choices being made, the capes
    /// they can be made from, and whether a write is already out.
    ///
    /// The shell draws the modal layer, but the editor is the *page's* state rather
    /// than the shell's: a `Modal` variant holding a copy of it would be a second
    /// fact to keep in step with [`skins::State::edit`], and the two would drift the
    /// first time one of them was updated without the other. So the shell asks the
    /// page for it, one frame at a time, and wraps what it gets back in its own
    /// dialog frame ([`crate::shell`]).
    pub fn skins_edit(&self) -> Option<(&skins::Edit, &[palantir_net::MinecraftCape], bool)> {
        match self {
            Screen::Skins(state) => state.edit.as_ref().map(|edit| {
                // The capes are the account's own, from the same read the rows are
                // drawn from: an editor opened before that answer arrived has none to
                // offer, which is the reference's own state as its list loads.
                let capes = state
                    .appearance
                    .ready()
                    .map(|appearance| appearance.capes.as_slice())
                    .unwrap_or(&[]);
                (edit, capes, state.wearing)
            }),
            _ => None,
        }
    }

    /// The tab an instance page is showing.
    ///
    /// `cfg(test)` because nothing in the interface needs it: an instance's tab
    /// is in the address, so a caller that wants to know has the route already.
    /// This exists so the retarget gate can see what a tab change kept, which is
    /// a question only a test asks.
    #[cfg(test)]
    pub fn instance_tab(&self) -> Option<crate::route::InstanceTab> {
        match self {
            Screen::Instance(state) => Some(state.tab.clone()),
            _ => None,
        }
    }

    /// The project tab a project page is showing, for the same reason.
    #[cfg(test)]
    pub fn project_tab(&self) -> Option<crate::route::ProjectTab> {
        match self {
            Screen::Project(state) => Some(state.tab.clone()),
            _ => None,
        }
    }

    /// What a project page is saying about itself, for the same reason.
    ///
    /// The sentence an install leaves is the shell's to *deliver* and the page's
    /// to keep, so the gate that checks the delivery has to be able to read it
    /// here.
    #[cfg(test)]
    pub fn project_notice(&self) -> Option<&str> {
        match self {
            Screen::Project(state) => state.notice.as_deref(),
            _ => None,
        }
    }

    /// Draw the page on screen.
    ///
    /// No address: every page is built with what its address said when it was
    /// built, and a page that read the route as it drew could show one thing while
    /// [`Screen::retarget`] believed another. The profile page was the last caller
    /// that needed one, for the name in its header, and it keeps the name in its own
    /// state now -- because it is also what the page *asks* by, and `update` is
    /// handed no address to ask with.
    pub fn view<'a>(&'a self, theme: Gen, store: &'a Store) -> Element<'a, Message> {
        match self {
            Screen::Home(state) => home::view(theme, state, store).map(Message::Home),
            Screen::Discover(state) => discover::view(theme, state, store).map(Message::Discover),
            Screen::Project(state) => project::view(theme, state, store).map(Message::Project),
            Screen::Instance(state) => instance::view(theme, state, store).map(Message::Instance),
            Screen::Skins(state) => skins::view(theme, state, store).map(Message::Skins),
            Screen::Screenshots(state) => {
                screenshots::view(theme, state, store).map(Message::Screenshots)
            }
            Screen::Servers(state) => servers::view(theme, state, store).map(Message::Servers),
            Screen::User(state) => user::view(theme, state, store).map(Message::User),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::Load;
    use crate::route::{InstanceTab, ProfileTab, ProjectTab, ProjectType, ServerTab};


    /// Every address the route table has a shape for.
    fn sample_addresses() -> Vec<&'static str> {
        vec![
            "/",
            "/browse/modpack",
            "/browse/mod",
            "/browse/resourcepack",
            "/browse/shader",
            "/browse/datapack",
            "/browse/server",
            // `description` is the empty tab token, so the bare path is that
            // page: the route table has no `/description`.
            "/project/sodium",
            "/project/sodium/versions",
            "/project/sodium/version/abc",
            "/project/sodium/gallery",
            "/instance/atm10",
            "/instance/atm10/projects/mod",
            "/instance/atm10/files",
            "/instance/atm10/worlds",
            "/instance/atm10/screenshots",
            "/instance/atm10/logs",
            "/instance/atm10/share",
            "/skins",
            "/screenshots",
            "/hosting/manage/",
            "/user/jelly",
        ]
    }

    #[test]
    fn every_route_builds_a_page_and_that_page_draws() {
        // The gate that makes "there is a page for every route" a fact rather than
        // an intention: an address the route table knows and the dispatcher does
        // not would be a blank pane.
        for path in sample_addresses() {
            let address = Address::parse(path).unwrap_or_else(|| panic!("{path} is not a route"));
            let mut screen = Screen::at(&address);
            let store = Store::default();
            for theme in Gen::ALL {
                drop(screen.view(*theme, &store));
            }
            // And every page survives being pointed at its own address again.
            screen.retarget(&address);
            drop(screen.view(Gen::Dark, &store));
        }
    }

    #[test]
    fn a_tab_change_keeps_the_page_and_a_different_thing_does_not() {
        let store = Store::default();
        let atm = Address::parse("/instance/atm10/files").expect("an instance");
        let other = Address::parse("/instance/sodium/files").expect("another");
        let mut screen = Screen::at(&atm);
        assert_eq!(screen.instance_tab(), Some(InstanceTab::Files));
        // Another tab, same instance: the page is the one already there.
        let worlds = Address::parse("/instance/atm10/worlds").expect("a tab");
        screen.retarget(&worlds);
        assert_eq!(screen.instance_tab(), Some(InstanceTab::Worlds));
        // Something else: a new page.
        screen.retarget(&other);
        match &screen {
            Screen::Instance(state) => {
                assert_eq!(state.id, "sodium");
                assert_eq!(state.tab, InstanceTab::Files);
            }
            other => panic!("{other:?} is not an instance page"),
        }
        // A project's tab behaves the same way.
        let project = Address::parse("/project/sodium/gallery").expect("a project");
        let mut screen = Screen::at(&project);
        assert_eq!(screen.project_tab(), Some(ProjectTab::Gallery));
        screen.retarget(&Address::parse("/project/sodium/versions").expect("a tab"));
        assert_eq!(screen.project_tab(), Some(ProjectTab::Versions));
        screen.retarget(&Address::parse("/project/lithium/versions").expect("another"));
        match &screen {
            Screen::Project(state) => assert_eq!(state.id, "lithium"),
            other => panic!("{other:?} is not a project page"),
        }
        // And Discover's tab is a project type, not a new page.
        let mut screen = Screen::at(&Address::parse("/browse/modpack").expect("browse"));
        screen.retarget(&Address::parse("/browse/shader").expect("browse"));
        match &screen {
            Screen::Discover(state) => assert_eq!(state.project_type, ProjectType::Shader),
            other => panic!("{other:?} is not discover"),
        }
        drop(store);
    }

    #[test]
    fn a_message_for_a_page_that_is_not_on_screen_is_dropped() {
        // The shell routes a page's messages through this, so a stray one must not
        // land on the wrong page: it would silently change state nobody is looking
        // at, which is worse than being ignored.
        let store = Store::default();
        let address = Address::parse("/skins").expect("skins");
        let mut screen = Screen::at(&address);
        // The whole set of open sections, printed: the page's own state is not
        // `PartialEq` beyond the set, and what this test is about is that it did
        // not move.
        let read = |screen: &Screen| match screen {
            Screen::Skins(state) => format!("{:?}", state.open),
            other => format!("{other:?}"),
        };
        let before = read(&screen);
        assert_eq!(screen.update(Message::Home(home::Message::Search("x".to_string())), &store), None);
        let after = read(&screen);
        assert_eq!(before, after);
        // And the page's own message does land: its header closed the section it
        // named, which every section starts open (the reference's own first pass).
        assert_eq!(
            screen.update(Message::Skins(skins::Message::Select(skins::Section::Modrinth)), &store),
            None
        );
        match &screen {
            Screen::Skins(state) => assert!(!state.open.is_open(skins::Section::Modrinth)),
            other => panic!("{other:?} is not skins"),
        }
    }

    #[test]
    fn opening_a_card_is_reported_to_the_shell_rather_than_applied_by_the_page() {
        // The two navigations a page can ask for, and the fact that asking does
        // not change the page: only the shell owns the history.
        let store = Store::default();
        let mut home = Screen::at(&Address::parse("/").expect("home"));
        assert_eq!(
            home.update(Message::Home(home::Message::Open("atm10".to_string())), &store),
            Some(Ask::Open(Open::Instance("atm10".to_string())))
        );
        assert!(matches!(home, Screen::Home(_)), "the page is still the library");
        let mut discover = Screen::at(&Address::parse("/browse/modpack").expect("browse"));
        assert_eq!(
            discover.update(
                Message::Discover(discover::Message::Open("sodium".to_string())),
                &store
            ),
            Some(Ask::Open(Open::Project("sodium".to_string())))
        );
        // And on the wrong page the same message is not a navigation.
        let mut skins = Screen::at(&Address::parse("/skins").expect("skins"));
        assert_eq!(
            skins.update(Message::Home(home::Message::Open("atm10".to_string())), &store),
            None
        );
    }

    #[test]
    fn a_project_page_asks_for_its_own_documents_on_arrival() {
        // The second page that owes a request. `/project/sodium` is a document,
        // a team and a version list, and the page is one `Load`, so the shell is
        // asked once and the answer carries all three back.
        let store = Store::default();
        let mut screen = Screen::at(&Address::parse("/project/sodium").expect("a project page"));
        let Some(Ask::Project(first)) = screen.opening() else {
            panic!("a freshly drawn project page owes a request");
        };
        assert_eq!(first.id, "sodium");
        assert_eq!(first.round, 1);
        assert_eq!(screen.opening(), None, "asked once, and the refresh button is what asks again");
        // The answer comes back as a message of the page's own, built by the shell
        // and never named here.
        let message = Message::project_result(&first, Err("no such project".to_string()));
        screen.update(message, &store);
        let Screen::Project(state) = &screen else {
            panic!("still the project page");
        };
        assert_eq!(state.project.failure(), Some("no such project"));
    }

    #[test]
    fn a_search_is_reported_to_the_shell_and_the_answer_comes_back_as_a_message() {
        // The other half of what a page can ask for: a request goes out of
        // `update`, and its answer arrives as a message from the shell. Neither
        // end of that is the shell's to name.
        let store = Store::default();
        let mut screen = Screen::at(&Address::parse("/browse/modpack").expect("browse"));
        // A page that has been drawn owes its first request before anything is
        // pressed -- and the first is the tag list, because the sidebar's
        // filter sections are made of it and the shell hands back one ask at
        // a time.
        assert_eq!(screen.opening(), Some(Ask::Tags), "the tag list comes first");
        let Some(Ask::Search(first)) = screen.opening() else {
            panic!("and then the search");
        };
        assert_eq!(first.round, 1);
        // Asked once, and asking again is the button's.
        assert_eq!(screen.opening(), None);
        let Some(Ask::Search(second)) =
            screen.update(Message::Discover(discover::Message::Search), &store)
        else {
            panic!("the Search button asks");
        };
        assert_eq!(second.round, 2);
        // The answer is routed by the shell and lands on the page that asked.
        let hits = vec![discover::Hit {
            id: "AANobbMI".to_string(),
            title: "Sodium".to_string(),
            author: "jellysquid".to_string(),
            summary: "Modern rendering engine".to_string(),
            downloads: 1,
            follows: 1,
            // No icon: this test is about the request being routed, and the card's
            // icon is a request of its own (`Message::search_icons`).
            icon_url: String::new(),
            game_versions: Vec::new(),
            loaders: Vec::new(),
        }];
        let answer = Message::search_result(&second, Ok(hits.clone()));
        assert_eq!(screen.update(answer, &store), None);
        match &screen {
            Screen::Discover(state) => assert_eq!(state.results, Load::Ready(hits)),
            other => panic!("{other:?} is not discover"),
        }
        // And the same request on a page that is not Discover is not a request.
        // The Skins page owes one of its own now (G104), which is a *different*
        // ask rather than this one -- and a Discover message is still dropped
        // there rather than applied.
        let mut skins = Screen::at(&Address::parse("/skins").expect("skins"));
        assert!(matches!(skins.opening(), Some(Ask::Skins(_))));
        assert_eq!(
            skins.update(Message::Discover(discover::Message::Search), &store),
            None,
            "a message meant for another page is not a request from this one"
        );
    }

    #[test]
    fn a_change_to_what_is_worn_leaves_the_page_and_its_answer_comes_back_to_it() {
        // The one write a page can ask for: the *change* is the page's value, the
        // request is the shell's, and the outcome comes back as the page's own
        // message built here rather than named by the shell.
        let store = Store::default();
        let mut skins = Screen::at(&Address::parse("/skins").expect("skins"));
        let change = palantir_net::SkinChange::Cape { id: "cape-1".to_string() };
        let Some(Ask::Wear(worn)) =
            skins.update(Message::Skins(skins::Message::Wear(change.clone())), &store)
        else {
            panic!("the press leaves the page as a request for the shell");
        };
        assert_eq!(worn.change, change);
        // A refusal lands on the page that asked, as a sentence, and the page is
        // ready to press again.
        assert_eq!(skins.update(Message::skin_worn(&worn, Err("no".to_string())), &store), None);
        let Screen::Skins(state) = &skins else {
            panic!("still the skins page");
        };
        assert_eq!(state.notice.as_deref(), Some("no"));
        assert!(!state.wearing);
    }

    #[test]
    fn a_profile_page_asks_for_the_user_its_address_names() {
        // The third page that owes a request on arrival, and the first whose
        // question is about somebody else: `/user/jelly` is a name this page has
        // nothing of until it asks.
        let store = Store::default();
        let mut screen =
            Screen::at(&Address::parse("/user/jelly").expect("a profile page"));
        let Some(Ask::User(first)) = screen.opening() else {
            panic!("a freshly drawn profile page owes a request");
        };
        assert_eq!(first.user, "jelly");
        assert_eq!(first.round, 1);
        assert_eq!(screen.opening(), None, "asked once, and the refresh button is what asks again");
        // The answer comes back as a message of the page's own, built here and
        // never named by the shell.
        let message = Message::user_result(&first, Err("no such user".to_string()));
        assert_eq!(screen.update(message, &store), None);
        match &screen {
            Screen::User(state) => assert_eq!(state.profile.failure(), Some("no such user")),
            other => panic!("{other:?} is not a profile page"),
        }
    }

    #[test]
    fn a_profile_s_two_navigations_are_reported_rather_than_applied() {
        // A project row opens a project, and a filter tab moves the *address* the
        // list is filtered by -- which is what makes the strip's links and the
        // route agree about what is on screen. Neither press changes the page here:
        // what the page then keeps is the answer, and the address is the shell's.
        let store = Store::default();
        let mut screen = Screen::at(&Address::parse("/user/jelly").expect("a profile page"));
        assert_eq!(
            screen.update(
                Message::User(user::Message::Project("AANobbMI".to_string())),
                &store
            ),
            Some(Ask::Open(Open::Project("AANobbMI".to_string())))
        );
        assert_eq!(
            screen.update(
                Message::User(user::Message::Filter(Some(ProfileTab::Projects(
                    ProjectType::Mod
                )))),
                &store
            ),
            Some(Ask::Open(Open::User {
                user: "jelly".to_string(),
                project_type: Some(ProfileTab::Projects(ProjectType::Mod)),
            }))
        );
        // The strip's fourth tab is the same report, and `collections` is the
        // segment the shell writes for it -- one navigation for all four tabs.
        assert_eq!(
            screen.update(
                Message::User(user::Message::Filter(Some(ProfileTab::Collections))),
                &store
            ),
            Some(Ask::Open(Open::User {
                user: "jelly".to_string(),
                project_type: Some(ProfileTab::Collections),
            }))
        );
        // The whole strip, back to everything.
        assert_eq!(
            screen.update(Message::User(user::Message::Filter(None)), &store),
            Some(Ask::Open(Open::User { user: "jelly".to_string(), project_type: None }))
        );
        // And the page still believes the address it was built for, because
        // following the move is `retarget`'s job rather than `update`'s.
        match &screen {
            Screen::User(state) => {
                assert_eq!(state.user, "jelly");
                assert_eq!(state.project_type, None);
            }
            other => panic!("{other:?} is not a profile page"),
        }
    }

    #[test]
    fn a_filter_moves_the_profile_page_and_another_user_rebuilds_it() {
        // The profile page's half of the rule the instance page states below: a
        // change of *question* keeps the page, and a change of *user* is a new one.
        let mut screen = Screen::at(&Address::parse("/user/jelly").expect("a profile page"));
        screen.retarget(&Address::parse("/user/jelly/mods").expect("a filter"));
        match &screen {
            Screen::User(state) => {
                assert_eq!(state.user, "jelly");
                assert_eq!(state.project_type, Some(ProjectType::Mod));
            }
            other => panic!("{other:?} is not a profile page"),
        }
        screen.retarget(&Address::parse("/user/jellysquid3").expect("another user"));
        match &screen {
            Screen::User(state) => {
                assert_eq!(state.user, "jellysquid3");
                assert_eq!(state.project_type, None);
            }
            other => panic!("{other:?} is not a profile page"),
        }
    }

    #[test]
    fn the_two_routes_with_a_context_still_land_on_their_own_page() {
        // `/hosting/manage/:id/:tab` is the same page as the listing, and a project
        // with no tab is Description rather than nothing.
        let server = Address::at(Route::Server { id: "srv".to_string(), tab: ServerTab::Access });
        assert!(matches!(Screen::at(&server), Screen::Servers(_)));
        let project = Address::at(Route::Project { id: "sodium".to_string(), tab: ProjectTab::Description });
        assert_eq!(Screen::at(&project).project_tab(), Some(ProjectTab::Description));
    }
}
