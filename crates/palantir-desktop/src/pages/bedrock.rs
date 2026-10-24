//! The Bedrock shelf: the edition this launcher does not install.
//!
//! A shelf beside the Java one, and nothing more. `crate::bedrock` already answers
//! the three questions a reader has about the copy they own -- is it here, what
//! worlds do I have, which packs are installed -- and this page draws the answers.
//! It never writes: no download, no version picker, no file moved. The Store
//! package is licensed per account, so a launcher that fetched it would be
//! redistributing somebody's licence; the one action here opens the Store.
//!
//! **A page, not a section of Home**, because the two editions do not share a
//! shelf: a Bedrock world is not a Java instance and cannot be launched by the
//! same machinery, and a list that mixed them would be a list where half the rows
//! have no Play button. The rail entry is what makes it a place.
//!
//! **The scan is a request**, like every other read in this tree: a directory walk
//! on the frame thread is the defect `crate::scale` measured for the Files tab, so
//! the shell runs [`scan`] off-thread once and the page draws what came back. The
//! page asks on arrival ([`State::opening`]) and never again -- a shelf that
//! re-read the disk every frame would also be wrong, because the game may be
//! running beside it.

use iced::widget::{column, row, Space};
use iced::{Alignment, Element, Length};

use crate::bedrock::{self, Pack, PackKind, Presence, World};
use crate::page::{self, Load, GAP};
use crate::pages::{Ask, Open};
use crate::scroll;
use crate::style::{heading, medium, regular, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::theme_gen::{self, Theme as Gen};
use crate::ui;

/// Everything one visit reads off the machine.
///
/// One value rather than four reads, because they are one answer about one
/// folder: a page that could draw a world list from a different scan than its
/// pack list would be showing two machines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scan {
    /// What the machine has.
    pub presence: Presence,
    /// The worlds in `games/com.mojang/worlds`.
    pub worlds: Vec<World>,
    /// The resource packs in `games/com.mojang/resource_packs`.
    pub resource: Vec<Pack>,
    /// The behaviour packs in `games/com.mojang/behavior_packs`.
    pub behaviour: Vec<Pack>,
}

/// Read the machine's Bedrock data. **Blocking**: a directory walk, off-thread.
///
/// Nothing here can fail in a way worth reporting: a missing folder is an empty
/// list (the module says so), and the presence is the answer to "where is it"
/// rather than an error about it.
pub fn scan() -> Scan {
    let presence = bedrock::presence();
    // The root is copied out before `presence` moves into the answer: `root()`
    // borrows what it is called on, and a `Scan` that owns the presence cannot be
    // built while that borrow is alive.
    let root = presence.root().map(|root| root.to_path_buf());
    match root {
        Some(root) => Scan {
            presence,
            worlds: bedrock::worlds(&root),
            resource: bedrock::packs(&root, PackKind::Resource),
            behaviour: bedrock::packs(&root, PackKind::Behavior),
        },
        None => Scan {
            presence,
            worlds: Vec::new(),
            resource: Vec::new(),
            behaviour: Vec::new(),
        },
    }
}

/// What the Bedrock page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// The scan the shell ran, back.
    Scanned(Result<Scan, String>),
    /// Open the Store's page for the game.
    ///
    /// The page's only action, and it is a navigation rather than a download:
    /// see the module documentation.
    OpenStore,
    /// A wheel over this page's scroll region. The shell's policy moves it.
    Wheel(&'static str, scroll::Wheel),
    /// A pointer crossing, recorded into the interaction clock.
    Hover {
        /// The control's stable name, one per control.
        key: &'static str,
        /// Whether the pointer arrived or left.
        over: bool,
        /// The hover end, where the control declares one of its own.
        hover: Option<f32>,
    },
}

crate::hovered!(Message);

/// The Bedrock page's own state: what the scan said, and nothing else.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// The scan, once it has run.
    scan: Load<Scan>,
}

impl State {
    /// A page that has not asked yet.
    pub fn new() -> State {
        State::default()
    }

    /// Apply a message, and report anything only the shell can do.
    pub fn update(&mut self, message: Message) -> Option<Ask> {
        match message {
            Message::Scanned(scan) => {
                self.scan = match scan {
                    Ok(scan) => Load::Ready(scan),
                    Err(reason) => Load::Failed(reason),
                };
            }
            // The Store link is a navigation, and navigations are the shell's:
            // this is the same door the news feed's links use.
            Message::OpenStore => {
                return Some(Ask::Open(Open::Url(bedrock::STORE_URL.to_string())));
            }
            // The wheel is applied by `crate::scroll`, from the report the region
            // makes; there is nothing for the page to do with it.
            Message::Wheel(..) => {}
            // The crossing is recorded by the shell's clock through the `Hovered`
            // impl `crate::hovered!` generates; the page has no state for it.
            Message::Hover { .. } => {}
        }
        None
    }

    /// The scan this page owes because it has never run one.
    pub fn opening(&mut self) -> Option<Ask> {
        if self.scan == Load::Idle {
            self.scan = Load::Loading;
            return Some(Ask::Bedrock);
        }
        None
    }

    /// The page, drawn from the scan.
    pub fn view(&self, theme: Gen) -> Element<'_, Message> {
        let mut blocks: Vec<Element<'_, Message>> = vec![title(theme)];
        match &self.scan {
            Load::Idle | Load::Loading => {
                blocks.push(sentence(theme, "Looking for Bedrock on this machine…"));
            }
            Load::Ready(scan) => {
                blocks.push(presence_block(theme, &scan.presence));
                match scan.presence.root() {
                    Some(root) => {
                        blocks.push(worlds_block(theme, &scan.worlds, root));
                        blocks.push(packs_block(
                            theme,
                            "Resource packs",
                            &scan.resource,
                            "resource_packs",
                            root,
                        ));
                        blocks.push(packs_block(
                            theme,
                            "Behaviour packs",
                            &scan.behaviour,
                            "behavior_packs",
                            root,
                        ));
                    }
                    // Without a folder there is nothing behind the three lists,
                    // and three empty states stacked under a Store button would
                    // read as three failures rather than one absence.
                    None => {}
                }
            }
            Load::Failed(reason) => blocks.push(sentence(theme, reason)),
            Load::Empty => {
                blocks.push(sentence(
                    theme,
                    "No Bedrock data was found on this machine.",
                ));
            }
        }
        page::body(blocks, GAP, Message::Wheel)
    }
}

/// The page's heading. A literal rather than a locale key: the reference's table
/// has no entry for a page it does not have.
fn title(theme: Gen) -> Element<'static, Message> {
    ui::text("Bedrock".to_string())
        .size(24.0)
        .font(heading())
        .style(iced::theme::Text::Color(theme_gen::ink(
            theme,
            INK_CONTRAST,
        )))
        .into()
}

/// One sentence of body text in the secondary ink.
fn sentence<'a>(theme: Gen, text: impl ToString) -> Element<'a, Message> {
    ui::text(text)
        .size(14.0)
        .font(regular())
        .style(iced::theme::Text::Color(theme_gen::ink(
            theme,
            INK_SECONDARY,
        )))
        .into()
}

/// A heading with a count beside it, for the three lists.
fn heading_line<'a>(theme: Gen, text: &str, count: Option<usize>) -> Element<'a, Message> {
    row![
        ui::text(text.to_string())
            .size(16.0)
            .font(semibold())
            .style(iced::theme::Text::Color(theme_gen::ink(
                theme,
                INK_CONTRAST
            ))),
        ui::text(count.unwrap_or_default().to_string())
            .size(14.0)
            .font(medium())
            .style(iced::theme::Text::Color(theme_gen::ink(
                theme,
                INK_SECONDARY
            ))),
    ]
    .spacing(8.0)
    .align_items(Alignment::Center)
    .into()
}

/// What this machine has, as the one card that answers it.
fn presence_block<'a>(theme: Gen, presence: &Presence) -> Element<'a, Message> {
    match presence {
        Presence::Installed { root } => ui::card(
            theme,
            column![]
                .spacing(4.0)
                .push(heading_line(theme, "Installed", None))
                .push(sentence(theme, "Bedrock's data folder:"))
                .push(
                    ui::text(root.display().to_string())
                        .size(14.0)
                        .font(regular())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
                ),
        ),
        Presence::NotInstalled => ui::card(
            theme,
            column![]
                .spacing(8.0)
                .push(heading_line(theme, "Bedrock is not installed", None))
                .push(sentence(
                    theme,
                    "This launcher does not install it: the Store package is licensed to \
                     your account, so the Store is where it comes from. Once it has run \
                     once, its worlds and packs show up here.",
                ))
                .push(
                    row![
                        ui::button_text_sized(
                            theme,
                            "bedrock:store",
                            "Open the Store",
                            ui::Kind::Colored,
                            ui::Size::Md,
                            Message::OpenStore,
                        ),
                        Space::with_width(Length::Fill),
                    ]
                    .align_items(Alignment::Center),
                ),
        ),
        Presence::Unsupported => ui::card(
            theme,
            column![]
                .spacing(4.0)
                .push(heading_line(theme, "Bedrock is Windows-only", None))
                .push(sentence(
                    theme,
                    "This is the pocket edition's home platform; on this machine there is \
                     nothing to list.",
                )),
        ),
        Presence::Unknown { reason } => ui::card(
            theme,
            column![]
                .spacing(4.0)
                .push(heading_line(theme, "The folder could not be located", None))
                .push(sentence(theme, reason.as_str())),
        ),
    }
}

/// The worlds list, or the sentence that says where they would appear.
fn worlds_block<'a>(theme: Gen, worlds: &[World], root: &std::path::Path) -> Element<'a, Message> {
    let mut card = column![].spacing(8.0);
    card = card.push(heading_line(theme, "Worlds", Some(worlds.len())));
    if worlds.is_empty() {
        card = card.push(sentence(
            theme,
            format!(
                "No worlds yet. They appear here from {}.",
                root.join("worlds").display()
            ),
        ));
    }
    for world in worlds {
        card = card.push(row_row(theme, world.label(), world.has_icon));
    }
    ui::card(theme, card)
}

/// One list of packs, or the sentence naming the folder it would be read from.
fn packs_block<'a>(
    theme: Gen,
    title: &str,
    packs: &[Pack],
    folder: &str,
    root: &std::path::Path,
) -> Element<'a, Message> {
    let mut card = column![].spacing(8.0);
    card = card.push(heading_line(theme, title, Some(packs.len())));
    if packs.is_empty() {
        card = card.push(sentence(
            theme,
            format!(
                "Nothing installed. Packs live in {}.",
                root.join(folder).display()
            ),
        ));
    }
    for pack in packs {
        card = card.push(row_row(theme, pack.label(), true));
    }
    ui::card(theme, card)
}

/// One row of a list: its name, and whether the folder carried a picture.
///
/// A shelf and not a table: the reference has no Bedrock page, so the row is this
/// launcher's own, drawn in the same card and spacing vocabulary as the rest of
/// the shell rather than in a shape nothing has measured.
fn row_row<'a>(theme: Gen, label: &str, has_picture: bool) -> Element<'a, Message> {
    let mut line = row![].spacing(8.0).align_items(Alignment::Center).push(
        ui::text(label.to_string())
            .size(14.0)
            .font(medium())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
    );
    if has_picture {
        line = line.push(Space::with_width(Length::Fill)).push(
            ui::text("picture".to_string())
                .size(12.0)
                .font(regular())
                .style(iced::theme::Text::Color(theme_gen::ink(
                    theme,
                    INK_SECONDARY,
                ))),
        );
    }
    line.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scan with nothing installed, which is the state a Linux test machine is
    /// in and the state a fresh Windows one shows.
    fn bare() -> Scan {
        Scan {
            presence: Presence::NotInstalled,
            worlds: Vec::new(),
            resource: Vec::new(),
            behaviour: Vec::new(),
        }
    }

    #[test]
    fn the_page_asks_for_its_scan_once_and_only_from_idle() {
        let mut state = State::new();
        assert_eq!(state.opening(), Some(Ask::Bedrock), "a fresh page asks");
        assert_eq!(
            state.opening(),
            None,
            "and does not ask again while waiting"
        );
        let _ = state.update(Message::Scanned(Ok(bare())));
        assert_eq!(state.opening(), None, "nor once the answer is in");
    }

    #[test]
    fn the_store_button_is_a_navigation_to_the_store() {
        let mut state = State::new();
        assert_eq!(
            state.update(Message::OpenStore),
            Some(Ask::Open(Open::Url(bedrock::STORE_URL.to_string()))),
            "the one action this page has is opening the Store"
        );
    }

    #[test]
    fn the_page_draws_in_every_theme_in_every_state() {
        for theme in Gen::ALL {
            let mut state = State::new();
            drop(state.view(*theme));
            let _ = state.update(Message::Scanned(Ok(bare())));
            drop(state.view(*theme));
            let _ = state.update(Message::Scanned(Ok(Scan {
                presence: Presence::Unsupported,
                ..bare()
            })));
            drop(state.view(*theme));
            let _ = state.update(Message::Scanned(Ok(Scan {
                presence: Presence::Installed {
                    root: std::path::PathBuf::from("/nowhere/games/com.mojang"),
                },
                ..bare()
            })));
            drop(state.view(*theme));
        }
    }

    /// The page never writes: no message it accepts can reach a filesystem call
    /// that writes, and the only action is a URL. Pinned as source because a
    /// later "install this pack" button would look like an improvement.
    #[test]
    fn the_page_only_reads_and_never_writes() {
        // The page's own code, without this module: the forbidden names are
        // written *in* this test, and a scan of the whole file finds itself.
        let source = include_str!("bedrock.rs");
        let page = source
            .split("#[cfg(test)]")
            .next()
            .expect("a file with a body");
        for forbidden in ["fs::write", "remove_file", "create_dir", "OpenOptions"] {
            assert!(
                !page.contains(forbidden),
                "the Bedrock page must not write: found {forbidden}"
            );
        }
    }
}
