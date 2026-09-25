//! Skins, the reference's third rail slot: `pages/Skins.vue`.
//!
//! The page is a gallery of *sections* -- the bundles the reference ships, whose
//! names are its own keys -- over a preview of the skin in force. Both halves need
//! a service: the bundles come from Modrinth's skin store and the preview from a
//! rendered skin, so what this page draws today is the section list and the states
//! around it, with the sentence that says where the rest comes from rather than a
//! grid of empty boxes.

use iced::widget::{column, row, Space};
use iced::{Alignment, Element, Length};

use crate::page::{self, GAP, ROW_GAP};
use crate::store::Store;
use crate::style::INK_SECONDARY;
use crate::text_gen::Key;
use crate::theme_gen::{self, Theme as Gen};
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

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// A section was opened.
    Select(Section),
    /// The page was asked to add a skin from a file.
    AddSkin,
    /// The selected skin was asked to be applied to the account.
    Apply,
}

/// The page's own state.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Which section is open, if any.
    pub open: Option<usize>,
    /// The last thing the page could not do, shown rather than swallowed.
    pub notice: Option<String>,
}

impl State {
    /// Apply a message.
    pub fn update(&mut self, message: Message) {
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
        }
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
            .push(ui::button(theme, Key::AppSkinsAddButton, ui::Kind::Standard, Message::AddSkin))
            .push(ui::button(theme, Key::AppSkinsApplyButton, ui::Kind::Colored, Message::Apply))
            .into(),
    );
    // The sections, each a card that opens and closes. The skins inside them come
    // from the skin store; the sentence says so once, at the top, rather than once
    // per card.
    blocks.push(ui::admonition(
        theme,
        ui::Severity::Info,
        Key::AppSkinsPreviewingBadge.message(),
        &crate::store::not_implemented("The skin previews"),
    ));
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
                let state = State { open, notice: None };
                drop(view(*theme, &state, &store));
            }
            let state = State { open: None, notice: Some("x".into()) };
            drop(view(*theme, &state, &store));
        }
    }
}
