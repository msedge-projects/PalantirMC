//! Skins, the reference's third rail slot: `pages/Skins.vue`.
//!
//! The page is a gallery of *sections* -- the bundles the reference ships, whose
//! names are its own keys -- over a preview of the skin in force. Both halves need
//! a service: the bundles come from Modrinth's skin store and the preview from a
//! rendered skin, so what this page draws today is the section list and the states
//! around it, with the sentence that says where the rest comes from rather than a
//! grid of empty boxes.

use iced::widget::{column, image, row, text, Space};
use iced::{Alignment, Element, Length};

use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP, ROW_GAP};
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

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// A section was opened.
    Select(Section),
    /// The page was asked to add a skin from a file.
    AddSkin,
    /// The selected skin was asked to be applied to the account.
    Apply,
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

/// The page's two actions.
const ADD_KEY: &str = "skins:add";
const APPLY_KEY: &str = "skins:apply";

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
}

impl State {
    /// Apply a message, reporting anything only the shell can do.
    pub fn update(&mut self, message: Message) -> Option<Asked> {
        match message {
            Message::Select(section) => {
                let index = Section::ALL.iter().position(|candidate| *candidate == section);
                self.open = if self.open == index { None } else { index };
            }
            Message::AddSkin => {
                self.notice = Some(crate::store::not_implemented("Adding a skin"));
            }
            Message::Apply => {
                self.notice = Some(crate::store::not_implemented("Applying a skin"));
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
            .push(ui::button(theme, ADD_KEY, Key::AppSkinsAddButton, ui::Kind::Standard, Message::AddSkin))
            .push(ui::button(theme, APPLY_KEY, Key::AppSkinsApplyButton, ui::Kind::Colored, Message::Apply))
            .into(),
    );
    // The account's own appearance, which is what this page is for: the skin in
    // force drawn, and the two lists Minecraft publishes. The reference renders its
    // model through a plugin this tree does not have -- see [`crate::skin`] for what
    // is drawn instead and what that costs -- but the *lists* are the same service's
    // answer here as there.
    blocks.push(page::draw(theme, &state.appearance, "your skins", |appearance| {
        account_block(theme, appearance)
    }));
    // The sections, each a card that opens and closes. The skins inside them come
    // from Modrinth's own skin store, which is a service answer this launcher has
    // not been given: the sentence is inside the card that would draw them rather
    // than over the account's own skins above.
    let mut sections = column![].spacing(GAP).width(Length::Fill);
    for (index, section) in Section::ALL.iter().enumerate() {
        let open = state.open == Some(index);
        let body: Element<'a, Message> = if open {
            row![]
                .spacing(ROW_GAP)
                .push(
                    iced::widget::text(crate::store::not_implemented("This section's skins"))
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
fn account_block<'a>(theme: Gen, appearance: &'a Appearance) -> Element<'a, Message> {
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
    for skin in &appearance.skins {
        blocks.push(owned_row(theme, &variant_label(&skin.variant), skin.equipped()));
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
        blocks.push(owned_row(theme, name, worn));
    }
    column(blocks).spacing(GAP).width(Length::Fill).into()
}

/// One thing the account owns: what it is called, and the mark when it is worn.
///
/// The mark is the reference's own `CheckIcon`, in the accent ink rather than a word
/// this launcher would have had to invent: the document says `ACTIVE`, and a reader
/// who sees the check on the skin the page is drawing above knows which is which.
fn owned_row<'a>(theme: Gen, name: &str, worn: bool) -> Element<'a, Message> {
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
    fn the_two_things_this_page_cannot_do_yet_say_so() {
        let mut state = State::default();
        state.update(Message::AddSkin);
        assert!(state.notice.as_deref().unwrap_or_default().contains("is not implemented yet"));
        state.update(Message::Apply);
        assert!(state.notice.as_deref().unwrap_or_default().contains("is not implemented yet"));
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
