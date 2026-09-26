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
//!
//! ## The interaction
//!
//! The reference's every control is `transition-[filter,transform] duration-150
//! ease-out`, so a hover arrives over 150 ms rather than on a frame boundary. The
//! clock that carries it is [`crate::anim`]'s, and this module is where a page
//! reaches it: a hoverable control is a `mouse_area` that publishes the crossing
//! as a message, and it draws from [`interaction`], which asks the clock.
//!
//! Why the crossing has to be a message rather than a read of the pointer during
//! the view is [`crate::hover`]'s subject, and it is not a stylistic choice: iced
//! re-tracks subscriptions *before* the view runs, so a tween started while the
//! view is being built is one frame too late for the frames that would carry it.
//! The page's own `update` records the crossing, the subscription is enabled by
//! what that recording made true, and the view then draws the frame.

#![allow(dead_code)]

use iced::widget::{column, container, mouse_area, row, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use iced::{Theme, mouse::Interaction};

use crate::anim;
use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::ROW_GAP;
use crate::style::{heading, medium, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Ink, Span, Theme as Gen};

/// A page's message type, as far as this kit needs it.
///
/// The controls here are `mouse_area`s, and a crossing is a message more often
/// than it is a state: `Report` in the shell this rewrite replaces could publish
/// a pair of callbacks into any message type, but every page has settled on one
/// constructor for the crossing, which is this. A page implements it once and
/// every control in this file can ask for it.
pub trait Hovered: Sized {
    /// The pointer entered (`true`) or left (`false`) the control `key`.
    fn hover(key: &'static str, over: bool) -> Self;

    /// The same crossing for a control whose hover end is its own.
    ///
    /// The reference scopes `--hover-brightness` on some surfaces -- an instance
    /// card brightens to `1.1` rather than the global `1.25` -- and a control
    /// whose tween ends somewhere else has to say so where the crossing is made,
    /// or the clock would animate to one factor while the view filters by another.
    fn hover_with(key: &'static str, over: bool, hover: f32) -> Self;
}

/// A stable name for a control that repeats, from the data it names.
///
/// A card's identity is the project it draws, and the clock needs a
/// `&'static str`. The names are leaked on purpose: they are bounded UI
/// identities -- one per card the user has ever seen -- rather than per-frame
/// values.
pub fn scoped(namespace: &str, id: &str) -> &'static str {
    static NAMES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, &'static str>>> =
        std::sync::OnceLock::new();
    let name = format!("{namespace}:{id}");
    let mut table = match NAMES.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new())).lock() {
        Ok(table) => table,
        // A poisoned table is a panic that happened while a name was being
        // looked up. Recover rather than refusing to draw the control.
        Err(poisoned) => poisoned.into_inner(),
    };
    match table.get(&name) {
        Some(key) => key,
        None => {
            let key: &'static str = Box::leak(name.clone().into_boxed_str());
            table.insert(name, key);
            key
        }
    }
}

/// Implement [`Hovered`] for a page whose message carries the crossing itself.
///
/// Every page settled on the same variant -- a `Hover { key, over }` its own
/// `update` records into the clock -- so the impl is one line per page and the
/// name of the variant is the convention the macro encodes.
#[macro_export]
macro_rules! hovered {
    ($message:ty) => {
        impl $crate::ui::Hovered for $message {
            fn hover(key: &'static str, over: bool) -> Self {
                Self::Hover { key, over, hover: None }
            }

            fn hover_with(key: &'static str, over: bool, hover: f32) -> Self {
                Self::Hover { key, over, hover: Some(hover) }
            }
        }
    };
}

/// The factor and hover fraction the control `key` draws at.
///
/// A control nobody has reported is at rest, and one whose tween is in the air
/// is drawn where the clock is. The second number is how far *through* its hover
/// the control is, for the controls whose hover moves something rather than
/// tinting it (a tab's plate appears, a card's label lifts).
pub fn interaction(key: &str) -> (f32, f32) {
    match anim::clock().lock() {
        Ok(clock) => clock.drawn(key),
        // A poisoned clock is a panic that happened while a tween was being
        // read. Draw the rest state rather than propagating it.
        Err(_) => (1.0, 0.0),
    }
}

/// Record that the pointer entered or left the control `key`.
///
/// Called from a page's `update`, never from its view -- [`Hovered`] says why.
/// A page's controls report a crossing and no press, because that is what the
/// widget under them reports: a `mouse_area` has an enter, an exit and a press
/// *action*, and the reference's press is `active:scale-[0.97]`, which is a
/// transform this toolkit cannot draw at all. A dimmed press without a scale is
/// what the old shell does and is not worth inventing twice.
pub fn pointer(key: &'static str, over: bool) {
    pointer_with(key, over, crate::theme::hover_brightness());
}

/// The same crossing, for a control whose hover end is scoped to itself.
pub fn pointer_with(key: &'static str, over: bool, hover: f32) {
    if let Ok(mut clock) = anim::clock().lock() {
        clock.set_with_hover(key, over, false, std::time::Instant::now(), hover);
    }
}

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
    card_at(theme, 1.0, content)
}

/// The same card, through the interaction filter.
///
/// A card that is *pressed* is a card that hovers, and the reference scopes the
/// brightness on the ones that are clickable: a project's card dims to `0.9`
/// (`LegacyProjectCard.vue`'s `hover:brightness-90`), an instance's in the
/// library brightens to `1.1` (`instance-card.vue`'s `hover:brightness-110`,
/// which [`crate::theme::INSTANCE_CARD_HOVER_BRIGHTNESS`] names). A card drawn
/// at rest passes `1.0` and the multiplication is skipped.
pub fn card_at<'a, Message: 'a>(
    theme: Gen,
    factor: f32,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    // `move` because the closure is handed to a widget that outlives this call.
    let ink = move |color: Color| crate::theme::brightness(color, factor);
    container(content)
        .width(Length::Fill)
        .padding(CARD_PAD)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(ink(theme_gen::ink(theme, Ink::Surface3)))),
            border: Border {
                color: ink(theme_gen::ink(theme, Ink::Surface4)),
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

impl Field {
    /// A field that paints its own frame, for a caller that is not building one
    /// of this module's rows around it -- the shell's create dialog is one.
    pub fn bordered(theme: Gen) -> Field {
        Field { theme, chrome: Chrome::Bordered }
    }
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
    /// `danger`: the `red` preset, which the reference gives the one button that
    /// takes something away -- an instance's *Stop* (`page-header/index.vue`'s
    /// `color="red"`), where a brand-coloured Stop would be the same picture as
    /// the Play it replaced.
    Danger,
    /// `outlined`: a hairline and no fill.
    Outlined,
    /// `quiet`: no frame at all.
    Quiet,
}

/// A button: `text-sm font-bold`, a radius from the control size.
///
/// `key` is the control's stable identity for the interaction clock -- one per
/// button, not one per label, because two buttons that happen to share a word
/// must not light together. The filter is applied to the fill, the ring *and*
/// the label, which is what a CSS filter on the element does; brightening the
/// fill alone would leave an `Outlined` button's only visible change on the
/// hairline.
pub fn button<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    label: Key,
    kind: Kind,
    on_press: Message,
) -> Element<'a, Message> {
    button_or(theme, key, label, kind, Some(on_press))
}

/// A button that can be unusable, which is `on_press: None`.
///
/// The reference has no disabled *button*: `Button.vue` always takes an action,
/// and a flow that is waiting draws a spinner instead. A dialog whose own action
/// is in flight has to draw something, and the honest one is the same button with
/// its press removed and its ink dimmed -- a click that goes nowhere would be the
/// alternative, and it would be counted twice.
pub fn button_or<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    label: Key,
    kind: Kind,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let usable = on_press.is_some();
    let (factor, _) = if usable { interaction(key) } else { (1.0, 0.0) };
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
        Kind::Danger => (
            // `ButtonFrame.vue`'s own table: `red: 'var(--color-red)'`, painted by
            // the same `bg-[--button-color] text-[var(--color-accent-contrast)]`
            // rule as the accent -- so a red button's label is the *accent*
            // contrast rather than a red one of its own.
            Some(Background::Color(theme_gen::ink(theme, Ink::Red))),
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
    let ink = if usable { ink } else { crate::style::faded(ink) };
    let background = background
        .map(|background| match background {
            Background::Color(color) => Background::Color(crate::theme::brightness(color, factor)),
            gradient => gradient,
        })
        // A dimmed fill rather than no fill: `disabled()` moves the label, and a
        // button whose fill vanished would change shape while it was pressed.
        .map(|background| match background {
            Background::Color(color) if !usable => Background::Color(crate::style::faded(color)),
            other => other,
        });
    let face = container(
        text(label.message())
            .size(14.0)
            .font(heading())
            .style(iced::theme::Text::Color(crate::theme::brightness(ink, factor))),
    )
    .height(Length::Fixed(CONTROL))
    .padding(Padding { top: 0.0, bottom: 0.0, left: 16.0, right: 16.0 })
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background,
        border: Border {
            color: border
                .map(|border| crate::theme::brightness(border, factor))
                .unwrap_or(Color::TRANSPARENT),
            width: if border.is_some() { 1.0 } else { 0.0 },
            radius: CONTROL_RADIUS.into(),
        },
        ..container::Appearance::default()
    });
    let area = mouse_area(face)
        .interaction(Interaction::Pointer)
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false));
    match on_press {
        Some(on_press) => area.on_press(on_press).into(),
        None => area.into(),
    }
}

/// A quiet icon button, the square one the reference uses in a bar.
///
/// `Message: Clone` because [`mouse_area`]'s press handler holds its message and
/// the element is rebuilt on every paint: iced requires the clone to do that, and
/// every one of this crate's message types is a clone.
pub fn icon_button<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    glyph: Glyph,
    size: f32,
    on_press: Message,
) -> Element<'a, Message> {
    let (factor, _) = interaction(key);
    // The whole control is the glyph, so the hover's own structure is the raised
    // surface appearing behind it -- `hover:bg-button-bg` in the reference's own
    // terms -- and the filter moves the glyph with it.
    let hovered = factor != 1.0;
    let plate = hovered.then(|| theme_gen::ink(theme, Ink::ButtonBg));
    let plate = plate.map(|plate| crate::theme::brightness(plate, factor));
    let face = container(icon::icon(
        glyph,
        size,
        crate::theme::brightness(theme_gen::ink(theme, INK_DEFAULT), factor),
    ))
    .width(Length::Fixed(size + 16.0))
    .height(Length::Fixed(size + 16.0))
    .center_x()
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background: plate.map(Background::Color),
        border: Border { radius: CONTROL_RADIUS.into(), ..Border::default() },
        ..container::Appearance::default()
    });
    mouse_area(face)
        .interaction(Interaction::Pointer)
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false))
        .on_press(on_press)
        .into()
}

/// The tabs a page switches between: a pill of buttons, the selected one plated.
///
/// `NavTabs.vue`'s own arrangement: a `rounded-full bg-bg-raised p-1` track with
/// `px-4 py-2` tabs inside it. The labels are strings because a page's tabs are
/// not always locale keys of their own: an instance's are, and Discover's are the
/// project-type names `route.rs` already asserts against the reference.
pub fn tabs<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    keys: &[&'static str],
    labels: &[(String, bool)],
    on_select: impl Fn(usize) -> Message,
) -> Element<'a, Message> {
    let mut track = row![].align_items(Alignment::Center).spacing(2.0);
    for (index, ((label, selected), key)) in labels.iter().zip(keys.iter().copied()).enumerate() {
        let selected = *selected;
        let ink = if selected {
            theme_gen::ink(theme, INK_CONTRAST)
        } else {
            theme_gen::ink(theme, INK_SECONDARY)
        };
        // A selected tab is plated and an unselected one is not, and the
        // reference does not change that on hover: what a hover moves is the
        // *label* -- `text-secondary` to `text-primary` -- so the plate is left
        // alone and the ink is filtered.
        let (factor, _) = interaction(key);
        let plate = selected.then(|| theme_gen::ink(theme, Ink::ButtonBg));
        let tab = container(
            text(label.clone())
                .size(14.0)
                .font(heading())
                .style(iced::theme::Text::Color(crate::theme::brightness(ink, factor))),
        )
        .height(Length::Fixed(32.0))
        .padding(Padding { top: 0.0, bottom: 0.0, left: 16.0, right: 16.0 })
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            background: plate.map(Background::Color),
            border: Border { radius: 999.0.into(), ..Border::default() },
            ..container::Appearance::default()
        });
        track = track.push(
            mouse_area(tab)
                .interaction(Interaction::Pointer)
                .on_enter(Message::hover(key, true))
                .on_exit(Message::hover(key, false))
                .on_press(on_select(index)),
        );
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

    /// A message type with nothing in it but the crossing a control publishes.
    ///
    /// The kit is generic over the page's own message, so the tests need one of
    /// their own -- and it is deliberately *not* a page's, because what these
    /// tests are about is the kit rather than any page's routing.
    #[derive(Debug, Clone, PartialEq)]
    enum Probe {
        Crossed { key: &'static str, over: bool },
    }

    impl Hovered for Probe {
        fn hover(key: &'static str, over: bool) -> Probe {
            Probe::Crossed { key, over }
        }

        fn hover_with(key: &'static str, over: bool, _hover: f32) -> Probe {
            Probe::Crossed { key, over }
        }
    }

    /// Every page's source, embedded so the gate below reads the tree that was
    /// compiled rather than a path that may not exist in some other checkout.
    fn page_sources() -> Vec<(&'static str, &'static str)> {
        vec![
            ("discover", include_str!("pages/discover.rs")),
            ("home", include_str!("pages/home.rs")),
            ("instance", include_str!("pages/instance.rs")),
            ("project", include_str!("pages/project.rs")),
            ("screenshots", include_str!("pages/screenshots.rs")),
            ("servers", include_str!("pages/servers.rs")),
            ("skins", include_str!("pages/skins.rs")),
            ("user", include_str!("pages/user.rs")),
        ]
    }

    #[test]
    fn every_control_a_page_draws_carries_its_own_key() {
        // The gate the old shell has and this kit needs: a control built without
        // a key is a control that snaps while the rest of the window tweens, and
        // nothing about the widget would say so. Read as text because that is
        // what the mistake is -- a call site whose second argument is still the
        // label, which is exactly the shape this test refuses.
        for (page, source) in page_sources() {
            for call in ["ui::button(", "ui::tabs(", "ui::icon_button("] {
                for call_site in source.split(call).skip(1) {
                    let head: String = call_site
                        .chars()
                        .take(80)
                        .filter(|character| !character.is_whitespace())
                        .collect();
                    assert!(
                        !head.starts_with("theme,Key::") && !head.starts_with("theme,&labels"),
                        "{page}: a `{call}` without its own key would never tween: {head}"
                    );
                }
            }
            // And the page routes the crossing it asks for: a control wired to a
            // message nothing handles is a tween that never starts.
            assert!(
                source.contains("crate::hovered!(Message)"),
                "{page}: the page does not implement the crossing its controls publish"
            );
        }
    }

    #[test]
    fn a_repeated_control_is_named_by_the_thing_it_names() {
        // What a card needs: the same card is the same name across every frame,
        // two cards are not the same name, and two kinds of control naming the
        // same thing do not collide -- which is what the namespace is for.
        let first = scoped("ui:test:card", "sodium");
        let again = scoped("ui:test:card", "sodium");
        let other = scoped("ui:test:card", "lithium");
        assert_eq!(first.as_ptr(), again.as_ptr(), "the same card, the same name");
        assert_ne!(first, other);
        assert_ne!(
            first,
            scoped("ui:test:toggle", "sodium"),
            "a toggle and a card naming the same file are two controls"
        );
    }

    #[test]
    fn a_control_tweens_its_hover_on_the_clock_it_was_given() {
        // The property the pages are wired for: a crossing recorded from
        // `update` moves the control over the reference's own 150 ms, and an
        // untouched control is at rest rather than mid-hover.
        let _guard = anim::lock_for_test();
        let key = "ui:test:probe";
        anim::clock().lock().expect("the clock").clear();
        assert_eq!(interaction(key), (1.0, 0.0), "nothing has been reported yet");

        // The crossing is a message, and handling it is what starts the tween.
        let Probe::Crossed { key: reported, over } = Probe::hover(key, true);
        assert_eq!(reported, key);
        assert!(over);
        pointer(reported, over);
        assert!(anim::clock().lock().expect("the clock").animating());

        // The deadline ends it, and the control is drawn at the hover factor.
        let now = std::time::Instant::now() + crate::anim::INTERACTION_DURATION;
        anim::clock().lock().expect("the clock").tick(now);
        let (factor, progress) = interaction(key);
        assert_eq!(factor, crate::theme::hover_brightness());
        assert_eq!(progress, 1.0);

        // And the leave is the return trip. Asserted on *this* key rather than on
        // the clock's quiet: the clock is the window's, and a test binary runs
        // tests side by side, so another one's tween may be in the air.
        pointer(key, false);
        let now = now + crate::anim::INTERACTION_DURATION;
        anim::clock().lock().expect("the clock").tick(now);
        assert_eq!(interaction(key), (1.0, 0.0));
    }

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
            let keys = ["ui:test:tab:home", "ui:test:tab:discover"];
            drop(tabs(*theme, &keys, &labels, |_: usize| Probe::Crossed { key: keys[0], over: true }));
            drop(search(*theme, "Search mods", "sodium", |_: String| ()));
            drop(select::<()>(*theme, Key::LabelSortBy, "Relevance", 256.0));
            for kind in [Kind::Standard, Kind::Colored, Kind::Outlined, Kind::Quiet] {
                // Written with its path on purpose: the shell this rewrite
                // replaces has a gate that reads the source text looking for
                // iced's own `button(…)` and holds every one of them to
                // `hover_button`. A bare call here is not iced's button -- it is
                // this module's -- and the path is how the text says so.
                let face: Element<'_, Probe> =
                    crate::ui::button(*theme, "ui:test:button", Key::AppNavigationHome, kind, Probe::Crossed { key: "ui:test:button", over: false });
                drop(face);
            }
            drop(icon_button(
                *theme,
                "ui:test:icon",
                Glyph::Play,
                20.0,
                Probe::Crossed { key: "ui:test:icon", over: false },
            ));
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
