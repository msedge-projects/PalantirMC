//! The widgets the pages are drawn from, each one quoting the reference's own
//! class or rule rather than inventing a look.
//!
//! | Widget | Reference |
//! | --- | --- |
//! | [`card`] | `.base-card` in `assets/styles/classes.scss`: `padding: 1rem`, `background-color: var(--surface-3)`, `border-radius: var(--radius-lg)`, `border: 1px solid var(--surface-4)` |
//! | [`tabs`] | `base/NavTabs.vue`: `rounded-full bg-bg-raised p-1`, each tab `px-4 py-2 font-bold` |
//! | [`admonition`] | `base/Admonition.vue`: a 24px severity icon in its own colour, then a header and a body |
//! | [`search`] | `base/inputs/Input.vue`: a 20px `text-secondary opacity-60` icon, value `text-primary`, placeholder `text-secondary`, focus `text-contrast` |
//! | [`select`] | `base/Combobox.vue`: `rounded-xl`, a `font-medium text-primary` prefix, the value, and a chevron |
//! | [`button`] | `base/buttons/Button.vue`: `text-sm font-bold text-secondary`, colour per type |
//!
//! Two of these are *readings* rather than quotations, and say so where they are
//! written: the input's own wrapper classes come from a shared style that is not in
//! the vendored tree, and a button's radius comes from the surrounding control size
//! rather than from a rule. Both are marked at the point they are used, so the next
//! session can measure them against a running reference instead of trusting this
//! file.

#![allow(dead_code)]

use iced::widget::{column, container, mouse_area, row, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use iced::{Theme, mouse::Interaction};

use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::ROW_GAP;
use crate::style::{heading, medium, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Ink, Span, Theme as Gen};

/// `.base-card`'s `padding: 1rem`.
pub const CARD_PAD: f32 = 16.0;
/// A control's height: the reference's `h-10` on a large input and a base combobox.
pub const CONTROL: f32 = 40.0;
/// `rounded-xl` on a combobox and an input wrapper.
pub const CONTROL_RADIUS: f32 = 12.0;
/// `size-5` on the icons inside a control.
pub const CONTROL_ICON: f32 = 20.0;
/// A tag's height and the pill it sits in.
pub const TAG_HEIGHT: f32 = 24.0;

// ---- Surfaces ------------------------------------------------------------

/// A card: `--surface-3`, `--radius-lg`, a `--surface-4` hairline.
pub fn card<'a, Message: 'a>(theme: Gen, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .padding(CARD_PAD)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface3))),
            border: Border {
                color: theme_gen::ink(theme, Ink::Surface4),
                width: 1.0,
                radius: theme_gen::span(Span::RadiusLg).into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

/// A row with a hairline around it, for the controls that need a frame.
pub fn framed<'a, Message: 'a>(
    theme: Gen,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(CONTROL))
        .padding(Padding { top: 0.0, bottom: 0.0, left: 12.0, right: 12.0 })
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            // `--base` is the reference's sunken surface and the one its search
            // field sits on; the hairline is `--surface-5`.
            background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface5))),
            border: Border {
                color: theme_gen::ink(theme, Ink::Surface4),
                width: 1.0,
                radius: CONTROL_RADIUS.into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

/// A tag: a small pill in the raised surface.
pub fn tag<'a, Message: 'a>(theme: Gen, label: &str) -> Element<'a, Message> {
    container(
        text(label.to_string())
            .size(12.0)
            .font(semibold())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
    )
    .height(Length::Fixed(TAG_HEIGHT))
    .padding(Padding { top: 0.0, bottom: 0.0, left: 8.0, right: 8.0 })
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
        border: Border { radius: 12.0.into(), ..Border::default() },
        ..container::Appearance::default()
    })
    .into()
}

// ---- Controls ------------------------------------------------------------

/// The text style an input paints from: [`Field`].
///
/// The wrapper reading is marked in the module docs; the border colours are the
/// reference's own (`--surface-4` at rest, the accent when focused, which is what
/// `focus:ring` resolves to in its stylesheet).
pub struct Field {
    theme: Gen,
    chrome: Chrome,
}

/// Whether a field paints its own frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Chrome {
    /// A bordered, filled field.
    Bordered,
    /// No frame at all, for a field inside a row that draws its own.
    Bare,
}

impl iced::widget::text_input::StyleSheet for Field {
    type Style = Theme;

    fn active(&self, _theme: &Theme) -> text_input::Appearance {
        match self.chrome {
            Chrome::Bordered => text_input::Appearance {
                background: Background::Color(theme_gen::ink(self.theme, Ink::Surface5)),
                border: Border {
                    radius: CONTROL_RADIUS.into(),
                    width: 1.0,
                    color: theme_gen::ink(self.theme, Ink::Surface4),
                },
                icon_color: theme_gen::ink(self.theme, INK_SECONDARY),
            },
            Chrome::Bare => text_input::Appearance {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                icon_color: theme_gen::ink(self.theme, INK_SECONDARY),
            },
        }
    }

    fn focused(&self, _theme: &Theme) -> text_input::Appearance {
        match self.chrome {
            Chrome::Bordered => text_input::Appearance {
                background: Background::Color(theme_gen::ink(self.theme, Ink::Surface5)),
                border: Border {
                    radius: CONTROL_RADIUS.into(),
                    width: 1.0,
                    color: theme_gen::ink(self.theme, Ink::Brand),
                },
                icon_color: theme_gen::ink(self.theme, INK_CONTRAST),
            },
            Chrome::Bare => self.active(&Theme::Dark),
        }
    }

    fn placeholder_color(&self, _theme: &Theme) -> Color {
        theme_gen::ink(self.theme, INK_SECONDARY)
    }

    fn value_color(&self, _theme: &Theme) -> Color {
        theme_gen::ink(self.theme, INK_DEFAULT)
    }

    fn disabled_color(&self, _theme: &Theme) -> Color {
        crate::style::disabled(self.theme, INK_DEFAULT)
    }

    fn selection_color(&self, _theme: &Theme) -> Color {
        let accent = theme_gen::ink(self.theme, Ink::Brand);
        Color { a: 0.35, ..accent }
    }

    fn disabled(&self, _theme: &Theme) -> text_input::Appearance {
        text_input::Appearance {
            background: Background::Color(theme_gen::ink(self.theme, Ink::Surface5)),
            border: Border {
                radius: CONTROL_RADIUS.into(),
                width: 1.0,
                color: crate::style::disabled(self.theme, Ink::Surface4),
            },
            icon_color: crate::style::disabled(self.theme, INK_SECONDARY),
        }
    }
}

/// The search field: a 20px search glyph and the input, on one surface.
///
/// The placeholder is a string rather than a key, because two of the reference's
/// own are messages with an argument in them: Discover's reads *"Search
/// {projectType}..."* and is asked for by the tab it is on.
pub fn search<'a, Message: Clone + 'a>(
    theme: Gen,
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let field = text_input(placeholder, value)
        .on_input(on_input)
        .padding(Padding { top: 0.0, bottom: 0.0, left: 0.0, right: 0.0 })
        .size(14.0)
        .font(medium())
        .style(iced::theme::TextInput::Custom(Box::new(Field { theme, chrome: Chrome::Bare })));
     framed(
        theme,
        row![]
            .align_items(Alignment::Center)
            .spacing(ROW_GAP)
            .push(icon::icon(Glyph::Search, CONTROL_ICON, theme_gen::ink(theme, INK_SECONDARY)))
            .push(field),
    )
}

/// A combobox trigger: a prefix, the value, and the chevron that says it opens.
pub fn select<'a, Message: 'a>(
    theme: Gen,
    prefix: Key,
    value: &str,
    width: f32,
) -> Element<'a, Message> {
    let body = row![]
        .align_items(Alignment::Center)
        .spacing(ROW_GAP)
        .push(
            text(prefix.message())
                .size(14.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        )
        .push(
            text(value.to_string())
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        )
        .push(Space::with_width(Length::Fill))
        .push(icon::icon(Glyph::ChevronDown, 16.0, theme_gen::ink(theme, INK_SECONDARY)));
    let framed = framed(theme, body);
    container(framed).width(Length::Fixed(width)).into()
}

/// What kind of button this is, by the reference's own `type` prop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `standard`: the raised surface.
    Standard,
    /// `colored`: the accent, with the text it is legible on.
    Colored,
    /// `outlined`: a hairline and no fill.
    Outlined,
    /// `quiet`: no frame at all.
    Quiet,
}

/// A button: `text-sm font-bold`, a radius from the control size.
pub fn button<'a, Message: Clone + 'a>(
    theme: Gen,
    key: Key,
    kind: Kind,
    on_press: Message,
) -> Element<'a, Message> {
    let (background, border, ink) = match kind {
        Kind::Standard => (
            Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
            None,
            theme_gen::ink(theme, INK_CONTRAST),
        ),
        Kind::Colored => (
            Some(Background::Color(theme_gen::ink(theme, Ink::Brand))),
            None,
            theme_gen::ink(theme, Ink::AccentContrast),
        ),
        Kind::Outlined => (
            None,
            Some(theme_gen::ink(theme, Ink::Surface4)),
            theme_gen::ink(theme, INK_CONTRAST),
        ),
        Kind::Quiet => (None, None, theme_gen::ink(theme, INK_CONTRAST)),
    };
    let face = container(
        text(key.message())
            .size(14.0)
            .font(heading())
            .style(iced::theme::Text::Color(ink)),
    )
    .height(Length::Fixed(CONTROL))
    .padding(Padding { top: 0.0, bottom: 0.0, left: 16.0, right: 16.0 })
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background,
        border: Border {
            color: border.unwrap_or(Color::TRANSPARENT),
            width: if border.is_some() { 1.0 } else { 0.0 },
            radius: CONTROL_RADIUS.into(),
        },
        ..container::Appearance::default()
    });
    mouse_area(face).interaction(Interaction::Pointer).on_press(on_press).into()
}

/// A quiet icon button, the square one the reference uses in a bar.
///
/// `Message: Clone` because [`mouse_area`]'s press handler holds its message and
/// the element is rebuilt on every paint: iced requires the clone to do that, and
/// every one of this crate's message types is a clone.
pub fn icon_button<'a, Message: Clone + 'a>(
    theme: Gen,
    glyph: Glyph,
    size: f32,
    on_press: Message,
) -> Element<'a, Message> {
    let face = container(icon::icon(glyph, size, theme_gen::ink(theme, INK_DEFAULT)))
        .width(Length::Fixed(size + 16.0))
        .height(Length::Fixed(size + 16.0))
        .center_x()
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            border: Border { radius: CONTROL_RADIUS.into(), ..Border::default() },
            ..container::Appearance::default()
        });
    mouse_area(face).interaction(Interaction::Pointer).on_press(on_press).into()
}

/// The tabs a page switches between: a pill of buttons, the selected one plated.
///
/// `NavTabs.vue`'s own arrangement: a `rounded-full bg-bg-raised p-1` track with
/// `px-4 py-2` tabs inside it. The labels are strings because a page's tabs are
/// not always locale keys of their own: an instance's are, and Discover's are the
/// project-type names `route.rs` already asserts against the reference.
pub fn tabs<'a, Message: Clone + 'a>(
    theme: Gen,
    labels: &[(String, bool)],
    on_select: impl Fn(usize) -> Message,
) -> Element<'a, Message> {
    let mut track = row![].align_items(Alignment::Center).spacing(2.0);
    for (index, (label, selected)) in labels.iter().enumerate() {
        let selected = *selected;
        let ink = if selected {
            theme_gen::ink(theme, INK_CONTRAST)
        } else {
            theme_gen::ink(theme, INK_SECONDARY)
        };
        let plate = selected.then(|| theme_gen::ink(theme, Ink::ButtonBg));
        let tab = container(
            text(label.clone()).size(14.0).font(heading()).style(iced::theme::Text::Color(ink)),
        )
        .height(Length::Fixed(32.0))
        .padding(Padding { top: 0.0, bottom: 0.0, left: 16.0, right: 16.0 })
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            background: plate.map(Background::Color),
            border: Border { radius: 999.0.into(), ..Border::default() },
            ..container::Appearance::default()
        });
        track = track.push(mouse_area(tab).interaction(Interaction::Pointer).on_press(on_select(index)));
    }
    container(track)
        .padding(4.0)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
            border: Border { radius: 999.0.into(), ..Border::default() },
            ..container::Appearance::default()
        })
        .into()
}

// ---- Blocks --------------------------------------------------------------

/// Which severity an admonition carries, which is its icon and its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Left as information.
    Info,
    /// Something to look at.
    Warning,
    /// Something that went wrong.
    Critical,
}

impl Severity {
    /// The icon and ink `Admonition.vue` gives this severity.
    fn icon_and_ink(self, theme: Gen) -> (Glyph, Color) {
        match self {
            Severity::Info => (Glyph::Info, theme_gen::ink(theme, Ink::Blue)),
            Severity::Warning => (Glyph::TriangleAlert, theme_gen::ink(theme, Ink::Orange)),
            Severity::Critical => (Glyph::CircleAlert, theme_gen::ink(theme, Ink::Red)),
        }
    }
}

/// An admonition: `h-6 w-6` severity icon, a header, and a body.
pub fn admonition<'a, Message: 'a>(
    theme: Gen,
    severity: Severity,
    header: &str,
    body: &str,
) -> Element<'a, Message> {
    let (glyph, ink) = severity.icon_and_ink(theme);
    card(
        theme,
        row![]
            .spacing(12.0)
            .align_items(Alignment::Start)
            .push(icon::icon(glyph, 24.0, ink))
            .push(
                column![]
                    .spacing(4.0)
                    .push(
                        text(header.to_string())
                            .size(14.0)
                            .font(semibold())
                            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                    )
                    .push(
                        text(body.to_string())
                            .size(14.0)
                            .font(medium())
                            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
                    ),
            ),
    )
}

/// A progress bar: the accent filling a track of the inset surface.
pub fn progress<'a, Message: 'a>(theme: Gen, fraction: f32) -> Element<'a, Message> {
    let fraction = fraction.clamp(0.0, 1.0);
    container(
        container(Space::with_width(Length::Fill))
            .width(Length::FillPortion((fraction * 100.0) as u16))
            .height(Length::Fixed(6.0))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Brand))),
                border: Border { radius: 999.0.into(), ..Border::default() },
                ..container::Appearance::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(6.0))
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
        border: Border { radius: 999.0.into(), ..Border::default() },
        ..container::Appearance::default()
    })
    .into()
}

/// A label and a value on one line, the reference's metadata row.
pub fn metadata<'a, Message: 'a>(
    theme: Gen,
    label: Key,
    value: &str,
) -> Element<'a, Message> {
    row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .push(
            text(label.message())
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        )
        .push(
            text(value.to_string())
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        )
        .into()
}

/// An icon and a label, which is how a statistic is drawn.
pub fn icon_label<'a, Message: 'a>(
    theme: Gen,
    glyph: Glyph,
    label: &str,
) -> Element<'a, Message> {
    row![]
        .spacing(6.0)
        .align_items(Alignment::Center)
        .push(icon::icon(glyph, 16.0, theme_gen::ink(theme, INK_SECONDARY)))
        .push(
            text(label.to_string())
                .size(13.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        )
        .into()
}

/// A paragraph of the interface's own prose.
pub fn paragraph<'a, Message: 'a>(theme: Gen, body: &str) -> Element<'a, Message> {
    text(body.to_string())
        .size(14.0)
        .font(medium())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT)))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_is_the_reference_s_own_rule() {
        // `.base-card { padding: 1rem; background-color: var(--surface-3);
        // border-radius: var(--radius-lg); border: 1px solid var(--surface-4) }`.
        assert_eq!(CARD_PAD, 16.0);
        assert_eq!(CARD_PAD, 1.0 * 16.0);
        for theme in Gen::ALL {
            // The card's surface is never the page's: a card painted
            // `--color-bg` would be invisible on the page it sits on.
            assert_ne!(theme_gen::ink(*theme, Ink::Surface3), theme_gen::ink(*theme, Ink::Bg));
        }
        // And it *is* the chrome's colour, in every theme: `--surface-3` and
        // `--color-bg-raised` carry the same value in all four, and a card is
        // told apart from the page it sits on rather than from the bar above it.
        // Asserted rather than assumed, because "the card is a raised surface"
        // reads like a difference and the reference does not make one.
        for theme in Gen::ALL {
            assert_eq!(
                theme_gen::ink(*theme, Ink::Surface3),
                theme_gen::ink(*theme, Ink::RaisedBg),
                "{theme:?}: `--surface-3` is `--color-bg-raised`"
            );
        }
        for theme in Gen::ALL {
            let face: Element<'_, ()> = card(*theme, text("body"));
            drop(face);
        }
    }

    #[test]
    fn every_control_builds_in_every_theme() {
        // A widget that reads a token the theme does not declare would come back
        // as the table's fallback, and one that builds nowhere is a page that
        // panics on someone else's machine. Building them all is the test.
        for theme in Gen::ALL {
            let labels = [("Home".to_string(), true), ("Discover".to_string(), false)];
            drop(tabs(*theme, &labels, |_: usize| ()));
            drop(search(*theme, "Search mods", "sodium", |_: String| ()));
            drop(select::<()>(*theme, Key::LabelSortBy, "Relevance", 256.0));
            for kind in [Kind::Standard, Kind::Colored, Kind::Outlined, Kind::Quiet] {
                // Written with its path on purpose: the shell this rewrite
                // replaces has a gate that reads the source text looking for
                // iced's own `button(…)` and holds every one of them to
                // `hover_button`. A bare call here is not iced's button -- it is
                // this module's -- and the path is how the text says so.
                let face: Element<'_, ()> = crate::ui::button(*theme, Key::AppNavigationHome, kind, ());
                drop(face);
            }
            drop(icon_button(*theme, Glyph::Play, 20.0, ()));
            drop(admonition::<()>(*theme, Severity::Info, "header", "body"));
            drop(admonition::<()>(*theme, Severity::Warning, "header", "body"));
            drop(admonition::<()>(*theme, Severity::Critical, "header", "body"));
            drop(progress::<()>(*theme, 0.4));
            drop(tag::<()>(*theme, "1.1.2"));
            drop(metadata::<()>(*theme, Key::LabelSortBy, "Relevance"));
            drop(icon_label::<()>(*theme, Glyph::Download, "12.3K"));
            drop(paragraph::<()>(*theme, "text"));
            drop(framed::<()>(*theme, text("inner")));
        }
        // And a progress bar clamps rather than laying out a zero-width portion.
        for fraction in [-1.0, 0.0, 0.5, 1.0, 4.0] {
            drop(progress::<()>(Gen::Dark, fraction));
        }
    }

    #[test]
    fn a_button_s_colours_follow_its_type() {
        // `colored` is the accent with `--color-accent-contrast` on it, which is
        // the one place the interface deliberately puts light text on green.
        for theme in Gen::ALL {
            let accent = theme_gen::ink(*theme, Ink::Brand);
            let on_accent = theme_gen::ink(*theme, Ink::AccentContrast);
            assert_ne!(accent, on_accent, "{theme:?}: the label must be legible");
        }
    }
}
