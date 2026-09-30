//! Skins, the reference's third rail slot: `pages/Skins.vue`.
//!
//! The page is a gallery of *sections* -- the bundles the reference ships, whose
//! names are its own keys -- over a preview of the skin in force. Both halves need
//! a service: the bundles come from Modrinth's skin store and the preview from a
//! rendered skin, so what this page draws today is the section list and the states
//! around it, with the sentence that says where the rest comes from rather than a
//! grid of empty boxes.
//!
//! What *is* real is the account's own half: the skins and capes Minecraft says it
//! owns, the skin in force cut into a front view (G104), the ability to put any of
//! them on (G106), and -- since G123 -- the ability to add one from a file: the
//! header's Add opens the launcher's own file dialog, the chosen texture is padded to
//! the 64x64 the service takes and its arm style is read from its own pixels, and the
//! upload is one multipart write (`crate::pick`, `crate::skin`, `Ask::AddSkin`).
//!
//! Three of those are a *write* to the reader's own Minecraft account -- putting a skin
//! on, putting a cape on, and adding the file -- so what a press will do is written
//! where the press is. Two things the reference has are still not here, and this page
//! says so rather than drawing a control that lies: its edit modal, which is where its
//! own `unequip_skin` is reached from (the client *can* take a skin off -- see
//! `palantir_net::SkinChange` -- and no control asks for it), and the store of the
//! skins a reader has added, which its Saved skins sections are drawn from.

use iced::widget::{column, image, row, text, Space};
use iced::{Alignment, Element, Length};
use palantir_net::SkinChange;

use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP, ROW_GAP};
use crate::pages::Ask;
use crate::skin::Appearance;
use crate::store::Store;
use crate::style::{INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Ink, Theme as Gen};
use crate::ui;

/// The reference's bundled sections, in its own order.
///
/// `Skins.vue` draws these from a constant list of skin packs; the names here are
/// the locale's, so the section headings are the reference's words rather than a
/// paraphrase of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// The skins a new account starts with.
    DefaultSkins,
    /// Modrinth's own.
    Modrinth,
    /// The pride bundle.
    ModrinthPride,
    /// The user's own saved skins.
    SavedSkins,
    /// Builders and Biomes.
    BuildersAndBiomes,
    /// Chaos Cubed.
    ChaosCubed,
    /// Chase the Skies.
    ChaseTheSkies,
    /// Minecon Earth 2017.
    MineconEarth2017,
    /// Mounts of Mayhem.
    MountsOfMayhem,
    /// Striding Hero.
    StridingHero,
    /// The Copper Age.
    TheCopperAge,
    /// The Garden Awakens.
    TheGardenAwakens,
    /// Tiny Takeover.
    TinyTakeover,
}

impl Section {
    /// Every section, in the reference's order.
    pub const ALL: [Section; 13] = [
        Section::DefaultSkins,
        Section::Modrinth,
        Section::ModrinthPride,
        Section::SavedSkins,
        Section::BuildersAndBiomes,
        Section::ChaosCubed,
        Section::ChaseTheSkies,
        Section::MineconEarth2017,
        Section::MountsOfMayhem,
        Section::StridingHero,
        Section::TheCopperAge,
        Section::TheGardenAwakens,
        Section::TinyTakeover,
    ];

    /// The reference's key for this section's heading.
    pub const fn key(self) -> Key {
        match self {
            Section::DefaultSkins => Key::AppSkinsSectionDefaultSkins,
            Section::Modrinth => Key::AppSkinsSectionModrinth,
            Section::ModrinthPride => Key::AppSkinsSectionModrinthPride,
            Section::SavedSkins => Key::AppSkinsSectionSavedSkins,
            Section::BuildersAndBiomes => Key::AppSkinsSectionBuildersAndBiomes,
            Section::ChaosCubed => Key::AppSkinsSectionChaosCubed,
            Section::ChaseTheSkies => Key::AppSkinsSectionChaseTheSkies,
            Section::MineconEarth2017 => Key::AppSkinsSectionMineconEarth2017,
            Section::MountsOfMayhem => Key::AppSkinsSectionMountsOfMayhem,
            Section::StridingHero => Key::AppSkinsSectionStridingHero,
            Section::TheCopperAge => Key::AppSkinsSectionTheCopperAge,
            Section::TheGardenAwakens => Key::AppSkinsSectionTheGardenAwakens,
            Section::TinyTakeover => Key::AppSkinsSectionTinyTakeover,
        }
    }

    /// The heading, as the reference writes it.
    pub fn label(self) -> &'static str {
        self.key().message()
    }
}

/// The request the page makes: read the account's own appearance.
///
/// Nothing but the round travels. *Which* account is the shell's to know -- it is
/// the one a launch would sign in as, and a page has never seen an account file or
/// a token -- so what a page asks is the question, and the shell is what can answer
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Which request this is, counting from one.
    pub round: u64,
}

/// A change to what the account wears, asked for by this page and made by the shell.
///
/// The same shape as [`Asked`], one step further out: the *change* is this page's to
/// describe -- the reader pressed a row, and the row knows which skin or cape it
/// is -- and the account, the token and the request are the shell's, for [`Asked`]'s
/// reason. The round travels so the answer can be matched to the change that is
/// actually waiting for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wear {
    /// Which request this is, counting from one, beside the reads' own counter.
    pub round: u64,
    /// What to change.
    pub change: SkinChange,
}

/// The request that opens the launcher's file dialog and uploads what it returns.
///
/// The third shape in this file, and the one where the page has the least: the path
/// comes from a dialog and the bytes from a file, so neither is a page's to describe --
/// what travels is the round and nothing else, and the change is built where the file
/// is read. The round is here for [`Wear`]'s reason: the answer has to be matchable to
/// the press that is waiting for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Add {
    /// Which request this is, counting from one, beside the reads' and writes' own
    /// counters.
    pub round: u64,
}

/// What came of asking the reader for a file.
///
/// Three answers rather than a `Result`, because a cancel is not a failure: a reader
/// who opened the dialog and changed their mind is a finished interaction, and a page
/// that apologised for it would be apologising for nothing. The third is not the
/// reader's doing either -- a build with no picker in it -- which is why it is its own
/// arm rather than a sentence in the second's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    /// The reader closed the dialog without choosing anything.
    Cancelled,
    /// A texture was chosen and the account wears it -- or would not, with the
    /// reason the service gave.
    Done(Result<(), String>),
    /// This machine has no dialog to open at all.
    NoPicker(String),
}

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// A section was opened.
    Select(Section),
    /// The page was asked to add a skin from a file.
    AddSkin,
    /// What came of asking the reader for a file.
    Added {
        /// Which request this answers, so an answer to one the page has replaced is
        /// dropped rather than drawn.
        round: u64,
        /// The reader's answer, or the machine's.
        picked: Picked,
    },
    /// The reader asked to put something the account owns on -- or take it off.
    Wear(SkinChange),
    /// The shell's answer to a change: which change it answers, and what happened.
    ///
    /// A failure is a sentence in the same slot every other thing this page could
    /// not do goes; a success says nothing, because what it changed is drawn two
    /// rows down by the reload that follows it.
    Applied {
        /// Which change this answers.
        round: u64,
        /// Nothing on success, or the reason it could not be made.
        result: Result<(), String>,
    },
    /// The account's own appearance arrived.
    ///
    /// Boxed at the crossing, for the reason `project::Message::Found` is: this is
    /// the one variant that is a page's worth of data beside a dozen unit ones, and
    /// boxing it here keeps the page's own arms from boxing anything.
    Found {
        /// Which request this answers, so an answer to a question the page has
        /// replaced is dropped rather than drawn.
        round: u64,
        /// The account's appearance, or why it could not be read.
        result: Result<Box<Appearance>, String>,
    },
    /// The pointer entered or left one of the page's controls, for the clock
    /// that carries a hover's 150 ms (see [`crate::ui`]).
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

/// The one control in the page's header. The rows carry the other action, and their
/// hover names come from [`ui::scoped`] because they repeat.
const ADD_KEY: &str = "skins:add";

/// The name space the per-row Apply buttons take their hover names from.
const WEAR_KEY: &str = "skins:wear";

/// The front view's own size on the page: the texture's 16x32 at six times, which
/// is as large as the reference draws its model and the only kind of size that
/// does not blur a face that is eight pixels wide.
const DOLL_WIDTH: f32 = 96.0;
const DOLL_HEIGHT: f32 = 192.0;

/// The mark beside the skin or cape that is in force.
const WORN_MARK: f32 = 14.0;

/// The page's own state.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Which section is open, if any.
    pub open: Option<usize>,
    /// The last thing the page could not do, shown rather than swallowed.
    pub notice: Option<String>,
    /// The account's own appearance: what Minecraft says it owns, and the skin in
    /// force cut into a front view. A `Load`, because it is a service's answer and
    /// the page is drawn before it arrives.
    pub appearance: Load<Appearance>,
    /// Which request the page is waiting for, so an answer to a question it has
    /// replaced is dropped rather than drawn.
    round: u64,
    /// Whether a change the reader asked for is still on its way.
    ///
    /// The rows draw their Apply buttons unusable while it is: a reader who pressed
    /// one and saw nothing happen would press it again, and the second press would
    /// be a second write to their own account.
    pub wearing: bool,
}

impl State {
    /// Apply a message, and answer with what the shell has to do about it.
    ///
    /// An [`Ask`] rather than this page's own [`Asked`], for the project page's
    /// reason: this page asks for two kinds of thing now -- read me the account's
    /// own appearance, and change what it wears -- and only one of them is a
    /// question.
    pub fn update(&mut self, message: Message) -> Option<Ask> {
        match message {
            Message::Select(section) => {
                let index = Section::ALL.iter().position(|candidate| *candidate == section);
                self.open = if self.open == index { None } else { index };
            }
            Message::AddSkin => {
                // One at a time, like a row's Apply and for a stronger reason: the
                // dialog this opens is modal, so a second press would be a second
                // dialog behind the first.
                if !self.wearing {
                    self.wearing = true;
                    self.notice = None;
                    let round = self.round + 1;
                    self.round = round;
                    return Some(Ask::AddSkin(Add { round }));
                }
            }
            Message::Added { round, picked } => {
                if round == self.round {
                    self.wearing = false;
                    match picked {
                        // The reader changed their mind: the press is finished and
                        // there is nothing to say about it.
                        Picked::Cancelled => {}
                        // A texture that went up reloads the lists, for the reason a
                        // change that worked does: what changed is the document, and
                        // the new row is a better confirmation than a sentence about
                        // it would be.
                        Picked::Done(Ok(())) => return Some(Ask::Skins(self.ask())),
                        Picked::Done(Err(reason)) | Picked::NoPicker(reason) => {
                            self.notice = Some(reason)
                        }
                    }
                }
            }
            Message::Wear(change) => {
                // The sentence about the last thing this page could not do is about
                // a press this one replaces, and the change is on its way with the
                // round that will tell its answer from a stale one.
                if !self.wearing {
                    self.wearing = true;
                    self.notice = None;
                    let round = self.round + 1;
                    self.round = round;
                    return Some(Ask::Wear(Wear { round, change }));
                }
            }
            Message::Applied { round, result } => {
                if round == self.round {
                    self.wearing = false;
                    match result {
                        // A success says nothing and reloads instead: what changed
                        // is the document's answer, so the answer is what is drawn
                        // again -- the check moves to the row that is now in force,
                        // which is a better confirmation than a sentence about it.
                        Ok(()) => return Some(Ask::Skins(self.ask())),
                        Err(reason) => self.notice = Some(reason),
                    }
                }
            }
            Message::Found { round, result } => {
                if round == self.round {
                    self.appearance = match result {
                        Ok(appearance) => Load::Ready(*appearance),
                        Err(reason) => Load::Failed(reason),
                    };
                }
            }
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
        }
        None
    }

    /// The request the page owes because nothing has been asked for yet.
    ///
    /// The reference reads the account's skins as the page mounts; this is the
    /// same rule stated where the shell can see it, so a window opened straight on
    /// `/skins` draws the account's own skins rather than an empty gallery.
    pub fn opening(&mut self) -> Option<Asked> {
        if self.appearance == Load::Idle {
            Some(self.ask())
        } else {
            None
        }
    }

    /// Bump the round, mark the page as waiting, and describe the request.
    fn ask(&mut self) -> Asked {
        self.round += 1;
        self.appearance = Load::Loading;
        Asked { round: self.round }
    }
}

/// Draw the page.
pub fn view<'a>(theme: Gen, state: &'a State, _store: &'a Store) -> Element<'a, Message> {
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = &state.notice {
        blocks.push(ui::admonition(
            theme,
            ui::Severity::Info,
            Key::AppSkinsTitle.message(),
            notice,
        ));
    }
    blocks.push(
        row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Center)
            .push(page::title(theme, Key::AppSkinsTitle))
            .push(Space::with_width(Length::Fill))
            // The reference's header has Add and Apply side by side, and the Apply
            // acts on whatever its preview panel is showing. This page has no
            // preview panel -- a candidate skin is never rendered here, because
            // the doll draws what Minecraft says is in force -- so the Apply that
            // wears something lives on the row that *is* something, and the header
            // keeps the one control that is about the page rather than about a skin:
            // Add, which since G123 opens this launcher's own file dialog, pads what
            // it returns to the shape the service takes and uploads it (G106).
            .push(ui::button_or(
                theme,
                ADD_KEY,
                Key::AppSkinsAddButton,
                ui::Kind::Standard,
                // Unusable while a change is in flight, like the rows' Apply: this
                // one opens a modal dialog, and what it uploads afterwards is a
                // write to the reader's own account.
                (!state.wearing).then_some(Message::AddSkin),
            ))
            .into(),
    );
    // The account's own appearance, which is what this page is for: the skin in
    // force drawn, and the two lists Minecraft publishes. The reference renders its
    // model through a plugin this tree does not have -- see [`crate::skin`] for what
    // is drawn instead and what that costs -- but the *lists* are the same service's
    // answer here as there.
    blocks.push(page::draw(theme, &state.appearance, "your skins", |appearance| {
        account_block(theme, appearance, state.wearing)
    }));
    // The sections, each a card that opens and closes. The skins inside them come
    // from Modrinth's own skin store, which is an account service this launcher
    // does not hold (G118): the sentence is inside the card that would draw them
    // rather than over the account's own skins above, and it says which of the two
    // kinds of gap it is. The account's own half above is Minecraft's service and
    // is not affected -- it is what the reader's own upload writes to.
    let mut sections = column![].spacing(GAP).width(Length::Fill);
    for (index, section) in Section::ALL.iter().enumerate() {
        let open = state.open == Some(index);
        let body: Element<'a, Message> = if open {
            row![]
                .spacing(ROW_GAP)
                .push(
                    iced::widget::text(crate::store::needs_account("The skin store"))
                        .size(13.0)
                        .font(crate::style::medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
                )
                .into()
        } else {
            Space::with_height(Length::Fixed(0.0)).into()
        };
        let head = row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Center)
            .push(
                iced::widget::text(section.label())
                    .size(16.0)
                    .font(crate::style::semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(
                        theme,
                        crate::style::INK_CONTRAST,
                    ))),
            )
            .push(Space::with_width(Length::Fill));
        let plain = iced::widget::mouse_area(column![head, body].spacing(ROW_GAP).width(Length::Fill))
            .interaction(iced::mouse::Interaction::Pointer)
            .on_press(Message::Select(*section));
        // Into the column, not straight onto the page: the sections are one block
        // of it, and pushing the cards onto `blocks` left the column empty behind
        // them.
        sections = sections.push(ui::card(theme, plain));
    }
    blocks.push(sections.into());
    page::body(blocks, GAP)
}

/// The account's own appearance, as the page draws it.
///
/// Two halves that come from different places and meet here: the *picture*, which is
/// this launcher's own arithmetic over a texture (see [`crate::skin`]), and the two
/// lists, which are Minecraft's own document. A skin whose texture would not come
/// back is not a failure of the page -- the account still owns every skin it owns --
/// so the reason is drawn where the picture would be and the lists are drawn
/// underneath it.
fn account_block<'a>(theme: Gen, appearance: &'a Appearance, wearing: bool) -> Element<'a, Message> {
    let doll: Element<'a, Message> = match &appearance.front {
        Some(front) => image(front.handle())
            .width(Length::Fixed(DOLL_WIDTH))
            .height(Length::Fixed(DOLL_HEIGHT))
            .into(),
        None => text(
            appearance
                .note
                .clone()
                .unwrap_or_else(|| crate::store::not_implemented("This skin")),
        )
        .size(14.0)
        .font(crate::style::medium())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY)))
        .into(),
    };
    let hero = row![]
        .spacing(GAP)
        .align_items(Alignment::Center)
        .push(doll)
        .push(
            column![]
                .spacing(ROW_GAP)
                .push(
                    text(appearance.username.clone())
                        .size(20.0)
                        .font(crate::style::semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                )
                .push(caption(theme, &wearing_line(appearance))),
        );
    // Named rather than inferred, for the same reason `owned_row`'s is: `card`
    // takes `impl Into<Element>`.
    let hero: Element<'a, Message> = hero.into();
    let mut blocks: Vec<Element<'a, Message>> = vec![ui::card(theme, hero)];

    // The account's own skins. Minecraft's document names each one by its id and
    // its variant and nothing else -- the reference's names come from the bundles
    // *it* ships, and this launcher has none of those -- so a row says which
    // variant it is and whether it is the one in force.
    blocks.push(heading(theme, Key::AppSkinsSectionSavedSkins.message()));
    for (index, skin) in appearance.skins.iter().enumerate() {
        // The row's own identity for the clock that carries its hover: two skins can
        // be the same variant, and an index is the only thing that tells them apart
        // when the document names neither a texture nor an id they share. The id is
        // preferred when there is one, because a list that arrives in another order
        // is the same list.
        let identity = if skin.id.is_empty() { index.to_string() } else { skin.id.clone() };
        let key = ui::scoped(WEAR_KEY, &identity);
        blocks.push(owned_row(
            theme,
            &variant_label(&skin.variant),
            skin.equipped(),
            key,
            wear_skin(skin, wearing),
        ));
    }
    blocks.push(heading(theme, Key::AppSkinsModalCapeSection.message()));
    if appearance.capes.is_empty() {
        blocks.push(caption(theme, Key::AppSkinsModalNoneCapeOption.message()));
    }
    for cape in &appearance.capes {
        let name = if cape.alias.is_empty() { cape.id.as_str() } else { cape.alias.as_str() };
        // One answer for which cape is in force rather than each row's own state:
        // asked once, the rows and the hero block cannot disagree about it.
        let worn = appearance.equipped_cape().is_some_and(|worn| worn.id == cape.id);
        let identity = if cape.id.is_empty() { name.to_string() } else { cape.id.clone() };
        let key = ui::scoped(WEAR_KEY, &identity);
        blocks.push(owned_row(theme, name, worn, key, wear_cape(cape, worn, wearing)));
    }
    // The reference's own "no cape" choice, drawn only when something has to be
    // taken off: a row that hides a cape the account is not wearing would be a
    // button whose only effect is a request to change nothing.
    if let Some(cape) = appearance.equipped_cape() {
        blocks.push(owned_row(
            theme,
            Key::AppSkinsModalNoneCapeOption.message(),
            false,
            ui::scoped(WEAR_KEY, &format!("none:{}", cape.id)),
            (!wearing).then_some(Message::Wear(SkinChange::NoCape)),
        ));
    }
    column(blocks).spacing(GAP).width(Length::Fill).into()
}

/// The change that puts one of the account's own skins on, or nothing when there
/// is nothing to do.
///
/// Its own function so the rule -- not the skin already in force, and not while a
/// change is in flight -- is a thing a test can read rather than something to be
/// inferred from a drawn row.
fn wear_skin(skin: &palantir_net::MinecraftSkin, wearing: bool) -> Option<Message> {
    if skin.equipped() || wearing {
        return None;
    }
    Some(Message::Wear(SkinChange::Skin {
        variant: skin.variant.clone(),
        url: skin.url.clone(),
    }))
}

/// The same rule for one of the account's capes.
fn wear_cape(cape: &palantir_net::MinecraftCape, worn: bool, wearing: bool) -> Option<Message> {
    if worn || wearing {
        return None;
    }
    Some(Message::Wear(SkinChange::Cape { id: cape.id.clone() }))
}

/// One thing the account owns: what it is called, and the mark when it is worn.
///
/// The mark is the reference's own `CheckIcon`, in the accent ink rather than a word
/// this launcher would have had to invent: the document says `ACTIVE`, and a reader
/// who sees the check on the skin the page is drawing above knows which is which.
fn owned_row<'a>(
    theme: Gen,
    name: &str,
    worn: bool,
    key: &'static str,
    action: Option<Message>,
) -> Element<'a, Message> {
    let ink = if worn { INK_CONTRAST } else { INK_SECONDARY };
    let mut line = row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .push(
            text(name.to_string())
                .size(14.0)
                .font(crate::style::medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, ink))),
        )
        .push(Space::with_width(Length::Fill));
    if worn {
        line = line.push(icon::icon(
            Glyph::Check,
            WORN_MARK,
            theme_gen::ink(theme, Ink::AccentContrast),
        ));
    }
    // The action is the reference's own Apply, on the row it applies to. It is
    // drawn unusable rather than dropped while a change is in flight (`None`,
    // which is what `button_or` takes for that): a second press would be a second
    // write to the reader's own account.
    let line = line.push(ui::button_or(
        theme,
        key,
        Key::AppSkinsApplyButton,
        ui::Kind::Standard,
        action,
    ));
    // Named rather than inferred: `ui::card` takes `impl Into<Element>`, and an
    // `into()` inside that is a conversion with two possible targets.
    let row: Element<'a, Message> = line.into();
    ui::card(theme, row)
}

/// A heading inside the account's block: the reference's section titles.
fn heading<'a>(theme: Gen, label: &str) -> Element<'a, Message> {
    text(label.to_string())
        .size(16.0)
        .font(crate::style::semibold())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)))
        .into()
}

/// A line of prose under something, in the panel's own secondary ink.
fn caption<'a>(theme: Gen, line: &str) -> Element<'a, Message> {
    text(line.to_string())
        .size(13.0)
        .font(crate::style::medium())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY)))
        .into()
}

/// The one sentence about the skin in force, from the document's own vocabulary.
fn wearing_line(appearance: &Appearance) -> String {
    match appearance.equipped() {
        Some(skin) => format!("Wearing the {} skin", variant_label(&skin.variant).to_lowercase()),
        None => "Minecraft does not say which skin is in force.".to_string(),
    }
}

/// `CLASSIC` and `SLIM` as a reader reads them.
///
/// The document's words are the format's; the reference's are the modal's own two
/// arm-style labels, and they are the same two things.
fn variant_label(variant: &str) -> String {
    if variant.eq_ignore_ascii_case("SLIM") {
        Key::AppSkinsModalArmStyleSlim.message().to_string()
    } else {
        Key::AppSkinsModalArmStyleWide.message().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sections_are_the_reference_s_own_and_carry_its_names() {
        assert_eq!(Section::ALL.len(), 13);
        // No two sections show the same heading: a list built from one key twice
        // would look deliberate and be wrong.
        let mut labels: Vec<&str> = Section::ALL.iter().map(|section| section.label()).collect();
        let count = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(count, labels.len());
        assert_eq!(Section::DefaultSkins.label(), "Default skins");
        assert_eq!(Section::ModrinthPride.label(), "Modrinth Pride");
        for section in Section::ALL {
            assert_eq!(section.label(), section.key().message());
            assert!(section.key().name().starts_with("app.skins.section."));
        }
    }

    #[test]
    fn opening_a_section_toggles_it_and_only_one_is_open_at_a_time() {
        let mut state = State::default();
        assert_eq!(state.open, None);
        state.update(Message::Select(Section::Modrinth));
        assert_eq!(state.open, Section::ALL.iter().position(|s| *s == Section::Modrinth));
        state.update(Message::Select(Section::TheCopperAge));
        assert_eq!(state.open, Section::ALL.iter().position(|s| *s == Section::TheCopperAge));
        // Pressing the open one closes it, which is what the reference's
        // disclosure does.
        state.update(Message::Select(Section::TheCopperAge));
        assert_eq!(state.open, None);
    }

    #[test]
    fn the_page_asks_once_and_keeps_the_answer_that_answers_it() {
        let mut state = State::default();
        assert_eq!(state.appearance, Load::Idle, "nothing asked for yet");
        let asked = state.opening().expect("a first request");
        assert_eq!(asked.round, 1);
        assert_eq!(state.appearance, Load::Loading, "and it is waiting on it");
        assert!(state.opening().is_none(), "asked once, not once per frame");
        // An answer to a question the page has replaced is dropped rather than
        // drawn: the round travels with it, which is what makes that decidable.
        assert!(state.update(Message::Found { round: 0, result: Err("old".into()) }).is_none());
        assert_eq!(state.appearance, Load::Loading, "so the stale one is not drawn");
        // The answer that does answer it becomes the page's own state, and a
        // failure stays a sentence rather than becoming an empty gallery.
        let appearance = crate::skin::Appearance::of(
            "Steve",
            palantir_net::MinecraftSkins::default(),
            Err("the network is down".into()),
        );
        state.update(Message::Found { round: asked.round, result: Ok(Box::new(appearance)) });
        assert!(matches!(state.appearance, Load::Ready(_)));
        state.update(Message::Found { round: asked.round, result: Err("signed out".into()) });
        assert_eq!(state.appearance, Load::Failed("signed out".into()));
    }

    #[test]
    fn adding_a_skin_asks_the_shell_for_a_file_and_takes_one_at_a_time() {
        // The page's half of an upload: one request, carrying the round, and no
        // second one while the first is out. The dialog is the shell's (`Add`), and
        // what the reader chose never passes through here -- only the round does.
        let mut state = State::default();
        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("the press asks the shell for a file");
        };
        assert_eq!(add.round, 1);
        assert!(state.wearing, "and the page waits on it");
        assert_eq!(state.notice, None);
        assert_eq!(state.update(Message::AddSkin), None, "and a second press is dropped");
        assert!(state.wearing);
    }

    #[test]
    fn the_three_answers_to_that_request_draw_three_different_pages() {
        // Cancelled: the press is finished and nothing is said. Failed: a sentence in
        // the slot every other failure goes. No picker: also a sentence, and a
        // different one -- the reader did nothing wrong.
        let mut state = State::default();
        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("the press asks the shell for a file");
        };
        assert_eq!(
            state.update(Message::Added { round: add.round, picked: Picked::Cancelled }),
            None,
            "a cancel asks for nothing"
        );
        assert!(!state.wearing);
        assert_eq!(state.notice, None, "and says nothing");

        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("asked again");
        };
        assert_eq!(
            state.update(Message::Added {
                round: add.round,
                picked: Picked::Done(Err("Minecraft refused the change: not a skin".to_string())),
            }),
            None
        );
        assert!(!state.wearing);
        assert!(state.notice.as_deref().unwrap_or_default().contains("not a skin"));

        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("asked a third time");
        };
        state.update(Message::Added {
            round: add.round,
            picked: Picked::NoPicker("This build has no file picker".to_string()),
        });
        assert!(!state.wearing);
        assert!(state.notice.as_deref().unwrap_or_default().contains("no file picker"));
    }

    #[test]
    fn an_upload_that_worked_reloads_the_lists_and_a_stale_answer_is_dropped() {
        // The same rule a successful wear follows, from the same place: what changed is
        // Minecraft's document, so the document is what is read again.
        let mut state = State::default();
        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("the press asks the shell for a file");
        };
        // An answer to a request the page has replaced is dropped: the round is what
        // makes that decidable.
        assert_eq!(
            state.update(Message::Added { round: 0, picked: Picked::Cancelled }),
            None
        );
        assert!(state.wearing, "so the page is still waiting");
        let Some(Ask::Skins(asked)) = state
            .update(Message::Added { round: add.round, picked: Picked::Done(Ok(())) })
        else {
            panic!("an upload that worked reloads");
        };
        assert_eq!(asked.round, add.round + 1);
        assert_eq!(state.appearance, Load::Loading);
        assert!(!state.wearing, "the request is finished even while the read is out");
        assert_eq!(state.notice, None, "a success is not a sentence");
    }

    #[test]
    fn wearing_something_is_one_request_and_the_second_press_is_not_a_second_write() {
        let mut state = State::default();
        let change = SkinChange::Cape { id: "cape-1".to_string() };
        let Some(Ask::Wear(worn)) = state.update(Message::Wear(change.clone())) else {
            panic!("the press asks the shell");
        };
        assert_eq!(worn.round, 1);
        assert_eq!(worn.change, change);
        assert!(state.wearing, "and the page is waiting on it");
        // A second press while the first is in flight is dropped, not sent: the
        // rows draw their buttons unusable, and this is the same rule where it
        // cannot be sidestepped.
        assert_eq!(state.update(Message::Wear(change.clone())), None);
        assert!(state.wearing);
        // A failure is a sentence, and the page is ready to press again.
        let answered = state.update(Message::Applied {
            round: 1,
            result: Err("Minecraft refused the change: nope".to_string()),
        });
        assert_eq!(answered, None);
        assert!(!state.wearing);
        assert!(state.notice.as_deref().unwrap_or_default().contains("nope"));
        // And an answer to a change the page has replaced is dropped rather than
        // reported: the round is what makes that decidable.
        assert_eq!(
            state.update(Message::Applied { round: 0, result: Err("old".to_string()) }),
            None
        );
        assert!(state.notice.as_deref().unwrap_or_default().contains("nope"));
    }

    #[test]
    fn a_change_that_worked_reloads_the_lists_instead_of_saying_so() {
        let mut state = State::default();
        let Some(Ask::Wear(worn)) = state.update(Message::Wear(SkinChange::NoCape)) else {
            panic!("the press asks the shell");
        };
        // The answer to the write is not the answer to the read: the page asks
        // again, and the reload is what shows the check on the row that is now in
        // force.
        let Some(Ask::Skins(asked)) = state.update(Message::Applied { round: worn.round, result: Ok(()) }) else {
            panic!("a change that worked reloads");
        };
        assert_eq!(asked.round, worn.round + 1);
        assert_eq!(state.appearance, Load::Loading);
        assert!(!state.wearing, "the press is finished even while the read is out");
        assert_eq!(state.notice, None, "a success is not a sentence");
    }

    #[test]
    fn the_rows_offer_to_wear_everything_that_is_not_already_on() {
        // The rule the drawn rows follow, read where it can be: a skin already in
        // force is not offered, a cape already on is not offered, and neither is
        // anything at all while a change is in flight.
        let worn_skin = palantir_net::MinecraftSkin {
            id: "skin-1".to_string(),
            state: "ACTIVE".to_string(),
            url: "http://textures.minecraft.net/texture/aaa".to_string(),
            variant: "CLASSIC".to_string(),
        };
        let cold_skin = palantir_net::MinecraftSkin {
            state: "INACTIVE".to_string(),
            ..worn_skin.clone()
        };
        assert!(wear_skin(&worn_skin, false).is_none());
        let Some(Message::Wear(SkinChange::Skin { variant, url })) = wear_skin(&cold_skin, false)
        else {
            panic!("a skin that is not on is offered");
        };
        assert_eq!(variant, "CLASSIC");
        assert!(url.ends_with("aaa"));
        assert!(wear_skin(&cold_skin, true).is_none(), "not while one is in flight");
        let cape = palantir_net::MinecraftCape {
            id: "cape-1".to_string(),
            state: "INACTIVE".to_string(),
            url: "http://textures.minecraft.net/texture/ccc".to_string(),
            alias: "Migrator".to_string(),
        };
        assert!(matches!(
            wear_cape(&cape, false, false),
            Some(Message::Wear(SkinChange::Cape { id })) if id == "cape-1"
        ));
        assert!(wear_cape(&cape, true, false).is_none(), "it is already on");
        assert!(wear_cape(&cape, false, true).is_none(), "something is in flight");
    }

    #[test]
    fn the_page_draws_in_every_theme_and_in_both_of_its_shapes() {
        let store = Store::default();
        for theme in Gen::ALL {
            for open in [None, Some(0), Some(12)] {
                let state = State { open, ..State::default() };
                drop(view(*theme, &state, &store));
            }
            let state = State { notice: Some("x".into()), ..State::default() };
            drop(view(*theme, &state, &store));
            // And with an account's appearance in hand, which is the shape the page
            // is drawn in after the answer arrives -- including the one where the
            // picture could not be made.
            let appearance = crate::skin::Appearance::of(
                "Steve",
                palantir_net::MinecraftSkins {
                    skins: vec![palantir_net::MinecraftSkin {
                        id: "skin-1".into(),
                        state: "ACTIVE".into(),
                        url: "http://textures.minecraft.net/texture/aaa".into(),
                        variant: "SLIM".into(),
                    }],
                    capes: vec![palantir_net::MinecraftCape {
                        id: "cape-1".into(),
                        state: "ACTIVE".into(),
                        url: "http://textures.minecraft.net/texture/ccc".into(),
                        alias: "Migrator".into(),
                    }],
                },
                Err("the network is down".into()),
            );
            let state = State {
                open: None,
                appearance: Load::Ready(appearance),
                ..State::default()
            };
            drop(view(*theme, &state, &store));
        }
    }
}
