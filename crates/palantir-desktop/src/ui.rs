//! The widgets the pages are drawn from, each one quoting the reference's own
//! class or rule rather than inventing a look.
//!
//! | Widget | Reference |
//! | --- | --- |
//! | [`card`] | `.base-card` in `assets/styles/classes.scss`: `padding: 1rem`, `background-color: var(--surface-3)`, `border-radius: var(--radius-lg)`, `border: 1px solid var(--surface-4)` |
//! | [`tabs`] | `base/NavTabs.vue`: `rounded-full bg-bg-raised p-1` inside `border border-solid border-surface-4`, each tab `px-4 py-2 font-bold`; the `card-shadow` on the same element is **not** drawn, for [`card_shadow`] |
//! | [`tag`] | `base/TagItem.vue`: `bg-button-bg border-surface-5 border-[1px] px-2 py-1 leading-none rounded-full text-sm font-normal`, the label in `text-secondary` |
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
use crate::style::{heading, inter, medium, regular, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
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
/// A tag's label size, which is `TagItem.vue`'s `text-sm` at the reference's
/// sixteen-pixel root.
pub const TAG_LABEL_SIZE: f32 = 14.0;
/// `TagItem.vue`'s `px-2`, a tag's padding either side of its content.
pub const TAG_PAD: f32 = 8.0;
/// `TagItem.vue`'s `gap-1`, between a tag's icon and its label.
pub const TAG_GAP: f32 = 4.0;
/// The size of the glyph `TagItem.vue`'s `baseClass` sizes: `[&>svg]:shrink-0
/// [&>svg]:h-4 [&>svg]:w-4`, which is sixteen pixels.
pub const TAG_ICON: f32 = 16.0;
/// A tag with no icon in it: `border-[1px] border-solid` and `py-1` around a
/// `text-sm` label on `leading-none`, which is 1 + 4 + 14 + 4 + 1.
pub const TAG_HEIGHT: f32 = 24.0;
/// The same pill with the `h-4` glyph in front of the label, which is the taller
/// of the two children and so the height: 1 + 4 + 16 + 4 + 1.
///
/// This is not a second height this port chooses between. The sixteen-pixel icon
/// and the fourteen-pixel line sit side by side in one `inline-flex` box with
/// `items-center`, so the pill is as tall as the taller of them. Measured on the
/// reference's own /user/FlameFire capture at 1280x720, over the eighteen pills of
/// the three cards on it: *Client and server*, *Forge*, *Modpack*, *Server*,
/// *Fabric*, *Forge* and *Data Pack* are 26 rows, and *Challenging*, *Combat*,
/// *Minigame*, *World Generation*, *Mobs*, *+1* and *+3* are 24. Every one of the
/// 26 is a tag that draws an icon and every one of the 24 is a tag that does not.
pub const TAG_HEIGHT_ICON: f32 = 26.0;
/// The pill's height for a tag that does or does not carry an icon, which is the
/// one number a caller laying out a row of tags needs.
///
/// A caller that has already decided whether its tag draws a glyph asks here
/// rather than picking a constant, because the two heights are the two children of
/// the same flex line and a row built from one and not the other is a row with a
/// one-pixel jog in it.
pub const fn tag_height(icon: bool) -> f32 {
    if icon {
        TAG_HEIGHT_ICON
    } else {
        TAG_HEIGHT
    }
}
/// The size a button's label is set at, which is `Button.vue`'s `text-sm`.
///
/// A constant rather than a literal at the builders below because [`button_width`]
/// has to measure exactly what [`button_text`] draws: a strip of buttons is broken
/// on the measured widths, and a size written in two places is a size that can be
/// changed in one of them.
pub const BUTTON_LABEL_SIZE: f32 = 14.0;
/// A button's horizontal padding, which is `ButtonFrame.vue`'s `px-4`.
///
/// The legacy frame's: [`button`] and the three builders beside it draw every
/// button as this one row, 40 pixels tall with a 14-pixel label, which is the
/// geometry the kit had before [`Size`] was ported. Callers move onto the sized
/// builders as their surface is measured, and the four legacy constructors go
/// with the last of them.
pub const BUTTON_PAD: f32 = 16.0;

// ---- `ButtonFrame.vue`'s own size table ----------------------------------

/// One row of `ButtonFrame.vue`'s own size table.
///
/// A button in the reference is not one size: `ButtonFrame.vue` declares five,
/// `xs` through `xl`, and each row carries its own height, radius, horizontal
/// padding, icon gap, label size and icon size. The row is read as a whole here
/// -- the numbers are methods so that a caller cannot take a height from one row
/// and an icon from another.
///
/// [`Size::Md`] is the reference's own default: `Button.vue` and `IconButton.vue`
/// both default their `size` prop to `'md'`, so every button without a `size`
/// attribute is that row -- `h-9`, 36 pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// `h-7`: 28 pixels tall, `rounded-lg`, `text-sm`.
    Xs,
    /// `h-8`: 32, `rounded-[10px]`, `text-sm`.
    Sm,
    /// `h-9`: 36, `rounded-xl`, `text-base` -- the reference's default.
    Md,
    /// `h-10`: 40, `rounded-[14px]`, `text-base`.
    Lg,
    /// `h-12`: 48, `rounded-2xl`, `text-base` in `font-extrabold`.
    Xl,
}

impl Size {
    /// The row's height: `h-7` through `h-12`.
    pub const fn height(self) -> f32 {
        match self {
            Size::Xs => 28.0,
            Size::Sm => 32.0,
            Size::Md => 36.0,
            Size::Lg => 40.0,
            Size::Xl => 48.0,
        }
    }

    /// The row's corner radius: `rounded-lg` through `rounded-2xl`.
    pub const fn radius(self) -> f32 {
        match self {
            Size::Xs => 8.0,
            Size::Sm => 10.0,
            Size::Md => 12.0,
            Size::Lg => 14.0,
            Size::Xl => 16.0,
        }
    }

    /// The row's horizontal padding: `px-1.5`, `px-2.5`, `px-4` and `px-3.5`.
    pub const fn pad(self) -> f32 {
        match self {
            Size::Xs | Size::Sm => 6.0,
            Size::Md => 10.0,
            Size::Lg => 16.0,
            Size::Xl => 14.0,
        }
    }

    /// The gap between the things a button carries: `gap-1`, `gap-1.5` and
    /// `gap-2`.
    pub const fn gap(self) -> f32 {
        match self {
            Size::Xs | Size::Sm => 4.0,
            Size::Md => 6.0,
            Size::Lg | Size::Xl => 8.0,
        }
    }

    /// The label's size: `text-sm` on the two smallest rows, `text-base` above
    /// them.
    pub const fn label(self) -> f32 {
        match self {
            Size::Xs | Size::Sm => 14.0,
            Size::Md | Size::Lg | Size::Xl => 16.0,
        }
    }

    /// The size a slot's own icons are set at: `size-4` through `size-6`.
    pub const fn icon(self) -> f32 {
        match self {
            Size::Xs | Size::Sm => 16.0,
            Size::Md | Size::Lg => 20.0,
            Size::Xl => 24.0,
        }
    }

    /// The face a label is set in: `font-semibold` on four rows and
    /// `font-extrabold` on `xl`, which is the one row the reference weights
    /// differently from the rest of the table.
    pub const fn font(self) -> iced::Font {
        match self {
            Size::Xl => heading(),
            Size::Xs | Size::Sm | Size::Md | Size::Lg => semibold(),
        }
    }

    /// The line a label takes: `leading-5`, twenty pixels, on every row.
    pub const fn line(self) -> f32 {
        20.0
    }

    /// The width an icon-only button takes: `w-7` through `w-12` with `!px-0`,
    /// which is the row's own height again.
    pub const fn square(self) -> f32 {
        self.height()
    }
}

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

/// A tag: a small pill in the raised surface, with no icon in it.
///
/// `TagItem.vue`'s own `baseClass`, which is one string every tag in the
/// reference is built from -- the project card's `TagTagItem`, the project
/// header's categories, a version's platforms:
///
/// ```text
/// bg-[--_bg-color,var(--color-button-bg)] border-[--_bg-color,var(--surface-5)]
/// border-[1px] border-solid px-2 py-1 leading-none rounded-full font-normal
/// text-sm inline-flex items-center gap-1 text-[--_color,var(--color-secondary)]
/// ```
///
/// The label is `text-sm font-normal` -- fourteen pixels at weight 400 -- and was
/// drawn here at twelve and semibold, which is the pair the pixel audit measured
/// on the reference's own pills: *Combat* is eleven ink rows there and nine of
/// ours, and our *Challenging* came out ten pixels narrower than the reference's.
///
/// The ring was missing outright. `border-[--_bg-color,var(--surface-5)]` with
/// `border-[1px] border-solid` is a one-pixel `--surface-5` stroke, which the
/// reference's own table gives as `#42444a` for the dark look
/// (`variables.scss:239`, `:7` for the light one), and the capture reads `#42444A`
/// on the top and bottom edge of every pill on the page. The fill beside it is
/// `--color-button-bg` (`variables.scss:329`), which resolves to `--surface-4`
/// (`variables.scss:238`) and is the `#34363C` that was already right.
///
/// `rounded-full` is not a radius but a radius *rule*: CSS resolves it against the
/// border box, so it is half the height -- twelve here, and thirteen on the
/// twenty-six-row pill [`TAG_HEIGHT_ICON`] describes. Measured on the reference's
/// own pills, the straight run of the top edge is inset by twelve on the
/// twenty-four-row ones and thirteen on the twenty-six-row ones.
///
/// The label's ink is the one number in it that was wrong before this one:
/// `text-[--_color,var(--color-secondary)]`, which the reference's own table
/// resolves to `--color-text-tertiary` (`variables.scss:337`, `:321`) and measures
/// as `#96A2B0`. Every caller of this function is a place the reference draws a
/// plain `TagItem`, and no caller anywhere in the reference overrides `--_color`
/// except `TagTagItem`, which sets it to the platform's own colour for a loader --
/// a token this kit's table does not carry, and recorded as the residual it is
/// rather than guessed at here.
///
/// What is still not drawn is the icon, because this function is not asked for
/// one: a tag only becomes twenty-six rows when it carries the `h-4` glyph, and
/// which tags carry it is the caller's answer, not this file's. The heights are
/// both named -- [`TAG_HEIGHT`] and [`TAG_HEIGHT_ICON`], with [`tag_height`] to
/// ask -- and [`tag_with_icon`] is the other half of that answer, for a caller
/// that has one.
pub fn tag<'a, Message: 'a>(theme: Gen, label: &str) -> Element<'a, Message> {
    container(
        text(label.to_string())
            .size(TAG_LABEL_SIZE)
            .font(regular())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Secondary))),
    )
    .height(Length::Fixed(TAG_HEIGHT))
    .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: TAG_PAD,
        right: TAG_PAD,
    })
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
        border: Border {
            color: theme_gen::ink(theme, Ink::Surface5),
            width: 1.0,
            radius: (TAG_HEIGHT / 2.0).into(),
        },
        ..container::Appearance::default()
    })
    .into()
}

/// A tag pill with the reference's `h-4` glyph in front of its label.
///
/// The other [`tag`], and the one that is [`TAG_HEIGHT_ICON`] tall rather than
/// [`TAG_HEIGHT`]. The three differences between the two, all off
/// `TagItem.vue`'s own `baseClass`:
///
/// * the content is a [`row`] of the glyph and the label rather than the label,
///   with [`TAG_GAP`] between them -- `inline-flex ... gap-1`, which is what puts
///   the four pixels between the icon and the word;
/// * the box is [`TAG_ICON`] tall rather than [`TAG_LABEL_SIZE`], because
///   `items-center` puts the icon and the label on one line and the line is as
///   tall as the taller of them;
/// * the corner radius is [`TAG_HEIGHT_ICON`] / 2 rather than [`TAG_HEIGHT`] / 2.
///   `rounded-full` is a rule and not a number: CSS resolves it against the border
///   box, so it is thirteen on this pill and twelve on the other, which is what the
///   reference's own straight top-edge runs measure.
///
/// The glyph is drawn in the label's ink, which is the one thing
/// `text-[--_color,var(--color-secondary)]` covers: `TagItem.vue` gives `[&>svg]`
/// a size and nothing else, so the icon inherits the same `--color-secondary` the
/// label does and is `#96A2B0` on the dark look.
///
/// The label is the same widget [`tag`] builds, deliberately, so that a pill which
/// carries an icon and one which does not cannot drift apart in any of the four
/// things a label is made of.
pub fn tag_with_icon<'a, Message: 'a>(
    theme: Gen,
    label: &str,
    glyph: Glyph,
) -> Element<'a, Message> {
    let ink = theme_gen::ink(theme, Ink::Secondary);
    container(
        row![icon::icon(glyph, TAG_ICON, ink)]
            .spacing(TAG_GAP)
            .align_items(Alignment::Center)
            .push(
                text(label.to_string())
                    .size(TAG_LABEL_SIZE)
                    .font(regular())
                    .style(iced::theme::Text::Color(ink)),
            ),
    )
    .height(Length::Fixed(TAG_HEIGHT_ICON))
    .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: TAG_PAD,
        right: TAG_PAD,
    })
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
        border: Border {
            color: theme_gen::ink(theme, Ink::Surface5),
            width: 1.0,
            radius: (TAG_HEIGHT_ICON / 2.0).into(),
        },
        ..container::Appearance::default()
    })
    .into()
}

/// The glyph a tag draws in front of its label, if the reference has one.
///
/// `getTagIcon` (`assets/index.ts:176`), which is `getLoaderIcon(tag)` and then
/// `getCategoryIcon(tag)` -- the loader table first, because a Modrinth tag is a
/// loader or a category and `minecraft` is both depending on the project type.
///
/// Each of those is a lookup in a table the reference generates from a directory:
/// [`crate::icons_gen::TAG_LOADERS`] for `icons/tags/loaders/*.svg` and
/// [`crate::icons_gen::TAG_CATEGORIES`] for `icons/tags/categories/*.svg` -- 26
/// and 102, against the reference's 30 and 102. The keys are the file stems, and
/// the keys Modrinth publishes agree on every one of them that is a tag, which is
/// what makes the lookup a lookup rather than a table this file keeps beside the
/// reference's.
///
/// Four loaders have no glyph here: `geyser`, `legacy-fabric`, `purpur` and
/// `quilt`, because the icon generator refuses the construct each of them uses
/// rather than approximating it -- a non-uniform transform scale in three cases
/// and a `clip-path` in the fourth. They are named in `icons_gen.rs` and in its
/// test, and a pill for one of them is the [`TAG_HEIGHT`] pill rather than a
/// twenty-six-row one: the height a tag with no icon has, and not a wrong shape.
///
/// This is `TagTagItem`'s rule *without* `hide-non-loader-icon`. A project card
/// passes that prop, so a caller drawing a card's tag row asks
/// [`is_loader_tag`] first and gets nothing for a category; a caller drawing a
/// tag list elsewhere in the reference gets the category icon.
pub fn tag_icon(tag: &str) -> Option<Glyph> {
    // `getLoaderIcon(tag) ?? getCategoryIcon(tag)`, in that order and with no
    // question asked in between: the reference's `??` is on the two lookups, not
    // on whether the tag is a loader, so a loader whose glyph this port does not
    // draw falls through to the category table and a tag that is both answers
    // with the loader's.
    loader_tag_icon(tag).or_else(|| category_tag_icon(tag))
}

/// Whether a tag is a loader, which is what `hide-non-loader-icon` turns on.
///
/// `getTagMessage(tag, 'loader') !== undefined` (`TagTagItem.vue:30`), which is
/// the reference's `tag.loader.<tag>` being a key rather than a list of loaders
/// kept here -- so this is the generated message table asked the same question,
/// and a loader that table has not caught up with is one no caller will draw an
/// icon for.
pub fn is_loader_tag(tag: &str) -> bool {
    // One loader's message is not named after it. `bta-fabric` is the tag the API
    // publishes, and `tag-messages.ts:10-11` gives it the id `tag.loader.bta-babric`
    // -- so asking for `tag.loader.bta-fabric` finds nothing, and the answer costs
    // that tag both its glyph and its *BTA (Babric)* label, because both are the
    // one message. Named here rather than worked around at the call site, because
    // the exception is upstream's and there is exactly one of it.
    let message = if tag == "bta-fabric" { "bta-babric" } else { tag };
    crate::text_gen::from_name(&format!("tag.loader.{message}")).is_some()
}

/// `getLoaderIcon`: `loaderIconMap[tag.toLowerCase()]`.
fn loader_tag_icon(tag: &str) -> Option<Glyph> {
    match tag {
        "fabric" => Some(Glyph::TagLoaderFabric),
        "bta-babric" => Some(Glyph::TagLoaderBtaBabric),
        "forge" => Some(Glyph::TagLoaderForge),
        "neoforge" => Some(Glyph::TagLoaderNeoforge),
        "minecraft" => Some(Glyph::TagLoaderMinecraft),
        "mrpack" => Some(Glyph::TagLoaderMrpack),
        "bukkit" => Some(Glyph::TagLoaderBukkit),
        "spigot" => Some(Glyph::TagLoaderSpigot),
        "paper" => Some(Glyph::TagLoaderPaper),
        "sponge" => Some(Glyph::TagLoaderSponge),
        "canvas" => Some(Glyph::TagLoaderCanvas),
        "datapack" => Some(Glyph::TagLoaderDatapack),
        "folia" => Some(Glyph::TagLoaderFolia),
        "bungeecord" => Some(Glyph::TagLoaderBungeecord),
        "waterfall" => Some(Glyph::TagLoaderWaterfall),
        "velocity" => Some(Glyph::TagLoaderVelocity),
        "iris" => Some(Glyph::TagLoaderIris),
        "optifine" => Some(Glyph::TagLoaderOptifine),
        "vanilla" => Some(Glyph::TagLoaderVanilla),
        "java-agent" => Some(Glyph::TagLoaderJavaAgent),
        "liteloader" => Some(Glyph::TagLoaderLiteloader),
        "modloader" => Some(Glyph::TagLoaderModloader),
        "nilloader" => Some(Glyph::TagLoaderNilloader),
        "ornithe" => Some(Glyph::TagLoaderOrnithe),
        "babric" => Some(Glyph::TagLoaderBabric),
        "rift" => Some(Glyph::TagLoaderRift),
        _ => None,
    }
}

/// `getCategoryIcon`: `categoryIconMap[tag.toLowerCase()]`.
fn category_tag_icon(tag: &str) -> Option<Glyph> {
    match tag {
        "adventure" => Some(Glyph::TagCategoryAdventure),
        "atmosphere" => Some(Glyph::TagCategoryAtmosphere),
        "audio" => Some(Glyph::TagCategoryAudio),
        "backpack" => Some(Glyph::TagCategoryBackpack),
        "badge" => Some(Glyph::TagCategoryBadge),
        "badge-check" => Some(Glyph::TagCategoryBadgeCheck),
        "bed-double" => Some(Glyph::TagCategoryBedDouble),
        "blocks" => Some(Glyph::TagCategoryBlocks),
        "bloom" => Some(Glyph::TagCategoryBloom),
        "building-2" => Some(Glyph::TagCategoryBuilding2),
        "camera" => Some(Glyph::TagCategoryCamera),
        "cartoon" => Some(Glyph::TagCategoryCartoon),
        "castle" => Some(Glyph::TagCategoryCastle),
        "challenging" => Some(Glyph::TagCategoryChallenging),
        "clapperboard" => Some(Glyph::TagCategoryClapperboard),
        "cloud" => Some(Glyph::TagCategoryCloud),
        "colored-lighting" => Some(Glyph::TagCategoryColoredLighting),
        "combat" => Some(Glyph::TagCategoryCombat),
        "compass" => Some(Glyph::TagCategoryCompass),
        "core-shaders" => Some(Glyph::TagCategoryCoreShaders),
        "crown" => Some(Glyph::TagCategoryCrown),
        "cursed" => Some(Glyph::TagCategoryCursed),
        "decoration" => Some(Glyph::TagCategoryDecoration),
        "dices" => Some(Glyph::TagCategoryDices),
        "economy" => Some(Glyph::TagCategoryEconomy),
        "entities" => Some(Glyph::TagCategoryEntities),
        "environment" => Some(Glyph::TagCategoryEnvironment),
        "equipment" => Some(Glyph::TagCategoryEquipment),
        "fantasy" => Some(Glyph::TagCategoryFantasy),
        "film" => Some(Glyph::TagCategoryFilm),
        "flag" => Some(Glyph::TagCategoryFlag),
        "foliage" => Some(Glyph::TagCategoryFoliage),
        "fonts" => Some(Glyph::TagCategoryFonts),
        "food" => Some(Glyph::TagCategoryFood),
        "footprints" => Some(Glyph::TagCategoryFootprints),
        "game-mechanics" => Some(Glyph::TagCategoryGameMechanics),
        "gamepad-2" => Some(Glyph::TagCategoryGamepad2),
        "gauge" => Some(Glyph::TagCategoryGauge),
        "globe" => Some(Glyph::TagCategoryGlobe),
        "grid-3x3" => Some(Glyph::TagCategoryGrid3x3),
        "gui" => Some(Glyph::TagCategoryGui),
        "handshake" => Some(Glyph::TagCategoryHandshake),
        "heart-crack" => Some(Glyph::TagCategoryHeartCrack),
        "heart-pulse" => Some(Glyph::TagCategoryHeartPulse),
        "high" => Some(Glyph::TagCategoryHigh),
        "house" => Some(Glyph::TagCategoryHouse),
        "items" => Some(Glyph::TagCategoryItems),
        "kitchen-sink" => Some(Glyph::TagCategoryKitchenSink),
        "library" => Some(Glyph::TagCategoryLibrary),
        "lightweight" => Some(Glyph::TagCategoryLightweight),
        "locale" => Some(Glyph::TagCategoryLocale),
        "lock" => Some(Glyph::TagCategoryLock),
        "low" => Some(Glyph::TagCategoryLow),
        "magic" => Some(Glyph::TagCategoryMagic),
        "management" => Some(Glyph::TagCategoryManagement),
        "map-pinned" => Some(Glyph::TagCategoryMapPinned),
        "medium" => Some(Glyph::TagCategoryMedium),
        "minigame" => Some(Glyph::TagCategoryMinigame),
        "mobs" => Some(Glyph::TagCategoryMobs),
        "modded" => Some(Glyph::TagCategoryModded),
        "models" => Some(Glyph::TagCategoryModels),
        "multiplayer" => Some(Glyph::TagCategoryMultiplayer),
        "network" => Some(Glyph::TagCategoryNetwork),
        "optimization" => Some(Glyph::TagCategoryOptimization),
        "palette" => Some(Glyph::TagCategoryPalette),
        "path-tracing" => Some(Glyph::TagCategoryPathTracing),
        "paw-print" => Some(Glyph::TagCategoryPawPrint),
        "pbr" => Some(Glyph::TagCategoryPbr),
        "pickaxe" => Some(Glyph::TagCategoryPickaxe),
        "potato" => Some(Glyph::TagCategoryPotato),
        "quests" => Some(Glyph::TagCategoryQuests),
        "realistic" => Some(Glyph::TagCategoryRealistic),
        "reflections" => Some(Glyph::TagCategoryReflections),
        "refresh-ccw" => Some(Glyph::TagCategoryRefreshCcw),
        "screenshot" => Some(Glyph::TagCategoryScreenshot),
        "scroll-text" => Some(Glyph::TagCategoryScrollText),
        "semi-realistic" => Some(Glyph::TagCategorySemiRealistic),
        "shadows" => Some(Glyph::TagCategoryShadows),
        "shield" => Some(Glyph::TagCategoryShield),
        "simplistic" => Some(Glyph::TagCategorySimplistic),
        "skull" => Some(Glyph::TagCategorySkull),
        "social" => Some(Glyph::TagCategorySocial),
        "square" => Some(Glyph::TagCategorySquare),
        "storage" => Some(Glyph::TagCategoryStorage),
        "sword" => Some(Glyph::TagCategorySword),
        "swords" => Some(Glyph::TagCategorySwords),
        "target" => Some(Glyph::TagCategoryTarget),
        "technology" => Some(Glyph::TagCategoryTechnology),
        "terminal" => Some(Glyph::TagCategoryTerminal),
        "theater" => Some(Glyph::TagCategoryTheater),
        "themed" => Some(Glyph::TagCategoryThemed),
        "transportation" => Some(Glyph::TagCategoryTransportation),
        "tree-pine" => Some(Glyph::TagCategoryTreePine),
        "trophy" => Some(Glyph::TagCategoryTrophy),
        "tweaks" => Some(Glyph::TagCategoryTweaks),
        "users" => Some(Glyph::TagCategoryUsers),
        "utility" => Some(Glyph::TagCategoryUtility),
        "vanilla-like" => Some(Glyph::TagCategoryVanillaLike),
        "wand-sparkles" => Some(Glyph::TagCategoryWandSparkles),
        "wifi-off" => Some(Glyph::TagCategoryWifiOff),
        "worldgen" => Some(Glyph::TagCategoryWorldgen),
        "zap" => Some(Glyph::TagCategoryZap),
        _ => None,
    }
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

/// A button at one row of [`Size`], which is `ButtonFrame.vue`'s own frame.
///
/// The difference from [`button`] beyond the size is the paint, which is the
/// reference's rather than the legacy frame's: `base` and `colored-text` carry
/// `box-shadow: inset 0 0 0 1px var(--surface-5)`, drawn here as the 1-pixel
/// border it is (iced paints a border inside the bounds, which is what an inset
/// shadow is); an `outlined` button's `0 0 0 1px` ring and a `quiet` button's
/// `hover:bg-surface-4` are drawn the same way; and a `quiet` button's ink is
/// `--color-base`, the token its rule actually names, rather than the legacy
/// frame's contrast ink.
///
/// What the reference draws that this does not is recorded rather than
/// approximated: the `colored` type's outer 1-pixel `color-mix(... 30%,
/// transparent)` ring, its four soft drop shadows, and the `::before` top-edge
/// highlight are all blurred or outer paint, and this backend's own note on
/// shadows (`crate::theme`'s, on `modal`) is that a blurred rectangle costs a
/// per-pixel pass every frame.
pub fn button_sized<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    label: Key,
    kind: Kind,
    size: Size,
    on_press: Message,
) -> Element<'a, Message> {
    button_or_sized(theme, key, label, kind, size, Some(on_press))
}

/// The same button with its press optional, which is the disabled form.
///
/// `ButtonFrame.vue` draws a disabled button as itself at `disabled:opacity-50`
/// with the pointer's cursor forbidden and no hover. This renderer has no group
/// opacity, so the fifty percent is applied to each colour the frame paints --
/// fill, ring and ink -- which is the arithmetic the overlay does anyway.
pub fn button_or_sized<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    label: Key,
    kind: Kind,
    size: Size,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    sized_face(theme, key, kind, size, Length::Shrink, false, false, on_press, move |ink| {
        text(label.message())
            .size(size.label())
            .line_height(iced::Pixels(size.line()))
            .font(size.font())
            .style(iced::theme::Text::Color(ink))
            .into()
    })
}

/// The same button with a label that is a string rather than a generated key.
///
/// See [`button_text`]: the language rows are names the vendored tree carries,
/// and a name upstream adds without a message must still draw.
pub fn button_text_sized<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    label: &str,
    kind: Kind,
    size: Size,
    on_press: Message,
) -> Element<'a, Message> {
    let label = label.to_string();
    sized_face(theme, key, kind, size, Length::Shrink, false, false, Some(on_press), move |ink| {
        text(label.clone())
            .size(size.label())
            .line_height(iced::Pixels(size.line()))
            .font(size.font())
            .style(iced::theme::Text::Color(ink))
            .into()
    })
}

/// A button at one row of [`Size`] with an icon in front of its label, at the
/// row's own icon size and gap.
pub fn button_with_icon_sized<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    glyph: Glyph,
    label: Key,
    kind: Kind,
    size: Size,
    width: Length,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    sized_face(theme, key, kind, size, width, false, false, on_press, move |ink| {
        row![]
            .spacing(size.gap())
            .align_items(Alignment::Center)
            .push(icon::icon(glyph, size.icon(), ink))
            .push(
                text(label.message())
                    .size(size.label())
                    .line_height(iced::Pixels(size.line()))
                    .font(size.font())
                    .style(iced::theme::Text::Color(ink)),
            )
            .into()
    })
}

/// A square icon button: `IconButton.vue`, which is `ButtonFrame` with
/// `icon-only` and, by its own default, `circular`.
///
/// The row's `w-7`..`w-12` class is the same number as its height, and `!px-0`
/// throws the horizontal padding away, so the button is a square -- a circle at
/// `rounded-full`. The reference's IconButton defaults to `md` and to circular,
/// which is the 36-pixel round control the settings dialog's close is.
pub fn icon_button_sized<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    glyph: Glyph,
    kind: Kind,
    size: Size,
    on_press: Message,
) -> Element<'a, Message> {
    sized_face(theme, key, kind, size, Length::Shrink, true, true, Some(on_press), move |ink| {
        icon::icon(glyph, size.icon(), ink).into()
    })
}

/// How much room a button at `size` with this label needs: the label and its own
/// padding, measured with the same face [`button_text_sized`] draws it in.
pub fn button_width_sized(label: &str, size: Size) -> f32 {
    size.pad() * 2.0 + advance(label, size.font(), size.label())
}

/// The face of a button at one row of [`Size`]: its frame, its fill, and the
/// crossing it publishes.
///
/// One colour table for every sized builder, [`Kind`]-keyed, as `ButtonFrame`'s
/// own `typeClasses` is: the fill, the 1-pixel ring, and the ink the label and
/// the icon in front of it share. The hover is the reference's own two rules
/// together -- the `brightness(--hover-brightness)` filter, and the
/// `hover:bg-surface-4` plate a type with no fill fades in over the same 150 ms.
fn sized_face<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    kind: Kind,
    size: Size,
    width: Length,
    icon_only: bool,
    circular: bool,
    on_press: Option<Message>,
    face: impl FnOnce(Color) -> Element<'a, Message>,
) -> Element<'a, Message> {
    let usable = on_press.is_some();
    let (factor, _) = if usable { interaction(key) } else { (1.0, 0.0) };
    // `ButtonFrame.vue`'s type table, resolved to colours.
    let (fill, ring, ink) = match kind {
        Kind::Standard => (
            Some(theme_gen::ink(theme, Ink::ButtonBg)),
            Some(theme_gen::ink(theme, Ink::Surface5)),
            theme_gen::ink(theme, INK_CONTRAST),
        ),
        Kind::Colored => (
            Some(theme_gen::ink(theme, Ink::Brand)),
            None,
            theme_gen::ink(theme, Ink::AccentContrast),
        ),
        Kind::Danger => (
            Some(theme_gen::ink(theme, Ink::Red)),
            None,
            theme_gen::ink(theme, Ink::AccentContrast),
        ),
        Kind::Outlined => (
            None,
            Some(theme_gen::ink(theme, Ink::Surface5)),
            theme_gen::ink(theme, INK_CONTRAST),
        ),
        Kind::Quiet => (None, None, theme_gen::ink(theme, Ink::Base)),
    };
    // The pointer's crossing, as the two rules it is: the filter's brightness
    // travels 0..1 with the clock, and the plate under a fill-less type is that
    // same travel as an alpha.
    let travel = crate::theme::hover_brightness() - 1.0;
    let amount = if travel == 0.0 {
        0.0
    } else {
        ((factor - 1.0) / travel).clamp(0.0, 1.0)
    };
    let fill = match fill {
        Some(color) => Some(color),
        // Transparent at rest, so a quiet or outlined button keeps the surface
        // it sits on until the pointer arrives.
        None => Some(Color { a: amount, ..theme_gen::ink(theme, Ink::Surface4) }),
    };
    let dim = |color: Color| if usable { color } else { crate::style::at_opacity(color, 0.5) };
    let ink = crate::theme::brightness(dim(ink), factor);
    let fill = fill.map(|color| Background::Color(crate::theme::brightness(dim(color), factor)));
    let ring = ring.map(|color| crate::theme::brightness(dim(color), factor));
    let face = container(face(ink))
        // `w-7`..`w-12` and `!px-0` on an icon-only button: the width is the
        // row's own height and the padding is gone, which is what makes it a
        // square -- a circle once `rounded-full` is on it.
        .width(if icon_only { Length::Fixed(size.square()) } else { width })
        .height(Length::Fixed(size.height()))
        .padding(Padding {
            top: 0.0,
            bottom: 0.0,
            left: if icon_only { 0.0 } else { size.pad() },
            right: if icon_only { 0.0 } else { size.pad() },
        })
        .center_x()
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            background: fill,
            border: Border {
                color: ring.unwrap_or(Color::TRANSPARENT),
                width: if ring.is_some() { 1.0 } else { 0.0 },
                radius: (if circular { 999.0 } else { size.radius() }).into(),
            },
            ..container::Appearance::default()
        });
    let area = mouse_area(face)
        // `disabled:cursor-not-allowed`: a button nobody may press draws the
        // forbidden cursor rather than the pointer that says it can be pressed.
        .interaction(if usable { Interaction::Pointer } else { Interaction::NotAllowed })
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false));
    match on_press {
        Some(on_press) => area.on_press(on_press).into(),
        None => area.into(),
    }
}

/// A field at one row of [`InputSize`], which is `InputFrame.vue`'s own table.
///
/// A row is a height, a radius, a horizontal padding, a gap and a label size --
/// the same shape as [`Size`], which is [`ButtonFrame.vue`'s] -- and the two are
/// kept apart because they disagree: the frame's `lg` is 40 pixels with 16 of
/// padding, the input's `medium` is 40 pixels with 12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSize {
    /// `h-8`: 32, `rounded-xl`, `px-3`, `gap-1.5`, a `text-sm` label.
    Small,
    /// `h-9`: 36, `rounded-xl`, `px-3`, `gap-2`, `text-base`. The reference's
    /// own default -- `Input.vue` defaults its `size` prop to `'standard'`.
    Standard,
    /// `h-10`: 40, `rounded-[14px]`, `px-4`, `gap-2`.
    Medium,
    /// `h-12`: 48, `rounded-[14px]`, `px-4`, `gap-2`.
    Large,
}

impl InputSize {
    /// The row's height: `h-8` through `h-12`.
    pub const fn height(self) -> f32 {
        match self {
            InputSize::Small => 32.0,
            InputSize::Standard => 36.0,
            InputSize::Medium => 40.0,
            InputSize::Large => 48.0,
        }
    }

    /// The row's radius: `rounded-xl` below 40 pixels and `rounded-[14px]` at it.
    pub const fn radius(self) -> f32 {
        match self {
            InputSize::Small | InputSize::Standard => 12.0,
            InputSize::Medium | InputSize::Large => 14.0,
        }
    }

    /// The row's horizontal padding: `px-3` or `px-4`.
    pub const fn pad(self) -> f32 {
        match self {
            InputSize::Small | InputSize::Standard => 12.0,
            InputSize::Medium | InputSize::Large => 16.0,
        }
    }

    /// The gap between a leading icon and the value: `gap-1.5` or `gap-2`.
    pub const fn gap(self) -> f32 {
        match self {
            InputSize::Small => 6.0,
            InputSize::Standard | InputSize::Medium | InputSize::Large => 8.0,
        }
    }

    /// The size a label is set at: `text-sm` on the smallest row, `text-base`
    /// above it.
    pub const fn label(self) -> f32 {
        match self {
            InputSize::Small => 14.0,
            InputSize::Standard | InputSize::Medium | InputSize::Large => 16.0,
        }
    }
}

/// An input at one row of [`InputSize`], which is `Input.vue`'s own field.
///
/// The frame is `InputFrame.vue`'s: a 1-pixel `border-surface-5` hairline over a
/// `bg-surface-4` fill, the value in `font-medium text-primary` with its
/// placeholder in `text-secondary`, and a leading icon at `size-5` (20 pixels) in
/// the secondary ink at 60%. Those two surface tokens are the reference's and they
/// are not the pair this module's [`framed`] draws (a `bg-surface-5` fill over a
/// `border-surface-4` hairline), which is why this is its own builder rather than a
/// size on that one: a field has to be one or the other.
pub fn input_sized<'a, Message: Clone + 'a>(
    theme: Gen,
    size: InputSize,
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let field = text_input(placeholder, value)
        .on_input(on_input)
        .padding(Padding { top: 0.0, bottom: 0.0, left: 0.0, right: 0.0 })
        .size(size.label())
        .font(medium())
        .style(iced::theme::TextInput::Custom(Box::new(Field { theme, chrome: Chrome::Bare })));
    container(
        row![]
            .align_items(Alignment::Center)
            .spacing(size.gap())
            .push(icon::icon(Glyph::Search, 20.0, theme_gen::ink(theme, INK_SECONDARY)))
            .push(field),
    )
    .width(Length::Fill)
    .height(Length::Fixed(size.height()))
    .padding(Padding { top: 0.0, bottom: 0.0, left: size.pad(), right: size.pad() })
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        // `appearanceClass`: `border-surface-5 bg-surface-4`.
        background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface4))),
        border: Border {
            color: theme_gen::ink(theme, Ink::Surface5),
            width: 1.0,
            radius: size.radius().into(),
        },
        ..container::Appearance::default()
    })
    .into()
}

/// One row of the reference's language list: `CheckCircleButton.vue`.
///
/// `ButtonFrame` at `lg` -- 40 pixels, `rounded-[14px]`, the label face of
/// `font-semibold` -- with `interaction="none"` (so the frame's brightness filter
/// is off) and the row's own overrides: `w-full`, `!gap-4`, `!px-2`, a 1-pixel
/// border and `text-left`, and a check circle at the end. Chosen, the row is
/// `!border-brand !bg-brand-highlight !text-contrast`; otherwise its border is
/// transparent and `enabled:hover:!bg-surface-3` fades a surface in.
///
/// The circle is `size-6`: a `bg-brand` disc with a 16-pixel `CheckIcon` in
/// `--color-accent-contrast` (which the Tailwind preset also calls
/// `brand-inverted`), or an empty ring in `border-surface-5`.
///
/// Two things the reference draws here are recorded rather than invented: the
/// 24x16 flag image in front of the name comes from `flagcdn.com`, and the names
/// are `truncate`d rather than wrapped -- iced has no ellipsis.
pub fn check_row<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    checked: bool,
    name: &str,
    translated: Option<&str>,
    coverage: Option<&str>,
    on_press: Message,
) -> Element<'a, Message> {
    let (factor, _) = interaction(key);
    // The row's hover is a colour rather than the frame's filter, so the clock's
    // travel is read as a plate fading in instead of as a brightness.
    let travel = crate::theme::hover_brightness() - 1.0;
    let amount = if travel == 0.0 {
        0.0
    } else {
        ((factor - 1.0) / travel).clamp(0.0, 1.0)
    };
    let (fill, edge) = if checked {
        (
            theme_gen::ink(theme, Ink::ColorBrandHighlight),
            theme_gen::ink(theme, Ink::Brand),
        )
    } else {
        (Color { a: amount, ..theme_gen::ink(theme, Ink::Surface3) }, Color::TRANSPARENT)
    };
    let contrast = theme_gen::ink(theme, INK_CONTRAST);
    let secondary = theme_gen::ink(theme, INK_SECONDARY);
    let mut names = row![].spacing(8.0).align_items(Alignment::Center);
    names = names.push(
        text(name.to_string())
            .size(16.0)
            .font(semibold())
            .style(iced::theme::Text::Color(contrast)),
    );
    if let Some(translated) = translated {
        // `text-xs sm:text-sm font-normal text-secondary`: the same line, one
        // step smaller and in the tertiary ink.
        names = names.push(
            text(translated.to_string())
                .size(14.0)
                .font(crate::style::regular())
                .style(iced::theme::Text::Color(secondary)),
        );
    }
    names = names.push(Space::with_width(Length::Fill));
    if let Some(coverage) = coverage {
        names = names.push(
            text(coverage.to_string())
                .size(14.0)
                .font(crate::style::regular())
                .style(iced::theme::Text::Color(secondary)),
        );
    }
    let circle = if checked {
        container(icon::icon(Glyph::Check, 16.0, theme_gen::ink(theme, Ink::AccentContrast)))
            .width(Length::Fixed(24.0))
            .height(Length::Fixed(24.0))
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Brand))),
                border: Border { radius: 999.0.into(), ..Border::default() },
                ..container::Appearance::default()
            })
    } else {
        container(Space::new(Length::Fixed(24.0), Length::Fixed(24.0)))
            .style(move |_theme: &Theme| container::Appearance {
                border: Border {
                    radius: 999.0.into(),
                    width: 1.0,
                    color: theme_gen::ink(theme, Ink::Surface5),
                },
                ..container::Appearance::default()
            })
    };
    let face = container(
        row![]
            .spacing(16.0)
            .align_items(Alignment::Center)
            .push(names.width(Length::Fill))
            .push(circle),
    )
    .width(Length::Fill)
    .height(Length::Fixed(40.0))
    .padding(Padding { top: 0.0, bottom: 0.0, left: 8.0, right: 8.0 })
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(fill)),
        border: Border { color: edge, width: 1.0, radius: 14.0.into() },
        ..container::Appearance::default()
    });
    mouse_area(face)
        .interaction(Interaction::Pointer)
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false))
        .on_press(on_press)
        .into()
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

/// The reference's own checkbox: `Checkbox.vue`.
///
/// The control is a `button` with no frame of its own around a 20x20
/// `rounded-md` square with a 1-pixel border -- `bg-brand border-button-border
/// text-brand-inverted` when it is on, `bg-surface-2 border-surface-5` when it is
/// off -- and a 16-pixel `CheckIcon` or `MinusIcon` in the middle. The hover is
/// the same `brightness(--hover-brightness)` filter every other control here
/// uses, and the press scale (`checkbox-shadow group-active:scale-95`) is the one
/// this kit has no way to draw.
///
/// The indeterminate state is the reference's own: a `MinusIcon` where the tick
/// would be, and a caller that has both a checked and an unchecked child passes
/// `true` for the row and `Some(true)` here. A checkbox is the Content tab's
/// enable control, so the content row's old *Deselect*/*Edit* button pair was a
/// control the reference does not draw at all.
pub fn checkbox<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    checked: bool,
    indeterminate: bool,
    on_toggle: Message,
) -> Element<'a, Message> {
    let (factor, _) = interaction(key);
    let (fill, edge, ink) = if checked {
        (
            theme_gen::ink(theme, Ink::Brand),
            theme_gen::ink(theme, Ink::ButtonBorder),
            // `text-brand-inverted` is the same token the Tailwind preset also
            // calls `--color-accent-contrast`.
            theme_gen::ink(theme, Ink::AccentContrast),
        )
    } else {
        (
            theme_gen::ink(theme, Ink::Surface2),
            theme_gen::ink(theme, Ink::Surface5),
            theme_gen::ink(theme, INK_DEFAULT),
        )
    };
    let glyph = if indeterminate { Glyph::Minus } else { Glyph::Check };
    let face = container(icon::icon(
        glyph,
        CHECK_SIZE,
        crate::theme::brightness(ink, factor),
    ))
    .width(Length::Fixed(CHECK_SIZE))
    .height(Length::Fixed(CHECK_SIZE))
    .center_x()
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(crate::theme::brightness(fill, factor))),
        border: Border {
            color: edge,
            width: 1.0,
            // `rounded-md`: the 6-pixel step of Tailwind's own radius scale.
            radius: 6.0.into(),
        },
        ..container::Appearance::default()
    });
    mouse_area(face)
        .interaction(Interaction::Pointer)
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false))
        .on_press(on_toggle)
        .into()
}

/// One row of a page header's metadata: the reference's `PageHeaderMetadata`.
///
/// `page-header/metadata/index.vue` lays the row out as `flex min-w-0 flex-wrap
/// items-center gap-x-[1.625rem] gap-y-2` -- 26 pixels between items -- and every
/// item carries its own `BulletDivider` in an `absolute right-full flex h-full
/// w-[1.625rem]` span, which puts a 6-pixel `--surface-5` dot in the middle of the
/// gap *behind* it. The scoped rule hides that dot on the first child and on any
/// item that starts a row (`data-page-header-metadata-row-start`), so in a
/// one-row strip the first fact has no dot and every other one does.
///
/// The item itself is `relative flex min-w-0 items-center font-medium leading-none
/// text-secondary text-nowrap` around an `inline-flex items-center gap-2`, with
/// the icon at `block size-5 shrink-0 text-current`.
pub fn metadata_row<'a, Message: 'a>(
    theme: Gen,
    items: &[(Glyph, String)],
) -> Element<'a, Message> {
    let mut row = row![].align_items(Alignment::Center);
    for (index, (glyph, label)) in items.iter().enumerate() {
        // The dot is drawn in front of the item rather than behind it, which is
        // the same picture: nothing else in the row occupies the gap.
        if index > 0 {
            row = row.push(container(Space::new(Length::Fixed(METADATA_DOT), Length::Fixed(METADATA_DOT))).center_y().style(
                move |_theme: &Theme| container::Appearance {
                    background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface5))),
                    border: Border { radius: 999.0.into(), ..Border::default() },
                    ..container::Appearance::default()
                },
            ));
            // `gap-x-[1.625rem]` is the whole gap, dot included: 26 less the
            // dot's own six, halved either side of it.
            row = row.push(Space::with_width((METADATA_GAP - METADATA_DOT) / 2.0));
        }
        let item = row![]
            .align_items(Alignment::Center)
            .spacing(8.0)
            .push(icon::icon(*glyph, METADATA_ICON, theme_gen::ink(theme, INK_SECONDARY)))
            .push(
                text(label.clone())
                    .size(METADATA_LABEL)
                    // `leading-none`: the item's line is the text's own size, and
                    // the row is as tall as the icon beside it either way.
                    .line_height(iced::Pixels(METADATA_LABEL))
                    .font(crate::style::medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            );
        row = row.push(item);
    }
    row.into()
}

/// `gap-x-[1.625rem]`: the space between two metadata items, dot included.
pub const METADATA_GAP: f32 = 26.0;
/// `BulletDivider`'s `min-w-1.5 min-h-1.5`: the dot's own size.
pub const METADATA_DOT: f32 = 6.0;
/// The item's `size-5` icon.
pub const METADATA_ICON: f32 = 20.0;
/// The label's `font-medium` at the interface's base size.
pub const METADATA_LABEL: f32 = 16.0;

/// The width and height of a [`checkbox`]: `w-5 h-5`.
pub const CHECK_SIZE: f32 = 20.0;

/// `NavTabs.vue`'s own tab measurements: `size-5` on the icon, `gap-2` between it
/// and the label, `px-4` on both sides of the tab, and `text-sm` -- fourteen
/// pixels on `text-sm`'s own twenty-pixel line -- for the label itself.
pub const TAB_ICON: f32 = 20.0;
pub const TAB_GAP: f32 = 8.0;
pub const TAB_PAD: f32 = 16.0;
pub const TAB_LABEL: f32 = 14.0;
pub const TAB_LINE: f32 = 20.0;

/// The weight `NavTabs.vue:9` puts on the label, which is seven hundred and not
/// the eight hundred a heading is. The class is on the `<nav>`, not on the label:
/// `text-xs sm:text-sm font-bold` wraps the whole strip, and the label `<span>` at
/// `NavTabs.vue:35` and `:57` carries only `tab-color text-nowrap` and a colour,
/// so both the size and the weight are inherited.
///
/// **Why this is a constant and not [`heading`].** The strip was drawn in
/// `heading()`, which is `--font-weight-heading` -- eight hundred. Measured on the
/// reference's own capture of `/user/FlameFire` at 1280x720 against this port's
/// build at the same size, on the `ll` of *Collections*, which Inter draws as a
/// bare vertical stem so its width reads the weight directly:
///
/// | | `l` stem | ink runs, *Data Packs* / *Modpacks* / *Collections* |
/// | --- | --- | --- |
/// | reference | 2.22px | 9 / 8 / 11 |
/// | this port in `heading()` | 2.52px | 3 / 5 / 8 |
/// | Inter 700 at [`TAB_LABEL`] | 2.118px | |
/// | Inter 800 at [`TAB_LABEL`] | 2.431px | |
///
/// Both renderers draw the stem about a tenth of a pixel fatter than the outline
/// says, so the reference's 2.22 is Inter 700 and this port's 2.52 is Inter 800,
/// and the run counts say the same thing: eight hundred's stems touch their
/// neighbours and merge *Data Packs* into three blobs where seven hundred keeps
/// every letter separate. The reference's own shipped stylesheet agrees --
/// `.font-bold{font-weight:700}` and `--font-weight-bold:700`, in the same rule
/// block as the `font-extrabold{font-weight:800}` this was borrowing.
///
/// The size was already right and this does not move it: both images put the label
/// on eleven ink rows, and the strip's own height pins the root at sixteen pixels
/// (see [`TAB_STRIP`]).
pub const TAB_LABEL_WEIGHT: iced::font::Weight = iced::font::Weight::Bold;

/// The tab's own height: `py-2` around [`TAB_LINE`].
pub const TAB_HEIGHT: f32 = TAB_LINE + 16.0;

/// The strip's own height, which is the tab plus everything the track wraps it in.
///
/// `NavTabs.vue:9` puts a `p-1` around the tabs and `NavTabs.vue:11` adds
/// `border border-solid border-surface-4` to the same element in navigation mode,
/// so the pill is `1 + 4 + 36 + 4 + 1` -- forty-six, borders included. Measured on
/// the reference's own capture of `/user/FlameFire` at 1280x720: the pill's rows
/// are y=201..246 and the two border rows are y=201 and y=246.
pub const TAB_STRIP: f32 = TAB_HEIGHT + 8.0 + 2.0;

/// The `border-[1px]` `NavTabs.vue:11` puts on the strip, in `--surface-4`.
pub const TAB_STRIP_BORDER: f32 = 1.0;

/// `--shadow-card`, the class `NavTabs.vue:11` puts on the strip as `card-shadow`.
///
/// It resolves to `rgba(0, 0, 0, 0.25) 0px 2px 4px 0px` in the dark theme
/// (`assets/styles/variables.scss:368`) and `rgba(50, 50, 100, 0.1) 0px 2px 4px 0px`
/// in the light one (`:140`); `.oled-mode` and `.retro-mode` extend `.dark-mode`
/// and so take dark's. **None of it is drawn**, and that is the documented limit
/// rather than a residual shape: iced 0.12.3 cannot draw it here at all. The
/// numbers are kept because they are what the measurement is compared against and
/// because a backend that grows a separate shadow pass can draw them without a
/// second decision.
///
/// **Why.** `iced_widget::container::draw_background` has no shadow of its own: it
/// hands the fill, the ring and the `Shadow` to a single `renderer::fill_quad`,
/// and `iced_wgpu`'s solid pipeline expands that one quad by the blur and the
/// offset and composites the shadow in the same fragment
/// (`shader/quad/solid.wgsl`):
///
/// ```wgsl
/// return mix(base_color, shadow_color, (1.0 - radius_alpha) * shadow_alpha);
/// ```
///
/// `radius_alpha` is meant to be that quad's own rounded-box coverage, so the
/// shadow only reaches the pixels the box does not cover, and on paper nothing
/// lands in the fill. Measured on this port's own build at 1280x720, it lands
/// there anyway: `#27292E` reads `#1D1F22` from x=243 to x=356, `#161719` from
/// x=140 to x=236 and `#101113` from x=237 to x=242, with hard vertical steps at
/// x=357, x=243 and x=237 -- which are the strip's own tab boundaries -- and the
/// selected tab's plate is darkened with them. The steps are one, two and three
/// layers of `rgba(0,0,0,0.25)` stacked on the pill's own fill.
///
/// Three placements were built and measured, each a full release build:
///
/// | where the shadow goes | x>=357 | x=243..356 | x=140..236 | x=237..242 |
/// | --- | --- | --- | --- | --- |
/// | on the pill's own quad | `#27292E` | 0.75x | 0.75^2 | 0.75^3 |
/// | on a parent quad drawn *before* the pill | `#27292E` | 0.875x | 0.77x | 0.667x |
/// | not drawn at all | `#27292E` | `#27292E` | `#27292E` | `#27292E` |
///
/// A parent quad is drawn first -- `Container::draw` calls `draw_background`
/// before it draws its child -- so the second row is a case where the pill's own
/// opaque fill is painted last and still does not win, which is the whole of the
/// limit: there is no placement of an `iced::Shadow` that leaves this pill's fill
/// alone. Two other probes rule out the alternatives: the steps do not move when
/// the blur is changed from 4 to 40, so they are not the shadow's own edge, and
/// they reproduce at the same x on a freshly created surface at a different window
/// size, so they are not a stale framebuffer.
#[cfg(test)]
fn card_shadow(theme: Gen) -> iced::Shadow {
    let color = match theme {
        Gen::Light => Color::from_rgba(50.0 / 255.0, 50.0 / 255.0, 100.0 / 255.0, 0.1),
        _ => Color::from_rgba(0.0, 0.0, 0.0, 0.25),
    };
    iced::Shadow {
        color,
        offset: iced::Vector::new(0.0, TAB_SHADOW_OFFSET),
        blur_radius: TAB_SHADOW_BLUR,
    }
}

/// `0px 2px` in `--shadow-card`: how far the shadow sits below the pill.
#[cfg(test)]
const TAB_SHADOW_OFFSET: f32 = 2.0;
/// `4px` in `--shadow-card`, which is the blur radius and not a diameter.
#[cfg(test)]
const TAB_SHADOW_BLUR: f32 = 4.0;

/// The selected tab's plate, as the reference composites it.
///
/// `bg-button-bgSelected` is `--color-button-bg-selected`, which the dark theme
/// resolves to `var(--color-brand-highlight)` = `#1BD96A40` (`variables.scss:376`,
/// `:356`) -- the brand green at a quarter alpha, over the track's own
/// `--color-bg-raised` = `#27292E`. The reference's capture measures the result as
/// **`#24543D`**, and this is the function that gets there.
///
/// It is worked out here rather than handed to the rasteriser because iced passes
/// the alpha straight through and the byte that comes back is the backend's
/// choice, not ours: `tiny_skia` 0.11.4 picks its u16 pipeline by default and
/// composites this exact pair to `#25553E`, and forced into its high-precision
/// pipeline the same two colours give `#24543D`. One page, one colour, two
/// answers, so the composite is done here and painted opaque.
///
/// Chromium's own blend of the pair is not a straight sRGB-space lerp, and the
/// arithmetic below is fitted to that measurement rather than derived: the exact
/// product at `64/255` is (36.0, 85.2, 61.1) and the reference measures (36, 84,
/// 61), which no single rounding of the product produces. What does produce all
/// three bytes is a source premultiplied to eight bits and rounded added to the
/// backdrop scaled by one minus the alpha and truncated -- the shape of Skia's u8
/// `source_over`. Each channel is read from the token table as the byte the
/// stylesheet declared, so the same blend carries to any theme the way Chromium's
/// compositor does.
fn plate(theme: Gen) -> Color {
    let [r, g, b, alpha] = theme_gen::ink_rgba(theme, Ink::ButtonBgSelected);
    let [raised_r, raised_g, raised_b, _] = theme_gen::ink_rgba(theme, Ink::RaisedBg);
    let over = |source: u8, backdrop: u8| -> f32 {
        let premultiplied = (source as u32 * alpha as u32 + 127) / 255;
        let behind = (backdrop as u32 * (255 - alpha as u32)) / 255;
        (premultiplied + behind) as f32 / 255.0
    };
    Color::from_rgb(over(r, raised_r), over(g, raised_g), over(b, raised_b))
}

/// The tabs a page switches between: a pill of buttons, the selected one plated.
///
/// `NavTabs.vue`'s own arrangement: a `relative flex w-fit rounded-full
/// bg-bg-raised p-1 text-xs sm:text-sm font-bold` track with `px-4 py-2` tabs
/// inside it and **no gap between them** -- the `p-1` is the only space, which is
/// why the track's own tabs touch. The labels are strings because a page's tabs
/// are not always locale keys of their own: an instance's are, and Discover's are
/// the project-type names `route.rs` already asserts against the reference.
///
/// A tab with no icon is a shape the reference has: the icon is `v-if="link.icon"`
/// on the link, and the two pages that pass none pass none.
///
/// Every caller of this function is a `NavTabs` in the reference's *navigation*
/// mode, which is what decides the strip's frame: `'card-shadow border
/// border-solid border-surface-4': mode === 'navigation'` (`NavTabs.vue:11`), and
/// nothing in the vendored tree passes `mode="local"` -- the three call sites are
/// the profile's `page-nav` strip, the browse tab's, and the hosting manager's. So
/// the border belongs on all five of this port's strips ([`TAB_STRIP`],
/// [`TAB_STRIP_BORDER`]). The `card-shadow` on the same element does not:
/// [`card_shadow`] is why.
pub fn tabs<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    keys: &[&'static str],
    labels: &[(String, bool)],
    on_select: impl Fn(usize) -> Message,
) -> Element<'a, Message> {
    let glyphs: Vec<Option<Glyph>> = vec![None; labels.len()];
    tabs_with_glyphs(theme, keys, &glyphs, labels, on_select)
}

/// The same strip with the icon each tab registers, which is `NavTabs.vue`'s
/// `size-5` icon in front of the label.
///
/// The icon is `tab-color hidden sm:block size-5` in navigation mode and a plain
/// `size-5` in local mode: `hidden` below Tailwind's `sm` -- 640 pixels -- and
/// shown above it, which is every window this launcher opens at. Its ink is
/// `getIconClasses`' own rule and it is *not* the label's rule: a selected icon
/// is `text-button-textSelected` like its label, but an inactive one is
/// `text-secondary` where the label is `text-contrast`. The label itself is
/// `text-nowrap` at the track's `text-sm`, and the pair sit in the link's own
/// `gap-2`.
pub fn tabs_with_glyphs<'a, Message: Clone + Hovered + 'a>(
    theme: Gen,
    keys: &[&'static str],
    glyphs: &[Option<Glyph>],
    labels: &[(String, bool)],
    on_select: impl Fn(usize) -> Message,
) -> Element<'a, Message> {
    // `v-if="filteredLinks.length > 1"`: a strip of one is not a strip.
    if labels.len() < 2 {
        return Space::new(Length::Shrink, Length::Shrink).into();
    }
    let mut track = row![].align_items(Alignment::Center);
    // The plate is the same colour on every selected tab, so it is worked out
    // once for the strip rather than per tab.
    let plate = plate(theme);
    for (index, ((label, selected), key)) in labels.iter().zip(keys.iter().copied()).enumerate() {
        let selected = *selected;
        let glyph = glyphs.get(index).copied().flatten();
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
        // The icon's own rule, which is a different ink from the label's.
        let icon_ink = if selected {
            theme_gen::ink(theme, Ink::ButtonTextSelected)
        } else {
            theme_gen::ink(theme, INK_SECONDARY)
        };
        let (factor, _) = interaction(key);
        // A selected tab is plated and an unselected one is not, and the
        // reference does not change that on hover: what a hover moves is the
        // *label* -- `text-secondary` to `text-primary` -- so the plate is left
        // alone and the ink is filtered. The plate is `bg-button-bgSelected`
        // composited over the track's own fill; see [`plate`] for why that
        // composite is worked out here rather than left to the rasteriser, which
        // answers `#25553E` on the backend this build selects where the
        // reference measures `#24543D`.
        let fill = selected.then_some(plate);
        let mut face = row![].align_items(Alignment::Center).spacing(TAB_GAP);
        if let Some(glyph) = glyph {
            face = face.push(icon::icon(
                glyph,
                TAB_ICON,
                crate::theme::brightness(icon_ink, factor),
            ));
        }
        let tab = container(
            face.push(
                text(label.clone())
                    .size(TAB_LABEL)
                    // `text-sm`'s own line: twenty pixels of fourteen-pixel text.
                    .line_height(iced::Pixels(TAB_LINE))
                    .font(inter(TAB_LABEL_WEIGHT))
                    .style(iced::theme::Text::Color(crate::theme::brightness(ink, factor))),
            ),
        )
        // `py-2` around the line, which is a 36-pixel tab; the port fixed it at 32
        // and let the label sit wherever the leftover space put it.
        .padding(Padding { top: 8.0, bottom: 8.0, left: TAB_PAD, right: TAB_PAD })
        .style(move |_theme: &Theme| container::Appearance {
            background: fill.map(Background::Color),
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
    // `p-1` on `NavTabs.vue:9` plus the `border border-solid border-surface-4`
    // that `NavTabs.vue:11` adds to the same element in navigation mode, which is
    // what makes the pill [`TAB_STRIP`] tall rather than the forty-four it was:
    // the port had the four pixels of `p-1` and none of the border.
    //
    // The border is drawn *inside* the strip's bounds -- CSS puts a border
    // outside the padding and so does iced -- so the height is set rather than
    // left to the padding, and it is the reference's own arithmetic:
    // 1 + 4 + 36 + 4 + 1.
    //
    // There is no `shadow` in this appearance and there is not one anywhere else
    // in this file: iced 0.12.3 paints a container's shadow as part of that
    // container's own quad, and on this pill the shadow reaches inside the fill.
    // [`card_shadow`] has the measurements.
    container(track)
        .height(Length::Fixed(TAB_STRIP))
        .padding(4.0)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
            border: Border {
                radius: 999.0.into(),
                width: TAB_STRIP_BORDER,
                color: theme_gen::ink(theme, Ink::Surface4),
            },
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

    /// The severity's own box: the border its rule names and the fill under it.
    ///
    /// `Admonition.vue`'s `typeClasses`: `border-brand-orange bg-bg-orange` for a
    /// warning, and the same pair in blue or red for the other two. The fill is
    /// the ten-odd-percent brand token itself, which composites over whatever the
    /// box sits on -- the capture's measured (76, 58, 43) is `--color-orange-bg`
    /// over this modal's own raised surface.
    fn surface(self, theme: Gen) -> (Color, Color) {
        match self {
            Severity::Info => (theme_gen::ink(theme, Ink::Blue), theme_gen::ink(theme, Ink::BlueBg)),
            Severity::Warning => {
                (theme_gen::ink(theme, Ink::Orange), theme_gen::ink(theme, Ink::OrangeBg))
            }
            Severity::Critical => {
                (theme_gen::ink(theme, Ink::Red), theme_gen::ink(theme, Ink::RedBg))
            }
        }
    }
}

/// An admonition with only a body, which is `Admonition.vue` with no header.
///
/// The reference's own box: `relative grid grid-cols-[1.5rem_minmax(0,1fr)_auto]
/// gap-x-2 rounded-2xl border border-solid p-4` with the severity's pair of
/// tokens, a 24-pixel `h-6 w-6` icon in the severity's colour beside the text, and
/// the body in `font-normal text-contrast/85 leading-tight` -- 16 pixels on
/// `leading-tight`'s own 20.
///
/// This is the shape the settings dialog's language warning takes: the reference
/// passes that pane an `Admonition` with nothing but the slot in it, and the
/// capture's own box measures 114 rows -- 16 of padding, four 20-pixel lines,
/// 16 more, and the two border pixels.
pub fn admonition_body<'a, Message: 'a>(theme: Gen, severity: Severity, body: &str) -> Element<'a, Message> {
    let (glyph, ink) = severity.icon_and_ink(theme);
    let (edge, fill) = severity.surface(theme);
    container(
        row![]
            .spacing(8.0)
            .align_items(Alignment::Start)
            .push(icon::icon(glyph, 24.0, ink))
            .push(
                text(body.to_string())
                    .size(16.0)
                    // `leading-tight` is 1.25 of 16, and iced reads a bare number
                    // as a multiple of the size (see [`NAV_LABEL_LINE`]).
                    .line_height(iced::Pixels(20.0))
                    .font(crate::style::regular())
                    .style(iced::theme::Text::Color(crate::style::at_opacity(
                        theme_gen::ink(theme, INK_CONTRAST),
                        0.85,
                    )))
                    .width(Length::Fill),
            ),
    )
    .width(Length::Fill)
    // `p-4` inside a `border border-solid`, which in CSS is sixteen of padding
    // with the border *outside* it -- seventeen rows before the text on every
    // side. iced paints a container's border inside its bounds and does not
    // spend a row on it, so the seventeen is the padding here: the box measures
    // 114 rows, which is what the reference's own capture measures. See
    // [`Shell::tabbed_dialog`] for the same border drawn the other way round.
    .padding(Padding::from(17.0))
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(fill)),
        border: Border { color: edge, width: 1.0, radius: 16.0.into() },
        ..container::Appearance::default()
    })
    .into()
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
    fn the_tab_strip_is_its_own_classes_added_up() {
        // `NavTabs.vue:9` wraps the tabs in a `p-1`, `NavTabs.vue:11` puts a
        // `border border-solid border-surface-4` around that, and a tab is a
        // `text-sm` line under `py-2`. So the pill is forty-six rows tall and the
        // reference measures forty-six: y=201..246 on its own 1280x720 capture of
        // /user/FlameFire, both border rows included.
        assert_eq!(TAB_HEIGHT, 36.0);
        assert_eq!(TAB_HEIGHT, TAB_LINE + 16.0);
        assert_eq!(TAB_STRIP, 46.0);
        assert_eq!(TAB_STRIP, TAB_HEIGHT + 8.0 + 2.0);
        assert_eq!(TAB_STRIP_BORDER, 1.0);
        // The border is `--surface-4`, and the reference's dark token is
        // `#34363c` (`assets/styles/variables.scss:238`) -- the byte at y=201 and
        // at y=246 of that capture.
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, Ink::Surface4), [0x34, 0x36, 0x3c, 0xff]);
        // `--shadow-card` still names all three of its numbers here, so that a
        // backend which can draw the shadow has them: iced's `Shadow` has no
        // `spread_radius` and this value's spread is 0px, so nothing of it is
        // lost on the way across.
        for theme in Gen::ALL {
            let shadow = card_shadow(*theme);
            assert_eq!(shadow.offset, iced::Vector::new(0.0, 2.0));
            assert_eq!(shadow.blur_radius, 4.0);
            assert!(shadow.color.a > 0.0, "{theme:?}: a shadow nobody can see");
        }
        // Dark's own colour, from `variables.scss:368`.
        assert_eq!(card_shadow(Gen::Dark).color, Color::from_rgba(0.0, 0.0, 0.0, 0.25));
    }

    #[test]
    fn the_tab_strip_carries_no_shadow_at_all() {
        // The claim this replaces was that only the shadow's *shape* is a
        // residual. It is not: iced 0.12.3 cannot draw the shadow here without
        // reaching inside the pill's own fill, and the pill's fill is the one
        // thing on this widget the reference is exact about.
        //
        // Measured on this port's own build at 1280x720, y=212 across the pill,
        // as multiples of the pill's own `#27292E`:
        //
        // | the shadow goes | x>=357 | x=243..356 | x=140..236 | x=237..242 |
        // | --- | --- | --- | --- | --- |
        // | on the pill | 1.0 | 0.750 | 0.5625 | 0.4219 |
        // | on a parent quad drawn first | 1.0 | 0.875 | 0.769 | 0.667 |
        // | nowhere | 1.0 | 1.0 | 1.0 | 1.0 |
        //
        // so the appearance `tabs_with_glyphs` builds must not carry a shadow,
        // and `draw_background`'s guard proves it will not draw one from
        // anywhere else: with `background` set it draws, but the shadow branch of
        // the solid pipeline is taken on `shadow.color.a > 0.0` alone, and
        // `container::Appearance::default()`'s is `Color::default()`, which is
        // `Color::TRANSPARENT`.
        assert_eq!(container::Appearance::default().shadow.color.a, 0.0);
        assert_eq!(iced::Shadow::default().color, Color::TRANSPARENT);
        // The pill is `#27292E` and the plate composites over exactly that, so the
        // fill a shadow lands on is not a colour this port chooses.
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, Ink::RaisedBg), [0x27, 0x29, 0x2e, 0xff]);
        // And the ring, the height and the padding are unchanged by this, because
        // the shadow was never part of any of them.
        assert_eq!(TAB_STRIP, 46.0);
        assert_eq!(TAB_STRIP_BORDER, 1.0);
        assert_eq!(TAB_PAD, 16.0);
    }

    #[test]
    fn the_selected_tabs_plate_is_the_reference_s_own_composite() {
        // `--color-button-bg-selected` is the brand green at a quarter alpha over
        // the track's own `#27292E`, and the reference's capture reads `#24543D`.
        // The plate is worked out rather than handed to the rasteriser because the
        // rasteriser's answer depends on which pipeline it picks: tiny-skia 0.11.4
        // composites this pair to `#25553E` on its default u16 pipeline and to
        // `#24543D` on its high-precision one, which is a one-byte difference in
        // two of the three channels and nothing anybody could see in a code
        // review.
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, Ink::ButtonBgSelected), [0x1b, 0xd9, 0x6a, 0x40]);
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, Ink::RaisedBg), [0x27, 0x29, 0x2e, 0xff]);
        assert_eq!(&plate(Gen::Dark).into_rgba8()[..3], &[0x24, 0x54, 0x3d]);
        // Opaque on purpose: the composite is already done, and a plate that
        // still carried the alpha would ask the backend for the same answer twice.
        assert_eq!(plate(Gen::Dark).a, 1.0);
        // Every theme composites rather than panicking, and none of them lands on
        // the plate of another: the blend is a compositor's, so it carries.
        for theme in Gen::ALL {
            let color = plate(*theme);
            assert_eq!(color.a, 1.0, "{theme:?}");
        }
        assert_ne!(plate(Gen::Dark), plate(Gen::Light));
    }

    #[test]
    fn a_tags_label_is_the_secondary_ink_and_not_the_default_one() {
        // `TagItem.vue`'s `baseClass` ends in `text-[--_color,
        // var(--color-secondary)]`, which the reference's own table resolves to
        // `--color-text-tertiary` (`variables.scss:337`, `:321`). The port drew
        // `--color-text-default` here and the capture reads `#96A2B0` on every tag
        // pill of the /user/FlameFire page. Asserted on the token rather than on
        // the widget because the number the reference fixes is the token's.
        assert_eq!(
            theme_gen::ink_rgba(Gen::Dark, Ink::Secondary),
            [0x96, 0xa2, 0xb0, 0xff]
        );
        assert_eq!(theme_gen::ink(Gen::Dark, Ink::Secondary), theme_gen::ink(Gen::Dark, INK_SECONDARY));
        assert_ne!(
            theme_gen::ink(Gen::Dark, Ink::Secondary),
            theme_gen::ink(Gen::Dark, INK_DEFAULT),
            "the default ink is `--color-text-default`, which is not what TagItem sets"
        );
        // And the height is the no-icon one, because no caller draws the `h-4`
        // glyph that would make it 26.
        assert_eq!(TAG_HEIGHT, 24.0);
    }

    #[test]
    fn a_tags_label_is_text_sm_at_weight_normal() {
        // `TagItem.vue:19` ends in `... rounded-full font-normal text-sm ...`, so
        // the label is fourteen pixels at weight 400. The port drew it at twelve and
        // semibold. Measured against the reference's own /user/FlameFire capture at
        // 1280x720, on the two pills neither of them puts an icon in: *Combat* is
        // eleven ink rows there and nine here, and *Challenging* is ten pixels wider
        // there (96 against 86) because a fourteen-pixel label is a wider label.
        assert_eq!(TAG_LABEL_SIZE, 14.0);
        assert_eq!(TAG_LABEL_SIZE, TAG_HEIGHT - 2.0 - 8.0, "1 + 4 + 14 + 4 + 1");
        assert_eq!(regular().weight, iced::font::Weight::Normal);
        assert_ne!(regular().weight, semibold().weight);
        // `px-2` and `gap-1` off the same class string, which are the two other
        // numbers it fixes and which a caller reading only the height cannot see.
        assert_eq!(TAG_PAD, 8.0);
        assert_eq!(TAG_GAP, 4.0);
    }

    #[test]
    fn a_tags_height_is_the_taller_of_its_own_two_children() {
        // `leading-none` puts the label's line box at its own size, so the pill is
        // 1 + 4 + line + 4 + 1 with no icon and 1 + 4 + icon + 4 + 1 with one.
        // `[&>svg]:h-4 [&>svg]:w-4` on the same `baseClass` is the sixteen-pixel
        // icon that takes the second sum.
        assert_eq!(TAG_ICON, 16.0);
        // `py-1` is four above and four below, and `border-[1px]` is one each side.
        let py = 4.0;
        let border = 1.0;
        assert_eq!(TAG_HEIGHT, border + py + TAG_LABEL_SIZE + py + border);
        assert_eq!(TAG_HEIGHT_ICON, border + py + TAG_ICON + py + border);
        assert_eq!(tag_height(false), TAG_HEIGHT);
        assert_eq!(tag_height(true), TAG_HEIGHT_ICON);
        assert_ne!(tag_height(true), tag_height(false));
        // The icon is the taller child, which is the whole reason there are two
        // heights at all rather than one height and a guess.
        assert!(tag_height(true) > tag_height(false));
    }

    #[test]
    fn a_tags_ring_is_one_pixel_of_surface_five() {
        // `border-[--_bg-color,var(--surface-5)] border-[1px] border-solid` off the
        // same `baseClass`, which the reference's table gives as `#42444a` in the
        // dark look (`variables.scss:239`) and `#dddddd` in the light one
        // (`variables.scss:8`). The capture reads `#42444A` on the top edge of
        // every pill of the /user/FlameFire page, and the port drew no ring at all.
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, Ink::Surface5), [0x42, 0x44, 0x4a, 0xff]);
        assert_eq!(theme_gen::ink_rgba(Gen::Light, Ink::Surface5), [0xdd, 0xdd, 0xdd, 0xff]);
        // The fill beside it is `--color-button-bg`, which is `--surface-4`
        // (`variables.scss:329` into `:238`) -- the `#34363C` the capture reads and
        // which was already right, and which is *not* the ring: two different
        // surfaces, one apart.
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, Ink::ButtonBg), [0x34, 0x36, 0x3c, 0xff]);
        assert_ne!(theme_gen::ink(Gen::Dark, Ink::Surface5), theme_gen::ink(Gen::Dark, Ink::ButtonBg));
    }

    #[test]
    fn every_tag_builds_in_every_theme() {
        for theme in Gen::ALL {
            let face: Element<'_, ()> = tag(*theme, "Challenging");
            drop(face);
            let with_icon: Element<'_, ()> = tag_with_icon(*theme, "Forge", Glyph::TagLoaderForge);
            drop(with_icon);
        }
    }

    #[test]
    fn a_tag_with_an_icon_is_the_pill_the_reference_measures_at_twenty_six() {
        // The three numbers `tag_with_icon` changes, all off `TagItem.vue`'s one
        // class string, and the reason the pill has two heights rather than one:
        //
        // * `inline-flex items-center` puts the `[&>svg]:h-4` glyph and the
        //   `leading-none` label on one line, so the line is the taller of them;
        // * `gap-1` puts four pixels between them;
        // * `rounded-full` is a rule CSS resolves against the border box, so the
        //   radius is half the height and not a fixed twelve.
        //
        // Measured on the reference's own /user/FlameFire capture: *Client and
        // server*, *Forge* and *Modpack* are 26 rows and *Challenging*, *Combat*
        // and *+1* are 24, and every 26 drew an icon. The port drew all six at 24
        // with none.
        assert_eq!(tag_height(true), 1.0 + 4.0 + TAG_ICON + 4.0 + 1.0);
        assert_eq!(tag_height(false), 1.0 + 4.0 + TAG_LABEL_SIZE + 4.0 + 1.0);
        assert_eq!(TAG_GAP, 4.0, "`gap-1` between the icon and the label");
        // And the icon takes its colour from the label, because `baseClass` gives
        // `[&>svg]` a size and no ink of its own.
        assert_eq!(theme_gen::ink(Gen::Dark, Ink::Secondary), theme_gen::ink(Gen::Dark, INK_SECONDARY));
    }

    #[test]
    fn a_tag_icon_is_the_reference_get_tag_icon() {
        // `getTagIcon` (`assets/index.ts:176`) is `getLoaderIcon(tag)` and then
        // `getCategoryIcon(tag)`, each a lookup in a table the reference generates
        // from a directory of SVGs. The three loaders on the reference's own first
        // card and the two categories beside them are the cases that matter: the
        // loaders answer, the categories answer under `getTagIcon`, and a card
        // does not ask for a category because it passes `hide-non-loader-icon`.
        assert_eq!(tag_icon("forge"), Some(Glyph::TagLoaderForge));
        assert_eq!(tag_icon("mrpack"), Some(Glyph::TagLoaderMrpack));
        assert_eq!(tag_icon("datapack"), Some(Glyph::TagLoaderDatapack));
        assert_eq!(tag_icon("challenging"), Some(Glyph::TagCategoryChallenging));
        assert_eq!(tag_icon("combat"), Some(Glyph::TagCategoryCombat));
        assert_eq!(tag_icon("mobs"), Some(Glyph::TagCategoryMobs));
        assert_eq!(tag_icon("worldgen"), Some(Glyph::TagCategoryWorldgen));
        // A tag neither table has is no icon, which is the `+N` pill: `TagsOverflow`
        // writes `+{{ tags.length }}` into a plain `TagItem` with no slot in it.
        assert_eq!(tag_icon("+1"), None);
        assert_eq!(tag_icon("1.20.1"), None);
        assert_eq!(tag_icon(""), None);
    }

    #[test]
    fn is_loader_is_the_reference_tag_loader_table() {
        // `getTagMessage(tag, 'loader') !== undefined` (`TagTagItem.vue:30`). The
        // three loaders on the reference's first card answer true and the two
        // categories beside them answer false, which is what makes *Client and
        // server*, *Forge* and *Modpack* the three pills that grow an icon and
        // *Challenging* and *Combat* the two that do not.
        for loader in ["forge", "mrpack", "fabric", "neoforge", "datapack", "quilt"] {
            assert!(is_loader_tag(loader), "{loader} is a loader");
        }
        for category in ["challenging", "combat", "mobs", "worldgen", "minigame"] {
            assert!(!is_loader_tag(category), "{category} is not a loader");
        }
        // `minecraft` is the case the reference's own comment is about: it is a
        // loader for a resource pack and a category for a mod, and `getTagIcon`
        // looks the loader table first for exactly that reason.
        assert!(is_loader_tag("minecraft"));
        assert_eq!(tag_icon("minecraft"), Some(Glyph::TagLoaderMinecraft));
    }

    #[test]
    fn every_tag_glyph_in_the_two_tables_is_reachable_by_its_tag() {
        // The two functions above are a hand-written table over the generator's two
        // generated ones, so the gate is that they agree: every key in
        // `TAG_LOADERS` answers through `tag_icon`, and no key is claimed that the
        // generator did not emit. A tag the reference adds upstream without this
        // port hearing fails here rather than drawing a pill with no icon in it.
        for (name, _) in crate::icons_gen::TAG_LOADERS {
            let tag = name.rsplit('/').next().unwrap_or(name);
            assert_eq!(
                tag_icon(tag),
                crate::icons_gen::Glyph::by_name(name),
                "{tag} is in the generator's loader table and not in tag_icon"
            );
        }
        for (name, _) in crate::icons_gen::TAG_CATEGORIES {
            let tag = name.rsplit('/').next().unwrap_or(name);
            // `getTagIcon` asks the loader table first, so a category whose stem
            // is also a loader key answers with the loader's glyph. `minecraft` is
            // the only such pair, and `getTagIcon` gives it the loader, so this is
            // asserted rather than assumed.
            let expected =
                if crate::icons_gen::TAG_LOADERS.iter().any(|(loader, _)| *loader == tag) {
                    crate::icons_gen::Glyph::by_name(&format!("tags/loaders/{tag}"))
                } else {
                    crate::icons_gen::Glyph::by_name(name)
                };
            assert_eq!(tag_icon(tag), expected, "{tag}");
        }
    }

    #[test]
    fn the_four_loaders_with_no_glyph_are_the_ones_the_generator_refuses() {
        // `icons_gen.rs` refuses four of the thirty loader icons rather than
        // approximating what they use: a non-uniform transform scale in three
        // cases and a `clip-path` in the fourth. A pill for one of them draws no
        // icon, which makes it the 24-row pill -- the height a tag with no icon
        // has, and not a wrong shape.
        for tag in ["geyser", "legacy-fabric", "purpur", "quilt"] {
            assert!(is_loader_tag(tag), "{tag} is a loader the reference has");
            assert_eq!(tag_icon(tag), None, "{tag} has a glyph this port does not draw");
        }
        assert_eq!(crate::icons_gen::TAG_LOADERS.len(), 26, "30 less the four refused");
        assert_eq!(crate::icons_gen::TAG_CATEGORIES.len(), 102);
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
