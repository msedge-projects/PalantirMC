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
//! | [`switch`] | `base/Toggle.vue`: a 48x24 rounded track with a `--surface-5` hairline, `bg-brand` on and `bg-button-bg` off, and a 16px knob inset 4px from the edge it sits on |
//! | [`icon_box`] | `base/Avatar.vue`: a `--color-button-bg` box at `border-radius: calc(16 / 96 * size)`, with a 1px `rgb(255 255 255 / 15%)` outline at `-1px` and the picture rounded into it |
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

use iced::advanced::text::Renderer as TextRenderer;
use iced::widget::text::{Shaping, StyleSheet as TextStyle};
use iced::widget::{column, container, image, mouse_area, row, text_input, Space, Text};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Length, Padding};
use iced::{Theme, mouse::Interaction};

use crate::anim;
use crate::avatar;
use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::ROW_GAP;
use crate::style::{heading, medium, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Ink, Span, Theme as Gen};

/// The interface's text, shaped so a glyph the chosen face does not carry can
/// still be found.
///
/// Every string the shell draws comes through this rather than through
/// `iced::widget::text`, and the difference is a whole script's worth of pixels.
/// iced's `Text` defaults to `Shaping::Basic`, which its own documentation
/// describes as "no shaping and no font fallback ... will not try to find missing
/// glyphs in your system fonts" -- so a Chinese project's title, summary and body
/// drew every hanzi as `.notdef`, the filled boxes a reader calls tofu. The
/// reference never had that choice to make: it is a web view, and a browser
/// shapes through HarfBuzz and falls back through the system's fonts.
/// `Shaping::Advanced` is iced's equivalent of that, and it is the whole of the
/// difference here.
///
/// The fallback it then reaches for is `vendor/cosmic-text`'s, patched for the
/// other half of the same defect -- a request at a weight no face of the script
/// publishes had no candidate at all. See `THIRD_PARTY_NOTICES.md`.
///
/// `iced::widget::text` takes `impl ToString` and so does this, so a call site
/// reads the same either way. The test below refuses any module that imports the
/// original, which is what keeps "every string is shaped" true as pages are
/// added.
pub fn text<'a, Theme, Renderer>(text: impl ToString) -> Text<'a, Theme, Renderer>
where
    Theme: TextStyle,
    Renderer: TextRenderer,
{
    Text::new(text.to_string()).shaping(Shaping::Advanced)
}

/// How wide a label draws, in the frame that will draw it.
///
/// The number the strips below are broken on, and it is measured with iced's own
/// engine rather than estimated: `font_system` is the single
/// `cosmic_text::FontSystem` the window draws every glyph from, and
/// `to_attributes` is the very conversion the renderer applies to an
/// [`iced::Font`] before it shapes. Counting characters would be a different
/// number, and wrong by a whole word for a label like `English (United States)`.
///
/// Memoized because a view is rebuilt every frame while something is moving, and
/// the same thirty labels would otherwise be shaped thirty times a frame. The
/// table is bounded by what this interface draws: one entry per distinct string at
/// one size and one font.
pub fn advance(label: &str, font: iced::Font, size: f32) -> f32 {
    type Key = (String, u32, iced::Font);
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<Key, f32>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let key: Key = (label.to_string(), size.to_bits(), font);
    if let Ok(table) = cache.lock() {
        if let Some(width) = table.get(&key) {
            return *width;
        }
    }
    let width = shape_width(label, font, size);
    if let Ok(mut table) = cache.lock() {
        table.insert(key, width);
    }
    width
}

/// The bounds a label is measured in: wider and taller than any control one is
/// drawn in, so that a label is measured as one line. A wrapped measurement is a
/// *smaller* number, and a row broken on one would then overflow after all.
const MEASURE_SPAN: f32 = 4096.0;

/// The leading the interface's text is measured at, which is
/// `iced_core::text::LineHeight`'s own default. A control that sets a line of its
/// own sets an absolute one ([`NAV_LABEL_LINE`] and the theme cards' rows are the
/// settings dialog's), and a line is not part of a width either way: it is here
/// because the metrics have to carry a leading to shape at all.
const LEADING: f32 = 1.3;

/// One line of `label`, shaped once and measured the way iced measures a widget.
///
/// This is iced's own arithmetic rather than a second opinion about it:
/// `iced_graphics::text::Paragraph::with_text` -- the wrapper the text widget is
/// laid out as -- builds a buffer with `Metrics::new(size, line_height)` and this
/// very `LEADING`, sets its bounds, sets the attributes through the same
/// `to_attributes`, and measures it with the same `measure`. The test at the
/// bottom of this file holds the two to each other.
fn shape_width(label: &str, font: iced::Font, size: f32) -> f32 {
    use iced::advanced::graphics::text::{cosmic_text, font_system, measure, to_attributes};
    let borrowed = font_system().write();
    let mut guard = match borrowed {
        Ok(guard) => guard,
        // A panic while the window's system was borrowed somewhere else must not
        // cost the layout its measurement: recover the guard and measure anyway.
        Err(poisoned) => poisoned.into_inner(),
    };
    // iced's own `FontSystem` is a wrapper around `cosmic_text`'s -- the version
    // counter it keeps across a font load is the difference -- and `raw` is the
    // documented way to the engine underneath it.
    let system = guard.raw();
    let mut buffer =
        cosmic_text::Buffer::new(system, cosmic_text::Metrics::new(size, size * LEADING));
    buffer.set_size(system, MEASURE_SPAN, MEASURE_SPAN);
    buffer.set_text(system, label, to_attributes(font), cosmic_text::Shaping::Advanced);
    buffer.shape_until_scroll(system);
    measure(&buffer).width
}

/// How much room a button with this label needs: the label and its own padding.
///
/// [`BUTTON_LABEL_SIZE`], [`BUTTON_PAD`] and [`heading`] are what [`button_text`]
/// draws a label with, so this is the width that button will occupy rather than an
/// approximation of it.
pub fn button_width(label: &str) -> f32 {
    BUTTON_PAD * 2.0 + advance(label, heading(), BUTTON_LABEL_SIZE)
}

/// The width a row of buttons occupies: the buttons, and the gaps between them.
pub fn row_width(labels: &[impl AsRef<str>], gap: f32) -> f32 {
    let mut width = 0.0;
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            width += gap;
        }
        width += button_width(label.as_ref());
    }
    width
}

/// The rows a strip of buttons carrying `labels` breaks into, as positions.
///
/// A browser never has to make this decision: flex box carries a strip of chips
/// that does not fit onto the next line, and the reference's own grids are CSS
/// grids that cannot overflow. iced has neither -- a `Row` whose children do not
/// fit draws the last of them past its own right edge -- which is what a
/// screenshot of this launcher's language list showed: `Finnish` clipped at the
/// card's edge and `Russian` outside it, with two chips wrapping their own text
/// because a row had given them the last of its room.
///
/// So the break is the caller's, and this is how it is decided: as many buttons
/// per row as fit `avail` with `gap` between them, in order, and another row for
/// the rest. A label wider than `avail` on its own gets a row to itself rather
/// than being dropped: it is a language's name, and a name with nowhere to fit is
/// still a name.
pub fn wrap_labels(labels: &[impl AsRef<str>], avail: f32, gap: f32) -> Vec<Vec<usize>> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut row: Vec<usize> = Vec::new();
    let mut used = 0.0;
    for (index, label) in labels.iter().enumerate() {
        let width = button_width(label.as_ref());
        if !row.is_empty() && used + gap + width > avail {
            rows.push(std::mem::take(&mut row));
            used = 0.0;
        }
        used += if row.is_empty() { width } else { gap + width };
        row.push(index);
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

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
/// The size a button's label is set at, which is `Button.vue`'s `text-sm`.
///
/// A constant rather than a literal at the builders below because [`button_width`]
/// has to measure exactly what [`button_text`] draws: a strip of buttons is broken
/// on the measured widths, and a size written in two places is a size that can be
/// changed in one of them.
pub const BUTTON_LABEL_SIZE: f32 = 14.0;
/// A button's horizontal padding, which is `ButtonFrame.vue`'s `px-4`.
pub const BUTTON_PAD: f32 = 16.0;

// ---- The vertical tab list ---------------------------------------------

/// A tab row's height: `py-2`'s own eight above and below, on the 18-pixel line a
/// `text-base` label takes in the reference -- 34, which is the height its captured
/// tab plate measures (y 207..240 of a settings dialog opened at 1280x720).
pub const NAV_ITEM: f32 = 34.0;
/// The same row when it carries a badge, which is 36: the badge's own pill is
/// `text-xs`'s 16-pixel line plus `py-0.5` twice -- 20 pixels, taller than the
/// label's line -- so it is the side that sets the row's content height. Measured
/// on the reference's Language tab: its badge spans y 329..348 with the row's own
/// eight-pixel padding above it, which puts that row at 321..356.
pub const NAV_ITEM_BADGE: f32 = 36.0;
/// `rounded-xl` on a tab row.
pub const NAV_ITEM_RADIUS: f32 = 12.0;
/// `px-4` on a tab row.
pub const NAV_ITEM_PAD: f32 = 16.0;
/// `gap-2` between a tab row's icon, label and badge.
pub const NAV_ITEM_GAP: f32 = 8.0;
/// `w-4 h-4` on the icon inside a tab row.
pub const NAV_ICON: f32 = 16.0;
/// `text-base` on a tab row's label: the size `ButtonFrame.vue` gives the label of
/// every button from `md` up, and not [`BUTTON_LABEL_SIZE`].
pub const NAV_LABEL_SIZE: f32 = 16.0;
/// The line a tab row's own label is set on: an inherited 16-pixel name at the
/// browser's root `line-height: 1.15` (18.4 pixels of CSS, painted as 18 -- the
/// fraction is explained at [`crate::shell`]'s theme constants), which `py-2`
/// around it turns into the 34-pixel row the reference's captured plate measures
/// (y 207..240).
///
/// Every line height here is an [`iced::Pixels`] rather than a bare `f32`, because
/// iced's `From<f32>` for `LineHeight` is a *multiple* of the text's size: a bare
/// `18.0` on a 16-pixel label is a 288-pixel line.
pub const NAV_LABEL_LINE: f32 = 18.0;
/// `text-xs`'s own line, `1rem`, on a category heading.
pub const NAV_HEADING_LINE: f32 = 16.0;
/// `text-xs`'s own line on the badge beside a tab's label.
pub const NAV_BADGE_LINE: f32 = 16.0;

/// The hairline around a project's avatar: `Avatar.vue`'s
/// `outline: 1px solid rgb(255 255 255 / 15%)`.
///
/// A literal in the reference's stylesheet rather than one of its tokens, which is
/// why it is a literal here too: it is the same in every theme, including the one
/// whose surfaces are light.
const AVATAR_OUTLINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.15);

// ---- Surfaces ------------------------------------------------------------

/// A project's icon, in the box the reference's `Avatar` draws it in.
///
/// The box is `Avatar.vue`'s own: a `--color-button-bg` background, the 1px
/// `AVATAR_OUTLINE` hairline drawn *inside* it (`outline-offset: -1px`), and
/// `border-radius: calc(16 / 96 * size)`. The picture arrives already fitted to
/// the box and already rounded ([`crate::avatar::Icon`]), because the toolkit
/// cannot clip one; what is left for this function is the box around it.
///
/// A card with no icon -- not fetched yet, not an image, or a project that never
/// uploaded one -- draws the *same box, empty*. That is deliberate and it is what
/// the reference's placeholder sits on, but it is also the only thing that can
/// keep a card from reflowing: an icon arrives in a message of its own, and a box
/// that appeared with it would move the title a frame after the reader read it.
pub fn icon_box<'a, Message: 'a>(
    theme: Gen,
    side: f32,
    picture: Option<&avatar::Icon>,
) -> Element<'a, Message> {
    let box_size = Length::Fixed(side);
    let content: Element<'a, Message> = match picture {
        // `ContentFit::Fill` rather than `Contain`: the handle is already the
        // box's size, so the two are the same picture and this one cannot letterbox
        // it a second time.
        Some(icon) => image(icon.handle())
            .width(box_size)
            .height(box_size)
            .content_fit(ContentFit::Fill)
            .into(),
        None => Space::new(box_size, box_size).into(),
    };
    container(content)
        .width(box_size)
        .height(box_size)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
            border: Border {
                color: AVATAR_OUTLINE,
                width: 1.0,
                radius: (avatar::radius(side as u32) as f32).into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

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

/// A switch: the reference's `base/Toggle.vue`, at its own 48x24.
///
/// `Toggle.vue` is a `role="switch"` button drawing a 48x24 rounded track with a
/// hairline in `--surface-5`, `--color-brand` behind the knob when it is on and
/// `--color-button-bg` when it is off, and a 16px knob inset 4px from the edge it
/// sits on (`bg-black/90` on, `--text-secondary` off). It is a `mouse_area` here
/// rather than a `Button`, because iced's button brings a frame of its own to a
/// drawing that is already a frame.
///
/// What it does not draw is the reference's two transitions: its 200ms knob
/// travel, and the knob growing under the pointer. Both are motion this kit has
/// no value tween for yet -- the interaction clock carries hover *filters*, not
/// positions -- so the knob is drawn where it is and the switch is read at the
/// moment it is pressed rather than watched. That is a departure with a reason,
/// and the first caller that wants a moving switch should spend the tween here.
pub fn switch<'a, Message: Clone + 'a>(
    theme: Gen,
    on: bool,
    on_press: Message,
) -> Element<'a, Message> {
    const TRACK_W: f32 = 48.0;
    const TRACK_H: f32 = 24.0;
    const KNOB: f32 = 16.0;
    const INSET: f32 = 4.0;
    let knob_ink = if on {
        // `bg-black/90`, which is a colour rather than a token: the reference
        // paints the knob with a literal translucent black on both themes it
        // travels across.
        Color { r: 0.0, g: 0.0, b: 0.0, a: 0.9 }
    } else {
        theme_gen::ink(theme, INK_SECONDARY)
    };
    let knob = container(Space::new(Length::Fixed(KNOB), Length::Fixed(KNOB))).style(
        move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(knob_ink)),
            border: Border { radius: (KNOB / 2.0).into(), ..Border::default() },
            ..container::Appearance::default()
        },
    );
    let mut track = row![]
        .align_items(Alignment::Center)
        .width(Length::Fixed(TRACK_W))
        .height(Length::Fixed(TRACK_H))
        .padding(Padding { top: INSET, bottom: INSET, left: INSET, right: INSET });
    track = if on {
        track.push(Space::with_width(Length::Fill)).push(knob)
    } else {
        track.push(knob).push(Space::with_width(Length::Fill))
    };
    let track = container(track).style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(if on {
            theme_gen::ink(theme, Ink::Brand)
        } else {
            theme_gen::ink(theme, Ink::ButtonBg)
        })),
        border: Border {
            radius: (TRACK_H / 2.0).into(),
            width: 1.0,
            color: theme_gen::ink(theme, Ink::Surface5),
        },
        ..container::Appearance::default()
    });
    mouse_area(track)
        .interaction(Interaction::Pointer)
        .on_press(on_press)
        .into()
}

/// The same switch, disabled: `Toggle.vue`'s `opacity-50` on a control nobody may
/// press.
///
/// Group opacity is not something this renderer composites -- an element's `opacity`
/// would have to reach every colour drawn under it, and iced has no layer whose
/// alpha could carry it -- so the two colours the switch would draw are mixed
/// halfway into the surface they sit on instead, which is the arithmetic a
/// fifty-percent overlay does anyway. The knob is mixed into the track it sits on
/// rather than into the surface, because that is the colour it would have covered.
///
/// Measured against the reference's capture, where the settings dialog's own
/// *sync theme across devices* row is signed out: the track comes out at
/// (45, 47, 53) of a dialog at a (39, 41, 46) surface and the knob at (101, 108,
/// 118) over it, and both are under the pane's own fade in that capture.
pub fn disabled_switch<'a, Message: 'a>(theme: Gen) -> Element<'a, Message> {
    const TRACK_W: f32 = 48.0;
    const TRACK_H: f32 = 24.0;
    const KNOB: f32 = 16.0;
    const INSET: f32 = 4.0;
    let behind = theme_gen::ink(theme, Ink::RaisedBg);
    let track = mix(theme_gen::ink(theme, Ink::ButtonBg), behind, 0.5);
    let line = mix(theme_gen::ink(theme, Ink::Surface5), behind, 0.5);
    let knob = mix(theme_gen::ink(theme, INK_SECONDARY), track, 0.5);
    let knob = container(Space::new(Length::Fixed(KNOB), Length::Fixed(KNOB))).style(
        move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(knob)),
            border: Border { radius: (KNOB / 2.0).into(), ..Border::default() },
            ..container::Appearance::default()
        },
    );
    let track = container(
        row![]
            .align_items(Alignment::Center)
            .width(Length::Fixed(TRACK_W))
            .height(Length::Fixed(TRACK_H))
            .padding(Padding { top: INSET, bottom: INSET, left: INSET, right: INSET })
            .push(knob)
            .push(Space::with_width(Length::Fill)),
    )
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(track)),
        border: Border {
            radius: (TRACK_H / 2.0).into(),
            width: 1.0,
            color: line,
        },
        ..container::Appearance::default()
    });
    track.into()
}

/// Two colours, `amount` of the way from the first to the second.
///
/// Straight component interpolation, which is what a browser's own compositing
/// does between two opaque tones.
fn mix(from: Color, to: Color, amount: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: from.a + (to.a - from.a) * amount,
    }
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

/// A button whose label is a string rather than a generated key.
///
/// `Button.vue` labels a control with a message, and every other button in this
/// launcher has a [`Key`] behind its words. The language row is the one caller
/// that cannot promise one: [`crate::locale::label`] resolves the reference's own
/// `locale.<tag>` name for all 32 offered codes today, but the resolver is an
/// `Option` because the list is the vendored tree's and a code upstream adds
/// without a name must still draw. A missing name becomes its tag, and a tag is
/// not a key -- so the label is a string here, which is the whole of why this
/// constructor exists.
pub fn button_text<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    label: &str,
    kind: Kind,
    on_press: Message,
) -> Element<'a, Message> {
    let label = label.to_string();
    button_face(theme, key, kind, Length::Shrink, Some(on_press), move |ink| {
        text(label.clone())
            .size(BUTTON_LABEL_SIZE)
            .font(heading())
            .style(iced::theme::Text::Color(ink))
            .into()
    })
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
    button_face(theme, key, kind, Length::Shrink, on_press, move |ink| {
        text(label.message())
            .size(BUTTON_LABEL_SIZE)
            .font(heading())
            .style(iced::theme::Text::Color(ink))
            .into()
    })
}

/// A button with an icon in front of its label, which is what the reference's
/// own `Button` slot carries.
///
/// `ButtonFrame.vue` sizes a slot's icons from the button's size -- `md` is
/// `[&>svg]:size-5` at `gap-1.5`, so 20px in a 14px label's row -- and paints
/// them the label's own colour, which is why the kit builds the row rather than
/// leaving a caller to place an icon beside a button that would then tween
/// without it. `width` is the caller's because one of the two reference buttons
/// this exists for is `w-full` and the other is not.
pub fn button_with_icon<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    glyph: Glyph,
    label: Key,
    kind: Kind,
    width: Length,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    button_face(theme, key, kind, width, on_press, move |ink| {
        row![]
            .spacing(6.0)
            .align_items(Alignment::Center)
            .push(icon::icon(glyph, 20.0, ink))
            .push(
                text(label.message())
                    .size(BUTTON_LABEL_SIZE)
                    .font(heading())
                    .style(iced::theme::Text::Color(ink)),
            )
            .into()
    })
}

/// The body of a button: its frame, its fill, and the crossing it publishes.
///
/// `face` is handed the ink the inside paints in -- already faded if the button
/// is unusable, already filtered if the pointer is over it -- because a button's
/// label and the icon beside it are one ink. The two builders above differ in
/// what is inside that face and not in how it is painted, so the colour table
/// lives here once.
fn button_face<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    kind: Kind,
    width: Length,
    on_press: Option<Message>,
    face: impl FnOnce(Color) -> Element<'a, Message>,
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
    let face = container(face(crate::theme::brightness(ink, factor)))
        .width(width)
        .height(Length::Fixed(CONTROL))
        .padding(Padding { top: 0.0, bottom: 0.0, left: BUTTON_PAD, right: BUTTON_PAD })
        // Centred on both axes because a button's face is `justify-center` in the
        // reference whatever the slot holds; `center_x` is the one the full-width
        // buttons need, and on a shrinking one it changes nothing.
        .center_x()
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
    icon_button_kind(theme, key, glyph, size, Kind::Quiet, on_press)
}

/// The same control in one of the reference's colour presets.
///
/// `IconButton.vue` takes a `color`, and its hover is `ButtonFrame.vue`'s
/// `filled` interaction: the plate becomes the preset's colour *and* the glyph
/// becomes the accent contrast, which is what the accounts card's remove button
/// is built for -- `!bg-button-bg !text-primary` at rest, `hover:!bg-red
/// hover:!text-[var(--color-accent-contrast)]` under the pointer. Both ends are
/// mixed rather than filtered: the two ends of a `filled` hover are two colours,
/// not one colour at two brightnesses, and the reference's own transition list
/// fades one into the other over the same 150ms the clock runs.
pub fn icon_button_kind<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    glyph: Glyph,
    size: f32,
    kind: Kind,
    on_press: Message,
) -> Element<'a, Message> {
    let (factor, fraction) = interaction(key);
    let (ink, plate) = match kind {
        Kind::Danger => (
            crate::theme::mix(
                theme_gen::ink(theme, INK_DEFAULT),
                crate::theme::brightness(theme_gen::ink(theme, Ink::AccentContrast), factor),
                fraction,
            ),
            Some(crate::theme::mix(
                theme_gen::ink(theme, Ink::ButtonBg),
                crate::theme::brightness(theme_gen::ink(theme, Ink::Red), factor),
                fraction,
            )),
        ),
        // The whole control is the glyph, so the hover's own structure is the
        // raised surface appearing behind it -- `hover:bg-button-bg` in the
        // reference's own terms -- and the filter moves the glyph with it.
        _ => {
            let hovered = factor != 1.0;
            let plate = hovered.then(|| theme_gen::ink(theme, Ink::ButtonBg));
            (
                crate::theme::brightness(theme_gen::ink(theme, INK_DEFAULT), factor),
                plate.map(|plate| crate::theme::brightness(plate, factor)),
            )
        }
    };
    let face = container(icon::icon(glyph, size, ink))
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
        // `NavTabs.vue`'s two label colours exactly: the active label is
        // `text-button-textSelected` -- the brand green, `#1bd96a` in the dark
        // theme -- and an inactive one is `text-contrast`, which is the same ink
        // the rest of the shell's headings use. The port had these the other way
        // round: it plated the selected tab in `--button-bg` (`surface-4`, a
        // grey) and inked its label in contrast, so the one tab the reader is on
        // was the one tab drawn in no colour at all.
        let ink = if selected {
            theme_gen::ink(theme, Ink::ButtonTextSelected)
        } else {
            theme_gen::ink(theme, INK_CONTRAST)
        };
        // A selected tab is plated and an unselected one is not, and the
        // reference does not change that on hover: what a hover moves is the
        // *label* -- `text-secondary` to `text-primary` -- so the plate is left
        // alone and the ink is filtered.
        let (factor, _) = interaction(key);
        // The plate is `bg-button-bgSelected`, which is `--brand-highlight`:
        // the same green at a quarter alpha, so the pill reads as green without
        // becoming the solid call-to-action the Install button is.
        let plate = selected.then(|| theme_gen::ink(theme, Ink::ButtonBgSelected));
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

/// One row of a vertical tab list: `TabbedModal.vue`'s own tab button.
///
/// The reference's settings dialog is a `TabbedModal`, and its tabs are a column of
/// buttons rather than a strip of pills ([`tabs`], which is `NavTabs.vue`): the
/// classes are `flex min-w-0 shrink-0 gap-2 items-center rounded-xl px-4 py-2
/// font-semibold`, `bg-button-bgSelected text-button-textSelected` while selected and
/// `text-button-text hover:bg-button-bg hover:text-contrast` otherwise, with a
/// `w-4 h-4` icon in front of a `min-w-0 flex-1 truncate` label. `py-2` on an 18-pixel
/// line is a 34-pixel row, which is [`NAV_ITEM`]; a row whose badge is taller than
/// that line is [`NAV_ITEM_BADGE`].
///
/// No `active:scale-[0.97]` and no `tracking-wide`: this kit has no press-scale and
/// iced 0.12's text has no letter spacing. Both are recorded rather than approximated.
pub fn nav_item<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    glyph: Glyph,
    label: &str,
    badge: Option<&str>,
    selected: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let (factor, _) = interaction(key);
    let ink = if selected {
        theme_gen::ink(theme, Ink::ButtonTextSelected)
    } else {
        crate::theme::brightness(theme_gen::ink(theme, INK_CONTRAST), factor)
    };
    let mut label_row = row![]
        .align_items(Alignment::Center)
        .spacing(NAV_ITEM_GAP)
        .push(icon::icon(glyph, NAV_ICON, ink))
        .push(
            text(label.to_string())
                .size(NAV_LABEL_SIZE)
                // The line a tab row's label takes: 18 pixels of 16-pixel text,
                // which with `py-2` is the 34-pixel row the capture measures.
                // `Pixels` rather than a bare number: iced reads a `f32` line
                // height as a *multiple* of the size, so `18.0` would be a
                // 288-pixel line.
                .line_height(iced::Pixels(NAV_LABEL_LINE))
                .font(semibold())
                .width(Length::Fill)
                .style(iced::theme::Text::Color(ink)),
        );
    if let Some(badge) = badge {
        // `shrink-0 rounded-full px-1.5 py-0.5 text-xs font-bold bg-brand-highlight
        // text-brand-green`: the quarter-alpha brand pill behind the Language tab's
        // `beta`.
        label_row = label_row.push(
            container(
                text(badge.to_string())
                    .size(12.0)
                    .line_height(iced::Pixels(NAV_BADGE_LINE))
                    .font(heading())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Brand))),
            )
            .padding(Padding { top: 2.0, bottom: 2.0, left: 6.0, right: 6.0 })
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(
                    theme,
                    Ink::ColorBrandHighlight,
                ))),
                border: Border { radius: 999.0.into(), ..Border::default() },
                ..container::Appearance::default()
            }),
        );
    }
    // The pointer's crossing, as a plate that fades in: the reference transitions
    // `bg-button-bg` in over a default 150ms, and this kit's clock carries one
    // brightness factor whose travel from 1.0 to its hover end is the same 0..1.
    let hover = theme_gen::ink(theme, Ink::ButtonBg);
    let amount = ((factor - 1.0) / 0.25).clamp(0.0, 1.0);
    let plate = if selected {
        theme_gen::ink(theme, Ink::ButtonBgSelected)
    } else {
        Color { a: hover.a * amount, ..hover }
    };
    let row = container(label_row)
        .width(Length::Fill)
        .height(Length::Fixed(if badge.is_some() { NAV_ITEM_BADGE } else { NAV_ITEM }))
        .padding(Padding { top: 0.0, bottom: 0.0, left: NAV_ITEM_PAD, right: NAV_ITEM_PAD })
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(plate)),
            border: Border { radius: NAV_ITEM_RADIUS.into(), ..Border::default() },
            ..container::Appearance::default()
        });
    mouse_area(row)
        .interaction(Interaction::Pointer)
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false))
        .on_press(on_press)
        .into()
}

/// A category heading above a run of [`nav_item`]s: `TabbedModal.vue`'s own
/// `shrink-0 truncate px-4 pb-1 pt-2 text-xs font-bold uppercase tracking-wide
/// text-secondary`.
///
/// The caller uppercases the label: the reference's text transform is a CSS one, and
/// a locale's own casing is not something a drawing layer should decide.
pub fn nav_heading<'a, Message: 'a>(theme: Gen, label: &str) -> Element<'a, Message> {
    container(
        text(label.to_string())
            .size(12.0)
            // `text-xs`'s own line, so the heading's box is the reference's
            // `pt-2 pb-1` around 16 pixels. Absolute: see [`NAV_LABEL_LINE`].
            .line_height(iced::Pixels(NAV_HEADING_LINE))
            // `font-bold`, not the `font-semibold` the tab rows' own labels carry:
            // the reference weights the category heading above a run of tabs one step
            // heavier than the tabs.
            .font(heading())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
    )
    .width(Length::Fill)
    .padding(Padding { top: 8.0, bottom: 4.0, left: 16.0, right: 16.0 })
    .into()
}

/// A row of chips: `Chips.vue`, which is the reference's selectable pill.
///
/// The creation flow's custom step makes both of its choices this way -- the
/// modloader and which kind of loader build -- and both are the same picture: a
/// standard button at rest, and `bg-brand-highlight text-brand` with a brand
/// hairline and a check glyph while chosen. The row does not keep itself to one
/// choice; that is the caller's rule, because the reference's own `never-empty`
/// prop leaves it to the flow whether pressing the chosen chip takes it away.
///
/// A chip whose press is `None` is one the reference disables rather than hides,
/// which is how its loader-version row says *this loader published nothing
/// stable*: the chip keeps its place and goes dim, so the row does not reflow
/// when the answer arrives.
pub fn chips<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    keys: &[&'static str],
    labels: &[(String, bool)],
    on_select: impl Fn(usize) -> Option<Message>,
) -> Element<'a, Message> {
    let mut row = row![].spacing(8.0).align_items(Alignment::Center);
    for (index, ((label, selected), key)) in labels.iter().zip(keys.iter().copied()).enumerate() {
        let selected = *selected;
        let press = on_select(index);
        let usable = press.is_some();
        let (factor, _) = if usable { interaction(key) } else { (1.0, 0.0) };
        let (background, border, ink) = if selected {
            (
                theme_gen::ink(theme, Ink::ColorBrandHighlight),
                theme_gen::ink(theme, Ink::Brand),
                theme_gen::ink(theme, Ink::Brand),
            )
        } else {
            (
                theme_gen::ink(theme, Ink::ButtonBg),
                Color::TRANSPARENT,
                theme_gen::ink(theme, INK_CONTRAST),
            )
        };
        let ink = if usable { ink } else { crate::style::faded(ink) };
        // The check is the selected chip's own, and it is the reference's: a
        // colour alone would leave a choice that only reads as a choice to
        // somebody who can see it.
        let mut face = row![].spacing(6.0).align_items(Alignment::Center).width(Length::Shrink);
        if selected {
            face = face.push(icon::icon(Glyph::Check, 16.0, crate::theme::brightness(ink, factor)));
        }
        face = face.push(
            text(label.clone())
                .size(14.0)
                .font(heading())
                .style(iced::theme::Text::Color(crate::theme::brightness(ink, factor))),
        );
        let chip = container(face)
            .height(Length::Fixed(CONTROL))
            // 10px rather than the button's 16: the reference's own chips are
            // `!px-2.5`, which is the padding that keeps a row of five inside
            // the dialog it is drawn in.
            .padding(Padding { top: 0.0, bottom: 0.0, left: 10.0, right: 10.0 })
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(crate::theme::brightness(background, factor))),
                border: Border {
                    color: if usable { border } else { crate::style::faded(border) },
                    width: 1.0,
                    radius: CONTROL_RADIUS.into(),
                },
                ..container::Appearance::default()
            });
        let area = mouse_area(chip)
            .interaction(if usable { Interaction::Pointer } else { Interaction::Idle })
            .on_enter(Message::hover(key, true))
            .on_exit(Message::hover(key, false));
        row = row.push(match press {
            Some(press) => area.on_press(press),
            None => area,
        });
    }
    row.into()
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
    use iced::advanced::graphics::text::cosmic_text;

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

    /// Every Rust file in this crate, so the gate below reads the tree that was
    /// compiled rather than a list of modules that goes stale as pages are added.
    fn crate_sources() -> Vec<(String, String)> {
        fn walk(dir: &std::path::Path, root: &std::path::Path, found: &mut Vec<(String, String)>) {
            let entries = match std::fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(_) => return,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, root, found);
                } else if path.extension().is_some_and(|kind| kind == "rs") {
                    if let Ok(source) = std::fs::read_to_string(&path) {
                        let name = path.strip_prefix(root).unwrap_or(&path).display().to_string();
                        found.push((name, source));
                    }
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        walk(&root, &root, &mut found);
        found
    }

    #[test]
    fn a_line_height_is_an_absolute_length_and_never_a_multiplier() {
        // iced's `From<f32> for LineHeight` is `Relative`: a bare number is a
        // multiple of the text's size, so `18.0` on a 16-pixel label is a 288-pixel
        // line. The settings dialog was drawn with five of those before this gate
        // -- the tab headings, the card names and the modal's own title each
        // measured their size times their line -- and a multiplier is not a typo
        // the compiler can see. So the argument has to be `iced::Pixels`, iced's
        // absolute form, and the scan covers every source in this crate rather than
        // the two that had the defect. The needle is assembled from two pieces so
        // that this test's own text is not one of the sites.
        let needle = concat!(".line_height", "(");
        for (name, source) in crate_sources() {
            for (index, _) in source.match_indices(needle) {
                let rest = &source[index + needle.len()..];
                let argument = rest.split(')').next().unwrap_or("");
                assert!(
                    argument.contains("Pixels"),
                    "{name}.rs: a line height of `{argument}` is a multiplier, not a length"
                );
            }
        }
    }

    #[test]
    fn the_number_a_row_is_broken_on_is_the_width_iced_lays_the_label_out_at() {
        // The one thing the test below cannot say by itself: that [`advance`] is the
        // width iced will draw the label at, rather than a second opinion about it.
        // `Paragraph` is the wrapper a text widget is laid out as, so this is that
        // parity spelled out -- and it is the claim the whole fix rests on, because
        // a row broken on a number smaller than the real one is a row drawn past
        // the card again.
        use iced::advanced::graphics::text::Paragraph;
        use iced::advanced::text::{LineHeight, Paragraph as _, Text as Laid};

        for label in ["Dark", "Sync with system", "English (United States)", "简体中文"] {
            let font = heading();
            let laid = Laid {
                content: label,
                bounds: iced::Size::new(MEASURE_SPAN, MEASURE_SPAN),
                size: iced::Pixels(BUTTON_LABEL_SIZE),
                line_height: LineHeight::default(),
                font,
                horizontal_alignment: iced::alignment::Horizontal::Left,
                vertical_alignment: iced::alignment::Vertical::Top,
                shaping: Shaping::Advanced,
            };
            let iced = Paragraph::with_text(laid).min_bounds().width;
            let measured = advance(label, font, BUTTON_LABEL_SIZE);
            assert!(
                (iced - measured).abs() < 0.01,
                "`{label}` measures {measured} here and {iced} in iced's own paragraph"
            );
        }
    }

    #[test]
    fn a_row_of_buttons_broken_for_a_width_measures_within_that_width() {
        // The gate behind [`wrap_labels`]: iced wraps nothing, so the break is
        // arithmetic on the measured widths, and this is that arithmetic read
        // back. Both halves of the claim, because either alone is satisfied by a
        // grid that is wrong -- nothing dropped, reordered or doubled, and no row
        // measuring wider than the space it was broken for.
        let labels: Vec<String> = crate::locale::OFFERED
            .iter()
            .map(|tag| crate::locale::label(tag))
            .collect();
        assert!(
            button_width(&labels[0]) > BUTTON_PAD * 2.0,
            "a label with no width would make every wrap free and this gate empty"
        );
        // The dialog's own inner width -- the real one, bar reserved and all
        // (`crate::shell::DIALOG_INNER`) -- a column narrower, and a strip too
        // narrow for one button: the last one is the case where a row holds a
        // single label wider than the space it was broken for.
        for avail in [crate::shell::DIALOG_INNER, 240.0, 100.0] {
            let rows = wrap_labels(&labels, avail, ROW_GAP);
            let flattened: Vec<usize> = rows.iter().flatten().copied().collect();
            assert_eq!(
                flattened,
                (0..labels.len()).collect::<Vec<usize>>(),
                "every label is offered once, in the reference's order, at {avail}"
            );
            for row in &rows {
                let row_labels: Vec<&str> = row.iter().map(|&index| labels[index].as_str()).collect();
                let width = row_width(&row_labels, ROW_GAP);
                // A row of *one* label that is wider than the space is the case
                // [`wrap_labels`] documents: a name with nowhere to fit still gets
                // a row, and it is the only row this test lets past the edge.
                assert!(
                    width <= avail || row.len() == 1,
                    "{row_labels:?} measures {width} at gap {ROW_GAP}, which does not fit {avail}"
                );
            }
        }
    }

    #[test]
    fn every_string_the_shell_draws_is_shaped_through_this_module() {
        // The gate behind [`text`] above. iced's own `Text` draws with
        // `Shaping::Basic`, which never looks a glyph up in another font, so one
        // module that imports it is one page that draws tofu again -- and nothing
        // about the widget, the size or the colour would say so. Read as text
        // because that is what the mistake is, an import line, and read from the
        // directory so a page added later cannot escape it.
        let mut offenders = Vec::new();
        for (path, source) in crate_sources() {
            let mut importing = false;
            for (index, line) in source.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                if trimmed.starts_with("use iced::widget") {
                    importing = true;
                }
                let at = index + 1;
                if importing {
                    let names_text = line
                        .split(',')
                        .flat_map(|item| item.split_whitespace())
                        .any(|item| item == "text");
                    if names_text {
                        offenders.push(format!("{path}:{at}: {}", trimmed.trim_end()));
                    }
                    if trimmed.ends_with(';') {
                        importing = false;
                    }
                }
                // The needle is spelled in two pieces so that this test does
                // not read its own source as a call site.
                if line.contains(concat!("iced::widget::", "text(")) {
                    offenders.push(format!("{path}:{at}: {}", trimmed.trim_end()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these draw through iced's own `text`, which never falls back to a face \
             that has the glyph; every string goes through `ui::text`: {:?}",
            offenders
        );
        // And that helper is the shaped one -- the gate above only says whose
        // door the strings come through.
        assert!(
            include_str!("ui.rs").contains(".shaping(Shaping::Advanced)"),
            "`ui::text` is the whole reason every string is shaped; it is not shaped any more"
        );
    }

    /// How many of `text`'s glyphs came out as `.notdef` at this weight, and
    /// which faces drew it.
    fn shape(
        system: &mut cosmic_text::FontSystem,
        weight: cosmic_text::Weight,
        text: &str,
    ) -> (usize, Vec<String>) {
        let mut buffer = cosmic_text::Buffer::new(system, cosmic_text::Metrics::new(16.0, 20.0));
        buffer.set_size(system, 900.0, 900.0);
        buffer.set_text(
            system,
            text,
            cosmic_text::Attrs::new()
                .family(cosmic_text::Family::Name(crate::style::FAMILY))
                .weight(weight),
            cosmic_text::Shaping::Advanced,
        );
        buffer.shape_until_scroll(system);
        let mut missing = 0;
        let mut faces = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                if glyph.glyph_id == 0 {
                    missing += 1;
                }
                if let Some(face) = system.db().face(glyph.font_id) {
                    let name = format!(
                        "{} {}",
                        face.families.first().map(|(name, _)| name.as_str()).unwrap_or("?"),
                        face.weight.0
                    );
                    if !faces.contains(&name) {
                        faces.push(name);
                    }
                }
            }
        }
        (missing, faces)
    }

    /// The system the launcher draws in: the five bundled Inter weights, and
    /// whatever this machine has installed.
    fn app_font_system() -> cosmic_text::FontSystem {
        cosmic_text::FontSystem::new_with_fonts(crate::FONTS.iter().map(|bytes| {
            cosmic_text::fontdb::Source::Binary(
                std::sync::Arc::new(bytes.to_vec())
                    as std::sync::Arc<dyn AsRef<[u8]> + Send + Sync>,
            )
        }))
    }

    #[test]
    fn a_hanzi_is_found_at_every_weight_the_interface_sets_text_at() {
        // The measurement both halves of the fix rest on, taken through the same
        // engine the window draws with.
        //
        // iced reaches another font only under `Shaping::Advanced`, which is what
        // [`text`] above asks for, and the fallback it then looks for is only
        // reachable because `vendor/cosmic-text` offers every installed face
        // instead of the ones whose weight equals the request. Windows publishes
        // its CJK faces at 400 and 700 alone while the interface sets text at
        // five weights, so without either half a hanzi is a filled box: the
        // screenshot that started this was a Chinese modpack's own description.
        let mut system = app_font_system();
        let hanzi = "简体中文";
        let (control, control_faces) = shape(&mut system, cosmic_text::Weight::NORMAL, hanzi);
        if control > 0 {
            // A machine with no CJK face has nothing for a hanzi to fall back
            // to. That is a fact about the machine, not a failure of this test,
            // and saying so is better than a green test that measured nothing.
            eprintln!("no installed face covers {hanzi}: {control_faces:?}");
            return;
        }
        for weight in [500u16, 600, 700, 800] {
            let (missing, faces) = shape(&mut system, cosmic_text::Weight(weight), hanzi);
            assert_eq!(
                missing, 0,
                "weight {weight} draws {missing} tofu glyph(s) of {hanzi}, from {faces:?}"
            );
        }
    }

    #[test]
    fn every_control_a_page_draws_carries_its_own_key() {
        // The gate the old shell has and this kit needs: a control built without
        // a key is a control that snaps while the rest of the window tweens, and
        // nothing about the widget would say so. Read as text because that is
        // what the mistake is -- a call site whose second argument is still the
        // label, which is exactly the shape this test refuses.
        for (page, source) in page_sources() {
            for call in [
                "ui::button(",
                "ui::button_with_icon(",
                "ui::tabs(",
                "ui::icon_button(",
                "ui::icon_button_kind(",
            ] {
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
