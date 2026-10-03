//! The shell: the chrome every page is drawn inside.
//!
//! This shell is the reference's own: the rail, the head, the page pane, the right
//! panel, the window controls and Settings as a modal, all painted from
//! [`crate::theme_gen`] and paced by [`crate::motion`], on the information
//! architecture [`crate::route`] describes. Every page [`crate::route`] can address
//! is drawn, from [`crate::pages`].
//!
//! Every number below is quoted from the reference's own `App.vue`, with the
//! declaration it came from:
//!
//! | Number | Source |
//! | --- | --- |
//! | 48px head | `--top-bar-height: 3rem` |
//! | 64px rail | `--left-bar-width: 4rem` |
//! | 300px right panel | `--right-bar-width: 300px` |
//! | 48px rail plate, 4px gaps, 8px padding | `.nav-button`'s `w-12 h-12`, the rail's `gap-[0.25rem] p-[0.5rem]` |
//! | 24px rail icon | `text-2xl` on the same button |
//! | 20px page radius | `.app-contents`'s `border-top-left-radius: var(--radius-xl)` |
//! | 28px head buttons, 16px chevrons | `!h-7 !w-7`, `!size-4` |
//! | 36px control buttons, 20px icons, 16px corner | the `md` icon-only `IconButton` (`h-9 w-9`, `size-5`) and `WindowControls`' `rounded-bl-2xl` |
//!
//! Three things about the reference's chrome are easy to get backwards, and each
//! is written down where it bites:
//!
//! * **The "status bar" is at the top.** `app-grid-statusbar` is the first row of
//!   the grid and holds the logo, the history buttons, the breadcrumbs, the
//!   sidebar toggle and the window controls. There is no bottom bar.
//! * **`text-primary` is not `--color-text-primary`.** The preset remaps
//!   Tailwind's names onto different tokens (`primary: var(--color-text-default)`,
//!   `contrast: var(--color-text-primary)`, `secondary: var(--color-text-tertiary)`),
//!   so a port that reads the class name as the token draws the wrong ink.
//!   [`INK_DEFAULT`] and friends name the class, not the token.
//! * **The window controls are an overlay, not a row item.** In the reference
//!   they are `position: fixed` at the window's top-right and the status bar
//!   reserves their measured width with `padding-right`. Drawing them as the last
//!   item of the row paints the same picture -- the bar and the controls are the
//!   same colour and the same height -- and [`CONTROLS_WIDTH`] is the reservation
//!   the reference publishes, computed here rather than measured.

use std::collections::BTreeMap;
use std::time::Duration;

use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path};
use iced::widget::{column, container, image, mouse_area, row, text_input, Space};
use iced::window;
use iced::advanced::widget::{tree, Tree};
use iced::advanced::{layout, mouse, renderer, Layout, Widget};
use iced::{
    gradient, mouse::{Cursor, Interaction}, window::Id, Alignment, Background, Border, Color,
    Element, Length, Padding, Point, Radians, Rectangle, Renderer, Subscription, Theme, Vector,
};

use crate::accounts::{AccountEntry, AccountsStore};
use crate::anim;
use crate::brand;
use crate::color_theme::ColorTheme;
use crate::launch::{self, ActiveRunData, ChildSlot};

use crate::checklist::{Checklist, Step};
use crate::icon;
use crate::style::{
    disabled, heading, medium, semibold, INK_CONTRAST, INK_DEFAULT, INK_HOVER_BG, INK_PLATE,
    INK_PLATE_TEXT, INK_SECONDARY,
};
use crate::icons_gen::{self, Glyph};
use crate::install;
use crate::instances::InstanceCard;
use crate::motion::{Timing, Tween};
use crate::page::{Load, ROW_GAP};
use crate::text_gen::Key;
use crate::pages::{self, discover, home, instance, project, screenshots, servers, skins, user, Screen};
use palantir_net::engine::Search as ApiSearch;
use palantir_net::modrinth::{NewsArticle, NEWS_PAGE_URL};
use crate::route::{self, Address, Mark, Rail};
use crate::store::{self, Engine, Store};
use crate::ui::{text, Hovered};
use crate::theme_gen::{self, Ink, Raw, Theme as Gen};

/// What asking the reader for a file left to do.
///
/// Two things, and the second is not an error path: a dialog that was cancelled, a
/// file that could not be read, bytes that are not a skin and a build with no picker
/// all end the interaction with nothing to send, and what the page needs is the answer
/// rather than a request. `skins::Picked` is where that distinction is a type.
enum Choice {
    /// A texture to upload.
    Upload(palantir_net::SkinChange),
    /// Nothing to send, and this is what the page is told.
    Answered(skins::Picked),
}

/// A request a turn left for this shell to run off the frame thread.
///
/// [`Shell::act`]'s answer was `Option<discover::Asked>` while Discover was the
/// only page that asked for anything. The project page made it two, and this is
/// an enum rather than the one type widened: what a page asks for is a *value*
/// only that page can build, and each one travels back to its own page as a
/// message of that page's own.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Asked {
    /// A search, as Discover describes it.
    Search(discover::Asked),
    /// The icons of a page of results, as Discover describes them.
    ///
    /// The only ask here whose answer is *decoration*: nothing waits for it, no page
    /// state turns on it, and a picture that will not come back leaves a card's box
    /// empty rather than a page half drawn (see [`crate::avatar`]).
    Icons(discover::Icons),
    /// The tag list, as the browse sidebar's filter options are made of.
    Tags,
    /// A project, as the project page describes it.
    Project(project::Asked),
    /// A profile, as the user page describes it.
    ///
    /// The one request whose *address* is part of the question: a profile is read
    /// by the name the reader navigated to, which is why the name travels with the
    /// request rather than being read here.
    User(user::Asked),
    /// The account's own appearance, as the Skins page describes it.
    ///
    /// The shortest of the three, and the only one the shell answers from the
    /// *launcher's* own files rather than from the network alone: which account is
    /// the one a launch would sign in as, and the token that account carries, are
    /// both in the accounts store this shell owns.
    Skins(skins::Asked),
    /// A change to what the account wears, as the Skins page describes it.
    ///
    /// The only *write* a page can ask for, and the reason it is here rather than
    /// on the page: it is made with the account's own game token against
    /// Minecraft's skin service, and both the account and its token are the
    /// shell's.
    Wear(skins::Wear),
    /// Add a skin from a file the reader picks, as the Skins page describes it.
    ///
    /// The one ask that is not a request at all: it is a dialog, a read and a padding
    /// *here*, on the frame thread the window lives on, followed by the same write
    /// [`Asked::Wear`] makes. The page has no part in any of the three, which is why it
    /// asks for the round rather than describing a change.
    AddSkin(skins::Add),
    /// One tab's own listing, as the instance page describes it.
    ///
    /// The only read here that is not about a *document*: it is a folder on this
    /// machine, and it is asked for through the shell so it happens once per tab
    /// rather than once per frame. A page that read it while drawing made every
    /// frame a directory walk -- `crate::scale` measured the Files tab at 2,304 ms
    /// of frame at five thousand entries before this moved.
    Instance(instance::Asked),
    /// Do what the Skins page's editor asked for: write a stored row and put it on,
    /// forget it, or take the account's skin off.
    ///
    /// The third kind of thing a page can ask the shell for, and the first that
    /// touches this launcher's *own* files: `crate::saved_skins` is a folder under
    /// this product's directory, and a page has never read a file in this tree --
    /// the same rule the picker follows, for the same reason. Two of its three
    /// actions also need the account's token, which is the shell's.
    EditSkin(skins::Edit),
    /// Write the Skins page's saved rows in the order the reader put them in.
    ///
    /// The only ask here with no account and no service in it: the order lives in
    /// this launcher's own store ([`crate::saved_skins`]), which is a folder under
    /// the product's directory and therefore the shell's -- the reference reaches
    /// the same write through its own store's `set_custom_skin_order`.
    Reorder(skins::Reorder),
}

// ---- Geometry, quoted from the reference --------------------------------

/// `--left-bar-width: 4rem`.
pub const RAIL: f32 = 64.0;
/// `--top-bar-height: 3rem`, shared by the head and the window controls.
pub const BAR: f32 = 48.0;
/// `--right-bar-width: 300px`.
pub const PANEL: f32 = 300.0;
/// `.nav-button`'s `w-12 h-12`.
pub const PLATE: f32 = 48.0;
/// `rounded-full`.
pub const PLATE_RADIUS: f32 = PLATE / 2.0;
/// The rail's `p-[0.5rem]`, appplied to both sides and the foot.
pub const RAIL_PAD: f32 = 8.0;
/// The rail's `gap-[0.25rem]`.
pub const RAIL_GAP: f32 = 4.0;
/// `h-px w-8` on the switcher's separator rule.
pub const RAIL_RULE: f32 = 32.0;
/// The rail's link group, in `App.vue`'s order: the five slots above the
/// quick-instance switcher. The two conditional ones are listed here anyway and
/// left out when the settings turn them off; see [`Shell::nav_slots`].
pub const RAIL_NAV: [Rail; 5] = [
    Rail::Home,
    Rail::Discover,
    Rail::Skins,
    Rail::Screenshots,
    Rail::Servers,
];
/// The rail's foot, in `App.vue`'s order. The switcher's growth is what holds
/// these two on the rail's floor; see [`Shell::rail`].
pub const RAIL_FOOT: [Rail; 2] = [Rail::Settings, Rail::Profile];
/// How far apart the centres of two rail buttons are. The pitch `REFERENCE.md`
/// recorded as 52, kept as arithmetic so it cannot drift from its parts.
#[cfg(test)]
pub const RAIL_PITCH: f32 = PLATE + RAIL_GAP;
/// `text-2xl` on the rail button: a 24-unit icon drawn at 24 pixels.
pub const RAIL_ICON: f32 = 24.0;
/// `!h-7 !w-7` on the head's history buttons.
pub const HEAD_BUTTON: f32 = 28.0;
/// `!size-4` on the chevrons inside them.
pub const HEAD_ICON: f32 = 16.0;
/// `TextLogo class="h-7"`. The mark is square where the reference's wordmark is
/// wide, which `REFERENCE.md` records as a difference in the art rather than in
/// the head.
pub const LOGO: f32 = 28.0;
/// `border-top-left-radius: var(--radius-xl)` on `.app-contents`.
pub const PAGE_RADIUS: f32 = 20.0;
/// The `md` icon-only `IconButton`: `h-9 w-9`.
pub const CONTROLS_BUTTON: f32 = 36.0;
/// `size-5` on the icons inside them.
pub const CONTROLS_ICON: f32 = 20.0;
/// `rounded-bl-2xl` on the controls' own surface.
pub const CONTROLS_RADIUS: f32 = 16.0;
/// `px-1.5` on that surface.
pub const CONTROLS_PAD: f32 = 6.0;
/// `gap-2` between the controls.
pub const CONTROLS_GAP: f32 = 8.0;
/// What `.app-grid-statusbar`'s `padding-right: var(--window-controls-width)`
/// reserves.
///
/// The reference *measures* its controls with a `ResizeObserver` and publishes
/// the result, because a browser can lay the row out and this shell cannot. The
/// arithmetic is the same either way: two paddings, three buttons, two gaps.
pub const CONTROLS_WIDTH: f32 = 2.0 * CONTROLS_PAD + 3.0 * CONTROLS_BUTTON + 2.0 * CONTROLS_GAP;

// ---- The scrollbar ------------------------------------------------------

/// How wide the reference's scrollbar is: six pixels.
///
/// Not iced's number, which is **ten**. That is the whole of the defect this
/// section exists for, and it has three parts that have to agree:
///
/// * `iced_widget-0.12.3/src/scrollable.rs:157-167` — `Properties::default()` is
///   `{ width: 10.0, margin: 0.0, scroller_width: 10.0 }`.
/// * `iced_widget-0.12.3/src/scrollable.rs:1383-1401` — the bar goes at
///   `bounds.right - total/2 - width/2`, so it is drawn *over* the region's last
///   pixels rather than beside them. A ten-pixel band over a page's own padding
///   is a band in the middle of the page.
/// * `iced_style-0.12.1/src/theme.rs:1188-1214` — a region with no `.style()` gets
///   `Scrollable::Default`, whose track is `palette.background.weak.color` and
///   whose thumb is `palette.background.strong.color`. With this shell's palette
///   (`widget_theme`) in dark those two mixes come out as `#4C5156` and `#757C84`
///   exactly — which is why the colours this port drew are in no token table and
///   where they came from.
///
/// Six is what the reference's sidebar draws, and it is
/// `overlayscrollbars`' own default: `App.vue:940-946` configures that instance
/// with `overflow` and nothing else, so every other knob — `--os-size`,
/// `--os-padding-inline`, `--os-track-bg`, the handle's radius — is the library's.
/// `App.vue:2839-2844` then sets `--os-handle-bg: var(--color-scrollbar)`, which
/// `variables.scss:331` remaps onto `--surface-5` and `variables.scss:239` fixes
/// at `#42444a`. Measured, on every reference capture, the bar occupies
/// `x=1272..1277` of a 1280x720 window with two clear columns beside it.
pub const SCROLLBAR: f32 = 6.0;

/// The clear track either side of [`SCROLLBAR`], which is what the two columns at
/// `x=1278..1279` in that same capture are: `overlayscrollbars`' default
/// `--os-padding-inline`.
///
/// It is also what puts the bar where it belongs. iced adds
/// `2 * margin` to `max(width, scroller_width)` for the space a region reserves,
/// so ten pixels of reserve over a six-pixel bar is two either side — and the
/// reservation is what a dialog's body is broken for
/// ([`DIALOG_SCROLLBAR`]), so that arithmetic stays true there too.
pub const SCROLLBAR_PAD: f32 = 2.0;

/// The scrollbar the reference paints, in iced's own terms.
///
/// The track is `None` and not a colour because the reference's is transparent —
/// `*::-webkit-scrollbar-track { background: transparent }`
/// (`app-frontend/src/assets/stylesheets/global.scss:137-139`) for a native bar,
/// `overlayscrollbars`' default `--os-track-bg` for the sidebar's — so what shows
/// through is whatever the region is drawn on. The handle is `--color-scrollbar`,
/// which is [`Ink::Scrollbar`]: `theme_gen.rs:632` already carries it, and its
/// dark value is `#42444a`.
///
/// The radius is a third of the width rather than `--radius-lg` (1rem) because a
/// radius wider than half a six-pixel box *is* half a six-pixel box, and the
/// reference's capture shows the handle with fully rounded ends.
///
/// `hovered` and `dragging` answer the same as `active` on purpose. The
/// reference's only hover rule is `*::-webkit-scrollbar:hover { opacity: 1 }`
/// (`global.scss:133-135`) off a base of `opacity: 0.5`, and the capture measures
/// the bar at full `--color-scrollbar` with the pointer nowhere near it — so there
/// is no measured hover colour, and a brighter one would be a guess about a
/// reference this port is matching pixel for pixel.
pub struct Bar {
    /// The generated theme, carried rather than taken from iced's `Theme`: a
    /// stylesheet is handed `&iced::Theme` and this port's tokens are not in it,
    /// which is why every other themed closure in this file captures a [`Gen`]
    /// and ignores the argument it is given.
    theme: Gen,
}

impl Bar {
    /// The one appearance, which is what the bar is at rest, over it and
    /// dragged.
    fn appearance(&self) -> iced::widget::scrollable::Appearance {
        iced::widget::scrollable::Appearance {
            container: iced::widget::container::Appearance::default(),
            scrollbar: iced::widget::scrollable::Scrollbar {
                background: None,
                border: Border::default(),
                scroller: iced::widget::scrollable::Scroller {
                    color: theme_gen::ink(self.theme, Ink::Scrollbar),
                    border: Border {
                        radius: (SCROLLBAR / 3.0).into(),
                        ..Border::default()
                    },
                },
            },
            gap: None,
        }
    }
}

impl iced::widget::scrollable::StyleSheet for Bar {
    type Style = Theme;

    fn active(&self, _theme: &Theme) -> iced::widget::scrollable::Appearance {
        self.appearance()
    }

    fn hovered(
        &self,
        _theme: &Theme,
        _is_mouse_over_scrollbar: bool,
    ) -> iced::widget::scrollable::Appearance {
        self.appearance()
    }
}

/// The style a region that shows the reference's bar hands to `Scrollable::style`.
///
/// [`Bar`] is a stylesheet while `Scrollable::style` takes the theme's style
/// enum, so the conversion lives here rather than at each of the five call sites:
/// a region writes `.style(crate::shell::bar(theme))`.
///
/// [`DIALOG_SCROLLBAR`] is what the ten-pixel reserve is *for*, and it is that
/// constant rather than a coincidence: the dialog bodies this file builds are
/// broken for ten pixels so that nothing sits under a bar, and a six pixel bar
/// with two pixels either side is exactly what now goes there.
fn bar(theme: Gen) -> iced::theme::Scrollable {
    iced::theme::Scrollable::custom(Bar { theme })
}

/// [`SCROLLBAR`] wide with [`SCROLLBAR_PAD`] of clear track either side, as
/// `iced::widget::scrollable::Properties` describes it.
///
/// Width and margin are both needed and they are not the same knob: `width` is
/// what is drawn and `margin` is what is reserved, and the reserve is what iced
/// centres the bar inside — `x = bounds.right - (max(width, scroller_width) +
/// 2 * margin) / 2 - width / 2` — which is how a six-pixel bar lands on
/// `x=1272..1277` rather than flush against the window's edge.
fn bar_direction() -> iced::widget::scrollable::Direction {
    iced::widget::scrollable::Direction::Vertical(
        iced::widget::scrollable::Properties::new()
            .width(SCROLLBAR)
            .scroller_width(SCROLLBAR)
            .margin(SCROLLBAR_PAD),
    )
}

/// The room iced's own bar reserves at a region's right edge, which is the width
/// it draws and its default: [`Properties::default`]'s `width` and `margin` are
/// both 10.0 and 0.0, so `max(10, 10) + 0` is ten.
///
/// Only the page column needs this number, and only because its region is built
/// in another file: see [`pane_gutter`].
const ICED_BAR: f32 = 10.0;

/// How many of [`ICED_BAR`]'s columns the pane paints in the page's own
/// background rather than in the reserved gutter's ink.
///
/// The strip covers the whole ten, because iced's bar is drawn *over* the
/// page's content — `iced_widget-0.12.3/src/scrollable.rs:909-918` lays the
/// content down and `scrollable.rs:960-975` lays the bar on top of it — so the
/// only way to keep the bar off the page is to paint over it. Its first four
/// columns (`x=970..973`) are painted in `Ink::Bg`, which is the page's own
/// background and what the reference measures in them
/// (`ref/user-ref.png`, `x=964..973 #16181C`), so the page's background runs
/// unbroken to the gutter; the remaining six are the gutter itself.
const GUTTER_BAR: f32 = ICED_BAR - GUTTER;

/// The column the reference reserves at the page's right edge, and the ink it
/// paints there.
///
/// `.app-viewport` (`App.vue:2747-2752`) is `overflow: auto` with
/// `scrollbar-gutter: stable`, so it reserves its scrollbar's width at its right
/// edge whether or not anything has been scrolled, and nothing is painted in it:
/// the track is transparent (`global.scss:137-139`) and the thumb paints nothing
/// (`global.scss:141-146` — `--color-scrollbar` behind a five-pixel transparent
/// border with `background-clip: content-box`, over a ten-pixel content box).
///
/// Every reference capture measures the same six columns, on every route and on a
/// scrolled page alike (`/tmp/ref/project-scroll.png` has the identical band), and
/// the band is five columns of one ink with a sixth that is a blend of it and the
/// page beside it:
///
/// ```text
/// ref, x=964..973         #16181C   (the page's own background, = --surface-1)
/// ref, x=974              #16181B   (one unit of blue down: the band's own edge)
/// ref, x=975..979         #15171B   (five columns, flat, the whole rest of the pane)
/// ref, x=980              the sidebar's own 1px border, --brand-gradient-border
/// ```
///
/// `#15171B` is not a token and is not declared anywhere in the vendored
/// reference: `variables.scss:233` fixes `--surface-1` at `#16181c` and nothing
/// darkens it by one unit per channel. It is `--color-bg` under the same
/// `rgba(0, 0, 0, 0.1)` the pane's own inset shadow is, at the depth that shadow
/// holds across its middle six pixels — see [`SHADOW_MID`], whose five bands are
/// the same three values this one is made of. So the band is drawn in the same
/// ink, and the two measurements stand or fall together.
const GUTTER: f32 = 6.0;

// ---- The pane's inset shadow --------------------------------------------
//
// `.app-contents::before` (`App.vue:2755-2770`) carries
// `box-shadow: 1px 1px 15px rgba(0, 0, 0, 0.1) inset` beside its one-pixel
// `--surface-5` rule, and both are painted *over* the page: the pseudo-element is
// `z-index: 30` and `position: fixed`, and `.app-contents` declares no border of
// its own, so the rule and the shadow are layers and not layout.
//
// The reference measures the shadow as three depths over the pane's own
// `--surface-1`, and the same three on both of the edges it shows on. Along the
// left edge (`ref/user-ref.png`, `y=300`) and down the top (`x=300`):
//
// ```text
// x=64 / y=48   #42444A   the rule itself, --surface-5
// x=65 / y=49   #15171A   SHADOW_HARD   one column, one row
// x=66..71      #15171B   SHADOW_MID    six columns, six rows
// x=72 / y=56   #16181B   SHADOW_SOFT   one column, one row
// x=73 / y=57   #16181C   --surface-1, no shadow left
// ```
//
// Eight pixels on two edges of every page, on every route in every reference
// capture. The three numbers are not declared in the reference -- `0.1` over
// `#16181c` is, and which of the three an eight-pixel-wide slice of a fifteen-
// pixel blur lands on is not -- so they are measured, and they are measured as
// *depths* rather than as inks because they are the same three the reserved
// gutter is made of ([`GUTTER`]): `22 * (1 - a)` and the two others land on
// `(21, 23, 26)`, `(21, 23, 27)` and `(22, 24, 27)` at the three values below,
// which is what the capture reads.

/// The depth of the reference's inset shadow on its first column and row.
const SHADOW_HARD: f32 = 0.058;
/// The depth it holds across the six columns and rows inside that.
const SHADOW_MID: f32 = 0.038;
/// The depth of the last one before the shadow is gone.
const SHADOW_SOFT: f32 = 0.019;
/// The three widths, which are measured rather than derived: `x=65`, `x=66..71`
/// and `x=72` down the pane's left edge, and `y=49`, `y=50..55` and `y=56` across
/// its top. The two runs are the same run of eight, which is why they are one
/// number's worth of constants rather than four.
const SHADOW_HARD_W: f32 = 1.0;
const SHADOW_MID_W: f32 = 6.0;
const SHADOW_SOFT_W: f32 = 1.0;
/// How wide the shadow's own reach is: one hard, six mid, one soft.
const SHADOW_REACH: f32 = SHADOW_HARD_W + SHADOW_MID_W + SHADOW_SOFT_W;

/// The column a region's bar starts at, counted from the region's own right edge
/// back into it.
///
/// iced's own arithmetic, written out rather than reached into:
/// `scrollable.rs:1383-1401` reserves `max(width, scroller_width) + 2 * margin`
/// and then centres `width` inside that reserve, so
/// `x = right - total / 2 - width / 2`. It is spelled here because the number it
/// has to produce is a measured one — `x=1272` in a window 1280 wide, which is
/// what every reference capture shows — and the margin is the only thing standing
/// between that and a bar flush against the window's edge.
///
/// [`SCROLLBAR_PAD`] is what makes the reserve and the bar different numbers, and
/// iced needs both: `margin` is the reserved space and `width` is the painted
/// space, and a region reserves the first while showing the second.
///
/// `#[cfg(test)]` because iced does the arithmetic at run time and this only says
/// what it comes to; [`Shell::panel`] is where the answer has to be right.
#[cfg(test)]
fn bar_at(right: f32) -> f32 {
    // `scroller_width` is set to the same six, so the wider of the two is six.
    let reserve = SCROLLBAR + 2.0 * SCROLLBAR_PAD;
    right - reserve / 2.0 - SCROLLBAR / 2.0
}

// ---- The panel's first section -----------------------------------------

/// `p-4` on each of the sidebar's sections, and `text-base` on the heading
/// inside one.
const PANEL_SECTION_PAD: f32 = 16.0;
/// The ad block's three measurements, all off a 1280x720 plate of the reference
/// rather than off its class list.
///
/// The reference writes the ad's height twice -- `min-h-[250px]` on the link and
/// `bottom-[250px]` on the block above it -- and the 300 comes from the image's
/// own `min-w-[300px]`. The link's own padding is `py-3`, its icon is
/// `text-2xl`, and the fade above the ad is five `rem` of
/// `--brand-gradient-fade-out-color`.
const PROMO_PLATE: f32 = 250.0;
/// `py-3` on the *Upgrade to Modrinth Plus* link.
const PROMO_LINK_PAD_Y: f32 = 12.0;
/// `text-2xl` on the link's `ArrowBigUpDashIcon`.
const PROMO_ICON: f32 = 24.0;
/// The link's own label, which is `font-medium` and inherits the panel's
/// base size.
const PROMO_LABEL: f32 = 16.0;
/// Five `rem` of fade, from `.app-sidebar::after`'s own `height: 5rem`.
const PROMO_FADE: f32 = 80.0;
/// The link's own height: its `text-2xl` icon with the `py-3` either side of it.
const PROMO_LINK_H: f32 = PROMO_ICON + 2.0 * PROMO_LINK_PAD_Y;
/// `pb-12` on the scroll region, which is what keeps the last section clear of
/// the link above the ad.
const PANEL_PROMO_RESERVE: f32 = 48.0;
/// Where the reference's own link goes: `modrinth.plus?app`.
const PROMO_PLUS_URL: &str = "https://modrinth.plus?app";
const PANEL_HEADING: f32 = 16.0;
/// The accounts card's frame: `rounded-xl`, `p-3`, and the `mt-2` that holds it
/// off the heading.
///
/// `p-3` is the empty state's -- it is written on that branch of the card -- and
/// the accordion draws its own padding instead, which is why the frame's padding
/// is an argument rather than a constant of the frame.
const CARD_FRAME_PAD: f32 = 12.0;
const CARD_FRAME_RADIUS: f32 = 12.0;
const CARD_TOP: f32 = 8.0;
/// `gap-3` between what the empty card stacks, and `gap-2` between the parts of
/// the accordion's header and of an account row.
const CARD_STACK_GAP: f32 = 12.0;
const CARD_ROW_GAP: f32 = 8.0;
/// The accordion's header: `px-3 py-2`.
const CARD_HEAD_SIDE: f32 = 12.0;
const CARD_HEAD_PAD: f32 = 8.0;
/// `p-2` on an account row's own button.
const CARD_ROW_PAD: f32 = 8.0;
/// `w-5 h-5` on the radio marks and the header's chevron.
const CARD_MARK: f32 = 20.0;
/// `text-xs` on the card's own "Minecraft account" line.
const CARD_LABEL: f32 = 12.0;
/// `.button-base`'s `filter: brightness(0.85)` under the pointer, which is what
/// the card's two pressable surfaces declare: the accordion's header and an
/// account row.
///
/// Not [`crate::theme::hover_brightness`], which is the global brightening the
/// kit's controls use: `button-base` overrides it, so the card's own controls
/// dim rather than brighten, and they are dimmed by their own key so the two
/// ends are read from the same clock as everything else.
const CARD_PRESS_HOVER: f32 = 0.85;
/// The card's controls, each with its own name for the interaction clock.
const ACCOUNTS_HEADER: &str = "shell:accounts:header";
const ACCOUNTS_SIGN_IN: &str = "shell:accounts:sign-in";
const ACCOUNTS_ADD: &str = "shell:accounts:add";
const ACCOUNTS_NOTE_DISMISS: &str = "shell:accounts:note";
/// The checklist's own header: the accordion the section opens and closes with.
///
/// One key for the header and one per step (`crate::ui::scoped` over the step's
/// name), because each row is a control of its own and a crossing is reported per
/// control.
const CHECKLIST_HEADER: &str = "shell:checklist:header";
/// The sentence the Modrinth step could not do anything about.
const MODRINTH_NOTE_DISMISS: &str = "shell:checklist:modrinth-note";
/// The circle a finished step's check sits in: the reference's `size-[18px]`.
const STEP_MARK_CIRCLE: f32 = 18.0;
/// The check inside that circle: `size-3`.
const STEP_CHECK: f32 = 12.0;
/// A finished step's `opacity-50`: the whole row fades, fill and hairline and
/// label together, which is what the reference's `opacity-50` does to a button.
const STEP_DONE_OPACITY: f32 = 0.5;
/// A step row's own padding: the reference's `h-10 rounded-xl ... px-4`.
const STEP_SIDE: f32 = 16.0;
/// The accordion's padding, which is `p-3` on both its header and its body.
const CHECKLIST_PAD: f32 = 12.0;

// ---- Modals ------------------------------------------------------------

/// The width every dialog in this shell is drawn at, which is the reference's
/// own `max-w-[35rem]` on a modal (`ConfirmModal.vue`).
pub const DIALOG_WIDTH: f32 = 560.0;
/// A dialog's padding on all four sides: `p-6`.
pub const DIALOG_PAD: f32 = 24.0;
/// The room a dialog's own scrollbar takes out of its body.
///
/// **iced draws a `Scrollable`'s bar *over* its content rather than beside it.**
/// `iced_widget-0.12.3/src/scrollable.rs` places the bar at
/// `bounds.right - width.max(scroller_width)`, and `Properties::default()` is 10
/// wide with no margin; this theme sets the bar's colours and nothing else. So
/// the last ten pixels of a body that scrolls are covered -- and the content that
/// pays is exactly the content broken for the body's whole width: a row of chips
/// packed to the padding's edge, a list row's own background, a value aligned to
/// the right.
///
/// Which is what a screenshot of the *Create instance* and *Appearance* dialogs
/// showed: `Ukrainian` clipped in the language grid's last row, the `Game version`
/// value sitting behind the bar, and the selected version's row ending ten pixels
/// early -- all three by the bar's own width, in the two dialogs tall enough to
/// draw one.
///
/// So it is reserved rather than argued about: [`DIALOG_INNER`] is what a body's
/// content is broken for, and [`Shell::dialog_titled`] draws the body that much
/// narrower, which leaves the bar in the dialog's padding where it covers nothing.
/// The cost is the same ten pixels for a body that does not scroll; the
/// alternative is content nobody can read on the ones that do.
pub const DIALOG_SCROLLBAR: f32 = 10.0;
/// What a dialog's body has to fit inside, which is what a row of controls in
/// one is broken for.
///
/// Less than the padding leaves by [`DIALOG_SCROLLBAR`], and deliberately so: the
/// body is drawn at this width, so this is the width a row really has.
pub const DIALOG_INNER: f32 = DIALOG_WIDTH - 2.0 * DIALOG_PAD - DIALOG_SCROLLBAR;
/// The room a dialog leaves above and below itself, so that a dialog as tall as
/// the window is not drawn flush against both of its edges.
const DIALOG_MARGIN: f32 = 16.0;
/// A dialog's head: its title, its close button, and the `gap-3` under them.
///
/// The arithmetic rather than a measurement, which is what it has to be: the
/// height the body is allowed is decided before the head is built. The close
/// button is the taller of the two (`HEAD_BUTTON`), and 40 is it plus the gap.
const DIALOG_HEAD: f32 = 40.0;
/// The shortest a dialog's body may be, whatever the window reports: below this
/// a list is a peephole rather than something to read, and the window's own
/// floor (`MINIMUM_SIZE` in `main.rs`) is 640 logical pixels tall.
const DIALOG_BODY_MIN: f32 = 240.0;
/// The window a shell starts believing it is in, until iced reports the real one.
///
/// The entry point's own preferred size -- `PREFERRED_SIZE` in `main.rs`, and a
/// `--shot` run's default -- so that a modal opened on the first frame is sized
/// by a number that is at worst the screen's, and [`window_size`] corrects it
/// long before anyone opens one.
const DIALOG_VIEWPORT: iced::Size = iced::Size::new(1280.0, 720.0);

/// The settings dialog's width: `AppSettingsModal.vue`'s
/// `min(928px, calc(95vw - 10rem))`.
///
/// The `10rem` is the reference's own reservation for the space either side of a
/// dialog at a narrow window -- 160 pixels -- and it is why a settings dialog on a
/// small window is narrower than 928 rather than overflowing it.
const SETTINGS_WIDTH: f32 = 928.0;
/// The width the reference's `TabbedModal` gives its tab column: the first track of
/// `grid-cols-[minmax(12.5rem,18rem)_minmax(0,1fr)]`, at its own cap of `18rem`.
///
/// Measured against the reference's own settings dialog open at 1280x720, the column
/// and its 1-pixel divider put the divider's column at x=488 of a dialog whose left
/// edge is 177: 177 + 24 (the body's `p-6`) + 288 - 1 = 488.
const SETTINGS_NAV: f32 = 288.0;
/// The tab column's own padding: `TabbedModal.vue`'s `pr-4`, inside the track and
/// to the left of its 1-pixel divider.
const SETTINGS_NAV_PAD: f32 = 16.0;
/// `rounded-2xl` on the modal's own box.
const SETTINGS_RADIUS: f32 = 16.0;
/// The 1-pixel `--surface-5` hairline around it, drawn as a padding rather than as
/// an iced border so that it sits outside the content the way CSS puts it.
const SETTINGS_BORDER: f32 = 1.0;
/// The settings body's own horizontal padding: `TabbedModal.vue`'s `p-6`, whose
/// `pr-0` cancels the right side, and then the content pane's own `px-6` inside the
/// second track. Both sides come out at 24 against the capture.
const SETTINGS_BODY_PAD: f32 = 24.0;

// ---- The appearance settings' theme cards -------------------------------

/// The height of a theme-preview card, a measurement of the reference's own: a
/// 1-pixel border, 114 of preview (`1.5rem` of padding around a 66-pixel mock),
/// 42 of label (`0.75rem 1rem` around a 16-pixel line) and the border again.
const THEME_CARD_HEIGHT: f32 = 158.0;
/// The preview card's `--radius-md`.
const THEME_CARD_RADIUS: f32 = 12.0;
/// The preview band's height: `1.5rem` of padding above and below the mock's 66.
const THEME_PREVIEW_HEIGHT: f32 = 114.0;
/// The preview's own padding, `1.5rem`.
const THEME_PREVIEW_PAD: f32 = 24.0;
/// The mock card's height: a 1-pixel border, `1rem` of padding, the `2rem` icon
/// (which is the two grid rows and the gap between them), the padding and the
/// border again.
const THEME_MOCK_HEIGHT: f32 = 66.0;
/// `button-base`'s hover: `filter: brightness(0.85)`, which is where the card's
/// tween ends rather than at the kit's default brightening.
const THEME_CARD_HOVER: f32 = 0.85;
/// The mock card's padding, `1rem`, inside a 1-pixel `--surface-4` border.
const THEME_MOCK_PAD: f32 = 16.0;
/// The mock's icon, `2rem` square.
const THEME_MOCK_ICON: f32 = 32.0;
/// `rounded-lg` on the mock, its icon and its text bars.
const THEME_MOCK_RADIUS: f32 = 8.0;
/// The mock's text bars, `h-2`.
const THEME_BAR_HEIGHT: f32 = 8.0;
/// The second bar's width: `w-[60%]` of the column it is in.
const THEME_BAR2_SHARE: f32 = 0.6;
/// The mock's grid gap, `0.5rem`, which is also the label's `margin-right` on the
/// radio and the gap around a tab row's icon.
const THEME_MOCK_GAP: f32 = 8.0;
/// The second bar's top, inside the mock: the icon's 32 pixels are two 12-pixel
/// grid rows with the 8-pixel gap between them, so the bars start 12 + 8 = 20
/// below the first.
const THEME_BAR2_TOP: f32 = 20.0;
/// The label row's height: `padding: 0.75rem 1rem` around the 16-pixel line the
/// theme's name is set on -- `--gap-md` and `--gap-lg`.
const THEME_LABEL_HEIGHT: f32 = 42.0;
/// `--gap-lg`: the theme grid's gap, the label's horizontal padding and the card's
/// own `mt-4`/`pr-4`-sized spacing.
const THEME_GAP: f32 = 16.0;
/// A radio button or a `theme-icon` in a card's label, `1rem`.
const THEME_LABEL_ICON: f32 = 16.0;
/// `margin-left: 0.25rem` on a card's `theme-icon`.
const THEME_ICON_GAP: f32 = 4.0;
/// The reference's `minmax(12rem, 1fr)` on the theme grid: the narrowest a card
/// may be before the grid drops a column.
const THEME_CARD_MIN: f32 = 192.0;
/// `mt-6`, between the theme grid and the sync row under it.
const THEME_SECTION_GAP: f32 = 24.0;
/// `mt-4`, between a section's heading block and its control.
const THEME_HEADING_GAP: f32 = 16.0;
/// `gap-1`, between a section's heading and its own description.
const THEME_HEADING_LEAD: f32 = 4.0;
/// The title of a section: `text-xl`.
const THEME_TITLE_SIZE: f32 = 20.0;
/// A section's description and the theme cards' names: `text-base`.
const THEME_BODY_SIZE: f32 = 16.0;
/// The sync row's own title: `text-lg`.
const THEME_ROW_TITLE_SIZE: f32 = 18.0;
/// The line a `text-xl` or `text-lg` heading is set on, which Tailwind gives both:
/// `1.75rem`.
///
/// Every line height here is an [`iced::Pixels`] rather than a bare `f32`, because
/// iced's `From<f32>` for `LineHeight` is a *multiple* of the text's size: a bare
/// `28.0` on a 20-pixel heading is a 560-pixel line.
const THEME_TITLE_LINE: f32 = 28.0;
/// The line an unclassed paragraph is set on: the body's own 16 pixels at the
/// browser's root `line-height: 1.15` -- `modern-normalize`'s rule, which is what
/// the reference's stylesheets inherit. That is 18.4 pixels of CSS, and this is 18,
/// because 18 is what the reference *paints*.
///
/// **Not `text-base`'s `1.5rem`.** A `text-*` class carries a line with it, and
/// the descriptions here have no size class: `text-secondary` is a colour. The
/// capture is what says so: the heading block is `text-xl`'s 28 and one `gap-1`, so
/// a line of this order puts the theme grid's top border on y=241, which is where
/// the reference draws it; `text-base`'s 24 would put it on 247. And the fraction:
/// a browser lays the 18.4 out and then pixel-snaps each box's edges, so every row
/// it paints is a whole one -- the grid's border lands on 241, not 241.4 -- while
/// this rasteriser draws the fractional position and softens the border across two
/// rows. The painted value is the one to draw.
const THEME_BODY_LINE: f32 = 18.0;
/// The line a theme card's own name is set on: the inherited box a tab row's label
/// takes ([`THEME_BODY_LINE`]).
const THEME_LABEL_LINE: f32 = 18.0;
/// The line the modal's title is set on: `text-2xl`'s own `2rem`.
const THEME_MODAL_TITLE_LINE: f32 = 32.0;
/// `h-16` on the fade the pane shows over content it cannot show at once.
const SETTINGS_FADE: f32 = 64.0;

/// The `md` `IconButton`'s corner radius, `rounded-xl`.
const CONTROL_RADIUS: f32 = 12.0;
/// `rounded-lg` on the head's `!h-7` buttons.
const HEAD_RADIUS: f32 = 8.0;

// ---- Timing ------------------------------------------------------------

/// One frame of the shell's clock.
///
/// 60Hz, the cadence a browser drives CSS transitions at. The clock only runs
/// while something is moving -- see [`Shell::animating`] -- so an idle window
/// asks for no frames at all.
const FRAME: Duration = Duration::from_millis(16);

/// Subscription id of the frame clock. A `&str`, because iced keys
/// subscriptions by any `Hash` and a stable key is what keeps the subscription
/// from being torn down and rebuilt every frame.
const FRAME_ID: &str = "palantirmc-shell-frames";

/// One running instance: what its worker needs, and the slot its process lands in.
///
/// The slot is per run rather than per shell, and that is the whole of what a
/// second concurrent launch needed: the worker puts the child it starts into the
/// slot it was handed, so one shared slot would leave the first run's process
/// unreachable the moment a second run started -- and with it the Stop button of
/// a game still on screen.
struct Run {
    /// What the subscription streams under and what the worker needs to start.
    data: ActiveRunData,
    /// The game process this run's own worker put in its slot.
    child: ChildSlot,
}

// ---- The shell ----------------------------------------------------------

/// What the shell is showing and what is moving.
pub struct Shell {
    /// Where the user is, with the context it was opened in.
    address: Address,
    /// The pages behind this one, newest last.
    back: Vec<Address>,
    /// The pages ahead of this one, newest last, emptied by any new navigation.
    forward: Vec<Address>,
    /// The generated theme, resolved once at startup rather than on every paint.
    theme: Gen,
    /// Whether the right panel is showing. The reference's `sidebarToggled`,
    /// which is the opposite sense from its `toggle_sidebar` *setting*: the
    /// setting says whether the user wants the panel at all, this says whether
    /// it is up right now.
    sidebar: bool,
    /// Whether the rail offers its Skins slot, from the reference's
    /// `show_skin_selector_in_sidebar` setting.
    skins_slot: bool,
    /// Whether the rail offers its Screenshots slot, from the reference's
    /// `show_all_screenshots_in_sidebar` setting.
    screenshots_slot: bool,
    /// Which rail slot the pointer is over.
    hovered: Option<Rail>,
    /// One selection tween per rail slot, in [`Rail::ALL`] order.
    plates: Vec<Tween>,
    /// Which modal is open, if any.
    modal: Option<Modal>,
    /// Which tab the settings dialog is showing.
    ///
    /// Shell state rather than the dialog's own, because the dialog is rebuilt from
    /// the shell every frame and a tab that lived in an element would be forgotten
    /// between them.
    settings_tab: SettingsTab,
    /// Whether the window is maximized, which the window controls' icon needs.
    maximized: bool,
    /// The window's own size in logical pixels, from [`window_size`]. What the
    /// modal layer caps a dialog's body with: a dialog is drawn centred, so one
    /// taller than the window loses its head at the top and its last row at the
    /// bottom with nothing to scroll them with.
    viewport: iced::Size,
    /// One glide per scroll region, keyed by the name the region was built with.
    ///
    /// What a wheel moves, and what keeps the frame clock awake while it does:
    /// see [`crate::scroll::Glides`]. The regions themselves are the pages'
    /// (`crate::page::body`), this panel's, a dialog's and the instance-settings
    /// modal's -- all of them named out of [`crate::scroll`] rather than here, so
    /// that the one place a name is written is the one place the region is built.
    glides: crate::scroll::Glides,
    /// Where a `--shot` run's picture is going, if this is one. The request is in
    /// place before the first frame, the timer [`Shell::capture`] asks for fires
    /// once the window has settled, and writing the frame is what ends the run.
    shot: Option<std::path::PathBuf>,
    /// Whether the frame has already been asked for, so a timer that fires twice
    /// cannot write the same file twice.
    shot_taken: bool,
    /// How many of the extra frames a window owes its first one have arrived.
    /// See [`REPAINT_KICKS`].
    kicks: u8,
    /// The page in the pane, with its own state.
    screen: Screen,
    /// What the pages are answered with: the launcher's own filesystem, and the
    /// engine for what has to come from a service.
    store: Store,
    /// The preferences the shell's own settings are drawn from.
    ///
    /// The shell owns the colour theme rather than being handed one, because
    /// Settings is where it changes: a choice made in the modal has to redraw the
    /// window it was made in, and be in the file for the next launch.
    prefs: crate::prefs::Prefs,
    /// Where that file is, once the application has said. `None` in a test, which
    /// is what keeps a test from writing to the real preferences.
    home: Option<palantir_core::paths::PalantirPaths>,
    /// The name in the creation dialog.
    create_name: String,
    /// What the last create could not do, shown in the dialog it was asked from.
    create_error: Option<String>,
    /// A create is in flight, which is what makes the dialog's button unusable
    /// rather than counted twice.
    creating: bool,
    /// The dialog's button was pressed and the request has not left yet.
    ///
    /// A flag rather than a return value because [`Shell::act`]'s only answer is
    /// [`discover::Asked`]: a create is not a page's request, and widening that
    /// return type would make every arm of its match say so.
    create_requested: bool,
    /// What the last import scan found, read once when the dialog opens rather
    /// than on every frame it is drawn: the scan walks other launchers' roots.
    import_found: Vec<crate::instances::ImportCandidate>,
    /// What the last import could not do, shown in the dialog it was asked from.
    import_error: Option<String>,
    /// An import is in flight.
    importing: bool,
    /// The row whose button was pressed, taken by `handle` to build the command.
    import_requested: Option<std::path::PathBuf>,
    /// Mojang's version list, as the creation dialog asked for it: read once per
    /// opening rather than per frame, for [`Shell::import_found`]'s reason -- the
    /// list is every version ever published, and it is a request.
    versions: Load<store::VersionList>,
    /// What the dialog's version search field holds.
    version_query: String,
    /// Whether the picker is showing everything Mojang publishes rather than the
    /// releases: the reference's `showSnapshots`, off until its footer is used.
    version_snapshots: bool,
    /// The version picked in the dialog. `None` is "whatever Mojang says is
    /// current", which is what the picker opens on.
    version_choice: Option<String>,
    /// The dialog was opened and the version request has not left yet. Taken by
    /// `handle` for [`Shell::create_requested`]'s reason: the dialog's own
    /// requests are not a page's, so they do not travel as an `Asked`.
    versions_requested: bool,
    /// Which modloader the dialog's chips are on.
    ///
    /// Fabric rather than Vanilla, because that is what the reference's flow
    /// opens on: vanilla is offered first -- the list is the same five chips in
    /// both -- and the choice the flow *makes* for a user who has not chosen is
    /// the loader most of the library is published for.
    create_loader: crate::catalog::LoaderKind,
    /// Which of the loader-version chips the dialog is on.
    build_choice: BuildChoice,
    /// The build the user picked when the chips are on [`BuildChoice::Other`].
    ///
    /// A version string rather than an index, because the list it came from is a
    /// service's answer that can be read again: an index would name a different
    /// build after a reload, and the row the user pressed is the version itself.
    build: Option<String>,
    /// What the dialog's build search field holds.
    build_query: String,
    /// The loader's own builds for the game version in force, as the dialog asked
    /// for them: read once per loader and game version rather than per frame, for
    /// [`Shell::versions`]'s reason.
    loader_builds: Load<Vec<store::LoaderBuild>>,
    /// The loader and game version the list above was asked for.
    ///
    /// Written down so a slow answer about a choice the user has moved off can be
    /// dropped rather than drawn: without it, a Fabric list that arrived after
    /// the Quilt chip was pressed would be drawn under the Quilt chip, which is a
    /// dialog that lies about what it will install.
    loader_builds_for: Option<(crate::catalog::LoaderKind, String)>,
    /// A chip was pressed and the loader-build request has not left yet. Taken by
    /// `handle` for [`Shell::versions_requested`]'s reason.
    loader_builds_requested: bool,
    /// The instance-settings modal's installation tab has asked for Mojang's
    /// version list. A second flag rather than the dialog's own, because the two
    /// reads feed two states and a shared reply would have the second answer
    /// overwrite the first.
    installation_versions_requested: bool,
    /// The same for one platform's builds at one game version: the pair *is* the
    /// question, so it travels here rather than being read off the form again.
    installation_builds_requested: Option<(crate::catalog::LoaderKind, String)>,
    /// The project whose name the installation tab's modpack card is waiting for.
    ///
    /// An id rather than a flag, for [`Shell::installation_builds_requested`]'s
    /// reason: the link can be taken away while the request is out, and an answer
    /// about a project that is no longer linked must land nowhere.
    installation_modpack_requested: Option<String>,
    /// The instance whose files the installation tab asked to have installed
    /// again.
    ///
    /// An id rather than a flag, for the modpack request's reason: the repair is a
    /// minute of hashing and fetching, the modal can be closed or opened on another
    /// instance while it runs, and a sentence about an instance the reader has left
    /// must land nowhere -- while the repair itself happened either way, which is
    /// why nothing here cancels it.
    installation_repair_requested: Option<String>,
    /// The instance whose pack versions the installation tab's *Change version*
    /// list is waiting for.
    ///
    /// An id rather than a flag, for the modpack request's reason: the link can be
    /// taken away, or moved to another version, while the request is out, and a
    /// list about an instance the reader has left must land nowhere.
    installation_pack_versions_requested: Option<String>,
    /// The instance whose linked pack the reader asked to have laid over it again.
    ///
    /// An id rather than a flag, for the repair request's reason: the modal can be
    /// closed while the pack is being fetched, and the pack lands either way.
    installation_reinstall_requested: Option<String>,
    /// The instance and the version of a *Change version* press: which instance the
    /// archive is laid over, and which version's archive it is.
    ///
    /// A pair rather than one id, because the question is about both -- and because
    /// the link the modal holds may already have moved by the time the answer
    /// lands.
    installation_change_requested: Option<(String, String)>,
    /// The launches in flight, one per instance, oldest first.
    ///
    /// A list rather than the single run this shell used to hold: the reference
    /// runs several processes at once, its bar's popover lists them and can stop
    /// any one of them, and none of that can be built on a shell that refuses a
    /// second Play. Each run owns the slot its process lands in, so starting one
    /// cannot orphan another.
    runs: Vec<Run>,
    /// How many runs this shell has ever asked for.
    ///
    /// It is what tells one run's messages from another's: every
    /// [`launch::LaunchEvent`] arrives with the id of the run that sent it, and a
    /// done for a run that has already ended must not touch the ones still going.
    next_run_id: u64,
    /// The launcher's accounts, read when a launch needs one and written when a
    /// launch renews a session. `None` in a test, which is what keeps a test from
    /// reading -- or writing -- a real `accounts.json`.
    accounts: Option<AccountsStore>,
    /// What reading the accounts file could not do, shown in Settings.
    ///
    /// [`crate::accounts::AccountsStore::load_with_report`] answers a corrupt file
    /// with an empty store and a warning, and the warning is deliberately not
    /// dropped: a launcher that silently forgot which account was signed in is the
    /// failure that reporting it exists to prevent.
    accounts_warning: Option<String>,
    /// What the language list's search field holds, which is the only thing about
    /// that pane this shell remembers between repaints.
    ///
    /// The reference's own field is `Input` with a search icon over the list, and
    /// the list it filters is the pane's whole body; a string is all a field is.
    locale_query: String,
    /// Whether the accounts card's body is open. The reference's accordion is
    /// `open-by-default: false`, so the panel shows a header until it is pressed.
    accounts_open: bool,
    /// What one of the card's own controls could not do, drawn in the panel under
    /// it.
    ///
    /// The card's sign-in and add-account buttons open the Microsoft flow, which
    /// is a later stage's; the sentence is kept here rather than dropped because a
    /// control that does nothing at all is the failure mode this rewrite reports
    /// instead of hiding.
    accounts_note: Option<String>,
    /// Whether the checklist's body is open. The reference's accordion is
    /// `open-by-default`, so the panel shows all three steps until it is pressed.
    checklist_open: bool,
    /// What the checklist's *Sign in to Modrinth* step could not do, drawn under
    /// the section it was pressed in.
    ///
    /// Its own sentence rather than the accounts note's, because Modrinth's
    /// sign-in is not the Microsoft flow that note is about -- and its own field
    /// rather than a flag on that one, so that a reader who reads one does not
    /// clear the other.
    modrinth_note: Option<String>,
    /// Where each run that is downloading is: the last
    /// [`launch::LaunchEvent::Progress`] its worker reported, kept as *numbers*
    /// rather than only as the sentence the instance's header draws.
    ///
    /// A bar needs a fraction and a state needs a word, and the same fact cannot
    /// be both if only the formatted line survives. It lives on the shell rather
    /// than on a page because a run outlives the page it was started from: this is
    /// what the action bar's download chip is drawn from, and it is what lets the
    /// launcher be watched from *any* page -- which the per-page line could not
    /// do, and is the whole reason the reference keeps this surface in its status
    /// bar.
    ///
    /// Keyed by instance because two runs can be fetching at once: the chip is
    /// drawn from the selected run's phase, and a run that starts fetching closes
    /// nothing that another run's phase has open.
    jobs: BTreeMap<String, install::Progress>,
    /// Whether the download manager's panel is open: the reference's own toggle,
    /// off until its chip is pressed.
    downloads: bool,
    /// Whether the popover over every running instance is open.
    ///
    /// A toggle of its own rather than a flag shared with the panel: the
    /// reference draws the two from two different controls (`DownloadManager` and
    /// the chevron beside the running instance's name), and either can be open
    /// with the other closed.
    switchers: bool,
    /// Whether an install is in flight, so that a second press cannot start a
    /// second transfer of the same file.
    ///
    /// The flag is set as the request leaves rather than when it comes back, for
    /// the reason the creation flow's is: the gap between the two is a window a
    /// user can press in, and two transfers writing one file through one part
    /// file is a corrupted mod.
    installing: bool,
    /// The install the dialog's last press asked for: which project, into which
    /// instance. Taken by the command that runs it, so it is `None` while there is
    /// nothing to do.
    install_requested: Option<(String, String)>,
    /// A pack's install, which has no instance to name: the project that becomes
    /// one. Taken by its command, for [`Shell::install_requested`]'s reason.
    pack_requested: Option<String>,
    /// The panel's news section: the newest articles, or why there are none.
    ///
    /// Asked for once, at startup, because the panel is drawn on every route --
    /// a section that waited for a page to ask for it would blank out whenever
    /// the reader moved. `Loading` until the answer arrives, and nothing is drawn
    /// for either that or a failure: the reference's own `v-if="news.length"`
    /// draws no section for an empty feed, and an unreachable feed is not an
    /// error a reader can act on.
    news: Load<Vec<NewsArticle>>,
    /// Why the last link did not open, drawn under the news section it was
    /// pressed in.
    ///
    /// A link that does not open is the one thing here a reader can report, so it
    /// is said rather than dropped -- and the opener refuses anything that is not
    /// `http`/`https` before a program is ever built, which is the failure this
    /// note is most likely to carry (`crate::open`).
    link_note: Option<String>,
    /// Why the last install did not work, drawn inside the dialog.
    ///
    /// The dialog's own sentence rather than the page's notice, because the
    /// failure is about the choice the dialog is asking for -- no version for that
    /// instance, no permission in its folder -- and the answer is to pick another
    /// instance, which a reader can only do while the dialog is still up.
    install_error: Option<String>,
    /// What the last install came to, waiting to be handed to the page.
    ///
    /// Raised by [`Shell::act`] and taken by [`Shell::handle`], like the create
    /// flow's flag and for the same reason: a page is told in a message, and only
    /// the caller that builds commands can send one.
    installed: Option<Result<String, String>>,
}

/// Which of the reference's rail conditions are on.
///
/// Two of the rail's eight slots are conditional in `App.vue`, and both
/// conditions are settings the user owns: the Skins slot appears when
/// `show_skin_selector_in_sidebar` is on, and the Screenshots slot when the
/// screenshots sync option is. The rail is therefore built from the settings
/// rather than from a fixed list, which is also why [`crate::route::Rail`] has
/// an entry for a slot that may not be drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailSettings {
    /// The reference's `toggle_sidebar`, in its own negative sense: the setting
    /// is named for hiding the panel.
    pub hide_sidebar: bool,
    /// `show_skin_selector_in_sidebar`.
    pub show_skins: bool,
    /// `show_all_screenshots_in_sidebar`.
    pub show_screenshots: bool,
}

impl Default for RailSettings {
    /// The reference's own defaults: `toggleSidebar` off, the skin selector on
    /// -- a fresh install of the reference draws the shirt in its rail, and
    /// `use-app-settings.ts` ships `showSkinSelectorInSidebar: true` -- and
    /// screenshots on as well.
    fn default() -> RailSettings {
        RailSettings { hide_sidebar: false, show_skins: true, show_screenshots: true }
    }
}

/// The creation dialog's own button, which is one control on one dialog.
const CREATE_BUTTON: &str = "shell:create";

/// The action bar's four controls: the download chip (which is also the panel's
/// own toggle), the two the running chip carries, and the panel's close button.
const DOWNLOAD_CHIP: &str = "shell:download-chip";
const BAR_STOP: &str = "shell:bar-stop";
const BAR_LOGS: &str = "shell:bar-logs";
const BAR_PANEL_CLOSE: &str = "shell:bar-panel-close";

/// The running chip's chevron, which opens the popover over every running
/// instance.
const BAR_SWITCHERS: &str = "shell:bar-switchers";

/// The reference's `py-1.5 px-3 rounded-xl` on both chips: a 20px icon and 6px
/// of padding on each side, which is 32.
const BAR_CHIP: f32 = 32.0;
/// How much of the download chip is the bar rather than the words. The reference
/// gives its own bar the whole trigger's width and the label to a marquee; a
/// fixed 64px is this toolkit's version of the same arrangement, and it is what
/// keeps the chip from resizing on every percentage.
const DOWNLOAD_BAR: f32 = 64.0;
/// The same arrangement inside the panel, where the bar shares its line with a
/// name and a phase rather than sitting under them.
const JOB_BAR: f32 = 96.0;
/// The reference's `w-[20rem]` on the popover over the running instances.
const SWITCHER_PANEL: f32 = 320.0;
/// `OnlineIndicatorIcon`'s dot.
const INDICATOR: f32 = 8.0;

/// The version picker's two footer states, each its own control: the same row
/// says one thing when the snapshots are hidden and the other when they are not.
const VERSION_SHOW_ALL: &str = "shell:version-show-all";
const VERSION_HIDE_SNAPSHOTS: &str = "shell:version-hide-snapshots";

/// Which loader build the creation dialog should install.
///
/// The reference's own three chips on its *Loader version* row, and the rule
/// each one names. `Stable` is the default arm in both shells -- it is what a
/// user almost always wants, and it is the one build per loader that has been
/// published as tested -- and `Other` is the only one that carries a version of
/// its own rather than deriving one from the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildChoice {
    /// The newest build the loader itself calls stable, or the newest of all
    /// when it has published nothing stable for this game version.
    #[default]
    Stable,
    /// The newest build there is, stable or not.
    Latest,
    /// A build the user picks out of the list.
    Other,
}

impl BuildChoice {
    /// The three chips, in the reference's own order.
    pub const ALL: [BuildChoice; 3] = [BuildChoice::Stable, BuildChoice::Latest, BuildChoice::Other];

    /// The chip's label, which is the reference's own message for it.
    pub fn label(self) -> Key {
        match self {
            BuildChoice::Stable => Key::CreationFlowModalCustomSetupLoaderVersionTypeStable,
            BuildChoice::Latest => Key::CreationFlowModalCustomSetupLoaderVersionTypeLatest,
            BuildChoice::Other => Key::CreationFlowModalCustomSetupLoaderVersionTypeOther,
        }
    }

    /// The chip's stable identity for the interaction clock: one per chip, not
    /// one per label, because two chips that happen to share a word must not
    /// light together.
    fn key(self) -> &'static str {
        match self {
            BuildChoice::Stable => "shell:create-build:stable",
            BuildChoice::Latest => "shell:create-build:latest",
            BuildChoice::Other => "shell:create-build:other",
        }
    }
}

/// The word for the state a run is in, drawn beside its name in the action bar.
///
/// The reference's bar carries no such word -- a process it can see is a process
/// that is running -- and this launcher has two states it cannot see from the
/// outside: preparing, and being stopped. Both are states in which the controls
/// beside the word are missing or about to be, which is exactly when a word is
/// worth drawing. *Running* is the arm with nothing to say, and it says nothing.
fn state_label(state: store::LaunchState) -> Option<&'static str> {
    match state {
        store::LaunchState::Idle | store::LaunchState::Running => None,
        store::LaunchState::Starting => Some(Key::InstanceActionStarting.message()),
        store::LaunchState::Stopping => Some(Key::InstanceActionStopping.message()),
    }
}

/// The name of an instance, for a surface that has only its id.
///
/// The library the pages are drawn from is where a name lives, and a run records
/// an id: an instance the list has not read yet -- or one that was removed while
/// it ran -- falls back to the id itself, which is what the reference's own bar
/// shows for an instance it cannot load.
fn instance_name(store: &Store, id: &str) -> String {
    store
        .instance(id)
        .ready()
        .map(|card| card.name.clone())
        .unwrap_or_else(|| id.to_string())
}

/// The one facet group a list of installed projects becomes.
///
/// `search.ts`'s `negativeByType` arm: every excluded project of a field goes
/// into *one* group, `project_id NOT IN [...]`, because a second group would be
/// a second thing the project has to match -- and a project that is not any of
/// these ids is not a second thing, it is the other half of an "or".
///
/// `None` for an empty list, because `facets=[[]]` asks Modrinth for a project
/// that satisfies nothing and answers with no results at all: a launcher with no
/// instances has not hidden anything, and a search that hides everything is not
/// the same honest answer as a search that was never narrowed.
fn exclusion_facet(ids: &[String]) -> Option<String> {
    if ids.is_empty() {
        return None;
    }
    let list = ids.iter().map(|id| format!("\"{id}\"")).collect::<Vec<String>>().join(",");
    Some(format!("project_id NOT IN [{list}]"))
}

/// The job the download chip and its panel are about.
///
/// The selected run's phase when it has one, and otherwise the first run that is
/// fetching. The reference draws its chip from its download store's total rather
/// than from the selected process, and the same reason holds here: a chip that
/// went blank because the run it names stopped downloading while another was
/// still going would hide work that is happening. The key is an instance id, so
/// "first" is the same run on every frame.
fn shown_job<'a>(
    jobs: &'a BTreeMap<String, install::Progress>,
    selected: Option<&str>,
) -> Option<(&'a str, &'a install::Progress)> {
    if let Some(id) = selected {
        if let Some((key, progress)) = jobs.get_key_value(id) {
            return Some((key.as_str(), progress));
        }
    }
    jobs.iter().next().map(|(id, progress)| (id.as_str(), progress))
}

/// The download panel's own sentence: what is being fetched, how many of them are
/// done, and how many bytes it has cost.
///
/// The rate the reference shows beside these is deliberately absent: a rate needs
/// a clock and a window, and a number computed from one frame's difference is a
/// number that jumps. Bytes-to-date is the same fact without the lie.
fn progress_line(progress: &install::Progress) -> String {
    if progress.is_indeterminate() {
        return format!("{}…", progress.label);
    }
    let megabytes = progress.bytes as f64 / (1024.0 * 1024.0);
    format!(
        "{}: {} of {} · {megabytes:.1} MB",
        progress.label, progress.done, progress.total
    )
}

/// The modloader chips' identities, one per chip.
///
/// Written out rather than derived from the label, for [`BuildChoice::key`]'s
/// reason -- and because five names a chip can be is five names worth being able
/// to read in one place.
fn loader_chip_key(loader: crate::catalog::LoaderKind) -> &'static str {
    use crate::catalog::LoaderKind;
    match loader {
        LoaderKind::Vanilla => "shell:create-loader:vanilla",
        LoaderKind::Fabric => "shell:create-loader:fabric",
        LoaderKind::NeoForge => "shell:create-loader:neoforge",
        LoaderKind::Forge => "shell:create-loader:forge",
        LoaderKind::Quilt => "shell:create-loader:quilt",
    }
}

/// `Combobox.vue`'s `DEFAULT_MAX_HEIGHT`: its options stop growing at 300px and
/// scroll past that. The footers of the dropdown are not part of it -- they are
/// the dropdown's own last rows -- so this bounds the list and nothing else.
const VERSION_LIST_HEIGHT: f32 = 300.0;
/// The reference's option row: `px-4 py-3`.
const VERSION_ROW_SIDE: f32 = 16.0;
const VERSION_ROW_HEIGHT: f32 = 12.0;
/// `getOptionClasses`' `hover:brightness-[115%]` on an option that is not the
/// one in force. Not [`crate::theme::hover_brightness`]: the picker declares its
/// own hover end, which is what [`crate::ui::Hovered::hover_with`] is for.
const VERSION_ROW_HOVER: f32 = 1.15;

/// The modal's controls are the kit's, and the kit asks for a crossing.
///
/// Written by hand rather than through `crate::hovered!` because this message
/// family already has a `Hover` of its own -- the rail's, whose payload is a slot
/// rather than a key -- and a second variant of the same name would not compile.
impl crate::ui::Hovered for Message {
    fn hover(key: &'static str, over: bool) -> Message {
        Message::Control { key, over, hover: None }
    }

    fn hover_with(key: &'static str, over: bool, hover: f32) -> Message {
        Message::Control { key, over, hover: Some(hover) }
    }
}

/// The modals the shell can put up.
///
/// Settings is the one stage 2 owes, and it is a *modal* rather than a route on
/// purpose: the reference has no `/settings` route, its modal is opened from the
/// rail's settings button, and the old shell's `/settings` page is one of the
/// pages [`crate::route`] refuses. The rest arrive with the pages that open them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Modal {
    /// `AppSettingsModal`.
    Settings,
    /// The creation flow (`CreationFlowModal`): a name, and the button that
    /// makes the instance.
    Create,
    /// The same flow's import step: what the other launchers on this machine are
    /// holding, and a button per instance.
    Import,
    /// Where a project goes: into an instance the reader picks, or -- for a pack --
    /// into an instance the install makes. The reference's *Install to instance*,
    /// drawn as a dialog (see [`Shell::install_dialog`]).
    Install {
        /// Which project, as the API names it.
        project: String,
        /// What to call it, which the page that owns it sent along: a dialog that
        /// asked the engine again for a title it could have been handed would be a
        /// request per press for a word the page is already holding.
        title: String,
        /// Whether the project is a pack, which is the one kind with no folder to
        /// be offered: it becomes an instance of its own instead.
        pack: bool,
    },
    /// One instance's own settings, read back for the form the instance page's
    /// gear opens (see [`crate::instance_settings`]).
    InstanceSettings(Box<crate::instance_settings::State>),
}

/// The settings dialog's tabs, in the reference's own order.
///
/// `AppSettingsModal.vue`'s tab list has eleven entries in three categories:
/// `display` carries Appearance, Features, Behavior and Language (Feature flags too,
/// on a developer build), `account` carries Profile, Social and Privacy, and
/// `instances` carries Synced settings, Java installations and Resource management. The two listed here are the ones whose content this launcher has
/// ported; a tab joins [`SettingsTab::ALL`] when its own component is ported, and the
/// rest are named in `NOTES.md` as the work still owed rather than drawn as empty
/// shells. The order is the reference's and is never sorted: it is the order a
/// reader's eye travels down the column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    /// `AppearanceSettings.vue`.
    Appearance,
    /// `LanguageSettings.vue`.
    Language,
}

impl SettingsTab {
    /// Every tab this dialog offers, in the reference's order.
    pub const ALL: [SettingsTab; 2] = [SettingsTab::Appearance, SettingsTab::Language];

    /// The tab's own label, which is the reference's `app.settings.tabs.<name>`: the
    /// same message the reference's `defineMessage` gives the tab's `name`.
    fn label(self) -> Key {
        match self {
            SettingsTab::Appearance => Key::AppSettingsTabsAppearance,
            SettingsTab::Language => Key::AppSettingsTabsLanguage,
        }
    }

    /// The icon in front of the label: [`Glyph::Paintbrush`] and [`Glyph::Languages`],
    /// which are `AppSettingsModal.vue`'s own `PaintbrushIcon` and `LanguagesIcon`.
    fn glyph(self) -> Glyph {
        match self {
            SettingsTab::Appearance => Glyph::Paintbrush,
            SettingsTab::Language => Glyph::Languages,
        }
    }

    /// The badge the reference puts on the tab, which is `commonMessages.beta` on
    /// Language and nothing on the others.
    fn badge(self) -> Option<Key> {
        match self {
            SettingsTab::Appearance => None,
            SettingsTab::Language => Some(Key::BadgeBeta),
        }
    }

    /// The heading the tab sits under: `tabCategories.display`, "Display".
    fn category(self) -> Key {
        match self {
            SettingsTab::Appearance | SettingsTab::Language => Key::SettingsSidebarLabelDisplay,
        }
    }

    /// The tab's control name for the interaction clock: one per row, so that two
    /// rows cannot light together.
    fn key(self) -> &'static str {
        match self {
            SettingsTab::Appearance => "settings:tab:appearance",
            SettingsTab::Language => "settings:tab:language",
        }
    }
}

/// What the instance-settings modal's installation tab still wants read.
///
/// A private vocabulary rather than a shell field per question: [`Shell::act`]
/// decides which of these the form is owed and [`Shell::update`] turns them into
/// commands, which is the split the creation dialog's own two requests already
/// use.
enum InstallationNeed {
    /// Mojang's version list: the tab's game-version picker is empty without it.
    Versions,
    /// One platform's builds at one game version, as a pair because a pair is
    /// what the answer is about.
    Builds(crate::catalog::LoaderKind, String),
    /// The project an instance was installed from, whose title the card draws.
    /// The project's id rather than the instance's, because that is what the
    /// service is asked about.
    Modpack(String),
    /// The versions of one instance's linked pack that fit it, for the same tab's
    /// *Change version* list. The instance's id rather than the project's, because
    /// the filter is the instance's own game version and loader.
    PackVersions(String),
}

/// Everything the shell can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// Navigate, from a path.
    Go(String),
    /// A page was told something.
    Screen(pages::Message),
    /// The history, as the head's two buttons drive it.
    Back,
    Forward,
    /// The pointer entered or left a rail slot.
    Hover(Option<Rail>),
    /// A rail slot was clicked.
    Rail(Rail),
    /// A colour theme was chosen in Settings.
    ColorTheme(ColorTheme),
    /// One of the settings dialog's tabs was chosen.
    SettingsTab(SettingsTab),
    /// A language was chosen in Settings, by its BCP-47 tag.
    ///
    /// The tag rather than a type: the offer is the reference's own list of codes
    /// ([`crate::locale::OFFERED`]) and the tables are keyed by them, so a tag is
    /// what both sides already name a language with. An unknown one resolves to
    /// English in [`crate::locale::set`] rather than being refused here.
    Locale(&'static str),
    /// The language list's search field changed, by its own text.
    ///
    /// The reference's language pane searches its list as the reader types
    /// (`language-settings-selector.vue`'s `Input` over a Fuse index), and this
    /// is the text that decides which rows are drawn.
    LocaleSearch(String),
    /// A control drawn by the shell itself published a pointer crossing.
    ///
    /// The rail has its own tween and its own message ([`Message::Hover`]); the
    /// modal's controls are built by [`crate::ui`], which asks for this.
    Control {
        /// The control's stable name, one per control.
        key: &'static str,
        /// Whether the pointer arrived or left.
        over: bool,
        /// The hover end, where the control declares one of its own.
        hover: Option<f32>,
    },
    /// The right panel was shown or hidden.
    Sidebar(bool),
    /// One of the install dialog's rows: put this project into that instance.
    InstallInto(String),
    /// The install dialog's pack action: make an instance out of this project.
    InstallPack,
    /// The panel's news feed came back, or did not.
    News(Result<Vec<NewsArticle>, String>),
    /// One of the news section's links: open it in the machine's own browser.
    OpenUrl(String),
    /// The link note's dismiss button.
    DismissLinkNote,
    /// The install that was asked for came back, with the line it succeeded with
    /// or why it did not.
    ///
    /// [`store::Outcome`] rather than the [`store::Installed`] value itself: the
    /// page that shows it is a page, and what a page draws is a line of text -- but
    /// a pack *made* something, and the reader is left in it rather than on the
    /// project page.
    Installed(Result<store::Outcome, String>),
    /// The install dialog's empty state: open the creation flow, which is where an
    /// instance comes from in the first place.
    OpenCreate,
    /// The accounts card's header: open its body or close it.
    ToggleAccounts,
    /// One of the card's rows: sign a launch in as this account.
    ///
    /// The uuid rather than the row's index, because the file is what the
    /// selection means -- the profile id is the account, and a list that was read
    /// again between the press and the frame cannot renumber it.
    SelectAccount(String),
    /// One of the card's rows: take this account away.
    RemoveAccount(String),
    /// The card's sign-in and add-account controls, which open the Microsoft
    /// flow this stage does not have yet.
    SignIn,
    /// The sentence under the card was read.
    DismissAccountsNote,
    /// One of the checklist's three steps, pressed.
    ///
    /// The step rather than its label, because what a press *does* is the step's
    /// own: one opens the creation flow, one opens the accounts card's sign-in and
    /// one has no flow behind it at all.
    Checklist(Step),
    /// The checklist's header: open its body or close it.
    ToggleChecklist,
    /// The sentence under the checklist was read.
    DismissModrinthNote,
    /// The action bar's download chip: open or close the panel under the head.
    ToggleDownloads,
    /// The chevron beside the running instance's name: open or close the popover
    /// over every running instance.
    ToggleRuns,
    /// One of that popover's rows: make this run the one the chip is about.
    ///
    /// The instance id rather than a position, for [`Message::SelectAccount`]'s
    /// reason: the list is drawn from what is running, and a run that ended
    /// between the press and the frame would renumber it.
    SelectRun(String),
    /// A stop control: end the run of the instance this press was drawn for.
    ///
    /// Named rather than implicit, because there is more than one stop control
    /// now -- the chip's own, which stops the instance it names, and one per row
    /// of the popover, which stops that row's.
    StopRun(String),
    /// The instance-settings modal's own controls, and its save.
    InstanceSettings(crate::instance_settings::Message),
    /// Mojang's version list, read for the instance-settings modal's
    /// installation tab.
    InstallationVersions(Result<store::VersionList, String>),
    /// The pack an instance was installed from, named by the service, for the same
    /// tab's card.
    ///
    /// `Ok(None)` is a link that was taken away while the request was out, which
    /// the card draws as no link at all: the answer is about the file as it was,
    /// and the reader has since said it is not what they want.
    Modpack(Result<Option<store::LinkedModpack>, String>),
    /// The installation tab's *Repair instance*, over: the instance's own files
    /// were installed again, with every file already on disk checked against the
    /// digest its metadata publishes, and the sentence is what the check came to.
    ///
    /// The modal's own line under the button, landed like [`Message::Modpack`]'s
    /// card and for the same reason: a repair is that form's action, and a modal
    /// that was closed while the check ran has nowhere to draw a sentence about an
    /// instance the reader has left.
    Repaired(Result<String, String>),
    /// The versions of the linked pack that fit this instance, read for the same
    /// tab's *Change version* list.
    ///
    /// The modal's own field, landed like [`Message::Modpack`]'s card: an empty
    /// answer is a pack that publishes nothing for the game version and loader this
    /// instance is.
    PackVersions(Result<Vec<store::PackVersion>, String>),
    /// The installation tab's *Re-install modpack*, over: the linked version was
    /// laid over the instance again, and the sentence is what that came to.
    ///
    /// Landed on the modal for [`Message::Repaired`]'s reason: a pack being laid
    /// over is that form's action, and a modal closed while it ran has nowhere to
    /// draw a sentence about an instance the reader has left.
    Reinstalled(Result<String, String>),
    /// The same tab's *Change version*, over: the version the reader pressed was
    /// laid over the instance, and the sentence is what that came to.
    VersionChanged(Result<String, String>),
    /// One platform's builds at one game version, read for the same tab. The
    /// question travels back with the answer, for [`Message::LoaderBuilds`]'s
    /// reason: the reader can move to another version while it is out.
    InstallationBuilds {
        /// The platform the builds were asked about.
        loader: crate::catalog::LoaderKind,
        /// The game version they were asked about.
        game: String,
        /// The builds the loader's own service published, or why it could not
        /// be read.
        builds: Result<Vec<store::LoaderBuild>, String>,
    },
    /// A modal asked to close, from its own button or from its scrim.
    CloseModal,
    /// The name in the creation dialog changed.
    CreateName(String),
    /// The creation dialog's own button: make the instance.
    Create,
    /// The answer to a create: the new instance's id, or why there is none.
    Created(Result<String, String>),
    /// One of the import dialog's rows: bring this instance in.
    Import(std::path::PathBuf),
    /// The answer to an import: the instance's new id, or why there is none.
    Imported(Result<String, String>),
    /// The answer to the dialog's version request: Mojang's list, or why there is
    /// none.
    Versions(Result<store::VersionList, String>),
    /// The dialog's version search field changed.
    VersionQuery(String),
    /// The picker's footer: show everything Mojang publishes, or only releases.
    VersionSnapshots(bool),
    /// A version was picked in the dialog.
    VersionChoice(String),
    /// A modloader chip was pressed: make the instance with this loader.
    LoaderChoice(crate::catalog::LoaderKind),
    /// One of the loader-version chips was pressed.
    BuildChoice(BuildChoice),
    /// The dialog's build search field changed.
    BuildQuery(String),
    /// A build was picked in the dialog's list.
    BuildPicked(String),
    /// The answer to the dialog's loader-build request: the loader's own builds
    /// for one game version, or why there are none.
    ///
    /// The question travels with the answer rather than being read from the
    /// dialog when it arrives, because the dialog may have moved on: `loader` and
    /// `game` are what tell an answer about the choice in force from one about a
    /// choice the user has replaced.
    LoaderBuilds {
        /// The loader the request was made for.
        loader: crate::catalog::LoaderKind,
        /// The game version it was made for.
        game: String,
        /// The builds, newest first, or the reason there are none.
        builds: Result<Vec<store::LoaderBuild>, String>,
    },
    /// One thing a launch reported: a batch of lines, a level, the game coming
    /// up, the run ending, or a session it renewed.
    ///
    /// The worker speaks [`launch::LaunchEvent`] rather than this enum because
    /// two shells watch a launch, and this variant is the whole of the crossing.
    Launched(launch::LaunchEvent),
    /// One frame of the clock.
    Tick,
    Minimize,
    ToggleMaximize,
    Close,
    /// The pointer scrolled in one of this shell's own scroll regions.
    ///
    /// A page's region raises the same shape inside its own message and the
    /// instance-settings modal's inside its; [`Shell::handle`] takes all three
    /// before `act` can hand any of them to a page, because the answer is a
    /// command and a page has no way to return one (see [`crate::scroll`]). The
    /// name is the region's own, which is what the shell's glides are keyed by.
    Wheel(&'static str, crate::scroll::Wheel),
    /// The window's own state changed, reported by the window procedure: it is
    /// maximized now, or it is not, or the pointer moved on or off the maximize
    /// control. See [`window_state`] for why neither can be a widget's message.
    WindowStateChanged,
    /// The window's own size, in logical pixels, reported by iced when it is
    /// resized. See [`window_size`] for what needs to know it: a dialog taller
    /// than the window it is centred in is a dialog whose own close button is off
    /// the screen.
    Viewport(iced::Size),
    /// A `--shot` run's settle timer fired: ask the window for its frame.
    ShotDue,
    /// The frame [`Message::ShotDue`] asked for, to be written and to end the run.
    ShotTaken(iced::window::Screenshot),
    /// A frame a window owes its own first one. See [`REPAINT_KICKS`] for the
    /// measurement that asks for it, and [`Shell::repaint`] for how it arrives.
    Repaint,
}

impl Shell {
    /// A shell at `address`, in `theme`, with the settings' own panel state.
    pub fn new(address: Address, theme: Gen, settings: &RailSettings) -> Shell {
        // Built before the address is moved into the shell, which is what the
        // screen an address draws from is.
        let screen = Screen::at(&address);
        let mut shell = Shell {
            address,
            back: Vec::new(),
            forward: Vec::new(),
            theme,
            sidebar: !settings.hide_sidebar,
            skins_slot: settings.show_skins,
            screenshots_slot: settings.show_screenshots,
            hovered: None,
            plates: Rail::ALL.iter().map(|_| Tween::at(0.0, Timing::NAV_PLATE)).collect(),
            modal: None,
            settings_tab: SettingsTab::Appearance,
            maximized: false,
            viewport: DIALOG_VIEWPORT,
            glides: crate::scroll::Glides::default(),
            shot: None,
            shot_taken: false,
            kicks: 0,
            create_name: String::new(),
            create_error: None,
            creating: false,
            create_requested: false,
            import_found: Vec::new(),
            import_error: None,
            importing: false,
            import_requested: None,
            versions: Load::Idle,
            version_query: String::new(),
            version_snapshots: false,
            version_choice: None,
            versions_requested: false,
            create_loader: crate::catalog::LoaderKind::Fabric,
            build_choice: BuildChoice::default(),
            build: None,
            build_query: String::new(),
            loader_builds: Load::Idle,
            loader_builds_for: None,
            loader_builds_requested: false,
            installation_versions_requested: false,
            installation_builds_requested: None,
            installation_modpack_requested: None,
            installation_repair_requested: None,
            installation_pack_versions_requested: None,
            installation_reinstall_requested: None,
            installation_change_requested: None,
            runs: Vec::new(),
            next_run_id: 0,
            accounts: None,
            accounts_warning: None,
            locale_query: String::new(),
            accounts_open: false,
            accounts_note: None,
            checklist_open: true,
            modrinth_note: None,
            news: Load::Loading,
            link_note: None,
            jobs: BTreeMap::new(),
            downloads: false,
            switchers: false,
            installing: false,
            install_requested: None,
            pack_requested: None,
            install_error: None,
            installed: None,
            screen,
            store: Store::default(),
            prefs: crate::prefs::Prefs::default(),
            home: None,
        };
        shell.settle();
        shell
    }

    /// A shell with a store behind it, which is how the application builds one.
    pub fn with_store(mut self, store: Store) -> Shell {
        self.store = store;
        self
    }

    /// The preferences the shell's own settings come from, and the file behind
    /// them.
    pub fn with_prefs(
        mut self,
        home: palantir_core::paths::PalantirPaths,
        prefs: crate::prefs::Prefs,
    ) -> Shell {
        self.prefs = prefs;
        self.home = Some(home);
        self
    }

    /// The launcher's accounts, which a launch signs in with, and what reading
    /// them could not do.
    pub fn with_accounts(mut self, accounts: AccountsStore, warning: Option<String>) -> Shell {
        self.accounts = Some(accounts);
        self.accounts_warning = warning;
        self
    }

    /// Take a colour theme: the window, the setting, and the file.
    ///
    /// A write that fails does not undo the choice, for [`crate::prefs`]'s
    /// reason: the window should still look the way it was just asked to, and a
    /// settings file that cannot be written is a problem for the next launch
    /// rather than a reason to refuse this one.
    fn choose_theme(&mut self, choice: ColorTheme) {
        self.prefs.color_theme = choice.id().to_string();
        self.theme = generated_theme(choice, crate::theme::os_prefers_light());
        if let Some(home) = &self.home {
            let _ = crate::prefs::save(home, &self.prefs);
        }
    }

    /// Take a language: the module, the setting, and the file.
    ///
    /// The same shape as [`Shell::choose_theme`], for the same reason: what is in
    /// force changes now and the file is written so the next launch starts in it. A
    /// language is *not* re-rendered from a stored table the way a theme is -- the
    /// tables are all in the binary and [`crate::locale`] is what reads one -- so
    /// there is nothing to recompute here, only to record and to redraw.
    ///
    /// English is stored as the empty string rather than as `"en-US"`, which is
    /// [`crate::prefs`]'s own rule: a setting nobody changed is not written, and
    /// "no language chosen" has to read as the default rather than as a choice.
    fn choose_locale(&mut self, tag: &'static str) {
        crate::locale::set(tag);
        self.prefs.locale = if tag == crate::locale::ENGLISH {
            None
        } else {
            Some(tag.to_string())
        };
        if let Some(home) = &self.home {
            let _ = crate::prefs::save(home, &self.prefs);
        }
    }

    /// Where the shell is.
    #[cfg(test)]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Whether the pane is drawing a page rather than a placeholder.
    ///
    /// True now that the panes are the pages: every route the address table knows
    /// builds one (`pages::Screen::at`), and each page's own gate covers its
    /// states. The flag stays because it is what the shell's gate asserts, and a
    /// future route without a page would make it false again by construction.
    #[cfg(test)]
    pub fn pages_are_drawn(&self) -> bool {
        true
    }

    /// The mark a rail slot carries for the address on screen.
    fn mark(&self, slot: Rail) -> Option<Mark> {
        self.address
            .marks()
            .into_iter()
            .find(|(rail, _)| *rail == slot)
            .map(|(_, mark)| mark)
    }

    /// The tween for a rail slot.
    fn plate(&self, slot: Rail) -> Tween {
        let index = Rail::ALL.iter().position(|rail| *rail == slot).unwrap_or(0);
        self.plates.get(index).copied().unwrap_or_else(|| Tween::at(0.0, Timing::NAV_PLATE))
    }

    /// The path a rail slot leads to, where it leads anywhere.
    ///
    /// Three of the eight slots are buttons rather than links, which is the
    /// reference's own arrangement and the reason [`crate::route::Rail`] names
    /// them: create opens the creation flow, settings opens [`Modal::Settings`],
    /// and the profile slot opens the account menu.
    fn destination(slot: Rail) -> Option<&'static str> {
        match slot {
            Rail::Home => Some("/"),
            Rail::Discover => Some("/browse/modpack"),
            Rail::Skins => Some("/skins"),
            Rail::Screenshots => Some("/screenshots"),
            Rail::Servers => Some("/hosting/manage/"),
            Rail::CreateInstance | Rail::Settings | Rail::Profile => None,
        }
    }

    /// Whether an icon for a control that cannot be used is greyed out.
    fn icon_ink(&self, enabled: bool) -> Color {
        if enabled {
            theme_gen::ink(self.theme, INK_DEFAULT)
        } else {
            disabled(self.theme, INK_DEFAULT)
        }
    }

    /// Point the selection tweens at the marks the address on screen asks for.
    ///
    /// Called on every navigation. A slot that keeps its mark is left alone --
    /// [`Tween::retarget`] is a no-op when the target has not changed -- so
    /// moving between two Discover pages does not restart the plate.
    fn settle(&mut self) {
        for (index, slot) in Rail::ALL.iter().enumerate() {
            let target = if self.mark(*slot) == Some(Mark::Primary) { 1.0 } else { 0.0 };
            if let Some(tween) = self.plates.get_mut(index) {
                tween.retarget(target);
            }
        }
    }

    /// Move to an address, recording it in the history.
    fn go(&mut self, address: Address) {
        if address == self.address {
            return;
        }
        self.forward.clear();
        self.back.push(std::mem::replace(&mut self.address, address));
        self.settle();
        self.screen.retarget(&self.address);
        self.forget_pointer();
    }

    /// Forget where the pointer was on the page that is being left.
    ///
    /// A control the pointer was on is not drawn on the page that arrives, and
    /// the page's controls draw from the clock rather than keeping a pointer
    /// state of their own -- so without this a control the new page happens to
    /// name the same way would arrive lit.
    fn forget_pointer(&self) {
        if let Ok(mut clock) = anim::clock().lock() {
            clock.forget_pointer();
        }
    }

    /// Go back one page, if there is one.
    fn back(&mut self) {
        let Some(previous) = self.back.pop() else {
            return;
        };
        self.forward.push(std::mem::replace(&mut self.address, previous));
        self.settle();
        // Back and forward move between pages the same way a link does: the page
        // that is already built keeps what it can (a tab change is not a new page)
        // and anything else is built fresh, which is what a browser does when it
        // returns to a document it no longer holds.
        self.screen.retarget(&self.address);
        self.forget_pointer();
    }

    /// Go forward one page, if the user has not navigated since.
    fn forward(&mut self) {
        let Some(next) = self.forward.pop() else {
            return;
        };
        self.back.push(std::mem::replace(&mut self.address, next));
        self.settle();
        self.screen.retarget(&self.address);
        self.forget_pointer();
    }

    /// Advance every moving tween by `delta`.
    fn advance(&mut self, delta: Duration) {
        for tween in &mut self.plates {
            tween.advance(delta);
        }
    }

    /// Take a wheel on one scroll region, and answer with the command that moves
    /// it to where the policy says it should be.
    ///
    /// This is the whole of the scrolling the shell owns. [`crate::scroll::Glides`]
    /// keeps one glide per named region, decides whether the offset the wheel
    /// measured belongs to this shell or to something else that moved the region,
    /// and eases toward the target the notches asked for. What comes back is
    /// either the frame that starts a glide or -- on a machine measured too slow
    /// to draw one -- the whole of the gesture.
    fn glide(&mut self, name: &'static str, wheel: crate::scroll::Wheel) -> iced::Command<Message> {
        self.glides.wheel(name, wheel, std::time::Instant::now())
    }

    /// Whether anything is still moving, which is what keeps the clock awake.
    ///
    /// Three things now, and every one of them has to be asked. The rail's plates
    /// are the shell's own; the pages' controls are [`crate::anim`]'s process-wide
    /// interaction clock -- a hover that started on a control is a frame
    /// subscription the shell owes it, or the tween would paint its first frame
    /// and sit there (see [`crate::hover`]); and a scroll region mid-glide is
    /// waiting for exactly this frame ([`crate::scroll::Glides`]).
    pub fn animating(&self) -> bool {
        if self.plates.iter().any(Tween::is_running) {
            return true;
        }
        // A region mid-glide is moving as well, and the frame it is waiting for is
        // this one: without it a glide would draw its first frame and stop there.
        if self.glides.animating() {
            return true;
        }
        anim::clock().lock().map(|clock| clock.animating()).unwrap_or(false)
    }

    /// Whether the right panel belongs on screen.
    ///
    /// The reference's `sidebarVisible`: the user's toggle, or a page that
    /// forces it (`App.vue`'s `forceSidebar`, on Discover, Project and User).
    fn panel_shown(&self) -> bool {
        self.sidebar || self.address.route.forces_sidebar()
    }

    /// Whether the head draws the arrow that opens and closes the panel.
    ///
    /// The reference's own gate, and the reason it is not simply "always":
    /// `App.vue` puts `v-if="!forceSidebar && appSettings.toggleSidebar"` on the
    /// `IconButton`, so on Discover, Project and User -- the three routes in
    /// [`route::Route::forces_sidebar`] -- there is no arrow at all. The panel is
    /// up on those pages whatever the reader's toggle says, so a control drawn
    /// there is one whose press cannot move the thing it points at.
    fn panel_toggle_shown(&self) -> bool {
        !self.address.route.forces_sidebar()
    }

    /// Apply a message.
    ///
    /// Named `handle` rather than `update` so that a call can never be read as
    /// the trait's own method, which arrives one indirection later.
    ///
    /// A message goes in and at most one command comes out, in two steps: the
    /// window controls, which are commands of their own rather than page state;
    /// then [`Shell::act`], whose only kind of answer is a request. A request is
    /// either the one the message asked for or the one the page it left behind
    /// owes -- never both, and never two.
    fn handle(&mut self, message: Message) -> iced::Command<Message> {
        // Hand the window's frame to Windows, as soon as there is a window.
        //
        // `native::install_hit_test` takes over `WM_NCHITTEST`, which is what
        // makes Windows run its own resize loop (with its own cursors, including
        // the diagonals iced has no way to ask for) and what puts Windows 11's
        // Snap Layouts on the maximize control. It cannot happen in
        // [`Shell::new`]: iced runs `Application::new` *before* it builds the
        // window. The first dispatched message is the earliest safe moment, and
        // there is one before the window is ever painted. Once it has taken
        // effect this is an atomic load, so it is called from every message
        // rather than tracked. A failure is not reported and not retried forever:
        // the window stays usable, and what is lost is the native cursors, the
        // resize edges and the flyout.
        let _ = crate::native::install_hit_test();
        match message {
            Message::Minimize => return window::minimize(Id::MAIN, true),
            Message::ToggleMaximize => {
                // With the shim in place this arm is not what the button does:
                // Windows answers that point as a caption button once
                // [`caption_target`] names it, so the click goes to the window
                // rather than to iced. This is what a window without the shim
                // has, and asking the window instead of inverting a flag keeps
                // the two paths from disagreeing.
                self.maximized = !crate::native::window_maximized().unwrap_or(self.maximized);
                return window::maximize(Id::MAIN, self.maximized);
            }
            Message::Close => return window::close(Id::MAIN),
            // The caption reads the window itself, so this arm's real job is to
            // be a message at all: it is what makes iced rebuild the view, and
            // the rebuild is what shows a maximize the user performed with Snap,
            // the taskbar or the keyboard.
            Message::WindowStateChanged => {
                self.maximized = crate::native::window_maximized().unwrap_or(self.maximized);
                return iced::Command::none();
            }
            // The window's own size, recorded because the modal layer is the one
            // thing in this shell that has to know it and a widget cannot ask the
            // window anything: what is drawn centred and taller than the window is
            // clipped at both ends, and there is no scroll to reach it with.
            Message::Viewport(size) => {
                self.viewport = size;
                return iced::Command::none();
            }
            // The frame a window owes its own first one, which is the frame a page
            // is drawn in: see [`REPAINT_KICKS`]. The count is all this arm has to
            // do -- the message's own job is to *be* a message, because a message
            // is what makes iced build a frame.
            Message::Repaint => {
                self.kicks = self.kicks.saturating_add(1);
                return iced::Command::none();
            }
            // The capture is taken here, at the point the runtime is asked for
            // the window's frame, rather than by a tool outside the process:
            // iced draws this window, so it is the only thing that can hand back
            // exactly what was drawn. `write_shot` then writes it and the window
            // closes, which is what ends a capture run.
            Message::ShotDue => {
                if self.shot.is_some() && !self.shot_taken {
                    self.shot_taken = true;
                    return window::screenshot(Id::MAIN, Message::ShotTaken);
                }
                return iced::Command::none();
            }
            Message::ShotTaken(shot) => {
                if let Some(path) = self.shot.clone() {
                    if let Err(error) = write_shot(&path, &shot) {
                        eprintln!("shot failed: {error}");
                    }
                }
                return window::close(Id::MAIN);
            }
            // A wheel is not a page's to apply, and not a dialog's either: iced
            // moves a `Scrollable` with `scroll_to`, which is a command, and a
            // command is this match's to return. Every shape a wheel can arrive in
            // is one of these three -- this shell's own regions, a page's, and the
            // instance-settings modal's -- and all three answer the same way.
            Message::Wheel(name, wheel) => return self.glide(name, wheel),
            Message::Screen(
                pages::Message::Home(home::Message::Wheel(name, wheel))
                | pages::Message::Discover(discover::Message::Wheel(name, wheel))
                | pages::Message::Project(project::Message::Wheel(name, wheel))
                | pages::Message::Instance(instance::Message::Wheel(name, wheel))
                | pages::Message::Skins(skins::Message::Wheel(name, wheel))
                | pages::Message::Screenshots(screenshots::Message::Wheel(name, wheel))
                | pages::Message::Servers(servers::Message::Wheel(name, wheel))
                | pages::Message::User(user::Message::Wheel(name, wheel)),
            ) => return self.glide(name, wheel),
            Message::InstanceSettings(crate::instance_settings::Message::Wheel(name, wheel)) => {
                return self.glide(name, wheel)
            }
            // The frame clock, which exists only while something is moving. The
            // regions glide on it with everything else -- one clock in the window,
            // whichever page is drawing -- and what it answers with is the offsets
            // of the regions that this frame moved.
            Message::Tick => {
                let now = std::time::Instant::now();
                self.advance(FRAME);
                if let Ok(mut clock) = anim::clock().lock() {
                    clock.tick(now);
                }
                return self.glides.tick(now);
            }
            _ => {}
        }
        if let Some(asked) = self.act(message) {
            return match asked {
                Asked::Search(asked) => self.search(asked),
                Asked::Icons(asked) => self.icons(asked),
                Asked::Tags => self.tags(),
                Asked::Instance(asked) => self.instance(asked),
                Asked::Project(asked) => self.project(asked),
                Asked::User(asked) => self.user(asked),
                Asked::Skins(asked) => self.skins(asked),
                Asked::Wear(worn) => self.wear(worn),
                Asked::AddSkin(add) => self.add_skin(add),
                Asked::EditSkin(edit) => self.edit_skin(edit),
                Asked::Reorder(order) => self.reorder_skins(order),
            };
        }
        // A create and an import are not a page's requests and do not go through
        // `Asked`, so they are raised as flags by `act` and taken here: one press,
        // one command.
        if std::mem::take(&mut self.create_requested) {
            return self.create();
        }
        if let Some(source) = self.import_requested.take() {
            return self.import(source);
        }
        if let Some((project, instance)) = self.install_requested.take() {
            return self.install(&project, &instance);
        }
        if let Some(project) = self.pack_requested.take() {
            return self.install_pack(&project);
        }
        if let Some(result) = self.installed.take() {
            // The sentence reaches the page the way the shell tells a page
            // anything -- `Message::Screen` -- applied here rather than through a
            // command that yields it: iced has no "send this message now" command,
            // and a future that resolved to one would be adding a turn to a
            // sentence the page can draw in this frame.
            let _ = self.act(Message::Screen(pages::Message::install_result(result)));
        }
        if std::mem::take(&mut self.versions_requested) {
            return self.versions_command();
        }
        if std::mem::take(&mut self.loader_builds_requested) {
            return self.loader_builds_command();
        }
        // The instance-settings modal's reads and its one action, batched because
        // they are owed at once and none waits on another: the version list is
        // Mojang's, the builds are the loader's, the pack's name is Modrinth's, and
        // the repair fetches from all three -- so a modal opened on the
        // installation tab asks for what it is owed and draws whichever lands
        // first.
        let mut installation = Vec::new();
        if std::mem::take(&mut self.installation_versions_requested) {
            installation.push(self.installation_versions_command());
        }
        if let Some((loader, game)) = self.installation_builds_requested.take() {
            installation.push(self.installation_builds_command(loader, &game));
        }
        if let Some(project) = self.installation_modpack_requested.take() {
            installation.push(self.installation_modpack_command(&project));
        }
        if let Some(instance) = self.installation_repair_requested.take() {
            installation.push(self.installation_repair_command(&instance));
        }
        if let Some(instance) = self.installation_pack_versions_requested.take() {
            installation.push(self.installation_pack_versions_command(&instance));
        }
        if let Some(instance) = self.installation_reinstall_requested.take() {
            installation.push(self.installation_reinstall_command(&instance));
        }
        if let Some((instance, version)) = self.installation_change_requested.take() {
            installation.push(self.installation_change_command(&instance, &version));
        }
        if !installation.is_empty() {
            return iced::Command::batch(installation);
        }
        // Nothing was asked for, but a page may be on screen that has never
        // been asked anything -- a tab that was just switched, or the page a
        // window opened on. Both are the same answer: ask on its behalf.
        self.opening_command()
    }

    /// Apply a message, and answer with the request it made.
    ///
    /// Everything that is not a request is a change of state here, which is what
    /// keeps the return type one thing: a `None` means "nothing left this turn",
    /// and the only thing that can leave is a request.
    fn act(&mut self, message: Message) -> Option<Asked> {
        match message {
            Message::Go(path) => {
                if let Some(address) = Address::parse(&path) {
                    self.go(address);
                }
                None
            }
            Message::Back => {
                self.back();
                None
            }
            // Not one of these is a request the pages could answer, and none of
            // them reaches this far: `handle` answers the window's own state, the
            // capture's timer and the owed frames before it asks `act` anything.
            // They are matched here so that adding a message is a compile error in
            // the one place that has to decide what to do with it.
            Message::WindowStateChanged
            | Message::Viewport(_)
            | Message::ShotDue
            | Message::ShotTaken(_)
            | Message::Repaint => None,
            Message::Forward => {
                self.forward();
                None
            }
            Message::Hover(slot) => {
                self.hovered = slot;
                None
            }
            Message::ColorTheme(choice) => {
                self.choose_theme(choice);
                None
            }
            Message::SettingsTab(tab) => {
                self.settings_tab = tab;
                None
            }
            Message::Locale(tag) => {
                self.choose_locale(tag);
                None
            }
            Message::LocaleSearch(query) => {
                self.locale_query = query;
                None
            }
            Message::InstallInto(instance) => {
                if !self.installing {
                    if let Some(Modal::Install { project, .. }) = &self.modal {
                        // Set as the request leaves rather than when it comes
                        // back: a second press in the gap would put two transfers
                        // on one file, through one part file.
                        self.installing = true;
                        self.install_error = None;
                        self.install_requested = Some((project.clone(), instance));
                    }
                }
                None
            }
            Message::InstallPack => {
                if !self.installing {
                    if let Some(Modal::Install { project, pack: true, .. }) = &self.modal {
                        // The same guard for the same reason, one step stronger:
                        // this press unpacks an archive and then downloads every
                        // file it lists.
                        self.installing = true;
                        self.install_error = None;
                        self.pack_requested = Some(project.clone());
                    }
                }
                None
            }
            Message::Installed(result) => {
                self.installing = false;
                match &result {
                    Ok(outcome) => {
                        // The dialog has done its job and the sentence belongs
                        // where the button was: a modal over it would be covering
                        // the answer it just produced.
                        self.modal = None;
                        self.install_error = None;
                        // A pack made an instance, and the reference leaves the
                        // reader in what it just made -- so the list the pages are
                        // drawn from is read again, and the reader is sent to it.
                        if let store::Outcome::Pack { id, .. } = outcome {
                            self.store.reload();
                            self.go(Address::at(route::Route::Instance {
                                id: id.clone(),
                                tab: route::InstanceTab::Content,
                            }));
                        }
                    }
                    // A failure keeps the dialog up, because the answer to it is
                    // to pick a different instance -- which needs the list.
                    Err(reason) => self.install_error = Some(reason.clone()),
                }
                // A file's line goes on to the page, because the page that asked
                // is still the page on screen and the line stays after the dialog
                // is gone -- which is what makes it worth saying. A pack's does
                // not: the reader has been moved to the instance it made, and a
                // notice is a page's, drawn on a page they are no longer on. The
                // instance appearing in the library is the feedback, and it is a
                // better one than a sentence about it.
                self.installed = match result {
                    Ok(store::Outcome::File(line)) => Some(Ok(line)),
                    Ok(store::Outcome::Pack { .. }) => None,
                    Err(reason) => Some(Err(reason)),
                };
                None
            }
            Message::OpenCreate => {
                self.open_create();
                None
            }
            Message::CreateName(name) => {
                self.create_name = name;
                // The sentence was about the name that has just changed, so it is
                // not about this one.
                self.create_error = None;
                None
            }
            Message::Create => {
                if !self.creating {
                    // Marked busy as the request leaves rather than when it comes
                    // back: a second press before the answer arrives would create
                    // the instance twice.
                    self.creating = true;
                    self.create_requested = true;
                }
                None
            }
            Message::Import(source) => {
                if !self.importing {
                    self.importing = true;
                    self.import_requested = Some(source);
                }
                None
            }
            Message::Launched(event) => {
                self.launched(event);
                None
            }
            Message::Versions(result) => {
                // A list with nothing in it is `Empty` rather than a list: the
                // picker draws "no versions available" for one and a scrollable
                // of nothing for the other.
                self.versions = match result {
                    Ok(list) if list.versions.is_empty() => Load::Empty,
                    Ok(list) => Load::Ready(list),
                    Err(reason) => Load::Failed(reason),
                };
                // The loader's build list is asked for by game version, and the
                // version list is what tells the dialog which one is in force:
                // a dialog opened a moment ago has its answer here and nowhere
                // earlier, so this is where its own request is raised.
                if self.create_loader.loads_mods() && self.loader_builds_for.is_none() {
                    self.ask_loader_builds();
                }
                None
            }
            Message::VersionQuery(query) => {
                self.version_query = query;
                None
            }
            Message::VersionSnapshots(show) => {
                self.version_snapshots = show;
                None
            }
            Message::VersionChoice(id) => {
                self.version_choice = Some(id);
                // A different game version is a different set of loader builds,
                // so the list the dialog is holding is no longer the answer to
                // anything.
                self.ask_loader_builds();
                None
            }
            Message::LoaderChoice(loader) => {
                self.choose_loader(loader);
                self.ask_loader_builds();
                None
            }
            Message::BuildChoice(choice) => {
                self.build_choice = choice;
                // The list is only read under this chip, and a user who pressed
                // it has been told nothing yet if the request for it was never
                // made -- which is the arm a loader chip pressed before the
                // version list arrived lands in.
                if self.loader_builds_for.is_none() {
                    self.ask_loader_builds();
                }
                None
            }
            Message::BuildQuery(query) => {
                self.build_query = query;
                None
            }
            Message::BuildPicked(build) => {
                self.build = Some(build);
                None
            }
            Message::LoaderBuilds { loader, game, builds } => {
                // The answer is dropped unless it is the answer to the question
                // in force: see [`Shell::loader_builds_for`].
                if self.loader_builds_for.as_ref() == Some(&(loader, game)) {
                    // A list with nothing in it is `Empty` rather than a list,
                    // for [`Shell::versions`]'s reason: one draws "no builds for
                    // this game version" and the other a scrollable of nothing.
                    self.loader_builds = match builds {
                        Ok(builds) if builds.is_empty() => Load::Empty,
                        Ok(builds) => Load::Ready(builds),
                        Err(reason) => Load::Failed(reason),
                    };
                }
                None
            }
            Message::InstallationVersions(result) => {
                // The installation tab's own read, landed on the modal rather than
                // on the dialog: the two version lists are read by different
                // readers and drawn in different places, so one reply message
                // would have the second answer erase the first.
                if let Some(Modal::InstanceSettings(state)) = &mut self.modal {
                    // A list with nothing in it is `Empty`, for
                    // [`Shell::versions`]'s reason.
                    state.versions = match result {
                        Ok(list) if list.versions.is_empty() => Load::Empty,
                        // The list the picker draws is the versions alone: the
                        // dialog's recommendation (`latest_release`) is the
                        // creation flow's question and not this tab's.
                        Ok(list) => Load::Ready(list.versions),
                        Err(reason) => Load::Failed(reason),
                    };
                }
                self.installation_needs();
                None
            }
            Message::Repaired(result) => {
                // The check's own sentence, landed on the modal like the card's
                // name below and for the same reason: it is that form's field. A
                // modal that went away while the check ran keeps nothing, and the
                // files are installed all the same.
                if let Some(Modal::InstanceSettings(state)) = &mut self.modal {
                    state.repair = match result {
                        Ok(line) => Load::Ready(line),
                        Err(reason) => Load::Failed(reason),
                    };
                }
                None
            }
            Message::PackVersions(result) => {
                // The list, landed on the modal like the two lists beside it. An
                // empty answer is `Empty`, which draws the reference's own sentence
                // for a list with nothing in it.
                if let Some(Modal::InstanceSettings(state)) = &mut self.modal {
                    state.pack_versions = match result {
                        Ok(versions) if versions.is_empty() => Load::Empty,
                        Ok(versions) => Load::Ready(versions),
                        Err(reason) => Load::Failed(reason),
                    };
                }
                self.installation_needs();
                None
            }
            Message::Reinstalled(result) => {
                // The pack's own sentence, landed like the repair's: the link is
                // unchanged by a re-install, so nothing else on the form moves.
                if let Some(Modal::InstanceSettings(state)) = &mut self.modal {
                    state.reinstall = match result {
                        Ok(line) => Load::Ready(line),
                        Err(reason) => Load::Failed(reason),
                    };
                }
                None
            }
            Message::VersionChanged(result) => {
                // The link moves with the files, so it is read *again* rather than
                // assumed: a card naming the old version over files the new one
                // wrote is the one thing this answer could get wrong. The link read
                // is taken outside the borrow of the modal, the way
                // `unlink_instance` takes its own.
                let id = match &self.modal {
                    Some(Modal::InstanceSettings(state)) => Some(state.id.clone()),
                    _ => None,
                };
                if let Some(id) = id {
                    let link = self.store.instance_link(&id);
                    if let Some(Modal::InstanceSettings(state)) = &mut self.modal {
                        state.change = match result {
                            Ok(line) => Load::Ready(line),
                            Err(reason) => Load::Failed(reason),
                        };
                        state.link = match link {
                            Ok(Some(link)) => Load::Ready(link),
                            Ok(None) => Load::Idle,
                            Err(problem) => Load::Failed(problem),
                        };
                        // The card names the new version and the list's mark moves
                        // with it, so both are read again -- the list from the pair
                        // that has not changed, but the mark has.
                        state.modpack = Load::Idle;
                        state.pack_versions = Load::Idle;
                    }
                    self.installation_needs();
                }
                None
            }
            Message::Modpack(result) => {
                // The card's name, landed on the modal like the two lists above,
                // and for the same reason: it is that form's own field. A link that
                // went away while the request was out is no link at all, rather
                // than a card naming a pack the reader has just unlinked.
                if let Some(Modal::InstanceSettings(state)) = &mut self.modal {
                    state.modpack = match result {
                        Ok(Some(named)) => Load::Ready(named),
                        Ok(None) => Load::Idle,
                        Err(reason) => Load::Failed(reason),
                    };
                }
                self.installation_needs();
                None
            }
            Message::InstallationBuilds { loader, game, builds } => {
                // The answer is dropped unless it is the answer to the question
                // in force: see [`crate::instance_settings::State::builds_for`].
                if let Some(Modal::InstanceSettings(state)) = &mut self.modal {
                    if state.builds_for.as_ref() == Some(&(loader, game)) {
                        state.builds = match builds {
                            Ok(builds) if builds.is_empty() => Load::Empty,
                            Ok(builds) => Load::Ready(builds),
                            Err(reason) => Load::Failed(reason),
                        };
                    }
                }
                self.installation_needs();
                None
            }
            Message::Imported(result) => {
                self.importing = false;
                match result {
                    Ok(id) => {
                        // The same two things a create does, for the same reasons:
                        // the list is read again, and the reader ends up in the
                        // instance that was brought in.
                        self.store.reload();
                        self.import_error = None;
                        self.modal = None;
                        self.go(Address::at(route::Route::Instance {
                            id,
                            tab: route::InstanceTab::Content,
                        }));
                    }
                    Err(reason) => self.import_error = Some(reason),
                }
                None
            }
            Message::Created(result) => {
                self.creating = false;
                match result {
                    Ok(id) => {
                        // The list the pages are drawn from was read at startup, so
                        // an instance that was just written is on disk and not on
                        // the page until it is read again -- and the flow leaves
                        // the reader in the instance it made.
                        self.store.reload();
                        self.create_error = None;
                        self.create_name.clear();
                        self.modal = None;
                        self.go(Address::at(route::Route::Instance {
                            id,
                            tab: route::InstanceTab::Content,
                        }));
                    }
                    // The dialog stays up with the reason in it: a failure the
                    // reader cannot see is a button that does nothing.
                    Err(reason) => self.create_error = Some(reason),
                }
                None
            }
            Message::Control { key, over, hover } => {
                crate::ui::pointer_with(
                    key,
                    over,
                    hover.unwrap_or_else(crate::theme::hover_brightness),
                );
                None
            }
            Message::Rail(slot) => {
                if let Some(path) = Shell::destination(slot) {
                    if let Some(address) = Address::parse(path) {
                        self.go(address);
                    }
                }
                // The two rail buttons that open a flow rather than a page: the
                // reference's `+` and the gear.
                match slot {
                    Rail::Settings => self.modal = Some(Modal::Settings),
                    Rail::CreateInstance => self.open_create(),
                    _ => {}
                }
                None
            }
            Message::Screen(message) => {
                // A page reports what it wants and this is where the report is
                // acted on: opening a thing is a navigation, and a search is a
                // request, and neither is the page's to perform.
                match self.screen.update(message, &self.store) {
                    Some(pages::Ask::Open(open)) => {
                        let path = match open {
                            pages::Open::Instance(id) => format!("/instance/{id}"),
                            pages::Open::Project(id) => format!("/project/{id}"),
                            // The one navigation that can carry a choice as well as
                            // a name: a profile's filter strip is links in the
                            // reference, so a chosen tab is a different address
                            // rather than a different state.
                            pages::Open::User { user, project_type } => match project_type {
                                Some(kind) => format!("/user/{user}/{}", kind.profile_token()),
                                None => format!("/user/{user}"),
                            },
                        };
                        if let Some(address) = Address::parse(&path) {
                            self.go(address);
                        }
                        None
                    }
                    Some(pages::Ask::Search(asked)) => Some(Asked::Search(asked)),
                    Some(pages::Ask::Icons(asked)) => Some(Asked::Icons(asked)),
                    Some(pages::Ask::Tags) => Some(Asked::Tags),
                    Some(pages::Ask::Instance(asked)) => Some(Asked::Instance(asked)),
                    Some(pages::Ask::Project(asked)) => Some(Asked::Project(asked)),
                    Some(pages::Ask::User(asked)) => Some(Asked::User(asked)),
                    Some(pages::Ask::Skins(asked)) => Some(Asked::Skins(asked)),
                    Some(pages::Ask::Wear(worn)) => Some(Asked::Wear(worn)),
                    // A file dialog is not *owed* by a page's arrival, so it is not in
                    // `opening`'s match above: what this does is ask the reader for a
                    // file they already decided to add.
                    Some(pages::Ask::AddSkin(add)) => Some(Asked::AddSkin(add)),
                    // The editor's three actions, which are also not owed by arrival: a
                    // press of Save, Forget or Take off is a reader's doing.
                    Some(pages::Ask::EditSkin(edit)) => Some(Asked::EditSkin(edit)),
                    // Nor is a move: the order is the reader's, so the rows arrive in
                    // whatever order the store holds until they say otherwise.
                    Some(pages::Ask::Reorder(order)) => Some(Asked::Reorder(order)),
                    Some(pages::Ask::Install(install)) => {
                        // The dialog is opened rather than a transfer started: the
                        // missing half of the request is *which instance*, and only
                        // a reader knows that.
                        self.install_error = None;
                        self.modal = Some(Modal::Install {
                            project: install.id,
                            title: install.title,
                            pack: install.pack,
                        });
                        None
                    }
                    Some(pages::Ask::Create) => {
                        self.open_create();
                        None
                    }
                    Some(pages::Ask::Import) => {
                        self.open_import();
                        None
                    }
                    // The instance page's gear: the modal is the shell's, and so
                    // is the read behind it.
                    Some(pages::Ask::InstanceSettings(id)) => {
                        self.open_instance_settings(&id);
                        None
                    }
                    // The two that start and stop a game rather than a request:
                    // neither answers with one, so `None` -- what the shell owes
                    // the page afterwards travels as a launch event instead.
                    Some(pages::Ask::Play(id)) => {
                        self.play(id);
                        None
                    }
                    Some(pages::Ask::Stop(id)) => {
                        self.stop(&id);
                        None
                    }
                    None => None,
                }
            }
            Message::InstanceSettings(message) => {
                // Saving is the one message here the form cannot act on: the write
                // is the store's, and the store is the shell's -- and there are
                // two of them, because the two tabs write different files.
                match message {
                    crate::instance_settings::Message::Save => self.save_instance_settings(),
                    crate::instance_settings::Message::SaveInstallation => {
                        self.save_instance_installation();
                    }
                    crate::instance_settings::Message::Unlink => self.unlink_instance(),
                    crate::instance_settings::Message::Repair => self.request_repair(),
                    crate::instance_settings::Message::ReinstallModpack => {
                        self.request_reinstall();
                    }
                    crate::instance_settings::Message::ChangeVersion(version) => {
                        self.request_change_version(&version);
                    }
                    other => {
                        if let Some(Modal::InstanceSettings(state)) = &mut self.modal {
                            state.update(other);
                        }
                    }
                }
                // And whatever the form is still owed is raised here rather than
                // when the modal opened: a platform or a game version can change
                // while it is up, and each change is a new question.
                self.installation_needs();
                None
            }
            Message::Sidebar(shown) => {
                self.sidebar = shown;
                None
            }
            Message::ToggleAccounts => {
                self.accounts_open = !self.accounts_open;
                None
            }
            Message::ToggleChecklist => {
                self.checklist_open = !self.checklist_open;
                None
            }
            Message::Checklist(step) => {
                // `App.vue`'s own three handlers: `@create-instance` opens the
                // creation flow, `@login-minecraft` the accounts card's sign-in,
                // and `@login-modrinth` Modrinth's. Two of the three land on an
                // answer this launcher does not have: the first is this shell's
                // own dialog, the second the sentence the card already gives, and
                // the third is the one press here that is not waiting for a slice
                // at all -- this launcher does not hold a Modrinth credential
                // (G118), so its sentence is [`store::needs_account`] rather than
                // the *not implemented yet* the two sign-ins used to share.
                match step {
                    Step::CreateInstance => self.act(Message::OpenCreate),
                    Step::LoginMinecraft => self.act(Message::SignIn),
                    Step::LoginModrinth => {
                        self.modrinth_note = Some(store::needs_account("Signing in to Modrinth"));
                        None
                    }
                }
            }
            Message::DismissModrinthNote => {
                self.modrinth_note = None;
                None
            }
            Message::SelectAccount(uuid) => {
                self.select_account(&uuid);
                None
            }
            Message::RemoveAccount(uuid) => {
                self.remove_account(&uuid);
                None
            }
            Message::SignIn => {
                // `AccountsCard.vue`'s `login()` opens the reference's sign-in
                // modal. The flow it starts is a later stage's here, and the
                // card is where the press was made, so this is where the sentence
                // goes rather than nowhere.
                self.accounts_note = Some(store::not_implemented("Signing in to Minecraft"));
                None
            }
            Message::DismissAccountsNote => {
                self.accounts_note = None;
                None
            }
            Message::News(result) => {
                // A failure is kept in the state rather than thrown away, and the
                // section draws nothing for it -- but the *reason* is here for the
                // next reader of this code, and for a later slice that decides a
                // broken feed is worth a line.
                self.news = match result {
                    Ok(articles) => Load::Ready(articles),
                    Err(reason) => Load::Failed(reason),
                };
                None
            }
            Message::OpenUrl(url) => {
                // The opener is where the scheme is checked: a feed is a
                // stranger's JSON, and a press is what hands a string to the
                // operating system.
                self.link_note = crate::open::url(&url).err();
                None
            }
            Message::DismissLinkNote => {
                self.link_note = None;
                None
            }
            Message::ToggleDownloads => {
                self.downloads = !self.downloads;
                None
            }
            Message::ToggleRuns => {
                self.switchers = !self.switchers;
                None
            }
            Message::SelectRun(id) => {
                self.store.select_launch(&id);
                None
            }
            Message::StopRun(id) => {
                // The instance is the one the control was drawn for, which is what
                // makes a second run's stop button stop the second run: the bar is
                // drawn from what this launcher is running, and a page the user has
                // since navigated away from is not asked.
                self.stop(&id);
                None
            }
            Message::CloseModal => {
                self.modal = None;
                // The Skins editor is the page's state rather than this shell's
                // (`pages::Screen::skins_edit`), so dismissing it is a message to the
                // page. A close with no editor open is the same nothing: the page
                // ignores `CloseEdit` when it holds no edit.
                if self.screen.skins_edit().is_some() {
                    let _ = self.screen.update(
                        pages::Message::Skins(skins::Message::CloseEdit),
                        &self.store,
                    );
                }
                None
            }
            // Handled before this, in `handle`: they are the messages whose
            // answer is a command rather than a request -- and a wheel's answer is
            // that the region it names glides.
            Message::Minimize
            | Message::ToggleMaximize
            | Message::Close
            | Message::Tick
            | Message::Wheel(_, _) => None,
        }
    }

    /// Open the settings modal for one instance.
    ///
    /// The read is synchronous -- the instance's `instance.cfg` and this
    /// launcher's own preferences are two small files -- and it happens here
    /// rather than in the form, because a form that invented a heap for an
    /// instance it could not read would be exactly the failure [`crate::store`]
    /// exists to prevent. A read that fails opens the modal with the sentence
    /// alone (see [`crate::instance_settings::State::failed`]).
    fn open_instance_settings(&mut self, id: &str) {
        let name = self
            .store
            .instance(id)
            .ready()
            .map(|card| card.name.clone())
            .unwrap_or_else(|| id.to_string());
        // Both halves are read here: the modal's two tabs are two files, and a
        // form that opened on one and had to read the other when its tab was
        // pressed would flash an empty installation for a frame. Two small reads,
        // which is why they are synchronous ([`crate::store::Store::instance_settings`]).
        let state = match (
            self.store.instance_settings(id),
            self.store.instance_installation(id),
        ) {
            (Ok(settings), Ok(installation)) => crate::instance_settings::State::new(
                id.to_string(),
                name,
                &settings,
                &installation,
                // The link is read here with the other two, and its failure is
                // deliberately *not* this form's: an instance installed from a pack
                // is still an instance whose heap can be edited, so a link file that
                // cannot be read is the card's own sentence rather than a modal that
                // draws nothing but a complaint.
                match self.store.instance_link(id) {
                    Ok(Some(link)) => Load::Ready(link),
                    Ok(None) => Load::Idle,
                    Err(problem) => Load::Failed(problem),
                },
            ),
            // Either failure draws the sentence alone: a form with values under it
            // would be this launcher claiming a file it never read.
            (Err(problem), _) | (_, Err(problem)) => {
                crate::instance_settings::State::failed(id.to_string(), name, problem)
            }
        };
        self.modal = Some(Modal::InstanceSettings(Box::new(state)));
    }

    /// Forget an instance's link to the pack it came from, and re-read the card.
    ///
    /// The modal stays up: what changed is one card, not the form, and closing it
    /// here would take the reader out of the settings they were in the middle of.
    /// The link is read *again* rather than assumed gone, so a file this launcher
    /// could not remove is a sentence in the form instead of a card that vanishes
    /// while the file it was drawn from is still there.
    fn unlink_instance(&mut self) {
        let Some(Modal::InstanceSettings(state)) = &mut self.modal else {
            return;
        };
        let id = state.id.clone();
        state.error = None;
        match self.store.clear_instance_link(&id) {
            Ok(_) => {
                state.link = match self.store.instance_link(&id) {
                    Ok(Some(link)) => Load::Ready(link),
                    Ok(None) => Load::Idle,
                    Err(problem) => Load::Failed(problem),
                };
                // Nothing to name: the card's own read is dropped with the link it
                // was about, and `needs_modpack` will not ask again while the link
                // is gone.
                state.modpack = Load::Idle;
            }
            Err(problem) => state.error = Some(problem),
        }
    }

    /// Ask for one instance's files to be installed again, and mark the form busy.
    ///
    /// Nothing runs here. A repair is the store's longest blocking call
    /// ([`crate::store::Store::repair_instance`]), so the command leaves from
    /// `handle`'s flag-taking with every other request, and what this does is the
    /// two things that belong to the press: the form is put into the state the
    /// button draws (`Load::Loading`) in the same frame, and the id of the instance
    /// it is about is written down, because the answer comes back minutes
    /// later and the modal may be about something else by then.
    ///
    /// A second press while the first is running -- a keyboard, or a frame raced by
    /// the pointer -- is dropped rather than queued: the form draws the button
    /// disabled (see `repair_section`), and two installs over one instance would
    /// fetch the same files twice and race each other's renames.
    fn request_repair(&mut self) {
        let Some(Modal::InstanceSettings(state)) = &mut self.modal else {
            return;
        };
        if state.repair == Load::Loading {
            return;
        }
        state.repair = Load::Loading;
        // Whatever was wrong with the last save is not what the reader is asking
        // about now, and the sentence under the button is where the answer goes.
        state.error = None;
        self.installation_repair_requested = Some(state.id.clone());
    }

    /// Ask for the linked pack to be laid over the instance again, and mark the
    /// form busy.
    ///
    /// [`Shell::request_repair`]'s shape over the other action, and its reasons hold
    /// word for word: nothing runs here, the form is put into the state the button
    /// draws (`Load::Loading`) in the same frame, and a second press while the first
    /// is out is dropped because two runs would fetch the same archive twice.
    fn request_reinstall(&mut self) {
        let Some(Modal::InstanceSettings(state)) = &mut self.modal else {
            return;
        };
        if state.reinstall == Load::Loading {
            return;
        }
        state.reinstall = Load::Loading;
        state.error = None;
        self.installation_reinstall_requested = Some(state.id.clone());
    }

    /// Ask for one version of the linked pack to be laid over the instance.
    ///
    /// The version is written down with the instance, because the answer arrives
    /// after the modal may have been closed or pointed at another instance: what
    /// the sentence is about is the pair, not whichever form is up when it lands.
    /// A press while another version is being laid over is dropped, for
    /// [`Shell::request_reinstall`]'s reason.
    fn request_change_version(&mut self, version: &str) {
        let Some(Modal::InstanceSettings(state)) = &mut self.modal else {
            return;
        };
        if state.change == Load::Loading {
            return;
        }
        state.change = Load::Loading;
        state.error = None;
        self.installation_change_requested = Some((state.id.clone(), version.to_string()));
    }

    /// Write what the installation tab holds, and close the modal when the write
    /// lands. [`Shell::save_instance_settings`]'s shape over the other file, and
    /// its refusals are the store's own ([`crate::store::Store::save_instance_installation`]).
    fn save_instance_installation(&mut self) {
        let Some(Modal::InstanceSettings(state)) = &mut self.modal else {
            return;
        };
        match self.store.save_instance_installation(&state.id, &state.installation()) {
            Ok(()) => self.modal = None,
            Err(problem) => state.error = Some(problem),
        }
    }

    /// Write what the instance-settings form holds, and close it when the write
    /// lands.
    ///
    /// A refusal -- a field that is not a number, or a heap the store will not
    /// write -- stays in the form's own sentence rather than being dropped: the
    /// modal remains up with what the reader typed still in it.
    fn save_instance_settings(&mut self) {
        let Some(Modal::InstanceSettings(state)) = &mut self.modal else {
            return;
        };
        let edit = match state.edit() {
            Ok(edit) => edit,
            Err(problem) => {
                state.error = Some(problem);
                return;
            }
        };
        match self.store.save_instance_settings(&state.id, &edit) {
            Ok(()) => self.modal = None,
            Err(problem) => state.error = Some(problem),
        }
    }

    /// Open the creation dialog, on a name nobody has typed yet and the version
    /// list asked for.
    ///
    /// The list is a request rather than a field read: it is Mojang's whole
    /// version manifest, and the dialog cannot offer a version it has not been
    /// told about. The flag carries it out of here for the reason
    /// [`Shell::create_requested`] exists -- `act`'s only answer is a page's
    /// request, and this is the dialog's.
    fn open_create(&mut self) {
        self.create_name.clear();
        self.create_error = None;
        self.creating = false;
        self.versions = Load::Loading;
        self.versions_requested = true;
        self.version_query.clear();
        self.version_snapshots = false;
        self.version_choice = None;
        // The reference's own opening choice, and no build: which builds exist
        // is a question for the game version, and the version list has not
        // arrived yet. The answer's arm raises the request.
        self.create_loader = crate::catalog::LoaderKind::Fabric;
        self.build_choice = BuildChoice::default();
        self.build = None;
        self.build_query.clear();
        self.loader_builds = Load::Idle;
        self.loader_builds_for = None;
        self.loader_builds_requested = false;
        self.modal = Some(Modal::Create);
    }

    /// Take the dialog's loader to `loader`.
    ///
    /// A build is a version of one *specific* loader, so the choices in force
    /// are not answers to anything once the loader changes -- which is why the
    /// reference resets its own build choice too. Pressing the chip that is
    /// already chosen changes nothing: the row always has one chip chosen, and
    /// `never-empty: false` on the reference's row is what lets its own selection
    /// go missing, which is a dialog with no loader in it that this one does not
    /// have an arm for.
    fn choose_loader(&mut self, loader: crate::catalog::LoaderKind) {
        if self.create_loader == loader {
            return;
        }
        self.create_loader = loader;
        self.build_choice = BuildChoice::default();
        self.build = None;
        self.build_query.clear();
        self.create_error = None;
    }

    /// Ask the loader's own service which builds it published for the game
    /// version in force.
    ///
    /// The request travels as a flag for [`Shell::versions_requested`]'s reason,
    /// and the pair it was asked for is written down here so that a slow answer
    /// about a choice the user has moved off can be dropped rather than drawn.
    ///
    /// A loader that does not load mods asks nothing: vanilla has no service and
    /// no build, so its arm clears the list rather than leaving the previous
    /// loader's on screen.
    fn ask_loader_builds(&mut self) {
        self.loader_builds_for = None;
        self.loader_builds_requested = false;
        if !self.create_loader.loads_mods() {
            self.loader_builds = Load::Idle;
            return;
        }
        let Some(game) = self.chosen_version() else {
            // Nothing to ask about yet: the version list is what names the game
            // version, and its own arm is where this request is raised.
            self.loader_builds = Load::Loading;
            return;
        };
        self.loader_builds = Load::Loading;
        self.loader_builds_for = Some((self.create_loader, game));
        self.loader_builds_requested = true;
    }

    /// The loader build the dialog's chips come to.
    ///
    /// The reference's own rule, read off the list in force: *Stable* is the
    /// newest build the loader calls stable -- or the newest of all when it has
    /// published nothing stable for this game version, which is
    /// `palantir_net::engine::default_build`'s rule and the reason Quilt's list
    /// still opens on something -- *Latest* is simply the newest, and *Other* is
    /// the build the user picked.
    ///
    /// `None` while the list is still coming, or when it failed, or when it came
    /// back with nothing in it: all three are "there is no build to install",
    /// and the dialog draws the button unusable rather than making an instance
    /// other than the one it named.
    fn chosen_build(&self) -> Option<String> {
        let builds = self.loader_builds.ready()?;
        match self.build_choice {
            BuildChoice::Stable => builds
                .iter()
                .find(|build| build.stable)
                .or_else(|| builds.first())
                .map(|build| build.version.clone()),
            BuildChoice::Latest => builds.first().map(|build| build.version.clone()),
            // A build the list no longer carries is not a build: the list is the
            // answer to what exists, and a version chosen against a list that has
            // since been read again is one this dialog cannot install.
            BuildChoice::Other => self
                .build
                .clone()
                .filter(|version| builds.iter().any(|build| &build.version == version)),
        }
    }

    /// Whether the dialog's own button can make the instance the dialog names.
    ///
    /// Unusable while a create is in flight, because a second press before the
    /// answer arrives would create the instance twice.
    ///
    /// Unusable while a loader is chosen whose build list has not answered, and
    /// *usable* once it has -- including when the answer is "none" or "it
    /// failed". That last part is deliberate: the list's own sentence is drawn on
    /// the loader-version row, so a reader who can see *no versions available for
    /// this game version* has been told what choosing this loader comes to, and a
    /// button that stayed dark would leave them with a dialog they cannot leave.
    /// The one arm this does not cover is the store's own: a create with a loader
    /// and no resolved build writes a vanilla instance and returns a warning, and
    /// that warning is dropped by [`crate::store::Store::create_instance`] today.
    /// It is the honest thing to fix next, and it is not this button's doing.
    fn create_usable(&self) -> bool {
        if self.creating {
            return false;
        }
        !self.create_loader.loads_mods() || self.loader_builds.settled()
    }

    /// Whether the *Stable* chip can be pressed at all.
    ///
    /// The reference disables it when the loader has published nothing stable
    /// for this game version (`disabledItems` on its own row, with *No such
    /// versions available* as the tooltip), so the chip stays in its place and
    /// goes dim rather than disappearing from the row.
    fn stable_offered(&self) -> bool {
        match self.loader_builds.ready() {
            Some(builds) => builds.iter().any(|build| build.stable),
            // While the list is coming, nothing is known to be missing: the chip
            // is left pressable rather than dimmed and then undimmed.
            None => true,
        }
    }

    /// Open the import step of the creation flow, on what this machine holds.
    ///
    /// The scan runs here rather than in the view: it walks every launcher root
    /// the launcher knows how to read, and a walk per frame is a walk per frame.
    fn open_import(&mut self) {
        self.import_found = self.store.importable();
        self.import_error = None;
        self.importing = false;
        self.modal = Some(Modal::Import);
    }

    /// Bring one instance in, off the frame thread.
    ///
    /// An import copies a whole instance tree -- mods, worlds, configs and all --
    /// which is why it goes where a create goes: a thread, and back as a message.
    fn import(&self, source: std::path::PathBuf) -> iced::Command<Message> {
        let store = self.store.clone();
        iced::Command::perform(
            crate::store::off_thread(move || store.import_instance(&source)),
            Message::Imported,
        )
    }

    /// Create the instance the dialog names, off the frame thread.
    ///
    /// A create writes a folder, a config and a version profile, so it goes where
    /// a search goes: a thread, and back as a message. What it is for is the
    /// version the picker is on; when there is none -- a dialog whose list never
    /// arrived -- the store asks Mojang itself, which is the same answer as the
    /// picker would have opened on.
    fn create(&self) -> iced::Command<Message> {
        let store = self.store.clone();
        let name = self.create_name.clone();
        let game = self.chosen_version();
        // Read here rather than inside the worker, because the build is chosen
        // against the list in the dialog: a worker that read it again would be
        // resolving the chips against a second answer to the same question.
        let loader = self.create_loader;
        let build = self.chosen_build();
        iced::Command::perform(
            crate::store::off_thread(move || {
                store.create_instance(&name, game.as_deref(), loader, build.as_deref())
            }),
            Message::Created,
        )
    }

    /// Ask Mojang which versions exist, off the frame thread.
    ///
    /// The dialog's own request, so it is built where the dialog is: a `Loading`
    /// state was set by [`Shell::open_create`], and this is what turns it into an
    /// answer or into [`crate::store::not_implemented`]'s sentence.
    fn versions_command(&self) -> iced::Command<Message> {
        let store = self.store.clone();
        iced::Command::perform(crate::store::off_thread(move || store.versions()), Message::Versions)
    }

    /// Ask the loader's own service which builds it published for the game
    /// version in force, off the frame thread.
    ///
    /// The question travels back with the answer, because the dialog can move on
    /// while it is out: without it, a Fabric list that arrived after the Quilt
    /// chip was pressed would be drawn under the Quilt chip.
    fn loader_builds_command(&self) -> iced::Command<Message> {
        let store = self.store.clone();
        let loader = self.create_loader;
        let Some(game) = self.chosen_version() else {
            return iced::Command::none();
        };
        let asked = game.clone();
        iced::Command::perform(
            crate::store::off_thread(move || store.loader_builds(loader, &game)),
            move |builds| Message::LoaderBuilds { loader, game: asked.clone(), builds },
        )
    }

    /// Ask Mojang which versions exist, for the instance-settings modal.
    ///
    /// [`Shell::versions_command`]'s twin, and deliberately a second method: the
    /// two reads feed different states, and one reply message would have the
    /// second answer overwrite the first.
    fn installation_versions_command(&self) -> iced::Command<Message> {
        let store = self.store.clone();
        iced::Command::perform(
            crate::store::off_thread(move || store.versions()),
            Message::InstallationVersions,
        )
    }

    /// Ask the loader's own service which builds it published for one game
    /// version, for the instance-settings modal.
    ///
    /// The pair travels back with the answer, for [`Message::InstallationBuilds`]'s
    /// reason: the reader can move to another platform or game version while the
    /// request is out.
    fn installation_builds_command(
        &self,
        loader: crate::catalog::LoaderKind,
        game: &str,
    ) -> iced::Command<Message> {
        let store = self.store.clone();
        let asked = game.to_string();
        let back = asked.clone();
        iced::Command::perform(
            crate::store::off_thread(move || store.loader_builds(loader, &asked)),
            move |builds| Message::InstallationBuilds { loader, game: back.clone(), builds },
        )
    }

    /// Install one instance's own files again, off the frame thread.
    ///
    /// One command for a minute of blocking work the store owns
    /// ([`crate::store::Store::repair_instance`]), for
    /// [`Shell::installation_modpack_command`]'s reason and a bigger one: this is
    /// the longest thing this launcher does that is not a launch, and a frame spent
    /// on it is a window that stops drawing.
    fn installation_repair_command(&self, instance: &str) -> iced::Command<Message> {
        let store = self.store.clone();
        let asked = instance.to_string();
        iced::Command::perform(
            crate::store::off_thread(move || store.repair_instance(&asked)),
            Message::Repaired,
        )
    }

    /// Read the versions of one instance's linked pack that fit it, off the frame
    /// thread.
    ///
    /// One command for what the store answers with two requests (the project's
    /// versions and the instance's own installation), for
    /// [`Shell::installation_modpack_command`]'s reason: the list is one answer.
    fn installation_pack_versions_command(&self, instance: &str) -> iced::Command<Message> {
        let store = self.store.clone();
        let asked = instance.to_string();
        iced::Command::perform(
            crate::store::off_thread(move || store.pack_versions(&asked)),
            Message::PackVersions,
        )
    }

    /// Lay one instance's linked pack over it again, off the frame thread.
    ///
    /// [`Shell::installation_repair_command`]'s shape and reason: a blocking store
    /// call -- a cached archive read, an unpack and however many files the pack
    /// lists -- on a thread of its own.
    fn installation_reinstall_command(&self, instance: &str) -> iced::Command<Message> {
        let store = self.store.clone();
        let asked = instance.to_string();
        iced::Command::perform(
            crate::store::off_thread(move || store.reinstall_modpack(&asked)),
            Message::Reinstalled,
        )
    }

    /// Lay one version of one instance's linked pack over it, off the frame thread.
    fn installation_change_command(&self, instance: &str, version: &str) -> iced::Command<Message> {
        let store = self.store.clone();
        let asked = instance.to_string();
        let wanted = version.to_string();
        iced::Command::perform(
            crate::store::off_thread(move || store.change_pack_version(&asked, &wanted)),
            Message::VersionChanged,
        )
    }

    /// Ask the service to name the pack an instance was installed from.
    ///
    /// One command for what the store answers with three requests, because the
    /// card wants one answer: the *title* of the pack this instance came from.
    /// What that costs is [`crate::store::Store::linked_modpack`]'s business.
    fn installation_modpack_command(&self, project: &str) -> iced::Command<Message> {
        let store = self.store.clone();
        let asked = project.to_string();
        iced::Command::perform(
            crate::store::off_thread(move || store.linked_modpack(&asked)),
            Message::Modpack,
        )
    }

    /// Raise whatever the instance-settings modal's installation tab is owed.
    ///
    /// Called after every message that form handles rather than once when it
    /// opens, because a platform or game-version change makes a new question. The
    /// form answers what it is on ([`crate::instance_settings::State::needs_builds`])
    /// and the pair is written into it here, so the next frame cannot ask the
    /// same question twice while the answer is still out.
    fn installation_needs(&mut self) {
        let need = match &mut self.modal {
            Some(Modal::InstanceSettings(state)) => {
                if state.needs_versions() {
                    state.versions = Load::Loading;
                    Some(InstallationNeed::Versions)
                } else if let Some((loader, game)) = state.needs_builds() {
                    state.builds = Load::Loading;
                    state.builds_for = Some((loader, game.clone()));
                    Some(InstallationNeed::Builds(loader, game))
                } else if let Some(project) = state.needs_modpack() {
                    state.modpack = Load::Loading;
                    Some(InstallationNeed::Modpack(project))
                } else if let Some(instance) = state.needs_pack_versions() {
                    state.pack_versions = Load::Loading;
                    Some(InstallationNeed::PackVersions(instance))
                } else {
                    None
                }
            }
            _ => None,
        };
        match need {
            Some(InstallationNeed::Versions) => self.installation_versions_requested = true,
            Some(InstallationNeed::Builds(loader, game)) => {
                self.installation_builds_requested = Some((loader, game));
            }
            Some(InstallationNeed::Modpack(project)) => {
                self.installation_modpack_requested = Some(project);
            }
            Some(InstallationNeed::PackVersions(instance)) => {
                self.installation_pack_versions_requested = Some(instance);
            }
            None => {}
        }
    }

    /// The version the picker is on: what the user chose, or Mojang's own latest
    /// release once the list has arrived.
    ///
    /// One reading rather than two, because the heading and the create command
    /// both need it and a dialog that showed one version while creating another
    /// is the failure this exists to prevent.
    fn chosen_version(&self) -> Option<String> {
        self.version_choice
            .clone()
            .or_else(|| self.versions.ready().map(|list| list.latest_release.clone()))
    }

    /// The request the page on screen owes, if it owes one.
    fn opening_command(&mut self) -> iced::Command<Message> {
        match self.screen.opening() {
            Some(pages::Ask::Search(asked)) => self.search(asked),
            // An icon request is not *owed* by a page's arrival either: the page
            // cannot make one until a set of results has landed, and the one that
            // made it is the message that delivered them.
            Some(pages::Ask::Icons(_)) => iced::Command::none(),
            Some(pages::Ask::Tags) => self.tags(),
            Some(pages::Ask::Instance(asked)) => self.instance(asked),
            Some(pages::Ask::Project(asked)) => self.project(asked),
            Some(pages::Ask::User(asked)) => self.user(asked),
            Some(pages::Ask::Skins(asked)) => self.skins(asked),
            // A navigation, a creation and a launch are not *owed*: nothing is
            // waiting for one, and the page that owes nothing says nothing. A
            // launch in particular is a button's doing rather than a page's
            // arrival, and starting a game because a window opened on its page
            // would be a launcher that plays by itself.
            // An install is not *owed* either: nothing is waiting for one, and the
            // dialog it opens is a press away. The instance settings modal is the
            // same shape: the gear is a press, and the shell already has the
            // store to read it from.
            Some(
                pages::Ask::Open(_)
                | pages::Ask::Install(_)
                | pages::Ask::Create
                | pages::Ask::Import
                | pages::Ask::InstanceSettings(_)
                | pages::Ask::Play(_)
                | pages::Ask::Stop(_)
                | pages::Ask::Wear(_)
                | pages::Ask::AddSkin(_)
                | pages::Ask::EditSkin(_)
                | pages::Ask::Reorder(_),
            ) => iced::Command::none(),
            None => iced::Command::none(),
        }
    }

    /// Read one user's profile and bring the answer back as a page message.
    ///
    /// [`Shell::project`]'s twin, and blocking for the same reason: three requests
    /// on the frame thread would be three dropped frames, and the engine's own cache
    /// is what makes a second look at the same profile cost nothing. The name is
    /// cloned rather than borrowed because the worker outlives this call.
    fn user(&self, asked: user::Asked) -> iced::Command<Message> {
        let store = self.store.clone();
        let username = asked.user.clone();
        iced::Command::perform(
            crate::store::off_thread(move || store.user(&username)),
            move |result| Message::Screen(pages::Message::user_result(&asked, result)),
        )
    }

    /// Read the account's own appearance and bring it back as a page message.
    ///
    /// Three things in one trip, off the frame thread for the other requests'
    /// reason: which account this launcher would sign in as -- the shell's own
    /// selection, read here rather than by the page -- its game token, and then the
    /// store's two reads behind that. An account with no Microsoft session is
    /// answered with a sentence instead of a request, because an offline account has
    /// no skins service to ask: that is the reference's own gate, where a reader who
    /// is not signed into Minecraft is told to sign in rather than shown an empty
    /// gallery.
    fn skins(&self, asked: skins::Asked) -> iced::Command<Message> {
        let store = self.store.clone();
        let account = self.account();
        iced::Command::perform(
            crate::store::off_thread(move || {
                let appearance = match account.access_token.as_deref() {
                    Some(token) => store.appearance(&account.username, token),
                    None => {
                        return Err(
                            "Sign in to a Microsoft account to see the skins it owns."
                                .to_string(),
                        )
                    }
                }?;
                // The launcher's own store is read on this thread too: a row's Ears
                // marker is in its PNG, and decoding a handful of 64x64 textures is
                // not work for the thread that draws.
                Ok(skins::Loaded { appearance, saved: stored_rows(&store) })
            }),
            move |result| Message::Screen(pages::Message::skins_result(&asked, result)),
        )
    }

    /// Read one tab's own listing and bring it back as a page message.
    ///
    /// [`Shell::search`]'s twin, and blocking for [`Shell::project`]'s reason: the
    /// read is a directory walk plus a widget's worth of work per row, which on the
    /// frame thread would be a dropped frame for every entry -- the measurement in
    /// `crate::scale` is what says so. The directory is resolved here, on the frame
    /// thread, because turning an instance id into a folder is the store's own and
    /// costs nothing to ask.
    fn instance(&self, asked: instance::Asked) -> iced::Command<Message> {
        let directory = self.store.instance_dir(&asked.id);
        let tab = asked.tab.clone();
        iced::Command::perform(
            crate::store::off_thread(move || store::listing(&directory, &tab)),
            move |listing| Message::Screen(pages::Message::instance_result(&asked, listing)),
        )
    }

    /// Change what the account is wearing, and bring the outcome back as a page
    /// message.
    ///
    /// Off the frame thread for [`Shell::skins`]'s reason, and the same account and
    /// token as that read: it is a write to the reader's own Minecraft account, made
    /// through the service that owns it, so an account with no Microsoft session is
    /// answered with the same sentence the read is rather than with a request that
    /// has no token to carry. What the *change* is -- which skin, which cape, or
    /// taking one off -- is the page's to describe, and it arrives here as a value.
    fn wear(&self, worn: skins::Wear) -> iced::Command<Message> {
        let store = self.store.clone();
        let account = self.account();
        let change = worn.change.clone();
        iced::Command::perform(
            crate::store::off_thread(move || match account.access_token.as_deref() {
                Some(token) => store.wear(token, change),
                None => Err(
                    "Sign in to a Microsoft account to change what it wears.".to_string(),
                ),
            }),
            move |result| Message::Screen(pages::Message::skin_worn(&worn, result)),
        )
    }

    /// Add a skin from a file the reader picks, and bring the outcome back as a page
    /// message.
    ///
    /// The flow is split over two threads, and the split is not a preference. The
    /// dialog is opened *here*, on the frame thread, because `GetOpenFileNameW` is
    /// modal to the window it is given and this launcher's window belongs to this
    /// thread -- see [`crate::pick`], which is where that is argued and where the one
    /// untestable call in this flow lives. Reading the few kilobytes it returned and
    /// padding them to the shape the service takes happen on the same thread, since
    /// they are arithmetic over a 64x64 image and a crossing would cost more than they
    /// do. The *upload* is a request to Mojang, and that goes off the frame thread for
    /// [`Shell::wear`]'s reason, with the same account, the same token and the same
    /// sentence for an account that has neither.
    ///
    /// Both halves of the answer travel the same way, including the ones with nothing
    /// to send: a cancel, an unreadable file and a machine with no picker are all
    /// `skins::Picked` values that reach the page through one seam, and the page is
    /// what decides that one of them is not a failure.
    fn add_skin(&self, add: skins::Add) -> iced::Command<Message> {
        let choice = match crate::pick::choose(Key::AppSkinsAddButton.message()) {
            // No dialog on this machine: the page is told that instead of being left
            // waiting on a picker that will never open.
            Err(reason) => Choice::Answered(skins::Picked::NoPicker(reason)),
            Ok(None) => Choice::Answered(skins::Picked::Cancelled),
            Ok(Some(path)) => match self.read_skin(&path) {
                Err(reason) => Choice::Answered(skins::Picked::Done(Err(reason))),
                Ok(change) => Choice::Upload(change),
            },
        };
        let store = self.store.clone();
        let account = self.account();
        iced::Command::perform(
            // One future for both outcomes, so the page has one seam rather than two
            // kinds of message. Only the upload blocks, and only the upload goes to a
            // worker thread: the answered cases resolve here, and the *upload* is
            // awaited inside the same future through [`crate::store::off_thread`], which
            // is what every other request in this shell does with the same two lines.
            async move {
                match choice {
                    Choice::Answered(picked) => picked,
                    Choice::Upload(change) => {
                        // The texture is cloned for the store before the change is moved
                        // into the worker: the row the reader will see in the Saved-skins
                        // section is this same file, so the two must agree about what was
                        // added.
                        let picked = change.clone();
                        skins::Picked::Done(
                            crate::store::off_thread(move || {
                                let result = match account.access_token.as_deref() {
                                    Some(token) => store.wear(token, change),
                                    None => Err(
                                        "Sign in to a Microsoft account to add a skin."
                                            .to_string(),
                                    ),
                                };
                                if result.is_ok() {
                                    keep_picked(&store, &picked);
                                }
                                result
                            })
                            .await,
                        )
                    }
                }
            },
            move |picked| Message::Screen(pages::Message::skin_added(&add, picked)),
        )
    }

    /// The change a chosen file *is*: read it, read its arm style out of its own
    /// pixels, and pad it to the 64x64 the service takes.
    ///
    /// Its own function because it is the testable half of the flow: it takes a path,
    /// and a test can write one. The two refusals are named where they are made -- a
    /// file that cannot be read (too big included, which `pick::read` answers) and bytes
    /// that are not a texture of either shape -- and neither is the reference's own
    /// sentence: its nearest one is `app.skins.dropped-file-error.text`, which is about a
    /// file *dropped* on the page, and this flow has no drop in it. The reader's file
    /// name travels as the part's name and is reduced on the wire (`file_part_name`),
    /// because a value that came from a dialog is not one a header line can trust.
    fn read_skin(&self, path: &std::path::Path) -> Result<palantir_net::SkinChange, String> {
        let bytes = crate::pick::read(path)?;
        let prepared = crate::skin::prepare(&bytes).ok_or_else(|| {
            "That file is not a Minecraft skin: a skin is a 64x64 or 64x32 PNG.".to_string()
        })?;
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(palantir_net::SkinChange::Upload {
            variant: prepared.model.variant().to_string(),
            file_name,
            texture: prepared.png,
        })
    }

    /// Do what the Skins page's editor asked for, and bring the outcome back as a page
    /// message.
    ///
    /// Off the frame thread for [`Shell::wear`]'s reason, and the one ask that reaches
    /// two stores at once: the launcher's own ([`crate::saved_skins`]) for the row's arm
    /// style and cape, or for forgetting it, and the account's through Minecraft's
    /// service for the two actions that change what is *worn*. The account and its token
    /// are read here, the way [`Shell::skins`] reads them, so a reader who is not signed
    /// in is told to sign in rather than sent a request with no token -- for the two
    /// actions that need one. Forgetting a row is a local file and works signed out, which
    /// is why the token is read inside the arms that need it rather than once above them.
    fn edit_skin(&self, edit: skins::Edit) -> iced::Command<Message> {
        let store = self.store.clone();
        let account = self.account();
        let outcome = edit.clone();
        iced::Command::perform(
            crate::store::off_thread(move || {
                let paths = store
                    .paths()
                    .ok_or_else(|| "This launcher has no folder for saved skins.".to_string())?;
                match outcome.act {
                    // Forgetting is this launcher's own file and needs no token: a reader
                    // signed out can still tidy their store.
                    skins::Act::Forget => {
                        crate::saved_skins::forget(paths, &outcome.key)?;
                        Ok(())
                    }
                    // Taking the skin off is the one change with nothing to describe: the
                    // service's own DELETE (`SkinChange::NoSkin`), which G106 implemented
                    // and left unreachable until there was a control to reach it from.
                    skins::Act::TakeOff => match account.access_token.as_deref() {
                        Some(token) => store.wear(token, palantir_net::SkinChange::NoSkin),
                        None => Err(
                            "Sign in to a Microsoft account to change what it wears."
                                .to_string(),
                        ),
                    },
                    // Save writes the row and puts it on. The row is written first, so the
                    // two choices the editor collected are the reader's whatever the
                    // service does with the upload; the texture is read back from the
                    // store rather than carried here, because a page has never held a file
                    // and the store is where the picker's copy lives.
                    skins::Act::Save => {
                        crate::saved_skins::update(
                            paths,
                            &outcome.key,
                            &outcome.variant,
                            &outcome.cape,
                        )?;
                        let Some(entry) =
                            crate::saved_skins::load(paths).get(&outcome.key).cloned()
                        else {
                            return Err(
                                "That saved skin is no longer in the store.".to_string()
                            );
                        };
                        let texture = crate::saved_skins::texture(paths, &entry)?;
                        match account.access_token.as_deref() {
                            Some(token) => store.wear(
                                token,
                                palantir_net::SkinChange::Upload {
                                    variant: outcome.variant.clone(),
                                    // The store's own file name, which is the digest and
                                    // a `.png`: `file_part_name` reduces whatever a caller
                                    // sends, and a name this launcher wrote is not a name
                                    // from a dialog.
                                    file_name: entry.file.clone(),
                                    texture,
                                },
                            ),
                            None => Err(
                                "Sign in to a Microsoft account to change what it wears."
                                    .to_string(),
                            ),
                        }
                    }
                }
            }),
            move |result| Message::Screen(pages::Message::skin_saved(&edit, result)),
        )
    }

    /// Write the Skins page's saved rows in the order the reader put them in, and
    /// bring the outcome back as a page message.
    ///
    /// Off the frame thread for [`Shell::edit_skin`]'s reason, and the one ask in
    /// this shell that reaches nothing but this launcher's own store: no account, no
    /// token and no service, because the order is an index under the product's own
    /// directory. The whole order travels rather than a move, because that is the
    /// write -- the reference's `set_custom_skin_order` takes the list of texture
    /// keys and [`crate::saved_skins::reorder`] takes the same -- and the store
    /// ignores keys it does not hold and keeps rows it was not told about, so a page
    /// that drew a shorter list than the file holds cannot lose a row by asking.
    fn reorder_skins(&self, order: skins::Reorder) -> iced::Command<Message> {
        let store = self.store.clone();
        let outcome = order.clone();
        iced::Command::perform(
            crate::store::off_thread(move || write_skin_order(&store, &outcome.keys)),
            move |result| Message::Screen(pages::Message::skin_reordered(&order, result)),
        )
    }

    /// Read the news feed and bring the answer back as a shell message.
    ///
    /// Off the frame thread, for [`Shell::search`]'s reason: it is one blocking
    /// request, and the panel is drawn on the first frame the window has.
    fn news_command(&self) -> iced::Command<Message> {
        let store = self.store.clone();
        iced::Command::perform(crate::store::off_thread(move || store.news()), Message::News)
    }

    /// Run a search and bring the answer back as a page message.
    ///
    /// The work itself happens in [`crate::store::off_thread`], which is the
    /// crossing between a blocking engine and a toolkit that is not: awaiting the
    /// call here, on the thread that draws, would be a dropped frame for every
    /// millisecond the service took. The ceiling the engine holds is what keeps a
    /// user who types quickly from opening a connection per keystroke.
    ///
    /// The round the request was made in travels with the message, so a slow
    /// answer to a question the page has replaced is dropped by the page rather
    /// than drawn (`discover::State::update`).
    fn search(&self, asked: discover::Asked) -> iced::Command<Message> {
        let store = self.store.clone();
        // Cloned rather than borrowed: the worker takes the query, and the answer
        // still needs the round it came with.
        let query = if asked.hide_installed {
            self.without_installed(asked.query.clone())
        } else {
            asked.query.clone()
        };
        iced::Command::perform(crate::store::off_thread(move || store.search(&query)), move |result| {
            Message::Screen(pages::Message::search_result(&asked, result))
        })
    }

    /// Read the tag lists the browse sidebar's options are made of.
///
/// Blocking for the reason every other read here is: the requests are network
/// round trips and this runs on the frame thread, so it goes through
/// [`crate::store::off_thread`] like its siblings. It is asked once per page, not
/// once per search -- the answer changes when Modrinth ships a release -- and the
/// engine holds each list, so a second visit to Discover costs no request at all.
fn tags(&self) -> iced::Command<Message> {
    let store = self.store.clone();
    iced::Command::perform(
        crate::store::off_thread(move || store.tags()),
        |result| Message::Screen(pages::Message::tags(result)),
    )
}

/// The same search with the projects this launcher already has taken out.
    ///
    /// `Browse.vue`'s `instanceFilters` pushes `{ type: 'project_id', option:
    /// 'project_id:<id>', negative: true }` for every installed project while the
    /// sidebar's *Hide already installed* switch is on, and `search.ts` renders
    /// those into one `project_id NOT IN [...]` group. So this is a *request*
    /// filter rather than a row filter: the count the API answers is the count
    /// after the hiding, which dropping rows on the way to the screen could not
    /// give -- it would report the unhidden total beside a shortened list.
    ///
    /// "Installed" is the reference's own reading: the Modrinth project an
    /// instance was installed from, which is this launcher's [`store::InstanceLink`]
    /// -- read out of the small file beside the instance, so this costs a file per
    /// instance and no request at all. An instance with no link, or one whose link
    /// cannot be read, is not in the list, because there is no project id to hide
    /// by; the search is then asked unchanged rather than with an empty group,
    /// which Modrinth would answer with nothing.
    fn without_installed(&self, query: ApiSearch) -> ApiSearch {
        let mut ids: Vec<String> = Vec::new();
        if let Load::Ready(cards) = self.store.instances() {
            for card in cards {
                let Ok(Some(link)) = self.store.instance_link(&card.id) else { continue };
                if !ids.contains(&link.project_id) {
                    ids.push(link.project_id);
                }
            }
        }
        match exclusion_facet(&ids) {
            Some(group) => {
                // Pushed rather than *set*: the page's own facets (the categories
                // chosen in the sidebar) are already on the search, and a
                // replacement would drop them and answer a question nobody asked.
                let mut query = query;
                query.facets.push(group);
                query
            }
            None => query,
        }
    }

    /// Fetch and decode a page of icons, and bring them back as a page message.
    ///
    /// [`Shell::search`]'s twin, off the frame thread for a reason of its own: a
    /// search is one blocking request and this is twenty of them plus a PNG decode
    /// each, and a decode on the frame thread is a hitch with no request behind it
    /// to explain it.
    ///
    /// No request travels back beside the answer, unlike the two around it: an icon
    /// is keyed by the URL it was fetched from, so there is no round to check
    /// (`pages::Message::search_icons`).
    fn icons(&self, asked: discover::Icons) -> iced::Command<Message> {
        let store = self.store.clone();
        let urls = asked.urls;
        iced::Command::perform(
            // `Ok` because [`crate::store::off_thread`] answers with a `Result` and
            // this call has no failure of its own: `project_icons` drops the icons
            // that would not come back, so the only way to reach the arm below at all
            // is a worker that died before answering -- and the answer to that is the
            // same empty boxes as a fetch that failed (`crate::avatar` has that as a
            // named departure, not an accident).
            crate::store::off_thread(move || Ok::<_, String>(store.project_icons(&urls))),
            |arrived| {
                Message::Screen(pages::Message::search_icons(arrived.unwrap_or_default()))
            },
        )
    }

    /// Read one project and bring the answer back as a page message.
    ///
    /// [`Shell::search`]'s twin, and blocking for the same reason: three requests
    /// on the frame thread would be three dropped frames, and the engine's own
    /// cache is what makes a page revisited cost nothing.
    fn project(&self, asked: project::Asked) -> iced::Command<Message> {
        let store = self.store.clone();
        let id = asked.id.clone();
        iced::Command::perform(crate::store::off_thread(move || store.project(&id)), move |result| {
            Message::Screen(pages::Message::project_result(&asked, result))
        })
    }

    /// Install one project into one instance, and bring the outcome back as a
    /// sentence.
    ///
    /// [`Shell::project`]'s twin, blocking for the same reason and a stronger one:
    /// this one is not three document reads but a document read, a version choice
    /// and a file transfer, and the transfer is where a frame thread would sit for
    /// as long as the file takes.
    fn install(&self, project: &str, instance: &str) -> iced::Command<Message> {
        let store = self.store.clone();
        // Cloned rather than borrowed: the worker outlives this call, and both
        // names are its.
        let (project, instance) = (project.to_string(), instance.to_string());
        iced::Command::perform(
            crate::store::off_thread(move || store.install_project(&project, &instance)),
            // The value becomes the line here rather than in the page: what
            // landed, which version of it and which instance are all facts the
            // page asked for a transfer rather than for, and the sentence is
            // drawn in the same shape whether it worked or not.
            |result| Message::Installed(result.map(|installed| store::Outcome::File(installed.summary()))),
        )
    }

    /// Install one project as an instance of its own, and bring the outcome back
    /// as a sentence plus the instance it made.
    ///
    /// [`Shell::install`]'s twin, blocking for the same reason and a longer one:
    /// this downloads the pack, unpacks it and then downloads every file its index
    /// lists, which is the longest thing this launcher does that is not a launch.
    fn install_pack(&self, project: &str) -> iced::Command<Message> {
        let store = self.store.clone();
        // Cloned rather than borrowed: the worker outlives this call.
        let project = project.to_string();
        iced::Command::perform(
            crate::store::off_thread(move || store.install_pack(&project)),
            Message::Installed,
        )
    }

    // ---- Launching ------------------------------------------------------

    /// The account this launcher is signed in as, or an anonymous one.
    ///
    /// The whole entry travels rather than just the name, for the old shell's
    /// reason: a Microsoft account launches with its tokens, and renewing them is
    /// the worker's first step.
    fn account(&self) -> launch::AccountRef {
        match self.accounts.as_ref().and_then(AccountsStore::selected_account) {
            Some(account) => launch::AccountRef::from_entry(account),
            // No account file, or no account chosen: the offline session the
            // launcher has always been able to play with.
            None => launch::AccountRef::anonymous(),
        }
    }

    /// Sign this launcher's next launch in as `uuid`, and write the file.
    ///
    /// The file is the record rather than this window: the other launcher reads
    /// the same one, so an account chosen here is the account it signs in as too.
    /// A write that fails is said in the panel, because the alternative is a
    /// selection that looks made and is not.
    fn select_account(&mut self, uuid: &str) {
        let Some(accounts) = self.accounts.as_mut() else {
            return;
        };
        match accounts.select(uuid) {
            Ok(()) => self.save_accounts(),
            Err(problem) => self.accounts_note = Some(problem),
        }
    }

    /// Take this account away, and write the file.
    ///
    /// Removing the account a launch would have signed in as leaves no selection,
    /// which is the anonymous session rather than an error: the store clears it and
    /// the card's header falls back to the reference's own "Select account".
    fn remove_account(&mut self, uuid: &str) {
        let Some(accounts) = self.accounts.as_mut() else {
            return;
        };
        if accounts.remove(uuid) {
            self.save_accounts();
        }
    }

    /// Write the accounts file, leaving in the panel whatever a write could not do.
    fn save_accounts(&mut self) {
        let Some(accounts) = self.accounts.as_ref() else {
            return;
        };
        self.accounts_note = accounts.save().err();
    }

    /// Start the instance `id`, and remember the run.
    ///
    /// The run is what the subscription reads, so this is the whole of starting
    /// a game: everything the worker needs was read here, at the moment the user
    /// asked, because a worker has no business going back to a preferences file
    /// to find out what it was told to use.
    ///
    /// A press for an instance that is already running is ignored rather than
    /// queued: the reference's Play button is the Stop button by then, so the only
    /// way to send this is a stale frame. A press for a *different* instance is
    /// the thing this used to refuse and now does not: the reference runs several
    /// processes at once, and its bar is where they are listed.
    fn play(&mut self, id: String) {
        if id.trim().is_empty() || self.runs.iter().any(|run| run.data.instance_id == id) {
            return;
        }
        let Some(data_root) = self.home.as_ref().map(|home| home.root.clone()) else {
            // A shell with no data root is a test's shell: there is nowhere for an
            // instance to be, so there is nothing to launch.
            self.store.set_launch(
                &id,
                store::Launch {
                    state: store::LaunchState::Idle,
                    line: Some(store::not_implemented("Launching an instance")),
                },
            );
            return;
        };
        self.next_run_id += 1;
        let run_id = self.next_run_id;
        self.runs.push(Run {
            data: ActiveRunData {
                run_id,
                instance_id: id.clone(),
                data_root,
                account: self.account(),
                defaults: launch::LaunchDefaults::from_prefs(&self.prefs),
            },
            // Its own slot, empty until its worker starts the game: this is what
            // makes a second run's stop control the second run's.
            child: ChildSlot::default(),
        });
        self.store.set_launch(
            &id,
            store::Launch {
                // The same sentence the worker's own first line uses, because it
                // is the same fact: the run is being prepared and nothing has
                // happened yet.
                line: Some(format!("preparing '{id}'")),
                state: store::LaunchState::Starting,
            },
        );
    }

    /// Stop the run of the instance named `id`, when it is up.
    ///
    /// The kill is on the child that run's own worker put in its slot, which is
    /// the only handle to that game this process has. What comes back is the
    /// worker's own `Done`: killing a process is not the same as reaping it, and
    /// the note the run ends with is the worker's to write.
    fn stop(&mut self, id: &str) {
        // Cloned out of the list rather than borrowed through the rest of this:
        // the slot is an `Arc`, and the run it belongs to is about to be written
        // to in the store.
        let Some(child) = self
            .runs
            .iter()
            .find(|run| run.data.instance_id == id)
            .map(|run| run.child.clone())
        else {
            return;
        };
        // Nothing to kill while the launcher is still preparing: the child slot
        // is empty until the worker has resolved the version, fetched what was
        // missing and started the process. Setting *Stopping* here would be a
        // word with nothing behind it -- the run would go on to start the game
        // and then report itself running under a chip that says it is being
        // stopped -- so this is the one state where a stop is refused rather than
        // performed. It is refused by *not being drawn*, too: the action bar's own
        // control and the instance page's are both absent while a run prepares.
        if self.store.launch_state(id) != store::LaunchState::Running {
            return;
        }
        self.store.set_launch(
            id,
            store::Launch {
                state: store::LaunchState::Stopping,
                line: Some(Key::InstanceActionStopping.message().to_string()),
            },
        );
        if let Ok(mut slot) = child.lock() {
            if let Some(process) = slot.as_mut() {
                let _ = process.kill();
            }
        };
    }

    /// What a launch reported, applied to the run it belongs to.
    ///
    /// Every arm looks the run up by id first: a done for a run that has already
    /// ended must not clear one that is still going, and a line from the run
    /// before must not appear in the header of the run that replaced it.
    fn launched(&mut self, event: launch::LaunchEvent) {
        match event {
            launch::LaunchEvent::Log { run_id, lines } => {
                let Some(id) = self.instance_of(run_id) else {
                    return;
                };
                if let Some(line) = lines.last() {
                    self.say(&id, line.clone());
                }
            }
            launch::LaunchEvent::Progress { run_id, progress } => {
                let Some(id) = self.instance_of(run_id) else {
                    return;
                };
                // Kept as well as said: the line is the instance's own, and the
                // numbers are what the action bar's chip and the manager's panel
                // are drawn from.
                self.jobs.insert(id.clone(), progress.clone());
                self.say(&id, progress.status_line());
            }
            launch::LaunchEvent::Started { run_id } => {
                let Some(id) = self.instance_of(run_id) else {
                    return;
                };
                // What changes here is the control and not the line: the fact is
                // about the game's window being up, and the run's own last line
                // is still the last thing the launcher said.
                //
                // The level goes with it: fetching is over, and a bar left at
                // the last phase's fraction would be a download that never
                // finished. The panel closes only once the level that opened it is
                // gone -- with another run still fetching, its job holds it open.
                self.jobs.remove(&id);
                if self.jobs.is_empty() {
                    self.downloads = false;
                }
                self.set_run_state(&id, store::LaunchState::Running);
            }
            launch::LaunchEvent::Done { run_id, note } => {
                let Some(index) = self.runs.iter().position(|run| run.data.run_id == run_id) else {
                    return;
                };
                let run = self.runs.remove(index);
                let id = run.data.instance_id;
                if let Ok(mut slot) = run.child.lock() {
                    *slot = None;
                }
                self.jobs.remove(&id);
                if self.jobs.is_empty() {
                    self.downloads = false;
                }
                self.store.set_launch(
                    &id,
                    store::Launch {
                        state: store::LaunchState::Idle,
                        line: Some(note),
                    },
                );
                // The run wrote the play time and the last-played stamp into the
                // instance's own files, so the library the pages are drawn from is
                // stale until it is read again.
                self.store.reload();
            }
            launch::LaunchEvent::Tokens(tokens) => self.renewed(tokens),
        }
    }

    /// The instance a run id belongs to, while that run is still going.
    ///
    /// `None` is what every arm of [`Shell::launched`] checks before it touches
    /// anything: an event for a run that has ended is a fact about the past, and
    /// acting on it is how one run's stop would clear another run's state.
    fn instance_of(&self, run_id: u64) -> Option<String> {
        self.runs
            .iter()
            .find(|run| run.data.run_id == run_id)
            .map(|run| run.data.instance_id.clone())
    }

    /// Record what one run is doing, for the pages to draw.
    fn say(&mut self, id: &str, line: String) {
        let state = self.store.launch_state(id);
        self.store.set_launch(id, store::Launch { state, line: Some(line) });
    }

    /// Put one running instance in another state, keeping the line it is on.
    ///
    /// The state and the line are two facts rather than one: the game coming up
    /// changes what the button is, and the line is still the last thing the
    /// launcher said.
    fn set_run_state(&mut self, id: &str, state: store::LaunchState) {
        let line = self.store.launch_line(id).map(str::to_string);
        self.store.set_launch(id, store::Launch { state, line });
    }

    /// Write a session a launch renewed back to the account store.
    ///
    /// A renewal happens inside the worker because the launch cannot wait for the
    /// window to notice; storing it is the window's, because the file is the
    /// window's. A write that fails is left for the next launch to renew again
    /// rather than drawn in front of the user: it is the same account, still
    /// signed in, and the failure is about a file.
    fn renewed(&mut self, tokens: launch::RefreshedTokens) {
        let Some(accounts) = self.accounts.as_mut() else {
            return;
        };
        if accounts
            .update_tokens(
                &tokens.uuid,
                &tokens.access_token,
                tokens.refresh_token.as_deref(),
                tokens.expires_at_ms,
            )
            .is_ok()
        {
            let _ = accounts.save();
        }
    }

    /// Every run's subscription: one channel per run, keyed by that run's id.
    ///
    /// A run at a time was the whole of what this used to stream, and the reason
    /// it is a batch now is the same reason the shell holds a list: iced keeps
    /// subscriptions apart by key, and each run's key is its own id, which is
    /// already how the events tell themselves apart. The worker is blocking and
    /// lives on a thread of its own; what crosses back is a channel of
    /// [`launch::LaunchEvent`]s, pumped into this shell's own messages. A full
    /// window is waited for rather than dropped -- a line the launcher wrote is not
    /// a level, and a shell that never hears a run ended would go on drawing Stop
    /// for a game that is gone.
    fn launching(&self) -> Subscription<Message> {
        Subscription::batch(self.runs.iter().map(|entry| {
            let run = entry.data.clone();
            let slot = entry.child.clone();
            let id = run.run_id;
            iced::subscription::channel(id, 128, move |sender| async move {
                let params = launch::LaunchParams {
                    data_root: run.data_root.clone(),
                    instance_id: run.instance_id.clone(),
                    account: run.account.clone(),
                    run_id: run.run_id,
                    defaults: run.defaults.clone(),
                };
                let (events, mut stream) =
                    futures::channel::mpsc::channel::<launch::LaunchEvent>(128);
                let _ = std::thread::spawn(move || {
                    launch::run_launch_worker(params, slot, events);
                });
                let mut sender = sender;
                while let Some(event) = futures::StreamExt::next(&mut stream).await {
                    let mut pending = Some(Message::Launched(event));
                    while let Some(message) = pending.take() {
                        match sender.try_send(message) {
                            // Delivered, and the next event is fetched.
                            Ok(()) => {}
                            // Full: the window is a frame behind. A fact is not a
                            // frame, so the send waits for room rather than
                            // dropping it.
                            Err(error) if error.is_full() => {
                                pending = Some(error.into_inner());
                                let had_room =
                                    futures::future::poll_fn(|cx| sender.poll_ready(cx)).await;
                                if had_room.is_err() {
                                    break;
                                }
                            }
                            // The window is gone: there is nobody left to tell.
                            Err(_) => break,
                        }
                    }
                }
                loop {
                    futures::future::pending::<()>().await;
                }
            })
        }))
    }

    // ---- Drawing --------------------------------------------------------

    fn render(&self) -> Element<'_, Message> {
        // Keep the window's hit test pointed at the maximize control: the
        // rectangle is derived from this shell's own constants, so re-publishing
        // it is a store behind a lock rather than a measurement to keep in step.
        crate::native::set_caption_target(caption_target());
        let theme = self.theme;
        let chrome = move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
            ..container::Appearance::default()
        };
        let mut window = column![].width(Length::Fill).height(Length::Fill).push(
            container(self.head()).width(Length::Fill).height(Length::Fixed(BAR)).style(chrome),
        );
        // The two panels the bar opens, between the bar and the page they are
        // about, in the order the reference stacks them under its own head: the
        // popover over every running instance, then the download manager. See
        // [`Shell::run_switchers`] and [`Shell::download_panel`].
        //
        // The popover is drawn only when there is a second run to switch to --
        // the reference's own condition on the chevron that opens it -- so a
        // flag left open while a run ended cannot leave a panel with one row in
        // it, or with none.
        if self.switchers && self.store.running_launches().len() > 1 {
            window = window.push(self.run_switchers());
        }
        if self.downloads && !self.jobs.is_empty() {
            window = window.push(self.download_panel());
        }
        // The window behind everything is `--bg-raised`, the reference's own
        // root: wherever a page does not reach, the raised colour is what shows,
        // and the page's rounded corner is a cut into the chrome rather than
        // into another copy of the page. iced clears a window with its theme's
        // `background`, and that palette entry is `--bg` -- the colour unstyled
        // widgets sit on -- so the root is drawn as a chrome surface here
        // instead of reaching into the palette every widget reads.
        let window = container(window.push(row![self.rail(), self.pane()].height(Length::Fill)))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(chrome);
        // The Skins page's editor is a modal too, and it is not in `Modal`: it is the
        // page's own state (`pages::Screen::skins_edit`), so the layer is drawn for it
        // as well as for the shell's own modals.
        if self.modal.is_some() || self.screen.skins_edit().is_some() {
            // A modal covers the window rather than replacing it. The reference's
            // overlay is `position: fixed` over the page it dims, and its own
            // capture shows that: at y=350 the rail's chrome reads `(25, 34, 36)`
            // through the scrim and the page beside it `(20, 29, 32)` -- two
            // backdrops, one translucent layer. Drawing the layer *instead of*
            // the window makes every row read the same whatever was under it, and
            // measured exactly that: the bed solved to an alpha of `1.0` where
            // the reference's solves to `0.647` at y=200, `0.706` at 350 and
            // `0.824` at 600.
            //
            // iced 0.12 composites a tree in order and has no stack widget, so
            // the covering is done by the layout: two `Fill` children of a column
            // split twice the height it was given when its spacing is minus that
            // height, which puts the second child back at the first one's own
            // top. The alternative was a custom widget with `advanced`, which is
            // a hundred lines of the same arithmetic plus an event path that a
            // modal -- a thing that swallows presses -- would then have to get
            // right.
            return column![window, self.modal_layer()]
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(-self.viewport.height)
                .into();
        }
        window.into()
    }

    /// The head: the mark, the history, the breadcrumb, the panel toggle and
    /// the window controls.
    fn head(&self) -> Element<'_, Message> {
        // Bound before the style closure rather than reached for inside it: a
        // `move` closure that names `self` captures the borrow, and the element
        // it is attached to outlives the method.
        let theme = self.theme;
        let row = row![]
            .align_items(Alignment::Center)
            .padding(Padding { top: 0.0, bottom: 0.0, left: RAIL_PAD, right: 0.0 })
            .push(image(brand::logo_handle()).height(Length::Fixed(LOGO)))
            // The wordmark beside the mark, which is the reference's own head:
            // its `TextLogo` is one SVG carrying both -- the mark, then
            // "modrinth" in `text-contrast` -- so the name is part of the
            // identity the reader sees on every page, not only in the About
            // page and the title bar.
            //
            // 25px is measured rather than picked. That SVG is 593 units tall
            // and its letterforms occupy y 98..490 of it, so at the head's own
            // 28px height the reference's wordmark has a cap height of
            // 18.5px; Inter's is 0.727 em, and 18.5 / 0.727 is 25.4. The 2px
            // space is optical: the mark's own art is a 28px square whose ink
            // stops ~5.6px short of its right edge, so this lands the gap the
            // reference's SVG has between the two.
            .push(Space::with_width(2.0))
            .push(
                text(brand::APP_NAME)
                    .size(25.0)
                    .font(heading())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            // The reference's `ml-2` then a `gap-2` group of the two history
            // buttons.
            .push(Space::with_width(8.0))
            .push(self.history_button(Glyph::ChevronLeft, !self.back.is_empty(), Message::Back))
            .push(Space::with_width(8.0))
            .push(self.history_button(
                Glyph::ChevronRight,
                !self.forward.is_empty(),
                Message::Forward,
            ))
            // `Breadcrumbs.vue`'s own `pl-4`, which is the reference's whole gap
            // between the history pair and the trail -- the head's flex row puts
            // nothing between them and the trail pads itself.
            .push(Space::with_width(CRUMB_INSET))
            .push(self.breadcrumbs())
            // The reference's own order on the right of the status bar: the
            // action bar, then the panel toggle, then the window controls. The
            // action bar is what makes a run watchable from any page -- the
            // reference keeps it here rather than on the instance page for exactly
            // that reason, and this shell used to draw the run only in the header
            // of the instance it belonged to.
            .push(Space::with_width(Length::Fill))
            // The toggle comes *before* the action bar, which is the reference's
            // own order (`App.vue`'s head section puts the `IconButton` ahead of
            // the `AppActionBar` div), and it is not drawn at all on a page that
            // forces the panel: `v-if="!forceSidebar && appSettings.toggleSidebar"`.
            // On Discover, Project and User there is nothing the arrow could do
            // -- the panel is up whatever the toggle says -- so drawing one there
            // is a control that lies about what the reader can change.
            .push_maybe(self.panel_toggle_shown().then(|| self.panel_toggle()))
            // `mr-3` on the toggle, then the action bar's own.
            .push(Space::with_width(12.0))
            .push(self.action_bar())
            .push(Space::with_width(12.0));
        container(row.push(self.window_controls()))
            .width(Length::Fill)
            .height(Length::Fixed(BAR))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                ..container::Appearance::default()
            })
            .into()
    }

    /// The head's breadcrumb trail, as `Breadcrumbs.vue` draws it.
    ///
    /// One `flex shrink-0 items-center gap-1.5 whitespace-nowrap text-base
    /// font-medium leading-6` entry per crumb, each with its `size-5` visual in
    /// `text-primary`, and a `size-5 text-primary` `ChevronRightIcon` between
    /// entries in a `gap-2` row. The last entry's label is `text-contrast` and
    /// every earlier one `text-primary`, which is the reference's own rule for
    /// which of the two you are on.
    ///
    /// The reference fades the trail out at the row's own edge when it
    /// overflows (`breadcrumb-fade-mask`) and scrolls it back into view; a head
    /// that cannot scroll has one entry on every route this launcher has, so
    /// there is nothing to fade.
    fn breadcrumbs(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let trail = crumbs(&self.address, self.instance_name().as_deref());
        let last = trail.len().saturating_sub(1);
        let mut row = row![].align_items(Alignment::Center).spacing(CRUMB_GAP);
        for (index, crumb) in trail.iter().enumerate() {
            if index > 0 {
                row = row.push(icon::icon(
                    Glyph::ChevronRight,
                    CRUMB_ICON,
                    theme_gen::ink(theme, INK_DEFAULT),
                ));
            }
            let mut entry = row![].align_items(Alignment::Center).spacing(CRUMB_ENTRY_GAP);
            if let Some(glyph) = crumb.icon {
                entry = entry.push(icon::icon(glyph, CRUMB_ICON, theme_gen::ink(theme, INK_DEFAULT)));
            }
            let ink = if index == last { INK_CONTRAST } else { INK_DEFAULT };
            row = row.push(
                entry.push(
                    text(crumb.label.clone())
                        .size(CRUMB_LABEL)
                        // `leading-6` on `text-base`: `Pixels` rather than a bare
                        // number, which iced reads as a multiple of the size.
                        .line_height(iced::Pixels(CRUMB_LINE))
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, ink))),
                ),
            );
        }
        row.into()
    }

    /// The name of the instance the address is on, when the store has it.
    ///
    /// The reference's instance breadcrumb is `instance.value?.name` with a
    /// `loadingLabel` beside it while the instance is still being read, because
    /// the name is a field of the instance rather than of the address. This
    /// reads the same field, and answers `None` when the scan has not produced
    /// that card -- which is the state [`crumbs`] falls back to the id from.
    fn instance_name(&self) -> Option<String> {
        let route::Route::Instance { id, .. } = &self.address.route else {
            return None;
        };
        match self.store.instance(id) {
            Load::Ready(card) => Some(card.name.clone()),
            _ => None,
        }
    }

    /// The action bar: what this launcher is running, and how far the run is
    /// through whatever it is fetching.
    ///
    /// `AppActionBar.vue`'s right-hand cluster, in its own order: the download
    /// manager's chip first, then the one chip that says what is running. Two
    /// pieces of it are deliberately not built here, and both are absent features
    /// rather than missing pixels: the offline banner needs an online/offline
    /// source this launcher has no opinion about yet, and the update button
    /// belongs to a self-updater it does not have.
    fn action_bar(&self) -> Element<'_, Message> {
        let mut bar = row![].align_items(Alignment::Center).spacing(8.0);
        if let Some((_, progress)) = shown_job(&self.jobs, self.store.selected_launch()) {
            bar = bar.push(self.download_chip(progress));
        }
        bar.push(self.running_chip()).into()
    }

    /// The download manager's chip: the glyph, the count of jobs the bar is
    /// carrying, the phase's own label and either its percentage or the fact that
    /// it has no length yet.
    ///
    /// The bar under the label is [`crate::ui::progress`] over the phase's own
    /// fraction, so the number and the fill cannot disagree: both are read from
    /// the same `Progress`.
    fn download_chip(&self, progress: &install::Progress) -> Element<'_, Message> {
        let theme = self.theme;
        let (factor, _) = crate::ui::interaction(DOWNLOAD_CHIP);
        // The reference's `bg-brand-highlight` while its panel is open, which is
        // how a chip that toggles something says that it is open.
        let background = self
            .downloads
            .then(|| theme_gen::ink(theme, Ink::ColorBrandHighlight));
        let ink = crate::theme::brightness(theme_gen::ink(theme, INK_CONTRAST), factor);
        let mut body = row![].align_items(Alignment::Center).spacing(8.0);
        body = body.push(icon::icon(Glyph::Download, 16.0, ink));
        // The count the reference's own bar carries beside its icon, and the
        // reason the panel below is a list: the chip is about one job, and the
        // number is what keeps the other ones from being invisible while it is
        // the only thing on screen.
        body = body.push(self.job_count(self.jobs.len()));
        body = body.push(
            text(progress.label.clone())
                .size(13.0)
                .font(medium())
                .style(iced::theme::Text::Color(ink)),
        );
        body = body.push(
            text(if progress.is_indeterminate() {
                // No fraction to show, and a percentage would have to be
                // invented: the reference's own arm says "working" with a
                // spinner rather than with a number.
                "…".to_string()
            } else {
                format!("{}%", progress.percent())
            })
            .size(13.0)
            .font(semibold())
            .style(iced::theme::Text::Color(ink)),
        );
        body = body.push(
            container(crate::ui::progress(theme, progress.fraction()))
                .width(Length::Fixed(DOWNLOAD_BAR))
                .center_y(),
        );
        let chip = container(body)
            .height(Length::Fixed(BAR_CHIP))
            .padding(Padding { top: 0.0, bottom: 0.0, left: 12.0, right: 12.0 })
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: background.map(|background| {
                    Background::Color(crate::theme::brightness(background, factor))
                }),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 12.0.into(),
                },
                ..container::Appearance::default()
            });
        mouse_area(chip)
            .interaction(Interaction::Pointer)
            .on_enter(Message::hover(DOWNLOAD_CHIP, true))
            .on_exit(Message::hover(DOWNLOAD_CHIP, false))
            .on_press(Message::ToggleDownloads)
            .into()
    }

    /// The chip that says what this launcher is running, with the three controls
    /// the reference gives it: the chevron over the other running instances, stop,
    /// and go to the instance's logs.
    ///
    /// The chip is about *one* run -- the store's selected one -- and the chevron
    /// is drawn only when there is another one to switch between, which is the
    /// reference's own condition (`v-if="currentProcesses.length > 1"`).
    ///
    /// Nothing running is a chip of its own rather than no chip: the reference
    /// draws *No instances running* beside a grey dot so that the surface keeps
    /// its place and its shape, and a bar that came and went as processes started
    /// and stopped would move everything beside it.
    fn running_chip(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let selected = self.store.selected_launch().map(str::to_string);
        let launch = selected.as_deref().and_then(|id| self.store.launch(id));
        let mut body = row![].align_items(Alignment::Center).spacing(8.0);
        if let (Some(id), Some(launch)) = (selected, launch) {
            // A dot, the instance's name, and the state it is in -- which the
            // reference leaves to the instance's own header and this shell can
            // afford to say twice: a run being *stopped* looks exactly like a run
            // being started from the outside.
            body = body.push(self.indicator(Ink::Green));
            body = body.push(
                mouse_area(
                    text(instance_name(&self.store, &id))
                        .size(13.0)
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                )
                .interaction(Interaction::Pointer)
                .on_press(Message::Go(format!("/instance/{id}"))),
            );
            if let Some(state) = state_label(launch.state) {
                body = body.push(
                    text(state)
                        .size(13.0)
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
                );
            }
            // The chevron over the other running instances, which the reference
            // draws only when there is a second one: with one run there is nothing
            // to switch to, and a control that opened a list of one would be a
            // control that does nothing.
            if self.store.running_launches().len() > 1 {
                body = body.push(crate::ui::icon_button(
                    theme,
                    BAR_SWITCHERS,
                    if self.switchers { Glyph::ChevronUp } else { Glyph::ChevronDown },
                    16.0,
                    Message::ToggleRuns,
                ));
            }
            // The stop control exists when there is something to stop, which is
            // [`store::LaunchState::Running`] rather than "not idle": the kill
            // goes to the child the worker put in the slot, and while a run is
            // *preparing* there is no such child -- a control that drew itself
            // there would be a button whose press changed a word and nothing
            // else. The reference's own condition is the same one: it draws this
            // for a process it can see.
            if launch.state == store::LaunchState::Running {
                body = body.push(crate::ui::icon_button_kind(
                    theme,
                    BAR_STOP,
                    Glyph::StopCircle,
                    18.0,
                    crate::ui::Kind::Danger,
                    Message::StopRun(id.clone()),
                ));
            }
            body = body.push(crate::ui::icon_button(
                theme,
                BAR_LOGS,
                Glyph::TerminalSquare,
                18.0,
                Message::Go(format!("/instance/{id}/logs")),
            ));
        } else {
            body = body.push(self.indicator(INK_SECONDARY));
            body = body.push(
                text(Key::AppActionBarNoInstancesRunning.message())
                    .size(13.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            );
        }
        container(body)
            .height(Length::Fixed(BAR_CHIP))
            .padding(Padding { top: 0.0, bottom: 0.0, left: 12.0, right: 12.0 })
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 12.0.into(),
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// The popover over every running instance, drawn under the bar it was opened
    /// from.
    ///
    /// `AppActionBar.vue`'s `FloatingMenu`: `w-[20rem]`, one row per process, each
    /// row a `rounded-xl bg-surface-4` holding that process's name -- with a star
    /// on the selected one -- a stop control and its terminal, where pressing the
    /// row makes that process the one the chip is about. iced 0.12 has no z-order,
    /// so a panel that *floats* over the page is not something this toolkit can
    /// express: this is the same wall the version picker and the download manager
    /// hit, taken the same way -- a row under the bar, which is where a panel
    /// opened from the bar belongs. It is pushed to the right, under the chip that
    /// opened it, because that is where the reference drops it.
    fn run_switchers(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let selected = self.store.selected_launch().map(str::to_string);
        let mut rows = column![].spacing(6.0);
        for (id, launch) in self.store.running_launches() {
            rows = rows.push(self.switcher_row(id, launch.state, selected.as_deref() == Some(id)));
        }
        let panel = container(rows)
            .width(Length::Fixed(SWITCHER_PANEL))
            .padding(Padding { top: 4.0, bottom: 4.0, left: 4.0, right: 4.0 });
        container(row![Space::with_width(Length::Fill), panel])
            .width(Length::Fill)
            .padding(Padding { top: 8.0, bottom: 8.0, left: RAIL_PAD, right: CONTROLS_WIDTH })
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// One row of that popover: a running instance, its name and its two controls.
    ///
    /// The two controls are glyphs in a [`mouse_area`] rather than
    /// [`crate::ui::icon_button`]s, and the reason is the hover machinery it is
    /// built on: that is keyed by a `&'static str`, so every row would have to
    /// share one key and hovering one row's stop would light up all of them. What
    /// the reference gives them instead is `active:scale-95` -- a press transform
    /// this toolkit cannot draw at all -- and its `@click.stop` is what stops a
    /// stop from also selecting the process, which here is harmless: a press on a
    /// row's control selects the instance it names and then stops it, and the
    /// instance a user just stopped is a sensible thing for the bar to be about.
    fn switcher_row(
        &self,
        id: &str,
        state: store::LaunchState,
        selected: bool,
    ) -> Element<'_, Message> {
        let theme = self.theme;
        let mut body = row![].align_items(Alignment::Center).spacing(6.0);
        body = body.push(self.indicator(if selected { Ink::Green } else { INK_SECONDARY }));
        body = body.push(
            container(
                text(instance_name(&self.store, id))
                    .size(13.0)
                    .font(if selected { semibold() } else { medium() })
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .width(Length::Fill),
        );
        if selected {
            // `StarIcon` on the process the bar is about, so the mark moves with
            // the press that makes a row this one.
            body = body.push(icon::icon(Glyph::Star, 14.0, theme_gen::ink(theme, INK_SECONDARY)));
        }
        if let Some(label) = state_label(state) {
            body = body.push(
                text(label)
                    .size(12.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            );
        }
        // A stop control where there is something to stop, which is the chip's own
        // rule: while a run prepares there is no process in its slot, and a stop
        // for one would be a word with nothing behind it.
        if state == store::LaunchState::Running {
            body = body.push(
                mouse_area(icon::icon(Glyph::StopCircle, 16.0, theme_gen::ink(theme, Ink::Red)))
                    .interaction(Interaction::Pointer)
                    .on_press(Message::StopRun(id.to_string())),
            );
        }
        body = body.push(
            mouse_area(icon::icon(
                Glyph::TerminalSquare,
                16.0,
                theme_gen::ink(theme, INK_SECONDARY),
            ))
            .interaction(Interaction::Pointer)
            .on_press(Message::Go(format!("/instance/{id}/logs"))),
        );
        mouse_area(
            container(body)
                .width(Length::Fill)
                .padding(Padding { top: 6.0, bottom: 6.0, left: 8.0, right: 8.0 })
                .style(move |_theme: &Theme| container::Appearance {
                    background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface4))),
                    border: Border {
                        color: theme_gen::ink(theme, Ink::Surface5),
                        width: 1.0,
                        radius: 12.0.into(),
                    },
                    ..container::Appearance::default()
                }),
        )
        .interaction(Interaction::Pointer)
        .on_press(Message::SelectRun(id.to_string()))
        .into()
    }

    /// The reference's `OnlineIndicatorIcon`: a dot in the colour of the fact it
    /// carries.
    fn indicator(&self, ink: Ink) -> Element<'_, Message> {
        let theme = self.theme;
        container(Space::with_width(INDICATOR))
            .width(Length::Fixed(INDICATOR))
            .height(Length::Fixed(INDICATOR))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, ink))),
                border: Border { radius: 999.0.into(), ..Border::default() },
                ..container::Appearance::default()
            })
            .into()
    }

    /// The download manager's panel, drawn under the head while it is open.
    ///
    /// The reference floats this in a teleported popup; iced 0.12 has no
    /// z-order and this is the same wall the version picker hit, so it is drawn
    /// in the layout: a row under the bar, which is where a panel the user opened
    /// from the bar belongs.
    ///
    /// What it holds is the reference's own shape: a *Tasks* head carrying the
    /// number of jobs still going, then one row per job ([`Shell::job_row`]). A
    /// job here is a run's own phase, keyed by instance id, so the list is every
    /// run that is fetching at once rather than only the one the chip is about --
    /// two instances installing side by side are two rows, which is the whole
    /// reason `jobs` is a map and not a field. The head's count is the chip's
    /// count over the same rows, so the two cannot disagree about how many there
    /// are.
    fn download_panel(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let selected = self.store.selected_launch().map(str::to_string);
        let mut head = row![]
            .align_items(Alignment::Center)
            .spacing(8.0)
            .push(
                text("Tasks")
                    .size(14.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(self.job_count(self.jobs.len()))
            .push(Space::with_width(Length::Fill));
        head = head.push(crate::ui::icon_button(
            theme,
            BAR_PANEL_CLOSE,
            Glyph::X,
            16.0,
            Message::ToggleDownloads,
        ));
        let mut rows = column![].spacing(6.0);
        for (id, progress) in &self.jobs {
            rows = rows.push(self.job_row(id, progress, selected.as_deref() == Some(id.as_str())));
        }
        let body = column![].spacing(8.0).push(head).push(rows);
        container(body)
            .width(Length::Fill)
            .padding(Padding { top: 10.0, bottom: 10.0, left: RAIL_PAD, right: CONTROLS_WIDTH })
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// One job of that panel: the instance it belongs to, the phase's own line,
    /// the same bar the chip is drawn from, and the way to that instance's logs.
    ///
    /// The row is a [`mouse_area`] on [`Message::SelectRun`], which is exactly
    /// what that message is for: pressing a job makes the bar about the run that
    /// job belongs to, the way pressing a row of the popover above it does. The
    /// indicator is the popover's own rule as well -- green on the run the bar is
    /// about, secondary on the rest.
    ///
    /// The reference's job row carries a much larger set of controls -- pause,
    /// resume, retry, cancel, dismiss, copy details, and a finish time -- and not
    /// one of them is drawn here, because none of them has anything behind it
    /// yet: a job in this shell is a running launch's own phase, and the words a
    /// stopped phase would answer are the run's, on the chip and on the popover
    /// row. A pause for a fetch that cannot pause would be a control that lies.
    fn job_row(
        &self,
        id: &str,
        progress: &install::Progress,
        selected: bool,
    ) -> Element<'_, Message> {
        let theme = self.theme;
        let mut body = row![].align_items(Alignment::Center).spacing(8.0);
        body = body.push(self.indicator(if selected { Ink::Green } else { INK_SECONDARY }));
        body = body.push(
            container(
                column![]
                    .spacing(2.0)
                    .push(
                        text(instance_name(&self.store, id))
                            .size(13.0)
                            .font(if selected { semibold() } else { medium() })
                            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                    )
                    .push(
                        text(progress_line(progress))
                            .size(12.0)
                            .font(medium())
                            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
                    ),
            )
            .width(Length::Fill),
        );
        // The fraction again, as a bar: the same `Progress` the chip was drawn
        // from, so a panel row and a chip cannot disagree.
        body = body.push(
            container(crate::ui::progress(theme, progress.fraction()))
                .width(Length::Fixed(JOB_BAR))
                .center_y(),
        );
        body = body.push(
            mouse_area(icon::icon(
                Glyph::TerminalSquare,
                16.0,
                theme_gen::ink(theme, INK_SECONDARY),
            ))
            .interaction(Interaction::Pointer)
            .on_press(Message::Go(format!("/instance/{id}/logs"))),
        );
        mouse_area(
            container(body)
                .width(Length::Fill)
                .padding(Padding { top: 6.0, bottom: 6.0, left: 8.0, right: 8.0 })
                .style(move |_theme: &Theme| container::Appearance {
                    background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface4))),
                    border: Border {
                        color: theme_gen::ink(theme, Ink::Surface5),
                        width: 1.0,
                        radius: 12.0.into(),
                    },
                    ..container::Appearance::default()
                }),
        )
        .interaction(Interaction::Pointer)
        .on_press(Message::SelectRun(id.to_string()))
        .into()
    }

    /// The reference's count pill: how many jobs the bar is carrying, in
    /// `text-brand` on `--color-green-highlight` inside a `border-brand` ring.
    ///
    /// Drawn on the download chip and on the panel's own head from the same
    /// number, which is what the reference does with the one it puts in both
    /// places.
    fn job_count(&self, count: usize) -> Element<'_, Message> {
        let theme = self.theme;
        container(
            text(count.to_string())
                .size(12.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Brand))),
        )
        .height(Length::Fixed(20.0))
        .padding(Padding { top: 0.0, bottom: 0.0, left: 6.0, right: 6.0 })
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::GreenHighlight))),
            border: Border {
                color: theme_gen::ink(theme, Ink::Brand),
                width: 1.0,
                radius: 999.0.into(),
            },
            ..container::Appearance::default()
        })
        .into()
    }

    /// A history button: a 28px square, `!border !border-surface-4`.
    fn history_button(&self, glyph: Glyph, enabled: bool, message: Message) -> Element<'_, Message> {
        let ink = self.icon_ink(enabled);
        let border = theme_gen::ink(self.theme, Ink::Surface4);
        let face = container(icon::icon(glyph, HEAD_ICON, ink))
            .width(Length::Fixed(HEAD_BUTTON))
            .height(Length::Fixed(HEAD_BUTTON))
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                border: Border { color: border, width: 1.0, radius: HEAD_RADIUS.into() },
                ..container::Appearance::default()
            });
        if enabled {
            mouse_area(face).interaction(Interaction::Pointer).on_press(message).into()
        } else {
            // No message at all rather than a message that is ignored: the
            // reference disables the button, and a control that cannot be used
            // should not report that it was.
            mouse_area(face).into()
        }
    }

    /// The close button of the reference's `NewModal`: the `md` icon-only
    /// `IconButton`, which `IconButton.vue` builds `circular` by default -- `h-9
    /// w-9`, `!rounded-full`, `bg-surface-4` inside a 1-pixel `--surface-5`
    /// ring, with a `size-5` glyph in `text-primary`.
    ///
    /// The header is `p-6` around this button and the title at `items-center`,
    /// and the button is the taller of the two: its 36 pixels are the row's own
    /// height, which is what puts the header's rule at y=150 of a dialog whose
    /// border is at y=65 and every row of the body four pixels below where a
    /// header sized to its title alone would have them. Measured in the
    /// reference's own settings capture at x 1044..1079, y 90..125.
    fn dialog_close(&self) -> Element<'_, Message> {
        const CLOSE: &str = "modal:close";
        let theme = self.theme;
        let (factor, _) = crate::ui::interaction(CLOSE);
        let plate = crate::theme::brightness(theme_gen::ink(theme, Ink::Surface4), factor);
        let ring = crate::theme::brightness(theme_gen::ink(theme, Ink::Surface5), factor);
        let ink = crate::theme::brightness(theme_gen::ink(theme, INK_DEFAULT), factor);
        let face = container(icon::icon(Glyph::X, CONTROLS_ICON, ink))
            .width(Length::Fixed(CONTROLS_BUTTON))
            .height(Length::Fixed(CONTROLS_BUTTON))
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(plate)),
                border: Border { color: ring, width: 1.0, radius: 999.0.into() },
                ..container::Appearance::default()
            });
        mouse_area(face)
            .interaction(Interaction::Pointer)
            .on_enter(Message::hover(CLOSE, true))
            .on_exit(Message::hover(CLOSE, false))
            .on_press(Message::CloseModal)
            .into()
    }

    /// The panel toggle: `RightArrowIcon`, flipped when the panel is down,
    /// `mr-3` from the action bar.
    ///
    /// `App.vue`'s own `IconButton` beside the action bar: a `RightArrowIcon`
    /// carrying `rotate-180` while the panel is hidden, so the arrow points at
    /// the edge the panel would come back from, and a `base` face while it is
    /// shown against a `quiet` one while it is not -- which is the difference
    /// between the raised plate this draws and no plate at all.
    fn panel_toggle(&self) -> Element<'_, Message> {
        let showing = self.sidebar;
        let ink = theme_gen::ink(self.theme, if showing { INK_CONTRAST } else { INK_DEFAULT });
        let background = if showing {
            Some(Background::Color(theme_gen::ink(self.theme, INK_HOVER_BG)))
        } else {
            None
        };
        // The reference rotates the one icon it has rather than holding a
        // second, so a launcher with both arrows picks the one that points at
        // where the panel would go rather than drawing a mirror of the shape.
        let glyph = if showing { Glyph::RightArrow } else { Glyph::LeftArrow };
        let face = container(icon::icon(glyph, CONTROLS_ICON, ink))
            .width(Length::Fixed(CONTROLS_BUTTON))
            .height(Length::Fixed(CONTROLS_BUTTON))
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background,
                border: Border { radius: CONTROL_RADIUS.into(), ..Border::default() },
                ..container::Appearance::default()
            });
        mouse_area(face)
            .interaction(Interaction::Pointer)
            .on_press(Message::Sidebar(!showing))
            .into()
    }

    /// The three window controls, on their own `bg-bg-raised` surface.
    fn window_controls(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let bar = row![]
            .align_items(Alignment::Center)
            .padding(Padding { top: 0.0, bottom: 0.0, left: CONTROLS_PAD, right: CONTROLS_PAD })
            .push(self.control_button(Glyph::Minimize, Message::Minimize))
            .push(Space::with_width(CONTROLS_GAP))
            .push(self.control_button(
                if self.maximized { Glyph::Restore } else { Glyph::Maximize },
                Message::ToggleMaximize,
            ))
            .push(Space::with_width(CONTROLS_GAP))
            .push(self.control_button(Glyph::X, Message::Close));
        container(bar)
            .height(Length::Fixed(BAR))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                border: Border {
                    radius: iced::border::Radius::from([0.0, 0.0, 0.0, CONTROLS_RADIUS]),
                    ..Border::default()
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// One window control: a 36px square, `type="quiet"`.
    fn control_button(&self, glyph: Glyph, message: Message) -> Element<'_, Message> {
        let ink = theme_gen::ink(self.theme, INK_DEFAULT);
        let face = container(icon::icon(glyph, CONTROLS_ICON, ink))
            .width(Length::Fixed(CONTROLS_BUTTON))
            .height(Length::Fixed(CONTROLS_BUTTON))
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                border: Border { radius: CONTROL_RADIUS.into(), ..Border::default() },
                ..container::Appearance::default()
            });
        mouse_area(face).interaction(Interaction::Pointer).on_press(message).into()
    }

    /// The rail: the link group, the switcher's growth under it, and the foot.
    ///
    /// `App.vue`'s rail is one `flex flex-col gap-1` whose middle child is the
    /// quick-instance switcher, `flex min-h-0 flex-1 flex-col`: the switcher
    /// takes every pixel the rail has left and holds it, so the two children
    /// after it -- the settings button and the profile menu -- sit on the rail's
    /// floor rather than under the buttons. The slots and the 52px pitch are the
    /// same as before; what changed is where the height between them went.
    fn rail(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let mut items = column![].spacing(RAIL_GAP).padding(Padding {
            top: 0.0,
            right: RAIL_PAD,
            bottom: RAIL_PAD,
            left: RAIL_PAD,
        });
        for slot in self.nav_slots() {
            items = items.push(self.rail_button(slot));
        }
        items = items.push(self.switcher());
        for slot in RAIL_FOOT {
            items = items.push(self.rail_button(slot));
        }
        container(items)
            .width(Length::Fixed(RAIL))
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                ..container::Appearance::default()
            })
            .into()
    }

    /// The switcher's strip of the rail: the separator, the create button under
    /// it, and the height the rail had left.
    ///
    /// The reference's switcher is `flex min-h-0 flex-1`, and this is that claim
    /// in iced: a `Fill` child of the rail's column takes the room between the
    /// link group and the foot, so where the rail used to stack its buttons
    /// contiguously the foot now lands on the floor. With no recent instances --
    /// what a first run has -- the list above the separator is an empty wrapper
    /// that still spends its `gap-1`, and what is left to draw is the separator
    /// and the create button. At 720px that puts the rule at y=320 and the plus
    /// at 356, which is where the reference's own capture measures them.
    fn switcher(&self) -> Element<'_, Message> {
        let strip = column![]
            .spacing(RAIL_GAP)
            .align_items(Alignment::Center)
            .push(rail_separator(self.theme))
            .push(self.rail_button(Rail::CreateInstance));
        container(strip).width(Length::Fill).height(Length::Fill).into()
    }

    /// The rail's five link slots, in `App.vue`'s order, with the two
    /// conditional ones left out when the settings turn them off.
    fn nav_slots(&self) -> Vec<Rail> {
        RAIL_NAV
            .iter()
            .copied()
            .filter(|slot| match slot {
                Rail::Skins => self.skins_slot,
                Rail::Screenshots => self.screenshots_slot,
                _ => true,
            })
            .collect()
    }

    /// Every slot the rail draws, in `App.vue`'s order: the link group, then the
    /// switcher's create button, then the foot. The rail draws the three groups
    /// itself -- the switcher's strip goes between the first and the last -- but
    /// the order as a whole is the reference's, so one list can say what is
    /// drawn without walking the three call sites.
    #[cfg(test)]
    fn slots(&self) -> Vec<Rail> {
        self.nav_slots()
            .into_iter()
            .chain([Rail::CreateInstance])
            .chain(RAIL_FOOT)
            .collect()
    }

    /// One rail slot: a 48px circle, its hover or subpage background, the
    /// selection plate mid-growth, and the icon.
    fn rail_button(&self, slot: Rail) -> Element<'_, Message> {
        let theme = self.theme;
        let mark = self.mark(slot);
        let primary = mark == Some(Mark::Primary);
        let hovered = self.hovered == Some(slot);
        let plate = self.plate(slot);
        let background = if hovered || mark == Some(Mark::Subpage) {
            Some(theme_gen::ink(theme, INK_HOVER_BG))
        } else {
            None
        };
        // The profile slot is `LogInIcon class="!text-brand"` whenever there is
        // no account to show, and the `!` is doing work: the reference's
        // `hover:text-contrast` is overridden, so a pointer over it changes the
        // background and leaves the icon brand green.
        let ink = if slot == Rail::Profile {
            theme_gen::ink(theme, Ink::Brand)
        } else if primary {
            theme_gen::ink(theme, INK_PLATE_TEXT)
        } else if hovered || mark.is_some() {
            theme_gen::ink(theme, INK_CONTRAST)
        } else {
            theme_gen::ink(theme, INK_DEFAULT)
        };
        let glyph = rail_glyph(slot);
        let face = Canvas::new(RailButton {
            glyph,
            background,
            plate: if plate.value() > 0.0 {
                Some((plate.value(), Timing::NAV_PLATE_FROM_SCALE + (1.0 - Timing::NAV_PLATE_FROM_SCALE) * plate.value()))
            } else {
                None
            },
            plate_ink: theme_gen::ink(theme, INK_PLATE),
            ink,
        })
        .width(Length::Fixed(PLATE))
        .height(Length::Fixed(PLATE));
        mouse_area(face)
            .interaction(Interaction::Pointer)
            .on_enter(Message::Hover(Some(slot)))
            .on_exit(Message::Hover(None))
            .on_press(Message::Rail(slot))
            .into()
    }

    /// The page pane: the page's edge, the page's container, and the page in it.
    fn pane(&self) -> Element<'_, Message> {
        let theme = self.theme;
        // The page speaks `pages::Message` and the shell speaks its own, so the
        // one that holds the page is the one that wraps it -- which is also what
        // keeps a page from being able to navigate on its own.
        let body = container(self.screen.view(theme, &self.store).map(Message::Screen))
            .width(Length::Fill)
            .height(Length::Fill);
        // The page column's own width, which is where the pane ends and the
        // panel begins. The pane's edge stops there too: `pane_rule` and
        // `pane_shadow` both stop short of the panel.
        let panel = if self.panel_shown() { PANEL } else { 0.0 };
        let page_width = self.viewport.width - RAIL - panel;
        let elements: Vec<Element<Message>> = if self.panel_shown() {
            vec![body.into(), self.panel()]
        } else {
            vec![body.into()]
        };
        let page = container(row(elements).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Bg))),
                border: Border {
                    radius: iced::border::Radius::from([PAGE_RADIUS, 0.0, 0.0, 0.0]),
                    ..Border::default()
                },
                ..container::Appearance::default()
            });
        // The pane's edge is `.app-contents::before` (`App.vue:2755-2769`): a
        // `position: fixed` pseudo-element at `z-index: 30` carrying
        // `border-width: 1px; border-color: var(--surface-5)` and
        // `border-radius: var(--radius-xl)`. `.app-contents` itself declares no
        // border, so its content box -- and every page's -- starts at
        // `left: var(--left-bar-width); top: var(--top-bar-height)`, which is
        // (64, 48) at the reference's own 1280x720, and the rule is painted
        // *over* that content rather than beside it. Laying the page one pixel
        // in, which is what this used to do, therefore moved every page's own
        // padding down and right by one: `48 + 24 = 72` is where the
        // reference's first page ink is on both pages and `49 + 24 = 73` is
        // where this one put it.
        //
        // The rule therefore goes over the page as its own layer, and the corner
        // keeps the sliver the two rounded corners' radii show between them:
        // this container is still the pane in the rule's colour and the page is
        // still laid over all of it, so the only thing that moved is which
        // pixel each of them starts on.
        //
        // The pane's reserved scrollbar gutter goes over the page in the same
        // way, and for the same reason. See [`Shell::pane_gutter_at`] and
        // [`pane_gutter`].
        //
        // One layer is not the page's to draw: the page's own, which is the
        // hosting toast, and which goes *over* the gutter and *under* the shadow
        // and the rule. See [`Shell::page_overlay`].
        let mut layers =
            crate::pages::overlay::Stack::at(Vector::ZERO, page)
                .over(self.pane_gutter_at(), pane_gutter(theme));
        if let Some((at, overlay)) = self.page_overlay(theme, page_width) {
            // Clipped to the page column, which is `.app-viewport` and the reason
            // the toast can be drawn out here at all. The box is stated in this
            // layer's own coordinates rather than the pane's: the stack has already
            // moved it by `at`, and `Clipped` measures its box from wherever the
            // element laid itself out, so the page column -- `page_width` from the
            // pane's own top-left corner -- starts `at` back out of the toast's
            // corner. See [`Clipped`].
            let column = Rectangle {
                x: -at.x,
                y: -at.y,
                width: page_width,
                height: self.viewport.height - BAR,
            };
            layers = layers.over(at, Clipped::new(column, overlay));
        }
        container(
            // The reference's own order, which is the order one box paints its
            // own three things in: the reserved column is under the page's
            // shadow and under the rule, and the rule is over the shadow (CSS
            // draws an inset shadow over a background and under a border). The
            // page's own layer goes between the reserved column and the shadow,
            // which is the order the reference's `z-index`es give: the pane's
            // `::before` is `z-index: 30` and outranks a `z-10` toast, while the
            // reserved column stands in for what is under the page in the flow
            // and is outranked by it.
            layers
                .over(Vector::ZERO, pane_shadow(page_width, theme))
                .over(Vector::ZERO, pane_rule(theme)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface5))),
            border: Border {
                radius: iced::border::Radius::from([PAGE_RADIUS, 0.0, 0.0, 0.0]),
                ..Border::default()
            },
            ..container::Appearance::default()
        })
        .into()
    }

    /// The layer the page draws over the pane's own furniture, if it has one.
    ///
    /// One page has one, and it is the hosting page's invite toast. The reference
    /// draws that toast as a *sibling* of the `overflow-hidden` preview panel
    /// inside `ServerListEmptyPreview`'s own 400-wide `relative` root, so it
    /// escapes the panel; and it is a `position: absolute; z-index: 10` box inside
    /// `.app-viewport`, which is what clips it at the pane's edge (the box's last
    /// column is `x=984`, its fill's is `x=979`, and the pane ends at 980).
    /// Nothing inside a page can be over the pane's own layers, because
    /// they are siblings of the page rather than layers of it: the reserved gutter
    /// that hides iced's scrollbar, the rule and the inset shadow all are.
    ///
    /// So the page hands the layer back and the pane's stack puts it where the
    /// reference's `z-index`es put it, between the gutter and the shadow. What
    /// this is *not* allowed to be is a second reason to touch the gutter: the
    /// strip's five bands are the reference's own, measured at `x=970..979` on six
    /// routes, and the two answers that were tried and are wrong are recorded at
    /// [`pane_gutter`] -- deleting the strip, and painting it under the page.
    ///
    /// `page` is the page column's width, and it is passed because the toast's
    /// place in the page depends on it: the page's spacers are `FillPortion`s of
    /// what the column leaves over, so the toast's offset is a function of the
    /// width and cannot be a constant. The knowledge stays on the page side --
    /// `servers::page_overlay` is what turns the width into an offset, out of the
    /// same `MARGIN_SHARE` ratio the row itself is laid out by.
    ///
    /// **And the offset is taken back up by however far the page is scrolled**,
    /// which is the one thing that has to be undone when a layer leaves the page
    /// it was measured in. In the reference the toast is inside the content
    /// `.app-viewport` scrolls (`overflow: auto` at `App.vue:2747-2752`), so it
    /// rides up with the preview panel it is drawn against; here it is a layer of
    /// the *pane's* stack, which the scroll does not touch, so without this it
    /// would stay at `y=440` while everything around it moved and would end up 48
    /// pixels out of place at the bottom of this page's scroll -- `672 + 48 - 720`
    /// of it, the whole of what `ServerListEmpty`'s `-mb-10` row leaves to scroll.
    /// Subtracting is the direction that matters: the content moves *up* by the
    /// offset, and the toast is measured down from the pane's own top-left corner,
    /// so up is negative. The clip box the caller states is derived from the
    /// offset it is given (`Clipped`'s box is stated from where the element laid
    /// itself out), which is why the scroll has to be applied here rather than
    /// after it.
    ///
    /// The offset itself is [`crate::scroll::Glides::offset`] and what it is is
    /// written down there: the offset this shell last commanded the page's region
    /// to, which is where the region is to within the frame a command takes to
    /// land.
    fn page_overlay(&self, theme: Gen, page: f32) -> Option<(Vector, Element<'_, Message>)> {
        match &self.screen {
            pages::Screen::Servers(_) => {
                let (at, overlay) = servers::page_overlay(theme, page)?;
                let scrolled = at - Vector::new(0.0, self.glides.offset(crate::scroll::PAGE));
                Some((scrolled, overlay.map(pages::Message::Servers).map(Message::Screen)))
            }
            _ => None,
        }
    }

    /// Where the pane's reserved scrollbar gutter begins, measured from the pane's
    /// own left edge.
    ///
    /// Arithmetic rather than a measurement, and it cannot be anything else: the
    /// gutter is drawn over the page rather than laid out beside it, so nothing
    /// about it is ever reported. The pane is the window less the rail and, when
    /// it is up, the panel — the same two constants [`Shell::rail`] and
    /// [`Shell::panel`] are drawn at — so at the reference's own 1280x720 with the
    /// panel up this is `1280 - 64 - 300 - 10 = 906`, which is window `x=970`,
    /// and 970 is exactly where iced put the bar it hides.
    ///
    /// Ten and not [`GUTTER`], and the arithmetic says so: the strip starts where
    /// iced's own bar starts, because the first [`GUTTER_BAR`] columns of it are
    /// what hide that bar and are painted in the page's own background so the
    /// reference's page background measures the same on both sides of them. See
    /// [`pane_gutter`].
    ///
    /// The panel's own case is the other half of the same arithmetic and needs
    /// none of it: a bar inside the panel is drawn by the panel's region, which
    /// knows where the panel ends.
    fn pane_gutter_at(&self) -> Vector {
        let panel = if self.panel_shown() { PANEL } else { 0.0 };
        Vector::new(
            self.viewport.width - RAIL - panel - GUTTER - GUTTER_BAR,
            0.0,
        )
    }

    /// The right panel: the reference's own column, with its sections in it.
    ///
    /// `App.vue`'s `app-sidebar`: a `--right-bar-width` column under the wash,
    /// a hairline down its page edge (`border-l border-[--brand-gradient-border]`),
    /// and one scroll region inside it (`app-sidebar-scrollable`) that the
    /// sections stack in. The sections are the onboarding checklist, this card
    /// ("Playing as", `app.sidebar.playing-as`), the friends list, the fundraiser
    /// banner and the news feed. Four of those are here now -- the checklist
    /// (G102), this card (G83), the friends list in the state the reference draws
    /// for a reader with no Modrinth session (G102 too) and the news feed (G101) --
    /// in the reference's own order, with the fundraiser banner absent rather than
    /// drawn empty: its campaign is served by an endpoint this launcher does not
    /// read, and it is one of the four account surfaces dropped by decision
    /// (G118) rather than deferred -- so the news feed is the last section of the
    /// reference's column this shell is going to draw.
    /// The checklist's own rule is what decides between them, and it is drawn where
    /// the reference draws it -- the first thing in the scroll region, above the
    /// card rather than inside the block the rest of the sections sit in.
    fn panel(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let mut sections = column![].width(Length::Fill);
        if let Some(checklist) = self.checklist_section() {
            sections = sections.push(checklist);
        }
        // `#sidebar-teleport-target`, which `App.vue` puts between the checklist
        // and the sections below: Discover teleports its own `BrowseSidebar` into
        // it (`Browse.vue`'s last two lines), so this is where a page that has a
        // sidebar of its own puts it. It is drawn above the *Playing as* card and
        // the news feed rather than instead of them, because the reference's
        // target is a div in the middle of the column and not a replacement for
        // it.
        if let Some(pages::Screen::Discover(state)) = Some(&self.screen) {
            sections = sections.push(
                pages::discover::sidebar(theme, state).map(pages::Message::Discover).map(Message::Screen),
            );
        }
        // *Playing as* is `v-show="hasLoggedIntoMinecraft"`, which is the
        // checklist's second fact: a launcher with no account signed in draws the
        // steps that lead to one rather than a card about the account it has not
        // got. The card's own empty branch is still there for a store that exists
        // and holds nothing, which is the state this launcher's tests build.
        if self.logged_into_minecraft() {
            sections = sections.push(self.playing_as());
        }
        if let Some(friends) = self.friends_section() {
            sections = sections.push(friends);
        }
        // The Modrinth note is drawn here rather than under the checklist, because
        // two sections raise it: the checklist's third step while the steps are up,
        // and the friends sentence once they are not. Under both, the sentence is
        // always the one that was just pressed.
        if let Some(note) = &self.modrinth_note {
            sections = sections.push(
                container(self.panel_note_block(Key::OnboardingChecklistLoginModrinth.message(), note, MODRINTH_NOTE_DISMISS, Message::DismissModrinthNote))
                    .width(Length::Fill)
                    .padding(PANEL_SECTION_PAD),
            );
        }
        if let Some(note) = &self.accounts_note {
            sections = sections.push(
                container(self.panel_note_block(Key::MinecraftAccountSignIn.message(), note, ACCOUNTS_NOTE_DISMISS, Message::DismissAccountsNote))
                    .width(Length::Fill)
                    .padding(PANEL_SECTION_PAD),
            );
        }
        // The news section, where the reference has it: after the accounts card and
        // the two sections this launcher does not draw. Nothing at all is drawn
        // when the feed has no articles -- the reference's own `v-if` -- which is
        // also what a feed that could not be reached looks like.
        if let Some(news) = self.news_section() {
            sections = sections.push(news);
        }
        if let Some(note) = &self.link_note {
            sections = sections.push(
                container(self.panel_note_block("link", note, LINK_NOTE_DISMISS, Message::DismissLinkNote))
                    .width(Length::Fill)
                    .padding(PANEL_SECTION_PAD),
            );
        }
        // `border-l` over the wash: iced paints a `Border` on all four sides, so
        // the panel's own edge is a one-pixel column rather than a border width.
        //
        // The bar is the reference's own. `.app-sidebar-scrollable` is an
        // `overlayscrollbars` instance (`App.vue:2490-2494`, given nothing but
        // `overflow` to configure at `App.vue:940-946`), so what it draws is the
        // library's defaults: six pixels of `--color-scrollbar` on a transparent
        // track, inset two from the container's edge, ends fully rounded. The
        // panel ends at the window's edge, so this bar is the window's — which is
        // where the reference's capture puts it, at `x=1272..1277`.
        // `pb-12` on the scroll region, which is what keeps the last section
        // clear of the link above the ad -- and which belongs *inside* the
        // scrollable rather than beside it. `.app-sidebar-scrollable` carries
        // the padding itself (`App.vue:2491-2494`), so the region's own height
        // is the whole column above the ad and its content height is the
        // sections plus the reserve. That is the pair the handle is computed
        // from, so a reserve taken outside the region would take 48 pixels off
        // the numerator and none off the denominator.
        let sections = container(sections)
            .width(Length::Fill)
            .padding(Padding {
                top: 0.0,
                bottom: PANEL_PROMO_RESERVE,
                left: 0.0,
                right: 0.0,
            });
        let scroll = crate::scroll::region(crate::scroll::PANEL, sections, Message::Wheel)
            .direction(bar_direction())
            .style(bar(theme))
            .width(Length::Fill)
            .height(Length::Fill);
        let body: Element<'_, Message> = if self.promo_shown() {
            // The ad is *under* the scroll region and nothing else is: the
            // reference's `.app-sidebar` is a flex column holding
            // `app-sidebar-scrollable` (flex-grow) and `PromotionWrapper`, and
            // the ad's own two hundred and fifty pixels are the whole of what
            // the second one takes. The link and the fade are not in that flow
            // at all -- `App.vue:2554-2564` puts the link at
            // `absolute bottom-[250px] ... z-10` and `.app-sidebar::after`
            // (`App.vue:2705-2712`) puts the fade at `bottom: 250px; height:
            // 5rem` -- so they go over the column rather than under it, and the
            // scroll region above them is `672 - 250 = 422` tall where it was
            // `672 - 48 - 48 - 80 - 250 = 246`.
            crate::pages::overlay::Stack::at(
                Vector::ZERO,
                column![container(scroll).width(Length::Fill).height(Length::Fill), self.promo()]
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            // `over_control` and not `over`, because this block is the reference's
            // `z-10` link and not a picture of one: the link is `App.vue:2554`'s
            // own `<a>`, it sits over the scroll region, and a click on its words
            // is the link's. The two blank spacers around it answer nothing, so
            // naming the whole block a control costs one rectangle test per
            // pointer event and nothing else. The fade inside it stays a picture
            // -- see [`Self::promo_overlay`].
            .over_control(
                Vector::ZERO,
                // The overlay block is [`PROMO_PLATE`] tall itself, so the
                // plate's own height is reserved *under* it: the block then ends
                // where the plate begins, which is the `bottom: 250px` both the
                // link and the fade are measured from.
                column![
                    Space::with_height(Length::Fill),
                    self.promo_overlay(),
                    Space::with_height(Length::Fixed(PROMO_PLATE)),
                ]
                .width(Length::Fill)
                .height(Length::Fill),
            )
            .into()
        } else {
            scroll.into()
        };
        container(row![hairline(theme, true), body].height(Length::Fill))
            .width(Length::Fixed(PANEL))
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(wash(theme)),
                ..container::Appearance::default()
            })
            .into()
    }

    /// Whether the panel carries the reference's ad block.
    ///
    /// `App.vue`'s `showAd`: the panel is up, the reader is not a Modrinth Plus
    /// subscriber, and the app holds credentials. The third of those is the one
    /// this launcher has no reading of -- it holds no Modrinth credential and
    /// never will, so `credentials.value !== undefined` is the reference's own
    /// answer for a launcher in this shape, and the block is drawn.
    ///
    /// The Plus half is a real gate rather than a formality: `hasPlus` also puts
    /// `has-plus` on the panel, which is what removes the gradient fade above the
    /// ad (`.app-sidebar.has-plus::after { display: none }`) and the `pb-12`.
    fn promo_shown(&self) -> bool {
        self.panel_shown()
    }

    /// The panel's ad block: the 300x250 plate `PromotionWrapper` holds.
    ///
    /// `App.vue:2565` puts `PromotionWrapper` after `app-sidebar-scrollable` in
    /// `.app-sidebar`'s flex column, so its two hundred and fifty pixels are in
    /// that column's flow and nothing else is: the link above it and the fade
    /// over it are both positioned out of it ([`Self::promo_overlay`]).
    ///
    /// The image inside is not drawn. It is a remote promotional asset this
    /// launcher does not fetch, and the alternatives were a placeholder box in
    /// its place or a hole where the reference has one; a box of the right size
    /// in the right place says "there is something here" without claiming what,
    /// which is the smaller lie. The link above it is real and is drawn in full.
    fn promo(&self) -> Element<'_, Message> {
        let theme = self.theme;
        // `bg-bg`, the 300x250 box `PromotionWrapper`'s wrapper is, with nothing
        // in it. See the note above on why.
        container(Space::new(Length::Fill, PROMO_PLATE))
            .width(Length::Fill)
            .height(Length::Fixed(PROMO_PLATE))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Bg))),
                ..container::Appearance::default()
            })
            .into()
    }

    /// The ad's own two layers over it: the fade above the plate and the
    /// *Upgrade to Modrinth Plus* link over that.
    ///
    /// `App.vue:2554-2564` is the link -- `absolute bottom-[250px] w-full flex
    /// justify-center items-center gap-1 px-4 py-3 text-purple font-medium
    /// hover:underline z-10` with a `text-2xl` `ArrowBigUpDashIcon` -- and
    /// `.app-sidebar::after` (`App.vue:2705-2712`) is the fade: `bottom: 250px;
    /// left: 0; right: 0; height: 5rem` in
    /// `--brand-gradient-fade-out-color`, which is
    /// `linear-gradient(to bottom, rgba(24, 30, 31, 0), #171d1e 80%)` in dark.
    ///
    /// Both are measured against the *bottom* of the panel, which is the bottom
    /// of the plate, so both land at `y=470` in a 1280x720 window -- and the
    /// reference's own capture reads them there: the plate's flat `--surface-1`
    /// starts at y=470 in `x=1200` and the link's ink at x=1100 runs y=444..452.
    /// So the block is a [`PROMO_PLATE`]-tall spacer with the two laid over the
    /// top of it, and neither of them is in the flow that decides how tall the
    /// scroll region is.
    fn promo_overlay(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let key = "panel:promo";
        let (factor, _) = crate::ui::interaction(key);
        let ink = crate::theme::brightness(theme_gen::ink(theme, Ink::Purple), factor);
        let link = row![]
            .align_items(Alignment::Center)
            .spacing(4.0)
            .push(icon::icon(Glyph::ArrowBigUpDash, PROMO_ICON, ink))
            .push(
                text(Key::AppNavUpgradeToModrinthPlus.message())
                    .size(PROMO_LABEL)
                    .font(medium())
                    .style(iced::theme::Text::Color(ink)),
            );
        // `justify-center` over a panel whose only box model is its own 300
        // pixels. A column lays its children out at their own widths from the
        // left, so without centring the whole block would sit against the panel's
        // edge rather than in the middle of it.
        let upgrade = mouse_area(
            container(link)
                .padding(Padding {
                    top: PROMO_LINK_PAD_Y,
                    bottom: PROMO_LINK_PAD_Y,
                    left: PANEL_SECTION_PAD,
                    right: PANEL_SECTION_PAD,
                })
                .width(Length::Fill)
                .center_x(),
        )
        .interaction(Interaction::Pointer)
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false))
        .on_press(Message::OpenUrl(PROMO_PLUS_URL.to_string()));
        // The fade the reference paints over the last five rem of the wash, so
        // the scroll region's last section dissolves into the ad rather than
        // stopping at a hard edge.
        let fade = container(Space::new(Length::Fill, PROMO_FADE))
            .width(Length::Fill)
            .height(Length::Fixed(PROMO_FADE))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Gradient(gradient::Gradient::Linear(
                    gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_2))
                        .add_stop(0.0, Color::from_rgba(24.0 / 255.0, 30.0 / 255.0, 31.0 / 255.0, 0.0))
                        .add_stop(0.8, Color::from_rgba(23.0 / 255.0, 29.0 / 255.0, 30.0 / 255.0, 1.0)),
                ))),
                ..container::Appearance::default()
            });
        // The link over the fade: `z-10` against an unnumbered `::after`, so it
        // is the later of the two layers in the reference and the later of the
        // two here.
        crate::pages::overlay::Stack::at(
            Vector::ZERO,
            container(Space::with_height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fixed(PROMO_PLATE)),
        )
        .over(Vector::new(0.0, PROMO_PLATE - PROMO_FADE), fade)
        // The link is a control over a picture, so it is the one layer here that
        // takes the pointer -- and it takes it before the block below it, which
        // is the reference's `z-10` against an unnumbered `::after`. The fade
        // above it is a pseudo-element painted over a gradient and answers
        // nothing, so it stays a picture.
        .over_control(Vector::new(0.0, PROMO_PLATE - PROMO_LINK_H), upgrade)
        .into()
    }

    /// Whether Home is drawing the reference's welcome screen.
    ///
    /// The first run's page, and the only screen this shell listens for the
    /// quick-create key on. `Index.vue`'s gate is `isReady && !hasCreatedInstance`
    /// and this is the same reading as [`Self::keys`]'s, in one place so that the
    /// hint under the button and the key it names cannot come apart: Home, no
    /// dialog over it, and nothing to play yet.
    fn welcome_shown(&self) -> bool {
        self.address.route == route::Route::Home
            && self.modal.is_none()
            && pages::home::first_run(self.store.instances())
    }

    /// The welcome screen's quick-create key: `n`.
    ///
    /// Built only while that screen is up, which is how the reference's
    /// `event.target` guard is kept: its listener is a window-wide `keydown` and
    /// stands down when the event came from an `INPUT`, a `TEXTAREA`, a `SELECT`
    /// or anything editable. iced hands a subscription the key and the modifiers
    /// and nothing about who had the focus, so the guard here is the screen
    /// instead -- the one screen in this shell with no text field on it -- plus
    /// [`Self::welcome_shown`]'s "no dialog", which is where every text field this
    /// shell can draw over Home lives.
    ///
    /// [`iced::keyboard::on_key_press`] takes a function pointer rather than a
    /// closure, so the guard cannot live inside it: it decides whether there is a
    /// subscription at all.
    fn quick_create(&self) -> Subscription<Message> {
        if !self.welcome_shown() {
            return Subscription::none();
        }
        iced::keyboard::on_key_press(|key, modifiers| {
            quick_create_press(&key, modifiers).then_some(Message::OpenCreate)
        })
    }

    /// Whether this launcher holds an account a launch would sign in as.
    ///
    /// The checklist's `has_logged_into_minecraft`, and the *Playing as*
    /// section's own gate: the reference asks a plugin for the flag and this
    /// launcher asks the file both it and the other launcher write.
    fn logged_into_minecraft(&self) -> bool {
        self.accounts.as_ref().is_some_and(|store| !store.list().is_empty())
    }

    /// The checklist as this launcher's facts have it.
    ///
    /// `has_created_instance` is the library list the shell already holds -- so a
    /// created, imported or pack-made instance all count, which is what the
    /// reference's flag means -- and `has_logged_into_minecraft` is an account in
    /// the file. The third fact is Modrinth's and is `false` here: this launcher
    /// has no Modrinth sign-in at all, so the step is outstanding rather than
    /// quietly ticked, and `crate::checklist` is where that reading is written
    /// down.
    fn checklist(&self) -> Checklist {
        Checklist::of(
            matches!(self.store.instances(), Load::Ready(cards) if !cards.is_empty()),
            self.logged_into_minecraft(),
            false,
        )
    }

    /// The panel's getting-started checklist, or nothing when every step is done.
    ///
    /// `onboarding-checklist/index.vue`, drawn where `App.vue` puts it: the first
    /// thing in the panel's scroll region, above the *Playing as* card and outside
    /// the block the other sections stack in, under a `border-b` and `px-3 p-4`.
    /// The accordion is the reference's `open-by-default` one, and its header is
    /// the `button-class` the checklist hands it -- `border-button-border
    /// bg-button-bg` at `rounded-2xl`, with the section's title (`Getting
    /// started`) and the chevron the panel's other accordion draws, `hover:
    /// brightness-110` over the whole row -- with the body under it at `p-3`,
    /// `border-surface-5` and `rounded-b-2xl`.
    ///
    /// The section is drawn only while [`Checklist::show`] says so, which is where
    /// this launcher's reading of the plugin's own `show_checklist` lives.
    fn checklist_section(&self) -> Option<Element<'_, Message>> {
        let theme = self.theme;
        let checklist = self.checklist();
        if !checklist.show() {
            return None;
        }
        // `rounded-2xl` is 1rem, which is the theme's own `--radius-lg`.
        let radius = theme_gen::span(theme_gen::Span::RadiusLg);
        // `rounded-t-2xl`, and `rounded-b-2xl` back while the body is away: a closed
        // checklist's header is a pill, which is the reference's own
        // `collapsedCornersVisible` class. Worked out here rather than inside the
        // closure that paints it, because that closure has to be `'static` and
        // reading the shell's own field in it would borrow the shell.
        let header_radius = if self.checklist_open {
            iced::border::Radius::from([radius, radius, 0.0, 0.0])
        } else {
            iced::border::Radius::from(radius)
        };
        let (factor, _) = crate::ui::interaction(CHECKLIST_HEADER);
        let header_ink = crate::theme::brightness(theme_gen::ink(theme, INK_CONTRAST), factor);
        let header = mouse_area(
            container(
                row![]
                    .width(Length::Fill)
                    .spacing(CARD_ROW_GAP)
                    .align_items(Alignment::Center)
                    .push(
                        text(Key::OnboardingChecklistTitle.message())
                            .size(PANEL_HEADING)
                            .font(semibold())
                            .style(iced::theme::Text::Color(header_ink)),
                    )
                    // The Accordion rotates its `DropdownIcon` when it opens;
                    // iced cannot rotate a glyph, so the two chevrons are drawn,
                    // as they are on the accounts card.
                    .push(icon::icon(
                        if self.checklist_open { Glyph::ChevronUp } else { Glyph::ChevronDown },
                        CARD_MARK,
                        header_ink,
                    )),
            )
            .width(Length::Fill)
            .padding(Padding {
                top: CHECKLIST_PAD,
                bottom: CHECKLIST_PAD,
                left: CHECKLIST_PAD,
                right: CHECKLIST_PAD,
            })
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(crate::theme::brightness(
                    theme_gen::ink(theme, Ink::ButtonBg),
                    factor,
                ))),
                border: Border {
                    color: theme_gen::ink(theme, Ink::ButtonBorder),
                    width: 1.0,
                    radius: header_radius,
                },
                ..container::Appearance::default()
            }),
        )
        .interaction(Interaction::Pointer)
        .on_enter(card_crossing(CHECKLIST_HEADER, true))
        .on_exit(card_crossing(CHECKLIST_HEADER, false))
        .on_press(Message::ToggleChecklist);

        let mut section = column![].width(Length::Fill).push(header);
        if self.checklist_open {
            let mut rows = column![].width(Length::Fill).spacing(CARD_TOP);
            for step in Step::ALL {
                rows = rows.push(self.checklist_step(step, checklist.complete(step)));
            }
            section = section.push(
                container(rows)
                    .width(Length::Fill)
                    .padding(Padding {
                        top: CHECKLIST_PAD,
                        bottom: CHECKLIST_PAD,
                        left: CHECKLIST_PAD,
                        right: CHECKLIST_PAD,
                    })
                    .style(move |_theme: &Theme| container::Appearance {
                        background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
                        border: Border {
                            color: theme_gen::ink(theme, Ink::Surface5),
                            width: 1.0,
                            radius: iced::border::Radius::from([0.0, 0.0, radius, radius]),
                        },
                        ..container::Appearance::default()
                    }),
            );
        }
        Some(
            column![]
                .width(Length::Fill)
                .push(
                    container(section).width(Length::Fill).padding(Padding {
                        top: PANEL_SECTION_PAD,
                        bottom: PANEL_SECTION_PAD,
                        // `px-3`: the checklist's own horizontal padding is 12
                        // rather than the panel sections' 16, which is the
                        // reference's `px-3 p-4`.
                        left: CHECKLIST_PAD,
                        right: CHECKLIST_PAD,
                    }),
                )
                .push(hairline(theme, false))
                .into(),
        )
    }

    /// The panel's friends section: what the reference draws for a reader with no
    /// Modrinth session, which is the only state this launcher will ever draw it in
    /// -- the signed-in half is four `plugin:friends` calls behind an account
    /// (G118), and G102's sentence is the whole of the section here.
    ///
    /// `App.vue` draws `FriendsList` `v-show="showFriendsList"`, and what that
    /// component holds with no credentials is the reference's own sentence --
    /// `friends.sign-in-to-add-friends`, whose `<link>` slot is the sign-in and the
    /// rest of which says what it is for -- under the section's own `p-4
    /// border-b`. No heading at that point, in the reference as here: its "Friends"
    /// heading is inside that component's own `v-if="userCredentials"`.
    ///
    /// One departure, and it is the toolkit's rather than a choice: iced 0.12's
    /// `text` is a single run, so the link's words cannot be drawn in the accent
    /// while the sentence around them is not -- there is no inline span in this
    /// version, and no `rich_text` widget either. The sentence is therefore one
    /// pressable paragraph rather than a sentence with a link inside it, which is
    /// the same press over a wider target rather than a second control.
    fn friends_section(&self) -> Option<Element<'_, Message>> {
        let theme = self.theme;
        if !self.checklist().friends_visible() {
            return None;
        }
        let sentence = Key::FriendsSignInToAddFriends.message();
        // The slot's own words with the markup taken out, because the generated
        // table keeps the tags verbatim: a panel that drew them would be the one
        // thing a reader would report.
        let plain = match crate::text::tagged(sentence, "link") {
            Some((before, slot, after)) => format!("{before}{slot}{after}"),
            None => sentence.to_string(),
        };
        Some(
            column![]
                .width(Length::Fill)
                .push(
                    container(
                        mouse_area(crate::ui::paragraph(theme, &plain))
                            .interaction(Interaction::Pointer)
                            // The same press the checklist's third step makes, up to
                            // and including the sentence it draws.
                            .on_press(Message::Checklist(Step::LoginModrinth)),
                    )
                    .width(Length::Fill)
                    .padding(PANEL_SECTION_PAD),
                )
                .push(hairline(theme, false))
                .into(),
        )
    }

    /// One step of the checklist: the reference's own row.
    ///
    /// `h-10 rounded-xl border-button-border bg-button-bg px-4`, the label at
    /// `font-medium`, and the picture on the left -- a filled accent circle with a
    /// check when the step is done (`bg-primary size-[18px]`, `size-3` check) and
    /// a radio-button glyph when it is not. A finished row is the reference's own
    /// `:disabled` state: `opacity-50` over the whole row, the label in
    /// `text-secondary`, and no press, so the row says what it is rather than
    /// being a control that looks live and does nothing.
    ///
    /// Two departures, both the toolkit's rather than a choice. iced has no
    /// strikethrough, so the finished label is the faded secondary ink without the
    /// line through it; and the reference's `shadow-[0_1px_0.5px_rgb(0_0_0_/_12%)]`
    /// is not painted, for the reason `reference_tokens.rs` gives -- a shadow is
    /// not a token this port paints with.
    fn checklist_step<'a>(&'a self, step: Step, complete: bool) -> Element<'a, Message> {
        let theme = self.theme;
        let key = crate::ui::scoped("panel:checklist", &format!("{step:?}"));
        let (factor, _) = if complete { (1.0, 0.0) } else { crate::ui::interaction(key) };
        // `move` because the frame's own style closure is `'static` and this is
        // called from inside it; both captured values are `Copy`.
        let fade = move |color: Color| {
            let color = crate::theme::brightness(color, factor);
            if complete {
                crate::style::at_opacity(color, STEP_DONE_OPACITY)
            } else {
                color
            }
        };
        let mark: Element<'a, Message> = if complete {
            container(icon::icon(
                Glyph::Check,
                STEP_CHECK,
                theme_gen::ink(theme, Ink::AccentContrast),
            ))
            .width(Length::Fixed(STEP_MARK_CIRCLE))
            .height(Length::Fixed(STEP_MARK_CIRCLE))
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Brand))),
                border: Border {
                    radius: (STEP_MARK_CIRCLE / 2.0).into(),
                    ..Border::default()
                },
                ..container::Appearance::default()
            })
            .into()
        } else {
            // Already an `Element`: this arm's `into()` was a conversion of the
            // same type to itself, which clippy's `useless_conversion` caught.
            icon::icon(
                Glyph::RadioButton,
                crate::ui::CONTROL_ICON,
                fade(theme_gen::ink(theme, INK_CONTRAST)),
            )
        };
        let label = text(step.label().message())
            .size(14.0)
            .font(medium())
            .style(iced::theme::Text::Color(fade(theme_gen::ink(theme, if complete {
                INK_SECONDARY
            } else {
                INK_CONTRAST
            }))));
        let row = container(
            row![]
                .width(Length::Fill)
                .spacing(CARD_ROW_GAP)
                .align_items(Alignment::Center)
                .push(mark)
                .push(label),
        )
        .width(Length::Fill)
        .height(Length::Fixed(crate::ui::CONTROL))
        .padding(Padding {
            top: 0.0,
            bottom: 0.0,
            left: STEP_SIDE,
            right: STEP_SIDE,
        })
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(fade(theme_gen::ink(theme, Ink::ButtonBg)))),
            border: Border {
                color: fade(theme_gen::ink(theme, Ink::ButtonBorder)),
                width: 1.0,
                radius: crate::ui::CONTROL_RADIUS.into(),
            },
            ..container::Appearance::default()
        });
        let area = mouse_area(row)
            .interaction(Interaction::Pointer)
            .on_enter(Message::hover(key, true))
            .on_exit(Message::hover(key, false));
        match complete {
            true => area.into(),
            false => area.on_press(Message::Checklist(step)).into(),
        }
    }

    /// The panel's first card: what a launch would sign in as.
    ///
    /// `App.vue` draws it `p-4` under a `border-b`, and only when
    /// `hasLoggedIntoMinecraft`. G83 drew it always, because that flag is the
    /// onboarding checklist's and the checklist had not landed; the flag is the
    /// checklist's second fact now (`crate::checklist`), so the section is drawn
    /// when there is an account and not otherwise -- see [`Shell::panel`] for the
    /// gate, which is that flag read as the fact this launcher holds.
    fn playing_as(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let section = column![]
            .width(Length::Fill)
            .push(
                container(
                    column![]
                        .width(Length::Fill)
                        .push(
                            text(Key::AppSidebarPlayingAs.message())
                                .size(PANEL_HEADING)
                                .font(medium())
                                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
                        )
                        // `mt-2` is the card's own, and the card carries it.
                        .push(container(self.accounts_card()).padding(Padding {
                            top: CARD_TOP,
                            bottom: 0.0,
                            left: 0.0,
                            right: 0.0,
                        })),
                )
                .width(Length::Fill)
                .padding(PANEL_SECTION_PAD),
            )
            .push(hairline(theme, false));
        section.into()
    }

    /// What one of the card's controls could not do, with the control that reads
    /// it -- [`crate::page::notice`]'s shape, drawn where the press was made.
    fn panel_note_block(
        &self,
        header: &str,
        note: &str,
        dismiss_key: &'static str,
        dismiss: Message,
    ) -> Element<'_, Message> {
        let theme = self.theme;
        row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Start)
            .push(crate::ui::admonition(
                theme,
                crate::ui::Severity::Info,
                header,
                note,
            ))
            .push(Space::with_width(Length::Fill))
            .push(crate::ui::icon_button(theme, dismiss_key, Glyph::X, 16.0, dismiss))
            .into()
    }

    /// The panel's news section: the newest articles, and the way to the rest.
    ///
    /// `App.vue` draws it as a `p-4` column with the reference's own heading
    /// (`app.news.title`), one `NewsArticleCard` per article and a `ButtonLink` to
    /// the news page. Three things are this launcher's rather than the reference's:
    ///
    /// * **Four articles**, which is the reference's own slice of the feed
    ///   (`articles.slice(0, 4)`), so the section is a fixed height in the panel
    ///   rather than as long as the feed happens to be.
    /// * **No thumbnail.** The card's first element is its image, and this launcher
    ///   has no way to draw a remote one yet: the image widget it has takes bytes
    ///   it was handed, and nothing here fetches a picture at all. The title, the
    ///   summary and the date are drawn, and the picture is named as missing rather
    ///   than faked with an empty frame.
    /// * **A card whose link is not openable is not drawn.** A press hands the URL
    ///   to the operating system, so a card that could not be opened would be a
    ///   control that does nothing -- which is the one thing this shell refuses.
    fn news_section(&self) -> Option<Element<'_, Message>> {
        let theme = self.theme;
        let shown = self.news_shown();
        if shown.is_empty() {
            return None;
        }
        let mut cards = column![].width(Length::Fill).spacing(CARD_TOP);
        for article in shown {
            cards = cards.push(self.news_card(article));
        }
        // The reference's own *View all* is a `ButtonLink` at `size="xl"` with
        // its `NewspaperIcon` in front (`App.vue`'s news block), which is the
        // 48-pixel row with a 24-pixel icon.
        cards = cards.push(crate::ui::button_with_icon_sized(
            theme,
            NEWS_VIEW_ALL_KEY,
            Glyph::Newspaper,
            Key::AppNewsViewAll,
            crate::ui::Kind::Colored,
            crate::ui::Size::Xl,
            Length::Fill,
            Some(Message::OpenUrl(NEWS_PAGE_URL.to_string())),
        ));
        Some(
            column![]
                .width(Length::Fill)
                .push(
                    container(
                        column![]
                            .width(Length::Fill)
                            .spacing(CARD_TOP)
                            .push(
                                text(Key::AppNewsTitle.message())
                                    .size(PANEL_HEADING)
                                    .font(medium())
                                    .style(iced::theme::Text::Color(theme_gen::ink(
                                        theme,
                                        INK_DEFAULT,
                                    ))),
                            )
                            .push(cards),
                    )
                    .width(Length::Fill)
                    .padding(PANEL_SECTION_PAD),
                )
                .into(),
        )
    }

    /// The articles the panel draws: the feed's newest four, minus the ones a card
    /// cannot be built from.
    ///
    /// A separate function from the drawing because what is *shown* is the part
    /// worth asserting: the panel draws a fixed number of cards whatever the feed
    /// holds, and an article with no title or a link this launcher will not open is
    /// not one of them.
    fn news_shown(&self) -> Vec<&NewsArticle> {
        let Load::Ready(articles) = &self.news else {
            return Vec::new();
        };
        articles
            .iter()
            .filter(|article| {
                !article.title.is_empty() && crate::open::is_openable(&article.link)
            })
            .take(MAX_NEWS)
            .collect()
    }

    /// One news article, as a pressable card.
    ///
    /// `NewsArticleCard.vue`: the title, the summary when there is one, and the
    /// date at the foot under `mt-auto` -- and the whole card is the link, which
    /// is what `AutoLink` is. The hover brightens it, the reference's own
    /// `hover:brightness-125` on the card rather than a tint of this launcher's.
    fn news_card<'a>(&'a self, article: &'a NewsArticle) -> Element<'a, Message> {
        let theme = self.theme;
        let key = crate::ui::scoped("panel:news", &article.link);
        let (factor, _) = crate::ui::interaction(key);
        let mut body = column![]
            .width(Length::Fill)
            .spacing(4.0)
            .push(
                text(article.title.clone())
                    .size(14.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            );
        if !article.summary.is_empty() {
            body = body.push(crate::ui::paragraph(theme, &article.summary));
        }
        body = body.push(
            text(article.date_label())
                .size(12.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        );
        let link = article.link.clone();
        crate::ui::card_at(
            theme,
            factor,
            mouse_area(body)
                .interaction(Interaction::Pointer)
                .on_enter(Message::hover_with(
                    key,
                    true,
                    crate::theme::INSTANCE_CARD_HOVER_BRIGHTNESS,
                ))
                .on_exit(Message::hover_with(
                    key,
                    false,
                    crate::theme::INSTANCE_CARD_HOVER_BRIGHTNESS,
                ))
                .on_press(Message::OpenUrl(link)),
        )
    }

    /// `AccountsCard.vue`: the launcher's accounts, as the panel shows them.
    ///
    /// Two states, which are the reference's own two branches: with no accounts,
    /// a sentence and the sign-in button; with accounts, an accordion whose header
    /// names the account a launch would sign in as and whose body lists them.
    ///
    /// The card's player heads are not drawn. The reference puts a 36px head in
    /// the header and a 24px one on every row, from the skin service or from its
    /// own Steve asset for an offline account, and this launcher has no head
    /// renderer yet -- the Skins page is a placeholder for the same reason -- so a
    /// row is its radio mark and its name. `GATES.md` G83 records it.
    fn accounts_card(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let accounts: &[AccountEntry] = match &self.accounts {
            Some(store) => store.list(),
            None => &[],
        };
        if accounts.is_empty() {
            let body = column![]
                .width(Length::Fill)
                .spacing(CARD_STACK_GAP)
                .push(
                    text(Key::MinecraftAccountNotSignedIn.message())
                        .size(14.0)
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
                )
                // `AccountsCard.vue`'s empty state: `<Button type="colored"
                // color="brand">` with a `LogInIcon` -- no `size`, so the frame's
                // own default `md` at a 20-pixel icon.
                .push(crate::ui::button_with_icon_sized(
                    theme,
                    ACCOUNTS_SIGN_IN,
                    Glyph::LogIn,
                    Key::MinecraftAccountSignIn,
                    crate::ui::Kind::Colored,
                    crate::ui::Size::Md,
                    Length::Shrink,
                    Some(Message::SignIn),
                ));
            return card_frame(theme, CARD_FRAME_PAD, body);
        }
        let selected = self.accounts.as_ref().and_then(AccountsStore::selected_uuid);
        let title = card_title(accounts, selected);
        let (factor, _) = crate::ui::interaction(ACCOUNTS_HEADER);
        let header = container(
            row![]
                .width(Length::Fill)
                .spacing(CARD_ROW_GAP)
                .align_items(Alignment::Center)
                .push(
                    column![]
                        .width(Length::Fill)
                        .push(
                            text(title)
                                .size(14.0)
                                .font(medium())
                                .style(iced::theme::Text::Color(crate::theme::brightness(
                                    theme_gen::ink(theme, INK_CONTRAST),
                                    factor,
                                ))),
                        )
                        .push(
                            text(Key::MinecraftAccountLabel.message())
                                .size(CARD_LABEL)
                                .font(medium())
                                .style(iced::theme::Text::Color(crate::theme::brightness(
                                    theme_gen::ink(theme, INK_SECONDARY),
                                    factor,
                                ))),
                        ),
                )
                // The Accordion rotates its `DropdownIcon` a half turn when it
                // opens (`class="rotate-180"`). iced cannot rotate a glyph, and
                // the two chevrons are the same picture drawn twice.
                .push(icon::icon(
                    if self.accounts_open { Glyph::ChevronUp } else { Glyph::ChevronDown },
                    CARD_MARK,
                    crate::theme::brightness(theme_gen::ink(theme, INK_CONTRAST), factor),
                )),
        )
        .width(Length::Fill)
        .padding(Padding {
            top: CARD_HEAD_PAD,
            bottom: CARD_HEAD_PAD,
            left: CARD_HEAD_SIDE,
            right: CARD_HEAD_SIDE,
        });
        let header = mouse_area(header)
            .interaction(Interaction::Pointer)
            .on_enter(card_crossing(ACCOUNTS_HEADER, true))
            .on_exit(card_crossing(ACCOUNTS_HEADER, false))
            .on_press(Message::ToggleAccounts);
        let mut card = column![].width(Length::Fill).push(header);
        if self.accounts_open {
            card = card.push(self.accounts_body(accounts, selected));
        }
        card_frame(theme, 0.0, card)
    }

    /// The card's body, open: one row per account, then the add-account button.
    ///
    /// `AccountsCard.vue`'s own arrangement -- a `border-t border-surface-5`
    /// hairline under the header, `pt-1 pb-2` on the body, and the reference's
    /// `flex flex-col gap-2 px-2 pt-2` around the button at its foot.
    fn accounts_body<'a>(
        &'a self,
        accounts: &'a [AccountEntry],
        selected: Option<&'a str>,
    ) -> Element<'a, Message> {
        let theme = self.theme;
        let mut rows = column![]
            .width(Length::Fill)
            .padding(Padding { top: 4.0, bottom: 8.0, left: 0.0, right: 0.0 })
            .push(hairline(theme, false));
        for account in accounts {
            let chosen = selected == Some(account.uuid.as_str());
            let key = crate::ui::scoped("shell:accounts:row", &account.uuid);
            let (factor, _) = crate::ui::interaction(key);
            let name = row![]
                .width(Length::Fill)
                .spacing(CARD_ROW_GAP)
                .align_items(Alignment::Center)
                .push(icon::icon(
                    if chosen { Glyph::RadioButtonChecked } else { Glyph::RadioButton },
                    CARD_MARK,
                    crate::theme::brightness(
                        theme_gen::ink(theme, if chosen { Ink::Brand } else { INK_SECONDARY }),
                        factor,
                    ),
                ))
                .push(
                    text(account.username.clone())
                        .size(14.0)
                        .font(if chosen { semibold() } else { medium() })
                        .style(iced::theme::Text::Color(crate::theme::brightness(
                            theme_gen::ink(theme, if chosen { INK_CONTRAST } else { INK_DEFAULT }),
                            factor,
                        ))),
                );
            let name = mouse_area(
                container(name).width(Length::Fill).padding(CARD_ROW_PAD),
            )
            .interaction(Interaction::Pointer)
            .on_enter(card_crossing(key, true))
            .on_exit(card_crossing(key, false))
            .on_press(Message::SelectAccount(account.uuid.clone()));
            rows = rows.push(
                row![]
                    .width(Length::Fill)
                    .spacing(4.0)
                    .align_items(Alignment::Center)
                    .push(name)
                    .push(crate::ui::icon_button_kind(
                        theme,
                        crate::ui::scoped("shell:accounts:remove", &account.uuid),
                        Glyph::Trash,
                        16.0,
                        crate::ui::Kind::Danger,
                        Message::RemoveAccount(account.uuid.clone()),
                    )),
            );
        }
        rows = rows.push(
            container(crate::ui::button_with_icon_sized(
                theme,
                ACCOUNTS_ADD,
                Glyph::Plus,
                Key::MinecraftAccountAddAccount,
                crate::ui::Kind::Standard,
                crate::ui::Size::Md,
                Length::Fill,
                Some(Message::SignIn),
            ))
            .width(Length::Fill)
            .padding(CARD_ROW_PAD),
        );
        rows.into()
    }

    /// The themes Settings offers, by the reference's own rule.
    ///
    /// `AppearanceSettings.vue` filters its list on `appSettings.devMode` and on
    /// the theme in force: retro is behind dev mode until it is the theme already
    /// chosen. `ColorTheme::options` owns that rule, and this launcher has no dev
    /// mode to hand it, so what the pane offers is the rule read once, with
    /// `false`.
    /// The Appearance pane: `layouts/shared/appearance-settings/layout.vue`.
    ///
    /// Its first two sections, which are the ones the reference draws in the pane
    /// at 1280x720: the colour-theme selector -- `repeat(auto-fit, minmax(12rem,
    /// 1fr))`, two 275-pixel columns of preview cards here -- and the *sync theme
    /// across devices* row under it at `mt-6`. What the reference's sections
    /// continue with (`Project list layouts`, the sidebar preferences, *Advanced
    /// rendering*) is recorded in `NOTES.md` rather than drawn: each is its own
    /// component with its own measurements, and none of them is on screen here.
    fn appearance_settings(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let heading = column![]
            .width(Length::Fill)
            .spacing(THEME_HEADING_LEAD)
            .push(
                text(Key::SettingsDisplayThemeTitle.message())
                    .size(THEME_TITLE_SIZE)
                    .line_height(iced::Pixels(THEME_TITLE_LINE))
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Contrast))),
            )
            .push(
                text(Key::SettingsDisplayThemeDescription.message())
                    .size(THEME_BODY_SIZE)
                    .line_height(iced::Pixels(THEME_BODY_LINE))
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Secondary))),
            );
        column![]
            .width(Length::Fill)
            .push(heading)
            .push(Space::with_height(Length::Fixed(THEME_HEADING_GAP)))
            .push(self.theme_cards())
            .push(Space::with_height(Length::Fixed(THEME_SECTION_GAP)))
            .push(self.sync_theme_row())
            .into()
    }

    /// The theme grid: the reference's `repeat(auto-fit, minmax(12rem, 1fr))` with
    /// `--gap-lg` between cards.
    ///
    /// iced lays a row out and never wraps one, so the columns are counted here by
    /// the grid's own rule -- `(available + gap) / (12rem + gap)`, floored, never
    /// below one -- and a short last row is padded with blanks, which is how a grid
    /// keeps its tracks' widths when it runs out of cards.
    fn theme_cards(&self) -> Element<'_, Message> {
        let options = self.themes_offered();
        let columns = theme_columns(self.settings_content_width());
        let mut grid = column![].width(Length::Fill).spacing(THEME_GAP);
        for chunk in options.chunks(columns) {
            let mut row = row![].width(Length::Fill).spacing(THEME_GAP);
            for option in chunk {
                row = row.push(self.theme_card(*option));
            }
            for _ in chunk.len()..columns {
                row = row.push(Space::with_width(Length::Fill));
            }
            grid = grid.push(row);
        }
        grid.into()
    }

    /// One theme-preview card: `appearance-settings-theme-selector.vue`'s button.
    ///
    /// The plate is the reference's `button-base` -- `--color-button-bg` behind a
    /// 1-pixel `--color-divider` hairline, `--radius-md` -- and the mock inside it is
    /// painted in the look the option *means*, not the one in force: `.preview`
    /// carries the option's own mode class, so every colour comes from the generated
    /// table read at that look's column. `selected` is the reference's own two
    /// changes (the label to `--color-contrast`, the radio to `--color-brand`) and
    /// the pointer dims the whole card to its `brightness(0.85)`.
    fn theme_card(&self, option: ColorTheme) -> Element<'_, Message> {
        let theme = self.theme;
        let selected = self.prefs.theme() == option;
        // The crossing is the card's own: `button-base`'s hover is a `filter` over
        // the whole plate, so the tween's end is scoped here rather than left at the
        // kit's default brightening.
        let key = crate::ui::scoped("settings:theme", option.id());
        let (factor, _) = crate::ui::interaction(key);
        // `move`, so the closure owns its factor and is `Copy`: each of the styles
        // below takes a copy of it rather than borrowing the one here.
        let dim = move |colour: Color| crate::theme::brightness(colour, factor);
        let skin = preview_colors(option, crate::native::system_prefers_light());
        let card_line = dim(theme_gen::ink(theme, Ink::Divider));
        let card_bg = dim(theme_gen::ink(theme, Ink::ButtonBg));
        // `.preview .example-card`: `1rem` of padding inside a 1-pixel `--surface-4`
        // border. This kit draws a border inside the box rather than around it, so
        // the padding carries the reference's border as well -- one pixel each side --
        // which is what puts the mock's own contents where the capture has them.
        let inset = THEME_MOCK_PAD + 1.0;
        let bar1 = theme_bar(dim(skin.bar1), Length::Fill);
        // `w-[60%]` is a portion pair rather than a measurement: 60 against what is
        // left, which is what the reference's own percentage does to a row this kit
        // fills. iced's portions are whole numbers, and 60/40 is the exact pair.
        let (portion, rest) = (THEME_BAR2_SHARE * 100.0, (1.0 - THEME_BAR2_SHARE) * 100.0);
        let bar2 = row![
            theme_bar(dim(skin.bar2), Length::FillPortion(portion as u16)),
            Space::new(Length::FillPortion(rest as u16), Length::Fixed(THEME_BAR_HEIGHT)),
        ]
        .width(Length::Fill)
        .height(Length::Fixed(THEME_BAR_HEIGHT));
        let mock = container(
            row![
                container(Space::new(
                    Length::Fixed(THEME_MOCK_ICON),
                    Length::Fixed(THEME_MOCK_ICON),
                ))
                .style(move |_theme: &Theme| container::Appearance {
                    background: Some(Background::Color(dim(skin.line))),
                    border: Border { radius: THEME_MOCK_RADIUS.into(), ..Border::default() },
                    ..container::Appearance::default()
                }),
                column![]
                    .width(Length::Fill)
                    .push(bar1)
                    .push(Space::with_height(Length::Fixed(
                        THEME_BAR2_TOP - THEME_BAR_HEIGHT,
                    )))
                    .push(bar2),
            ]
            .align_items(Alignment::Start)
            .spacing(THEME_MOCK_GAP),
        )
        .width(Length::Fill)
        .height(Length::Fixed(THEME_MOCK_HEIGHT))
        .padding(Padding::from(inset))
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(dim(skin.card))),
            border: Border {
                color: dim(skin.line),
                width: 1.0,
                radius: THEME_MOCK_RADIUS.into(),
            },
            ..container::Appearance::default()
        });
        let preview = container(mock)
            .width(Length::Fill)
            .height(Length::Fixed(THEME_PREVIEW_HEIGHT))
            .padding(Padding::from(THEME_PREVIEW_PAD))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(dim(skin.bg))),
                border: Border {
                    // Top corners rounded, bottom square: the card's own corners are
                    // the only rounded ones the reference has, and this band's lower
                    // two sit against the label under it.
                    radius: [THEME_CARD_RADIUS - 1.0, THEME_CARD_RADIUS - 1.0, 0.0, 0.0].into(),
                    ..Border::default()
                },
                ..container::Appearance::default()
            });
        // `.preview-radio .label`: `padding: 0.75rem 1rem`, the radio with its
        // `margin-right: 0.5rem`, the name, and the `theme-icon` the reference adds
        // after it -- a sun on the light card, a moon on the dark theme it would
        // return to, and nothing on the others.
        let mut label = row![]
            .align_items(Alignment::Center)
            .push(crate::icon::icon(
                if selected { Glyph::RadioButtonChecked } else { Glyph::RadioButton },
                THEME_LABEL_ICON,
                dim(if selected {
                    theme_gen::ink(theme, Ink::Brand)
                } else {
                    theme_gen::ink(theme, Ink::Base)
                }),
            ))
            .push(Space::with_width(Length::Fixed(THEME_MOCK_GAP)))
            .push(
                // Left unconstrained, so the name is measured as one line and never
                // wraps into a second one over the card's foot. The reference's card
                // is `overflow: hidden`, so a name longer than the room it has --
                // Czech's `Synchronizovat se systémem` measures 234 against 197 -- is
                // clipped at the card's edge there; this renderer has no clip a text
                // can ask for, so the name overflows instead, and the gate that walks
                // the 32 locales reports the worst of them.
                text(option.label_key().message())
                    .size(THEME_BODY_SIZE)
                    // The reference's tab-row line: `text-base` on an 18-pixel box,
                    // which is what the card's 42-pixel label row wraps.
                    .line_height(iced::Pixels(THEME_LABEL_LINE))
                    .font(medium())
                    .style(iced::theme::Text::Color(dim(if selected {
                        theme_gen::ink(theme, Ink::Contrast)
                    } else {
                        theme_gen::ink(theme, Ink::Base)
                    }))),
            );
        if option == ColorTheme::Light || option == self.preferred_dark_theme() {
            label = label.push(Space::with_width(Length::Fixed(THEME_ICON_GAP))).push(
                crate::icon::icon(
                    if option == ColorTheme::Light { Glyph::Sun } else { Glyph::Moon },
                    THEME_LABEL_ICON,
                    dim(theme_gen::ink(theme, Ink::Secondary)),
                ),
            );
        }
        let label = container(label)
            .width(Length::Fill)
            .height(Length::Fixed(THEME_LABEL_HEIGHT))
            .padding(Padding {
                top: 0.0,
                bottom: 0.0,
                left: THEME_GAP,
                right: THEME_GAP,
            })
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(card_bg)),
                ..container::Appearance::default()
            });
        let card = container(
            container(column![preview, label])
                .width(Length::Fill)
                .height(Length::Fixed(THEME_CARD_HEIGHT - 2.0))
                .style(move |_theme: &Theme| container::Appearance {
                    background: Some(Background::Color(card_bg)),
                    border: Border {
                        radius: (THEME_CARD_RADIUS - 1.0).into(),
                        ..Border::default()
                    },
                    ..container::Appearance::default()
                }),
        )
        .width(Length::Fill)
        .height(Length::Fixed(THEME_CARD_HEIGHT))
        .padding(Padding::from(1.0))
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(card_line)),
            border: Border { radius: THEME_CARD_RADIUS.into(), ..Border::default() },
            ..container::Appearance::default()
        });
        mouse_area(card)
            .interaction(Interaction::Pointer)
            .on_enter(Message::hover_with(key, true, THEME_CARD_HOVER))
            .on_exit(Message::hover_with(key, false, THEME_CARD_HOVER))
            .on_press(Message::ColorTheme(option))
            .into()
    }

    /// The *sync theme across devices* row: `AppearanceSettingRow.vue` with the
    /// toggle `AppearanceSettings.vue` disables while nobody is signed in.
    ///
    /// `flex items-center justify-between gap-4`: a heading at `text-lg` and its
    /// description at `mt-1` on the left, the control on the right.
    fn sync_theme_row(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let left = column![]
            .width(Length::Fill)
            .spacing(THEME_HEADING_LEAD)
            .push(
                text(Key::SettingsDisplayThemeSyncAcrossDevices.message())
                    .size(THEME_ROW_TITLE_SIZE)
                    .line_height(iced::Pixels(THEME_TITLE_LINE))
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Contrast))),
            )
            .push(
                text(Key::SettingsDisplayThemeSyncAcrossDevicesDescription.message())
                    .size(THEME_BODY_SIZE)
                    .line_height(iced::Pixels(THEME_BODY_LINE))
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Secondary))),
            );
        row![left, crate::ui::disabled_switch::<Message>(theme)]
            .align_items(Alignment::Center)
            .spacing(THEME_GAP)
            .width(Length::Fill)
            .into()
    }

    /// The dark theme the reference marks with a moon: `preferredDarkTheme`, which
    /// is the theme in force when it is one of the dark ones and the stored dark
    /// preference otherwise. This launcher leaves that preference at the
    /// reference's own default.
    fn preferred_dark_theme(&self) -> ColorTheme {
        let current = self.prefs.theme();
        if current.is_dark() {
            current
        } else {
            ColorTheme::Dark
        }
    }

    /// The width a tab's content gets: the dialog less its two border pixels, the
    /// body's own `p-6` on the left, the tab column with its divider, and the pane's
    /// `px-6` on both sides -- 566 at the window both clients were captured in, which
    /// is the two 275-pixel theme cards and the 16 between them.
    fn settings_content_width(&self) -> f32 {
        let dialog = SETTINGS_WIDTH.min(self.viewport.width - 160.0).max(320.0);
        (dialog - 2.0 - SETTINGS_BODY_PAD - SETTINGS_NAV - 2.0 * SETTINGS_BODY_PAD).max(0.0)
    }

    /// How tall the tab in view is, where that is arithmetic this file knows.
    ///
    /// The reference shows its bottom fade when the pane's scroll container can
    /// scroll, which is a reading (`scrollHeight > clientHeight`) a view does not
    /// have; the one pane whose height this file can account for says it instead.
    /// `None` is "no fade", which is what a pane this does not know has always
    /// drawn.
    fn settings_tab_height(&self) -> Option<f32> {
        match self.settings_tab {
            SettingsTab::Appearance => Some(appearance_settings_height(self.themes_offered().len(), theme_columns(self.settings_content_width()))),
            SettingsTab::Language => None,
        }
    }

    /// The themes this pane offers, by the reference's rule, read once.
    ///
    /// One list, so that a chip and the label its row was broken for are the same
    /// position in two lists rather than two readings of the same rule that could
    /// drift apart.
    fn themes_offered(&self) -> Vec<ColorTheme> {
        ColorTheme::options(false, self.prefs.theme())
    }

    /// The labels the theme cards offer, in the reference's own order.
    ///
    /// `#[cfg(test)]`: a card names itself straight from [`ColorTheme::label_key`],
    /// and this list exists for the localization gate, which measures every
    /// language's names against the room a card has for one.
    #[cfg(test)]
    fn theme_labels(&self) -> Vec<&'static str> {
        self.themes_offered().iter().map(|option| option.label_key().message()).collect()
    }

    /// The languages Settings offers, as the reference's own language page draws
    /// them.
    ///
    /// `language-settings/index.vue` and its selector, in that order and in the
    /// reference's own arithmetic: a `text-xl` heading, the language warning as an
    /// `Admonition` with nothing but a body in it, the description under it, a
    /// search field, and then the list -- a category heading over one
    /// `CheckCircleButton` row per language.
    ///
    /// Two readings are recorded rather than imitated. The rows search on a Fuse
    /// index with a 0.4 threshold over a display name, a translated name and the
    /// locale's own search terms, which is fuzzy matching; this is a
    /// case-insensitive containment test over the same three strings, because fuzzy
    /// matching is a scoring algorithm and a launcher should not carry one for a
    /// list of 32. And the reference's Crowdin link inside the description is drawn
    /// here as the words it names -- the markup is stripped, the way the
    /// checklist's own link is -- because the sentence's only press in the
    /// reference opens a translator in a browser, which is not this launcher's.
    fn language_options(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let platform = Key::SettingsLanguagePlatformApp.message();
        let warning = crate::text_gen::settings_language_warning(platform);
        let description = crate::text_gen::settings_language_description(platform);
        let plain = match crate::text::tagged(&description, "crowdin-link") {
            Some((before, slot, after)) => format!("{before}{slot}{after}"),
            None => description.to_string(),
        };
        let query = self.locale_query.trim().to_lowercase();
        let matches = self.language_matches(&query);
        let mut pane = column![]
            .width(Length::Fill)
            // `<h2 class="m-0 text-xl font-semibold text-contrast">`: 20 pixels on
            // that class's own 28-pixel line.
            .push(
                text(Key::SettingsLanguageTitle.message())
                    .size(20.0)
                    // `text-xl`'s own line, 1.75rem, on a 20-pixel heading.
                    .line_height(iced::Pixels(28.0))
                    .font(heading())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            // `class="mb-4 mt-2"` on the admonition: eight above, sixteen under.
            .push(Space::new(Length::Fill, Length::Fixed(8.0)))
            .push(crate::ui::admonition_body(theme, crate::ui::Severity::Warning, &warning))
            .push(Space::new(Length::Fill, Length::Fixed(16.0)))
            // `<p class="m-0 mb-4 text-secondary">`, which inherits the body line.
            .push(
                text(plain)
                    .size(16.0)
                    .line_height(iced::Pixels(THEME_BODY_LINE))
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            )
            .push(Space::new(Length::Fill, Length::Fixed(16.0)))
            .push(crate::ui::input_sized(
                theme,
                crate::ui::InputSize::Standard,
                &Key::SettingsLanguageLanguagesSearchFieldPlaceholder.message(),
                &self.locale_query,
                Message::LocaleSearch,
            ));
        // The list itself, `flex flex-col gap-1`: the heading, and then either the
        // no-results box or one row per language, four pixels between every pair
        // of its children. The heading is a child whatever the list holds -- the
        // reference's template puts it above the `v-if` -- so a search that
        // matches nothing still says which category matched nothing.
        //
        // `<strong class="pt-3 pb-1 font-semibold text-contrast">`: twelve above,
        // four under, and the category's own name.
        let category = if query.is_empty() {
            Key::SettingsLanguageCategoriesDefault
        } else {
            Key::SettingsLanguageCategoriesSearchResult
        };
        pane = pane.push(
            container(
                text(category.message())
                    .size(16.0)
                    .line_height(iced::Pixels(THEME_BODY_LINE))
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(
                        theme,
                        INK_CONTRAST,
                    ))),
            )
            .width(Length::Fill)
            .padding(Padding { top: 12.0, bottom: 4.0, left: 0.0, right: 0.0 }),
        );
        if matches.is_empty() {
            // `<div class="p-4 text-secondary">No languages match your search.</div>`
            pane = pane
                .push(Space::new(Length::Fill, Length::Fixed(4.0)))
                .push(
                    container(
                        text(Key::SettingsLanguageLanguagesSearchNoResults.message().to_string())
                            .size(16.0)
                            .line_height(iced::Pixels(THEME_BODY_LINE))
                            .font(medium())
                            .style(iced::theme::Text::Color(theme_gen::ink(
                                theme,
                                INK_SECONDARY,
                            ))),
                    )
                    .padding(Padding::from(16.0)),
                );
        } else {
            // The first row's plate lands on 467 with this four spent and on 463
            // without it: `gap-1` is between the heading and the first row too.
            let current = crate::locale::tag();
            for &tag in matches.iter() {
                pane = pane.push(Space::new(Length::Fill, Length::Fixed(4.0)));
                let name = crate::locale::label(tag);
                let translated = crate::locale::translated_label(tag);
                let coverage = crate::locale::coverage(tag).map(|percent| format!("{percent}%"));
                pane = pane.push(crate::ui::check_row(
                    theme,
                    crate::ui::scoped("settings:locale", tag),
                    tag == current,
                    &name,
                    translated.as_deref(),
                    coverage.as_deref(),
                    Message::Locale(tag),
                ));
            }
        }
        pane.into()
    }

    /// The languages the search field leaves standing, in the reference's order.
    ///
    /// The order is [`crate::locale::offered_by_coverage`]'s rather than
    /// [`crate::locale::OFFERED`]'s: the reference's selector sorts what it
    /// builds by coverage before it draws any of it, so the pane opens on the
    /// language the interface ships in and every language under it is one the
    /// interface mostly speaks.
    ///
    /// An empty query is every language: the reference's selector draws the whole
    /// list until the field says otherwise, and its `Fuse` index is only built
    /// once there is a query to run against it.
    fn language_matches(&self, query: &str) -> Vec<&'static str> {
        crate::locale::offered_by_coverage()
            .into_iter()
            .filter(|tag| {
                if query.is_empty() {
                    return true;
                }
                crate::locale::label(tag).to_lowercase().contains(query)
                    || crate::locale::translated_label(tag)
                        .is_some_and(|translated| translated.to_lowercase().contains(query))
                    || tag.to_lowercase().contains(query)
            })
            .collect()
    }

    /// The languages this pane offers, named the reference's way.
    ///
    /// A list of labels rather than a row of controls, so that the gate below can
    /// measure every language's own name against the room a row has for it. The
    /// order is [`crate::locale::OFFERED`]'s, which is the reference's.
    #[cfg(test)]
    fn language_labels() -> Vec<String> {
        crate::locale::OFFERED.iter().map(|&tag| crate::locale::label(tag)).collect()
    }

    /// How tall a dialog's body may be in the window this shell is drawing in.
    ///
    /// The window's height less the margin a centered dialog keeps at both ends,
    /// its own padding and its head -- and no shorter than [`DIALOG_BODY_MIN`],
    /// because a window smaller than this shell's arithmetic would otherwise give
    /// a body a peephole rather than a list.
    fn dialog_body_room(&self) -> f32 {
        (self.viewport.height - 2.0 * DIALOG_MARGIN - 2.0 * DIALOG_PAD - DIALOG_HEAD)
            .max(DIALOG_BODY_MIN)
    }

    /// The dialog's frame: the same width, padding, surface and close button
    /// whichever modal it holds.
    fn dialog<'a>(
        &'a self,
        title: Key,
        body: Element<'a, Message>,
    ) -> Element<'a, Message> {
        self.dialog_titled(title.message(), body)
    }

    /// The same frame for a title this shell built rather than a key: the
    /// instance-settings modal names the instance it is about.
    fn dialog_titled<'a>(&'a self, title: &str, body: Element<'a, Message>) -> Element<'a, Message> {
        let theme = self.theme;
        container(
            column![]
                .spacing(12.0)
                .push(
                    row![]
                        .align_items(Alignment::Center)
                        .push(
                            text(title.to_string())
                                .size(20.0)
                                .font(heading())
                                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                        )
                        .push(Space::with_width(Length::Fill))
                        .push(self.history_button(Glyph::X, true, Message::CloseModal)),
                )
                // The body scrolls when it is taller than the window leaves room
                // for, and does not otherwise: a `max_height` on the wrapper is
                // what turns the tree below it into a viewport -- iced clamps a
                // `Shrink` height to the limits it was given, and a scrollable
                // draws its content clipped to its own bounds -- so a dialog whose
                // content fits is drawn at its content's height with no bar, and
                // one that does not is drawn at the room it has and scrolls.
                .push(
                    // [`DIALOG_INNER`] rather than `Fill`: the width the body's
                    // content is broken for has to be the width the body is
                    // *given*, or the bar a scrollable draws over its right edge
                    // sits on the last chip of a row. See
                    // [`DIALOG_SCROLLBAR`].
                    //
                    // And the bar is six wide with two clear pixels either side,
                    // which is exactly the ten the body is broken for — so the
                    // reservation [`DIALOG_SCROLLBAR`] documents is now the
                    // reservation rather than an allowance for the wrong number.
                    container(crate::scroll::region(
                        crate::scroll::DIALOG,
                        body,
                        Message::Wheel,
                    )
                    .direction(bar_direction())
                    .style(bar(theme)))
                    .width(Length::Fixed(DIALOG_INNER))
                    .max_height(self.dialog_body_room()),
                ),
        )
        .width(Length::Fixed(DIALOG_WIDTH))
        .padding(DIALOG_PAD)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
            border: Border { radius: 16.0.into(), ..Border::default() },
            ..container::Appearance::default()
        })
        .into()
    }

    /// Settings: the reference's own tabbed modal.
    ///
    /// `AppSettingsModal.vue` is a `TabbedModal` inside a `NewModal`, and both are
    /// taken at their own numbers rather than approximated. The modal is
    /// `min(928px, calc(95vw - 10rem))` wide; its header is `p-6` around a
    /// `text-2xl font-semibold` title with a 1-pixel `--surface-5` rule under it and
    /// the close button at its right. The body is a two-column grid -- the first track
    /// `minmax(12.5rem, 18rem)`, which is [`SETTINGS_NAV`] at this window, behind a
    /// 1-pixel divider, and the second the tab's own content -- with a floor of
    /// `min(65vh, 600px)` on the content. Measured against the reference's own settings
    /// dialog open at 1280x720: 928 wide as a border box (x 176..1103, its surface
    /// 177..1102) and 591 tall (y 65..655, its surface 66..654 = 589 rows), the
    /// divider's column at x=488, and the title's ink at y 98..115. This port's box
    /// is one row shorter on purpose -- [`Shell::tabbed_dialog`] has the arithmetic
    /// -- and every content row inside it is the reference's.
    fn settings_dialog(&self) -> Element<'_, Message> {
        let theme = self.theme;
        // `flex h-full flex-col gap-1`: the tab column's own four-pixel gap.
        let mut nav = column![].width(Length::Fill).spacing(4.0);
        let mut category = None;
        for tab in SettingsTab::ALL {
            if category != Some(tab.category()) {
                category = Some(tab.category());
                // The reference's heading is `text-xs font-bold uppercase
                // tracking-wide`: the uppercasing is CSS's text transform and is done
                // here, the tracking is not available (iced 0.12's text carries no
                // letter spacing) and is recorded in `NOTES.md`.
                nav = nav.push(crate::ui::nav_heading(
                    theme,
                    &tab.category().message().to_uppercase(),
                ));
            }
            nav = nav.push(crate::ui::nav_item(
                theme,
                tab.key(),
                tab.glyph(),
                tab.label().message(),
                tab.badge().map(|key| key.message()),
                self.settings_tab == tab,
                Message::SettingsTab(tab),
            ));
        }
        // `min-h-[min(65vh,600px)]` on the content, capped by the room the window
        // leaves: the reference's content pane grows with its content and scrolls once
        // the modal reaches the window's edge, which is this same clamp.
        let pane = settings_content_min(self.viewport).min(self.dialog_body_room());
        // The tab column, `flex min-w-0 max-h-[min(65vh,600px)] flex-col border-0
        // border-r-[1px] border-solid border-divider pr-4`: its own 16 pixels of
        // padding are inside the 288-pixel track, so the divider's single pixel is
        // the track's last one -- x=488 of a dialog at 1280x720, which is the column
        // the capture shows, with the selected plate spanning 201..471.
        let nav_column = container(nav)
            .width(Length::Fixed(SETTINGS_NAV - 1.0))
            .height(Length::Fixed(pane))
            .padding(Padding {
                top: 0.0,
                bottom: 0.0,
                left: 0.0,
                right: SETTINGS_NAV_PAD,
            });
        let divider = container(Space::new(Length::Fixed(1.0), Length::Fixed(pane))).style(
            move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Divider))),
                ..container::Appearance::default()
            },
        );
        // The content pane: `absolute inset-0 overflow-y-auto px-6`, and the fade
        // over its last 64 pixels when the tab it is showing is taller than the pane.
        let pane_content = crate::scroll::region(
            crate::scroll::DIALOG,
            self.settings_tab_view(),
            Message::Wheel,
        )
        .style(crate::scroll::no_bar());
        let pane_height = pane;
        let pane_scrolled = self.settings_tab_height().map_or(false, |height| height > pane_height);
        let pane_content: Element<'_, Message> = if pane_scrolled {
            // Two children in a cell: the scroll region, then the fade drawn 64
            // above the region's bottom edge. The negative spacing is how this kit
            // places one element over another -- iced composites a tree in order and
            // has no z-order -- and it is the same trick the modal layer uses.
            column![
                container(pane_content).height(Length::Fixed(pane_height)),
                container(Space::new(Length::Fill, Length::Fixed(SETTINGS_FADE))).style(
                    move |_theme: &Theme| container::Appearance {
                        background: Some(settings_fade(theme)),
                        ..container::Appearance::default()
                    },
                ),
            ]
            .spacing(-SETTINGS_FADE)
            .width(Length::Fill)
            .into()
        } else {
            container(pane_content).height(Length::Fixed(pane_height)).into()
        };
        let content = container(pane_content)
            .width(Length::Fill)
            .height(Length::Fixed(pane_height))
            .padding(Padding {
                top: 0.0,
                bottom: 0.0,
                left: SETTINGS_BODY_PAD,
                right: SETTINGS_BODY_PAD,
            });
        let body = row![nav_column, divider, content];
        self.tabbed_dialog(body.into())
    }

    /// The body of the settings tab the dialog is showing.
    ///
    /// Each arm is one of the reference's own settings components: Appearance is
    /// `layouts/shared/appearance-settings/layout.vue`, Language is
    /// `LanguageSettings.vue`.
    fn settings_tab_view(&self) -> Element<'_, Message> {
        let theme = self.theme;
        match self.settings_tab {
            SettingsTab::Appearance => self.appearance_settings(),
            SettingsTab::Language => {
                let mut body = self.language_options();
                if let Some(warning) = &self.accounts_warning {
                    // A warning rather than a failure: the launcher works, and what
                    // the reader needs to know is that it does not know which account
                    // was signed in. It is under the list because the reference's
                    // pane has no such box and this one is the launcher's own.
                    body = column![]
                        .width(Length::Fill)
                        .push(body)
                        .push(crate::ui::admonition(
                            theme,
                            crate::ui::Severity::Warning,
                            "accounts",
                            warning,
                        ))
                        .into();
                }
                body
            }
        }
    }

    /// A dialog in the reference's `NewModal` frame around a body that brings its own
    /// padding: the settings modal, whose `TabbedModal` is `p-6 pb-3 pr-0`.
    ///
    /// The frame is `NewModal.vue`'s own: `bg-bg-raised`, `rounded-2xl`, a 1-pixel
    /// `--surface-5` border, a `p-6` header whose title is `text-2xl font-semibold
    /// text-contrast` with the close button at its right, and a 1-pixel `--surface-5`
    /// rule between the header and the content. Measured against the reference at
    /// 1280x720: the title's ink sits at y 98..115 and the rule at y=150 of a dialog
    /// whose border is at y=65 -- 65 + 1 (the border) + 24 (the header's padding) +
    /// 36 (the row itself, which the close button's `h-9` is the tall side of) + 24
    /// (padding again) = 150.
    fn tabbed_dialog<'a>(&'a self, body: Element<'a, Message>) -> Element<'a, Message> {
        let theme = self.theme;
        let head = container(
            row![]
                .align_items(Alignment::Center)
                .push(
                    text(Key::LabelSettings.message())
                        .size(24.0)
                        // `text-2xl`'s own `2rem` line: `NewModal.vue`'s header is
                        // `p-6` around it, and the dialog's whole height is that
                        // arithmetic plus the body's `p-6 pb-3` around the content
                        // floor -- 588 rows in the reference's capture.
                        .line_height(iced::Pixels(THEME_MODAL_TITLE_LINE))
                        .font(heading())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                )
                .push(Space::with_width(Length::Fill))
                .push(self.dialog_close()),
        )
        .width(Length::Fill)
        .padding(DIALOG_PAD);
        let rule = container(Space::new(Length::Fill, Length::Fixed(1.0))).style(
            move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface5))),
                ..container::Appearance::default()
            },
        );
        // The body's own padding, `TabbedModal.vue`'s `p-6 pb-3 pr-0`, with its
        // bottom short by the one pixel the reference's own box does not have to
        // spend. `pb-3` is 12; this is 11, and the reason is the dialog's own
        // height. The reference's box is 591 rows of CSS (589 of content, its two
        // border pixels outside that), centered in 720: 64.5, which a browser
        // snaps to 65 and a rasteriser at a half-pixel cannot. Shortening the box
        // by one gives an even 590, so the border lands on 65 exactly and every row
        // above this padding is the reference's own -- only the dialog's last two
        // rows (a padding row and the bottom border) sit one higher.
        let body = container(body)
            .width(Length::Fill)
            .padding(Padding { top: 24.0, bottom: 11.0, left: 24.0, right: 0.0 });
        // `NewModal.vue`'s own box: `rounded-2xl border border-solid
        // border-surface-5` around the content. The border is *outside* the content
        // here -- an outer container one pixel wider than the inner one on every
        // side -- because that is what `box-sizing: border-box` does to an `auto`
        // height in CSS: the border joins the box rather than sitting on top of the
        // first and last rows of its content. Drawn the other way (iced's own, a
        // border painted inside the bounds) every row of the header and body came
        // out one pixel left and one up of the capture.
        let frame = container(column![head, rule, body]).style(move |_theme: &Theme| {
            container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                border: Border {
                    radius: (SETTINGS_RADIUS - SETTINGS_BORDER).into(),
                    ..Border::default()
                },
                ..container::Appearance::default()
            }
        });
        container(frame)
            .width(Length::Fixed(SETTINGS_WIDTH.min(self.viewport.width - 160.0).max(320.0)))
            .padding(Padding::from(SETTINGS_BORDER))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface5))),
                border: Border { radius: SETTINGS_RADIUS.into(), ..Border::default() },
                ..container::Appearance::default()
            })
            .into()
    }

    /// The creation dialog: a name, the modloader, the version the instance is
    /// for, which build of that loader, and the button that makes it.
    ///
    /// The step is the reference's own (`CustomSetupStage.vue`), field for field:
    /// the name, its loader chips, the searchable game-version combobox with its
    /// snapshots footer, and then -- for a loader that has builds at all -- the
    /// loader-version chips over the list the *Other* one opens. The combobox is
    /// the picker: a searchable field over Mojang's version list, and under it the
    /// dropdown the field belongs to, which is the options and then the footer
    /// that adds the snapshots and old builds to them. What the reference
    /// *teleports* to the window's edge is drawn here in place, which is the same
    /// wall the modal layer hit: iced 0.12 composites a tree in order and has no
    /// z-order.
    fn create_dialog(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let usable = self.create_usable();
        let mut body = column![]
            .spacing(12.0)
            .push(
                text(Key::CreationFlowModalCustomSetupNameLabel.message())
                    .size(14.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(
                text_input(
                    Key::CreationFlowModalCustomSetupNamePlaceholder.message(),
                    &self.create_name,
                )
                .on_input(Message::CreateName)
                .padding(Padding { top: 10.0, bottom: 10.0, left: 12.0, right: 12.0 })
                .size(14.0)
                .font(medium())
                .style(iced::theme::TextInput::Custom(Box::new(crate::ui::Field::bordered(theme)))),
            )
            // The reference's own order on this step: the loader first, then the
            // game version it has to have been published for, then which build of
            // it.
            .push(self.loader_chips())
            .push(self.version_heading())
            .push(crate::ui::search(
                theme,
                Key::CreationFlowModalCustomSetupGameVersionSearchPlaceholder.message(),
                &self.version_query,
                Message::VersionQuery,
            ))
            .push(self.version_picker());
        // A loader with no build list is a vanilla instance, and the dialog says
        // so rather than drawing a row of chips that would change nothing.
        if self.create_loader.loads_mods() {
            body = body.push(self.loader_version());
        }
        if let Some(reason) = &self.create_error {
            body = body.push(
                text(reason.clone())
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Red))),
            );
        }
        body = body.push(
            row![]
                .align_items(Alignment::Center)
                .push(Space::with_width(Length::Fill))
                // `MultiStageModal.vue`'s own footer: the create action is its
                // right button, drawn as a `Button` with `color="brand"` -- no
                // `size`, so the frame's default `md` -- with the stage's own
                // `PlusIcon` before the label (`custom-setup-stage.ts`).
                .push(crate::ui::button_with_icon_sized(
                    theme,
                    CREATE_BUTTON,
                    Glyph::Plus,
                    Key::CreationFlowButtonCreateInstance,
                    crate::ui::Kind::Colored,
                    crate::ui::Size::Md,
                    Length::Shrink,
                    usable.then_some(Message::Create),
                )),
        );
        self.dialog(Key::CreationFlowTitleCreateInstance, body.into())
    }

    /// One field's heading: the reference's own label, with the value in force
    /// beside it.
    ///
    /// The reference shows the same value *in* its trigger -- a searchable
    /// combobox mirrors the selection over its own search field -- and a toolkit
    /// with no overlay has to say it somewhere, which is here rather than
    /// nowhere: a field showing a search term and a picker with a highlighted row
    /// are not enough on their own when the list is scrolled past the choice.
    ///
    /// It matters most on the loader-version row, where the chips name a *rule*
    /// rather than a version: without this, the build the rule comes to is one
    /// the user cannot see until the instance exists.
    fn field_heading(&self, label: Key, value: Option<String>) -> Element<'_, Message> {
        let theme = self.theme;
        let mut heading = row![].align_items(Alignment::Center).push(
            text(label.message())
                .size(14.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        );
        if let Some(value) = value {
            heading = heading.push(Space::with_width(Length::Fill)).push(
                text(value)
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            );
        }
        heading.into()
    }

    /// The version picker's own heading.
    fn version_heading(&self) -> Element<'_, Message> {
        self.field_heading(Key::LabelGameVersion, self.chosen_version())
    }

    /// The versions the picker's list is drawn from: Mojang's own order, with the
    /// search and the snapshots footer applied.
    ///
    /// A `contains` rather than a prefix match, because the part of a version a
    /// user remembers is as often its end as its beginning: `1.20` finds `1.20.1`,
    /// and `w02a` finds `25w02a`.
    fn version_matches(&self) -> Vec<&store::GameVersion> {
        let Some(list) = self.versions.ready() else {
            return Vec::new();
        };
        let needle = self.version_query.trim().to_ascii_lowercase();
        list.versions
            .iter()
            .filter(|version| self.version_snapshots || version.release)
            .filter(|version| {
                needle.is_empty() || version.id.to_ascii_lowercase().contains(&needle)
            })
            .collect()
    }

    /// The picker's list and its footer: the reference's dropdown body, drawn
    /// under the field it belongs to.
    ///
    /// Every state the request can be in is drawn as a sentence rather than as an
    /// empty list, which is the whole reason the store answers with a reason: a
    /// picker with no options because the request failed looks exactly like a
    /// picker with no options because there are none.
    fn version_picker(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let body: Element<'_, Message> = match &self.versions {
            // Asked, not answered. The reference shows its own loading label in
            // place of the options while that is true.
            Load::Idle | Load::Loading => {
                self.list_note(Key::LabelLoading.message(), INK_SECONDARY)
            }
            // A failure is a sentence and not an empty list -- and it is the
            // reader's own machine that will be fine, so it is not drawn as an
            // error either.
            Load::Failed(reason) => self.list_note(reason, Ink::Red),
            Load::Empty => self.list_note(
                Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                INK_SECONDARY,
            ),
            Load::Ready(_) => {
                let matches = self.version_matches();
                if matches.is_empty() {
                    // The reference's own arm: a search that found nothing gets
                    // the same sentence as a list that has nothing in it.
                    self.list_note(
                        Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                        INK_SECONDARY,
                    )
                } else {
                    let rows: Vec<Element<'_, Message>> = matches
                        .into_iter()
                        .map(|version| self.version_row(version))
                        .collect();
                    // The list scrolls past 300px, which is the reference's own
                    // bound on its options -- and it is the options rather than
                    // the dropdown, so the footer stays outside it.
                    //
                    // The bar is the dialog's, at the reference's six pixels: a
                    // `MultiSelect`/`Combobox` list is an `overlayscrollbars`
                    // instance like every other list in the reference
                    // (`MultiSelect.vue:1487-1489`, `Combobox.vue:1121-1123`).
                    container(crate::scroll::region(
                        crate::scroll::VERSIONS,
                        column(rows).width(Length::Fill),
                        Message::Wheel,
                    )
                    .direction(bar_direction())
                    .style(bar(theme)))
                        .width(Length::Fill)
                        .max_height(VERSION_LIST_HEIGHT)
                        .into()
                }
            }
        };
        let (key, label, glyph) = if self.version_snapshots {
            (VERSION_HIDE_SNAPSHOTS, Key::ButtonHideSnapshots, Glyph::EyeOff)
        } else {
            (VERSION_SHOW_ALL, Key::ButtonShowAllVersions, Glyph::Eye)
        };
        container(column![body, self.version_toggle(key, label, glyph)])
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                // The reference's teleported dropdown: `bg-surface-4`, a
                // `--surface-5` hairline and `rounded-[14px]`.
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface4))),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 14.0.into(),
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// The creation dialog's modloader row: the reference's five chips, with
    /// Vanilla offered first.
    ///
    /// The label is the reference's own *Modloader* for an instance flow, and the
    /// chips are `LoaderKind::all()` -- which is also what the store keys its
    /// loader requests by, so a chip and the request it makes cannot drift.
    fn loader_chips(&self) -> Element<'_, Message> {
        let labels: Vec<(String, bool)> = crate::catalog::LoaderKind::all()
            .iter()
            .map(|loader| (loader.label().to_string(), self.create_loader == *loader))
            .collect();
        let keys: Vec<&'static str> = crate::catalog::LoaderKind::all()
            .iter()
            .map(|loader| loader_chip_key(*loader))
            .collect();
        let chips = crate::ui::chips(self.theme, &keys, &labels, |index| {
            Some(Message::LoaderChoice(crate::catalog::LoaderKind::all()[index]))
        });
        column![]
            .spacing(8.0)
            .push(self.field_heading(Key::CreationFlowModalCustomSetupLoaderLabel, None))
            .push(chips)
            .into()
    }

    /// The creation dialog's loader-version row: which *kind* of build to
    /// install, and -- under *Other* -- the builds to choose between.
    ///
    /// Only drawn for a loader that loads mods: vanilla has no builds, and the
    /// reference's own row is inside a `v-if` on the same condition.
    fn loader_version(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let labels: Vec<(String, bool)> = BuildChoice::ALL
            .iter()
            .map(|choice| (choice.label().message().to_string(), self.build_choice == *choice))
            .collect();
        let keys: Vec<&'static str> = BuildChoice::ALL.iter().map(|choice| choice.key()).collect();
        let stable = self.stable_offered();
        let chips = crate::ui::chips(self.theme, &keys, &labels, |index| {
            let choice = BuildChoice::ALL[index];
            // The reference's `disabledItems`: a loader that published nothing
            // stable for this game version leaves the chip where it is and dim.
            (stable || choice != BuildChoice::Stable).then_some(Message::BuildChoice(choice))
        });
        let mut body = column![]
            .spacing(8.0)
            .push(self.field_heading(
                Key::CreationFlowModalCustomSetupLoaderVersionLabel,
                self.chosen_build(),
            ))
            .push(chips);
        match &self.loader_builds {
            Load::Ready(_) if self.build_choice == BuildChoice::Other => {
                // The reference's own search field, over its own dropdown: the
                // build a user is after is usually one they were told the
                // number of.
                body = body
                    .push(crate::ui::search(
                        theme,
                        Key::CreationFlowModalCustomSetupLoaderVersionSearchPlaceholder.message(),
                        &self.build_query,
                        Message::BuildQuery,
                    ))
                    .push(self.build_picker());
            }
            // Under *Stable* and *Latest* there is no list to draw, but the
            // three other states of the request still have to be said: the chips
            // name a rule, and *loading*, *it failed* and *this loader published
            // nothing for this game version* are what happened to it. Without
            // this the row would be a heading, three chips and a dark button.
            Load::Ready(_) => {}
            Load::Idle | Load::Loading => {
                body = body.push(self.list_note(Key::LabelLoading.message(), INK_SECONDARY));
            }
            Load::Empty => {
                body = body.push(self.list_note(
                    Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                    INK_SECONDARY,
                ));
            }
            Load::Failed(reason) => body = body.push(self.list_note(reason, Ink::Red)),
        }
        body.into()
    }

    /// The builds the *Other* chip's list is drawn from: the loader's own order,
    /// with the search applied.
    ///
    /// The same `contains` rule as the version picker's, for the same reason: a
    /// build is remembered by its end as often as its beginning, and `21.4`
    /// finds `21.4.100`.
    fn build_matches(&self) -> Vec<&store::LoaderBuild> {
        let Some(builds) = self.loader_builds.ready() else {
            return Vec::new();
        };
        let needle = self.build_query.trim().to_ascii_lowercase();
        builds
            .iter()
            .filter(|build| {
                needle.is_empty() || build.version.to_ascii_lowercase().contains(&needle)
            })
            // The dialog's own bound rather than the service's: it is the same
            // sixty the old shell's dropdown offered, and it is applied here so
            // that the list is scrollable rather than endless.
            .take(crate::catalog::MAX_OTHER_BUILDS)
            .collect()
    }

    /// The *Other* chip's list of builds, drawn in the picker's own box.
    fn build_picker(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let body: Element<'_, Message> = match &self.loader_builds {
            Load::Idle | Load::Loading => {
                self.list_note(Key::LabelLoading.message(), INK_SECONDARY)
            }
            Load::Failed(reason) => self.list_note(reason, Ink::Red),
            Load::Empty => self.list_note(
                Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                INK_SECONDARY,
            ),
            Load::Ready(_) => {
                let matches = self.build_matches();
                if matches.is_empty() {
                    self.list_note(
                        Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                        INK_SECONDARY,
                    )
                } else {
                    let rows: Vec<Element<'_, Message>> = matches
                        .into_iter()
                        .map(|build| self.build_row(build))
                        .collect();
                    container(crate::scroll::region(
                        crate::scroll::BUILDS,
                        column(rows).width(Length::Fill),
                        Message::Wheel,
                    )
                    .direction(bar_direction())
                    .style(bar(theme)))
                        .width(Length::Fill)
                        .max_height(VERSION_LIST_HEIGHT)
                        .into()
                }
            }
        };
        container(body)
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                // The same box the version picker draws its options in: one list
                // of options in this dialog, drawn one way.
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface4))),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 14.0.into(),
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// One row of the picker's list: the reference's option, lit green when it is
    /// the version in force.
    ///
    /// `getOptionClasses` in `Combobox.vue` is the whole of it: the option in
    /// force is `bg-highlight-green text-green`, every other one is `bg-surface-4
    /// text-contrast hover:brightness-[115%]`. The brightness is this option's own
    /// hover end, which is why the crossing carries it.
    fn version_row(&self, version: &store::GameVersion) -> Element<'_, Message> {
        let id = version.id.clone();
        let selected = self.chosen_version().as_deref() == Some(id.as_str());
        let key = crate::ui::scoped("shell:version", &id);
        self.option_row(key, id.clone(), selected, Message::VersionChoice(id))
    }

    /// One row of the build picker's list, which is the same option again.
    ///
    /// The version in force here is the *picked* build rather than the resolved
    /// one: the row that is lit is the row the user pressed, and under the two
    /// other chips there is no list to light anything in.
    fn build_row(&self, build: &store::LoaderBuild) -> Element<'_, Message> {
        let version = build.version.clone();
        let selected = self.build.as_deref() == Some(version.as_str());
        let key = crate::ui::scoped("shell:build", &version);
        self.option_row(key, version.clone(), selected, Message::BuildPicked(version))
    }

    /// One option of a picker's list, drawn identically wherever it is offered.
    fn option_row(
        &self,
        key: &'static str,
        label: String,
        selected: bool,
        on_press: Message,
    ) -> Element<'_, Message> {
        let theme = self.theme;
        let (factor, _) = crate::ui::interaction(key);
        let (background, ink) = if selected {
            (theme_gen::ink(theme, Ink::GreenHighlight), theme_gen::ink(theme, Ink::Green))
        } else {
            (
                crate::theme::brightness(theme_gen::ink(theme, Ink::Surface4), factor),
                crate::theme::brightness(theme_gen::ink(theme, INK_CONTRAST), factor),
            )
        };
        let option = container(
            text(label)
                .size(14.0)
                .font(semibold())
                .style(iced::theme::Text::Color(ink)),
        )
        .width(Length::Fill)
        .padding(Padding {
            top: VERSION_ROW_HEIGHT,
            bottom: VERSION_ROW_HEIGHT,
            left: VERSION_ROW_SIDE,
            right: VERSION_ROW_SIDE,
        })
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(background)),
            ..container::Appearance::default()
        });
        mouse_area(option)
            .on_enter(Message::hover_with(key, true, VERSION_ROW_HOVER))
            .on_exit(Message::hover_with(key, false, VERSION_ROW_HOVER))
            .on_press(on_press)
            .into()
    }

    /// One sentence of a picker's own body, in the box its options are drawn in.
    ///
    /// Shared by both of the creation dialog's lists -- the game versions and the
    /// loader builds -- because the four states they can be in are the same four
    /// states, and a picker that said *nothing here* for one of them and drew an
    /// empty box for the other would be two answers to one question.
    fn list_note(&self, sentence: &str, ink: Ink) -> Element<'_, Message> {
        let theme = self.theme;
        container(
            text(sentence.to_string())
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, ink))),
        )
        .width(Length::Fill)
        .padding(Padding {
            top: VERSION_ROW_HEIGHT,
            bottom: VERSION_ROW_HEIGHT,
            left: VERSION_ROW_SIDE,
            right: VERSION_ROW_SIDE,
        })
        .into()
    }

    /// The picker's footer: the reference's own row that adds the snapshots and
    /// old builds to the list, and takes them away again.
    ///
    /// Its hover is `text-secondary hover:text-contrast`, which is an ink that
    /// moves rather than a surface that brightens, so it is drawn from the clock's
    /// hover *fraction* instead of from its brightness factor.
    fn version_toggle(&self, key: &'static str, label: Key, glyph: Glyph) -> Element<'_, Message> {
        let theme = self.theme;
        let (_, fraction) = crate::ui::interaction(key);
        let ink = crate::theme::mix(
            theme_gen::ink(theme, INK_SECONDARY),
            theme_gen::ink(theme, INK_CONTRAST),
            fraction,
        );
        let row = container(
            row![]
                .align_items(Alignment::Center)
                .spacing(6.0)
                .push(icon::icon(glyph, 16.0, ink))
                .push(
                    text(label.message())
                        .size(14.0)
                        .font(semibold())
                        .style(iced::theme::Text::Color(ink)),
                ),
        )
        .width(Length::Fill)
        .center_x()
        .padding(Padding {
            top: VERSION_ROW_HEIGHT,
            bottom: VERSION_ROW_HEIGHT,
            left: 0.0,
            right: 0.0,
        });
        mouse_area(row)
            .on_enter(Message::hover(key, true))
            .on_exit(Message::hover(key, false))
            .on_press(Message::VersionSnapshots(!self.version_snapshots))
            .into()
    }

    /// The import step: what the other launchers on this machine hold, one row
    /// and one button each.
    ///
    /// The list was read when the dialog opened, which is why it is a field: the
    /// scan walks every root this launcher knows how to read, and a walk per frame
    /// would be a walk per frame. What it found is what the reference's own import
    /// step lists -- the launcher each instance came from is `ImportCandidate`'s
    /// own `origin`, not a guess made here.
    fn import_dialog(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let mut body = column![].spacing(12.0);
        if let Some(reason) = &self.import_error {
            body = body.push(
                text(reason.clone())
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Red))),
            );
        }
        if self.import_found.is_empty() {
            body = body.push(
                text(Key::CreationFlowModalImportInstanceNotificationNoInstancesFoundTitle.message())
                    .size(16.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            );
            body = body.push(
                text(Key::CreationFlowModalImportInstanceNotificationNoInstancesFoundText.message())
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
            );
        } else {
            for candidate in &self.import_found {
                let key = crate::ui::scoped("shell:import", &candidate.source.to_string_lossy());
                body = body.push(
                    row![]
                        .spacing(ROW_GAP)
                        .align_items(Alignment::Center)
                        .push(
                            column![]
                                .spacing(2.0)
                                .push(
                                    text(candidate.name.clone())
                                        .size(14.0)
                                        .font(semibold())
                                        .style(iced::theme::Text::Color(theme_gen::ink(
                                            theme,
                                            INK_CONTRAST,
                                        ))),
                                )
                                .push(
                                    text(candidate.origin)
                                        .size(13.0)
                                        .font(medium())
                                        .style(iced::theme::Text::Color(theme_gen::ink(
                                            theme,
                                            INK_SECONDARY,
                                        ))),
                                ),
                        )
                        .push(Space::with_width(Length::Fill))
                        .push(crate::ui::button_or_sized(
                            theme,
                            key,
                            Key::CreationFlowModalImportInstanceActionAdd,
                            crate::ui::Kind::Standard,
                            crate::ui::Size::Md,
                            (!self.importing).then_some(Message::Import(candidate.source.clone())),
                        )),
                );
            }
        }
        self.dialog(
            Key::CreationFlowModalImportInstanceLauncherInstancesTitle,
            body.into(),
        )
    }

    /// The install dialog: which of the launcher's instances this project goes
    /// into.
    ///
    /// The reference asks the same question with a dropdown on the project page
    /// (*Install to instance*). It is a modal here for the reason every dialog in
    /// this shell is: iced 0.12 composites its tree in order and has no z-order, so
    /// a thing drawn over a page is drawn by the one layer that is over a page.
    ///
    /// A pack is the one project for which the question has a single answer, so
    /// the list is not drawn: it becomes an instance of its own, and the dialog
    /// says so and asks for the press.
    fn install_dialog<'a>(&'a self, project: &str, title: &str, pack: bool) -> Element<'a, Message> {
        let theme = self.theme;
        let mut body = column![]
            .spacing(10.0)
            .push(
                text(if title.is_empty() { project.to_string() } else { title.to_string() })
                    .size(16.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            ;
        // The reference's own word for it (`app.project.version.installing`), shown
        // only while a transfer is running: a dialog that always said *Installing*
        // would be lying about a launcher that is waiting to be told where.
        if self.installing {
            body = body.push(
                text(Key::AppProjectVersionInstalling.message())
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            );
        }
        if pack {
            // One answer rather than a list, because a pack has no folder to land
            // in: the install makes the instance, names it after the project and
            // takes its Minecraft version and its loaders from the pack's own
            // index. The reference asks nothing here at all -- it makes the
            // instance and opens it -- and the dialog stays because a failure has
            // to be visible somewhere and this launcher says its installs here.
            let what = if title.is_empty() { project } else { title };
            body = body.push(
                text(format!(
                    "{what} becomes an instance of its own, with the Minecraft version and the \
                     loaders its own index names."
                ))
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            );
            body = body.push(crate::ui::button_sized(
                theme,
                INSTALL_PACK_KEY,
                // The reference's own word for it: its project cards install with
                // the copy the library's *Create instance* uses, because in the
                // reference installing a project *is* creating an instance.
                Key::AppLibraryContextMenuCreateInstance,
                crate::ui::Kind::Colored,
                crate::ui::Size::Md,
                Message::InstallPack,
            ));
        } else {
            match self.store.instances() {
                Load::Ready(cards) if !cards.is_empty() => {
                    for card in cards {
                        body = body.push(self.install_row(card));
                    }
                }
                // The empty state is the welcome screen's question, asked here:
                // there is nothing to install into, and the way out is the flow
                // that makes one. The button is a message of its own rather than
                // the rail's, because a rail message would also move the rail's
                // selection and whatever else pressing a rail slot does.
                _ => {
                    body = body.push(
                        text(Key::AppLibraryGroupNoInstancesFound.message())
                            .size(14.0)
                            .font(medium())
                            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
                    );
                    body = body.push(crate::ui::button_sized(
                        theme,
                        INSTALL_CREATE_KEY,
                        Key::AppWelcomeScreenCreateInstance,
                        crate::ui::Kind::Standard,
                        crate::ui::Size::Md,
                        Message::OpenCreate,
                    ));
                }
            }
        }
        if let Some(reason) = &self.install_error {
            body = body.push(crate::ui::admonition(
                theme,
                crate::ui::Severity::Critical,
                "install",
                reason,
            ));
        }
        self.dialog(Key::AppUserProjectInstallToInstance, body.into())
    }

    /// One instance in the install dialog: its name, what it runs, and the press
    /// that puts the project into it.
    ///
    /// The whole row is the target rather than a button beside it, which is what
    /// the reference's dropdown rows are -- and it is the same shape as the
    /// library's cards, whose pressable part is their body.
    fn install_row<'a>(&'a self, card: &'a InstanceCard) -> Element<'a, Message> {
        let theme = self.theme;
        let key = crate::ui::scoped("shell:install", &card.id);
        let (factor, _) = crate::ui::interaction(key);
        let details = column![]
            .spacing(2.0)
            .push(
                text(card.name.clone())
                    .size(14.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(
                text(card.subtitle())
                    .size(13.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            );
        let target = Message::InstallInto(card.id.clone());
        crate::ui::card_at(
            theme,
            factor,
            mouse_area(details.width(Length::Fill))
                .interaction(Interaction::Pointer)
                .on_enter(Message::hover_with(
                    key,
                    true,
                    crate::theme::INSTANCE_CARD_HOVER_BRIGHTNESS,
                ))
                .on_exit(Message::hover_with(
                    key,
                    false,
                    crate::theme::INSTANCE_CARD_HOVER_BRIGHTNESS,
                ))
                // A press while a transfer is in flight does nothing: two
                // transfers of one file through one part file is a corrupted mod.
                .on_press(target),
        )
    }

    /// The scrim and the dialog, over whatever the shell was drawing.
    fn modal_layer(&self) -> Element<'_, Message> {
        // The one modal that is not the shell's own. The Skins page's editor belongs to
        // the page's state -- a `Modal` variant would be a second copy of
        // [`skins::State::edit`] to keep in step -- so it is asked for here, and this
        // shell's own part is the frame around it. It is checked first because the page
        // cannot be showing an editor while a shell modal is up: a shell modal replaces
        // the window's contents, so nothing on the page is under it to press.
        if let Some((edit, capes, wearing)) = self.screen.skins_edit() {
            // The body is the *page's* message type, so every press in it is wrapped
            // back into `Message::Screen` here: the editor's controls are the page's,
            // and the shell is the one that draws them.
            let body = skins::edit_view(self.theme, edit, capes, wearing)
                .map(|message| Message::Screen(pages::Message::Skins(message)));
            let dialog = self.dialog(Key::AppSkinsModalEditTitle, body);
            return self.scrim(dialog);
        }
        let dialog = match &self.modal {
            Some(Modal::Create) => self.create_dialog(),
            Some(Modal::Import) => self.import_dialog(),
            Some(Modal::Install { project, title, pack }) => {
                self.install_dialog(project, title, *pack)
            }
            Some(Modal::InstanceSettings(state)) => self.dialog_titled(
                &format!("{} · {}", state.name, Key::LabelSettings.message()),
                crate::instance_settings::view(self.theme, state).map(Message::InstanceSettings),
            ),
            // The layer is only drawn while a modal is up, and Settings is the one
            // that exists: a `None` here is not reachable from `render`.
            Some(Modal::Settings) | None => self.settings_dialog(),
        };
        self.scrim(dialog)
    }

    /// The translucent bed a modal sits on, and the press that dismisses it.
    ///
    /// Its own function because there are now two ways into the layer -- a `Modal` and
    /// the Skins page's editor -- and a second copy of these eight lines would be a
    /// second place the scrim's colour could be changed alone.
    fn scrim<'a>(&self, dialog: Element<'a, Message>) -> Element<'a, Message> {
        // Read outside the closure: the ramp depends on the window's height, and
        // a closure that reached back into `self` for it would borrow the shell
        // for as long as the element lives.
        let bed = modal_scrim(self.viewport);
        let scrim = container(dialog)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(bed),
                ..container::Appearance::default()
            });
        mouse_area(scrim).on_press(Message::CloseModal).into()
    }
}

/// The canvas program behind one rail button.
///
/// All three layers in one program rather than a stack of widgets: the plate,
/// the background and the icon share a coordinate space in the reference (the
/// plate is `inset: 0` of the same 48px box the icon is centred in), and drawing
/// them together is what keeps them in the same one here.
struct RailButton {
    glyph: Glyph,
    /// Hover or subpage: `hover:bg-button-bg`.
    background: Option<Color>,
    /// The plate: how opaque it is, and how far it has grown.
    plate: Option<(f32, f32)>,
    plate_ink: Color,
    ink: Color,
}

impl<Message> canvas::Program<Message> for RailButton {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        if let Some(color) = self.background {
            frame.fill(&Path::circle(center, PLATE_RADIUS), color);
        }
        if let Some((opacity, scale)) = self.plate {
            let color = Color { a: self.plate_ink.a * opacity, ..self.plate_ink };
            frame.fill(&Path::circle(center, PLATE_RADIUS * scale), color);
        }
        // The icon sits in the middle of the plate, at `RAIL_ICON` pixels
        // whatever the plate's own size is.
        let (scale, offset) = icon::fit(self.glyph, RAIL_ICON);
        let inset = (PLATE - RAIL_ICON) / 2.0;
        frame.translate(Vector::new(inset + offset.x, inset + offset.y));
        frame.scale(scale);
        for (path, paint) in icons_gen::parts(self.glyph, scale, self.ink) {
            match paint {
                icons_gen::Paint::Stroke(stroke) => frame.stroke(&path, stroke),
                icons_gen::Paint::Fill(color) => frame.fill(&path, color),
            }
        }
        vec![frame.into_geometry()]
    }
}

/// The skins this launcher has stored, as the Skins page draws them.
///
/// A free function rather than a method because [`Shell::skins`]'s worker calls it with a
/// store it owns, and because it is pure: read the index, read each row's PNG, and answer
/// whether that PNG asks for the Ears features. A row whose texture will not come back is
/// still a row -- its name and arm style are in the index -- so a failed read answers
/// `false` for the marker rather than dropping the skin.
fn stored_rows(store: &Store) -> Vec<skins::SavedRow> {
    let Some(paths) = store.paths() else {
        // A store built over a fetch double has no folder, which is how the tests build
        // one: there is nothing saved, and that is not an error.
        return Vec::new();
    };
    crate::saved_skins::load(paths)
        .ordered()
        .into_iter()
        .map(|entry| {
            let ears = crate::saved_skins::texture(paths, entry)
                .ok()
                .and_then(|bytes| crate::skin::cut(&bytes))
                .and_then(|cut| cut.ears)
                .is_some();
            skins::SavedRow { entry: entry.clone(), ears }
        })
        .collect()
}

/// Write the reader's own order for the stored skins, the way the Skins page's move
/// asks for it.
///
/// A free function beside [`stored_rows`], for the same reason and one more: the write
/// is a file under the product's own directory, so it needs a store and a list of keys --
/// a test can hand it both and read the index back, where the command around it is a
/// `Command::perform` future no test can run. The refusal is what a store with no folder
/// on disk is answered with, rather than a write into nowhere.
fn write_skin_order(store: &Store, keys: &[String]) -> Result<(), String> {
    let Some(paths) = store.paths() else {
        return Err("This launcher has no folder for saved skins.".to_string());
    };
    crate::saved_skins::reorder(paths, keys)
}

/// Keep a texture the reader just picked, the way Add promises the Saved-skins section
/// will.
///
/// The name is the file's own with its extension dropped, because that is what the reader
/// called it and the reference names a custom skin by the file it came from. The source is
/// [`crate::saved_skins::Source::Custom`], the picker being the only way in this launcher
/// has for a texture of the reader's own; the reference's `custom_external` would be a
/// texture imported from another launcher, which this job has no importer for. A store
/// write that fails is *not* reported: the upload is what the reader asked for, and a row
/// missing from a section they have not opened is a smaller failure than a sentence saying
/// the skin they can see on their account did not go up.
fn keep_picked(store: &Store, change: &palantir_net::SkinChange) {
    let palantir_net::SkinChange::Upload { variant, file_name, texture } = change else {
        // The picker only ever builds an upload; any other change is not this function's.
        return;
    };
    let Some(paths) = store.paths() else {
        return;
    };
    let name = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    let _ = crate::saved_skins::add(
        paths,
        texture,
        name,
        variant,
        "",
        crate::saved_skins::Source::Custom,
    );
}

/// The switcher's separator: `py-2` around an `h-px w-8` `--surface-5` rule.
///
/// The reference makes this strip the drag handle that resizes how many
/// recent instances the rail shows, with its cursor, its hover colour and its
/// overdrag flash; there are no recent instances to resize here, so this is the
/// same strip without the behaviour. The rule does not sit at the strip's own
/// top: the reference's switcher is a `gap-1` column and the recent-instances
/// list above it is a wrapper that takes a gap even when it is empty, so the
/// rule is `RAIL_GAP + 8` -- the empty wrapper's gap, then `py-2` -- below the
/// strip's top edge, which is the 4px that puts the reference's rule at y=320.
fn rail_separator(theme: Gen) -> Element<'static, Message> {
    container(crate::page::rule(theme))
        .width(Length::Fixed(RAIL_RULE))
        .padding(Padding {
            top: RAIL_GAP + 8.0,
            right: 0.0,
            bottom: 8.0,
            left: 0.0,
        })
        .into()
}

/// The icon a rail slot draws, from `App.vue`'s own imports.
fn rail_glyph(slot: Rail) -> Glyph {
    match slot {
        Rail::Home => Glyph::Play,
        Rail::Discover => Glyph::Compass,
        Rail::Skins => Glyph::Shirt,
        Rail::Screenshots => Glyph::Image,
        Rail::Servers => Glyph::ServerStack,
        Rail::CreateInstance => Glyph::Plus,
        Rail::Settings => Glyph::Settings,
        Rail::Profile => Glyph::LogIn,
    }
}

/// One entry of the head's breadcrumb trail.
///
/// `Breadcrumbs.vue`'s own shape: a visual in front of the label, then the
/// label. The visual is `size-5` and is either an *icon* the page registered or
/// an *image* it fetched -- a project's icon, a user's avatar, an instance's own
/// art -- and this launcher draws the icon half and leaves the fetched half
/// empty rather than inventing art for it. That is why [`Crumb::icon`] is an
/// `Option`: the reference's `visual` is optional too.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Crumb {
    icon: Option<Glyph>,
    label: String,
}

impl Crumb {
    /// An entry whose visual is one of the reference's own icons.
    fn with_icon(glyph: Glyph, label: impl Into<String>) -> Self {
        Crumb { icon: Some(glyph), label: label.into() }
    }

    /// An entry whose visual the reference fetches, so there is none here.
    fn plain(label: impl Into<String>) -> Self {
        Crumb { icon: None, label: label.into() }
    }
}

/// `Breadcrumbs.vue`'s own measurements: `size-5` on both the visual and the
/// chevron between entries, `gap-1.5` inside an entry and `gap-2` between them,
/// and `text-base font-medium leading-6` on the label.
const CRUMB_ICON: f32 = 20.0;
const CRUMB_GAP: f32 = 8.0;
const CRUMB_ENTRY_GAP: f32 = 6.0;
const CRUMB_LABEL: f32 = 16.0;
/// `leading-6`: the label's own line, twenty-four pixels of sixteen-pixel text.
const CRUMB_LINE: f32 = 24.0;
/// `Breadcrumbs.vue`'s own `pl-4`: the trail's left padding, which is the gap
/// between it and the history pair in front of it.
const CRUMB_INSET: f32 = 16.0;

/// The trail the head shows, built the way the reference builds it.
///
/// Every page registers its own breadcrumb, and the reference's are one entry
/// each: a `useRootBreadcrumb` for a page that is the root of its section
/// (`Index.vue` *Home*, `Skins.vue`, `Screenshots.vue`, `Servers.vue`, `Browse.vue`
/// *Discover*, `instance/layout.vue`), and a `useBreadcrumb` pushed under that
/// root for a page that is *inside* one (`User.vue`, `project/Index.vue`,
/// `hosting/manage/Index.vue`). This launcher has no instance context on its
/// project and user pages and no server context on its server page, so one entry
/// is what the reference's own trail would hold here too -- and where the
/// reference pushes under a root, that root is the entry.
///
/// The labels are the reference's own, which is not what this used to say: the
/// servers page calls itself *Hosting*, not *Servers*, and the instance,
/// project and user pages are named after the thing rather than after the
/// section and the id.
fn crumbs(address: &Address, instance_name: Option<&str>) -> Vec<Crumb> {
    match &address.route {
        // `Index.vue`'s `useRootBreadcrumb`: `app.navigation.home` with a
        // `PlayIcon`.
        route::Route::Home => vec![Crumb::with_icon(Glyph::Play, Key::AppNavigationHome.message())],
        // `Browse.vue`'s own: `app.browse.discover-project-type` with a
        // `CompassIcon`. This launcher's Discover is never browsing inside a
        // server or an instance, so the label is always the project type.
        route::Route::Discover { project_type } => vec![Crumb::with_icon(
            Glyph::Compass,
            crate::text_gen::app_browse_discover_project_type(project_type.sentence(2)),
        )],
        // `Skins.vue` registers the label as a literal rather than a message, so
        // it is one here too.
        route::Route::Skins => vec![Crumb::with_icon(Glyph::Shirt, "Skin selector")],
        route::Route::Screenshots => {
            vec![Crumb::with_icon(Glyph::Image, Key::AppScreenshotsHeading.message())]
        }
        // `Servers.vue`, which spells its own name "Hosting".
        route::Route::Servers => vec![Crumb::with_icon(Glyph::ServerStack, "Hosting")],
        // `hosting/manage/Index.vue`: the server's own name, pushed under the
        // servers root. The name is the only part this route does not have; the
        // reference reads it from the server it loaded.
        route::Route::Server { id, .. } => {
            vec![Crumb::with_icon(Glyph::ServerStack, id.clone())]
        }
        // `User.vue`: the user's name with their avatar. The avatar is fetched.
        route::Route::User { user, .. } => vec![Crumb::plain(user.clone())],
        // `project/Index.vue`: the project's title with its icon, pushed under
        // an instance root when the project was reached from inside one.
        route::Route::Project { id, .. } => vec![Crumb::plain(id.clone())],
        // `instance/layout.vue`: the instance's name with its own art, and no
        // tab -- the tabs are `NavTabs` under the head, not crumbs in it.
        route::Route::Instance { id, .. } => vec![Crumb::plain(
            instance_name.map(str::to_string).unwrap_or_else(|| id.clone()),
        )],
    }
}

/// A hairline in `--brand-gradient-border`, the reference's `border-l` on the
/// panel and `border-b` under each of its sections.
///
/// iced paints a `Border` on all four sides of a box, so the one edge the
/// reference asks for is a widget: a one-pixel box, down the panel's page edge or
/// across a section's foot.
fn hairline(theme: Gen, vertical: bool) -> Element<'static, Message> {
    let (width, height) = if vertical {
        (Length::Fixed(1.0), Length::Fill)
    } else {
        (Length::Fill, Length::Fixed(1.0))
    };
    container(Space::with_width(Length::Fill))
        .width(width)
        .height(height)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::BrandGradientBorder))),
            ..container::Appearance::default()
        })
        .into()
}

/// The page pane's own edge, drawn *over* the page: `.app-contents::before`'s
/// one-pixel `--surface-5` rule along its top and left.
///
/// Two strips rather than a `Border`, because iced 0.12 paints a border on all
/// four sides and the reference's pseudo-element's other two edges are off the
/// window -- `right: calc(-1 * var(--left-bar-width))` and `bottom: calc(-1 *
/// var(--left-bar-width))` put them 64 pixels past the bottom right corner --
/// and because the corner's arc belongs to the page's own radius, which the
/// container behind it already shows.
fn pane_rule(theme: Gen) -> Element<'static, Message> {
    // One style for both strips: iced 0.12's `Column` carries no style of its
    // own, and a background on the column around them would paint the whole pane
    // in the rule's colour over the page.
    let rule = move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface5))),
        ..container::Appearance::default()
    };
    column![
        // `top`: the pseudo-element's one pixel of border across the pane, which
        // is also the corner.
        container(Space::with_width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(rule),
        // `left`: the same pixel down the pane's page edge.
        row![
            container(Space::with_height(Length::Fill))
                .width(Length::Fixed(1.0))
                .height(Length::Fill)
                .style(rule),
            Space::with_width(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The reference's `scrollbar-gutter: stable`, drawn as the pane draws its rule.
///
/// `.app-viewport` (`App.vue:2747-2752`) is `overflow: auto` with
/// `scrollbar-gutter: stable`, so it reserves its scrollbar's width at its right
/// edge whether or not anything has been scrolled. Nothing is painted in them --
/// the track is transparent (`global.scss:137-139`) and the thumb paints nothing
/// (`global.scss:141-146`: `--color-scrollbar` behind a five-pixel transparent
/// border with `background-clip: content-box`, over a ten-pixel content box) --
/// so what the column reads as is the page's own background under the pane's
/// shadow, which is what [`GUTTER`] measures.
///
/// The strip is [`ICED_BAR`] wide and not [`GUTTER`] because of where it starts,
/// not of how wide it is. What it hides is *iced's* bar: the page's region is
/// built in [`crate::page`], which never calls `.style(..)`, so the bar is
/// `Scrollable::Default` and is drawn at `bounds.right - 10`, ten pixels of
/// `palette.background.strong` (`iced_style-0.12.1/src/theme.rs:1196-1205`),
/// which is `#757C84` under this shell's palette. iced draws it *after* the
/// content (`scrollable.rs:909-918` lays the content down, `scrollable.rs:960-975`
/// the bar on top), so a bar that reached the page would sit over the page's own
/// right-hand pixels rather than beside them, and painting over it is the only
/// thing this file can do about that.
///
/// So the ten columns are split where the reference splits them, and the four
/// that are not the gutter paint nothing at all:
///
/// ```text
/// x=970..973   transparent -- the page's own `--surface-1` shows through
/// x=974        SHADOW_SOFT over that page background   = #16181B
/// x=975..979   SHADOW_MID  over that page background   = #15171B
/// ```
///
/// which is the reference's band to the byte on every route measured, and which
/// is also why the strip can be this narrow: [`GUTTER_BAR`] columns of it are the page's
/// own ink and change nothing.
///
/// What the strip cannot do is give back the ten pixels it covers to a layer
/// drawn *under* it -- which is why the hosting toast is not one. On
/// `/hosting/manage/` the reference's invite toast runs out to `x=979` -- the
/// page's content box ends at 974 and the toast overflows it, clipped by the
/// viewport's own padding box at 980 -- and a toast drawn inside the page stops at
/// `x=969`, because the strip is over the page. The toast is therefore a layer of
/// the *pane*, above this strip and below the pane's shadow and rule
/// ([`Shell::page_overlay`]), which is where the reference's `z-index`es put it.
///
/// # The two answers that do not work
///
/// Both were tried, and both leave the ten columns this strip owns covered by the
/// wrong pixels, so they are here rather than in the next reader's experiments.
///
/// **Delete the strip.** Its first job is hiding *iced's* bar, and the bands are
/// opaque rather than translucent for that reason alone (see [`shadow_band`]). It
/// is also the only thing standing between that bar and the page's own right-hand
/// pixels, on every route, and iced draws the bar over the content
/// (`scrollable.rs:960-975`), so without the strip a scrolling page shows a
/// `#757C84` bar down `x=970..979` that the reference does not have at all. The
/// five bands here are not decoration: they are what the reference measures in
/// those columns, byte for byte, on every route this shell draws.
///
/// **Paint the strip under the page.** Same failure, from the other side, and it is
/// worth writing down because it looks like the same fix: the bar it has to hide is
/// drawn by the scrollable *inside* the page, so a strip under the page is a strip
/// under the bar as well, and the bar comes back. The strip has to be over the
/// page; the toast has to be over the strip; and nothing inside a page can be over
/// a sibling of the page. That is the whole of the argument for [`Clipped`] and a
/// page layer the shell inserts itself.
fn pane_gutter(theme: Gen) -> Element<'static, Message> {
    row![
        // The four columns where the reference measures the page's own
        // `--surface-1`, and which are here only to put an opaque page background
        // under iced's bar.
        shadow_band(theme, 0.0, GUTTER_BAR),
        shadow_band(theme, SHADOW_SOFT, 1.0),
        shadow_band(theme, SHADOW_MID, GUTTER - 1.0),
    ]
    .width(Length::Fixed(ICED_BAR))
    .height(Length::Fill)
    .into()
}

/// One band of the reference's `rgba(0, 0, 0, 0.1) inset`, at a measured depth,
/// `width` of the pane's own `--surface-1` under it.
///
/// The depth is the number and not an ink, so the two measurements that share
/// these three values -- the pane's edge ramp and this reserved column -- cannot
/// come apart, and so that both themes get their own composite rather than a
/// colour typed in for the dark one.
///
/// Opaque rather than translucent, and the reason is the strip's first job: what
/// it covers is *iced's* bar, so a translucent band would be a band over the bar
/// rather than over the page. Nothing is lost by it, because the reference's own
/// page inset is 24 (`p-6`, sixteen on Skins) and the shadow's reach is eight, so
/// there is never page ink under a band for a translucent fill to darken.
///
/// Not a `Shadow` on a `container`, and deliberately: iced 0.12.3 composites a
/// `container`'s `Shadow` inside its own rounded-box coverage in one quad
/// (`solid.wgsl`'s
/// `mix(base_color, shadow_color, (1.0 - radius_alpha) * shadow_alpha)`), which
/// bands the fill and made the tab strip unreadable in `7a29753`. A plain
/// background is one quad with no radius and no shadow in it.
fn shadow_band(theme: Gen, depth: f32, width: f32) -> Element<'static, Message> {
    let ink = shadow_ink(theme, depth);
    container(Space::with_height(Length::Fill))
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(ink)),
            ..container::Appearance::default()
        })
        .into()
}

/// The pane's own `--surface-1` under one of [`SHADOW_HARD`],
/// [`SHADOW_MID`] and [`SHADOW_SOFT`].
///
/// `rgba(0, 0, 0, a)` composited over an opaque colour, which is a scale of each
/// channel and nothing else -- the reference's shadow has no alpha of its own to
/// blend and no other colour under it to blend with.
fn shadow_ink(theme: Gen, depth: f32) -> Color {
    let base = theme_gen::ink(theme, Ink::Bg);
    Color {
        r: base.r * (1.0 - depth),
        g: base.g * (1.0 - depth),
        b: base.b * (1.0 - depth),
        a: base.a,
    }
}

/// One element, drawn inside a box inside the box it laid out in: the
/// `overflow: hidden` that iced 0.12 has no element for.
///
/// `.app-viewport` (`App.vue:2747-2752`) is `overflow: auto; overflow-x: hidden`,
/// so nothing drawn inside the pane reaches past the pane's own edges -- and the
/// hosting page's toast is the case that needs it, because the toast's box runs to
/// `x=984` while the pane ends at 980 ([`Shell::page_overlay`]). Every other layer
/// in the pane is already inside the pane's box, so this changes the toast's last
/// four columns and nothing else.
///
/// `box_` is measured from the element's own top-left corner rather than being the
/// element's bounds, and that is the whole trap. The pane's stack takes its size
/// from its base layer, and that layer is the row holding the page *and* the
/// panel, so a box read off the stack's own bounds is the window's width and
/// clips nothing. Worse, this widget wraps one *layer* of that stack rather than
/// the stack, so what it lays out is the layer: an element already moved to
/// `x=649` reports `649` as its own origin, not the pane's `64`. Clipping to
/// either is a no-op, and each looked right for as long as it was not measured.
///
/// The caller therefore states the box in the layer's own coordinates, which it
/// can because it is the one that moved the layer. See [`Shell::pane`].
///
/// Not a `Container::clip`, which reads like the same thing and is not:
/// `container.rs:262-268` only hands its content a *narrower `viewport`*, and a
/// `container`'s own background is `draw_background(renderer, &style,
/// layout.bounds())` (`container.rs:361-363`) -- it never looks at the viewport it
/// was given, so a clipped container still paints its fill past its own edge. Nor
/// is there a `clip` element to reach for: iced 0.12.3 has no `stack` either, and
/// the three widgets that do put a scissor round their own drawing --
/// `Scrollable`, `Svg` and `Image` -- each clip to their own bounds while drawing
/// something else.
///
/// So the scissor is `Renderer::with_layer`, which is the same call
/// `scrollable.rs:909-918` makes, and everything else about the element inside it
/// is untouched: the same tag, the same tree, the same layout node, the same events
/// and the same cursor. It is here rather than in `crate::pages::overlay` because
/// this is the only place in the port that needs it, and `overlay`'s `Stack` is a
/// layout thing rather than a compositing one.
struct Clipped<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a,
{
    box_: Rectangle,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Clipped<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    /// Draw `content` inside `box_`, stated from where it laid itself out.
    fn new(
        box_: Rectangle,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Element<'a, Message, Theme, Renderer> {
        Element::new(Clipped {
            box_,
            content: content.into(),
        })
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Clipped<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    // Everything but the drawing is the content's, and the layout node is the
    // content's own node rather than a new parent around it: the clip is a
    // compositing decision, and nothing that lays out, diffs or takes a pointer
    // can tell the difference.
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> iced::Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget().layout(tree, renderer, limits)
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation<Message>,
    ) {
        self.content.as_widget().operate(tree, layout, renderer, operation);
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> iced::event::Status {
        self.content.as_widget_mut().on_event(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        // The box the content laid out in, from which the caller's box is measured.
        // `with_layer` intersects with whatever clip it is already inside
        // (`iced_wgpu`'s `layer.rs:251-268`), so a scissor here is never wider than
        // the window.
        let laid_out = layout.bounds();
        let box_ = Rectangle {
            x: laid_out.x + self.box_.x,
            y: laid_out.y + self.box_.y,
            width: self.box_.width,
            height: self.box_.height,
        };
        let Some(bounds) = box_.intersection(viewport) else { return };
        renderer.with_layer(bounds, |renderer| {
            self.content.as_widget().draw(
                tree, renderer, theme, style, layout, cursor, viewport,
            );
        });
    }
}

/// The pane's own inset shadow, `App.vue:2764`'s
/// `box-shadow: 1px 1px 15px rgba(0, 0, 0, 0.1) inset`, as the three bands every
/// reference capture measures.
///
/// Drawn over the page like [`pane_rule`] and for the same reason -- the
/// pseudo-element is `z-index: 30` -- and in two runs rather than one `Shadow`
/// for the reason [`shadow_band`] gives. The top run stops at the panel's own
/// edge, which is measured rather than assumed: the rule above it crosses the
/// panel (`#42444A` at `y=48` for `x=1000` in `ref/hosting-clean3.png`) and the
/// shadow does not (`x=1000` reads `#18211E`, `#18211E`, `#18211F`, `#18221F`...
/// across `y=49..57`, the wash's own gradient and nothing else).
///
/// The left run starts below the corner the two share. The reference's corner is
/// the sixteen-pixel arc both are clipped to, and a pair of crossing bands would
/// darken it twice over; the top run already covers those eight rows, so the left
/// one begins where the top one ends.
fn pane_shadow(page: f32, theme: Gen) -> Element<'static, Message> {
    let band = |depth: f32| {
        let ink = shadow_ink(theme, depth);
        move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(ink)),
            ..container::Appearance::default()
        }
    };
    column![
        // The rule's own pixel, which is `pane_rule`'s and this shadow's is not:
        // the pseudo-element's `border` and its `box-shadow` are two properties
        // of one box, and the capture reads `#42444A` at `x=64` and `y=48`
        // between them.
        Space::with_height(Length::Fixed(1.0)),
        // `top`: the three rows under the rule, hard then mid then soft.
        container(Space::with_width(Length::Fill))
            .width(Length::Fixed(page))
            .height(Length::Fixed(SHADOW_HARD_W))
            .style(band(SHADOW_HARD)),
        container(Space::with_width(Length::Fill))
            .width(Length::Fixed(page))
            .height(Length::Fixed(SHADOW_MID_W))
            .style(band(SHADOW_MID)),
        container(Space::with_width(Length::Fill))
            .width(Length::Fixed(page))
            .height(Length::Fixed(SHADOW_SOFT_W))
            .style(band(SHADOW_SOFT)),
        // `left`: the same three columns down the page edge, below the rows the
        // top run has already covered.
        row![
            Space::with_width(Length::Fixed(1.0)),
            container(Space::with_height(Length::Fill))
                .width(Length::Fixed(SHADOW_HARD_W))
                .height(Length::Fill)
                .style(band(SHADOW_HARD)),
            container(Space::with_height(Length::Fill))
                .width(Length::Fixed(SHADOW_MID_W))
                .height(Length::Fill)
                .style(band(SHADOW_MID)),
            container(Space::with_height(Length::Fill))
                .width(Length::Fixed(SHADOW_SOFT_W))
                .height(Length::Fill)
                .style(band(SHADOW_SOFT)),
            Space::with_width(page),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The account the card's header names: the one in force, or the reference's own
/// sentence for a file whose selection is gone.
///
/// `AccountsCard.vue`'s title slot: `selectedAccount ? selectedAccount.profile.name
/// : messages.selectAccount`. A selection that names an account the file no longer
/// holds is the same case as none at all rather than the wrong name.
fn card_title(accounts: &[AccountEntry], selected: Option<&str>) -> String {
    match selected.and_then(|uuid| accounts.iter().find(|account| account.uuid == uuid)) {
        Some(account) => account.username.clone(),
        None => Key::MinecraftAccountSelectAccount.message().to_string(),
    }
}

/// The accounts card's own frame: `bg-button-bg border-surface-5 rounded-xl`.
///
/// `pad` is the caller's because the card's two states differ: the empty one is
/// `p-3` around its own stack, and the accordion has none of its own -- its header
/// and its body each carry theirs.
fn card_frame<'a, Message: 'a>(
    theme: Gen,
    pad: f32,
    body: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(body)
        .width(Length::Fill)
        .padding(pad)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
            border: Border {
                color: theme_gen::ink(theme, Ink::Surface5),
                width: 1.0,
                radius: CARD_FRAME_RADIUS.into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

/// The install dialog's one control that is not a row: the button that opens the
/// creation flow when there is no instance to install into.
///
/// A stable name per control, like every other key in the shelf: two buttons that
/// happen to share a word must not light together.
const INSTALL_CREATE_KEY: &str = "shell:install:create";

/// The pack install's own button, in the dialog that has no rows at all.
const INSTALL_PACK_KEY: &str = "shell:install:pack";

/// The panel's news section: its *View all news* button's stable name, and how
/// many articles it draws.
///
/// Four is the reference's own number (`App.vue` slices the feed to four for the
/// sidebar), and it is what keeps the section a known height in a column that also
/// holds the accounts card.
const NEWS_VIEW_ALL_KEY: &str = "shell:news:view-all";
const MAX_NEWS: usize = 4;

/// The link note's dismiss button.
const LINK_NOTE_DISMISS: &str = "shell:link-note:dismiss";

/// Whether a key press is the welcome screen's quick-create key.
///
/// `WelcomeScreen.vue`'s own guard, in its own words: `event.key.toLowerCase()
/// !== 'n'` returns early, as do `metaKey`, `ctrlKey` and `altKey`. Shift is
/// deliberately *not* one of them -- the reference lower-cases the key before
/// comparing, so `Shift+N` opens the creation flow there too, and the comparison
/// here is case-insensitive for the same reason. The two guards that are not here
/// are the event's target and `navigator.onLine`: the first is replaced by the
/// subscription existing only while the screen with no text field is up, and the
/// second by nothing at all, because this launcher has no online signal anywhere
/// and an offline reader meets the flow's own failure where the flow asks.
fn quick_create_press(key: &iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> bool {
    if modifiers.control() || modifiers.alt() || modifiers.logo() {
        return false;
    }
    matches!(key, iced::keyboard::Key::Character(text) if text.eq_ignore_ascii_case("n"))
}

/// A crossing published by one of the card's own surfaces.
///
/// The kit's controls publish [`Message::Control`] with no hover end, and the
/// shell answers those with the global brightening. The card's header and its rows
/// are `button-base`, whose own rule is `filter: brightness(0.85)`, so the two
/// surfaces that dim name the factor they end at where the crossing is made.
fn card_crossing(key: &'static str, over: bool) -> Message {
    Message::Control { key, over, hover: Some(CARD_PRESS_HOVER) }
}

/// The bed a modal sits on, copied from `NewModal.vue`'s `.modal-overlay.standard`.
///
/// A constant rather than a theme token because the reference's overlay is the
/// same ramp in every theme it has, and because it is not one of its variables:
/// the value lives in the component's own `style` block.
const MODAL_SCRIM: &str =
    "linear-gradient(to bottom, rgba(29, 48, 43, 0.52) 0%, rgba(14, 21, 26, 0.95) 100%)";

/// The same rule's `inset: -5rem`, at the reference's sixteen-pixel root.
///
/// It is why the window does not show the whole of [`MODAL_SCRIM`]: the overlay's
/// box is five rem taller than the window at *each* end, so a window shows the
/// ramp between `5rem / (height + 10rem)` and `(height + 5rem) / (height + 10rem)`
/// of it -- at 720 pixels, nine percent in at the top and nine percent short of
/// the bottom.
const MODAL_SCRIM_INSET: f32 = 5.0 * 16.0;

/// The floor the settings dialog's content pane keeps: `TabbedModal.vue`'s
/// `min-h-[min(65vh,600px)]` on both of its tracks.
///
/// It is a *floor*, not a height: the reference's content grows with its tab and is
/// clamped by the window, and this is the number the floor is read at -- 468 pixels in
/// a 720-pixel window, which is what makes a settings dialog 588 tall there.
fn settings_content_min(viewport: iced::Size) -> f32 {
    (viewport.height * 0.65).min(600.0)
}

/// How many theme cards fit a row of `width`: the reference's own grid,
/// `repeat(auto-fit, minmax(12rem, 1fr))` with `--gap-lg` between its tracks.
fn theme_columns(width: f32) -> usize {
    (((width + THEME_GAP) / (THEME_CARD_MIN + THEME_GAP)).floor()).max(1.0) as usize
}

/// The appearance pane's own height at a number of cards and columns: its heading
/// block, the grid, and the sync row, added up from the constants that lay them
/// out.
///
/// This is what the pane's fade is decided on: the reference shows that fade
/// whenever its scroll container has somewhere to scroll, and this pane cannot fit
/// its own content in the 468 pixels a 720-pixel window gives it.
fn appearance_settings_height(options: usize, columns: usize) -> f32 {
    let columns = columns.max(1);
    let rows = ((options + columns - 1) / columns) as f32;
    let section = THEME_TITLE_LINE + THEME_HEADING_LEAD + THEME_BODY_LINE;
    let grid = rows * THEME_CARD_HEIGHT + (rows - 1.0) * THEME_GAP;
    section + THEME_HEADING_GAP + grid + THEME_SECTION_GAP + section
}

/// One of a theme card's mock text bars: `h-2`, `rounded-[--radius-sm]`, and the
/// colour the previewed look gives `--color-base` or `--color-secondary`.
fn theme_bar(colour: Color, width: Length) -> Element<'static, Message> {
    container(Space::new(Length::Fill, Length::Fixed(THEME_BAR_HEIGHT)))
        .width(width)
        .height(Length::Fixed(THEME_BAR_HEIGHT))
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(colour)),
            border: Border { radius: THEME_MOCK_RADIUS.into(), ..Border::default() },
            ..container::Appearance::default()
        })
        .into()
}

/// The colours a theme card's mock is drawn in -- in the look the card is
/// *previewing*, not the one in force.
///
/// `.preview` carries a mode class built from the option's own name, so its
/// background is that mode's `--color-bg`; the mock inside it is the reference's
/// `border border-solid border-surface-4 bg-surface-3`, and its two bars are
/// `--color-base` and `--color-secondary`. Every one of them is read out of the
/// generated table at that mode's own column.
struct PreviewColors {
    /// `.preview`'s `background-color: var(--color-bg)`.
    bg: Color,
    /// `.example-card`'s `bg-surface-3`.
    card: Color,
    /// Its `border-surface-4`, which is also the icon's `var(--color-button-bg)`:
    /// the reference's two names for one surface.
    line: Color,
    /// The first bar, `--color-base`.
    bar1: Color,
    /// The second, `--color-secondary`.
    bar2: Color,
}

/// The room a theme card's name has at a given content width: its column less the
/// card's two border pixels, the label's `1rem` of padding on each side, the radio,
/// the `0.5rem` after it, and the `theme-icon` the light and preferred-dark cards
/// add (`margin-left: 0.25rem` and its own `1rem`). 197 of a 275-pixel card at the
/// window both clients were captured in.
///
/// `#[cfg(test)]`: what it measures is what the localization gate compares the 32
/// locales' names against. The cards themselves do not clip -- this renderer has
/// no clip for a text -- so there is nothing for the drawing layer to do with the
/// number.
#[cfg(test)]
fn theme_label_room(content: f32) -> f32 {
    let columns = theme_columns(content) as f32;
    let card = ((content - (columns - 1.0) * THEME_GAP) / columns).max(0.0);
    card - 2.0
        - 2.0 * THEME_GAP
        - THEME_LABEL_ICON
        - THEME_MOCK_GAP
        - THEME_ICON_GAP
        - THEME_LABEL_ICON
}

fn preview_colors(option: ColorTheme, system_prefers_light: bool) -> PreviewColors {
    let look = generated_theme(option, system_prefers_light);
    PreviewColors {
        bg: theme_gen::ink(look, Ink::Bg),
        card: theme_gen::ink(look, Ink::RaisedBg),
        line: theme_gen::ink(look, Ink::ButtonBg),
        bar1: theme_gen::ink(look, Ink::Base),
        bar2: theme_gen::ink(look, Ink::Secondary),
    }
}

/// The fade the settings pane draws over content it cannot show at once:
/// `TabbedModal.vue`'s bottom indicator, `h-16 bg-gradient-to-t from-bg-raised
/// to-transparent`.
///
/// `to top` means `bg-raised` at the *bottom* edge of the box with transparent 64
/// pixels above it, which is the down-the-box direction iced gives `to bottom`'s
/// angle -- the ramp climbs in the same direction the dialog's own overlay does.
fn settings_fade(theme: Gen) -> Background {
    let raised = theme_gen::ink(theme, Ink::RaisedBg);
    let mut linear = gradient::Linear::new(Radians(std::f32::consts::PI));
    linear = linear.add_stop(0.0, Color { a: 0.0, ..raised });
    linear = linear.add_stop(1.0, raised);
    Background::Gradient(gradient::Gradient::Linear(linear))
}

/// The bed a modal sits on: the reference's overlay, read at the window's own
/// edges.
///
/// The reference composites its dialog over a `backdrop-filter: blur(5px)` this
/// toolkit has no term for, and the blurred chrome behind the scrim is the one
/// part of that picture which is not drawn here -- the same deviation the modal
/// layer's own comment records. Everything else is its: the ramp, its two
/// colours, and the box it is laid over. Measured against the captured window,
/// which is what the numbers in the test below hold this to: a 720-pixel window
/// shows `(32, 43, 43)` over its chrome at the top and `(18, 25, 29)` at the
/// bottom, and both come back within a level.
///
/// It is a *gradient* rather than a stack of tones, and its stops carry alpha,
/// which is worth naming because a first capture of it read as opaque: the bed
/// was right and the layer beneath it was missing -- [`Shell::render`] drew the
/// modal *instead of* the window, so the ramp composited over the window's own
/// clear colour at every row and solved to an alpha of exactly `1.0` against the
/// reference's `0.647` at y=200, `0.706` at 350 and `0.824` at 600. The toolkit
/// blends both of its quad pipelines with `SrcAlpha`/`OneMinusSrcAlpha`; what was
/// wrong was what it had to blend with.
fn modal_scrim(viewport: iced::Size) -> Background {
    // The line and the stops are literals in [`MODAL_SCRIM`], so this arm is
    // unreachable in practice; it exists so that a typo in that line shows the
    // page rather than panicking in a paint.
    let Some((angle, stops)) = parse_gradient(MODAL_SCRIM) else {
        return Background::Color(Color::TRANSPARENT);
    };
    let span = viewport.height + 2.0 * MODAL_SCRIM_INSET;
    let mut linear = gradient::Linear::new(Radians(angle));
    linear = linear.add_stop(0.0, ramp_at(&stops, MODAL_SCRIM_INSET / span));
    linear = linear.add_stop(
        1.0,
        ramp_at(&stops, (MODAL_SCRIM_INSET + viewport.height) / span),
    );
    Background::Gradient(gradient::Gradient::Linear(linear))
}

/// The colour a ramp's stops give `fraction` of the way along it.
///
/// Straight interpolation between the two stops that bracket the fraction, which
/// is what a browser does between its own: the reference's overlays are written
/// as two stops and read as one continuous ramp.
fn ramp_at(stops: &[(f32, Color)], fraction: f32) -> Color {
    let mut low = stops[0];
    for &(offset, colour) in stops.iter().skip(1) {
        if fraction <= offset {
            let span = offset - low.0;
            let amount = if span <= 0.0 { 1.0 } else { (fraction - low.0) / span };
            return Color {
                r: low.1.r + (colour.r - low.1.r) * amount,
                g: low.1.g + (colour.g - low.1.g) * amount,
                b: low.1.b + (colour.b - low.1.b) * amount,
                a: low.1.a + (colour.a - low.1.a) * amount,
            };
        }
        low = (offset, colour);
    }
    stops[stops.len() - 1].1
}

/// The right panel's background: `--brand-gradient-bg`, over the page's own
/// colour.
///
/// The stops are the reference's, composited the way CSS composites them: an
/// `rgba()` stop is drawn *over* whatever is behind it, so the panel needs the
/// page's colour under the gradient rather than the gradient alone. A single
/// iced `Background` replaces what is behind it instead of compositing, which is
/// why the panel's container carries the gradient and the pane behind it carries
/// `--color-bg`.
fn wash(theme: Gen) -> Background {
    let Some((angle, stops)) = parse_gradient(theme_gen::raw(theme, Raw::BrandGradientBg)) else {
        // Every theme declares it, so this branch is unreachable in practice;
        // it exists so a future theme that does not gets the page's colour
        // rather than a panic.
        return Background::Color(theme_gen::ink(theme, Ink::Bg));
    };
    let mut linear = gradient::Linear::new(Radians(angle));
    for (offset, color) in stops {
        linear = linear.add_stop(offset, color);
    }
    Background::Gradient(gradient::Gradient::Linear(linear))
}

/// Read a CSS `linear-gradient` into an angle in radians and its stops.
///
/// Handles what the reference's own `--brand-gradient-*` values use: `deg`
/// angles, `rgba()` and hex stops with percentage offsets, and a bare colour
/// (which several themes use where others use a gradient). Anything it does not
/// understand is `None`, which the caller turns into the page's colour rather
/// than into a guess.
///
/// The angle needs no conversion beyond degrees to radians: iced measures its
/// gradient angle counter-clockwise from the +x axis and subtracts a quarter
/// turn, which makes 0 point up -- CSS's `0deg` -- so the reference's numbers go
/// through unchanged.
fn parse_gradient(value: &str) -> Option<(f32, Vec<(f32, Color)>)> {
    let value = value.trim();
    let Some(arguments) = value
        .strip_prefix("linear-gradient(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        // Not a gradient: a colour on its own is a legal value for the same
        // token in another theme, and it is the whole gradient once it is.
        let color = parse_color(value)?;
        return Some((0.0, vec![(0.0, color), (1.0, color)]));
    };
    let mut parts = split_top_level(arguments);
    if parts.is_empty() {
        return None;
    }
    // CSS's four direction words, at the angles iced already measures: `to top`
    // is `0deg` and the rest follow it clockwise, so `to right` is a quarter
    // turn, `to bottom` half a turn and `to left` three quarters. A corner
    // (`to bottom right`) is a different linear gradient -- its line is the
    // box's diagonal -- and is left unparsed rather than read as one of these.
    let angle = match parts[0].trim() {
        "to top" => Some(Ok(0.0)),
        "to right" => Some(Ok(std::f32::consts::FRAC_PI_2)),
        "to bottom" => Some(Ok(std::f32::consts::PI)),
        "to left" => Some(Ok(3.0 * std::f32::consts::FRAC_PI_2)),
        spelled => spelled
            .strip_suffix("deg")
            .map(|degrees| degrees.trim().parse::<f32>().map(f32::to_radians)),
    };
    let angle = match angle {
        // The first part was the angle, so it is not also a colour stop.
        Some(Ok(angle)) => {
            parts.remove(0);
            angle
        }
        // An angle that is not a number is a value this cannot read.
        Some(Err(_)) => return None,
        // No angle at all: the first part is the gradient's first colour, and
        // CSS's own default for a `linear-gradient` is `to bottom` -- half a
        // turn. None of the reference's values rely on it, but reading it as
        // `to top` would turn one upside down rather than refusing it.
        None => std::f32::consts::PI,
    };
    let mut stops = Vec::with_capacity(parts.len());
    for part in parts {
        let (color, position) = split_stop(&part)?;
        let color = parse_color(color)?;
        let offset = match position {
            Some(percent) => percent.trim_end_matches('%').parse::<f32>().ok()? / 100.0,
            // A stop with no position is positioned by the browser; none of the
            // reference's have that, and a missing one is a stop at the start.
            None => 0.0,
        };
        stops.push((offset.clamp(0.0, 1.0), color));
    }
    if stops.is_empty() {
        return None;
    }
    Some((angle, stops))
}

/// Split a gradient stop into its colour and its position.
///
/// The two are separated by whitespace, but only whitespace *outside*
/// parentheses: the reference writes `rgba(68, 182, 138, 0.175) 0%`, and
/// `split_whitespace` would hand back `rgba(68,` as the colour. Only the first
/// split matters, so this returns the two halves rather than a list.
fn split_stop(stop: &str) -> Option<(&str, Option<&str>)> {
    // Leading space is the normal case here: the reference writes its stops
    // after a comma, so every part but the first arrives padded.
    let stop = stop.trim();
    let mut depth = 0usize;
    for (index, character) in stop.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if character.is_whitespace() && depth == 0 => {
                let position = stop[index..].trim();
                return Some((stop[..index].trim(), (!position.is_empty()).then_some(position)));
            }
            _ => {}
        }
    }
    Some((stop.trim(), None))
}

/// Split on commas that are not inside parentheses.
///
/// `rgba(68, 182, 138, 0.175) 0%, ...` has commas that are part of a colour and
/// commas that separate stops, and a plain `split(',')` cannot tell them apart.
fn split_top_level(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for character in value.chars() {
        match character {
            '(' => {
                depth += 1;
                current.push(character);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(character),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts.into_iter().filter(|part| !part.trim().is_empty()).collect()
}

/// Read `#rgb`, `#rrggbb`, `rgb()` or `rgba()`.
fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let digits: Vec<u8> = hex.chars().filter_map(|c| c.to_digit(16).map(|d| d as u8)).collect();
        return match digits.len() {
            3 => Some(Color::from_rgb8(
                digits[0] * 17,
                digits[1] * 17,
                digits[2] * 17,
            )),
            6 => Some(Color::from_rgb8(
                digits[0] * 16 + digits[1],
                digits[2] * 16 + digits[3],
                digits[4] * 16 + digits[5],
            )),
            _ => None,
        };
    }
    let inner = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let numbers: Vec<f32> = inner
        .split(',')
        .filter_map(|part| part.trim().parse::<f32>().ok())
        .collect();
    match numbers.len() {
        3 => Some(Color::from_rgb(
            numbers[0] / 255.0,
            numbers[1] / 255.0,
            numbers[2] / 255.0,
        )),
        4 => Some(Color {
            a: numbers[3],
            ..Color::from_rgb(numbers[0] / 255.0, numbers[1] / 255.0, numbers[2] / 255.0)
        }),
        _ => None,
    }
}

// ---- As an application --------------------------------------------------

/// What a run of the shell was asked for on the command line.
#[derive(Debug, Clone, Default)]
pub struct Flags {
    /// `--page PATH`, an address [`Address::parse`] knows. A path and not a
    /// name: the route table is the only list of pages, so a page that does not
    /// have a path does not exist.
    pub page: Option<String>,
    /// `--size WxH`, the client size to open at.
    pub size: Option<(u32, u32)>,
    /// `--shot PATH`: draw this window, write its pixels to PATH as a PNG, and
    /// close.
    ///
    /// The flag the page gates are run through (`tools/appshot.py`), and a flag
    /// of the launcher rather than a screenshot tool reaching in from outside for
    /// the reason [`write_shot`] gives: only the process that drew the frame can
    /// hand back exactly that frame. A capture states its own size and is born
    /// off the desktop, so taking one neither resizes anything nor interrupts
    /// whoever is at the machine.
    pub shot: Option<std::path::PathBuf>,
}

impl Flags {
    /// Read the shell's own flags out of a command line.
    pub fn from_args(args: impl Iterator<Item = String>) -> Flags {
        let mut flags = Flags::default();
        let mut args = args.peekable();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--page" => flags.page = args.next(),
                "--shot" => flags.shot = args.next().map(std::path::PathBuf::from),
                "--size" => {
                    flags.size = args.next().and_then(|value| {
                        let (width, height) = value.split_once('x')?;
                        Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
                    })
                }
                _ => {}
            }
        }
        flags
    }

    /// The address this run opens at: the one asked for, or Home.
    pub fn opening(&self) -> Address {
        self.page
            .as_deref()
            .and_then(Address::parse)
            .unwrap_or_else(|| Address::at(route::Route::Home))
    }
}

/// Subscription id of the window's own state, reported by the window procedure.
const WINDOW_STATE_ID: &str = "palantirmc-window-state";
/// Subscription id of the `--shot` capture's settle timer.
const SHOT_ID: &str = "palantirmc-shot";
/// Subscription id of the extra frames a window owes its first one.
const REPAINT_ID: &str = "palantirmc-repaint";
/// How many extra frames the shell asks for when a window opens, and when.
///
/// This is a workaround, and the measurement behind it is the reason it exists.
/// The **first** frame a window presents draws no page: the pane between the rail
/// and the panel comes out as its own background with the page's scrollbar over
/// it, and nothing draws again until an event arrives -- so a launcher opened and
/// left alone is a launcher showing a blank page, and a capture is a picture of
/// one, because nothing ever touches a `--shot` window. Measured on this machine
/// at a pinned 1280x720 on `/browse/modpack`, from a window nobody has touched:
/// 172 distinct colours in the pane's rectangle (x 64..1003, y 70..720), against
/// 11,954 in the same rectangle after one click *posted* into the window, and
/// 12,253 after a resize. Both of those are events, and both produce the second
/// frame -- which is the frame with the page in it.
///
/// A message is enough to ask for that frame, because iced requests a redraw
/// whenever its loop has messages to deliver; no input is needed. Two, because the
/// first one only lands after the window's first frame on a machine that drew one
/// quickly: the second lands on the far side of that frame whatever startup cost
/// was, and the pair costs two repaints of a launcher that has just opened.
const REPAINT_KICKS: u8 = 2;
/// When each of [`REPAINT_KICKS`]'s frames is asked for, from the window opening.
const KICK_AFTER: [Duration; REPAINT_KICKS as usize] =
    [Duration::from_millis(300), Duration::from_millis(1200)];
/// How long a `--shot` run gives the window before taking its picture.
///
/// The number the old shell used, kept because the page gates were measured
/// against it: long enough that a page which reads a file or asks a service has
/// its answer on screen, short enough that a run of captures is not waiting on
/// the timer.
const SHOT_SETTLE: std::time::Duration = std::time::Duration::from_millis(3000);

/// The window's own state, as the window procedure reports it.
///
/// Neither thing it reports can be a widget's message. The maximize control is
/// answered as non-client -- which is what puts Snap Layouts on it -- so no
/// widget ever sees that pointer; and a window maximizes by paths with no button
/// in them at all: Aero Snap, the taskbar, `Win`+`Up`, and the native control
/// itself. iced rebuilds a view only when a message arrives, so both need a
/// message, and both change from outside the app. Polling for them would redraw
/// an idle launcher forever; instead the window procedure -- which is told about
/// the pointer by Windows and about maximizing by the window itself -- reports
/// each change down the pipe this subscription holds open. The cost is zero
/// until something actually changes.
fn window_state() -> Subscription<Message> {
    iced::subscription::channel(WINDOW_STATE_ID, 4, |mut sender| async move {
        let (reports, mut receiver) = futures::channel::mpsc::unbounded();
        crate::native::watch_window_state(reports);
        loop {
            if futures::StreamExt::next(&mut receiver).await.is_none() {
                break;
            }
            if sender.try_send(Message::WindowStateChanged).is_err() {
                break;
            }
        }
        // A channel subscription's future never resolves -- iced ends it by
        // dropping the receiver -- and `Infallible` is how that is said.
        futures::future::pending::<std::convert::Infallible>().await
    })
}

/// The window's own size, as iced reports it when the window is resized.
///
/// A message rather than a query for the same reason [`window_state`] is one: a
/// widget cannot ask the window anything, and what needs the number is the modal
/// layer -- [`Shell::dialog_body_room`], which caps a dialog's body so that a
/// long list scrolls inside a window instead of being drawn past it. iced reports
/// logical pixels, which is the space `view` builds in, so no scale factor is
/// applied here; the event arrives in the window's first frames as well as on
/// every resize, which is why the shell's own guess is only ever a fallback.
fn window_size() -> Subscription<Message> {
    iced::event::listen_with(|event, _status| match event {
        iced::Event::Window(_, iced::window::Event::Resized { width, height }) => Some(
            Message::Viewport(iced::Size::new(width as f32, height as f32)),
        ),
        _ => None,
    })
}

/// The maximize control's rectangle, for the window procedure.
///
/// [`Shell::window_controls`] draws the row from these constants -- `px-1.5`,
/// three `h-9 w-9` buttons, two `gap-2`s, all centred in the head's own height --
/// so the button's rectangle is arithmetic rather than a measurement. A rectangle
/// that cannot go stale does not have to be kept in step by hand, and publishing
/// it is what makes Windows answer the pointer over it as a caption button.
fn caption_target() -> crate::native::CaptionTarget {
    crate::native::CaptionTarget {
        right_inset: CONTROLS_PAD + CONTROLS_BUTTON + CONTROLS_GAP,
        width: CONTROLS_BUTTON,
        top: (BAR - CONTROLS_BUTTON) / 2.0,
        bottom: (BAR + CONTROLS_BUTTON) / 2.0,
    }
}

/// Write a captured frame to `path` as a PNG.
///
/// Written by this process rather than read out of the window by a tool, and
/// that is the whole reason `--shot` is a launcher flag: iced draws the window,
/// so the pixels it hands back are exactly the frame it drew -- no occlusion by
/// whatever else is on the desktop, no scaling by a screen capture, and no
/// dependence on a compositor this build may not even be running on.
fn write_shot(path: &std::path::Path, shot: &iced::window::Screenshot) -> Result<(), String> {
    // `::image`, not `image`: this file imports iced's `image` widget, so the
    // bare name is the widget rather than the crate that encodes the PNG.
    let (width, height) = (shot.size.width, shot.size.height);
    let pixels = shot.bytes.as_ref().clone();
    let image = ::image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| format!("{} bytes is not {width}x{height} of RGBA", shot.bytes.len()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    image.save(path).map_err(|error| error.to_string())
}

/// The theme in force, as the generated tables index it.
///
/// All four of the reference's painted themes are here, retro included: it is a
/// real look rather than a label for one of the others, and the settings pane that
/// can offer it is [`crate::color_theme::ColorTheme::options`], which the new
/// Settings modal uses. What this function does *not* do is decide whether retro
/// should be offered -- that is the pane's rule, quoted in `color_theme`.
pub fn generated_theme(setting: ColorTheme, system_prefers_light: bool) -> Gen {
    match setting.resolve(system_prefers_light) {
        ColorTheme::Light => Gen::Light,
        ColorTheme::Oled => Gen::Oled,
        ColorTheme::Retro => Gen::Retro,
        ColorTheme::Dark | ColorTheme::System => Gen::Dark,
    }
}

/// iced's own widget theme, derived from the generated tokens.
///
/// The shell paints its chrome explicitly, so this is what iced falls back to
/// for anything it draws itself -- a text cursor, a scrollbar, a default label
/// colour. Deriving it from the same tokens is what keeps those from being the
/// only part of the window drawn with the old hand-written palette.
pub fn widget_theme(theme: Gen) -> Theme {
    Theme::custom(
        format!("PalantirMC {}", brand::version()),
        iced::theme::Palette {
            background: theme_gen::ink(theme, Ink::Bg),
            text: theme_gen::ink(theme, INK_DEFAULT),
            primary: theme_gen::ink(theme, Ink::Brand),
            success: theme_gen::ink(theme, Ink::Green),
            danger: theme_gen::ink(theme, Ink::Red),
        },
    )
}

impl iced::Application for Shell {
    type Executor = iced::executor::Default;
    type Flags = Flags;
    type Message = Message;
    type Theme = Theme;

    fn new(flags: Flags) -> (Self, iced::Command<Message>) {
        let home = palantir_core::paths::PalantirPaths::home();
        // Two different directories on purpose: the settings are this product's
        // own, and the instances are wherever the launcher has found them, which
        // `detect` is the one thing that knows -- `home` is the launcher's own
        // folder and holds no instances.
        let paths = palantir_core::paths::PalantirPaths::detect();
        let prefs = crate::prefs::load(&home);
        // The language goes in force before the first frame, for the same reason
        // the theme is resolved here rather than when the pane is opened: a window
        // that painted English and then became German would flash a language the
        // reader did not choose. An empty tag is the setting nobody changed, and
        // an unknown one is a file from a future build; `locale::set` answers both
        // with English, so the fallback is in one place rather than two.
        crate::locale::set(prefs.locale.as_deref().unwrap_or_default());
        let theme = generated_theme(prefs.theme(), crate::native::system_prefers_light());
        let settings = RailSettings {
            hide_sidebar: prefs.hide_right_sidebar,
            show_skins: prefs.show_skin_selector_in_sidebar,
            show_screenshots: prefs.show_all_screenshots_in_sidebar,
        };
        // The engine's cache goes in this launcher's own `cache/meta/`, which is
        // where the launcher already keeps metadata it has fetched: a second cache
        // directory would be a second set of stale answers nobody knows about.
        let store = Store::load(&paths).with_engine(Engine::new(&paths));
        // The accounts are the launcher's own file in its data root, read once
        // per window: a launch signs in with whatever is selected, and a launch
        // that renews a session writes it back here.
        let (accounts, accounts_warning) =
            crate::accounts::AccountsStore::load_with_report(&paths.root.join("accounts.json"));
        let mut shell = Shell::new(flags.opening(), theme, &settings)
            .with_store(store)
            .with_prefs(paths.clone(), prefs)
            .with_accounts(accounts, accounts_warning);
        // A capture request is in place before the first frame, which is what
        // makes the settle timer count from the window opening rather than from
        // the first message to arrive after it.
        if let Some(path) = flags.shot.clone() {
            shell.shot = Some(path);
        }
        // The page a window opens on may owe a request before any message has
        // arrived -- `/browse/modpack` owes a search -- and the first frame is the
        // first moment there is anywhere to put the answer, so it is asked for
        // here rather than waited for.
        //
        // The panel's news feed rides along, and this is the only place it is asked
        // for: the panel is on every route, so a section that waited for a page to
        // owe it would be blank until the reader providentially visited one. Two
        // commands on one frame, each of them off the frame thread.
        let command = iced::Command::batch([shell.opening_command(), shell.news_command()]);
        (shell, command)
    }

    fn title(&self) -> String {
        brand::window_title()
    }

    fn update(&mut self, message: Message) -> iced::Command<Message> {
        self.handle(message)
    }

    fn view(&self) -> Element<'_, Message> {
        self.render()
    }

    fn theme(&self) -> Theme {
        widget_theme(self.theme)
    }

    fn subscription(&self) -> Subscription<Message> {
        // Six subscriptions and none is owed: the frame clock while something is
        // moving, the launch while a game is being started or is up, the window's
        // own state while Windows can change it, the window's own size while it can
        // be resized (see [`window_size`]), the settle timer while a capture is
        // waiting, and the frames a window owes its own first one until they have
        // arrived ([`REPAINT_KICKS`]). Each says `none` when it is not needed,
        // which is what keeps an idle window -- and a launcher with nothing running
        // -- from waking anything up.
        let moving = self.animating();
        let frames = if moving { self.frames() } else { Subscription::none() };

        Subscription::batch([
            frames,
            self.launching(),
            self.quick_create(),
            window_state(),
            window_size(),
            self.capture(),
            self.repaint(),
        ])
    }
}

impl Shell {
    /// One frame of the clock, while anything is moving.
    fn frames(&self) -> Subscription<Message> {
        if !self.animating() {
            return Subscription::none();
        }
        // A thread and a channel rather than `iced::time::every`, which needs a
        // futures-runtime feature this build does not enable -- the same
        // arrangement the old shell's frame clock uses, and for the same reason.
        // The subscription's life is the animation's: while nothing is moving it
        // is not asked for, iced drops the receiver, and the thread's next send
        // fails and ends it.
        // The subscription's life is the animation's: while nothing is moving it
        // is not asked for, iced drops the receiver, and the thread's next send
        // fails and ends it. That is also what keeps an idle window from waking
        // the GPU sixty times a second to redraw the same picture.
        iced::subscription::channel(FRAME_ID, 4, |mut sender| async move {
            let _ = std::thread::spawn(move || loop {
                std::thread::sleep(FRAME);
                match sender.try_send(Message::Tick) {
                    Ok(()) => {}
                    // Full: the UI is a frame or two behind. Frames are
                    // droppable, so the animation skips ahead.
                    Err(error) if error.is_full() => {}
                    Err(_) => break,
                }
            });
            loop {
                futures::future::pending::<()>().await;
            }
        })
    }

    /// The frames a window owes its own first one, if it has not drawn them yet.
    ///
    /// A thread and a channel rather than `iced::time::every` is the arrangement
    /// the capture's settle timer uses, and for its reason; what is different here
    /// is that the thread sends twice and then ends, and the subscription ends with
    /// it -- iced drops the receiver once the shell stops asking, and the thread's
    /// next send fails. See [`REPAINT_KICKS`] for what the two frames are for.
    fn repaint(&self) -> Subscription<Message> {
        if !self.repainting() {
            return Subscription::none();
        }
        iced::subscription::channel(REPAINT_ID, 1, |mut sender| async move {
            let _ = std::thread::spawn(move || {
                let mut elapsed = Duration::ZERO;
                for at in KICK_AFTER {
                    std::thread::sleep(at.saturating_sub(elapsed));
                    elapsed = at;
                    if sender.try_send(Message::Repaint).is_err() {
                        return;
                    }
                }
            });
            loop {
                futures::future::pending::<()>().await;
            }
        })
    }

    /// Whether any of [`REPAINT_KICKS`]'s frames is still owed.
    fn repainting(&self) -> bool {
        self.kicks < REPAINT_KICKS
    }

    /// The capture a `--shot` run asked for: one settle timer, then the frame.
    ///
    /// The settle is not politeness: a window asked for its pixels on its first
    /// frame would capture whatever had been laid out by then, and a page that
    /// reads a file or asks a service is not finished by its first frame. The
    /// timer is a thread for the same reason the frame clock is one, and it ends
    /// with the subscription: iced drops the receiver once the run has its
    /// answer, and the thread's next send fails.
    fn capture(&self) -> Subscription<Message> {
        if self.shot.is_none() || self.shot_taken {
            return Subscription::none();
        }
        iced::subscription::channel(SHOT_ID, 1, |mut sender| async move {
            let _ = std::thread::spawn(move || {
                std::thread::sleep(SHOT_SETTLE);
                let _ = sender.try_send(Message::ShotDue);
            });
            loop {
                futures::future::pending::<()>().await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{InstanceTab, ProjectTab, Route};

    /// The reference's root font size. Every `rem` in the geometry table is
    /// measured against it, and `theme_gen` converts lengths at the same 16.
    const ROOT: f32 = 16.0;

    /// Apply a message the way the runtime would, and drop the command it
    /// returns.
    ///
    /// `handle` returns a `Command` because it is the body of `update`, and in
    /// a test there is no runtime to run one: every command the shell produces
    /// today is `Command::none`, and a gate that asserts a `Command::none` is
    /// testing the toolkit's own constructor. `#[must_use]` on `Command` is
    /// what makes the drop explicit rather than an oversight.
    fn press(shell: &mut Shell, message: Message) {
        let _ = shell.handle(message);
    }

    #[test]
    fn hiding_what_is_installed_is_one_facet_group_and_not_a_row_filter() {
        // One group, because `facets` is a list of *or* groups: `[["project_id
        // NOT IN ["A"]]]` and `[["project_id NOT IN ["A"],["project_id NOT IN
        // ["B"]]]]` would ask for a project matching both exclusions, which is
        // every project except none of them.
        assert_eq!(
            exclusion_facet(&["AANobbMI".to_string()]),
            Some("project_id NOT IN [\"AANobbMI\"]".to_string())
        );
        assert_eq!(
            exclusion_facet(&["AANobbMI".to_string(), "P7dR8mSH".to_string()]),
            Some("project_id NOT IN [\"AANobbMI\",\"P7dR8mSH\"]".to_string())
        );
        // A launcher with nothing installed has hidden nothing, and `facets=[[]]`
        // would be a request for a project that satisfies no facet -- an empty
        // answer wearing the shape of a filter.
        assert_eq!(exclusion_facet(&[]), None);
    }

    fn shell_at(address: &str) -> Shell {
        Shell::new(
            Address::parse(address).expect("a sample address"),
            Gen::Dark,
            &RailSettings::default(),
        )
    }

    fn settings(hide_sidebar: bool, show_skins: bool, show_screenshots: bool) -> RailSettings {
        RailSettings { hide_sidebar, show_skins, show_screenshots }
    }

    #[test]
    fn a_wheel_from_any_of_the_three_shapes_reaches_the_region_it_names() {
        // The wire this slice exists for. iced hands a wheel to the content of a
        // `Scrollable` before it applies it itself, so a region's guard takes it
        // and publishes it as its own message -- and the three shapes that message
        // can arrive in have to land in `Shell::handle`, because that is the only
        // place that can answer with the command that moves a region. A wheel that
        // reached `act` instead would be a wheel that did nothing.
        let wheel = crate::scroll::Wheel {
            notches: -1.0,
            content_height: 2_000.0,
            view_height: 500.0,
            offset: 0.0,
        };
        let mut shell = shell_at("/");
        // A page's region: what `page::body`'s guard publishes, mapped through the
        // page's own message and `pages::Message` on its way up.
        press(
            &mut shell,
            Message::Screen(pages::Message::Home(home::Message::Wheel(
                crate::scroll::PAGE,
                wheel,
            ))),
        );
        assert_eq!(
            shell.glides.anim(crate::scroll::PAGE).target,
            crate::scroll::WHEEL_PIXELS_PER_NOTCH
        );
        // One of the shell's own, which raises the shell's message directly.
        press(&mut shell, Message::Wheel(crate::scroll::PANEL, wheel));
        assert_eq!(
            shell.glides.anim(crate::scroll::PANEL).target,
            crate::scroll::WHEEL_PIXELS_PER_NOTCH
        );
        // And one of the instance-settings modal's three lists, which raises its
        // own message inside the shell's.
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Wheel(
                crate::scroll::LOADER_BUILDS,
                wheel,
            )),
        );
        assert_eq!(
            shell.glides.anim(crate::scroll::LOADER_BUILDS).target,
            crate::scroll::WHEEL_PIXELS_PER_NOTCH
        );
        // And the frame clock is what carries all three, which is the other half
        // of the wire: a glide nobody draws is a glide that never happened.
        assert!(shell.animating());
    }

    #[test]
    fn the_geometry_is_the_reference_s_own_arithmetic() {
        // `4rem` and `3rem` at a 16px root, and the panel's one absolute width.
        assert_eq!(RAIL, 4.0 * ROOT);
        assert_eq!(BAR, 3.0 * ROOT);
        assert_eq!(PANEL, 300.0);
        // `.nav-button`'s `w-12 h-12` and the rail's `gap-[0.25rem]`: the 52px
        // pitch the old shell's gate recorded, kept as the sum of its parts so
        // that changing either one moves it.
        assert_eq!(PLATE, 12.0 * 4.0);
        assert_eq!(RAIL_GAP, 0.25 * ROOT);
        assert_eq!(RAIL_PITCH, 52.0);
        // `.app-contents`'s corner is `var(--radius-xl)`, which is the
        // generated table's own 20px rather than a number written here twice.
        assert_eq!(PAGE_RADIUS, theme_gen::span(crate::theme_gen::Span::RadiusXl));
        // The `md` icon-only `IconButton` and the head's `!h-7 !w-7` buttons.
        assert_eq!(CONTROLS_BUTTON, 9.0 * 4.0);
        assert_eq!(CONTROLS_ICON, 5.0 * 4.0);
        assert_eq!(HEAD_BUTTON, 7.0 * 4.0);
        assert_eq!(HEAD_ICON, 4.0 * 4.0);
        assert_eq!(LOGO, 7.0 * 4.0);
    }

    #[test]
    fn the_window_controls_reserve_what_they_measure() {
        // `--window-controls-width`, which the reference publishes from a
        // `ResizeObserver` and this shell computes: `px-1.5`, three `w-9`
        // buttons and two `gap-2`s.
        assert_eq!(CONTROLS_WIDTH, 136.0);
        assert_eq!(CONTROLS_WIDTH, 2.0 * 6.0 + 3.0 * 36.0 + 2.0 * 8.0);
        // The controls and the head are the same height, which is why drawing
        // them as the row's last item paints what the reference's overlay does.
        assert_eq!(BAR, 48.0);
    }

    #[test]
    fn every_rail_slot_draws_the_icon_app_vue_imports_for_it() {
        // `App.vue`'s own imports, by file name: PlayIcon, CompassIcon,
        // ShirtIcon, ImageIcon, ServerStackIcon, PlusIcon, SettingsIcon,
        // LogInIcon. A slot that reached for a different icon would still draw
        // *something*, which is exactly the kind of mistake this catches.
        let expected = [
            (Rail::Home, "play"),
            (Rail::Discover, "compass"),
            (Rail::Skins, "shirt"),
            (Rail::Screenshots, "image"),
            (Rail::Servers, "server-stack"),
            (Rail::CreateInstance, "plus"),
            (Rail::Settings, "settings"),
            (Rail::Profile, "log-in"),
        ];
        for (slot, name) in expected {
            assert_eq!(rail_glyph(slot).name(), name, "{slot} draws the wrong icon");
        }
        // And the head's own seven, which are drawn outside the rail.
        assert_eq!(Glyph::ChevronLeft.name(), "chevron-left");
        assert_eq!(Glyph::ChevronRight.name(), "chevron-right");
        assert_eq!(Glyph::RightArrow.name(), "right-arrow");
        assert_eq!(Glyph::Minimize.name(), "minimize");
        assert_eq!(Glyph::Maximize.name(), "maximize");
        assert_eq!(Glyph::Restore.name(), "restore");
        assert_eq!(Glyph::X.name(), "x");
    }

    #[test]
    fn the_rail_offers_the_slots_the_settings_ask_for() {
        // The reference's two conditional slots: `showSkinSelectorInSidebar` on
        // the shirt, which its own defaults turn on, and the screenshots sync
        // option on the image, which is on too. A fresh install of the reference
        // draws all eight.
        let all = shell_at("/").slots();
        assert_eq!(all.len(), Rail::ALL.len(), "a fresh install draws every slot");
        assert!(all.contains(&Rail::Skins));
        assert!(all.contains(&Rail::Screenshots));
        // The order is the reference's, whatever is hidden.
        assert_eq!(all.as_slice(), Rail::ALL);
        // Either slot can be turned off on its own, and the settings are the
        // only thing that changes the list.
        let mut shell =
            Shell::new(Address::at(Route::Home), Gen::Dark, &settings(false, false, true));
        assert!(!shell.slots().contains(&Rail::Skins));
        assert!(shell.slots().contains(&Rail::Screenshots));
        assert_eq!(shell.slots().len(), Rail::ALL.len() - 1, "the shirt is hidden");
        press(&mut shell, Message::Sidebar(true));
        assert_eq!(shell.slots().len(), Rail::ALL.len() - 1);
        let shell =
            Shell::new(Address::at(Route::Home), Gen::Dark, &settings(false, true, false));
        assert!(shell.slots().contains(&Rail::Skins));
        assert!(!shell.slots().contains(&Rail::Screenshots));
        assert_eq!(shell.slots().len(), Rail::ALL.len() - 1, "the image is hidden");
    }

    #[test]
    fn the_marks_come_from_the_route_table_and_the_plate_follows_them() {
        // Discover, marked primary: its plate has to arrive at full size.
        let mut shell = shell_at("/browse/modpack");
        assert!(shell.animating(), "the plate starts from zero and grows");
        shell.advance(Duration::from_millis(250));
        assert_eq!(shell.plate(Rail::Discover).value(), 1.0);
        assert!(!shell.animating());
        assert_eq!(shell.plate(Rail::Home).value(), 0.0, "home is not marked");

        // A project page is Discover's subpage: no plate anywhere.
        press(&mut shell, Message::Go("/project/sodium".into()));
        shell.advance(Duration::from_millis(250));
        assert_eq!(shell.plate(Rail::Discover).value(), 0.0);
        assert_eq!(shell.mark(Rail::Discover), Some(Mark::Subpage));
        assert_eq!(shell.plate(Rail::Home).value(), 0.0);

        // Browsing *inside* an instance moves the subpage mark to Home, which
        // is still not a plate: only a primary mark grows one.
        press(&mut shell, Message::Go("/browse/mod?i=atm10".into()));
        assert_eq!(shell.mark(Rail::Home), Some(Mark::Subpage));
        assert_eq!(shell.mark(Rail::Discover), None);
        assert_eq!(shell.plate(Rail::Home).value(), 0.0);

        // An instance page marks nothing at all -- the reference's own
        // predicates, reproduced: see `route::Address::marks`.
        press(&mut shell, Message::Go("/instance/atm10/worlds".into()));
        assert!(shell.address().marks().is_empty());
        assert_eq!(shell.plate(Rail::Servers).value(), 0.0);
    }

    #[test]
    fn a_window_is_asked_for_the_frames_it_owes_its_own_first_one() {
        // `REPAINT_KICKS`' own rule: the first frame a window presents has no page
        // in it on this machine, and a message is what makes iced build the next
        // one. The shell asks for exactly that pair and then stops, so an idle
        // launcher is not a launcher drawing forever.
        let mut shell = shell_at("/browse/modpack");
        assert!(shell.repainting(), "a window that has just opened owes a frame");
        press(&mut shell, Message::Repaint);
        assert!(shell.repainting(), "one kick is not the pair");
        press(&mut shell, Message::Repaint);
        assert!(!shell.repainting(), "and the pair is where the asking ends");
        press(&mut shell, Message::Repaint);
        assert!(!shell.repainting(), "a stray kick does not start it again");
        // The debt is a window's opening rather than a page's, so a shell that has
        // navigated since does not owe one either.
        press(&mut shell, Message::Go("/".into()));
        assert!(!shell.repainting());
    }

    #[test]
    fn a_create_that_cannot_run_keeps_the_dialog_and_says_why() {
        // What a reader must not get is a dialog that closed and nothing else: a
        // store with no launcher behind it is the smallest failure to check that
        // with, and it is the one a test can produce.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        press(&mut shell, Message::CreateName("Sodium test".to_string()));
        press(&mut shell, Message::Create);
        assert!(shell.creating, "the button is unusable while it is in flight");
        assert!(
            !shell.create_requested,
            "the flag is what `handle` takes to build the command, so it is already spent"
        );
        press(
            &mut shell,
            Message::Created(Err("give the instance a name".to_string())),
        );
        assert_eq!(shell.modal, Some(Modal::Create), "the dialog stays up");
        assert_eq!(shell.create_error.as_deref(), Some("give the instance a name"));
        assert!(!shell.creating, "and the button is usable again");
        // Typing again clears it: the sentence was about the name that changed.
        press(&mut shell, Message::CreateName("Sodium".to_string()));
        assert_eq!(shell.create_error, None);
    }

    #[test]
    fn the_import_step_says_when_this_machine_holds_nothing() {
        // A test's shell has no launcher behind it, so the scan finds nothing and
        // the dialog has to say so: an empty list with a title is the shape that
        // reads as "there is nothing here" rather than as a broken dialog.
        let mut shell = shell_at("/");
        press(
            &mut shell,
            Message::Screen(pages::Message::Home(pages::home::Message::ImportFromLauncher)),
        );
        assert_eq!(shell.modal, Some(Modal::Import));
        assert!(shell.import_found.is_empty());
        drop(shell.render());
    }

    /// A shell with a launcher behind it: a data root, so a launch has somewhere
    /// to run, and no account file, which is the offline session a fresh install
    /// starts with.
    fn shell_with_home(name: &str) -> Shell {
        let root = std::env::temp_dir().join("palantirmc-shell-launch").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch root");
        let paths = palantir_core::paths::PalantirPaths::at(root);
        Shell::new(Address::at(Route::Home), Gen::Dark, &RailSettings::default())
            .with_store(Store::load(&paths))
            .with_prefs(paths, crate::prefs::Prefs::default())
    }

    /// A shell whose launcher has one instance, so the install dialog has
    /// something to offer it.
    fn shell_with_instance(name: &str) -> Shell {
        let root = std::env::temp_dir().join("palantirmc-shell-install").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch root");
        let paths = palantir_core::paths::PalantirPaths::at(root);
        crate::instances::create(
            &paths,
            &crate::instances::NewInstance::vanilla("atm10", "1.21.4"),
        )
        .expect("an instance");
        Shell::new(Address::at(Route::Home), Gen::Dark, &RailSettings::default())
            .with_store(Store::load(&paths))
            .with_prefs(paths, crate::prefs::Prefs::default())
    }

    #[test]
    fn installing_a_project_asks_which_instance_and_hands_the_sentence_to_the_page() {
        let mut shell = shell_with_instance("install");
        // The page reports the press; the shell opens the dialog, because *which
        // instances exist* is not something a page holds.
        press(&mut shell, Message::Go("/project/sodium".into()));
        press(
            &mut shell,
            Message::Screen(pages::Message::Project(project::Message::Install)),
        );
        match &shell.modal {
            Some(Modal::Install { project, title, pack }) => {
                assert_eq!(project, "sodium");
                assert_eq!(title, "", "the page has drawn nothing yet, so it names no title");
                assert!(!pack, "and the page has not loaded, so it cannot say it is a pack");
            }
            other => panic!("the install dialog: {other:?}"),
        }
        // The page's own sentence is cleared by the press that replaces it, and
        // the dialog is where the press is answered.
        assert_eq!(shell.screen.project_notice(), None);

        // A row is the whole of the request: the id travels as a flag `handle`
        // turns into the transfer, marked busy as it leaves rather than when it
        // comes back -- the gap is where a second press would be.
        press(&mut shell, Message::InstallInto("atm10".into()));
        assert!(shell.installing, "busy as the request leaves");
        assert_eq!(shell.install_requested, None, "and taken by the command");
        // A second press in that window changes nothing.
        shell.install_error = Some("still going".to_string());
        press(&mut shell, Message::InstallInto("atm10".into()));
        assert_eq!(shell.install_error.as_deref(), Some("still going"));

        // The answer comes back as a sentence, and both halves of the shell hear
        // it: the dialog closes (it has nothing left to ask) and the page keeps
        // the line, because the button that asked is on the page.
        press(
            &mut shell,
            Message::Installed(Ok(store::Outcome::File(
                "Installed Sodium 0.6.5 into atm10".to_string(),
            ))),
        );
        assert_eq!(shell.modal, None, "the dialog is done");
        assert!(!shell.installing);
        assert_eq!(shell.screen.project_notice(), Some("Installed Sodium 0.6.5 into atm10"));

        // A failure keeps the dialog up, because the answer to it is another
        // instance -- which is a choice drawn in the dialog and nowhere else.
        press(
            &mut shell,
            Message::Screen(pages::Message::Project(project::Message::Install)),
        );
        press(&mut shell, Message::InstallInto("atm10".into()));
        press(
            &mut shell,
            Message::Installed(Err("Sodium has no version for Fabric 1.21.4".to_string())),
        );
        assert_eq!(shell.install_error.as_deref(), Some("Sodium has no version for Fabric 1.21.4"));
        assert!(
            matches!(shell.modal, Some(Modal::Install { .. })),
            "the reader picks another instance rather than being told to start over"
        );
        assert_eq!(
            shell.screen.project_notice(),
            Some("Sodium has no version for Fabric 1.21.4"),
            "and the sentence stays after the dialog is dismissed"
        );
        // Every theme, with and without an instance to offer: the empty state is
        // the one arm the row loop cannot reach.
        for theme in Gen::ALL {
            shell.theme = *theme;
            drop(shell.render());
            shell.store = Store::default();
            drop(shell.render());
        }
    }

    #[test]
    fn the_install_dialog_offers_the_creation_flow_when_there_is_nothing_to_install_into() {
        // The one press a launcher with no instances owes: the flow that makes
        // one. It opens the creation dialog rather than installing anywhere,
        // which is what the reference's welcome screen does with the same words.
        let mut shell = shell_with_home("install-empty");
        press(&mut shell, Message::Go("/project/sodium".into()));
        press(
            &mut shell,
            Message::Screen(pages::Message::Project(project::Message::Install)),
        );
        assert!(matches!(shell.modal, Some(Modal::Install { .. })));
        press(&mut shell, Message::OpenCreate);
        assert_eq!(shell.modal, Some(Modal::Create));
        assert!(
            shell.install_requested.is_none(),
            "and nothing was installed on the way"
        );
    }

    #[test]
    fn the_install_dialog_s_words_are_the_reference_s_own() {
        // The three strings the dialog adds are existing keys rather than new
        // copy, and they are asserted here because a generator that moved under
        // them would otherwise be a dialog with the wrong words in it.
        assert_eq!(Key::AppUserProjectInstallToInstance.message(), "Install to instance");
        assert_eq!(Key::AppProjectVersionInstalling.message(), "Installing");
        assert_eq!(Key::AppWelcomeScreenCreateInstance.message(), "Create an instance");
        assert_eq!(Key::AppLibraryGroupNoInstancesFound.message(), "No instances found");
    }

    #[test]
    fn a_pack_is_asked_for_once_and_the_reader_is_left_in_the_instance_it_made() {
        // The pack path: the dialog draws one action rather than a list of
        // instances -- a pack has no folder to land in, so there is nothing to
        // choose -- and the answer does not just print a sentence, it makes an
        // instance and takes the reader to it, which is what the reference does.
        let mut shell = shell_with_instance("install-pack");
        press(&mut shell, Message::Go("/project/cobblemon".into()));
        press(
            &mut shell,
            Message::Screen(pages::Message::Project(project::Message::Install)),
        );
        // The page has drawn nothing, so the shell cannot know it is a pack: this
        // is the same dialog the file install gets.
        assert!(matches!(shell.modal, Some(Modal::Install { pack: false, .. })));
        // Whatever the page said, the pack action is the one that runs it, and a
        // pack action from a dialog that is not a pack's does nothing at all.
        press(&mut shell, Message::InstallPack);
        assert!(!shell.installing, "a file install is not a pack install");

        // The page says what it is, and the dialog changes shape with it.
        shell.modal = Some(Modal::Install {
            project: "cobblemon".to_string(),
            title: "Cobblemon".to_string(),
            pack: true,
        });
        press(&mut shell, Message::InstallPack);
        assert!(shell.installing, "busy as the request leaves");
        assert_eq!(shell.pack_requested, None, "and taken by the command");
        drop(shell.render());

        // The answer names the instance it made, and the reader lands in it with
        // the list of instances read again -- a new instance is on disk and not on
        // any page until then.
        press(
            &mut shell,
            Message::Installed(Ok(store::Outcome::Pack {
                id: "Cobblemon".to_string(),
                line: "Installed Cobblemon as Cobblemon, 12 files".to_string(),
            })),
        );
        assert_eq!(shell.modal, None, "the dialog is done");
        assert!(!shell.installing);
        assert_eq!(
            shell.screen.project_notice(),
            None,
            "and no line is left on a page the reader has been moved off"
        );
        // The instance itself is the store's doing, and `store.rs` is where it is
        // asserted; what the shell owes is the address. (Nothing is installed here
        // -- the outcome is injected -- so there is no folder to find either way.)
        assert_eq!(
            shell.address().route,
            Route::Instance { id: "Cobblemon".to_string(), tab: route::InstanceTab::Content },
            "the reader is left looking at the instance the outcome named"
        );

        // A pack that cannot be installed keeps the dialog, for the file path's
        // reason and a stronger one: there is no second place to try, so the
        // sentence is the whole of the answer.
        shell.modal = Some(Modal::Install {
            project: "cobblemon".to_string(),
            title: "Cobblemon".to_string(),
            pack: true,
        });
        press(&mut shell, Message::InstallPack);
        press(
            &mut shell,
            Message::Installed(Err("that archive is not a readable pack".to_string())),
        );
        assert_eq!(
            shell.install_error.as_deref(),
            Some("that archive is not a readable pack")
        );
        assert!(matches!(shell.modal, Some(Modal::Install { pack: true, .. })));
        for theme in Gen::ALL {
            shell.theme = *theme;
            drop(shell.render());
        }
    }

    /// One article, in the feed's own shape.
    fn news_article(title: &str, link: &str, date: &str) -> NewsArticle {
        NewsArticle {
            title: title.to_string(),
            summary: format!("{title}, in a sentence."),
            thumbnail: String::new(),
            date: date.to_string(),
            link: link.to_string(),
        }
    }

    #[test]
    fn the_panel_draws_the_four_newest_articles_and_the_way_to_the_rest() {
        let mut shell = shell_with_home("panel-news");
        // Nothing is drawn before the feed arrives, and nothing is drawn for a feed
        // that never does: the reference's own `v-if` is `news.length > 0`, and an
        // unreachable feed is not an error a reader can act on.
        assert!(shell.news_shown().is_empty());
        shell.news = Load::Failed("no network".to_string());
        assert!(shell.news_shown().is_empty());
        assert!(shell.news_section().is_none());

        shell.news = Load::Ready(
            (0..6)
                .map(|i| {
                    news_article(
                        &format!("Article {i}"),
                        &format!("https://modrinth.com/news/article/{i}"),
                        &format!("2026-09-0{}T19:00:00.000Z", i + 1),
                    )
                })
                .collect(),
        );
        // Four, in the feed's own order -- the reference's own slice of it -- and
        // the fifth and sixth are not drawn at all.
        let shown = shell.news_shown();
        assert_eq!(shown.len(), MAX_NEWS, "the panel draws a fixed number of cards");
        assert_eq!(shown[0].title, "Article 0");
        assert_eq!(shown[3].title, "Article 3");
        assert!(shell.news_section().is_some());

        // An article with no title, and one whose link this launcher will not open,
        // are not cards: a press hands the URL to the operating system, so a card
        // that could not be opened would be a control that does nothing.
        shell.news = Load::Ready(vec![
            news_article("", "https://modrinth.com/news/article/untitled", "2026-09-01T00:00:00.000Z"),
            news_article("A local file", "file:///etc/passwd", "2026-09-01T00:00:00.000Z"),
            news_article("Openable", "https://modrinth.com/news/article/good", "2026-09-02T00:00:00.000Z"),
        ]);
        assert_eq!(shell.news_shown().len(), 1);

        // The one press a test can make without starting a browser: a link the
        // opener refuses. It is said under the section rather than dropped, and the
        // dismiss clears it.
        press(&mut shell, Message::OpenUrl("file:///etc/passwd".to_string()));
        let note = shell.link_note.clone().expect("a sentence");
        assert!(note.contains("is not a link this launcher will open"), "{note}");
        press(&mut shell, Message::DismissLinkNote);
        assert!(shell.link_note.is_none());

        // And the section is drawn in every theme, with a feed and without one.
        for theme in Gen::ALL {
            shell.theme = *theme;
            drop(shell.render());
            shell.news = Load::Loading;
            drop(shell.render());
        }
    }

    /// Ask the instance page on screen to run its instance.
    fn play(shell: &mut Shell) {
        press(
            shell,
            Message::Screen(pages::Message::Instance(pages::instance::Message::Play)),
        );
    }

    #[test]
    fn playing_an_instance_starts_a_run_the_pages_can_see() {
        // The whole of what the Play button does: the page reports it, the shell
        // remembers the run -- which is what the subscription reads -- and the
        // store says so, which is what the header's control is drawn from.
        let mut shell = shell_with_home("play");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        let run = shell.runs.first().expect("a run").data.clone();
        assert_eq!(run.instance_id, "atm10");
        assert_eq!(run.run_id, 1);
        assert_eq!(run.data_root, shell.home.as_ref().expect("a home").root);
        assert!(
            !run.account.kind.is_online(),
            "no account chosen is the offline session, not a refusal to launch"
        );
        assert_eq!(run.account.username, "Player", "which is the same player every time");
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Starting);
        assert_eq!(shell.store.launch_line("atm10"), Some("preparing 'atm10'"));
        // A second instance's page asks for *that* instance, and the shell runs
        // both: the address is what the page is drawn from, and one launcher can
        // have two games going. What it does not do is run the same instance
        // twice.
        press(&mut shell, Message::Go("/instance/other".into()));
        play(&mut shell);
        let ids: Vec<&str> = shell.runs.iter().map(|run| run.data.instance_id.as_str()).collect();
        assert_eq!(ids, vec!["atm10", "other"], "a run per press, and one per instance");
        assert_eq!(shell.next_run_id, 2, "each run has an id of its own");
        play(&mut shell);
        assert_eq!(shell.runs.len(), 2, "a launch is not started twice behind one game");
    }

    #[test]
    fn a_run_reports_what_it_is_doing_and_the_header_follows() {
        // Every fact a launch sends has exactly one place to land, and the run id
        // is checked first: a `Done` from the run before must not clear the run
        // that is going, which is a stale frame the shell has to survive.
        let mut shell = shell_with_home("events");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Progress {
                run_id: 1,
                progress: crate::install::Progress::new("libraries", 3, 12, 0),
            }),
        );
        assert_eq!(shell.store.launch_line("atm10"), Some("libraries 3/12 (25%)"));
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Log {
                run_id: 1,
                lines: vec!["resolving version 1.21.4".to_string()],
            }),
        );
        assert_eq!(shell.store.launch_line("atm10"), Some("resolving version 1.21.4"));
        press(&mut shell, Message::Launched(launch::LaunchEvent::Started { run_id: 1 }));
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Running);
        assert_eq!(
            shell.store.launch_line("atm10"),
            Some("resolving version 1.21.4"),
            "the game coming up changes the control and not the run's own line"
        );
        // And the stale frame: a done from a run this shell has already replaced.
        press(&mut shell, Message::Launched(launch::LaunchEvent::Done {
            run_id: 1,
            note: "process exited (exit status: 0)".to_string(),
        }));
        assert!(shell.runs.is_empty(), "the run is over");
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Idle);
        assert_eq!(
            shell.store.launch_line("atm10"),
            Some("process exited (exit status: 0)"),
            "and the last thing it said is still what the header shows"
        );
        play(&mut shell);
        assert_eq!(shell.runs.first().map(|run| run.data.run_id), Some(2));
        press(&mut shell, Message::Launched(launch::LaunchEvent::Done {
            run_id: 1,
            note: "stale".to_string(),
        }));
        assert!(!shell.runs.is_empty(), "a stale done must not clear the run that is going");
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Progress {
                run_id: 1,
                progress: crate::install::Progress::new("assets", 1, 2, 0),
            }),
        );
        assert_eq!(
            shell.store.launch_line("atm10"),
            Some("preparing 'atm10'"),
            "and a stale line must not reach the run that replaced it"
        );
    }

    #[test]
    fn stopping_is_a_state_the_header_can_draw_and_a_kill_the_shell_can_send() {
        // Stop is two things at once, and the test asserts both: the store says
        // the run is stopping, which is what the button is drawn from, and the
        // shell has asked the child to die. There is no child in a test -- the
        // slot is empty -- so what is asserted is that the request is harmless
        // and that the state is set before the answer comes back.
        //
        // The game has to be *up* first, and that is the behaviour this test now
        // pins rather than an inconvenience: a stop while the launcher is still
        // preparing has no child to kill, and setting *Stopping* for it would
        // leave the run to start the game under a button that said it was being
        // taken down. Pressing it before the run is up is refused instead, which
        // is why the instance page draws the button unusable in that state.
        let mut shell = shell_with_home("stop");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(pages::instance::Message::Stop)),
        );
        assert_eq!(
            shell.store.launch_state("atm10"),
            store::LaunchState::Starting,
            "nothing to kill yet"
        );
        press(&mut shell, Message::Launched(launch::LaunchEvent::Started { run_id: 1 }));
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Running);
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(pages::instance::Message::Stop)),
        );
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Stopping);
        assert!(!shell.runs.is_empty(), "the run is not over until the worker says so");
        // A stop for an instance that is not the one running is ignored rather
        // than killing the wrong game, and so is a second one for the same run.
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(pages::instance::Message::Stop)),
        );
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Stopping);
    }

    #[test]
    fn a_renewed_session_is_written_back_to_the_accounts_file() {
        // A launch renews the Microsoft session inside the worker, because it
        // cannot wait for the window to notice; storing it is the window's,
        // because the file is. The test asserts the file, not the struct: a
        // store that was updated and never saved is the same as one that was
        // never updated.
        let root = std::env::temp_dir().join("palantirmc-shell-launch").join("accounts");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch root");
        let path = root.join("accounts.json");
        let mut accounts = crate::accounts::AccountsStore::load_with_report(&path).0;
        accounts
            .upsert_microsoft(crate::accounts::AccountEntry::microsoft(
                "Notch",
                "069a79f444e94726a5befca90e38aaf5",
                "old-token",
                "old-refresh",
                0,
                Some(true),
            ))
            .expect("an account");
        accounts.select("069a79f444e94726a5befca90e38aaf5").expect("selected");
        accounts.save().expect("written");
        let paths = palantir_core::paths::PalantirPaths::at(root.clone());
        let mut shell = Shell::new(Address::at(Route::Home), Gen::Dark, &RailSettings::default())
            .with_store(Store::load(&paths))
            .with_prefs(paths, crate::prefs::Prefs::default())
            .with_accounts(accounts, None);
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        assert!(
            shell.runs.first().expect("a run").data.account.kind.is_online(),
            "a Microsoft account launches with its own session"
        );
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Tokens(launch::RefreshedTokens {
                uuid: "069a79f444e94726a5befca90e38aaf5".to_string(),
                name: "Notch".to_string(),
                access_token: "new-token".to_string(),
                refresh_token: Some("new-refresh".to_string()),
                expires_at_ms: 42,
            })),
        );
        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(written.contains("new-token"), "the renewal did not reach the disk: {written}");
        assert!(written.contains("new-refresh"), "{written}");
        // And a renewal for an account this launcher does not know is dropped
        // rather than invented.
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Tokens(launch::RefreshedTokens {
                uuid: "nobody".to_string(),
                name: "Nobody".to_string(),
                access_token: "x".to_string(),
                refresh_token: None,
                expires_at_ms: 1,
            })),
        );
        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(!written.contains("\"x\""), "{written}");
    }

    #[test]
    fn a_shell_with_nowhere_to_launch_says_so_rather_than_starting_nothing() {
        // A test's shell has no data root, so there is nowhere for an instance to
        // be: the honest answer is the sentence every other unwired control in
        // this rewrite gives, and the run is not started.
        let mut shell = shell_at("/instance/atm10");
        play(&mut shell);
        assert!(shell.runs.is_empty());
        assert_eq!(
            shell.store.launch_line("atm10"),
            Some(store::not_implemented("Launching an instance").as_str())
        );
    }

    /// A version list in the shape Mojang publishes: two releases, a snapshot and
    /// an old beta, newest first.
    fn version_list() -> store::VersionList {
        store::VersionList {
            latest_release: "1.21.4".to_string(),
            versions: [("25w02a", false), ("1.21.4", true), ("1.21.3", true), ("b1.7.3", false)]
                .into_iter()
                .map(|(id, release)| store::GameVersion { id: id.to_string(), release })
                .collect(),
        }
    }

    /// The versions the picker would draw, in the order it draws them.
    fn version_ids(shell: &Shell) -> Vec<&str> {
        shell.version_matches().into_iter().map(|version| version.id.as_str()).collect()
    }

    #[test]
    fn the_action_bar_follows_the_run_from_any_page_and_stops_what_is_running() {
        // The surface the plan names: a run used to be watchable only in the
        // header of the instance it belonged to, which meant navigating away from
        // it hid it. The bar is drawn from the store, so the run follows the
        // reader instead -- and the two facts it carries are the two the reference
        // carries: what is running, and how to stop it.
        let mut shell = shell_with_home("action-bar");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Starting);
        press(&mut shell, Message::Go("/browse/modpack".into()));
        assert_eq!(
            shell.store.launch_state("atm10"),
            store::LaunchState::Starting,
            "the run is the launcher's, not the page's"
        );
        assert_eq!(state_label(store::LaunchState::Starting), Some("Starting..."));
        assert_eq!(state_label(store::LaunchState::Stopping), Some("Stopping..."));
        assert_eq!(state_label(store::LaunchState::Running), None, "nothing to say");
        assert_eq!(state_label(store::LaunchState::Idle), None, "and nothing running");
        // The bar's own stop, while the launcher is still *preparing*: there is no
        // child process to kill yet, so the control is not drawn and this message
        // is not one it can send. What a press would do otherwise is change a word
        // and leave the run to start the game anyway.
        press(&mut shell, Message::StopRun("atm10".to_string()));
        assert_eq!(
            shell.store.launch_state("atm10"),
            store::LaunchState::Starting,
            "nothing to stop yet"
        );
        press(&mut shell, Message::Launched(launch::LaunchEvent::Started { run_id: 1 }));
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Running);
        press(&mut shell, Message::StopRun("atm10".to_string()));
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Stopping);
        // And the name the chip draws: the library's own when it has been read,
        // the id when it has not -- which is the arm an instance removed while it
        // was running lands in.
        assert_eq!(
            instance_name(&shell.store, "atm10"),
            "atm10",
            "the id, for an instance the library has not read"
        );
        drop(shell.render());
    }

    #[test]
    fn the_download_panel_is_the_launch_s_own_level_and_it_goes_when_the_fetch_does() {
        // The panel is drawn from the level the run reported, which is the reason
        // `LaunchEvent::Progress` is kept as numbers rather than only as the
        // sentence the instance header shows.
        let mut shell = shell_with_home("downloads");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        press(&mut shell, Message::ToggleDownloads);
        assert!(shell.downloads, "the chip is the panel's own toggle");
        assert!(shell.jobs.is_empty(), "and there is nothing to show yet");
        drop(shell.render());
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Progress {
                run_id: 1,
                progress: crate::install::Progress::new("files", 3, 12, 6 * 1024 * 1024),
            }),
        );
        let progress = shell.jobs.get("atm10").cloned().expect("the level");
        assert_eq!(progress.fraction(), 0.25);
        assert_eq!(progress_line(&progress), "files: 3 of 12 · 6.0 MB");
        // An indeterminate level says so in words rather than inventing a
        // fraction: the launcher's own "loading" states have no countable total.
        assert_eq!(
            progress_line(&crate::install::Progress::starting("installing Java")),
            "installing Java…"
        );
        drop(shell.render());
        // The game coming up is the end of the fetch: the level goes with it, and
        // so does the panel a bar would have been left in.
        press(&mut shell, Message::Launched(launch::LaunchEvent::Started { run_id: 1 }));
        assert!(shell.jobs.is_empty(), "the level goes with the fetch that set it");
        assert!(!shell.downloads);
        // A run that ends clears both too, for the same reason.
        press(&mut shell, Message::ToggleDownloads);
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Progress {
                run_id: 1,
                progress: crate::install::Progress::starting("natives"),
            }),
        );
        press(&mut shell, Message::Launched(launch::LaunchEvent::Done {
            run_id: 1,
            note: "process exited (exit status: 0)".to_string(),
        }));
        assert!(shell.jobs.is_empty());
        assert!(!shell.downloads);
        assert!(shell.runs.is_empty());
    }

    #[test]
    fn a_second_run_is_listed_by_the_bar_and_the_chip_follows_the_row_that_is_pressed() {
        // The reference's popover: several processes at once, one of them the one
        // the chip is about, and pressing a row is what makes it that one. The
        // chevron over it is drawn only when there is something to switch
        // between, which is the condition this asserts the inputs of.
        let mut shell = shell_with_home("switchers");
        for id in ["atm10", "sodium"] {
            press(&mut shell, Message::Go(format!("/instance/{id}")));
            play(&mut shell);
        }
        let running: Vec<&str> = shell
            .store
            .running_launches()
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(running, vec!["atm10", "sodium"], "both runs are the bar's to list");
        assert_eq!(
            shell.store.selected_launch(),
            Some("sodium"),
            "the run that was just started is the one the chip is about"
        );
        // The popover is off until the chevron is pressed, and it is drawn only
        // while there is a second run to switch to -- which is also the chevron's
        // own condition in the chip.
        assert!(!shell.switchers);
        press(&mut shell, Message::ToggleRuns);
        assert!(shell.switchers);
        drop(shell.render());
        // Pressing a row makes that process the one the chip -- and with it the
        // stop control and the logs button -- is about.
        press(&mut shell, Message::SelectRun("atm10".to_string()));
        assert_eq!(shell.store.selected_launch(), Some("atm10"));
        // And the last run's end takes the popover with it: a list of one is not
        // a list, so what is left is a bar with nothing to switch between.
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Done {
                run_id: 2,
                note: "process exited (exit status: 0)".to_string(),
            }),
        );
        assert_eq!(shell.store.running_launches().len(), 1);
        drop(shell.render());
    }

    #[test]
    fn stopping_one_run_leaves_the_other_one_going() {
        // The reason the shell holds a list of runs rather than one: a stop is
        // aimed at an *instance*, and the other game must not be touched by it --
        // not its state, and not the run the subscription is streaming.
        let mut shell = shell_with_home("two-runs");
        for id in ["atm10", "sodium"] {
            press(&mut shell, Message::Go(format!("/instance/{id}")));
            play(&mut shell);
        }
        for run_id in [1, 2] {
            press(&mut shell, Message::Launched(launch::LaunchEvent::Started { run_id }));
        }
        press(&mut shell, Message::StopRun("atm10".to_string()));
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Stopping);
        assert_eq!(
            shell.store.launch_state("sodium"),
            store::LaunchState::Running,
            "the other game is not the one that was stopped"
        );
        assert_eq!(shell.runs.len(), 2, "and both runs are still the shell's");
        // The stopped run ends. The other keeps its state and its own run id, and
        // a done for a run that is already gone changes nothing at all -- which
        // is the same stale-frame rule the single run had, now that there is more
        // than one to get it wrong for.
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Done {
                run_id: 1,
                note: "process exited (exit status: 0)".to_string(),
            }),
        );
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Idle);
        assert_eq!(shell.store.launch_state("sodium"), store::LaunchState::Running);
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Done {
                run_id: 9,
                note: "stale".to_string(),
            }),
        );
        assert_eq!(shell.store.launch_state("sodium"), store::LaunchState::Running);
        assert_eq!(shell.runs.len(), 1);
    }

    #[test]
    fn the_download_chip_shows_the_selected_run_s_phase_or_whatever_is_fetching() {
        // Two runs can be fetching at once, and the chip is drawn from the
        // selected run's phase when it has one and from whatever is fetching
        // otherwise: a chip that went blank because the run it names stopped
        // downloading would hide work that is happening.
        let mut jobs = BTreeMap::new();
        jobs.insert("atm10".to_string(), install::Progress::new("libraries", 1, 4, 0));
        jobs.insert("sodium".to_string(), install::Progress::new("assets", 1, 2, 0));
        assert_eq!(
            shown_job(&jobs, Some("sodium")).map(|(id, _)| id),
            Some("sodium"),
            "the selected run's own phase wins"
        );
        assert_eq!(
            shown_job(&jobs, Some("other")).map(|(id, _)| id),
            Some("atm10"),
            "an instance that is not fetching falls back to one that is"
        );
        assert_eq!(shown_job(&jobs, None).map(|(id, _)| id), Some("atm10"));
        assert!(shown_job(&BTreeMap::new(), Some("atm10")).is_none());
    }

    #[test]
    fn the_download_panel_carries_a_row_for_every_job_it_is_holding() {
        // "More than the run's own job" is the whole of what this panel is for:
        // `jobs` is keyed by instance, so two runs fetching at once are two rows
        // and a head that counts two. A row is an element nothing can read back,
        // so what is asserted is the state the rows are built from -- and that the
        // surface draws in it, which is the only way a mistake in a two-row panel
        // can fail from here.
        let mut shell = shell_with_home("job-list");
        for id in ["atm10", "sodium"] {
            press(&mut shell, Message::Go(format!("/instance/{id}")));
            play(&mut shell);
        }
        for (run_id, label) in [(1, "libraries"), (2, "assets")] {
            press(
                &mut shell,
                Message::Launched(launch::LaunchEvent::Progress {
                    run_id,
                    progress: crate::install::Progress::new(label, 1, 4, 0),
                }),
            );
        }
        let levels: Vec<&str> = shell.jobs.keys().map(String::as_str).collect();
        assert_eq!(levels, vec!["atm10", "sodium"], "one job per run that is fetching");
        // The job arrives with the run, so the chip -- and the row the panel marks
        // -- is about the instance that was just started.
        assert_eq!(shell.store.selected_launch(), Some("sodium"));
        press(&mut shell, Message::ToggleDownloads);
        assert!(shell.downloads);
        drop(shell.render());
        // A row is a `SelectRun`, which is what makes this a way to *choose* a job
        // rather than only to read one: the chip follows the row that is pressed,
        // exactly as it follows a row of the popover above.
        press(&mut shell, Message::SelectRun("atm10".to_string()));
        assert_eq!(shell.store.selected_launch(), Some("atm10"));
        drop(shell.render());
        // And a row goes with the run that put it there: both runs end, the map
        // empties, and an empty map is a panel that is not drawn at all even with
        // its flag still up.
        for run_id in [1, 2] {
            press(
                &mut shell,
                Message::Launched(launch::LaunchEvent::Done {
                    run_id,
                    note: "process exited (exit status: 0)".to_string(),
                }),
            );
        }
        assert!(shell.jobs.is_empty());
        drop(shell.render());
    }

    #[test]
    fn the_creation_dialog_asks_mojang_for_the_versions_it_offers() {
        // The dialog cannot offer a version it has not been told about, and the
        // list is a request rather than a field read -- so opening the dialog sets
        // the waiting state and the flag that carries the request out of `act`,
        // which `handle` spends on the way to building it.
        let mut shell = shell_at("/");
        let _ = shell.act(Message::Rail(Rail::CreateInstance));
        assert_eq!(shell.modal, Some(Modal::Create));
        assert!(matches!(shell.versions, Load::Loading), "the dialog waits on its own request");
        assert!(shell.versions_requested, "and the request has not left `act` yet");
        press(&mut shell, Message::Versions(Ok(version_list())));
        assert_eq!(shell.versions.ready(), Some(&version_list()));
        assert!(
            !shell.versions_requested,
            "the flag is what `handle` takes to build the command, so it is already spent"
        );
        // And the picker opens on Mojang's own answer rather than on the first row
        // of the list, which is a snapshot.
        assert_eq!(shell.chosen_version().as_deref(), Some("1.21.4"));
        drop(shell.render());
    }

    /// A loader's builds as a service answers them: two stable builds, a
    /// pre-release, then an older one -- newest first, which is the order every
    /// one of these services publishes in.
    fn loader_build_list() -> Vec<store::LoaderBuild> {
        [("0.19.5", true), ("0.19.3-beta.1", false), ("0.19.2", true)]
            .into_iter()
            .map(|(version, stable)| store::LoaderBuild { version: version.to_string(), stable })
            .collect()
    }

    /// The loader and game version a request was made for, as `act` records it.
    fn asked_for(shell: &Shell) -> Option<(crate::catalog::LoaderKind, String)> {
        shell.loader_builds_for.clone()
    }

    #[test]
    fn the_creation_dialog_asks_the_loader_it_opens_on_which_builds_it_has() {
        // The chips are wired to the store's own request, and the order matters:
        // the dialog opens on Fabric, but the game version is what a loader's
        // builds are listed *for*, so the request waits for Mojang's answer rather
        // than being made against a version list that has not arrived.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        assert_eq!(shell.create_loader, crate::catalog::LoaderKind::Fabric);
        assert_eq!(asked_for(&shell), None, "nothing to ask about yet");
        // Through `act` rather than `press`, for the flag's own reason: `handle`
        // spends it on the way to building the command, and what is being checked
        // here is that `act` raised it.
        let _ = shell.act(Message::Versions(Ok(version_list())));
        assert!(matches!(shell.loader_builds, Load::Loading));
        assert!(shell.loader_builds_requested, "and the request has not left `act` yet");
        assert_eq!(
            asked_for(&shell),
            Some((crate::catalog::LoaderKind::Fabric, "1.21.4".to_string())),
            "asked for the version the picker opened on"
        );
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Fabric,
                game: "1.21.4".to_string(),
                builds: Ok(loader_build_list()),
            },
        );
        assert!(shell.loader_builds.ready().is_some(), "the list arrived");
        assert!(
            !shell.loader_builds_requested,
            "the flag is what `handle` takes to build the command, so it is already spent"
        );
        // *Stable* is the newest build the loader itself calls stable, which is not
        // the newest build: that is what *Latest* is for.
        assert_eq!(shell.chosen_build().as_deref(), Some("0.19.5"));
        assert!(shell.create_usable());
        drop(shell.render());
    }

    #[test]
    fn a_build_list_about_a_choice_the_dialog_has_left_is_dropped() {
        // The answer carries the question it answers, which is the whole reason
        // `LoaderBuilds` is not a bare `Result`: a Fabric list that arrived after
        // the Quilt chip was pressed would otherwise be drawn under the Quilt chip.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        press(&mut shell, Message::Versions(Ok(version_list())));
        press(&mut shell, Message::LoaderChoice(crate::catalog::LoaderKind::Quilt));
        assert_eq!(asked_for(&shell).map(|(loader, _)| loader), Some(crate::catalog::LoaderKind::Quilt));
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Fabric,
                game: "1.21.4".to_string(),
                builds: Ok(loader_build_list()),
            },
        );
        assert!(
            matches!(shell.loader_builds, Load::Loading),
            "the Fabric list is not an answer to the Quilt question"
        );
        // And the loader's own answer, once it is the one in force, is what the
        // chips read.
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Quilt,
                game: "1.21.4".to_string(),
                builds: Ok(vec![store::LoaderBuild {
                    version: "0.30.1".to_string(),
                    stable: true,
                }]),
            },
        );
        assert_eq!(shell.chosen_build().as_deref(), Some("0.30.1"));
        // *Latest* is the newest of all, which is what a pre-release is when it is
        // the newest -- and `Other` is a build the user picks, or nothing at all.
        press(&mut shell, Message::BuildChoice(BuildChoice::Latest));
        assert_eq!(shell.chosen_build().as_deref(), Some("0.30.1"));
        press(&mut shell, Message::BuildChoice(BuildChoice::Other));
        assert_eq!(shell.chosen_build(), None, "nothing has been picked yet");
        press(&mut shell, Message::BuildPicked("0.29.0".to_string()));
        assert_eq!(
            shell.chosen_build(),
            None,
            "a build the list does not carry is not one this dialog can install"
        );
        press(&mut shell, Message::BuildPicked("0.30.1".to_string()));
        assert_eq!(shell.chosen_build().as_deref(), Some("0.30.1"));
        drop(shell.render());
    }

    #[test]
    fn the_create_button_waits_for_the_loader_s_own_answer() {
        // What the button's own rule is: a create is in flight, or a loader is
        // chosen whose list has not answered. Both are states the dialog can be
        // left from rather than a dead end -- the chips go back to Vanilla, which
        // has no list to wait for.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        assert!(!shell.create_usable(), "the list has not even been asked for");
        press(&mut shell, Message::Versions(Ok(version_list())));
        assert!(!shell.create_usable(), "and it is still coming");
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Fabric,
                game: "1.21.4".to_string(),
                builds: Ok(loader_build_list()),
            },
        );
        assert!(shell.create_usable());
        // Vanilla is not a service: its chips drop the build row and the list with
        // it, and there is nothing to wait for.
        press(&mut shell, Message::LoaderChoice(crate::catalog::LoaderKind::Vanilla));
        assert!(matches!(shell.loader_builds, Load::Idle), "the Fabric list is gone");
        assert_eq!(shell.chosen_build(), None, "vanilla installs no build");
        assert!(shell.create_usable());
        drop(shell.render());
        // A loader whose list *failed* is a sentence on the row and a button the
        // reader can still press: the alternative is a dialog with no way out of
        // it, and the failure is already said where it happened.
        press(&mut shell, Message::LoaderChoice(crate::catalog::LoaderKind::Forge));
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Forge,
                game: "1.21.4".to_string(),
                builds: Err("Forge's build list could not be read: no route".to_string()),
            },
        );
        assert!(matches!(shell.loader_builds, Load::Failed(_)));
        assert!(shell.chosen_build().is_none());
        assert!(shell.create_usable());
        drop(shell.render());
        // The *Stable* chip's own rule: a loader that published nothing stable for
        // this game version leaves the chip in place and dim, which is the
        // reference's `disabledItems` -- and while the list is still coming,
        // nothing is known to be missing.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        assert!(shell.stable_offered(), "not asked yet");
        press(&mut shell, Message::Versions(Ok(version_list())));
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Fabric,
                game: "1.21.4".to_string(),
                builds: Ok(vec![store::LoaderBuild {
                    version: "0.19.3-beta.1".to_string(),
                    stable: false,
                }]),
            },
        );
        assert!(!shell.stable_offered(), "nothing stable to offer");
        // And the rule that chip stands for falls back to the newest build there
        // is, which is the one build this loader published: a row that offered
        // *Stable* and nothing to press would be a dead end of its own.
        assert_eq!(shell.chosen_build().as_deref(), Some("0.19.3-beta.1"));
        drop(shell.render());
    }

    #[test]
    fn the_picker_shows_the_releases_and_its_footer_adds_the_rest() {
        // The reference's own arrangement: the list is the releases until the
        // dropdown's footer asks for everything, and the search narrows whichever
        // list is showing.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        press(&mut shell, Message::Versions(Ok(version_list())));
        assert_eq!(version_ids(&shell), vec!["1.21.4", "1.21.3"], "releases until asked for more");
        press(&mut shell, Message::VersionSnapshots(true));
        assert_eq!(version_ids(&shell), vec!["25w02a", "1.21.4", "1.21.3", "b1.7.3"]);
        // A version is found by the part of it a user remembers rather than only
        // by its beginning: `21.3` is the end of `1.21.3`.
        press(&mut shell, Message::VersionQuery("21.3".to_string()));
        assert_eq!(version_ids(&shell), vec!["1.21.3"]);
        press(&mut shell, Message::VersionQuery("nothing publishes this".to_string()));
        assert!(version_ids(&shell).is_empty());
        // A picker that found nothing is a sentence, and the sentence is drawn by
        // the same code path as the list: this is the render that proves it.
        drop(shell.render());
        // Choosing is what the heading and the create both read, and a choice
        // outranks Mojang's own answer.
        press(&mut shell, Message::VersionSnapshots(false));
        press(&mut shell, Message::VersionQuery(String::new()));
        press(&mut shell, Message::VersionChoice("1.21.3".to_string()));
        assert_eq!(shell.chosen_version().as_deref(), Some("1.21.3"));
        drop(shell.render());
    }

    #[test]
    fn a_version_list_that_did_not_arrive_is_a_sentence_and_not_an_empty_picker() {
        // The failure path, and the one that would look like a working picker if
        // the list were drawn as an empty list: the reason is what is drawn, and
        // the create still leaves with nothing chosen -- which is the store's own
        // question to Mojang rather than a version this dialog invented.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        press(&mut shell, Message::Versions(Err(store::not_implemented("Minecraft's version list"))));
        assert!(matches!(shell.versions, Load::Failed(_)));
        assert_eq!(shell.chosen_version(), None);
        drop(shell.render());
        press(&mut shell, Message::Create);
        assert!(shell.creating);
        // And a list that came back with nothing in it is the other arm of the
        // same sentence: `Empty` rather than a list of nothing.
        press(
            &mut shell,
            Message::Versions(Ok(store::VersionList {
                latest_release: "1.21.4".to_string(),
                versions: Vec::new(),
            })),
        );
        assert_eq!(shell.versions, Load::Empty);
        drop(shell.render());
    }

    #[test]
    fn the_settings_modal_offers_the_themes_and_takes_one() {
        // The pane the modal has been waiting for since stage 2: the colour
        // themes `color_theme.rs` has carried all along, offered by the
        // reference's own rule and taken by the window that shows them.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::Settings));
        let offered = ColorTheme::options(false, shell.prefs.theme());
        assert!(offered.contains(&ColorTheme::Dark));
        assert!(offered.contains(&ColorTheme::Light));
        assert!(
            !offered.contains(&ColorTheme::Retro),
            "retro is behind dev mode until it is the theme already in force"
        );
        // Every option is a control the kit builds, which is what `render`
        // proves: a button the kit cannot build is a modal that panics.
        drop(shell.render());
        press(&mut shell, Message::ColorTheme(ColorTheme::Light));
        assert_eq!(shell.theme, Gen::Light);
        assert_eq!(shell.prefs.theme(), ColorTheme::Light);
        // And once retro *is* the theme in force, the filter keeps offering it:
        // the reference's rule read from the other side.
        press(&mut shell, Message::ColorTheme(ColorTheme::Retro));
        assert_eq!(shell.theme, Gen::Retro);
        assert!(ColorTheme::options(false, shell.prefs.theme()).contains(&ColorTheme::Retro));
        // A test has no home, so nothing was written to anyone's preferences.
        assert!(shell.home.is_none());
    }

    #[test]
    fn every_language_row_fits_its_own_room_in_every_language() {
        // The language pane is a column of rows now rather than a grid of chips, so
        // there is no row to break and nothing to wrap: `CheckCircleButton` gives
        // each language a full-width 40-pixel row, and its name, its own name in
        // itself and its coverage share the room in front of the check circle. What
        // the reference does with more text than that is `truncate`, and this
        // renderer has no ellipsis, so the margin is measured instead -- over every
        // language in every one of the 32 it can be read in, because a name is the
        // reference's word for a language and Arabic's are longer than English's.
        let shell = shell_at("/");
        // A row's own room: the pane's width less the row's 1-pixel border on each
        // side, its `!px-2` padding, the `!gap-4` before the circle and the circle
        // itself (`size-6`).
        let room = shell.settings_content_width() - 2.0 - 16.0 - 16.0 - 24.0;
        // The worst row the walk below finds, which is what the lines at the end
        // report: a gate that is only a verdict says nothing about how much room is
        // left, and the next session's question is the margin rather than the pass.
        let mut widest = 0.0f32;
        let mut widest_at = crate::locale::ENGLISH;
        // The worst a theme card's name overflows its own room, over the same walk.
        let mut theme_overflow = 0.0f32;
        let mut theme_overflow_at = crate::locale::ENGLISH;
        // English, read in English: the claim the capture makes, and the one that
        // has to hold before the margins beside it mean anything.
        let mut english_widest = 0.0f32;
        for &tag in crate::locale::OFFERED.iter() {
            crate::locale::set(tag);
            for &offered in crate::locale::OFFERED.iter() {
                // The row's three texts in the reference's own sizes and faces: the
                // name at `text-sm sm:text-base` in the row's `font-semibold`, the
                // translated name and the coverage at `text-xs sm:text-sm` in
                // `font-normal`, with `gap-2` between them.
                let mut runs: Vec<(String, f32, iced::Font)> =
                    vec![(crate::locale::label(offered), 16.0, semibold())];
                if let Some(translated) = crate::locale::translated_label(offered) {
                    runs.push((translated, 14.0, crate::style::regular()));
                }
                if let Some(coverage) = crate::locale::coverage(offered) {
                    runs.push((format!("{coverage}%"), 14.0, crate::style::regular()));
                }
                let width = runs
                    .iter()
                    .map(|(run, size, font)| crate::ui::advance(run, *font, *size))
                    .sum::<f32>()
                    + (runs.len() as f32 - 1.0) * 8.0;
                if tag == crate::locale::ENGLISH {
                    english_widest = english_widest.max(width);
                }
                if width > widest {
                    widest = width;
                    widest_at = tag;
                }
            }
            // The theme cards do not wrap: each is a fixed column with one name in
            // it, so what is measured is a name inside the label's own room -- the
            // card less its borders, its padding, the radio and the icon a couple of
            // the cards carry. This is the one claim of the walk that is reported and
            // not asserted, and the reason is in the drawing layer: the reference's
            // card is `overflow: hidden`, so a longer name is *clipped* there, and a
            // clip is the one thing a text here cannot be asked for. English fits
            // (that is the capture's own claim and it is asserted); the rest are
            // printed, so a locale that outgrows its card is a number in the run
            // rather than a surprise on a screen.
            let room = theme_label_room(shell.settings_content_width());
            for label in shell.theme_labels() {
                let width = crate::ui::row_width(&[label], 0.0);
                if label == shell.theme_labels()[0] {
                    assert!(
                        width <= room,
                        "the first theme card {label:?} measures {width} against {room}"
                    );
                }
                if width - room > theme_overflow {
                    theme_overflow = width - room;
                    theme_overflow_at = tag;
                }
            }
        }
        assert!(
            english_widest <= room,
            "the widest language row in English measures {english_widest:.1} against {room:.0}"
        );
        // Printed rather than asserted: a run asked for its output says what the
        // pane came to in the worst of the 32 languages, which is the number the
        // margin is read from.
        eprintln!(
            "language rows: widest {widest:.1} of {room:.0} px ({widest_at}), English {english_widest:.1}"
        );
        eprintln!(
            "theme cards: widest name overflow {theme_overflow:.1} px over {:.0} ({theme_overflow_at})",
            theme_label_room(shell.settings_content_width())
        );
        // The language is ambient, and every other test in this file reads English.
        crate::locale::set(crate::locale::ENGLISH);
    }

    #[test]
    fn a_language_grid_taller_than_the_window_scrolls_rather_than_drawing_past_it() {
        // The vertical half of the same defect: a dialog is drawn centred, so one
        // taller than its window loses its head at the top and its last rows at the
        // bottom with no scroll to reach either. The language pane is a column of
        // 40-pixel rows now -- 32 of them is nearly 1,400 pixels, which is more than
        // this window has twice over -- and the shell keeps the window's own size
        // and caps a dialog's body with it; these are the two ends of that cap.
        let mut shell = shell_at("/");
        assert_eq!(
            shell.viewport, DIALOG_VIEWPORT,
            "the shell starts at the size the entry point opens it at"
        );
        press(&mut shell, Message::Viewport(iced::Size::new(1000.0, 600.0)));
        assert_eq!(
            shell.viewport,
            iced::Size::new(1000.0, 600.0),
            "the window's own report is what the shell keeps"
        );
        // A window whose arithmetic leaves nothing still leaves a body to read.
        press(&mut shell, Message::Viewport(iced::Size::new(400.0, 200.0)));
        assert_eq!(shell.dialog_body_room(), DIALOG_BODY_MIN);
        // And the dialog is built at every one of them -- a scroll region the kit
        // cannot build is a modal that panics.
        press(&mut shell, Message::Rail(Rail::Settings));
        for size in [DIALOG_VIEWPORT, iced::Size::new(980.0, 640.0), iced::Size::new(1920.0, 1080.0)] {
            press(&mut shell, Message::Viewport(size));
            drop(shell.render());
        }
        press(&mut shell, Message::Viewport(DIALOG_VIEWPORT));
        // The grid's height in a language, which is the whole of what a language
        // changes about how tall this dialog is: the theme block and the sentences
        // above the list are the same in every one of them.
        let grid_height = |tag: &str| {
            crate::locale::set(tag);
            let labels = Shell::language_labels();
            let rows = crate::ui::wrap_labels(&labels, DIALOG_INNER, ROW_GAP).len() as f32;
            rows * crate::ui::CONTROL + (rows - 1.0) * ROW_GAP
        };
        let english = grid_height(crate::locale::ENGLISH);
        let indonesian = grid_height("id-ID");
        crate::locale::set(crate::locale::ENGLISH);
        let room = shell.dialog_body_room();
        assert!(
            english < room,
            "English's {english} of grid has to fit the {room} a 720-pixel window leaves, \
             or every reader gets a scrollbar for a list that fits"
        );
        assert!(
            indonesian > room,
            "Indonesian's {indonesian} of grid is the one that has to scroll rather than be cut off"
        );
    }

    #[test]
    fn a_dialog_body_is_broken_for_less_than_its_padding_leaves() {
        // The arithmetic behind the two screenshots this slice came from. The
        // number is iced's: a `Scrollable`'s bar is drawn over its content, at
        // `Properties::default()`'s 10 pixels, so a body's last ten pixels are
        // covered whenever something scrolls -- and the language grid's last row
        // and the `Game version` value are what sat under it. Read back as the
        // identity rather than as a literal, so that moving the dialog's width,
        // its padding or the bar cannot silently widen the content again: the
        // padding, the bar and the body's own width *are* the dialog.
        assert_eq!(DIALOG_PAD * 2.0 + DIALOG_SCROLLBAR + DIALOG_INNER, DIALOG_WIDTH);
        assert!(
            DIALOG_INNER < DIALOG_WIDTH - 2.0 * DIALOG_PAD,
            "a body broken for the full padding leaves its last chip under the bar"
        );
    }

    #[test]
    fn the_settings_modal_offers_the_languages_and_takes_one() {
        // The settings row the locale module has been waiting for: the
        // reference's own 32 codes, offered, taken, and recorded.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::Settings));
        assert_eq!(crate::locale::OFFERED.len(), 32);
        assert!(
            !crate::locale::OFFERED.contains(&"ar-SA"),
            "the reference comments ar-SA out as RTL, so this launcher does not offer it"
        );
        // Every option is a control the kit builds, which is what `render`
        // proves: a button the kit cannot build is a modal that panics.
        drop(shell.render());
        press(&mut shell, Message::Locale("de-DE"));
        assert_eq!(crate::locale::tag(), "de-DE");
        assert_eq!(shell.prefs.locale.as_deref(), Some("de-DE"));
        // The language is not a redraw of a table the way a theme is: it is in
        // force the moment it is taken, and the next frame is the proof.
        drop(shell.render());
        assert_eq!(Key::SettingsLanguageTitle.message(), "Sprache");
        // English is stored as the empty string rather than as "en-US", because
        // the preferences file is the diff from the defaults.
        press(&mut shell, Message::Locale(crate::locale::ENGLISH));
        assert_eq!(crate::locale::tag(), crate::locale::ENGLISH);
        assert_eq!(shell.prefs.locale, None);
        assert_eq!(Key::SettingsLanguageTitle.message(), "Language");
        // A test has no home, so nothing was written to anyone's preferences.
        assert!(shell.home.is_none());
    }

    #[test]
    fn settings_is_a_modal_and_not_a_page() {
        // The claim the rewrite is built on, in two halves: the rail's settings
        // button has no path, and `/settings` is not a page the route table
        // knows -- which is also what makes the old shell fail this gate.
        assert_eq!(Shell::destination(Rail::Settings), None);
        assert!(Address::parse("/settings").is_none());
        let mut shell = shell_at("/");
        assert_eq!(shell.modal, None);
        press(&mut shell, Message::Rail(Rail::Settings));
        assert_eq!(shell.modal, Some(Modal::Settings));
        press(&mut shell, Message::CloseModal);
        assert_eq!(shell.modal, None);
    }

    #[test]
    fn the_panel_is_up_when_the_reference_puts_it_up() {
        // The user's toggle, or a page that forces it: `App.vue`'s
        // `forceSidebar` on browse, project and user pages.
        let shell = shell_at("/");
        assert!(shell.panel_shown(), "the panel is on unless it was turned off");
        let hidden =
            Shell::new(Address::at(Route::Home), Gen::Dark, &settings(true, false, true));
        assert!(!hidden.panel_shown());
        for path in ["/browse/modpack", "/project/sodium", "/user/jelly"] {
            let forced = Shell::new(
                Address::parse(path).expect(path),
                Gen::Dark,
                &settings(true, false, true),
            );
            assert!(forced.panel_shown(), "{path} forces the panel open");
        }
        let instance = Shell::new(
            Address::at(Route::Instance { id: "atm10".into(), tab: InstanceTab::Content }),
            Gen::Dark,
            &settings(true, false, true),
        );
        assert!(!instance.panel_shown(), "an instance page does not force it");
    }

    #[test]
    fn the_panel_toggle_is_only_where_it_could_change_something() {
        // `v-if="!forceSidebar && appSettings.toggleSidebar"`: on a page that
        // forces the panel there is no arrow, because the panel is up whatever
        // the toggle says. A control drawn there is one whose press cannot move
        // what it points at.
        for path in ["/browse/modpack", "/project/sodium", "/user/jelly"] {
            let shell = Shell::new(
                Address::parse(path).expect(path),
                Gen::Dark,
                &settings(false, false, true),
            );
            assert!(
                !shell.panel_toggle_shown(),
                "{path} forces the panel, so the arrow is not drawn"
            );
        }
        for path in ["/", "/skins", "/screenshots"] {
            let shell = Shell::new(
                Address::parse(path).expect(path),
                Gen::Dark,
                &settings(false, false, true),
            );
            assert!(
                shell.panel_toggle_shown(),
                "{path} leaves the panel to the reader, so the arrow is there"
            );
        }
    }

    #[test]
    fn the_ad_block_is_where_the_panel_is() {
        // `showAd`: the panel is up, so the block is under it. It is a sibling of
        // the scroll region rather than a section inside it, which is what the
        // `pb-12` on the scroll is for -- a section that had the ad scroll past
        // it would be the reference's page with an extra step added.
        for (path, shown) in [
            ("/", true),
            ("/browse/modpack", true),
            ("/skins", true),
        ] {
            let shell = Shell::new(
                Address::parse(path).expect(path),
                Gen::Dark,
                &settings(false, false, true),
            );
            assert_eq!(shell.promo_shown(), shown, "{path}");
        }
        let off = Shell::new(
            Address::at(Route::Home),
            Gen::Dark,
            &settings(true, false, true),
        );
        assert!(!off.promo_shown(), "a panel that is down has no ad under it");

        // The two heights the reference writes twice: the ad's own 250, and the
        // five rem of fade above it. The fade exists so the last section
        // dissolves into the ad, so a zero here would be a hard edge where the
        // reference has a gradient.
        assert_eq!(PROMO_PLATE, 250.0);
        assert_eq!(PROMO_FADE, 80.0);
    }

    /// A shell whose accounts come from a file of its own, which is what the
    /// panel draws from.
    fn shell_with_accounts(path: &std::path::Path) -> Shell {
        let (accounts, warning) = AccountsStore::load_with_report(path);
        shell_at("/").with_accounts(accounts, warning)
    }

    #[test]
    fn the_card_names_the_account_a_launch_would_sign_in_as() {
        let steve = AccountEntry::offline("Steve");
        let uuid = steve.uuid.clone();
        // The account in force is the one the file says.
        assert_eq!(card_title(std::slice::from_ref(&steve), Some(&uuid)), "Steve");
        // Nothing selected, or a selection for an account that is gone: the
        // reference's own sentence rather than the wrong account.
        assert_eq!(
            card_title(std::slice::from_ref(&steve), None),
            Key::MinecraftAccountSelectAccount.message()
        );
        assert_eq!(
            card_title(&[steve], Some("99999999999999999999999999999999")),
            Key::MinecraftAccountSelectAccount.message()
        );
        assert_eq!(card_title(&[], None), Key::MinecraftAccountSelectAccount.message());
        // The uuid is the profile id and an offline account's is derived, so the
        // card names the same player Prism does.
        assert_eq!(uuid, crate::accounts::offline_uuid("Steve"));
    }

    #[test]
    fn the_panel_draws_its_section_and_its_card_in_every_theme() {
        // The panel is not a wash any more: the section, the card's two branches
        // and the note all draw. In every theme, because the inks and the frame
        // come from the generated tables and one of the four could be missing a
        // token the others have.
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("accounts.json");
        let mut shell = shell_with_accounts(&path);
        assert!(!shell.accounts_open, "the reference's accordion starts closed");
        for theme in Gen::ALL {
            shell.theme = *theme;
            let _ = shell.panel();
        }
        // With accounts, the card draws the accordion instead, open and closed.
        {
            let accounts = shell.accounts.as_mut().expect("a store");
            accounts.add("Steve").expect("a name");
            accounts.save().expect("a written file");
        }
        press(&mut shell, Message::ToggleAccounts);
        assert!(shell.accounts_open);
        press(&mut shell, Message::SignIn);
        for theme in Gen::ALL {
            shell.theme = *theme;
            let _ = shell.panel();
        }
        press(&mut shell, Message::ToggleAccounts);
        assert!(!shell.accounts_open);
    }

    #[test]
    fn a_shell_with_no_accounts_file_is_the_card_s_own_empty_state() {
        // The shell the application builds always has a store; a shell with none
        // is what the tests build, and it draws the same card a store with no
        // accounts does.
        let mut shell = shell_at("/");
        assert!(shell.accounts.is_none());
        let _ = shell.panel();
        assert!(shell.accounts_note.is_none());
        // Nothing is written and nothing panics: there is no file to write to.
        press(&mut shell, Message::SelectAccount("nobody".into()));
        press(&mut shell, Message::RemoveAccount("nobody".into()));
        assert!(shell.accounts_note.is_none());
    }

    #[test]
    fn the_quick_create_key_is_the_reference_s_and_only_on_its_own_screen() {
        use iced::keyboard::{key::Named, Key, Modifiers};
        let lower = Key::Character("n".into());
        assert!(quick_create_press(&lower, Modifiers::default()));
        // Shift is not one of the reference's guards: it compares the lower-cased
        // key, so `Shift+N` opens the creation flow there too.
        assert!(quick_create_press(&Key::Character("N".into()), Modifiers::SHIFT));
        for modifiers in [Modifiers::CTRL, Modifiers::ALT, Modifiers::LOGO] {
            assert!(!quick_create_press(&lower, modifiers), "{modifiers:?}");
        }
        // Anything that is not the letter, including a named key that happens to
        // start with it.
        assert!(!quick_create_press(&Key::Character("m".into()), Modifiers::default()));
        assert!(!quick_create_press(&Key::Character("".into()), Modifiers::default()));
        assert!(!quick_create_press(&Key::Named(Named::Enter), Modifiers::default()));

        // And the screen's own gate: Home with nothing to play and no dialog over
        // it. The subscription is built from this, so a key that arrives on any
        // other page never reaches the guard above.
        let mut shell = shell_with_home("welcome-keys");
        assert!(shell.welcome_shown(), "a first run on Home");
        // The creation dialog is what the key opens, and while it is up the key is
        // no longer listened for -- which is also what keeps its own name field
        // from re-opening it.
        press(&mut shell, Message::OpenCreate);
        assert_eq!(shell.modal, Some(Modal::Create));
        assert!(!shell.welcome_shown());
        press(&mut shell, Message::CloseModal);
        assert!(shell.welcome_shown());
        // A launcher with an instance is not on the welcome screen, and neither is
        // one that is not on Home.
        assert!(!shell_with_instance("welcome-keys-instance").welcome_shown());
        let mut elsewhere = shell_with_home("welcome-keys-route");
        elsewhere.address = Address::at(route::Route::Skins);
        assert!(!elsewhere.welcome_shown());
    }

    #[test]
    fn the_checklist_reads_the_facts_the_launcher_holds_and_gates_the_card_behind_them() {
        // The three facts, off this launcher: a library with an instance in it, an
        // account file with one in it, and Modrinth's -- which is false here,
        // because this launcher has no Modrinth sign-in at all.
        let shell = shell_with_instance("panel-checklist");
        let checklist = shell.checklist();
        assert!(checklist.show(), "two steps are outstanding");
        assert!(checklist.complete(Step::CreateInstance), "the library holds one");
        assert!(!checklist.complete(Step::LoginMinecraft));
        assert!(!checklist.complete(Step::LoginModrinth));
        assert!(!checklist.friends_visible(), "and the friends list waits behind it");
        // `App.vue` draws *Playing as* `v-show="hasLoggedIntoMinecraft"`, and that
        // flag is the checklist's own second fact now: no account, no card. Every
        // theme is drawn because the section's inks come from the generated
        // tables, and one of the four could be missing a token the others have.
        assert!(!shell.logged_into_minecraft(), "no account file at all");
        let mut shell = shell;
        for theme in Gen::ALL {
            shell.theme = *theme;
            let _ = shell.panel();
        }
        // With an account the card is back, that step is done, and the section is
        // *not* up any more -- both steps this launcher can finish are finished,
        // and Modrinth's third one cannot hold the section up forever. What is
        // drawn in its place is the friends section, whose own sentence is the
        // third step's prompt in the place the reference puts it.
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("accounts.json");
        let (accounts, warning) = AccountsStore::load_with_report(&path);
        let mut shell = shell.with_accounts(accounts, warning);
        {
            let accounts = shell.accounts.as_mut().expect("a store");
            accounts.add("Steve").expect("a name");
        }
        assert!(shell.logged_into_minecraft());
        assert!(shell.checklist().complete(Step::LoginMinecraft));
        assert!(!shell.checklist().show());
        assert!(shell.checklist().friends_visible());
        assert!(shell.friends_section().is_some());
        for theme in Gen::ALL {
            shell.theme = *theme;
            let _ = shell.panel();
        }
    }

    #[test]
    fn the_friends_section_is_the_reference_s_sentence_and_its_press_is_the_sign_in() {
        // A shell with an instance and an account: the two things the checklist
        // waits for, so the section is the one on screen -- which is the reference's
        // own `showFriendsList = !showChecklist || hasLoggedIntoModrinth` with the
        // third fact false.
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("accounts.json");
        let (accounts, warning) = AccountsStore::load_with_report(&path);
        let mut shell = shell_with_instance("panel-friends").with_accounts(accounts, warning);
        {
            let accounts = shell.accounts.as_mut().expect("a store");
            accounts.add("Steve").expect("a name");
        }
        assert!(shell.friends_section().is_some());
        // The sentence is the reference's, with its tag taken out: the generated
        // table keeps `<link>` verbatim, and drawing it would be the one thing a
        // reader would report.
        let sentence = Key::FriendsSignInToAddFriends.message();
        assert!(sentence.contains("<link>"), "the table keeps the markup");
        let (before, slot, after) = crate::text::tagged(sentence, "link").expect("a slot");
        assert_eq!(format!("{before}{slot}{after}"), "Sign in to a Modrinth account to add friends and see what they're playing!");
        // No heading while there are no credentials, which is the reference's own
        // `v-if="userCredentials"` around it.
        assert_eq!(Key::FriendsHeading.message(), "Friends");
        for theme in Gen::ALL {
            shell.theme = *theme;
            let _ = shell.panel();
        }
        // The press is the third step of the checklist, so it is the same sentence
        // the step gives rather than a second one about the same flow -- and that
        // sentence is the account one, because there is no Modrinth sign-in coming
        // here (G118).
        press(&mut shell, Message::Checklist(Step::LoginModrinth));
        assert_eq!(
            shell.modrinth_note.as_deref(),
            Some(store::needs_account("Signing in to Modrinth").as_str())
        );
        // Once the instance goes the checklist is up again and the friends section
        // waits behind it -- the other half of the rule, read from the panel's own
        // drawing rather than from the enum.
        assert!(Checklist::of(false, true, false).show());
        assert!(!Checklist::of(false, true, false).friends_visible());
    }

    #[test]
    fn the_checklist_s_header_opens_the_body_it_starts_open() {
        // The reference's accordion is `open-by-default`, like the card's is not:
        // a reader who has not made an instance is shown the steps rather than a
        // header they have to guess at.
        let mut shell = shell_with_instance("panel-checklist-toggle");
        assert!(shell.checklist_open);
        for theme in Gen::ALL {
            shell.theme = *theme;
            let _ = shell.panel();
        }
        press(&mut shell, Message::ToggleChecklist);
        assert!(!shell.checklist_open);
        for theme in Gen::ALL {
            shell.theme = *theme;
            let _ = shell.panel();
        }
        press(&mut shell, Message::ToggleChecklist);
        assert!(shell.checklist_open);
    }

    #[test]
    fn the_checklist_s_three_steps_do_what_the_reference_s_three_handlers_do() {
        let mut shell = shell_with_instance("panel-checklist-press");
        // *Create first instance* is `@create-instance`, which opens the creation
        // flow -- this shell's own dialog.
        press(&mut shell, Message::Checklist(Step::CreateInstance));
        assert_eq!(shell.modal, Some(Modal::Create));
        // *Sign in to Minecraft* is `@login-minecraft`, which opens the accounts
        // card's sign-in: the same press the card's own button makes, so it is the
        // same sentence rather than a second one.
        press(&mut shell, Message::Checklist(Step::LoginMinecraft));
        assert_eq!(
            shell.accounts_note.as_deref(),
            Some(store::not_implemented("Signing in to Minecraft").as_str())
        );
        // *Sign in to Modrinth* has no flow anywhere in this launcher and is not
        // going to get one -- it is an account this launcher does not hold (G118)
        // -- and its sentence is its own: it is not the Microsoft flow the note
        // above is about, and reading one does not clear the other.
        press(&mut shell, Message::Checklist(Step::LoginModrinth));
        assert_eq!(
            shell.modrinth_note.as_deref(),
            Some(store::needs_account("Signing in to Modrinth").as_str())
        );
        assert_ne!(
            shell.modrinth_note.as_deref(),
            shell.accounts_note.as_deref(),
            "the two sign-ins are told apart by their sentences"
        );
        press(&mut shell, Message::DismissModrinthNote);
        assert!(shell.modrinth_note.is_none());
        assert!(shell.accounts_note.is_some(), "the other sentence is still there");
        // And nothing a press could not finish is finished: the section is still up
        // with both sign-ins outstanding, which is the honest picture of a launcher
        // that has neither flow yet.
        let checklist = shell.checklist();
        assert!(checklist.show());
        assert!(!checklist.complete(Step::LoginMinecraft));
        assert!(!checklist.complete(Step::LoginModrinth));
    }

    #[test]
    fn choosing_an_account_writes_the_file_the_other_launcher_reads() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("accounts.json");
        let mut shell = shell_with_accounts(&path);
        let (steve, alex) = {
            let accounts = shell.accounts.as_mut().expect("a store");
            accounts.add("Steve").expect("a name");
            accounts.add("Alex").expect("a name");
            accounts.save().expect("a written file");
            let list = accounts.list();
            (list[0].uuid.clone(), list[1].uuid.clone())
        };
        assert_eq!(
            shell.accounts.as_ref().expect("a store").selected_uuid(),
            Some(steve.as_str()),
            "the first account added is the one in force"
        );

        press(&mut shell, Message::SelectAccount(alex.clone()));
        assert!(shell.accounts_note.is_none(), "{:?}", shell.accounts_note);
        let (back, warning) = AccountsStore::load_with_report(&path);
        assert!(warning.is_none(), "{warning:?}");
        assert_eq!(back.selected_uuid(), Some(alex.as_str()));

        // Taking one away is written the same way, and the file the other
        // launcher reads is the one without it.
        press(&mut shell, Message::RemoveAccount(alex));
        let (back, _) = AccountsStore::load_with_report(&path);
        assert_eq!(back.list().len(), 1);
        assert_eq!(back.list()[0].username, "Steve");
        assert!(back.selected_uuid().is_none(), "the removed account was in force");

        // A uuid that is not in the file is the store's own sentence rather than
        // a silent nothing.
        press(&mut shell, Message::SelectAccount("nope".into()));
        let note = shell.accounts_note.clone().expect("a sentence");
        assert!(note.contains("no account with id"), "{note}");
    }

    #[test]
    fn a_press_that_needs_the_sign_in_flow_says_so_in_the_panel() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("accounts.json");
        let mut shell = shell_with_accounts(&path);
        assert!(shell.accounts_note.is_none());
        press(&mut shell, Message::SignIn);
        assert_eq!(
            shell.accounts_note.as_deref(),
            Some(store::not_implemented("Signing in to Minecraft").as_str())
        );
        // The control that reads it is the one that clears it, and a change to
        // the accounts file clears it too: what it said is no longer the news.
        press(&mut shell, Message::DismissAccountsNote);
        assert!(shell.accounts_note.is_none());
        press(&mut shell, Message::SignIn);
        assert!(shell.accounts_note.is_some());
        // One that does write the file clears it: what the sentence said is no
        // longer the news.
        let uuid = {
            let accounts = shell.accounts.as_mut().expect("a store");
            accounts.add("Steve").expect("a name");
            accounts.list()[0].uuid.clone()
        };
        press(&mut shell, Message::SelectAccount(uuid));
        assert!(shell.accounts_note.is_none());
    }

    #[test]
    fn the_wash_is_the_reference_s_two_stops_in_every_theme() {
        for theme in Gen::ALL {
            let parsed = parse_gradient(theme_gen::raw(*theme, Raw::BrandGradientBg));
            let (angle, stops) = parsed.unwrap_or_else(|| panic!("{theme:?} has no wash"));
            assert_eq!(stops.len(), 2, "{theme:?}");
            // Every one of the four is `0deg`, and in iced that angle needs no
            // conversion: `Radians::to_distance` subtracts a quarter turn
            // before taking the direction vector and measures y downwards, so
            // iced's 0 faces up -- `to top`, which is CSS's `0deg`. The stops
            // therefore run up the right panel: index 0 at its foot, index 1
            // at its head.
            assert_eq!(angle, 0.0, "{theme:?}: the wash runs bottom to top");
            assert_eq!(stops[0].0, 0.0, "{theme:?}");
            assert_eq!(stops[1].0, 1.0, "{theme:?}");
            // A wash that was opaque would hide the page colour it composites
            // over, and one with no alpha at all would draw nothing.
            for (_, color) in stops {
                assert!(color.a > 0.0 && color.a < 1.0, "{theme:?}: {color:?}");
            }
            let _ = wash(*theme);
        }
        // The two the reference writes out in full, against its own strings.
        let dark = parse_gradient(theme_gen::raw(Gen::Dark, Raw::BrandGradientBg)).expect("dark");
        assert_eq!(dark.1[0].1, Color::from_rgba(14.0 / 255.0, 35.0 / 255.0, 19.0 / 255.0, 0.2));
        assert_eq!(dark.1[1].1, Color::from_rgba(55.0 / 255.0, 137.0 / 255.0, 73.0 / 255.0, 0.1));
        let light =
            parse_gradient(theme_gen::raw(Gen::Light, Raw::BrandGradientBg)).expect("light");
        assert_eq!(
            light.1[0].1,
            Color::from_rgba(68.0 / 255.0, 182.0 / 255.0, 138.0 / 255.0, 0.175)
        );
        assert_eq!(
            light.1[1].1,
            Color::from_rgba(58.0 / 255.0, 250.0 / 255.0, 112.0 / 255.0, 0.125)
        );
    }

    #[test]
    fn the_gradient_reader_handles_what_the_reference_writes() {
        // A hex stop list and a non-zero angle, from `--brand-gradient-strong-bg`.
        let strong =
            parse_gradient(theme_gen::raw(Gen::Dark, Raw::BrandGradientStrongBg)).expect("strong");
        assert!((strong.0 - 270.0f32.to_radians()).abs() < 1e-6);
        assert_eq!(
            strong.1,
            vec![
                (0.1, Color::from_rgb8(0x09, 0x11, 0x0d)),
                (1.0, Color::from_rgb8(0x13, 0x1f, 0x17)),
            ]
        );
        // The same token in another theme is a bare colour rather than a
        // gradient, and a gradient of one colour is what that means.
        let retro =
            parse_gradient(theme_gen::raw(Gen::Retro, Raw::BrandGradientStrongBg)).expect("retro");
        assert_eq!(retro.1.len(), 2);
        assert_eq!(retro.1[0].1, Color::from_rgb8(0x3a, 0x3b, 0x38));
        assert_eq!(retro.1[0].1, retro.1[1].1);
        // And nothing it cannot read is guessed at.
        for value in ["", "linear-gradient(", "linear-gradient(0deg)", "nonsense", "rgb(1, 2)"] {
            assert!(parse_gradient(value).is_none(), "{value} should not parse");
        }
        assert_eq!(parse_color("#1bd96a"), Some(Color::from_rgb8(0x1b, 0xd9, 0x6a)));
        assert_eq!(parse_color("#fff"), Some(Color::from_rgb8(255, 255, 255)));
        assert_eq!(
            parse_color("rgb(1, 2, 3)"),
            Some(Color::from_rgb(1.0 / 255.0, 2.0 / 255.0, 3.0 / 255.0))
        );
        assert_eq!(parse_color("hsl(1, 2%, 3%)"), None);
        // The four direction words, at the angles iced already measures: `to
        // top` is `0deg`, and a corner is not read as one of them.
        for (spelled, degrees) in [
            ("to top", 0.0f32),
            ("to right", 90.0),
            ("to bottom", 180.0),
            ("to left", 270.0),
        ] {
            let value = format!("linear-gradient({spelled}, #000000 0%, #ffffff 100%)");
            let (angle, stops) = parse_gradient(&value).unwrap_or_else(|| panic!("{spelled}"));
            assert!((angle - degrees.to_radians()).abs() < 1e-6, "{spelled}");
            assert_eq!(stops.len(), 2, "{spelled}");
        }
        assert!(parse_gradient("linear-gradient(to bottom right, #000 0%, #fff 100%)").is_none());
        // No angle at all is CSS's own default, `to bottom`, and not `to top`:
        // reading it the other way would draw a gradient upside down.
        let (angle, stops) = parse_gradient("linear-gradient(#000000, #ffffff)").expect("bare");
        assert_eq!(angle, std::f32::consts::PI);
        assert_eq!(stops.len(), 2);
    }

    #[test]
    fn the_settings_tabs_are_the_reference_s_own() {
        // `AppSettingsModal.vue`'s `tabs` array, read in its own order. Two of its
        // eight are ported here -- Appearance, which is `PaintbrushIcon` and
        // `app.settings.tabs.appearance`, and Language, which is `LanguagesIcon`, the
        // same id under `language`, and the `commonMessages.beta` badge -- and the
        // list is asserted rather than described because the *order* is the tab
        // order: a tab inserted in the wrong place would still draw.
        assert_eq!(SettingsTab::ALL, [SettingsTab::Appearance, SettingsTab::Language]);
        assert_eq!(SettingsTab::Appearance.label(), Key::AppSettingsTabsAppearance);
        assert_eq!(SettingsTab::Appearance.label().message(), "Appearance");
        assert_eq!(SettingsTab::Appearance.glyph(), Glyph::Paintbrush);
        assert_eq!(SettingsTab::Appearance.badge(), None);
        assert_eq!(SettingsTab::Language.label(), Key::AppSettingsTabsLanguage);
        assert_eq!(SettingsTab::Language.label().message(), "Language");
        assert_eq!(SettingsTab::Language.glyph(), Glyph::Languages);
        assert_eq!(SettingsTab::Language.badge(), Some(Key::BadgeBeta));
        // Both are `tabCategories.display`, whose message is `Display` -- the
        // reference uppercases it in CSS, and the drawing layer does that itself.
        for tab in SettingsTab::ALL {
            assert_eq!(tab.category(), Key::SettingsSidebarLabelDisplay);
            assert_eq!(tab.category().message(), "Display");
        }
        // Two tabs cannot share a control name, or a hover on one would light both.
        assert_ne!(SettingsTab::Appearance.key(), SettingsTab::Language.key());
    }

    #[test]
    fn the_theme_cards_are_the_reference_s_own_grid() {
        // `appearance-settings-theme-selector.vue`'s own numbers, against the
        // reference's captured dialog: the grid is `repeat(auto-fit, minmax(12rem,
        // 1fr))` with `--gap-lg`, which is two tracks of 275 in the 566-pixel pane
        // the settings dialog gives a tab at 1280x720 -- the capture's cards span
        // x 513..787 and 804..1078.
        let shell = shell_at("/");
        assert_eq!(shell.settings_content_width(), 566.0, "the pane's inner width");
        assert_eq!(theme_columns(566.0), 2, "two tracks at the captured window");
        assert_eq!(theme_columns(THEME_CARD_MIN), 1, "one track cannot be two");
        assert_eq!(theme_columns(THEME_CARD_MIN * 2.0 + THEME_GAP), 2);
        assert_eq!(theme_columns(THEME_CARD_MIN * 3.0 + 2.0 * THEME_GAP), 3);
        // A card: the panel's 566 less the two gaps and the between, halved.
        let card = (566.0 - THEME_GAP) / 2.0;
        assert_eq!(card, 275.0);
        // The card's own stack: a border, the preview band, the label row, a border.
        assert_eq!(THEME_PREVIEW_HEIGHT + THEME_LABEL_HEIGHT + 2.0, THEME_CARD_HEIGHT);
        // The preview band: `1.5rem` around the mock, whose 66 pixels are a border,
        // `1rem`, the `2rem` icon, `1rem` and the border again.
        assert_eq!(THEME_PREVIEW_PAD * 2.0 + THEME_MOCK_HEIGHT, THEME_PREVIEW_HEIGHT);
        assert_eq!(2.0 + THEME_MOCK_PAD * 2.0 + THEME_MOCK_ICON, THEME_MOCK_HEIGHT);
        // And the mock's own contents, against the capture's own columns: the mock
        // is 225 wide (x 538..762), and its first bar starts 82 pixels into the card
        // -- the card's border, the preview's 24, the mock's border and 16, the icon
        // and the grid's gap -- which is where the capture's bars begin (595), and
        // runs to the mock's inner right edge. The capture's own bars measure 150 of
        // that; a browser's fractional card width rounds it to 149 or 150 and this
        // kit's whole pixels come to 151.
        let mock = card - 2.0 - 2.0 * THEME_PREVIEW_PAD;
        assert_eq!(mock, 225.0, "the mock's own width, against the capture's 225");
        let bars = mock - 2.0 - 2.0 * THEME_MOCK_PAD - THEME_MOCK_ICON - THEME_MOCK_GAP;
        assert_eq!(bars, 151.0, "the bars' column, against the capture's 150");
        assert_eq!(1.0 + THEME_PREVIEW_PAD + 1.0 + THEME_MOCK_PAD + THEME_MOCK_ICON + THEME_MOCK_GAP, 82.0);
        // The second bar's top is the icon's own grid: two rows and the gap between
        // them are its 32 pixels, so a row is 12 and the second bar starts at 12 + 8.
        assert_eq!(THEME_BAR2_TOP, (THEME_MOCK_ICON - THEME_MOCK_GAP) / 2.0 + THEME_MOCK_GAP);
        // A name has to fit one card's label: the capture puts `Sync with system`
        // inside 197 pixels of room.
        let room = theme_label_room(566.0);
        assert!(room > 0.0 && room < card);
        assert_eq!(room, 197.0, "the reference's own label room at this window");
        // And the pane the cards sit in is taller than the 468 the window gives it,
        // which is why the reference's capture shows the pane's bottom fade over the
        // sync row. 472 = 50 (the heading block: 28 + gap-1 + the inherited
        // 18-pixel description) + 16 (`mt-4`) + 332 (two 158-pixel card rows and
        // their 16-pixel gap) + 24 (`mt-6`) + 50 (the sync row's own block).
        let height = appearance_settings_height(shell.themes_offered().len(), 2);
        assert_eq!(height, 472.0, "the pane's own height");
        assert!(height > settings_content_min(iced::Size::new(1280.0, 720.0)));
    }

    #[test]
    fn the_settings_dialog_is_the_reference_s_own_size() {
        // `AppSettingsModal.vue`'s `:width="'min(928px, calc(95vw - 10rem))'"` and
        // `TabbedModal.vue`'s `min-h-[min(65vh,600px)]`, at the window the two clients
        // were captured in. The height is the reference's own arithmetic on top of
        // them: `NewModal.vue`'s `p-6` header around its own row -- 36 pixels, the
        // close IconButton's `h-9`, which is the taller side of an `items-center`
        // row whose title is set on 32 (24 + 36 + 24) -- its 1-pixel rule, then the
        // body's `p-6 pb-3` around the content floor (24 + 468 + 12): 589 rows of
        // surface, which is the captured dialog's own y 66..654, and 591 with its
        // two border pixels, y 65..655. This box is one row shorter than that, with
        // the pixel given back at the body's *bottom* padding: 588 of content inside
        // 590, so that the box is even and its top border is the reference's own
        // row 65 (the 591-row original is a 64.5-pixel offset in CSS, which a
        // browser pixel-snaps and a rasteriser cannot). Every row from the top
        // border down to the pane's floor is the reference's; only the bottom
        // padding's last row and the bottom border sit one higher.
        let window = iced::Size::new(1280.0, 720.0);
        assert_eq!(SETTINGS_NAV, 288.0, "`minmax(12.5rem, 18rem)` at its cap");
        assert_eq!(SETTINGS_WIDTH, 928.0);
        // 65vh of 720 is 468, and it is a *float* 468: `720.0 * 0.65` is
        // 467.99997 in f32, which is the value the layout will use and the value a
        // tolerance has to admit.
        assert!((settings_content_min(window) - 468.0).abs() < 0.001, "65vh of 720");
        let interior = 24.0 + 36.0 + 24.0 + 1.0 + 24.0 + settings_content_min(window) + 11.0;
        // 588 of content, 590 as a border box. The reference's box is 591 (589 of
        // content and the browser's two border pixels), which is a half-pixel when
        // centered in a 720-pixel window and is painted from row 65 all the same;
        // 590 is even, so this box lands on 65 without a half-pixel anywhere.
        assert!((interior - 588.0).abs() < 0.01, "{interior} is not the content's 588");
        assert!((interior + 2.0 - 590.0).abs() < 0.01, "nor the border box's 590");
        let top = (window.height - (interior + 2.0)) / 2.0;
        assert!((top - 65.0).abs() < 0.01, "the top border lands on {top}");
        // And the divider's column, which the reference's capture puts at x=488: the
        // dialog's box starts at 176 in a 1280-pixel window, its 1-pixel border is
        // the box's own first pixel (which is why the captured surface starts at
        // 177), the body's own `p-6` is 24, and the divider is the last pixel of the
        // 288-wide track -- 176 + 1 + 24 + 288 - 1 = 488.
        let left = (window.width - SETTINGS_WIDTH) / 2.0;
        assert_eq!(left + 1.0 + 24.0 + SETTINGS_NAV - 1.0, 488.0, "the captured divider");
    }

    #[test]
    fn the_modal_scrim_is_the_reference_s_own_ramp() {
        // `NewModal.vue`'s `.modal-overlay.standard`, read at the window's own
        // edges: its `inset: -5rem` means a 720-pixel window shows the ramp from
        // nine percent in to nine percent short of its end, and both of those
        // tones are the ones the reference's captured window shows through the
        // scrim -- (32, 43, 43) over the rail's chrome at the top of it, and
        // (18, 25, 29) at the bottom.
        let Background::Gradient(gradient::Gradient::Linear(linear)) =
            modal_scrim(iced::Size::new(1280.0, 720.0))
        else {
            panic!("the scrim is a gradient");
        };
        let stops: Vec<Color> = linear.stops.iter().flatten().map(|stop| stop.color).collect();
        assert_eq!(stops.len(), 2, "the reference's overlay has two stops");
        // `to bottom`, which iced's `to_distance` turns into a direction down
        // the box rather than up it.
        assert_eq!(linear.angle, Radians(std::f32::consts::PI));

        // The chrome the rail is in the theme the capture was taken in,
        // `--color-bg-raised` in dark: #27292e. The level of tolerance is the
        // one an eight-bit composite can differ by between two rasterisers.
        let chrome = (39.0f32, 41.0f32, 46.0f32);
        for (colour, measured) in [(stops[0], (32.0f32, 43.0f32, 43.0f32)), (stops[1], (18.0f32, 25.0, 29.0))] {
            // `Color`'s channels are 0..1 and the measurements are 0..255.
            let over =
                |under: f32, ink: f32| ink * 255.0 * colour.a + under * (1.0 - colour.a);
            let got = (over(chrome.0, colour.r), over(chrome.1, colour.g), over(chrome.2, colour.b));
            for (near, want) in [(got.0, measured.0), (got.1, measured.1), (got.2, measured.2)] {
                assert!((near - want).abs() <= 1.0, "{got:?} is not {measured:?}");
            }
        }
    }

    #[test]
    fn the_inks_the_shell_paints_with_are_all_declared() {
        // Every token this module names, in every theme: a token that did not
        // resolve would come back as the table's fallback, and a shell painting
        // its chrome in a fallback is exactly the failure a generated design
        // system is supposed to make impossible.
        let tokens = [
            Ink::RaisedBg,
            Ink::Bg,
            Ink::Base,
            Ink::Surface4,
            Ink::Brand,
            Ink::Green,
            Ink::Red,
            INK_DEFAULT,
            INK_CONTRAST,
            Ink::TextTertiary,
            INK_HOVER_BG,
            INK_PLATE,
            INK_PLATE_TEXT,
        ];
        for theme in Gen::ALL {
            for token in tokens {
                let color = theme_gen::ink(*theme, token);
                assert!(color.a > 0.0, "{theme:?} {token:?} is transparent");
            }
            // The chrome and the page are different surfaces: `.app-grid-navbar`
            // is `bg-bg-raised` and `.app-contents` is `bg-bg`.
            assert_ne!(
                theme_gen::ink(*theme, Ink::RaisedBg),
                theme_gen::ink(*theme, Ink::Bg),
                "{theme:?}: the rail and the page must not be the same colour"
            );
        }
        // The plate, per theme, because the reference does not treat it the
        // same way in all four and "a wash of the accent" is a claim only half
        // the table supports. `--color-button-bg-selected` is opaque
        // `green-600` `#00af5c` in light -- the same value as its accent, so a
        // selected rail button in light mode is a solid plate -- the accent at
        // 25% in dark and OLED, and an opaque `#25421e` in retro, which is
        // neither the accent nor a dilution of it. Pinned per theme rather than
        // asserted uniformly so that the day one of them changes is the day the
        // shell's hover and selection states get looked at again.
        let plate = theme_gen::ink(Gen::Light, INK_PLATE);
        assert_eq!(plate, theme_gen::ink(Gen::Light, Ink::Brand), "light is solid brand");
        assert_eq!(plate.a, 1.0, "light is not a wash");
        for theme in [Gen::Dark, Gen::Oled] {
            let plate = theme_gen::ink(theme, INK_PLATE);
            let brand = theme_gen::ink(theme, Ink::Brand);
            assert_eq!(
                (plate.r, plate.g, plate.b),
                (brand.r, brand.g, brand.b),
                "{theme:?}: the wash is the accent"
            );
            assert_eq!((plate.a * 255.0).round(), 64.0, "{theme:?}: the accent at 25%");
        }
        let retro = theme_gen::ink(Gen::Retro, INK_PLATE);
        assert_eq!(retro.a, 1.0, "retro is opaque too");
        assert_ne!(retro, theme_gen::ink(Gen::Retro, Ink::Brand));
    }

    #[test]
    fn the_widget_theme_comes_from_the_same_tokens() {
        // iced draws a text cursor, a scrollbar and any unstyled label with
        // this; deriving it from `theme_gen` is what keeps the last part of the
        // window from being painted by the palette the rewrite replaces.
        for theme in Gen::ALL {
            let widget = widget_theme(*theme);
            assert_eq!(widget.palette().background, theme_gen::ink(*theme, Ink::Bg));
            assert_eq!(widget.palette().text, theme_gen::ink(*theme, INK_DEFAULT));
            assert_eq!(widget.palette().primary, theme_gen::ink(*theme, Ink::Brand));
        }
    }

    #[test]
    fn the_color_theme_setting_picks_a_generated_theme() {
        assert_eq!(generated_theme(ColorTheme::Dark, false), Gen::Dark);
        assert_eq!(generated_theme(ColorTheme::Light, false), Gen::Light);
        assert_eq!(generated_theme(ColorTheme::Oled, false), Gen::Oled);
        // Retro is a look of its own, not a dark synonym: the generated tables
        // resolve it separately, and this is the line that used to fall through to
        // `Gen::Dark` because the setting could not name it.
        assert_eq!(generated_theme(ColorTheme::Retro, false), Gen::Retro);
        assert_eq!(generated_theme(ColorTheme::Retro, true), Gen::Retro);
        // `System` follows the machine, and resolves to the ordinary dark look
        // rather than to OLED -- an OLED choice is the display's, not the OS's.
        assert_eq!(generated_theme(ColorTheme::System, false), Gen::Dark);
        assert_eq!(generated_theme(ColorTheme::System, true), Gen::Light);
        // Every theme the reference paints is reachable from the setting, which is
        // what makes `ColorTheme::options` the only place the dev-mode rule lives.
        assert_eq!(Gen::ALL.len(), 4);
        for theme in ColorTheme::ALL {
            let generated = generated_theme(theme, false);
            assert!(Gen::ALL.contains(&generated), "{theme:?} resolves nowhere");
        }
    }

    #[test]
    fn the_breadcrumb_is_the_page_s_own_registration_and_nothing_else() {
        let crumb = |path: &str| crumbs(&Address::parse(path).expect(path), None);
        // One entry per route, which is what `useRootBreadcrumb` registers and
        // what a `useBreadcrumb` pushes under it where there is no root.
        for path in [
            "/",
            "/browse/mod",
            "/browse/modpack",
            "/skins",
            "/screenshots",
            "/hosting/manage/",
            "/hosting/manage/srv/backups",
            "/project/sodium/versions",
            "/user/jelly",
            "/instance/ATM10",
            "/instance/ATM10/projects/shader",
            "/instance/ATM10/logs",
        ] {
            assert_eq!(crumb(path).len(), 1, "{path} registers one entry");
        }
        // The labels are the reference's own, which is not what this used to
        // say: the servers page is *Hosting*, and the instance, project and user
        // pages are named after the thing rather than after a section and an id.
        assert_eq!(crumb("/"), vec![Crumb::with_icon(Glyph::Play, "Home")]);
        assert_eq!(crumb("/browse/mod"), vec![Crumb::with_icon(Glyph::Compass, "Discover mods")]);
        assert_eq!(crumb("/browse/modpack").len(), 1);
        assert_eq!(crumb("/skins"), vec![Crumb::with_icon(Glyph::Shirt, "Skin selector")]);
        assert_eq!(crumb("/screenshots"), vec![Crumb::with_icon(Glyph::Image, "Screenshots")]);
        assert_eq!(crumb("/hosting/manage/"), vec![Crumb::with_icon(Glyph::ServerStack, "Hosting")]);
        assert_eq!(crumb("/hosting/manage/srv/backups"), vec![Crumb::with_icon(Glyph::ServerStack, "srv")]);
        assert_eq!(crumb("/project/sodium/versions"), vec![Crumb::plain("sodium")]);
        assert_eq!(crumb("/user/jelly"), vec![Crumb::plain("jelly")]);
        // The tabs are `NavTabs` under the head, not crumbs in it: an instance's
        // crumb is the instance's name however the page was reached.
        for tab in ["/instance/ATM10", "/instance/ATM10/projects/shader", "/instance/ATM10/logs"] {
            assert_eq!(crumb(tab), vec![Crumb::plain("ATM10")], "{tab}");
        }
        // And the store's name for the instance is the reference's label, the
        // address's id only standing in until the scan has produced it.
        let address = Address::parse("/instance/ATM10").expect("the path parses");
        assert_eq!(crumbs(&address, Some("All The Mods")), vec![Crumb::plain("All The Mods")]);
        // The fetched visuals are `None`: an icon is one the reference registers,
        // and these three are images.
        assert_eq!(crumb("/user/jelly")[0].icon, None);
        assert_eq!(crumb("/project/sodium")[0].icon, None);
        assert_eq!(crumb("/instance/ATM10")[0].icon, None);
    }

    #[test]
    fn flags_read_the_pages_the_route_table_knows() {
        let flags = Flags::from_args(
            ["--page", "/instance/ATM10/files", "--size", "1280x720"]
                .into_iter()
                .map(String::from),
        );
        assert_eq!(flags.size, Some((1280, 720)));
        assert_eq!(
            flags.opening().route,
            Route::Instance { id: "ATM10".into(), tab: InstanceTab::Files }
        );
        // A path that is not a page opens Home rather than failing to open.
        let bogus = Flags::from_args(["--page", "/nowhere"].into_iter().map(String::from));
        assert_eq!(bogus.opening().route, Route::Home);
        assert_eq!(Flags::default().opening().route, Route::Home);
        // A malformed size is no size rather than a panic.
        let bad = Flags::from_args(["--size", "1280"].into_iter().map(String::from));
        assert_eq!(bad.size, None);
    }

    #[test]
    fn every_page_shape_builds_and_the_pane_draws_a_page() {
        // A `render()` pass over one address of every shape the route table has,
        // with and without the panel and with the modal open: catches a layout
        // builder that panics, a style closure that borrows, a canvas handed a
        // zero-sized box. Building the tree is the test.
        let sample = [
            Route::Home,
            Route::Servers,
            Route::Server { id: "srv".into(), tab: route::ServerTab::Access },
            Route::Discover { project_type: route::ProjectType::Modpack },
            Route::Skins,
            Route::Screenshots,
            Route::User { user: "jelly".into(), project_type: None },
            Route::Project { id: "sodium".into(), tab: ProjectTab::Gallery },
            Route::Instance {
                id: "a/b".into(),
                tab: InstanceTab::ContentFilter(route::ProjectType::Shader),
            },
        ];
        for route in sample {
            for hide_sidebar in [false, true] {
                let mut shell = Shell::new(
                    Address::at(route.clone()),
                    Gen::Dark,
                    &settings(hide_sidebar, true, true),
                );
                drop(shell.render());
                press(&mut shell, Message::Rail(Rail::Settings));
                drop(shell.render());
            }
            for theme in Gen::ALL {
                let shell = Shell::new(Address::at(route.clone()), *theme, &RailSettings::default());
                drop(shell.render());
            }
        }
        // The pane's body is a page rather than a placeholder, on every shape the
        // route table has -- which is the claim the name of this test makes.
        for path in ["/", "/browse/modpack", "/skins", "/screenshots", "/hosting/manage/", "/user/jelly", "/project/sodium", "/instance/ATM10/logs"] {
            let shell = shell_at(path);
            assert!(shell.pages_are_drawn(), "{path} draws a page");
        }
    }

    #[test]
    fn a_page_control_s_hover_is_a_frame_subscription_the_shell_owes() {
        // The join between the two clocks: the pages' controls tween on the
        // interaction clock, and the shell is what asks for the frames -- so a
        // crossing has to show up in `animating()` or the tween would paint one
        // frame and sit there (see `crate::hover`).
        // The key is this test's own, so what is asserted below is about this
        // page's control rather than about a clock the other tests share.
        let key = "shell:test:hover";
        anim::clock().lock().expect("the clock").clear();
        let mut shell = shell_at("/browse/modpack");
        // The rail's plate is the shell's own tween and a fresh shell starts one;
        // let it arrive, so what is asked below is about the page's control.
        shell.advance(Duration::from_secs(1));
        let drawn = |key: &str| anim::clock().lock().expect("the clock").drawn(key);
        assert_eq!(drawn(key), (1.0, 0.0), "nothing has crossed this control");
        press(
            &mut shell,
            Message::Screen(pages::Message::Discover(discover::Message::Hover {
                key,
                over: true,
                hover: None,
            })),
        );
        assert!(
            shell.animating(),
            "a hover in the air is the shell's frames to ask for"
        );
        // The deadline ends it, and the window goes quiet without another
        // message: a tween that settles must not hold the frames open.
        anim::clock()
            .lock()
            .expect("the clock")
            .tick(std::time::Instant::now() + crate::anim::INTERACTION_DURATION);
        assert_eq!(drawn(key), (crate::theme::hover_brightness(), 1.0));
        assert!(!shell.animating(), "the rail settled too, so nothing is moving");
    }

    #[test]
    fn a_navigation_forgets_where_the_pointer_was() {
        // The page that is being left does not draw its controls on the one that
        // arrives, so the crossing that lit one goes with it. Without this a
        // control the new page happens to name the same way would arrive lit.
        let key = "shell:test:left";
        anim::clock().lock().expect("the clock").clear();
        let mut shell = shell_at("/browse/modpack");
        shell.advance(Duration::from_secs(1));
        press(
            &mut shell,
            Message::Screen(pages::Message::Discover(discover::Message::Hover {
                key,
                over: true,
                hover: None,
            })),
        );
        assert!(shell.animating(), "the hover is what is moving now");
        press(&mut shell, Message::Go("/skins".to_string()));
        // The control is not drawn any more, so it is not hovered any more --
        // asserted on the key rather than on the clock's quiet, which every test
        // sharing a process would have a say in.
        let drawn = anim::clock().lock().expect("the clock").drawn(key);
        assert_eq!(
            drawn,
            (1.0, 0.0),
            "the crossing belonged to the page that is gone"
        );
    }
    /// A real 64x64 skin in memory, with the Ears marker painted in when one is
    /// asked for: what the store's marker answer comes from is the pixels, so a mock
    /// of them would be testing the mock.
    fn skin_bytes(ears: Option<u32>) -> Vec<u8> {
        // The decode crate is spelled with a leading `::`, because this module has
        // `iced::widget::image` in scope and the name would otherwise resolve to the
        // widget rather than to the crate that writes a PNG (`pages::user`'s
        // `Avatar::of` is the same collision, handled the same way).
        let mut face = ::image::RgbaImage::from_pixel(64, 64, ::image::Rgba([0, 0, 0, 0]));
        if let Some(magic) = ears {
            face.put_pixel(
                0,
                32,
                ::image::Rgba([(magic >> 16) as u8, (magic >> 8) as u8, magic as u8, 255]),
            );
        }
        let mut bytes = Vec::new();
        ::image::DynamicImage::ImageRgba8(face)
            .write_to(&mut std::io::Cursor::new(&mut bytes), ::image::ImageFormat::Png)
            .expect("a PNG in memory");
        bytes
    }

    #[test]
    fn the_stored_skins_are_read_back_in_the_readers_order_and_with_their_marker() {
        let shell = shell_with_home("stored-skins");
        let paths = shell.store.paths().expect("a store with a folder");
        // One plain texture and one carrying the Ears format's first magic. The
        // second is added second and is therefore the *front* of the reader's order,
        // because `add` promotes what was just added -- which is the reference's own
        // behaviour for a newly added skin.
        crate::saved_skins::add(
            paths,
            &skin_bytes(None),
            "plain",
            "SLIM",
            "",
            crate::saved_skins::Source::Custom,
        )
        .expect("a stored row");
        crate::saved_skins::add(
            paths,
            &skin_bytes(Some(0x3f23d8)),
            "eared",
            "CLASSIC",
            "cape-1",
            crate::saved_skins::Source::Custom,
        )
        .expect("a second stored row");
        let rows = stored_rows(&shell.store);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].entry.name, "eared", "the most recent row is first");
        assert!(rows[0].ears, "and its marker reached the page");
        assert_eq!(rows[0].entry.cape, "cape-1");
        assert_eq!(rows[1].entry.name, "plain");
        assert!(!rows[1].ears, "a texture with no marker asks for nothing");
        // A row whose pixels will not come back is still a row: the name and the arm
        // style live in the index, and only the marker is the pixels' answer.
        crate::saved_skins::add(
            paths,
            b"this is not a PNG",
            "broken",
            "CLASSIC",
            "",
            crate::saved_skins::Source::Custom,
        )
        .expect("a third row");
        let rows = stored_rows(&shell.store);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].entry.name, "broken");
        assert!(!rows[0].ears, "bytes that are not a texture carry no marker");
    }

    #[test]
    fn the_editors_press_is_an_ask_and_its_close_comes_back_to_the_page() {
        let mut shell = shell_at("/skins");
        press(&mut shell, Message::Go("/skins".into()));
        // The row the shell's own read would have filled, placed by hand: the editor
        // is drawn from the *page's* state, and this is that state.
        let row = skins::SavedRow {
            entry: crate::saved_skins::Entry {
                key: "abc".to_string(),
                name: "my skin".to_string(),
                variant: "CLASSIC".to_string(),
                cape: String::new(),
                source: crate::saved_skins::Source::Custom,
                file: "abc.png".to_string(),
            },
            ears: true,
        };
        match &mut shell.screen {
            Screen::Skins(state) => state.saved = vec![row],
            _ => panic!("the shell is on the skins page"),
        }
        press(
            &mut shell,
            Message::Screen(pages::Message::Skins(skins::Message::Edit {
                key: "abc".to_string(),
            })),
        );
        assert!(shell.screen.skins_edit().is_some(), "the editor opened");
        // The editor is a modal, so the layer is what is drawn -- including the Ears
        // notice, because the row's marker is set.
        drop(shell.render());
        // A press of one of the three actions is the page's ask, and the shell takes
        // it; what the page records is that it is waiting on the answer.
        press(
            &mut shell,
            Message::Screen(pages::Message::Skins(skins::Message::Act(
                skins::Act::Forget,
            ))),
        );
        match shell.screen.skins_edit() {
            Some((edit, _, wearing)) => {
                assert_eq!(edit.act, skins::Act::Forget);
                assert!(wearing, "the page is waiting on the write");
            }
            None => panic!("the editor stays up while the write is out"),
        }
        // And the scrim's own dismiss closes the page's editor, which is a message to
        // the page rather than a flag on this shell.
        press(&mut shell, Message::CloseModal);
        assert!(shell.screen.skins_edit().is_none(), "the scrim closed it");
    }

    #[test]
    fn a_move_is_written_to_the_stores_own_order() {
        // What the shell owns of a move: the rule is the page's (`skins::moved`), and
        // the *file* is this shell's -- `crate::saved_skins`, under the product's own
        // directory. Two stored rows, then the order a reader asked for, then the
        // index read back the way the page's reload reads it: this is what the
        // command's own future does, with the future itself left to a window.
        let shell = shell_with_home("skin-order");
        let paths = shell.store.paths().expect("a store with a folder");
        let first = crate::saved_skins::add(
            paths,
            &skin_bytes(None),
            "first",
            "CLASSIC",
            "",
            crate::saved_skins::Source::Custom,
        )
        .expect("a stored row");
        let second = crate::saved_skins::add(
            paths,
            &skin_bytes(Some(0x7f3c5f00)),
            "second",
            "SLIM",
            "",
            crate::saved_skins::Source::Custom,
        )
        .expect("a second stored row");
        // `add` promotes what it just added, so the second row is the front of the
        // order the page would draw -- which is the order this test is about to move.
        let names = |store: &Store| -> Vec<String> {
            stored_rows(store)
                .into_iter()
                .map(|row| row.entry.name)
                .collect()
        };
        assert_eq!(names(&shell.store), ["second", "first"]);

        // What the page asks for is *keys* -- its rows' digests -- in the new order,
        // because that is what the reference's `set_custom_skin_order` takes too.
        write_skin_order(
            &shell.store,
            &[first.key.clone(), second.key.clone()],
        )
        .expect("the order is written");
        assert_eq!(
            names(&shell.store),
            ["first", "second"],
            "the reader's order is what the next read draws"
        );
        // Keys the store does not hold are ignored and rows the order did not name keep
        // their place: an order that arrives short is not an instruction to lose a row.
        write_skin_order(&shell.store, &[second.key.clone(), "gone".to_string()])
            .expect("an order with a stranger in it");
        assert_eq!(names(&shell.store), ["second", "first"]);
    }

    #[test]
    fn the_pages_move_is_an_ask_the_shell_takes_and_drops_while_one_is_out() {
        let mut shell = shell_with_home("skin-order-ask");
        press(&mut shell, Message::Go("/skins".into()));
        // The rows placed by hand, because the page's own list is what a move reads: the
        // shell's read would fill them from the store, and this is that state.
        let row = |key: &str| skins::SavedRow {
            entry: crate::saved_skins::Entry {
                key: key.to_string(),
                name: key.to_string(),
                variant: "CLASSIC".to_string(),
                cape: String::new(),
                source: crate::saved_skins::Source::Custom,
                file: format!("{key}.png"),
            },
            ears: false,
        };
        match &mut shell.screen {
            Screen::Skins(state) => state.saved = vec![row("a"), row("b")],
            _ => panic!("the shell is on the skins page"),
        }
        // `act` is what `handle` sends a message through, and its answer is the ask the
        // command is built from: what the press leaves here is the whole order -- the
        // page's list with two neighbours swapped -- and the page waiting on it.
        let Some(Asked::Reorder(order)) = shell.act(Message::Screen(pages::Message::Skins(
            skins::Message::Move { key: "b".to_string(), step: skins::Step::Up },
        ))) else {
            panic!("the press is an ask the shell takes");
        };
        assert_eq!(
            order.keys,
            ["b", "a"],
            "the order the page is drawing, two rows swapped"
        );
        match &shell.screen {
            Screen::Skins(state) => assert!(state.wearing, "and the page waits on the write"),
            _ => panic!("the shell is still on the skins page"),
        }
        drop(shell.render());

        // A second move while the first is out is not an ask at all: two reorders would
        // be two read-modify-writes of the same index.
        assert_eq!(
            shell.act(Message::Screen(pages::Message::Skins(skins::Message::Move {
                key: "a".to_string(),
                step: skins::Step::Down,
            }))),
            None
        );
    }

    #[test]
    fn the_instance_gear_opens_its_settings_and_save_writes_them() {
        // The whole path of one press: the page reports the gear, the shell reads
        // the instance and opens the form, and Save writes the file a launch
        // reads. This is the write side `model.rs` said belonged to a page that
        // had not arrived.
        let mut shell = shell_with_instance("settings");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(instance::Message::Settings)),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the gear opens the instance-settings modal");
        };
        assert_eq!(state.id, "atm10");
        assert_eq!(state.name, "atm10", "the dialog is titled by the instance it is about");
        assert!(state.loaded, "the read landed, so the form has values under it");

        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::OverrideMemory(true)),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::MemoryMin("2048".into())),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::MemoryMax("4096".into())),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Save),
        );
        assert!(shell.modal.is_none(), "a landed save closes the modal");
        let written = shell.store.instance_settings("atm10").expect("read back");
        assert!(written.override_memory);
        assert_eq!((written.memory_min, written.memory_max), (2048, 4096));
        drop(shell.render());
    }

    #[test]
    fn a_heap_that_cannot_be_written_keeps_the_modal_up_with_its_sentence() {
        // A refusal is a sentence in the form rather than a closed modal: the
        // reader keeps what they typed, and the instance file is not written.
        let mut shell = shell_with_instance("settings-refusal");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(instance::Message::Settings)),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::OverrideMemory(true)),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::MemoryMin("twelve".into())),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Save),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the modal stays up");
        };
        let problem = state.error.as_deref().expect("a sentence");
        assert!(problem.contains("twelve"), "{problem}");
        assert!(
            !shell.store.instance_settings("atm10").expect("read back").override_memory,
            "nothing was written"
        );
        drop(shell.render());
    }

    #[test]
    fn the_installation_tab_writes_the_platform_and_the_versions_it_holds() {
        // The second half's whole path, through the same gear the first half's
        // test opens: the form is switched to Installation, a platform, a game
        // version and a build are chosen, and Save writes the profile a launch
        // resolves. The reads the tab asks for are commands, and this shell drops
        // them the way `press` drops every command -- what this test is about is
        // the write, and the store's own tests are where the reads are proven.
        let mut shell = shell_with_instance("installation-save");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(instance::Message::Settings)),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the gear opens the instance-settings modal");
        };
        assert_eq!(
            (state.platform, state.game_version.as_str(), state.loader_build.as_str()),
            (crate::catalog::LoaderKind::Vanilla, "1.21.4", ""),
            "the installation half opens on what the profile says"
        );

        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Tab(
                crate::instance_settings::Tab::Installation,
            )),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Platform(
                crate::catalog::LoaderKind::Quilt,
            )),
        );
        // The game version first: a change to it clears the build chosen for the
        // old one, so the build is picked after it, the way a reader who really
        // means both would.
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::GameVersion(
                "1.21.1".to_string(),
            )),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::LoaderBuild(
                "0.9.2".to_string(),
            )),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::SaveInstallation),
        );
        assert!(shell.modal.is_none(), "a landed save closes the modal");
        let written = shell.store.instance_installation("atm10").expect("read back");
        assert_eq!(written.platform, crate::catalog::LoaderKind::Quilt);
        assert_eq!(written.game_version, "1.21.1");
        assert_eq!(written.loader_build, "0.9.2");
        drop(shell.render());
    }

    #[test]
    fn a_linked_instance_names_its_pack_and_unlinking_forgets_it() {
        // The whole path of the linked card: the gear's read finds the link file
        // beside the profile, the tab's switch is what makes the shell ask the
        // service to name the pack, the answer lands on the form, and Unlink takes
        // the file away while the form -- and the instance -- stay.
        let mut shell = shell_with_instance("installation-link");
        shell
            .store
            .save_instance_link(
                "atm10",
                &store::InstanceLink {
                    project_id: "cobblemon".to_string(),
                    version_id: "pack-1".to_string(),
                },
            )
            .expect("a link (an instance installed from a pack has one)");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(instance::Message::Settings)),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the gear opens the instance-settings modal");
        };
        assert_eq!(
            state.link,
            Load::Ready(store::InstanceLink {
                project_id: "cobblemon".to_string(),
                version_id: "pack-1".to_string(),
            }),
            "the card is drawn from the instance's own link file"
        );
        assert_eq!(state.needs_modpack(), None, "the Java half draws no card");

        // The reader moves to the installation tab, and the name the service
        // answers with lands on the form. `press` drops commands, so the answer is
        // delivered here as the message the command's own future resolves to --
        // which is exactly what the shell sees when it arrives for real.
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Tab(
                crate::instance_settings::Tab::Installation,
            )),
        );
        press(
            &mut shell,
            Message::Modpack(Ok(Some(store::LinkedModpack {
                project_id: "cobblemon".to_string(),
                title: "Cobblemon".to_string(),
                author: "jellysquid3".to_string(),
                version: "1.6.1".to_string(),
            }))),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the modal is still up");
        };
        assert_eq!(
            state.modpack.ready().map(|pack| pack.title.as_str()),
            Some("Cobblemon"),
            "the card's name is the service's and it is on the form"
        );

        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Unlink),
        );
        assert!(shell.modal.is_some(), "unlinking is not closing the modal");
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the modal is still up");
        };
        assert_eq!(state.link, Load::Idle, "the link is gone from the form");
        assert_eq!(state.modpack, Load::Idle, "and so is the name read from it");
        assert_eq!(
            shell.store.instance_link("atm10").expect("read back"),
            None,
            "and gone from the disk"
        );
        assert!(
            shell.store.instance_installation("atm10").is_ok(),
            "the profile the installation tab edits is not the link's to touch"
        );
        drop(shell.render());
    }

    #[test]
    fn pressing_repair_marks_the_form_busy_and_the_answer_stays_on_it() {
        // What the shell owns of the installation tab's *Repair instance*: the
        // press raises the check off the frame thread (`press` drops the command,
        // which is the future a real window runs), the form is busy in the same
        // frame so the button is drawn disabled, a second press while it runs does
        // nothing, and the sentence comes back to the form rather than to a closed
        // modal or a notification.
        let mut shell = shell_with_instance("installation-repair");
        shell
            .store
            .save_instance_link(
                "atm10",
                &store::InstanceLink {
                    project_id: "cobblemon".to_string(),
                    version_id: "pack-1".to_string(),
                },
            )
            .expect("a link (the repair is drawn on a linked instance)");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(instance::Message::Settings)),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Tab(
                crate::instance_settings::Tab::Installation,
            )),
        );

        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Repair),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("repairing does not close the modal");
        };
        assert_eq!(state.repair, Load::Loading, "the form is busy from the press");
        assert!(
            shell.installation_repair_requested.is_none(),
            "and the request left in the same frame"
        );

        // A second press while the check runs is dropped rather than starting a
        // second install over the first. The form draws the button disabled; this
        // is the rule behind that. (The request flag is `None` here, so a second
        // command cannot leave either.)
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Repair),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the modal is still up");
        };
        assert_eq!(state.repair, Load::Loading);
        assert!(shell.installation_repair_requested.is_none());

        // The answer -- what the command's own future resolves to in a real
        // window, which `press` cannot run -- lands on the form.
        press(
            &mut shell,
            Message::Repaired(Ok(
                "Repaired 'All the Mods 10': 412 file(s) checked, 1 fetched again (2.4 MB)"
                    .to_string(),
            )),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the sentence is drawn on the form");
        };
        assert!(
            state
                .repair
                .ready()
                .is_some_and(|line| line.contains("412 file(s) checked")),
            "got: {:?}",
            state.repair
        );

        // And a refusal is a sentence too, on the same form: a repair that could
        // not finish does not close the window it was asked from.
        press(
            &mut shell,
            Message::Repaired(Err("resolution failed with errors".to_string())),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("a failed repair keeps the modal");
        };
        assert!(
            matches!(&state.repair, Load::Failed(reason) if reason == "resolution failed with errors"),
            "got: {:?}",
            state.repair
        );
        drop(shell.render());
    }

    /// A shell with one instance, a link to a Modrinth pack on it, and the
    /// instance-settings modal open on the installation tab.
    ///
    /// The three pack actions are all drawn on that tab and only for a linked
    /// instance, so the three tests below open the same way.
    fn shell_on_installation_tab(name: &str) -> Shell {
        let mut shell = shell_with_instance(name);
        shell
            .store
            .save_instance_link(
                "atm10",
                &store::InstanceLink {
                    project_id: "cobblemon".to_string(),
                    version_id: "pack-1".to_string(),
                },
            )
            .expect("a link");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(instance::Message::Settings)),
        );
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::Tab(
                crate::instance_settings::Tab::Installation,
            )),
        );
        shell
    }

    #[test]
    fn pressing_reinstall_marks_the_form_busy_and_the_sentence_stays_on_it() {
        // What the shell owns of the installation tab's *Re-install modpack*: the
        // press raises the fetch off the frame thread (`press` drops the command,
        // which is the future a real window runs), the form is busy in the same
        // frame so the button is drawn disabled under the reference's own word
        // for the wait, a second press while the first is out is dropped rather
        // than starting a second fetch of the same archive, and the sentence
        // lands on the form rather than on a page or a notification.
        let mut shell = shell_on_installation_tab("installation-reinstall");
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::ReinstallModpack),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("re-installing does not close the modal");
        };
        assert_eq!(state.reinstall, Load::Loading, "the form is busy from the press");
        assert!(
            shell.installation_reinstall_requested.is_none(),
            "and the request left in the same frame"
        );

        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::ReinstallModpack),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the modal is still up");
        };
        assert_eq!(state.reinstall, Load::Loading);
        assert!(shell.installation_reinstall_requested.is_none());

        // The answer -- what the command's own future resolves to in a real
        // window, which `press` cannot run -- lands on the form.
        press(
            &mut shell,
            Message::Reinstalled(Ok(
                "Re-applied Cobblemon 1.6.1: 12 file(s), 1 fetched again (0.7 MB)".to_string(),
            )),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the sentence is drawn on the form");
        };
        assert!(
            state
                .reinstall
                .ready()
                .is_some_and(|line| line.contains("Re-applied Cobblemon 1.6.1")),
            "got: {:?}",
            state.reinstall
        );
        assert_eq!(
            state.link.ready().map(|link| link.version_id.as_str()),
            Some("pack-1"),
            "a re-install lays the same version over again and moves no link"
        );

        // And a refusal is a sentence too, on the same form.
        press(
            &mut shell,
            Message::Reinstalled(Err("the archive could not be fetched".to_string())),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("a failed re-install keeps the modal");
        };
        assert!(
            matches!(&state.reinstall, Load::Failed(reason) if reason == "the archive could not be fetched"),
            "got: {:?}",
            state.reinstall
        );
        drop(shell.render());
    }

    #[test]
    fn a_version_change_rewrites_the_link_and_re_reads_the_card_and_the_list() {
        // *Change version* is the one pack action that moves the link: another
        // version's files are laid over the instance and the link file is
        // rewritten with them. The shell checks the pair that travels, and when
        // the answer lands it reads the link *off the disk* again rather than
        // trusting the sentence -- a card naming the old version over files the
        // new one wrote is the one thing this could get wrong.
        let mut shell = shell_on_installation_tab("installation-change");
        // The Java/installation reads the tab made when it opened are settled
        // first, so the one question this test is about is the one the shell
        // asks next.
        press(&mut shell, Message::InstallationVersions(Ok(version_list())));

        shell.request_change_version("pack-2");
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the modal is up");
        };
        assert_eq!(state.change, Load::Loading, "the rows stop pressing from the press");
        assert_eq!(
            shell.installation_change_requested,
            Some(("atm10".to_string(), "pack-2".to_string())),
            "the instance and the version travel together"
        );
        // A second press while the first is out is dropped rather than queued.
        shell.request_change_version("pack-3");
        assert_eq!(
            shell.installation_change_requested,
            Some(("atm10".to_string(), "pack-2".to_string())),
            "the second press changed nothing"
        );
        // What the real window does on the frame the flag was set: `handle`
        // takes it and builds the command from it.
        press(
            &mut shell,
            Message::InstanceSettings(crate::instance_settings::Message::ChangeVersion(
                "pack-2".to_string(),
            )),
        );
        assert!(
            shell.installation_change_requested.is_none(),
            "the request left in the same frame"
        );

        // The answer arrives after the worker rewrote the link, which is the
        // order `Store::change_pack_version` keeps.
        shell
            .store
            .save_instance_link(
                "atm10",
                &store::InstanceLink {
                    project_id: "cobblemon".to_string(),
                    version_id: "pack-2".to_string(),
                },
            )
            .expect("the new link");
        press(
            &mut shell,
            Message::VersionChanged(Ok(
                "Re-applied Cobblemon 1.6.2: 12 file(s), 2 fetched again (4.1 MB)".to_string(),
            )),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the sentence is drawn on the form");
        };
        assert!(
            state.change.ready().is_some_and(|line| line.contains("1.6.2")),
            "got: {:?}",
            state.change
        );
        assert_eq!(
            state.link.ready().map(|link| link.version_id.as_str()),
            Some("pack-2"),
            "the link is read off the disk, not assumed from the press"
        );
        assert_eq!(
            state.modpack, Load::Loading,
            "the card is asked again, because it names the version"
        );
        assert_eq!(
            state.pack_versions, Load::Idle,
            "and the list follows when the card's answer lands -- one question per frame"
        );
        press(
            &mut shell,
            Message::Modpack(Ok(Some(store::LinkedModpack {
                project_id: "cobblemon".to_string(),
                title: "Cobblemon".to_string(),
                author: "jellysquid3".to_string(),
                version: "1.6.2".to_string(),
            }))),
        );
        let Some(Modal::InstanceSettings(state)) = &shell.modal else {
            panic!("the modal is up");
        };
        assert_eq!(
            state.pack_versions, Load::Loading,
            "the list is read once the card's answer lands, so its mark can move"
        );
        drop(shell.render());
    }

    // ---- The scrollbar ---------------------------------------------------

    #[test]
    fn the_reference_scrollbar_is_six_wide_and_stands_two_clear_of_the_edge() {
        // The audit's two bands, as numbers. `x=1272..1277` is what every
        // reference capture measures for the bar at the window's right edge, and
        // `x=1278..1279` is the clear wash beside it -- which is the only reason
        // `SCROLLBAR_PAD` exists, because iced reserves `width + 2 * margin` and
        // centres the bar in it: a margin of zero would put a six-pixel bar
        // flush against the window and land at 1274.
        assert_eq!(bar_at(1280.0), 1272.0);
        assert_eq!(bar_at(1280.0) + SCROLLBAR, 1278.0, "six columns, 1272..1277");
        // The reserve is ten, which is the same ten the dialog bodies are broken
        // for: two, six, two. `DIALOG_SCROLLBAR` is the number that has to keep
        // meaning "the room this bar takes", and it is asserted as the identity
        // rather than as a literal so that moving either half fails here.
        assert_eq!(SCROLLBAR + 2.0 * SCROLLBAR_PAD, DIALOG_SCROLLBAR);
        // And ten is iced's own bar, which is what the page column's gutter has
        // to cover rather than the six the reference reserves, so the strip is
        // ten wide with the reference's six at its right-hand end.
        assert_eq!(ICED_BAR, 10.0);
        assert!(ICED_BAR > SCROLLBAR, "a gutter narrower than the bar would show part of it");
        assert_eq!(GUTTER_BAR + GUTTER, ICED_BAR, "four of the page's own ink, then the gutter");
        assert_eq!(GUTTER, 6.0, "x=974..979, six columns of the reserved band");
    }

    /// `--surface-1` under one of the shadow's depths, as the bytes a capture
    /// reads. The rounding is to nearest, which is what a compositor does to a
    /// premultiplied quad and what the reference's own engine did.
    #[cfg(test)]
    fn under(depth: f32) -> (u8, u8, u8) {
        let bg = theme_gen::ink(Gen::Dark, Ink::Bg);
        (
            (bg.r * (1.0 - depth) * 255.0).round() as u8,
            (bg.g * (1.0 - depth) * 255.0).round() as u8,
            (bg.b * (1.0 - depth) * 255.0).round() as u8,
        )
    }

    #[test]
    fn the_reserved_gutter_and_the_panes_edge_are_the_same_three_depths() {
        // `#15171B` is not a token: nothing in the vendored reference declares
        // it, and `variables.scss:233` fixes `--surface-1` at `#16181c`. What the
        // capture says it is, is `--surface-1` under the depth the pane's own
        // inset shadow holds across its middle six pixels -- which is why the
        // gutter is painted as a depth and not as an ink, and why the two
        // measurements have to agree on the three numbers.
        //
        // The pairs are the capture's, to the byte: the ramp's own three columns
        // on the pane's left edge (`ref/user-ref.png`, `y=300`) and the reserved
        // band's two on the pane's right edge (`x=974`, `x=975`).
        assert_eq!(under(SHADOW_HARD), (0x15, 0x17, 0x1A), "x=65, y=49");
        assert_eq!(under(SHADOW_MID), (0x15, 0x17, 0x1B), "x=66..71, y=50..55, x=975..979");
        assert_eq!(under(SHADOW_SOFT), (0x16, 0x18, 0x1B), "x=72, y=56, x=974");
        // And under nothing, the page's own `--surface-1`, which is what the
        // reference measures at x=964..973 and at x=73 / y=57.
        assert_eq!(under(0.0), (0x16, 0x18, 0x1C));
        assert_eq!(SHADOW_REACH, 8.0, "one, six, one: eight pixels");
        assert_eq!(
            (SHADOW_HARD_W, SHADOW_MID_W, SHADOW_SOFT_W),
            (1.0, 6.0, 1.0),
            "x=65, x=66..71, x=72; y=49, y=50..55, y=56"
        );
    }

    #[test]
    fn the_bar_is_transparent_behind_and_the_reference_scrollbar_coloured_in_front() {
        // The two halves of `Bar`, read back off the stylesheet rather than off
        // the source: the track is `None` because both of the reference's bars
        // are transparent behind the handle (`global.scss:137-139` for a native
        // one, overlayscrollbars' default `--os-track-bg` for the sidebar's), and
        // the handle is `--color-scrollbar`, which is `--surface-5` = `#42444a`
        // in dark. `theme_gen.rs:632` carries it, so this is the reference's own
        // token rather than a colour typed in.
        use iced::widget::scrollable::StyleSheet as _;
        let bar = Bar { theme: Gen::Dark };
        let appearance = bar.active(&widget_theme(Gen::Dark));
        assert!(appearance.scrollbar.background.is_none(), "the track is transparent");
        assert_eq!(
            theme_gen::ink_rgba(Gen::Dark, Ink::Scrollbar),
            [0x42, 0x44, 0x4a, 0xff],
            "the handle is the reference's own --color-scrollbar"
        );
        assert_eq!(appearance.scrollbar.scroller.color, theme_gen::ink(Gen::Dark, Ink::Scrollbar));
        // Fully rounded ends: a radius wider than half the bar is half the bar,
        // which is what `--radius-lg` (1rem) comes to on a six-pixel box.
        assert!(SCROLLBAR / 3.0 <= SCROLLBAR / 2.0);
        // Hover and drag are the same appearance, because the reference's only
        // hover rule raises an opacity this port's captures already measure as 1.
        // `scrollable::Appearance` is not `PartialEq`, so the handle is what is
        // compared -- the rest of the appearance is the transparent track read
        // back a line above.
        for other in [
            bar.hovered(&widget_theme(Gen::Dark), true),
            bar.hovered(&widget_theme(Gen::Dark), false),
            bar.dragging(&widget_theme(Gen::Dark)),
        ] {
            assert_eq!(
                other.scrollbar.scroller.color,
                appearance.scrollbar.scroller.color,
                "there is no measured hover colour to draw"
            );
            assert!(other.scrollbar.background.is_none());
        }
        // Every generated theme answers, which is the point of carrying a `Gen`
        // rather than reaching into iced's `Theme`.
        for theme in Gen::ALL {
            let appearance = Bar { theme: *theme }.active(&widget_theme(*theme));
            assert_eq!(
                appearance.scrollbar.scroller.color,
                theme_gen::ink(*theme, Ink::Scrollbar),
                "{theme:?}"
            );
        }
    }

    #[test]
    fn the_pages_reserved_gutter_covers_the_bar_and_not_the_page() {
        // The strip the pane draws over its page, and the reason it is drawn at
        // all: a page's own region is built in `page.rs`, so its bar cannot be
        // restyled from here and is covered instead. What has to be true is that
        // the strip and the bar occupy the same ten columns -- otherwise the
        // gutter covers nothing and the band survives it.
        // Two routes that force the panel whatever the setting says, and one that
        // does not -- so the strip is proved against both ways the pane's own
        // right edge is arrived at.
        for (address, panel) in [
            ("/user/FlameFire", true),
            ("/browse/modpack", true),
            ("/skins", false),
        ] {
            let mut shell = Shell::new(
                Address::parse(address).expect("a sample address"),
                Gen::Dark,
                &settings(!panel, true, true),
            );
            press(&mut shell, Message::Viewport(iced::Size::new(1280.0, 720.0)));
            assert_eq!(shell.panel_shown(), panel, "{address}");
            let pane = 1280.0 - RAIL - if panel { PANEL } else { 0.0 };
            // The page's region ends at the pane's right edge, so iced's bar is
            // the last ten columns of it, and the gutter starts where the bar
            // starts.
            assert_eq!(
                shell.pane_gutter_at().x,
                pane - ICED_BAR,
                "{address}: the strip covers the bar"
            );
            // And it stops at the pane's edge rather than over the panel, which
            // at 1280x720 with the panel up is `x=980`: the strip owns
            // `970..980`, which is exactly the ten columns iced's default bar
            // occupied at `x=970..979` in the audit's capture.
            assert_eq!(RAIL + shell.pane_gutter_at().x + ICED_BAR, RAIL + pane, "{address}");
            // The strip can only ever cover padding, because a page's own inset
            // keeps content clear of the pane's edge by more than the bar's
            // width. `p-6` is the library pages' and sixteen is Skins', so the
            // narrowest of them is the one that matters.
            assert!(
                crate::page::INSET > ICED_BAR,
                "a page's content would reach under the strip at p-{}",
                crate::page::INSET
            );
            drop(shell.render());
        }
    }

    #[test]
    fn the_gutter_follows_the_window_rather_than_being_pinned() {
        // Resizing is the whole risk in a strip whose position is arithmetic: a
        // strip pinned to the first window's width would eat the page's content
        // on every other one. Every one of these has to end flush with the pane,
        // including the window's own floor (`MINIMUM_SIZE` in `main.rs`).
        for (width, height) in [(980.0, 640.0), (1280.0, 720.0), (1920.0, 1080.0)] {
            for panel in [true, false] {
                let mut shell = Shell::new(
                    Address::parse("/").expect("a sample address"),
                    Gen::Dark,
                    &settings(!panel, true, true),
                );
                press(&mut shell, Message::Viewport(iced::Size::new(width, height)));
                let pane = width - RAIL - if panel { PANEL } else { 0.0 };
                assert_eq!(shell.pane_gutter_at().x + ICED_BAR, pane, "{width}x{height} panel={panel}");
                assert!(pane > ICED_BAR, "the pane is wider than the strip it draws");
                drop(shell.render());
            }
        }
    }

    #[test]
    fn only_the_hosting_page_lends_the_pane_a_layer_of_its_own() {
        // The one thing [`Shell::page_overlay`] can get wrong by existing at all:
        // a layer on a page the reference draws none on would sit over the gutter
        // on every route, and the gutter is what hides iced's bar. So it is the
        // hosting route and nothing else, and the pane it is clipped to is the
        // page column -- which is narrower than the stack the layer is in, because
        // that stack's base layer is the row holding the page *and* the panel.
        for (address, lends) in [
            ("/hosting/manage", true),
            ("/user/FlameFire", false),
            ("/browse/modpack", false),
            ("/skins", false),
            ("/instance/atm10", false),
        ] {
            for panel in [true, false] {
                let mut shell = Shell::new(
                    Address::parse(address).expect("a sample address"),
                    Gen::Dark,
                    &settings(!panel, true, true),
                );
                press(&mut shell, Message::Viewport(iced::Size::new(1280.0, 720.0)));
                let page = 1280.0 - RAIL - if panel { PANEL } else { 0.0 };
                assert_eq!(
                    shell.page_overlay(shell.theme, page).is_some(),
                    lends,
                    "{address} panel={panel}"
                );
                // And the clip that keeps the toast inside it: the page column is
                // the pane's own width from the pane's own top, so the pane's
                // height is what is left under the bar.
                assert_eq!(720.0 - BAR, 672.0, "the page column's height at 720");
                assert!(page < 1280.0, "the page column is not the whole window");
                drop(shell.render());
            }
        }
    }
}
