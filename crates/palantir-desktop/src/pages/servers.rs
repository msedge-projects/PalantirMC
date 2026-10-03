//! Servers, the reference's fifth rail slot: the Modrinth Hosting listing at
//! `/hosting/manage/`.
//!
//! The reference's page is a listing of servers the user has bought from Modrinth
//! Hosting, and every fact on it comes from that service: the plans, their state,
//! the backups, the console. There is no local half to draw -- a server is not a
//! file on this machine -- and no account half either, because this launcher does
//! not hold a Modrinth credential (G118).
//!
//! Those two facts decide the page rather than describing it, because the
//! reference's `index.vue` reaches the same two conclusions and draws the same
//! thing for them. `serverList` is empty unless `loggedIn` is true
//! (`index.vue:526`), so with no account `showEmptyState` is true
//! (`index.vue:530`) and the listing branch is never reached: what the reference
//! draws at this route for a reader who is not signed in is `ServerListEmpty`
//! with `logged-in="false"` -- the marketing column, the decorative preview, and
//! the *Already have a server?* sign-in at the foot. That is this page, and it is
//! drawn from the reference's own keys rather than from a paraphrase of them.
//!
//! The rail slot's tooltip is `app.nav.modrinth-hosting` -- "Modrinth Hosting",
//! which is what the button means. A launcher that runs a server *of its own* --
//! a jar in an instance's folder, started like a game -- is a different feature
//! from this page and would not be this route.
//!
//! # Three things the reference draws that this toolkit has no word for
//!
//! Each is here rather than left out, and each is recorded at the point it is
//! drawn, because all three are the difference between a picture and an
//! approximation of one:
//!
//! * **Layers over one another.** `ServerListEmptyPreview.vue` paints its
//!   bottom fade over the panel and then the invite toast over *that*, both with
//!   `position: absolute`, and a feature's plate carries three more layers
//!   before its glyph. `iced::widget::stack` does not exist in iced 0.12.3 --
//!   `grep -rn "pub fn stack" iced_widget-0.12.3` answers nothing, and the
//!   `overlay` module is `overlay::menu` alone -- so [`Stack`] is the local
//!   equivalent: an element, an offset, and a paint order, on the same
//!   `iced::advanced::widget::Widget` footing `crate::scroll`'s wheel guard
//!   uses.
//! * **Blend modes.** `ServerListEmpty.vue`'s texture is
//!   `mix-blend-luminosity`. iced has no term for a blend mode, so the texture's
//!   own colour is folded into the share the reference's pixels show it reaches
//!   -- see [`TEXTURE_SHARE`] -- and what is left of the blend is the texture's
//!   shading structure at that share.
//! * **The reference's own button types.** `type="base"` is
//!   `ButtonFrame.vue`'s default and is not one of `ui::Kind`'s five; the
//!   preview's buttons also carry `!h-8`, `w-20` and `!font-medium`, which no row
//!   of `ButtonFrame`'s size table produces. `ui.rs` is reserved, so those are
//!   drawn here from the frame's own rules -- see [`frame_button`].

use std::sync::OnceLock;

use iced::gradient::{self, Gradient};
use iced::widget::{column, container, image, row, Space};
use iced::{
    Alignment, Background, Border, Color, ContentFit, Element, Length, Padding, Radians,
    Theme, Vector,
};

use crate::avatar;
use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::{self, GAP};
use crate::store::{self, Store};
use crate::style::{medium, regular, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Ink, Span, Theme as Gen};
use super::overlay::Stack;
use crate::ui::{self, text};

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// A new server was asked for.
    NewServer,
    /// The listing was asked for again.
    Refresh,
    /// Billing was asked for.
    ManageBilling,
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

    /// A wheel over this page's scroll region.
    ///
    /// Reported rather than applied: iced moves a scrollable with a `scroll_to`
    /// command, so which region glides, and how far, is the shell's -- see
    /// `crate::scroll`. This page's part is to hand the wheel on, and the name it
    /// carries is the region the widget was built with.
    Wheel(&'static str, crate::scroll::Wheel),
}

crate::hovered!(Message);

/// The listing's actions.
const MANAGE_BILLING_KEY: &str = "servers:manage-billing";
const NEW_SERVER_KEY: &str = "servers:new";
const REFRESH_KEY: &str = "servers:refresh";
const SIGN_IN_KEY: &str = "servers:sign-in";

/// The page's own state.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// The last thing the page could not do.
    pub notice: Option<String>,
}

impl State {
    /// Apply a message.
    ///
    /// Every one of the three answers with [`store::needs_account`] rather than
    /// with "not implemented yet": the listing, a new server and the billing page
    /// are all Modrinth Hosting, which is an account service, and the sentence a
    /// reader gets has to say which of the two kinds of gap this is. The *page*
    /// still draws `ServerListEmpty`, because that is what the reference draws for
    /// this same reader -- the notice is added above it, not swapped for it.
    pub fn update(&mut self, message: Message) {
        match message {
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

            Message::NewServer => self.notice = Some(store::needs_account("Creating a server")),
            Message::ManageBilling => self.notice = Some(store::needs_account("Billing")),
            Message::Refresh => self.notice = Some(store::needs_account("The server listing")),
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
        }
    }
}


// ---- ServerListEmpty.vue, quoted ------------------------------------------

/// `max-w-[20rem]` on the column that holds the heading, the features and the
/// buttons -- the reference's own measure for it, and the reason its description
/// wraps where it does.
const COLUMN: f32 = 320.0;
/// `max-w-[25rem]` on `ServerListEmptyPreview`.
const PREVIEW: f32 = 400.0;
/// `gap-8`, between the heading, the features and the buttons.
const COLUMN_GAP: f32 = 32.0;
/// `gap-8`, between the preview row and the sign-in at the foot.
const FOOT_GAP: f32 = 32.0;
/// `gap-6`, between the three features.
const FEATURE_GAP: f32 = 24.0;
/// `gap-4`, across a feature: its plate and its two lines.
const FEATURE_GAP_X: f32 = 16.0;
/// `gap-0.5`, between a feature's title and its description.
const FEATURE_TEXT_GAP: f32 = 2.0;
/// `gap-4`, between the *New server* button and *Learn more*, and between the two
/// parts of the sign-in at the foot.
const ACTION_GAP: f32 = 16.0;
/// `gap-2`, between the plate's glyph and its text.
const PLATE_GAP: f32 = 8.0;

/// `size-10` on a feature's plate, and `rounded-[0.875rem]` on it.
const FEATURE_PLATE: f32 = 40.0;
const FEATURE_PLATE_RADIUS: f32 = 14.0;
/// `size-5`, the glyph inside the plate and the arrow beside *Learn more*.
const GLYPH: f32 = 20.0;

/// `h-[38rem]` on the preview: the reference draws it taller than the viewport and
/// lets the page clip it, which is why its foot is under a fade.
const PREVIEW_HEIGHT: f32 = 608.0;
/// `-mb-10` on the preview's own root.
///
/// A negative bottom margin is how the reference gets a 608-pixel box into a
/// 568-pixel row and centres the *margin* box rather than the border box, so the
/// panel hangs twenty pixels above and below the row it is in. iced has no
/// negative margin, so the row is made 40 shorter and the panel is laid at the
/// row's own top: the panel's height and the foot's distance from it are the
/// reference's, and the panel's top is the row's rather than twenty above it.
const PREVIEW_MARGIN_BOTTOM: f32 = 40.0;
/// The row the panel is centred in, once `-mb-10` is taken off its height.
const PREVIEW_ROW: f32 = PREVIEW_HEIGHT - PREVIEW_MARGIN_BOTTOM;

/// `gap-2` on the row that holds the column and the preview -- the one gap among
/// the four auto margins, and the reason the middle spacer is wider than the two
/// that bracket it.
const ROW_GAP: f32 = 8.0;
/// One of the four auto margins, in pixels, at the reference's own window.
const MARGIN: u16 = 35;
/// The free space the four `mx-auto` margins share, and the one gap between them.
///
/// `ServerListEmpty`'s row is `flex-wrap items-center justify-center gap-2` and
/// *both* of its children carry `mx-auto`, so the row's free space is split evenly
/// between four auto margins and `justify-content` never runs. Measured at the
/// reference's own 1280x720 (`/tmp/ref/hosting-clean3.png`): a content column
/// 868 wide, `868 - 320 - 400 - 8 = 140` free, so 35 a margin -- the column's
/// text starts at x=123 (`88 + 35`) and the preview at x=521
/// (`123 + 320 + 35 + 35 + 8`).
///
/// The row below is therefore drawn with *no* spacing of its own and three
/// spacers instead, because iced adds a row's spacing between every pair of
/// children and this row has one gap among five. The middle spacer carries two
/// margins and that gap: `35 + 8 + 35 = 78`, and `35 + 78 + 35 = 148` is the whole
/// of the free space, so the picture lands on the reference's own pixels at the
/// reference's own window and keeps its shape at another.
const MARGIN_SHARE: [u16; 3] =
    [MARGIN, (MARGIN as f32 + ROW_GAP + MARGIN as f32) as u16, MARGIN];

/// The three features `ServerListEmpty.vue` lists, in its own order.
const FEATURES: [(Glyph, Key, Key); 3] = [
    (
        Glyph::PackageOpen,
        Key::ServersListEmptyOneClickModInstallsTitle,
        Key::ServersListEmptyOneClickModInstallsDescription,
    ),
    (
        Glyph::Globe,
        Key::ServersListEmptySimpleSetupTitle,
        Key::ServersListEmptySimpleSetupDescription,
    ),
    (
        Glyph::Users,
        Key::ServersListEmptyPlayWithFriendsTitle,
        Key::ServersListEmptyPlayWithFriendsDescription,
    ),
];

// ---- The plate's three layers --------------------------------------------

/// `h-[6.25rem] w-[9.8125rem]` on the `<img>` of `icon-texture.png`, centred on
/// the plate by `left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2`, and
/// `opacity-40`. Height first, because that is the order the two classes are
/// written in.
const TEXTURE_BOX: (f32, f32) = (100.0, 157.0);

/// The two ends of the slice of the green ramp a plate shows.
///
/// `.feature-icon-gradient` is `linear-gradient(180deg, var(--color-green-800)
/// 0%, var(--color-green-950) 100%)` at `opacity: 0.5` over a `size-[6.25rem]`
/// layer -- 100 by 100 -- placed at `left-[-1px] top-[-1px]`, so plate row `y` is
/// `(y + 1) / 100` of the way along it. `overflow: hidden` clips that layer to
/// the plate's 38-pixel padding box, which is rows 1..38 of it: the ramp's
/// **first 39 percent**, and nothing after it is ever on screen.
///
/// That slice is why the ramp is not handed to iced as a
/// [`Background::Gradient`] at all. A gradient can only be aimed inside the box
/// it is given, the plate's own box is 40 rows, and 39 percent of a 400-step
/// ramp spread over 38 rows is four hundredths of a step per row: any two-stop
/// gradient a 40-pixel box can express is either the whole ramp (which is what
/// this used, and which reaches `--color-green-950` at row 38 where the
/// reference is still 39 percent along) or a span so short that every pixel
/// clamps to the same stop. So the ramp is composited per row into the same
/// picture as the shade and the texture, which is the only place the slice
/// survives, and these two numbers are where the slice begins and ends.
const RAMP_FROM: f32 = 2.0 / 100.0;
const RAMP_TO: f32 = 39.0 / 100.0;

/// The plate's 1-pixel border, which is what `overflow: hidden` clips the layers
/// inside it to: `size-10` less a border either side is a 38-pixel padding box.
const PLATE_INSET: u32 = 1;

/// `.feature-icon-shade`: `linear-gradient(-14deg, color-mix(in srgb,
/// var(--color-green-950) 37%, transparent) 8%, transparent 86%)`.
const SHADE_FROM: f32 = 0.08;
const SHADE_TO: f32 = 0.86;
const SHADE_ALPHA: f32 = 0.37;

/// `opacity-40` on the texture `<img>`.
const TEXTURE_ALPHA: f32 = 0.40;

/// How much of the texture's own colour reaches the plate.
///
/// `mix-blend-luminosity` has no term in iced, and it is not a small residual:
/// the blend keeps the source's hue and saturation and takes the *backdrop's*
/// luminosity, so what it leaves of `icon-texture.png` is its own shading
/// structure and not its blue-grey -- and the texture's blue is what puts steps
/// into the plate's blue channel. Measured against
/// `/tmp/ref/hosting-clean3.png` over the 280 pixels of the three plates' pads
/// that carry no glyph and no antialiased corner:
///
/// | model | rms /255 |
/// |---|---|
/// | `ramp + 0.33 * (green-950 - ramp) + 0.07 * (texture - ramp)` | **2.4** |
/// | the texture at the `opacity-40` its class also carries | 8.6 |
/// | `ramp` alone | 5.1 |
///
/// 0.33 + 0.07 is the 0.4 the `opacity-40` slot is worth, so the slot is
/// `(1 - TEXTURE_SHARE) * --color-green-950 + TEXTURE_SHARE * texture` at
/// `TEXTURE_SHARE = 0.175`. The texture is still decoded and still lays its
/// structure into the plate at that share, which is what the reference's own
/// pixels show: a variation of about 3/255 in green across the pad.
const TEXTURE_SHARE: f32 = 0.175;

/// `icon-texture.png`, byte for byte from
/// `vendor/modrinth-app/ui/src/assets/welcome/`; see `THIRD_PARTY_NOTICES.md`.
const TEXTURE_PNG: &[u8] = include_bytes!("../../assets/hosting/icon-texture.png");

/// The ramp a plate's own surface is filled with at one row: `--color-green-800`
/// to `--color-green-950` at `t`, over the `--color-surface-1` the plate is
/// already sitting on, at the `opacity: 0.5` `.feature-icon-gradient` carries.
///
/// In 0..1, the scale a [`Color`] is in, so it drops straight into the composite.
fn plate_ramp(theme: Gen, t: f32) -> [f32; 3] {
    let green_800 = theme_gen::ink(theme, Ink::Green800);
    let green_950 = theme_gen::ink(theme, Ink::Green950);
    let surface = theme_gen::ink(theme, Ink::Surface1);
    [
        0.5 * (green_800.r + (green_950.r - green_800.r) * t) + 0.5 * surface.r,
        0.5 * (green_800.g + (green_950.g - green_800.g) * t) + 0.5 * surface.g,
        0.5 * (green_800.b + (green_950.b - green_800.b) * t) + 0.5 * surface.b,
    ]
}

/// The plate's two layers that are not a gradient: `.feature-icon-shade` and
/// `icon-texture.png`, as one 40x40 picture.
///
/// Both are a function of the plate's own 40 pixels and of nothing else, so they
/// are composited once per theme rather than per frame, and the result is one
/// `image`. Two things make that a composite rather than two widgets:
///
/// * **The clip.** `overflow: hidden` clips the layers to the plate's *padding*
///   box, whose rounded corner is the border's radius less the border's width --
///   14 - 1 = 13 on the 38 pixels from `(1, 1)`. iced's `Container::clip` clips to
///   the plain rectangle (`layout.bounds().intersection(viewport)`), so the
///   rounded corner is cleared here instead.
/// * **The paint order.** the shade, then the texture over it. That is the
///   source order of the two `absolute` divs and the `<img>`, and source-over is
///   associative, so compositing the pair onto a transparent plate and then
///   drawing that over the ramp is the same picture as compositing each onto the
///   ramp in turn.
///
/// `mix-blend-luminosity` is the one thing left out, and it is left out because
/// nothing in iced has a term for a blend mode. The texture is drawn as the
/// forty percent `opacity-40` it also carries.
fn plate_overlay(theme: Gen) -> iced::widget::image::Handle {
    static CACHE: [OnceLock<iced::widget::image::Handle>; 4] =
        [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
    CACHE[theme_slot(theme)]
        .get_or_init(|| {
            iced::widget::image::Handle::from_pixels(
                FEATURE_PLATE as u32,
                FEATURE_PLATE as u32,
                plate_overlay_pixels(theme),
            )
        })
        .clone()
}

/// The plate's four themes in [`Gen::ALL`] order, so a cache can be an array.
fn theme_slot(theme: Gen) -> usize {
    Gen::ALL.iter().position(|candidate| *candidate == theme).unwrap_or(0)
}

/// The pixels of [`plate_overlay`]: `40 * 40` RGBA, straight (not premultiplied).
///
/// All three of the plate's own layers, in the source's order, composited once
/// per theme: `bg-surface-1` under the `.feature-icon-gradient` ramp at its
/// `opacity: 0.5`, then `.feature-icon-shade`, then `icon-texture.png` at its
/// `opacity-40` -- and the result is opaque, because the padding box it is
/// painted into is the plate's whole interior and the plate's own background
/// would only show through where this clears it.
///
/// iced's tiny-skia backend premultiplies a decoded picture on upload
/// (`iced_tiny_skia-0.12.1/src/raster.rs`: `ColorU8::from_rgba(..).premultiply()`),
/// so what goes in here is straight colour and the layer's own alpha, and each
/// layer is multiplied by that alpha *before* the next one goes over it. Writing
/// the shade's colour at full strength where the shade's own gradient is
/// transparent, and applying `opacity-40` to the texture twice, is what put the
/// plate's green two steps low and its blue four steps high.
fn plate_overlay_pixels(theme: Gen) -> Vec<u8> {
    let side = FEATURE_PLATE as usize;
    let shade = theme_gen::ink(theme, Ink::Green950);
    let window = texture_window();
    let mut out = vec![0u8; side * side * 4];
    for y in 0..side {
        for x in 0..side {
            if !inside_padding(x, y) {
                continue;
            }
            // `.feature-icon-gradient`'s layer is 100 square and the shade is its
            // sibling, so both are placed in the same 100-by-100 space and the
            // padding box is rows 1..38 of it.
            let t = ((y as f32 + PLATE_INSET as f32) / 100.0).clamp(RAMP_FROM, RAMP_TO);
            let mut rgb = plate_ramp(theme, t);
            let shade_at =
                shade_alpha(x as f32 + PLATE_INSET as f32, y as f32 + PLATE_INSET as f32) * shade.a;
            for (channel, value) in [shade.r, shade.g, shade.b].into_iter().enumerate() {
                rgb[channel] = value * shade_at + rgb[channel] * (1.0 - shade_at);
            }
            // The window is the padding box, so its own `(0, 0)` is the plate's
            // `(1, 1)`.
            let inner = (x.checked_sub(PLATE_INSET as usize), y.checked_sub(PLATE_INSET as usize));
            let pixel: Option<[u8; 4]> = match inner {
                (Some(wx), Some(wy)) if wx < side && wy < side => {
                    let index = (wy * side + wx) * 4;
                    window.get(index..index + 4).map(|four| [four[0], four[1], four[2], four[3]])
                }
                _ => None,
            };
            if let Some(pixel) = pixel {
                // `opacity-40`, source-over, which is all an `opacity` on an
                // element is: the picture at 40 percent of its own alpha.
                let texture = TEXTURE_ALPHA * f32::from(pixel[3]) / 255.0;
                // What `mix-blend-luminosity` leaves of the picture, at
                // [`TEXTURE_SHARE`]: its own channels against `--color-green-950`,
                // which is the colour the backdrop carries where it shows. Both
                // sides of that mix are 0..1 -- `shade` came out of `Color` and
                // the picture is a byte -- and mixing a byte in unscaled would
                // saturate every channel at 255, which is a white plate.
                let shade_rgb = [shade.r, shade.g, shade.b];
                for (channel, value) in [pixel[0], pixel[1], pixel[2]].into_iter().enumerate() {
                    let own = f32::from(value) / 255.0;
                    let blended = (1.0 - TEXTURE_SHARE) * shade_rgb[channel] + TEXTURE_SHARE * own;
                    rgb[channel] = rgb[channel] * (1.0 - texture) + blended * texture;
                }
            }
            let index = (y * side + x) * 4;
            for (offset, channel) in rgb.iter().enumerate() {
                out[index + offset] = (channel * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            out[index + 3] = 0xff;
        }
    }
    out
}

/// Whether `(x, y)` is inside the padding box's rounded corner: the plate's own
/// 40 pixels, inset by the 1-pixel border, with a radius of 13.
///
/// 13 is `rounded-[0.875rem]`'s 14 less the border's own width, which is what a
/// border box's radius becomes for the padding box `overflow: hidden` clips to.
fn inside_padding(x: usize, y: usize) -> bool {
    const INSET: f32 = PLATE_INSET as f32;
    const RADIUS: f32 = FEATURE_PLATE_RADIUS - INSET;
    let side = FEATURE_PLATE;
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    if px < INSET || py < INSET || px > side - INSET || py > side - INSET {
        return false;
    }
    // Only the four corner boxes can be outside a rounded rectangle; the straight
    // edges between them are inside it by construction.
    if px >= INSET + RADIUS && px <= side - INSET - RADIUS {
        return true;
    }
    if py >= INSET + RADIUS && py <= side - INSET - RADIUS {
        return true;
    }
    let corner_x = if px < INSET + RADIUS { INSET + RADIUS } else { side - INSET - RADIUS };
    let corner_y = if py < INSET + RADIUS { INSET + RADIUS } else { side - INSET - RADIUS };
    let (dx, dy) = (px - corner_x, py - corner_y);
    dx * dx + dy * dy <= RADIUS * RADIUS
}

/// Where `.feature-icon-shade` is opaque at `(x, y)`, both in the 100-by-100
/// space its layer occupies.
///
/// `linear-gradient(-14deg, A 8%, transparent 86%)`: 0 degrees points at the top
/// and the angle runs clockwise, so -14 points up and to the *left*, the ramp's
/// first stop is the bottom-right corner and the plate darkens towards it. The
/// line's own length is `|100 sin θ| + |100 cos θ|` -- CSS takes the box's whole
/// projection onto the axis, which is what makes the two stops land where they
/// are written rather than somewhere inside them.
fn shade_alpha(x: f32, y: f32) -> f32 {
    let (sin, cos) = SHADE_ANGLE.to_radians().sin_cos();
    let line = 100.0 * sin.abs() + 100.0 * cos.abs();
    let start = (50.0 - line / 2.0 * sin, 50.0 + line / 2.0 * cos);
    let at = ((x - start.0) * sin - (y - start.1) * cos) / line;
    match at {
        t if t <= SHADE_FROM => SHADE_ALPHA,
        t if t >= SHADE_TO => 0.0,
        t => SHADE_ALPHA * (SHADE_TO - t) / (SHADE_TO - SHADE_FROM),
    }
}

/// `.feature-icon-shade`'s own angle.
const SHADE_ANGLE: f32 = -14.0;

/// The 40 by 40 window of `icon-texture.png` a plate shows, decoded once.
///
/// The `<img>` is `h-[6.25rem] w-[9.8125rem]` -- 100 by 157 -- centred on the
/// plate and `object-cover`ed. `cover` is the larger of the two ratios, so the
/// *width* governs: 157/2880 against 100/2788, which scales the 2880 by 2788
/// source to 157 by 152. That is 52 rows taller than the box it is fitted to,
/// and the 52 is the crop `object-fit` centres.
///
/// The plate then shows the 38 by 38 of that which lands inside its padding
/// box, which is the window this returns -- taken at the source's own resolution
/// rather than the drawn one, because a 2880-wide decode a frame is not a thing
/// this page can afford.
fn texture_window() -> &'static [u8] {
    static WINDOW: OnceLock<Vec<u8>> = OnceLock::new();
    WINDOW.get_or_init(|| {
        let decoded = ::image::load_from_memory(TEXTURE_PNG).ok();
        let Some(decoded) = decoded else { return Vec::new() };
        let (source_width, source_height) = (decoded.width(), decoded.height());
        let (box_height, box_width) = (TEXTURE_BOX.0, TEXTURE_BOX.1);
        // `object-fit: cover`: the larger of the two ratios, and the scaled
        // source is then larger than the box by whatever the smaller ratio left.
        let cover = (box_width / source_width as f32).max(box_height / source_height as f32);
        let scaled_width = (source_width as f32 * cover).round().max(1.0) as u32;
        let scaled_height = (source_height as f32 * cover).round().max(1.0) as u32;
        let mut scaled = decoded.resize_exact(
            scaled_width,
            scaled_height,
            ::image::imageops::FilterType::Lanczos3,
        );
        // The crop's own origin: `object-fit` centres, and the scaled source is
        // 52 rows taller than the 100 the `<img>` is, so 26 of them are off the
        // top and 26 off the bottom. Reading this term as zero is what put the
        // window 26 rows above where the reference draws the texture.
        let crop_x = (scaled_width as f32 - box_width).max(0.0) / 2.0;
        let crop_y = (scaled_height as f32 - box_height).max(0.0) / 2.0;
        // The plate's padding box, which is the plate's own 40 less its border,
        // measured from the `<img>`'s centre -- which is the plate's centre.
        let half = FEATURE_PLATE / 2.0;
        let inset = PLATE_INSET as f32;
        let left = (crop_x + box_width / 2.0 - half + inset).round().max(0.0) as u32;
        let top = (crop_y + box_height / 2.0 - half + inset).round().max(0.0) as u32;
        let side = FEATURE_PLATE - 2.0 * inset;
        let side = side.min((scaled_width.saturating_sub(left)) as f32).min((scaled_height.saturating_sub(top)) as f32);
        let side = side.max(0.0) as u32;
        scaled.crop(left, top, side, side).to_rgba8().into_raw()
    })
}

/// A feature's plate: `size-10 rounded-[0.875rem] border bg-surface-1` with three
/// layers inside it and the feature's glyph at `size-5 text-brand` on top.
///
/// The layers, in the source's own order: `.feature-icon-gradient`, then
/// `.feature-icon-shade`, then the `icon-texture.png` `<img>`, then
/// `.feature-icon-glyph`. The first is a [`Background::Gradient`] on the plate
/// itself; the second and third are [`plate_overlay`], one picture; the glyph is
/// a fourth layer of the [`Stack`] that puts it over them.
///
/// **What of `box-shadow` is drawn.** `.feature-icon`'s is three shadows
/// (`ServerListEmpty.vue:178-181`) and this draws the plate's own `border
/// border-solid`, which is a fourth thing rather than one of the three:
///
/// * `0 0 0 1px color-mix(in srgb, var(--color-brand) 30%, var(--surface-1))` --
///   an *outset* ring, `#185233`, measured in the reference at x=122 and x=163
///   and y=258 and y=299 around the first plate. `iced::Shadow` is
///   `color`/`offset`/`blur_radius` and has no `spread_radius`, so a spread ring
///   is not a shadow iced can hold; it would have to be drawn as geometry, a
///   `Stack` layer of its own at `(-1, -1)` carrying it as a 1px border. That is
///   four extra painted rows and columns per plate -- 48 pixels on the page --
///   and it is not drawn here, because the audit measured the reference's ring
///   and ours as "not separately resolvable": this plate's own 1px border and
///   the ring are one pixel apart, and adding the ring would put a second line
///   where there is currently one.
/// * `var(--shadow-card)`, `rgba(0, 0, 0, 0.25) 0px 2px 4px` -- expressible as a
///   [`iced::Shadow`], and cheap: the tiny-skia backend paints it as an SDF over
///   the box grown by the blur, so 48 by 52 evaluations per plate per frame. Not
///   drawn: every shadow this port puts on a plate-sized box is per-frame work on
///   a page that redraws its whole panel every frame, and the plate's contrast
///   against `--surface-1` is already carried by its border and its gradient.
/// * `0 0 3.75rem color-mix(in srgb, var(--color-brand) 10%, transparent)` --
///   also expressible as a [`iced::Shadow`] with a 60px blur, and not drawn: that
///   is a 160 by 160 SDF box per plate, three plates on this page, every frame.
fn plate<'a, Message: 'a>(theme: Gen, glyph: Glyph) -> Element<'a, Message> {
    let layers = Stack::at(
        Vector::ZERO,
        image(plate_overlay(theme))
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Fill),
    )
    .over(
        Vector::ZERO,
        container(icon::icon(glyph, GLYPH, theme_gen::ink(theme, Ink::Brand)))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y(),
    );
    container(layers)
        .width(Length::Fixed(FEATURE_PLATE))
        .height(Length::Fixed(FEATURE_PLATE))
        .clip(true)
        .style(move |_theme: &Theme| container::Appearance {
            // Nothing: the ramp is in the picture above, because the slice of it
            // a plate shows is 39 rows of a 100-row gradient and no box a
            // container owns is wide enough to aim one. The border is still this
            // container's, and the picture is clear on the border's own row and
            // column so it shows through.
            background: None,
            border: Border {
                // `color-mix(in srgb, var(--color-text-primary) 10%, transparent)`
                // is the colour at ten percent *alpha* and not the colour mixed
                // ten percent toward transparent: `color-mix` premultiplies, so
                // mixing a colour with `rgba(0, 0, 0, 0)` divides by the alpha and
                // hands the original colour back at the share's alpha. Mixing
                // toward `TRANSPARENT` instead darkens the channels by the share
                // and leaves the alpha at 0.9, which painted the whole plate a
                // light grey rather than a hairline.
                color: crate::style::at_opacity(theme_gen::ink(theme, Ink::TextPrimary), 0.10),
                width: 1.0,
                radius: FEATURE_PLATE_RADIUS.into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

// ---- The picture's own fixture -------------------------------------------

/// `Avatar size="1.5rem"` on a friend row.
const FRIEND_AVATAR: f32 = 24.0;
/// `size-2` on a friend's presence dot, with a `border border-solid
/// border-surface-1` ring.
const PRESENCE: f32 = 8.0;
/// `h-11` on a friend row.
const FRIEND_ROW: f32 = 44.0;
/// `px-4` on a friend row, and `gap-3` between its avatar, its name and its
/// button.
const FRIEND_PAD: f32 = 16.0;
/// `gap-2`, between a friend row's avatar and its name. `gap-3` on the row is
/// the space between that group and the button, which `justify-between` leaves.
const FRIEND_AVATAR_GAP: f32 = 8.0;
/// `opacity-40` on the rows the reference dims.
const FRIEND_DIM: f32 = 0.40;

/// The eight photographs, byte for byte from
/// `vendor/modrinth-app/ui/src/assets/servers/server-list-empty/`.
const JOSH_PNG: &[u8] = include_bytes!("../../assets/hosting/josh.png");
const PROSPECTOR_PNG: &[u8] = include_bytes!("../../assets/hosting/prospector.png");
const FETCH_PNG: &[u8] = include_bytes!("../../assets/hosting/fetch.png");
const IMB11_PNG: &[u8] = include_bytes!("../../assets/hosting/imb11.png");
const TRUMAN_PNG: &[u8] = include_bytes!("../../assets/hosting/truman.png");
const BORIS_PNG: &[u8] = include_bytes!("../../assets/hosting/boris.png");
const SAYA_PNG: &[u8] = include_bytes!("../../assets/hosting/saya.png");
const MICHAEL_PNG: &[u8] = include_bytes!("../../assets/hosting/michael.png");

/// One row of the preview's friend list.
///
/// The names, the photographs, the statuses, the presences and the one row the
/// pointer is over are the reference's own fixture -- `friends` in
/// `ServerListEmptyPreview.vue` -- not this launcher's, because the preview is
/// the reference's picture of the dialog and a row that said something else would
/// be a different picture.
const FRIENDS: [Friend; 8] = [
    Friend {
        name: "Josh",
        avatar: JOSH_PNG,
        status: FriendStatus::Added,
        presence: None,
        pointer: false,
        dimmed: false,
    },
    Friend {
        name: "Prospector",
        avatar: PROSPECTOR_PNG,
        status: FriendStatus::Invite,
        presence: Some(Presence::Online),
        pointer: true,
        dimmed: false,
    },
    Friend {
        name: "Fetch",
        avatar: FETCH_PNG,
        status: FriendStatus::Cancel,
        presence: Some(Presence::Playing),
        pointer: false,
        dimmed: false,
    },
    Friend { name: "IMB11", avatar: IMB11_PNG, status: FriendStatus::Invite, presence: None, pointer: false, dimmed: false },
    Friend { name: "Truman", avatar: TRUMAN_PNG, status: FriendStatus::Invite, presence: None, pointer: false, dimmed: false },
    Friend { name: "Boris", avatar: BORIS_PNG, status: FriendStatus::Invite, presence: None, pointer: false, dimmed: true },
    Friend { name: "Saya", avatar: SAYA_PNG, status: FriendStatus::Invite, presence: None, pointer: false, dimmed: true },
    Friend { name: "Michael", avatar: MICHAEL_PNG, status: FriendStatus::Invite, presence: None, pointer: false, dimmed: true },
];

/// One row: who it is, what is drawn over it and what its button says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Friend {
    /// `username`.
    name: &'static str,
    /// `avatarUrl`: the row's own photograph, drawn as a disc.
    avatar: &'static [u8],
    /// Which of the three buttons the row carries.
    status: FriendStatus,
    /// `presence`, when the fixture gives the row one.
    presence: Option<Presence>,
    /// `showPointer`: the row the picture's pointer is over, which is also the
    /// one row on `--surface-2`.
    pointer: bool,
    /// `index > 4`, the reference's own rule for which rows it dims.
    dimmed: bool,
}

/// The three states a friend row's button is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FriendStatus {
    /// `added`: disabled, with a check in front of the label.
    Added,
    /// `cancel`: `outlined`, so it reads as taking the invite back.
    Cancel,
    /// `invite`: the frame's own default, which is `base`.
    Invite,
}

impl FriendStatus {
    /// `:type="friend.status === 'cancel' ? 'outlined' : 'base'"`.
    const fn kind(self) -> ui::Kind {
        match self {
            FriendStatus::Cancel => ui::Kind::Outlined,
            FriendStatus::Added | FriendStatus::Invite => ui::Kind::Standard,
        }
    }

    /// `:disabled="friend.status === 'added'"`.
    const fn disabled(self) -> bool {
        matches!(self, FriendStatus::Added)
    }

    /// `friendStatusLabel(friend.status)`.
    const fn label(self) -> Key {
        match self {
            FriendStatus::Added => Key::SharingInvitePlayersModalAdded,
            FriendStatus::Cancel => Key::SharingInvitePlayersModalCancel,
            FriendStatus::Invite => Key::SharingInvitePlayersModalInvite,
        }
    }

    /// `:class="friend.status === 'added' ? '' : 'w-20'"`.
    const fn width(self) -> Length {
        match self {
            FriendStatus::Added => Length::Shrink,
            FriendStatus::Cancel | FriendStatus::Invite => Length::Fixed(FRIEND_BUTTON_WIDTH),
        }
    }
}

/// The two presences the fixture gives a row, and the ink each takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presence {
    /// `presence === 'online' ? 'bg-brand' : 'bg-blue'`.
    Online,
    /// The other half of that rule.
    Playing,
}

impl Presence {
    fn ink(self, theme: Gen) -> iced::Color {
        match self {
            Presence::Online => theme_gen::ink(theme, Ink::Brand),
            Presence::Playing => theme_gen::ink(theme, Ink::Blue),
        }
    }
}

/// `totalFriendCount` in the preview: the heading counts a list it does not draw.
const FRIEND_COUNT: &str = "11";

/// The photographs, decoded once.
///
/// [`avatar::Icon::circle`] is `Avatar.vue`'s `circle` prop: the `contain` fit,
/// letterboxed and scaled, with the alpha outside a *circle* of `side` cleared
/// rather than outside the 16/96 rounded rectangle a project icon takes. A
/// photograph decoded from `view` would be eight 128-pixel PNG decodes a frame.
fn friend_avatar(index: usize) -> Option<&'static avatar::Icon> {
    static CACHE: [OnceLock<Option<avatar::Icon>>; 8] = [
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
    ];
    let friend = FRIENDS.get(index)?;
    CACHE[index]
        .get_or_init(|| avatar::Icon::circle(friend.avatar, FRIEND_AVATAR as u32))
        .as_ref()
}

// ---- The reference's own buttons -----------------------------------------

/// `!h-8` on the preview's friend buttons, which are `size="md"` otherwise --
/// so the height is the `sm` row's 32 and the radius, the padding, the label and
/// the icon are all still `md`'s.
const FRIEND_BUTTON_HEIGHT: f32 = 32.0;
/// `w-20`, on every button that is not the disabled one.
const FRIEND_BUTTON_WIDTH: f32 = 80.0;
/// `gap-1.5`, the gap on `md` between an icon and the label beside it.
const MD_GAP: f32 = 6.0;

/// `text-base` is 16 and `leading-5` is 20 on every row of the frame's table.
const BUTTON_LABEL: f32 = 16.0;
const BUTTON_LINE: f32 = 20.0;

/// `type="colored" color="brand"` with no `size`, which is the frame's own
/// default: `h-9`, 36.
const MD_HEIGHT: f32 = 36.0;
const MD_RADIUS: f32 = 12.0;
const MD_PAD: f32 = 10.0;
/// `[&>svg]:size-5` on `md`, which is the check in front of *Added*.
const MD_ICON: f32 = 20.0;

/// One of the picture's own buttons, drawn from `ButtonFrame.vue` rather than
/// from `crate::ui`'s five kinds.
///
/// `ui.rs` is reserved, and three things here are outside its table:
///
/// | | the reference | [`crate::ui`] would draw |
/// | --- | --- | --- |
/// | height | `!h-8` on a `size="md"` button, so 32 | `Size::Md`'s 36, or `Size::Sm`'s 32 with a 10px radius, a 14px label and a 16px icon |
/// | label | `text-base font-semibold` with `!font-medium` | `Size::Md`'s `semibold` |
/// | width | `w-20` on a button with no icon in it | `Length::Shrink`, which is the label's own width |
///
/// So the numbers are `md`'s, with the height and the weight the class list
/// overrides. `type="base"` -- which is what `Button.vue` defaults to and what
/// every *Invite* is -- is `bg-surface-4 text-contrast` with
/// `inset 0 0 0 1px var(--surface-5)`, and in the Dark theme `--surface-4` and
/// `--color-button-bg` are the same `#34363c`, so `Kind::Standard` fills and rings
/// it identically; it is the height, the weight and the width that differ, and
/// they are the three rows above.
fn frame_button<'a, Message: 'a>(
    theme: Gen,
    label: Key,
    kind: ui::Kind,
    // `width` is `Length::Shrink` for a label with an icon in it and
    // `Length::Fixed(w-20)` for one without; `height` is the one number `!h-8`
    // overrides on `md`; and `icon` is an icon in front of the label at `md`'s
    // own icon size.
    width: Length,
    height: f32,
    icon: Option<Glyph>,
    disabled: bool,
) -> Element<'a, Message> {
    let (fill, ring, ink) = match kind {
        // `button-frame--base`: `bg-surface-4` over `inset 0 0 0 1px
        // var(--surface-5)`, and the label in `--color-text-primary`.
        ui::Kind::Standard => (
            Some(theme_gen::ink(theme, Ink::Surface4)),
            Some(theme_gen::ink(theme, Ink::Surface5)),
            theme_gen::ink(theme, INK_CONTRAST),
        ),
        ui::Kind::Colored => (
            Some(theme_gen::ink(theme, Ink::Brand)),
            None,
            theme_gen::ink(theme, Ink::AccentContrast),
        ),
        // `button-frame--outlined`: `box-shadow: 0 0 0 1px var(--button-color,
        // var(--surface-5))` and nothing behind it.
        _ => (None, Some(theme_gen::ink(theme, Ink::Surface5)), theme_gen::ink(theme, INK_CONTRAST)),
    };
    // `disabled:opacity-50`, applied to each colour the frame paints because this
    // renderer has no group opacity.
    fn dim(color: iced::Color, disabled: bool) -> iced::Color {
        if disabled {
            crate::style::at_opacity(color, 0.5)
        } else {
            color
        }
    }
    let label = text(label.message())
        .size(BUTTON_LABEL)
        .line_height(iced::Pixels(BUTTON_LINE))
        .font(medium())
        .style(iced::theme::Text::Color(dim(ink, disabled)));
    let face: Element<'a, Message> = match icon {
        Some(glyph) => row![]
            .spacing(MD_GAP)
            .align_items(Alignment::Center)
            .push(icon::icon(glyph, MD_ICON, dim(ink, disabled)))
            .push(label)
            .into(),
        None => label.into(),
    };
    container(face)
        .width(width)
        .height(Length::Fixed(height))
        .padding(Padding { top: 0.0, right: MD_PAD, bottom: 0.0, left: MD_PAD })
        .center_x()
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            // A fill-less type is transparent at rest, so an outlined button keeps
            // the surface it sits on until something happens to it -- which for a
            // picture of a dialog is never.
            background: Some(Background::Color(dim(fill.unwrap_or(Color::TRANSPARENT), disabled))),
            border: Border {
                color: ring.map_or(Color::TRANSPARENT, |ring| dim(ring, disabled)),
                width: if ring.is_some() { 1.0 } else { 0.0 },
                radius: MD_RADIUS.into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

// ---- The toast -----------------------------------------------------------

/// `left-[32%]` of the preview's own 400: 128.
const TOAST_LEFT: f32 = 0.32 * PREVIEW;
/// `top-[23rem]`.
const TOAST_TOP: f32 = 23.0 * 16.0;
/// `w-[21rem]`.
const TOAST_WIDTH: f32 = 21.0 * 16.0;
/// `px-4 py-3`.
const TOAST_PAD_X: f32 = 16.0;
const TOAST_PAD_Y: f32 = 12.0;
/// `border border-solid`: the one pixel the reference's own box carries on every
/// side.
///
/// This is the fifth term the toast's height is made of, and it is the one a
/// port drops without noticing, because CSS counts a border in the box it draws
/// (`box-sizing: border-box` is Tailwind's preflight, and an auto-height flex
/// container's height is its content plus its padding plus its border) while
/// iced's `container` paints the border *inside* the box its padding already
/// produced. The reference's own pixels give the whole sum:
/// 440..551 is 112 rows, its content is 453..538 -- 40 for the two `leading-5`
/// lines, `mt-2.5`'s 10, and the buttons' `h-9` 36 -- and 112 - 86 = 26 is
/// `py-3` twice with the border once at each end: 12 + 1 + 13.
/// `TOAST_PAD_Y` on its own is 24, which is the 110 this drew.
const TOAST_BORDER: f32 = 1.0;
/// The content's inset from the toast's border box: `py-3` and `px-4` measured
/// from *inside* the border, which is where CSS measures a padding from.
const TOAST_INSET_X: f32 = TOAST_PAD_X + TOAST_BORDER;
const TOAST_INSET_Y: f32 = TOAST_PAD_Y + TOAST_BORDER;
/// `gap-4`, between the avatar and the text beside it.
const TOAST_GAP: f32 = 16.0;
/// `mt-2.5`, between the toast's two lines and its buttons.
const TOAST_BUTTON_GAP: f32 = 10.0;
/// `rounded-2xl`.
const TOAST_RADIUS: f32 = 16.0;
/// `Avatar size="2.25rem" circle`.
const TOAST_AVATAR: f32 = 36.0;
/// The presence dot on it: `size-3 rounded-full border-2 border-solid
/// border-surface-2 bg-brand`, so 12 pixels with a 2-pixel ring.
const TOAST_PRESENCE: f32 = 12.0;
/// `Avatar size="1.25rem"` on the server's own mark.
const SERVER_MARK: f32 = 20.0;

/// The two photographs the toast carries.
const GEOMETRICALLY_PNG: &[u8] = include_bytes!("../../assets/hosting/geometrically.png");
const MODRINTH_SMP_PNG: &[u8] = include_bytes!("../../assets/hosting/modrinth-smp.png");

/// Geometrically's photograph, decoded once at [`TOAST_AVATAR`] round.
fn geometrically() -> Option<&'static avatar::Icon> {
    static AVATAR: OnceLock<Option<avatar::Icon>> = OnceLock::new();
    AVATAR
        .get_or_init(|| avatar::Icon::circle(GEOMETRICALLY_PNG, TOAST_AVATAR as u32))
        .as_ref()
}

/// The Modrinth SMP mark, decoded once at [`SERVER_MARK`] on the rounded square
/// an `Avatar` without `circle` takes.
fn modrinth_smp() -> Option<&'static avatar::Icon> {
    static MARK: OnceLock<Option<avatar::Icon>> = OnceLock::new();
    MARK.get_or_init(|| avatar::Icon::of(MODRINTH_SMP_PNG, SERVER_MARK as u32)).as_ref()
}

/// The picture's pointer badge and the hand in it.
const POINTER_PNG: &[u8] = include_bytes!("../../assets/hosting/Pointer.png");
/// `size-8` on the badge, `size-4` on the hand in it.
const POINTER_BADGE: f32 = 32.0;
const POINTER_MARK: f32 = 16.0;
/// `right-[14.25rem]` from the row's own right edge.
const POINTER_RIGHT: f32 = 14.25 * 16.0;
/// `top-9 -translate-y-1/2`: the badge's middle is 36 pixels down its row.
const POINTER_MIDDLE: f32 = 36.0;

fn pointer_badge<'a, Message: 'a>() -> Element<'a, Message> {
    static MARK: OnceLock<Option<avatar::Icon>> = OnceLock::new();
    let mark = MARK
        .get_or_init(|| avatar::Icon::of(POINTER_PNG, POINTER_MARK as u32))
        .as_ref();
    let hand: Element<'a, Message> = match mark {
        Some(icon) => image(icon.handle())
            .width(Length::Fixed(POINTER_MARK))
            .height(Length::Fixed(POINTER_MARK))
            .content_fit(ContentFit::Fill)
            .into(),
        None => Space::new(POINTER_MARK, POINTER_MARK).into(),
    };
    container(hand)
        .width(Length::Fixed(POINTER_BADGE))
        .height(Length::Fixed(POINTER_BADGE))
        .center_x()
        .center_y()
        // `bg-white/10 opacity-75`, which over `--surface-1` is the fill the
        // reference's own badge reads at: measured `(33, 35, 40)` on
        // `/tmp/ref/hosting-clean3.png` at the badge's centre (x=676, y=322).
        .style(|_theme: &Theme| container::Appearance {
            background: Some(Background::Color(Color {
                a: 0.75 * 0.10,
                ..Color::WHITE
            })),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: (POINTER_BADGE / 2.0).into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

// ---- The page ------------------------------------------------------------

/// Draw the page.
pub fn view<'a>(theme: Gen, state: &'a State, _store: &'a Store) -> Element<'a, Message> {
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = &state.notice {
        blocks.push(ui::admonition(
            theme,
            ui::Severity::Info,
            Key::AppNavModrinthHosting.message(),
            notice,
        ));
    }
    blocks.push(empty_state(theme));
    page::body(blocks, GAP, Message::Wheel)
}

/// `ServerListEmpty` with `logged-in="false"`: the column, the preview, and the
/// sign-in at the foot.
///
/// Three blocks and one gap, in the reference's order: the row that grows
/// (`grow`, so its two children are centred in what is left), then the foot,
/// with `justify-between` between them and `gap-8` for the space.
fn empty_state<'a>(theme: Gen) -> Element<'a, Message> {
    column![]
        .width(Length::Fill)
        .spacing(FOOT_GAP)
        .push(
            row![]
                .width(Length::Fill)
                .spacing(0.0)
                .align_items(Alignment::Center)
                .push(Space::with_width(Length::FillPortion(MARGIN_SHARE[0])))
                .push(column_text(theme))
                .push(Space::with_width(Length::FillPortion(MARGIN_SHARE[1])))
                // `-mb-10` is a row forty shorter than the panel it holds, with
                // the panel laid at the row's own top.
                .push(
                    container(preview(theme))
                        .width(Length::Fixed(PREVIEW))
                        .height(Length::Fixed(PREVIEW_ROW))
                        .align_y(iced::alignment::Vertical::Top),
                )
                .push(Space::with_width(Length::FillPortion(MARGIN_SHARE[2]))),
        )
        .push(
            column![]
                .width(Length::Fill)
                .spacing(ACTION_GAP)
                .align_items(Alignment::Center)
                .push(
                    text(Key::ServersListEmptyAlreadyHaveServerLabel.message())
                        .size(14.0)
                        .line_height(iced::Pixels(20.0))
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_SECONDARY,
                        ))),
                )
                .push(ui::button_with_icon_sized(
                    theme,
                    SIGN_IN_KEY,
                    Glyph::LogIn,
                    Key::ServersListEmptySignInButton,
                    // `<Button>` with no `type`, which `Button.vue` defaults to
                    // `base`.
                    ui::Kind::Standard,
                    ui::Size::Md,
                    Length::Shrink,
                    Some(Message::Refresh),
                )),
        )
        .into()
}

/// `mx-auto w-full max-w-[20rem] flex flex-col items-start gap-8`: the heading,
/// the three features, and the buttons.
fn column_text<'a>(theme: Gen) -> Element<'a, Message> {
    let mut column = column![]
        .width(Length::Fixed(COLUMN))
        .spacing(COLUMN_GAP)
        .align_items(Alignment::Start);
    column = column.push(
        column![]
            .spacing(PLATE_GAP)
            .push(
                text(Key::ServersListEmptyModrinthHostingLabel.message())
                    .size(30.0)
                    .line_height(iced::Pixels(36.0))
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(
                        theme,
                        INK_CONTRAST,
                    ))),
            )
            .push(
                text(Key::ServersListEmptyNoServersDescription.message())
                    .size(16.0)
                    // `text-base` is 16 on Tailwind's 24-pixel line, and iced's
                    // own default for a 16-pixel face measures 21 on the capture
                    // -- three pixels short per line, and six over the two lines
                    // this description takes, which is what puts the whole column
                    // three pixels low and the heading four.
                    .line_height(iced::Pixels(24.0))
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(
                        theme,
                        INK_DEFAULT,
                    ))),
            ),
    );
    let mut features = column![].spacing(FEATURE_GAP);
    for (glyph, title, description) in FEATURES {
        features = features.push(feature(theme, glyph, title, description));
    }
    column = column.push(features);
    column.push(
        row![]
            .spacing(ACTION_GAP)
            .align_items(Alignment::Center)
            // `size="lg"`: 40 pixels, which is the row `ButtonFrame.vue` calls
            // `lg` and the height measured at y=537..576.
            .push(ui::button_with_icon_sized(
                theme,
                NEW_SERVER_KEY,
                Glyph::Plus,
                Key::ServersListEmptyNewServerButton,
                ui::Kind::Colored,
                ui::Size::Lg,
                Length::Shrink,
                Some(Message::NewServer),
            ))
            // `AutoLink` with a `size-5` arrow and `gap-1`, in the label's own
            // order: the words, then the arrow. A link this launcher cannot follow
            // is drawn as the quiet button it behaves like, and the press says
            // which service it would have asked.
            .push(link_button(theme, MANAGE_BILLING_KEY, Message::ManageBilling)),
    )
    .into()
}

/// One feature: its plate and its two lines, `items-start gap-4`.
fn feature<'a>(theme: Gen, glyph: Glyph, title: Key, description: Key) -> Element<'a, Message> {
    row![]
        .spacing(FEATURE_GAP_X)
        .align_items(Alignment::Start)
        .push(plate(theme, glyph))
        .push(
            column![]
                .spacing(FEATURE_TEXT_GAP)
                .push(
                    text(title.message())
                        .size(18.0)
                        .line_height(iced::Pixels(24.0))
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_CONTRAST,
                        ))),
                )
                .push(
                    text(description.message())
                        .size(14.0)
                        .line_height(iced::Pixels(20.0))
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_DEFAULT,
                        ))),
                ),
        )
        .into()
}

/// The *Learn more* link: `AutoLink`, whose arrow follows its label.
///
/// `AutoLink` is `flex items-center gap-1 hover:brightness-125 font-semibold`
/// around `{{ label }}` and then `<RightArrowIcon class="size-5 shrink-0" />`, so
/// the words come first. `crate::ui`'s icon buttons put the icon first -- there is
/// no row of that table for a trailing one -- and `ui.rs` is reserved, so the
/// order is drawn here. Measured against the reference the label and the arrow
/// together are 110 wide and the label's own ink starts on the first of them,
/// which is what the reference's own order gives and the other does not.
fn link_button<'a, Message: Clone + crate::ui::Hovered + 'a>(
    theme: Gen,
    key: &'static str,
    on_press: Message,
) -> Element<'a, Message> {
    let ink = theme_gen::ink(theme, Ink::Base);
    let link = container(
        row![]
            .spacing(4.0)
            .align_items(Alignment::Center)
            .push(
                text(Key::ServersListEmptyLearnMoreLink.message())
                    .size(BUTTON_LABEL)
                    .line_height(iced::Pixels(BUTTON_LINE))
                    .font(semibold())
                    .style(iced::theme::Text::Color(ink)),
            )
            .push(icon::icon(Glyph::RightArrow, GLYPH, ink)),
    )
    .width(Length::Shrink);
    iced::widget::mouse_area(link)
        .interaction(iced::mouse::Interaction::Pointer)
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false))
        .on_press(on_press)
        .into()
}

/// `ServerListEmptyPreview`: the reference's own picture of the invite dialog,
/// drawn at its own measurements and with its own strings.
///
/// It is `inert aria-hidden` in the reference -- a picture of a dialog, not a
/// dialog -- so nothing here takes a press, and every control below is drawn the
/// way the picture draws it rather than the way it behaves.
///
/// The three layers are the reference's own paint order: the panel, then the fade
/// over its bottom 448 pixels, then the toast over both.
fn preview<'a>(theme: Gen) -> Element<'a, Message> {
    let mut list = column![].width(Length::Fill).push(
        container(
            text(text_gen::sharing_invite_players_modal_friends_heading(FRIEND_COUNT))
                .size(14.0)
                .line_height(iced::Pixels(20.0))
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        )
        .width(Length::Fill)
        .padding(Padding { top: 0.0, right: 0.0, bottom: 8.0, left: FRIEND_PAD }),
    );
    for (index, friend) in FRIENDS.iter().enumerate() {
        list = list.push(friend_row(theme, index, friend));
    }
    let friends = container(
        list.padding(Padding { top: 12.0, right: 0.0, bottom: 12.0, left: 0.0 }),
    )
    .width(Length::Fill)
    .style(move |_theme: &Theme| container::Appearance {
        // `bg-surface-1`: the picture's list sits on the sunken surface, a step
        // darker than the panel it is in.
        background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface1))),
        ..container::Appearance::default()
    });
    // `absolute inset-x-0 top-0 h-full` for the panel: its contents fill it, and
    // the two things the reference takes *out* of the flow -- the invite link at
    // `bottom-0` and the toast -- are layers over that rather than children of
    // it. A `Fill` space in a column would not do: iced's flex hands a `Fill`
    // child the room left by the children *before* it and lays the rest past the
    // end, which put the foot below the panel's own bottom edge.
    let content = container(
        column![]
            .width(Length::Fill)
            .spacing(0.0)
            .push(preview_head(theme))
            .push(panel_rule(theme))
            .push(preview_search(theme))
            .push(panel_rule(theme))
            .push(friends),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_y(iced::alignment::Vertical::Top)
    // The panel's 1-pixel border on every side, so that what is in the flow is
    // laid in the *padding* box the reference lays it in: its two rules are 398
    // wide rather than 400, a friend row's `px-4` starts at x=538 rather than
    // 537, the title row's X sits 2 pixels further left, and the title row's rule
    // is 61 rows down the panel rather than 60 -- the reference's own capture has
    // it at y=133 on a panel whose top border is at y=72, and `p-4` with a
    // 28-pixel line and the row's own `border-b` is 61 of those.
    .padding(Padding { top: 1.0, right: 1.0, bottom: 1.0, left: 1.0 });
    let layers = Stack::at(Vector::ZERO, content)
        .over(
            Vector::new(0.0, PREVIEW_HEIGHT - INVITE_LINK_HEIGHT),
            container(preview_invite_link(theme))
                .width(Length::Fill)
                .padding(Padding { top: 0.0, right: 1.0, bottom: 0.0, left: 1.0 }),
        )
        .over(
            Vector::new(0.0, PREVIEW_HEIGHT - PREVIEW_FADE),
            preview_fade(theme),
        )
        .over(Vector::new(TOAST_LEFT, TOAST_TOP), toast(theme));
    container(layers)
        .width(Length::Fixed(PREVIEW))
        .height(Length::Fixed(PREVIEW_HEIGHT))
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface2))),
            border: Border {
                color: theme_gen::ink(theme, Ink::Surface3),
                width: 1.0,
                radius: theme_gen::span(Span::RadiusLg).into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

/// `border-0 border-b border-solid border-surface-3` under the title row and
/// under the search row.
///
/// iced's `Border` is one width for all four sides, so the one-pixel rule these
/// two rows carry is a [`iced::widget::rule`] of its own rather than a border.
fn panel_rule<'a, Message: 'a>(theme: Gen) -> Element<'a, Message> {
    iced::widget::Rule::horizontal(1.0)
        .style(move |_theme: &Theme| iced::widget::rule::Appearance {
            color: theme_gen::ink(theme, Ink::Surface3),
            // The line's thickness is `Appearance::width`; `Rule::horizontal`'s
            // own height is only the slot it is given.
            width: 1,
            radius: 0.0.into(),
            fill_mode: iced::widget::rule::FillMode::Full,
        })
        .into()
}

/// `h-[28rem]` on the fade, and where it starts.
const PREVIEW_FADE: f32 = 28.0 * 16.0;

/// `pointer-events-none absolute inset-x-0 bottom-0 h-[28rem]
/// [background:linear-gradient(to_bottom,transparent,var(--surface-1))]`.
///
/// The ramp is `to bottom`, which is the direction iced gives `Radians(PI)`, and
/// it is transparent at the top of the box rather than a colour mixed into
/// anything: what it does is composite the panel and its contents toward
/// `--surface-1`, and the top of the box has to leave them alone. Measured against
/// the reference, the ramp is `(47, 49, 55)` where the *Invite* button's
/// `--surface-4` `(52, 54, 60)` stands at y=308, which is `(52, 54, 60)` moved 17
/// percent of the way to `--surface-1` -- and 308 is 76 of the 448 pixels above the
/// box's bottom.
fn preview_fade<'a, Message: 'a>(theme: Gen) -> Element<'a, Message> {
    // One colour and only its alpha, and the ramp runs *up* rather than down, so
    // that it is `--surface-1` at the bottom of the box and nothing at the top.
    // The alternative -- `transparent` at 0 and `--surface-1` at 1 -- is the same
    // picture in CSS and a different one here: CSS interpolates gradients in
    // premultiplied space and iced interpolates in straight space, so a ramp
    // between `(0, 0, 0, 0)` and `(22, 24, 28, 1)` passes through half-opaque
    // *black* and darkens everything it covers instead of washing it out. With
    // both stops the same colour the two spaces agree, which is measured: the
    // reference reads `(24, 25, 29)` over the panel at y=582 and the ramp that
    // runs through black reads `(17, 20, 23)`.
    let surface_1 = theme_gen::ink(theme, Ink::Surface1);
    let mut ramp = gradient::Linear::new(Radians(0.0));
    ramp = ramp.add_stop(0.0, surface_1);
    ramp = ramp.add_stop(1.0, Color { a: 0.0, ..surface_1 });
    container(Space::with_width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fixed(PREVIEW_FADE))
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Gradient(Gradient::Linear(ramp))),
            ..container::Appearance::default()
        })
        .into()
}

/// `absolute left-[32%] top-[23rem] z-10 flex w-[21rem] max-w-[calc(100%-1rem)]
/// gap-4 !overflow-hidden rounded-2xl border border-solid border-surface-4
/// bg-surface-2 px-4 py-3 shadow-card`.
fn toast<'a, Message: 'a>(theme: Gen) -> Element<'a, Message> {
    let mut lines = column![].width(Length::Fill).spacing(0.0);
    lines = lines.push(
        row![]
            .width(Length::Fill)
            .spacing(4.0)
            .align_items(Alignment::Start)
            // `<p class="m-0">`: the name in `--color-text-primary` at
            // `font-medium`, then the reference's own sentence.
            .push(
                text(TOAST_INVITED_BY.to_string())
                    .size(BUTTON_LABEL)
                    .line_height(iced::Pixels(BUTTON_LINE))
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(
                text(Key::SharingInvitePlayersToastInvitedYouTo.message())
                    .size(BUTTON_LABEL)
                    .line_height(iced::Pixels(BUTTON_LINE))
                    .font(regular())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
            )
            .push(Space::with_width(Length::Fill))
            .push(icon::icon(Glyph::X, GLYPH, theme_gen::ink(theme, INK_SECONDARY))),
    );
    lines = lines.push(
        row![]
            .width(Length::Fill)
            .spacing(4.0)
            .align_items(Alignment::Center)
            .push(picture(modrinth_smp(), SERVER_MARK))
            .push(
                text(TOAST_SERVER_NAME.to_string())
                    .size(BUTTON_LABEL)
                    .line_height(iced::Pixels(BUTTON_LINE))
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(
                text(Key::SharingInvitePlayersToastServerSuffix.message())
                    .size(BUTTON_LABEL)
                    .line_height(iced::Pixels(BUTTON_LINE))
                    .font(regular())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
            ),
    );
    let avatar_layers = Stack::at(
        Vector::ZERO,
        picture(geometrically(), TOAST_AVATAR),
    )
    .over(
        // `absolute bottom-0 right-[-1px]`: the dot's bottom on the avatar's
        // bottom and its right one pixel past the avatar's right.
        Vector::new(TOAST_AVATAR - TOAST_PRESENCE + 1.0, TOAST_AVATAR - TOAST_PRESENCE),
        container(Space::with_width(Length::Fixed(TOAST_PRESENCE)))
            .width(Length::Fixed(TOAST_PRESENCE))
            .height(Length::Fixed(TOAST_PRESENCE))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Brand))),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface2),
                    width: 2.0,
                    radius: (TOAST_PRESENCE / 2.0).into(),
                },
                ..container::Appearance::default()
            }),
    );
    container(
        row![]
            // The box's own width less its `px-4` and its border, stated rather
            // than `Fill`: iced resolves a `container`'s `width` against the
            // limits its padding leaves, so a `Fill` row is handed 302 only if
            // it happens to ask for no more, and the row is what puts the X 17
            // pixels from the edge.
            .width(Length::Fixed(TOAST_WIDTH - 2.0 * TOAST_INSET_X))
            .spacing(TOAST_GAP)
            .align_items(Alignment::Start)
            .push(
                container(avatar_layers)
                    .width(Length::Fixed(TOAST_AVATAR))
                    .height(Length::Fixed(TOAST_AVATAR)),
            )
            .push(
                column![]
                    .width(Length::Fill)
                    .spacing(TOAST_BUTTON_GAP)
                    .push(lines)
                    // `mt-2.5 flex gap-2`.
                    .push(
                        row![]
                            .spacing(8.0)
                            .push(frame_button(
                                theme,
                                Key::SharingInvitePlayersToastAccept,
                                ui::Kind::Colored,
                                Length::Shrink,
                                MD_HEIGHT,
                                None,
                                false,
                            ))
                            .push(frame_button(
                                theme,
                                Key::SharingInvitePlayersToastDecline,
                                ui::Kind::Outlined,
                                Length::Shrink,
                                MD_HEIGHT,
                                None,
                                false,
                            )),
                    ),
            ),
    )
    .width(Length::Fixed(TOAST_WIDTH))
        .padding(Padding {
            top: TOAST_INSET_Y,
            right: TOAST_INSET_X,
            bottom: TOAST_INSET_Y,
            left: TOAST_INSET_X,
        })
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface2))),
            border: Border {
                color: theme_gen::ink(theme, Ink::Surface4),
                width: 1.0,
                radius: TOAST_RADIUS.into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

/// The two names the toast spells out rather than asking for.
///
/// `sharing.invite-players-toast` has no message for either of them --
/// `<span class="font-medium text-contrast">Geometrically</span>` and
/// `<span class="font-medium text-contrast">Modrinth SMP</span>` are literals in
/// the reference -- so they are held here, next to the keys they sit beside,
/// rather than spelled at the call site.
const TOAST_INVITED_BY: &str = "Geometrically";
const TOAST_SERVER_NAME: &str = "Modrinth SMP";

/// One decoded picture at its own size, or the box it would have filled.
fn picture<'a, Message: 'a>(icon: Option<&'static avatar::Icon>, side: f32) -> Element<'a, Message> {
    let box_size = Length::Fixed(side);
    match icon {
        Some(icon) => image(icon.handle())
            .width(box_size)
            .height(box_size)
            .content_fit(ContentFit::Fill)
            .into(),
        None => Space::new(side, side).into(),
    }
}

/// The picture's title row: `p-4` over a `border-surface-3` rule.
fn preview_head<'a>(theme: Gen) -> Element<'a, Message> {
    row![]
        .width(Length::Fill)
        .spacing(PLATE_GAP)
        .align_items(Alignment::Center)
        .padding(16.0)
        .push(icon::icon(
            Glyph::UserPlus,
            GLYPH,
            theme_gen::ink(theme, INK_DEFAULT),
        ))
        .push(
            text(Key::SharingInvitePlayersModalHeading.message())
                .size(18.0)
                // `text-lg` is 18 on Tailwind's 28-pixel line.
                .line_height(iced::Pixels(28.0))
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(
                    theme,
                    INK_CONTRAST,
                ))),
        )
        .push(Space::with_width(Length::Fill))
        .push(icon::icon(
            Glyph::X,
            GLYPH,
            theme_gen::ink(theme, INK_SECONDARY),
        ))
        .into()
}

/// The picture's search row: the field, then *Add*.
fn preview_search<'a>(theme: Gen) -> Element<'a, Message> {
    row![]
        .width(Length::Fill)
        .spacing(PLATE_GAP)
        .align_items(Alignment::Center)
        .padding(Padding { top: 16.0, right: 16.0, bottom: 16.0, left: 16.0 })
        .push(
            container(
                row![]
                    .spacing(PLATE_GAP)
                    .align_items(Alignment::Center)
                    .push(icon::icon(
                        Glyph::Search,
                        16.0,
                        theme_gen::ink(theme, INK_SECONDARY),
                    ))
                    .push(
                        text(Key::SharingInvitePlayersModalSearchPlaceholder.message())
                            .size(14.0)
                            .line_height(iced::Pixels(20.0))
                            .font(medium())
                            .style(iced::theme::Text::Color(theme_gen::ink(
                                theme,
                                INK_SECONDARY,
                            ))),
                    ),
            )
            .width(Length::Fill)
            .height(Length::Fixed(MD_HEIGHT))
            .padding(Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 12.0 })
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface3))),
                border: Border { radius: MD_RADIUS.into(), ..Border::default() },
                ..container::Appearance::default()
            }),
        )
        // `<Button type="colored" color="brand" class="!cursor-default"
        // disabled>`: `disabled:opacity-50`, and the frame's own default size.
        .push(ui::button_with_icon_sized(
            theme,
            REFRESH_KEY,
            Glyph::Plus,
            Key::SharingInvitePlayersModalAdd,
            ui::Kind::Colored,
            ui::Size::Md,
            Length::Shrink,
            None,
        ))
        .into()
}

/// One friend: `relative flex h-11 items-center justify-between gap-3 px-4`.
///
/// Three things about this row are the fixture's rather than the layout's, and
/// all three are visible: `showPointer` puts the row on `--surface-2` and hangs
/// the pointer badge off its right, `index > 4` dims it, and a `presence` puts a
/// dot on the bottom-right of its avatar.
fn friend_row<'a, Message: 'a>(
    theme: Gen,
    index: usize,
    friend: &'static Friend,
) -> Element<'a, Message> {
    let dim = |color: iced::Color| {
        if friend.dimmed {
            crate::style::at_opacity(color, FRIEND_DIM)
        } else {
            color
        }
    };
    let mut avatar_layers = Stack::at(Vector::ZERO, picture(friend_avatar(index), FRIEND_AVATAR));
    if let Some(presence) = friend.presence {
        // `absolute bottom-0 right-[-1px] size-2 rounded-full border border-solid
        // border-surface-1`.
        avatar_layers = avatar_layers.over(
            Vector::new(FRIEND_AVATAR - PRESENCE + 1.0, FRIEND_AVATAR - PRESENCE),
            container(Space::with_width(Length::Fixed(PRESENCE)))
                .width(Length::Fixed(PRESENCE))
                .height(Length::Fixed(PRESENCE))
                .style(move |_theme: &Theme| container::Appearance {
                    background: Some(Background::Color(dim(presence.ink(theme)))),
                    border: Border {
                        color: dim(theme_gen::ink(theme, Ink::Surface1)),
                        width: 1.0,
                        radius: (PRESENCE / 2.0).into(),
                    },
                    ..container::Appearance::default()
                }),
        );
    }
    let row = row![]
        .width(Length::Fill)
        .height(Length::Fixed(FRIEND_ROW))
        .padding(Padding { top: 0.0, right: FRIEND_PAD, bottom: 0.0, left: FRIEND_PAD })
        .align_items(Alignment::Center)
        // `flex min-w-0 items-center gap-2`: the avatar and the name are one
        // group, and `justify-between` puts that group against the left of the
        // row and the button against its right.
        .push(
            row![]
                .spacing(FRIEND_AVATAR_GAP)
                .align_items(Alignment::Center)
                .push(
                    container(avatar_layers)
                        .width(Length::Fixed(FRIEND_AVATAR))
                        .height(Length::Fixed(FRIEND_AVATAR)),
                )
                .push(
                    text(friend.name.to_string())
                        .size(16.0)
                        .font(medium())
                        .style(iced::theme::Text::Color(dim(theme_gen::ink(
                            theme,
                            INK_DEFAULT,
                        )))),
                ),
        )
        // `justify-between`, with the name against the left and the button against
        // the right.
        .push(Space::with_width(Length::Fill))
        .push(frame_button(
            theme,
            friend.status.label(),
            friend.status.kind(),
            friend.status.width(),
            FRIEND_BUTTON_HEIGHT,
            (friend.status == FriendStatus::Added).then_some(Glyph::Check),
            friend.status.disabled(),
        ));
    // The badge is a sibling of the row's own contents and `absolute` inside it,
    // so it is a layer of the row rather than one of its children.
    let mut layers = Stack::at(Vector::ZERO, row);
    if friend.pointer {
        layers = layers.over(
            Vector::new(
                FRIEND_ROW_WIDTH - POINTER_RIGHT - POINTER_BADGE,
                POINTER_MIDDLE - POINTER_BADGE / 2.0,
            ),
            pointer_badge(),
        );
    }
    container(layers)
        .width(Length::Fill)
        .height(Length::Fixed(FRIEND_ROW))
        .style(move |_theme: &Theme| container::Appearance {
            // `friend.showPointer ? 'bg-surface-2' : ''`, and `index > 4 ?
            // 'opacity-40' : ''` on the row's own contents, which `dim` has
            // already applied.
            background: if friend.pointer {
                Some(Background::Color(dim(theme_gen::ink(theme, Ink::Surface2))))
            } else {
                None
            },
            ..container::Appearance::default()
        })
        .into()
}

/// The width of a friend row: the panel's padding box, which is `max-w-[25rem]`
/// less its 1-pixel border either side. `right-[14.25rem]` is measured from it.
const FRIEND_ROW_WIDTH: f32 = PREVIEW - 2.0;

/// The invite-link foot's own height: its 1-pixel `border-t`, `p-4` above and
/// below, a `text-base` line with `pb-2`, and the `h-8` link row.
const INVITE_LINK_HEIGHT: f32 = 1.0 + 16.0 + 24.0 + 8.0 + 32.0 + 16.0;

/// The picture's foot: the invite link, under a rule, over `--surface-2`.
fn preview_invite_link<'a>(theme: Gen) -> Element<'a, Message> {
    column![]
        .width(Length::Fill)
        .spacing(0.0)
        // `border-t border-solid border-surface-3`
        .push(panel_rule(theme))
        .push(
            column![]
                .width(Length::Fill)
                .spacing(PLATE_GAP)
                .padding(16.0)
                .push(
                    text(Key::SharingInvitePlayersModalInviteLinkHeading.message())
                        .size(16.0)
                        .line_height(iced::Pixels(24.0))
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_CONTRAST,
                        ))),
                )
                .push(
                    row![]
                        .width(Length::Fill)
                        .spacing(PLATE_GAP)
                        .align_items(Alignment::Center)
                        .height(Length::Fixed(32.0))
                        .padding(Padding {
                            top: 0.0,
                            right: 10.0,
                            bottom: 0.0,
                            left: 10.0,
                        })
                        .push(
                            text("https://modrinth.com/server/abc123")
                                .size(14.0)
                                .font(medium())
                                .style(iced::theme::Text::Color(theme_gen::ink(
                                    theme,
                                    INK_DEFAULT,
                                ))),
                        )
                        .push(Space::with_width(Length::Fill))
                        .push(icon::icon(
                            Glyph::ClipboardCopy,
                            16.0,
                            theme_gen::ink(theme, INK_SECONDARY),
                        )),
                ),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_says_the_service_it_needs_rather_than_that_it_is_unbuilt() {
        // The distinction this page exists to keep now: a Modrinth Hosting action
        // is not waiting for a slice, it is waiting for an account this launcher
        // does not hold. A notice that read "is not implemented yet" would be a
        // promise nothing is keeping.
        let mut state = State::default();
        for message in [Message::NewServer, Message::ManageBilling, Message::Refresh] {
            state.update(message);
            let notice = state.notice.clone().expect("a notice");
            assert!(notice.contains("Modrinth account"), "{notice}");
            assert!(!notice.contains("is not implemented yet"), "{notice}");
        }
    }

    #[test]
    fn the_page_draws_the_empty_state_in_every_theme_with_and_without_a_notice() {
        let store = Store::default();
        for theme in Gen::ALL {
            for notice in [None, Some("x".to_string())] {
                let state = State { notice };
                drop(view(*theme, &state, &store));
            }
        }
    }

    #[test]
    fn the_columns_are_the_reference_s_own_measures() {
        // `max-w-[20rem]` and `max-w-[25rem]`, and the free space their row's four
        // `mx-auto` margins split at the reference's own 868-pixel content column:
        // 868 - 320 - 400 - 8 = 140, so 35 a margin, which is `MARGIN_SHARE`'s
        // first and last thirds and its 43 for the middle margin plus the gap.
        assert_eq!(COLUMN, 20.0 * 16.0);
        assert_eq!(PREVIEW, 25.0 * 16.0);
        assert_eq!(PREVIEW_HEIGHT, 38.0 * 16.0);
        assert_eq!(ROW_GAP, 2.0 * 4.0);
        assert_eq!(COLUMN_GAP, 8.0 * 4.0);
        assert_eq!(FEATURE_GAP, 6.0 * 4.0);
        assert_eq!(FEATURE_GAP_X, 4.0 * 4.0);
        assert_eq!(FEATURE_TEXT_GAP, 0.5 * 4.0);
        assert_eq!(ACTION_GAP, 4.0 * 4.0);
        assert_eq!(FEATURE_PLATE, 10.0 * 4.0);
        let free = 868.0 - COLUMN - PREVIEW - ROW_GAP;
        assert_eq!(free, 140.0);
        assert_eq!(free / 4.0, 35.0);
        assert_eq!(MARGIN_SHARE[0], 35);
        assert_eq!(MARGIN_SHARE[2], 35);
        assert_eq!(MARGIN_SHARE[1] as f32, 35.0 + ROW_GAP + 35.0);
        // And the three of them are exactly the free space, which is what puts the
        // column's first ink at 123 and the preview's at 521 rather than near them.
        let spacers: f32 = MARGIN_SHARE.iter().map(|share| *share as f32).sum();
        assert_eq!(spacers, 148.0);
        assert_eq!(spacers, free + ROW_GAP);
    }

    #[test]
    fn the_features_are_the_reference_s_three_in_its_own_order() {
        assert_eq!(FEATURES.len(), 3);
        assert_eq!(FEATURES[0].1, Key::ServersListEmptyOneClickModInstallsTitle);
        assert_eq!(FEATURES[1].1, Key::ServersListEmptySimpleSetupTitle);
        assert_eq!(FEATURES[2].1, Key::ServersListEmptyPlayWithFriendsTitle);
        // The descriptions are the reference's own sentences, so the columns wrap
        // where the reference's wrap. Each is one key and never a literal.
        for (_, _, description) in FEATURES {
            assert!(description.message().ends_with('.'), "{:?}", description);
        }
    }

    #[test]
    fn the_preview_lists_the_reference_s_own_friends_and_statuses() {
        // The picture is the reference's: its names, its three button states, and
        // the count its heading quotes -- which is 11 while eight rows are drawn,
        // exactly as the reference draws it.
        assert_eq!(FRIENDS.len(), 8);
        assert_eq!(FRIEND_COUNT, "11");
        assert_eq!(FRIENDS[0].name, "Josh");
        assert_eq!(FRIENDS[0].status, FriendStatus::Added);
        assert_eq!(FRIENDS[2].status, FriendStatus::Cancel);
        assert_eq!(
            FRIENDS.iter().filter(|friend| friend.status == FriendStatus::Invite).count(),
            6
        );
        assert_eq!(
            FRIENDS.iter().filter(|friend| friend.status == FriendStatus::Added).count(),
            1
        );
        assert_eq!(
            FRIENDS.iter().filter(|friend| friend.status == FriendStatus::Cancel).count(),
            1
        );
        // `index > 4` is the reference's own rule and it dims three rows, not four:
        // Truman is the fifth, and `>` is not `>=`.
        assert_eq!(FRIENDS.iter().filter(|friend| friend.dimmed).count(), 3);
        assert!(!FRIENDS[4].dimmed, "index 4 is not `> 4`");
        assert!(FRIENDS[5].dimmed && FRIENDS[6].dimmed && FRIENDS[7].dimmed);
        // `showPointer` and the two presences, from the fixture.
        assert_eq!(FRIENDS.iter().filter(|friend| friend.pointer).count(), 1);
        assert!(FRIENDS[1].pointer);
        assert_eq!(FRIENDS[1].presence, Some(Presence::Online));
        assert_eq!(FRIENDS[2].presence, Some(Presence::Playing));
        assert_eq!(FRIENDS[0].presence, None);
    }

    #[test]
    fn the_friend_buttons_are_the_reference_s_own_three() {
        // `:type="friend.status === 'cancel' ? 'outlined' : 'base'"`, so *Added*
        // and *Invite* are `base` and only *Cancel* is outlined.
        assert_eq!(FriendStatus::Added.kind(), ui::Kind::Standard);
        assert_eq!(FriendStatus::Invite.kind(), ui::Kind::Standard);
        assert_eq!(FriendStatus::Cancel.kind(), ui::Kind::Outlined);
        // `:disabled="friend.status === 'added'"`.
        assert!(FriendStatus::Added.disabled());
        assert!(!FriendStatus::Invite.disabled());
        assert!(!FriendStatus::Cancel.disabled());
        // `:class="friend.status === 'added' ? '' : 'w-20'"`.
        assert_eq!(FriendStatus::Added.width(), Length::Shrink);
        assert_eq!(FriendStatus::Invite.width(), Length::Fixed(80.0));
        assert_eq!(FRIEND_BUTTON_WIDTH, 20.0 * 4.0);
        // `!h-8` on a `size="md"` button: 32 tall, and `md`'s radius, padding,
        // label and icon rather than `sm`'s.
        assert_eq!(FRIEND_BUTTON_HEIGHT, 8.0 * 4.0);
        assert_eq!(MD_RADIUS, 12.0, "`rounded-xl`");
        assert_eq!(MD_PAD, 2.5 * 4.0, "`px-2.5`");
        assert_eq!(BUTTON_LABEL, 16.0);
        assert_eq!(MD_ICON, 20.0);
    }

    #[test]
    fn the_toast_is_placed_where_the_reference_places_it() {
        // `left-[32%]` of the preview's own 400 is 128, `top-[23rem]` is 368, and
        // `w-[21rem]` is 336 -- which is 464 at the far side of a 400 box, so the
        // reference's own clip is what cuts the last 64 of it.
        assert_eq!(TOAST_LEFT, 128.0);
        assert_eq!(TOAST_TOP, 23.0 * 16.0);
        assert_eq!(TOAST_WIDTH, 21.0 * 16.0);
        assert_eq!(TOAST_LEFT + TOAST_WIDTH, 464.0);
        // Which is 64 pixels past the panel, and the page viewport is what cuts it.
        assert_eq!(TOAST_LEFT + TOAST_WIDTH - PREVIEW, 64.0);
        assert_eq!(TOAST_PAD_X, 4.0 * 4.0);
        assert_eq!(TOAST_PAD_Y, 3.0 * 4.0);
        assert_eq!(TOAST_GAP, 4.0 * 4.0);
        assert_eq!(TOAST_BUTTON_GAP, 2.5 * 4.0);
        assert_eq!(TOAST_RADIUS, 2.0 * 8.0);
        assert_eq!(TOAST_AVATAR, 2.25 * 16.0);
        assert_eq!(SERVER_MARK, 1.25 * 16.0);
        // The fade is `h-[28rem]` at the preview's own bottom, so it begins 160
        // pixels down a 608 box.
        assert_eq!(PREVIEW_FADE, 28.0 * 16.0);
        assert_eq!(PREVIEW_HEIGHT - PREVIEW_FADE, 160.0);
    }

    #[test]
    fn the_toast_is_the_height_the_reference_measures() {
        // `/tmp/ref/hosting-clean3.png`: the box is y 440..551, 112 rows, and its
        // content is 453..538, 86 of them. The height is those four terms and
        // nothing else -- two `leading-5` lines, `mt-2.5`, the buttons' `h-9`,
        // and `py-3` measured from inside the `border border-solid` that CSS
        // counts in the box and iced paints inside the padding's own.
        const LINES: f32 = 2.0 * BUTTON_LINE;
        let content = LINES + TOAST_BUTTON_GAP + MD_HEIGHT;
        assert_eq!(LINES, 40.0, "two `text-base leading-5` lines");
        assert_eq!(content, 86.0, "440 + 13 = 453 and 538 = 551 - 13");
        assert_eq!(content + 2.0 * TOAST_INSET_Y, 112.0);
        assert_eq!(TOAST_BORDER, 1.0, "`border border-solid`");
        assert_eq!(TOAST_INSET_Y, 3.0 * 4.0 + TOAST_BORDER);
        assert_eq!(TOAST_INSET_X, 4.0 * 4.0 + TOAST_BORDER);
        // The content's own measure: 336 less a border and `px-4` either side,
        // which is the reference's 302 rather than the 304 a border-blind
        // padding leaves.
        assert_eq!(TOAST_WIDTH - 2.0 * TOAST_INSET_X, 302.0);
    }

    #[test]
    fn the_fade_ramp_is_the_reference_s_own_at_the_height_measured() {
        // `transparent` at the top and `--surface-1` at the bottom, and the
        // reference's own Invite button reads `(47, 49, 55)` at y=308 where
        // `--surface-4` is `(52, 54, 60)`: 76 of the 448 pixels above the box's
        // bottom, which is 17 percent of the ramp.
        let theme = Gen::Dark;
        let (surface_1, surface_4) = (
            theme_gen::ink(theme, Ink::Surface1),
            theme_gen::ink(theme, Ink::Surface4),
        );
        // y=308 is 236 pixels into the panel, and the box's own ramp runs from 160
        // (its top, `608 - 448`) to 608 -- so 76 of the way along it.
        let at = (308.0 - 72.0 - (PREVIEW_HEIGHT - PREVIEW_FADE)) / PREVIEW_FADE;
        let expected = [
            surface_4.r + (surface_1.r - surface_4.r) * at,
            surface_4.g + (surface_1.g - surface_4.g) * at,
            surface_4.b + (surface_1.b - surface_4.b) * at,
        ];
        for (channel, value) in expected.iter().enumerate() {
            let rounded = (value * 255.0).round() as u8;
            let measured = [47u8, 49, 55][channel];
            assert!(
                (rounded as i32 - measured as i32).abs() <= 1,
                "channel {channel}: ramp says {rounded}, the reference reads {measured}"
            );
        }
    }

    #[test]
    fn the_plate_carries_the_ramp_slice_a_plate_shows() {
        // A 100-pixel layer at `left-[-1px] top-[-1px]`, clipped to a 38-pixel
        // padding box, shows rows 1..38 of its ramp -- and plate row `y` is
        // `(y + 1) / 100` along it, so the slice is 2/100 to 39/100 and not the
        // whole ramp.
        assert_eq!(RAMP_FROM, 2.0 / 100.0);
        assert_eq!(RAMP_TO, 39.0 / 100.0);
        assert!((RAMP_TO - RAMP_FROM - 37.0 / 100.0).abs() < 1e-6);
        // The shade's two stops, in the order they are written.
        assert_eq!(SHADE_FROM, 0.08);
        assert_eq!(SHADE_TO, 0.86);
        assert_eq!(SHADE_ALPHA, 0.37);
        assert_eq!(SHADE_ANGLE, -14.0);
        assert_eq!(TEXTURE_ALPHA, 0.40);
        assert_eq!(TEXTURE_BOX, (100.0, 157.0));
        // `-14deg` points up and to the left, so the ramp's first stop is the
        // bottom-right corner: the plate is darkest there, which is where the
        // reference's own plate is darkest (measured `(10, 49, 29)` at its
        // bottom-right against `(17, 60, 38)` at its top-left).
        assert!(shade_alpha(99.0, 99.0) > shade_alpha(1.0, 1.0));
        // And it is transparent across most of the box, because its first stop is
        // at 8 percent and its last at 86.
        assert!(shade_alpha(50.0, 50.0) < SHADE_ALPHA / 2.0);
    }

    #[test]
    fn the_texture_window_is_the_crop_the_reference_shows() {
        // 38 by 38 -- the plate's padding box -- of a 2880 by 2788 source, and
        // every channel of it is dark: the texture is a near-black blue-grey, its
        // brightest channel in the whole picture is 58.
        let window = texture_window();
        assert!(!window.is_empty(), "icon-texture.png must decode");
        assert_eq!(window.len(), 38 * 38 * 4, "the window is the padding box");
        let brightest = window[..]
            .chunks(4)
            .map(|pixel| pixel[..3].iter().copied().max().unwrap_or(0))
            .max()
            .unwrap_or(0);
        assert!(
            (55..=64).contains(&brightest),
            "the texture's brightest channel is {brightest}, and its source's is 58"
        );
        // And it is the *middle* of the picture that is shown, so the window is
        // neither a corner of the source nor a corner of the plate.
        let middle = (19 * 38 + 19) * 4;
        assert!(window[middle + 3] > 0, "the middle of the window is drawn");
        // `object-fit: cover` centres its crop, and the 52 rows the scaled source
        // is taller than the `<img>` are 26 off the top and 26 off the bottom.
        // Reading that term as zero is what put the window 26 rows high, and the
        // window it produced carried none of the structure the reference's own
        // plate shows.
        let (box_height, box_width) = TEXTURE_BOX;
        let cover = (box_width / 2880.0).max(box_height / 2788.0);
        assert_eq!((2880.0 * cover).round(), 157.0);
        assert_eq!((2788.0 * cover).round(), 152.0);
        assert_eq!((152.0 - box_height) / 2.0, 26.0);
    }

    /// The plate's interior as the reference's own pixels measure it, at one
    /// pixel of the first plate: the overlay's straight colour and alpha over
    /// the ramp at that row, which is the shade and the texture. In 0..255, the
    /// scale a capture reads in.
    fn plate_interior(theme: Gen, x: usize, y: usize) -> [f32; 3] {
        fn channels(color: Color) -> [f32; 3] {
            [color.r, color.g, color.b]
        }
        let pixels = plate_overlay_pixels(theme);
        let index = (y * 40 + x) * 4;
        let layer = &pixels[index..index + 4];
        let alpha = f32::from(layer[3]) / 255.0;
        // The ramp at this row of the plate: `--color-green-800` to
        // `--color-green-950` over the first `1 + y` percent of a 100-pixel
        // layer, at the `opacity: 0.5` it carries, over `--color-surface-1`.
        let t = (y as f32 + PLATE_INSET as f32) / 100.0;
        let green_800 = channels(theme_gen::ink(theme, Ink::Green800));
        let green_950 = channels(theme_gen::ink(theme, Ink::Green950));
        let surface = channels(theme_gen::ink(theme, Ink::Surface1));
        let mut out = [0.0f32; 3];
        for channel in 0..3 {
            let ramp = 0.5 * (green_800[channel] + (green_950[channel] - green_800[channel]) * t)
                + 0.5 * surface[channel];
            let straight = f32::from(layer[channel]) / 255.0;
            out[channel] = (straight * alpha + ramp * (1.0 - alpha)) * 255.0;
        }
        out
    }

    #[test]
    fn the_plate_interior_is_the_colour_the_reference_measures() {
        // `/tmp/ref/hosting-clean3.png`, the first plate at its interior row 6,
        // column 17: `#0F3A24`, and the modal of the whole pad `#113C26`. The
        // texture at the `opacity-40` its class also carries puts steps into the
        // blue channel and takes them out of the green, which is the whole of the
        // delta the audit measured on this plate.
        let theme = Gen::Dark;
        let measured = plate_interior(theme, 17, 6);
        assert!(
            (13.0..=16.0).contains(&measured[0]),
            "red reads {:.1}, and the reference's is 15",
            measured[0]
        );
        assert!(
            (55.0..=58.0).contains(&measured[1]),
            "green reads {:.1}, and the reference's is 58",
            measured[1]
        );
        assert!(
            (34.0..=37.0).contains(&measured[2]),
            "blue reads {:.1}, and the reference's is 36",
            measured[2]
        );
        // The texture is read and laid into the plate, and `mix-blend-luminosity`
        // is what leaves so little of it: at [`TEXTURE_SHARE`] the picture's own
        // row of structure is under one 8-bit step, so the plate is flat where the
        // reference's varies by three. That is the blend, measured.
        let window = texture_window();
        let row: Vec<u8> = (0..38).map(|x| window[(19 * 38 + x) * 4 + 1]).collect();
        let low = row.iter().copied().min().unwrap_or(0);
        let high = row.iter().copied().max().unwrap_or(0);
        assert!(high > low + 8, "the window's row is flat: {low} against {high}");
        let pixels = plate_overlay_pixels(theme);
        let green = |x: usize| pixels[(6 * 40 + x) * 4 + 1];
        assert!(
            green(6).abs_diff(green(20)) <= 1,
            "the pad varies by more than the blend leaves: {} against {}",
            green(6),
            green(20)
        );
        // `TEXTURE_SHARE` is that measurement and not a guess: the blend leaves
        // the texture this much of its own colour, and the rest of the
        // `opacity-40` slot is `--color-green-950`.
        assert!((0.17..=0.18).contains(&TEXTURE_SHARE), "{TEXTURE_SHARE}");
        assert!((0.4 * TEXTURE_SHARE - 0.07).abs() < 0.005);
        // And the slice runs the way the reference's does: lighter at the plate's
        // top than at its foot, and nowhere near `--color-green-950` at the foot
        // the way a whole-ramp gradient reaches it. The reference's own plate
        // measures green 61 at interior row 1 and 54 at row 37; this reads 57 and
        // 53, the span narrowed by the `mix-blend-luminosity` share the texture
        // takes of both ends.
        let pixels = plate_overlay_pixels(theme);
        let top = pixels[(40 + 19) * 4 + 1];
        let foot = pixels[(37 * 40 + 19) * 4 + 1];
        assert!(top > foot, "the slice is flat: {top} against {foot}");
    }

    #[test]
    fn the_plate_overlay_is_a_rounded_opaque_picture_in_every_theme() {
        for theme in Gen::ALL {
            let pixels = plate_overlay_pixels(*theme);
            assert_eq!(pixels.len(), (40 * 40 * 4) as usize, "{theme:?}");
            // The middle is inside the padding box, so it carries the whole
            // composite -- ramp, shade and texture -- and it is opaque: the
            // padding box is the plate's entire interior and the plate's own
            // background is not painted behind it.
            let middle = (20 * 40 + 20) * 4;
            let [r, g, b, a] = pixels[middle..middle + 4].try_into().unwrap_or([0, 0, 0, 0]);
            assert_eq!(a, 0xff, "{theme:?}: the middle is not opaque");
            assert!(
                g > r && g > 20,
                "{theme:?}: the middle reads ({r}, {g}, {b}, {a}), which is not a green"
            );
            // The four corners are outside its rounded corner, so they are clear:
            // the border is drawn under them and `overflow: hidden` clips to it.
            for (x, y) in [(0usize, 0usize), (39, 0), (0, 39), (39, 39)] {
                let index = (y * 40 + x) * 4;
                assert_eq!(pixels[index + 3], 0, "{theme:?}: the corner {x},{y} must be clear");
            }
            // A pixel on the straight edge of the padding box is kept: 14 - 1 = 13
            // rounds the corners and leaves the sides alone.
            for (x, y) in [(1usize, 20usize), (38, 20), (20, 1), (20, 38)] {
                let index = (y * 40 + x) * 4;
                assert!(pixels[index + 3] > 0, "{theme:?}: the edge {x},{y} must be kept");
            }
        }
    }

    #[test]
    fn the_picture_s_avatars_decode_at_the_sizes_the_reference_draws_them() {
        // Eight friends at `size="1.5rem"` round, Geometrically at
        // `size="2.25rem"` round, and the server's mark at `size="1.25rem"` on the
        // rounded square an `Avatar` without `circle` takes.
        for (index, friend) in FRIENDS.iter().enumerate() {
            let icon = friend_avatar(index).expect("a PNG in the tree must decode");
            assert!(!friend.avatar.is_empty(), "row {index} carries no bytes");
            assert_eq!(icon.handle().id(), icon.handle().id());
        }
        assert!(geometrically().is_some(), "geometrically.png must decode");
        assert!(modrinth_smp().is_some(), "modrinth-smp.png must decode");
        // And decoding is a cache, not a per-frame cost: the same handle comes
        // back twice.
        assert_eq!(friend_avatar(0).map(avatar::Icon::handle), friend_avatar(0).map(avatar::Icon::handle));
    }

    #[test]
    fn a_base_button_and_the_standard_kind_are_the_same_picture_in_the_dark_theme() {
        // `ButtonFrame.vue`'s `base` is `bg-surface-4 text-contrast` over
        // `inset 0 0 0 1px var(--surface-5)`, and `--color-button-bg` *is*
        // `var(--surface-4)`, so the fill and the ring `Kind::Standard` draws are
        // the reference's own numbers rather than an approximation of them.
        assert_eq!(theme_gen::ink(Gen::Dark, Ink::Surface4), theme_gen::ink(Gen::Dark, Ink::ButtonBg));
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, Ink::Surface4), [0x34, 0x36, 0x3c, 0xff]);
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, Ink::Surface5), [0x42, 0x44, 0x4a, 0xff]);
    }
}

