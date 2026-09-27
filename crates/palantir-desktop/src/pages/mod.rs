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
    /// Ask the store, through the engine, for one project: its own document, the
    /// people on its team, and its version list.
    ///
    /// One request rather than three because a project page draws all three at
    /// once, and three `Load`s for one page is three ways to be half drawn.
    Project(project::Asked),
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
            Route::User { .. } => Screen::User(user::State::default()),
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
                // The one page that asks for anything yet. The request it reports
                // is a value: the shell runs it, and the answer comes back as a
                // message rather than as a return value, because an answer
                // arrives turns later.
                if let Some(asked) = state.update(message) {
                    return Some(Ask::Search(asked));
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
            (Screen::Skins(state), Message::Skins(message)) => state.update(message),
            (Screen::Screenshots(state), Message::Screenshots(message)) => state.update(message),
            (Screen::Servers(state), Message::Servers(message)) => state.update(message),
            (Screen::User(state), Message::User(message)) => state.update(message),
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
            Screen::Discover(state) => state.opening().map(Ask::Search),
            Screen::Project(state) => state.opening().map(Ask::Project),
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
    pub fn view<'a>(
        &'a self,
        theme: Gen,
        address: &'a Address,
        store: &'a Store,
    ) -> Element<'a, Message> {
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
            Screen::User(state) => {
                let name = match &address.route {
                    Route::User { user, .. } => user.as_str(),
                    _ => "",
                };
                user::view(theme, state, store, name).map(Message::User)
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::Load;
    use crate::route::{InstanceTab, ProjectTab, ProjectType, ServerTab};

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
                drop(screen.view(*theme, &address, &store));
            }
            // And every page survives being pointed at its own address again.
            screen.retarget(&address);
            drop(screen.view(Gen::Dark, &address, &store));
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
        let before = format!("{:?}", match &screen {
            Screen::Skins(state) => state.open,
            _ => None,
        });
        assert_eq!(screen.update(Message::Home(home::Message::Search("x".to_string())), &store), None);
        let after = format!("{:?}", match &screen {
            Screen::Skins(state) => state.open,
            _ => None,
        });
        assert_eq!(before, after);
        // And the page's own message does land.
        assert_eq!(
            screen.update(Message::Skins(skins::Message::Select(skins::Section::Modrinth)), &store),
            None
        );
        match &screen {
            Screen::Skins(state) => assert!(state.open.is_some()),
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
        // pressed.
        let Some(Ask::Search(first)) = screen.opening() else {
            panic!("a freshly drawn Discover page owes a request");
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
        let mut skins = Screen::at(&Address::parse("/skins").expect("skins"));
        assert_eq!(skins.opening(), None);
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
