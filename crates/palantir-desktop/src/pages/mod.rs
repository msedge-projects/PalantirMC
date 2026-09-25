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

/// Something a page asked for that only the shell can do.
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
                    state.update(instance::Message::Tab(tab.clone()), &Store::default());
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
    /// or a listing is a request the store makes.
    pub fn update(&mut self, message: Message, store: &Store) -> Option<Open> {
        match (self, message) {
            // A card press is navigation, and is reported rather than applied: the
            // page does not know what else has to change when the pane does.
            (Screen::Home(_), Message::Home(home::Message::Open(id))) => {
                return Some(Open::Instance(id))
            }
            (Screen::Discover(_), Message::Discover(discover::Message::Open(id))) => {
                return Some(Open::Project(id))
            }
            (Screen::Home(state), Message::Home(message)) => state.update(message),
            (Screen::Discover(state), Message::Discover(message)) => state.update(message),
            (Screen::Project(state), Message::Project(message)) => state.update(message),
            (Screen::Instance(state), Message::Instance(message)) => state.update(message, store),
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
            Some(Open::Instance("atm10".to_string()))
        );
        assert!(matches!(home, Screen::Home(_)), "the page is still the library");
        let mut discover = Screen::at(&Address::parse("/browse/modpack").expect("browse"));
        assert_eq!(
            discover.update(
                Message::Discover(discover::Message::Open("sodium".to_string())),
                &store
            ),
            Some(Open::Project("sodium".to_string()))
        );
        // And on the wrong page the same message is not a navigation.
        let mut skins = Screen::at(&Address::parse("/skins").expect("skins"));
        assert_eq!(
            skins.update(Message::Home(home::Message::Open("atm10".to_string())), &store),
            None
        );
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
