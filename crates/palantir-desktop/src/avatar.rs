//! The reference's `Avatar`: a project's icon, fitted into its box and rounded.
//!
//! A search hands out an `icon_url` per hit and the bytes behind it are a PNG
//! somebody uploaded. `Avatar.vue` puts those bytes in an `<img>` and rounds the
//! box with CSS:
//!
//! ```css
//! outline: 1px solid rgb(255 255 255 / 15%);
//! outline-offset: -1px;
//! background-color: var(--color-button-bg);
//! object-fit: contain;
//! border-radius: calc(16 / 96 * var(--_override-size, var(--_size)));
//! ```
//!
//! Four rules, and all of them are done here on the pixels rather than asked of
//! the toolkit. The radius is *proportional to the drawn size*, so the corner
//! reads the same at a 100px row icon and at a 64px one; and iced has no rounded
//! image -- a `container`'s border radius paints the container's own background
//! and does not clip what is drawn over it, so a rounded frame around a square
//! image shows square corners poking out of the frame's corners. So the picture is
//! composited onto a canvas the size of the box, filled with the colour the
//! element's own background would be, and the alpha outside the rounded rectangle
//! is cleared: [`crate::ui::icon_box`] then draws a handle that already has the
//! shape, over the box's own background and hairline.
//!
//! Because the canvas is the box, the four things `Avatar.vue` gets from CSS all
//! fall out of the one composite:
//!
//! * **`contain`**, so a picture that is not square is letterboxed and centred
//!   inside the box rather than stretched, and a picture *smaller* than the box is
//!   scaled up to it, which is what a browser does with `object-fit: contain`.
//! * **The corner clip**, which CSS applies to the element's box -- so the mask is
//!   the box's, and a letterboxed picture is clipped where it reaches a corner
//!   exactly as the reference clips it.
//! * **The background**, `var(--color-button-bg)` painted on the element's own box
//!   *behind* the picture, which is what the letterbox shows. Without it a
//!   300x307 avatar in a 96 box (`UserPageHeader`'s) resizes to 94x96 and the
//!   resulting one-pixel gutters are transparent, so the disc measures 94 wide
//!   where the reference's measures 96.
//! * **`image-rendering: pixelated`**, which `Avatar.vue` turns on when
//!   `naturalWidth < 32`: a tiny icon is scaled with nearest-neighbour and reads
//!   as blocks rather than as a blur.
//!
//! And the fourth rule is the `outline`, which is drawn into the pixels here for
//! the round avatar and by [`crate::ui::icon_box`]'s container border for a
//! project icon -- see [`masked_as`]'s own note on why it cannot be one rule.
//!
//! The fifth is the shadow, and it is the one rule of the component that is
//! drawn per *element* rather than per shape. `Avatar.vue:299` gives every
//! avatar that is not `.no-shadow` a `box-shadow: var(--shadow-card)` -- which in
//! the dark look is `rgba(0, 0, 0, 0.25) 0px 2px 4px 0px` (`variables.scss:368`)
//! -- and of the four places this crate draws a circle exactly one asks for it:
//! `UserPageHeader.vue:4`. `ServerListEmptyPreview.vue:47` and `:100` and
//! `ServerListing.vue:54` all pass `no-shadow`, so their avatars are the plain
//! card [`Icon::circle`] builds, and `Icon::shadowed_circle` is the header's.
//! The shadow is painted into the pixels in bands of measured depth rather than
//! asked of an iced `Shadow`, because those are composited *inside* the element's
//! own rounded-box coverage by `solid.wgsl` and so band the very fill they are
//! meant to sit behind -- which is why [`crate::shell`] draws its own in bands and
//! so does this. See [`with_card_shadow`] and [`SHADOW_DEPTHS`].
//!
//! Four deliberate limits, each named rather than approximated:
//!
//! * **No placeholder art.** The reference falls back to an inline hexagon in
//!   `#9a9a9a` when a project has no icon -- and to a tinted version of it on a
//!   profile page -- so a missing icon is a drawn box rather than a hole. That path
//!   is a stroke path declared inside a `.vue` file rather than one of the 313 SVGs
//!   [`crate::icons_gen`] compiles, so drawing it means teaching the generator a
//!   source that is not a file; until then a card with no icon draws the empty box
//!   the placeholder sits on, which is the same box, its same background and its
//!   same hairline, without the hexagon.
//! * **No container query.** `ProjectCard.vue` drops the icon to 64px below 850px
//!   of card width. iced has no container queries and a page here is built from its
//!   state rather than from its pane's width, so every card draws [`ICON_SIDE`].
//! * **No cache of its own.** Decoding happens once per icon per page, in the
//!   store's worker (see [`crate::store::Store::project_icons`]), and what a page
//!   keeps is the [`Icon`] it was given.
//! * **The shadow does not land on a page yet.** [`Icon::shadowed_circle`] draws
//!   it, at the geometry and the depths [`ref/user-ref.png`] measures, but the
//!   shadow's reach is *outside* the card's own box and every caller in this
//!   crate lays its handle out in a box the size of the card -- so `image`'s
//!   default `ContentFit::Contain` scales the whole 102-pixel canvas into the
//!   96-pixel slot the header's avatar sits in. Measured on our own capture, that
//!   draws the disc 90.35 across instead of 96 and lays the shadow's five bands
//!   down *inside* the card where the reference has the picture's own pixels, and
//!   it would shrink `/hosting/manage`'s nine avatars from 24 and 36 to 19.2 and
//!   28.8. Both callers are in files this one does not own; see
//!   [`Icon::shadowed_circle`] for the two lines that fix it.
//!
//! One thing this cannot do at all: an image whose bytes are not an image this
//! build's `image` crate reads is refused rather than drawn as a grey square, so a
//! broken icon is a missing icon.

use iced::widget::image::Handle;

use crate::theme_gen::{self, Ink, Theme as Gen};

/// The radius rule, as a fraction of the drawn size.
///
/// `Avatar.vue`'s `calc(16 / 96 * …)`: the reference's own 96px avatar rounds by
/// 16, and every other size scales that rather than repeating it.
pub const RADIUS_SHARE: f32 = 16.0 / 96.0;

/// The side a project icon is drawn at.
///
/// `ProjectCard.vue`'s `Avatar size="100px"`, which its own container query drops
/// to 64px below 850px of card width -- a case [`crate::avatar`]'s module docs
/// record as not drawn here.
pub const ICON_SIDE: u32 = 100;

/// The width below which the reference scales an icon nearest-neighbour.
///
/// `Avatar.vue`'s `image.naturalWidth < 32`: an icon that small is scaled up to
/// the box as blocks rather than as a blur.
const PIXELATED_BELOW: u32 = 32;

/// `Avatar.vue:284`'s `outline: 1px solid rgb(255 255 255 / 15%)`, as the
/// premultiplied alpha a browser blends it with.
///
/// 15% of 255 is 38.25, and Chromium truncates the result rather than rounding it:
/// 38 out of 255 over the reference's own `#343a3f` reads `#52575b`, which
/// [`toward_white`]'s arithmetic gives exactly and a rounding one gives `#52585c`.
const OUTLINE_ALPHA: u8 = 38;

/// `Avatar.vue:286`'s `background-color: var(--color-button-bg)`, as the three
/// channels a canvas is filled with.
///
/// The reference resolves `--color-button-bg` to `--surface-4`
/// (`variables.scss:329`), which its dark look declares `#34363c`
/// (`variables.scss:238`). Read out of the generated table rather than written
/// here, so a palette change moves it with the paint. Note that the reference's
/// own profile header cannot be used to check this value: its outline covers the
/// one-pixel gutter exactly, so the gutter's colour is never a pixel of the
/// capture -- the ring over it is, and that is a different measurement.
fn button_bg(theme: Gen) -> [u8; 3] {
    let [red, green, blue, _] = theme_gen::ink_rgba(theme, Ink::ButtonBg);
    [red, green, blue]
}

// ---- The card's own shadow -----------------------------------------------
//
// `Avatar.vue:299`'s `box-shadow: var(--shadow-card)`, and the dark look's
// `--shadow-card` (`variables.scss:368`) is
// `rgba(0, 0, 0, 0.25) 0px 2px 4px 0px`: no spread, two pixels down, a
// four-pixel blur. Three of those four numbers this file can use as they are
// written; the fourth -- the blur -- is what a CSS blur *radius* means, and
// spreading a solid edge by half of it is a property of the spec rather than
// something a rasteriser is obliged to agree with, so the reach is measured and
// the falloff is measured. What follows is those measurements, and nothing in
// it is a Gaussian.
//
// # What was measured, and where
//
// Off `ref/user-ref.png`, the profile header's own avatar: a 96 box at
// `x=88..183`, `y=72..167`, centre `(136.0, 120.0)`, radius 48, on the page's
// own `--surface-1` `#16181c` `(22, 24, 28)`.
//
// ```text
//                   the card             the shadow's five steps out
//   x=85,86,87  (21,23,27) (21,23,26) (20,22,25)   |  the disc: x=88..183
//   y=71        (21,23,27)                          |  one row above y=72
//   y=168..172 (18,20,23) (19,21,24) (20,22,25) (21,23,26) (21,23,27)
// ```
//
// Two things come out of that. The shadow reaches **five** rows below a disc
// whose last row is `167` and **three** columns either side of a disc that runs
// `88..183`, and only **one** row above it: the shape is the card's own circle
// moved down [`SHADOW_OFFSET_Y`] and blurred, so the reach is the offset circle
// grown by [`SHADOW_REACH`] rather than the card grown by it -- which is why the
// bottom has five rows where the top has one. And each step is a *pre-composited
// ink*, not an alpha: `(18,20,23)` over `(22,24,28)` is `22 * (1 - a)` truncated
// to a byte at `a = 0.1429..0.1667`, so a depth is only ever pinned to the
// half-open interval a byte can express. [`SHADOW_DEPTHS`] takes the middle of
// each interval, which is what makes the five steps reproduce the capture's own
// bytes exactly rather than nearly.

/// `variables.scss:368`'s `0px 2px 4px 0px`, offset: the shadow is the card's
/// own shape moved down this far, with no spread.
///
/// Which is why the shadow's centre is the card's centre *plus* this, and why
/// the capture's shadow is one row above the card and five below it.
const SHADOW_OFFSET_Y: f32 = 2.0;

/// `variables.scss:368`'s `4px`.
///
/// A radius, so a solid edge spreads `blur / 2` past it -- but see
/// [`SHADOW_REACH`], which is one pixel more than that.
const SHADOW_BLUR: f32 = 4.0;

/// How far past the card's own edge the shadow is drawn, all four sides.
///
/// `blur / 2 = 2` is what the token's own arithmetic gives, and the reference
/// gives three. The capture above is the receipt: down the centre column the
/// page's own `#16181c` runs five rows past the disc's last row of `167`, and
/// across the centre row it runs three columns past `88` and `183`, which is the
/// offset circle (the card, moved down [`SHADOW_OFFSET_Y`]) grown by three. The
/// third pixel is the blur's own support rather than its nominal spread: a
/// rasteriser cuts a Gaussian's tail a pixel or so past the radius the spec
/// quotes, and `ref/user-ref.png` has nothing at all at `x=84` or at `y=173`,
/// which is where a fourth would land.
///
/// Public because a caller that draws a shadowed card has to lay the handle out
/// at `side + 2 * SHADOW_REACH`: see [`Icon::shadowed_circle`].
pub const SHADOW_REACH: f32 = SHADOW_BLUR / 2.0 + 1.0;

/// `variables.scss:368`'s own alpha at the shadow's peak, and the depth
/// [`SHADOW_DEPTHS`] is expressed as a share of.
const SHADOW_ALPHA: f32 = 0.25;

/// The light look's `--shadow-card` (`variables.scss:140`):
/// `rgba(50, 50, 100, 0.1) 0px 2px 4px 0px`.
///
/// The same offset and the same blur as the dark look's, so [`SHADOW_REACH`] and
/// [`SHADOW_DEPTHS`] are the geometry for both and only the ink moves. The tint
/// is a real colour rather than black, so it is named here rather than folded
/// into [`Shadow::ink`]; nothing in this crate paints the light look (see
/// [`masked_as`]), so unlike every other number here it is *not* measured off a
/// capture and this file does not claim it is.
const SHADOW_TINT: [u8; 3] = [50, 50, 100];

/// The light look's own alpha, against which [`Shadow::ink`] scales the depths.
const SHADOW_LIGHT_ALPHA: f32 = 0.1;

/// The depth of each of the five rings the shadow's reach is cut into, darkest
/// first, one pixel of reach per ring.
///
/// Read off `ref/user-ref.png` as in the note above, down the disc's own centre
/// column `x=136` where the card is opaque and cannot hide any of it, and
/// cross-checked across the disc's centre row `y=120` where the outer three of
/// the five are all the row has room for:
///
/// ```text
/// depth   0.155   0.116   0.077   0.039   0.018
/// ink     (18,20,23) (19,21,24) (20,22,25) (21,23,26) (21,23,27)
/// s at    46.5    47.5    48.5    49.5    50.5   pixels from the shadow's centre
/// ```
///
/// The five numbers are the middles of the intervals a byte can hold -- the
/// first is 0.1429..0.1667 and the last 0..0.0357 -- so they are not more
/// precise than the capture and are not claimed to be. The token's own `0.25` is
/// *not* one of them: it is the peak at the shadow's core, which is under the
/// card and is never a pixel of a capture.
const SHADOW_DEPTHS: [f32; 5] = [0.155, 0.116, 0.077, 0.039, 0.018];

/// The `rgba` a `--shadow-card` is, and the colour it is composited over.
///
/// Which is all of the shadow except its falloff: the offset, the blur and the
/// reach come from the token and the measurement, and neither look changes them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Shadow {
    /// What is behind the card, which is what the shadow darkens.
    ///
    /// `--surface-1`, because that is the page a profile's header avatar is drawn
    /// on and therefore the colour the reference's own capture composites over.
    /// Not the card's own `--color-button-bg`, which would halve every depth's
    /// effect: `#34363c` against `#16181c` is a little over twice the value, and
    /// the darkest ring would read `(13,14,17)` where the capture reads
    /// `(18,20,23)`.
    page: [u8; 3],
    /// The shadow's own colour: black in the dark look, [`SHADOW_TINT`] in the
    /// light one.
    tint: [u8; 3],
    /// The token's alpha, which scales [`SHADOW_DEPTHS`].
    alpha: f32,
}

impl Shadow {
    /// The `--shadow-card` of the look `theme` is, over that look's page.
    ///
    /// The theme comes from the caller for the same reason [`button_bg`]'s does:
    /// a card is handed an [`Icon`] and has a theme of its own, and the
    /// constructors below pass the look the app opens in rather than reaching for
    /// one they are not given.
    fn of(theme: Gen) -> Shadow {
        let [red, green, blue, _] = theme_gen::ink_rgba(theme, Ink::Bg);
        let (tint, alpha) = match theme {
            Gen::Dark => ([0, 0, 0], SHADOW_ALPHA),
            _ => (SHADOW_TINT, SHADOW_LIGHT_ALPHA),
        };
        Shadow {
            page: [red, green, blue],
            tint,
            alpha,
        }
    }

    /// Ring `index`'s ink, pre-composited over the page behind the card.
    ///
    /// `22 * (1 - a)` and nothing else, because the dark look's shadow is black
    /// over an opaque colour and a browser's blend of those is a scale of each
    /// channel. Truncating, because that is what the capture's own compositor
    /// does and because truncating is what puts all three channels of a ring on
    /// the same triple: rounding `28 * (1 - 0.018) = 27.496` gives `#1b` like
    /// truncation does, but rounding `28 * (1 - 0.039) = 26.908` gives `#1b`
    /// where truncation gives the `#1a` the capture reads.
    fn ink(&self, index: usize) -> [u8; 4] {
        let alpha = self.alpha * SHADOW_DEPTHS[index] / SHADOW_ALPHA;
        let mut ink = [u8::MAX; 4];
        for channel in 0..3 {
            let page = f32::from(self.page[channel]);
            let tint = f32::from(self.tint[channel]);
            ink[channel] = (page * (1.0 - alpha) + tint * alpha) as u8;
        }
        ink
    }
}

/// The canvas a `side`-square card and its shadow need: the shadow's own
/// footprint, which is the card's box moved down [`SHADOW_OFFSET_Y`] and grown
/// by [`SHADOW_REACH`] on every side.
///
/// Returned as `(canvas side, card x, card y)`. The card's own corner is at
/// `(reach, reach - offset)` and not `(reach, reach)`: the footprint is the
/// offset shape's, so the card sits a pixel below its top edge -- which is what
/// leaves the reference's *one* row above the disc rather than five below it
/// becoming five above. A 96 card is a 102 canvas with the card at `(3, 1)`.
fn shadow_canvas(side: u32) -> (u32, u32, u32) {
    let reach = SHADOW_REACH.round() as u32;
    let down = SHADOW_OFFSET_Y.round() as u32;
    (side + 2 * reach, reach, reach.saturating_sub(down))
}

/// The ring `index`'s outer radius, measured from the shadow's centre.
///
/// [`SHADOW_REACH`] past the card's own radius, less a ring per ring already
/// taken off it -- which is the even cut [`SHADOW_DEPTHS`] is, one pixel of
/// reach each. The *deepest* ring is the inner one, because that is the order a
/// shadow's own alpha falls in: so the faintest ring ends exactly at the reach
/// and the deepest stops one pixel short of the card's own edge, where the
/// card's opaque disc hides it anyway.
fn ring_radius(side: u32, index: usize) -> f32 {
    let inner = SHADOW_DEPTHS.len() - 1 - index;
    side as f32 / 2.0 + SHADOW_REACH - inner as f32
}

/// `card` with the shadow its `side`-square box casts, painted in behind it.
///
/// The card's own pixels are *copied*, not redrawn: what this returns is the
/// canvas [`masked_as`] built, untouched and at its own offset, with rings of
/// [`SHADOW_DEPTHS`]'s inks around it. That is the whole of the argument that the
/// card's fill, its picture, its outline and its radius are unchanged by the
/// shadow -- the code that draws them does not run here at all.
///
/// The rings are drawn by radius about the offset circle's centre and are *not*
/// clipped to the card, so a pixel inside the card's own box can carry shadow:
/// the reference does exactly that, at the corners of the disc's bounding box
/// where the disc has cleared away and the blurred edge has not yet ended. The
/// clip to the disc is the card paste, which comes after and wins.
fn with_card_shadow(card: Picture, shadow: Shadow) -> Picture {
    let Picture { side, pixels } = card;
    let (canvas, at_x, at_y) = shadow_canvas(side);
    let centre_x = at_x as f32 + side as f32 / 2.0;
    let centre_y = at_y as f32 + SHADOW_OFFSET_Y + side as f32 / 2.0;
    // Squared radii, so the test is one compare and no square root: the same
    // shape, asked once at setup instead of per pixel.
    let radii: Vec<f32> = (0..SHADOW_DEPTHS.len())
        .map(|ring| {
            let radius = ring_radius(side, ring);
            radius * radius
        })
        .collect();
    let inks: Vec<[u8; 4]> = (0..SHADOW_DEPTHS.len())
        .map(|ring| shadow.ink(ring))
        .collect();
    let mut out = vec![0u8; (canvas * canvas * 4) as usize];
    for y in 0..canvas {
        for x in 0..canvas {
            let (dx, dy) = (x as f32 + 0.5 - centre_x, y as f32 + 0.5 - centre_y);
            let squared = dx * dx + dy * dy;
            // The first ring whose own radius the pixel is inside: the radii
            // run outwards and each ring owns the band up to the next one out.
            let ring = radii.iter().position(|r| squared < *r);
            if let Some(ring) = ring {
                let offset = ((y * canvas + x) * 4) as usize;
                out[offset..offset + 4].copy_from_slice(&inks[ring]);
            }
        }
    }
    for y in 0..side {
        for x in 0..side {
            let from = ((y * side + x) * 4) as usize;
            let to = (((y + at_y) * canvas + x + at_x) * 4) as usize;
            out[to..to + 4].copy_from_slice(&pixels[from..from + 4]);
        }
    }
    Picture {
        side: canvas,
        pixels: out,
    }
}

/// One channel moved toward white by `alpha` out of 255, the way a browser
/// composites a translucent outline over what is already there.
///
/// Integer and truncating, because that is what the reference is measured with:
/// the blue channel of the picture's own `#343a3f` lifts by 192 * 38 / 255 =
/// 28.61, which truncates to the `#5b` the capture reads at the top of a disc
/// and rounds to a `#5c` it does not.
fn toward_white(channel: u8, alpha: u8) -> u8 {
    let lift = u16::from(255 - channel) * u16::from(alpha) / 255;
    (u16::from(channel) + lift) as u8
}

/// The radius a square of `side` pixels takes, rounded to whole pixels.
pub fn radius(side: u32) -> u32 {
    ((side as f32 * RADIUS_SHARE).round() as u32).min(side / 2)
}

/// A rounded mask over a square of `side` pixels, as one bit per pixel.
///
/// A function of its inputs, so the shape can be asserted without a PNG: a corner
/// must be `false`, the middle `true`, and the mask must be symmetric about both
/// axes -- the three ways a hand-rolled distance test goes wrong (an off-by-half
/// centre, a radius measured in the wrong units, an axis left unmirrored).
fn mask(side: u32) -> Vec<bool> {
    let radius = radius(side) as f32;
    let (centre, edge) = (side as f32 / 2.0, side as f32 / 2.0 - radius);
    let mut out = Vec::with_capacity((side * side) as usize);
    for y in 0..side {
        for x in 0..side {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Only the four corner boxes can be outside a rounded rectangle: the
            // straight edges between them are inside it by construction. Testing
            // the distance only there is what keeps this from being a circle.
            let (dx, dy) = (px - centre, py - centre);
            let outside = if dx.abs() <= edge || dy.abs() <= edge {
                false
            } else {
                let (cx, cy) = (dx.abs() - edge, dy.abs() - edge);
                cx * cx + cy * cy > radius * radius
            };
            out.push(!outside);
        }
    }
    out
}

/// The same mask for a `circle`, which is what `Avatar.vue`'s `circle` prop asks
/// for and what a profile's own avatar is drawn with.
///
/// A different shape rather than a different radius: the rounded rectangle above
/// tests the distance from the corner box's centre, so it cannot become a circle
/// by growing its radius -- at `side / 2` the straight edges would vanish and the
/// corner boxes would meet, and the result would be a rounded square with no
/// straight part at all rather than a disc. So the test is from the middle.
fn circle_mask(side: u32) -> Vec<bool> {
    let centre = side as f32 / 2.0;
    let radius = centre;
    let mut out = Vec::with_capacity((side * side) as usize);
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 + 0.5 - centre, y as f32 + 0.5 - centre);
            out.push(dx * dx + dy * dy <= radius * radius);
        }
    }
    out
}

/// A picture on the box's canvas: `side * side` pixels of RGBA.
///
/// Private, and only ever the input to one [`Icon`]: what a card is handed is the
/// handle, and what a test reads is this.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Picture {
    side: u32,
    pixels: Vec<u8>,
}

/// `bytes` fitted into a `side`-square box, letterboxed and rounded.
///
/// `None` when the bytes are not an image this build reads, or when they are
/// empty -- which is the fetch having failed rather than a project having no icon,
/// and both are the caller's to report.
///
/// A project icon, which is the shape whose outline [`crate::ui::icon_box`]
/// already draws as a `container` border, so this asks for no ring.
fn masked(bytes: &[u8], side: u32) -> Option<Picture> {
    masked_as(bytes, side, false, false, button_bg(Gen::Dark))
}

/// [`masked`], with the shape, the outline and the background asked for.
///
/// `circle` is `Avatar.vue`'s own prop, which `UserPageHeader.vue` passes for a
/// profile's avatar and which nothing else in the app's project pages uses.
/// `outline` is the same component's `outline`/`outline-offset: -1px` pair, drawn
/// into the pixels rather than asked of a widget -- see the note on it below.
/// `fill` is the `--color-button-bg` the element's own background would be, which
/// is what a letterboxed picture's gutters show.
///
/// The dark look is what the two callers pass. The theme is a property of the
/// widget tree rather than of the bytes -- [`crate::ui::icon_box`] takes it as an
/// argument and the pages have one -- and this function is called from the store's
/// worker, from a profile page and from the servers page, none of which can hand
/// a theme to a call whose signature they do not own. So the parameter exists and
/// the callers pass the theme the app opens in (`theme_gen::Theme::Dark`, "the one
/// the app opens in by default") rather than the one the caller happens to paint.
///
/// # Why the outline is composited and not a border
///
/// `outline-offset: -1px` puts the outline *inside* the box, and CSS clips an
/// outline to the element's radius -- so on a `circle` avatar it is a one-pixel
/// ring following the circle. A `container` cannot be that: iced draws a
/// `container`'s border as a rounded *rectangle* and its `clip` is a plain
/// rectangle, so a 96 box with a 48 radius would come out with square corners.
/// The ring is therefore `radius - 1 < distance <= radius` around the same centre
/// [`circle_mask`] measures from, and only on that path: a project icon's outline
/// is the 16/96 rounded rectangle, which a `container` border *can* draw, and
/// [`crate::ui::icon_box`] already draws it.
///
/// # Why the shadow is not drawn here
///
/// `Avatar.vue:299`'s `box-shadow: var(--shadow-card)` is painted *behind* the
/// card, and this function's canvas is the card's own box -- the picture, the
/// `--color-button-bg` behind it, the circle mask and the outline are all fitted
/// to `side`, and the four of them are one composite. So the shadow is not drawn
/// here but one step out, by [`with_card_shadow`], on a canvas this one's output
/// is copied into whole: the card's pixels are the bytes [`masked_as`] already
/// wrote, and the code that wrote them does not run again.
///
/// Which is also why the reach is the *measured* [`SHADOW_REACH`] and not the
/// Gaussian this note used to quote. Three sigma of a `blur/2 = 2` blur is six
/// pixels, so a 96 card wanted a 108 canvas and a row 12 taller; the capture
/// says the shadow stops three pixels out, which is a 102 canvas and six.
///
/// And it is drawn into the bitmap rather than asked of a `container`'s
/// `Shadow` for the reason [`crate::shell::shadow_band`] gives, which is the
/// same one: iced 0.12.3 composites a `Shadow` inside its own element's rounded
/// box coverage (`solid.wgsl`'s
/// `mix(base_color, shadow_color, (1.0 - radius_alpha) * shadow_alpha)`), so it
/// bands the very fill it is meant to sit behind. An outer shadow has to be
/// pixels this file controls, or elements in a tree it does not own.
fn masked_as(
    bytes: &[u8],
    side: u32,
    circle: bool,
    outline: bool,
    fill: [u8; 3],
) -> Option<Picture> {
    if side == 0 || bytes.is_empty() {
        return None;
    }
    let decoded = image::load_from_memory(bytes).ok()?;
    // The filter is the reference's own two cases, decided before anything is
    // scaled: `Avatar.vue` switches the element to `image-rendering: pixelated`
    // when the picture arrives narrower than 32 pixels, and lets the browser's
    // smooth filter do the rest.
    let filter = if decoded.width() < PIXELATED_BELOW {
        image::imageops::FilterType::Nearest
    } else {
        // Lanczos for the downscale every real icon is: a Modrinth icon is
        // uploaded at 512 or a thousand and drawn at a hundred.
        image::imageops::FilterType::Lanczos3
    };
    let fitted = decoded.resize(side, side, filter);
    // The canvas is the *box* rather than the picture, which is what makes the
    // mask below the box's own rounded rectangle -- CSS clips the element's box,
    // not its content, so a letterboxed picture is cut where it reaches a corner
    // and untouched where it does not.
    //
    // And it is filled rather than transparent, because `Avatar.vue:286` paints
    // `background-color: var(--color-button-bg)` on the element's own box, behind
    // the picture: a 300x307 avatar in a 96 box fits to 94x96 and leaves a gutter
    // down each side, which the reference paints with that colour and a
    // transparent canvas left showing whatever is behind the widget.
    let mut canvas = image::RgbaImage::from_pixel(
        side,
        side,
        image::Rgba([fill[0], fill[1], fill[2], u8::MAX]),
    );
    let (width, height) = (fitted.width(), fitted.height());
    // `overlay` rather than `replace`: a browser composites the `<img>` over its
    // own background, so a picture that arrives with transparent corners keeps the
    // button's colour through them and a translucent one blends with it. `replace`
    // would copy the alpha straight through and put whatever is *behind the
    // widget* in the letterbox, which is the hole this canvas exists to close.
    image::imageops::overlay(
        &mut canvas,
        &fitted,
        ((side - width) / 2) as i64,
        ((side - height) / 2) as i64,
    );
    let kept = if circle { circle_mask(side) } else { mask(side) };
    let centre = side as f32 / 2.0;
    let mut pixels = canvas.into_raw();
    for (index, kept) in kept.iter().enumerate() {
        let offset = index * 4;
        if !kept {
            pixels[offset + 3] = 0;
        } else if outline {
            // The ring is the outer pixel of every row and column the circle
            // reaches, and nothing else: a pixel is in it when its own distance
            // from the centre is past `radius - 1`, which on the four extremes is
            // the last pixel of the box and off the diagonal is the pixel a
            // 1px ring would cover.
            let (x, y) = (index as u32 % side, index as u32 / side);
            let (dx, dy) = (x as f32 + 0.5 - centre, y as f32 + 0.5 - centre);
            if (dx * dx + dy * dy).sqrt() > centre - 1.0 {
                pixels[offset] = toward_white(pixels[offset], OUTLINE_ALPHA);
                pixels[offset + 1] = toward_white(pixels[offset + 1], OUTLINE_ALPHA);
                pixels[offset + 2] = toward_white(pixels[offset + 2], OUTLINE_ALPHA);
            }
        }
    }
    Some(Picture { side, pixels })
}

/// One project's icon, decoded once and ready to draw.
///
/// Built off the frame thread by [`crate::store::Store::project_icons`] and kept
/// by the page that draws it, because decoding on the frame thread is a PNG
/// decode per card per frame.
///
/// The picture is the *box* -- a square of the size it was made for, whatever shape
/// the source was -- and the pixels are deliberately not kept beside the handle: the
/// handle already holds them, and the one thing this icon is for is being drawn at
/// the size it was made at ([`crate::ui::icon_box`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    /// The picture, rounded, as the renderer takes it.
    handle: Handle,
}

impl Icon {
    /// `bytes` as the icon a card of `side` pixels draws.
    ///
    /// `None` for bytes that are not an image, for empty bytes, and for a box of
    /// no pixels: all three are a caller that has nothing to draw rather than an
    /// icon that is wrong.
    ///
    /// The canvas carries the element's own `--color-button-bg` behind the
    /// picture, as [`Icon::circle`]'s does. It is invisible here and not there:
    /// [`crate::ui::icon_box`] draws a container of that colour behind this handle,
    /// so a letterboxed icon shows the same colour either way -- and it is that
    /// container which draws the outline for this shape, because a 16/96 rounded
    /// rectangle is a shape a `container` border *can* draw.
    ///
    /// And this canvas is the box and nothing else, with no shadow round it,
    /// which is `ProjectCard.vue:31`'s own `no-shadow` rather than a limit of
    /// this file's: [`Icon::shadowed_circle`] grows its canvas and this does not.
    pub fn of(bytes: &[u8], side: u32) -> Option<Icon> {
        let picture = masked(bytes, side)?;
        Some(Icon {
            handle: Handle::from_pixels(picture.side, picture.side, picture.pixels),
        })
    }

    /// `bytes` as the round avatar `UserPageHeader.vue` asks for: the same
    /// `contain` fit, letterboxed and scaled, with the alpha outside a *circle* of
    /// `side` cleared rather than outside the 16/96 rounded rectangle a project
    /// icon takes.
    ///
    /// The two rules a bare `image` widget cannot be given go into the pixels
    /// instead, because this is drawn over a page rather than inside
    /// [`crate::ui::icon_box`]'s container: the `--color-button-bg` the element's
    /// own background would be, so a letterboxed avatar's gutter is that colour
    /// and the disc is the box's full `side`; and the `1px` 15%-white outline,
    /// which CSS clips to the radius, so it is a ring following the circle rather
    /// than the rounded rectangle a `container` border would draw.
    ///
    /// **No shadow.** `Avatar.vue:299` casts `--shadow-card` on every avatar that
    /// is not `.no-shadow`, and every caller of *this* is one: the eight friends
    /// on `/hosting/manage` are `ServerListEmptyPreview.vue:47` (`no-shadow`),
    /// the invite toast's photograph is `ServerListEmptyPreview.vue:100` (the
    /// same), and the owner chip is `ServerListing.vue:54`. So the canvas is the
    /// box and nothing else, and [`Icon::shadowed_circle`] is the one constructor
    /// that grows.
    pub fn circle(bytes: &[u8], side: u32) -> Option<Icon> {
        let picture = masked_as(bytes, side, true, true, button_bg(Gen::Dark))?;
        Some(Icon {
            handle: Handle::from_pixels(picture.side, picture.side, picture.pixels),
        })
    }

    /// `bytes` as `UserPageHeader.vue:4`'s avatar: [`Icon::circle`]'s card *with*
    /// `Avatar.vue:299`'s `box-shadow: var(--shadow-card)` behind it.
    ///
    /// The one avatar the reference does not pass `no-shadow` to, and the reason
    /// this is a separate constructor rather than a flag on [`Icon::circle`]: the
    /// shadow reaches [`SHADOW_REACH`] past the card, so the handle this hands
    /// back is `side + 2 * reach` across and the card sits at
    /// `(reach, reach - SHADOW_OFFSET_Y)` inside it. See [`with_card_shadow`]
    /// for the geometry and the five measured depths.
    ///
    /// # The caller has to lay the handle out at the canvas's own size
    ///
    /// Which is the whole of what is missing, and it is two lines in
    /// `pages/user.rs`, which is not this file's:
    ///
    /// ```text
    /// user.rs:544   Icon::circle(&picture, AVATAR as u32)
    ///             -> Icon::shadowed_circle(&picture, AVATAR as u32)
    /// user.rs:1202  .width(Length::Fixed(AVATAR))     ->  AVATAR + 2 * SHADOW_REACH
    ///               .height(Length::Fixed(AVATAR))    ->  AVATAR + 2 * SHADOW_REACH
    /// ```
    ///
    /// `iced_widget-0.12.3/src/image.rs:137` fits the texture to the layout box
    /// (`ContentFit::Contain`, `image.rs:50`) and `image.rs:104` takes the layout
    /// box from the `width`/`height` given here rather than from the texture, so
    /// a 102-pixel canvas in a 96-pixel slot is scaled to 94.1% and the disc goes
    /// with it. Measured on our own capture of `/user/FlameFire`, growing the
    /// canvas without the box draws the disc 90.35 across where the reference
    /// draws 96, and lays the shadow's five bands down *inside* the card -- at
    /// `x=136`, `y=163..167`, where the reference has the avatar's own picture.
    /// That is why the module docs call this a limit rather than a fix, and why
    /// the canvas does not grow in [`Icon::circle`] where `pages/servers.rs`
    /// would squeeze nine avatars that the reference draws no shadow under.
    ///
    /// The `side` passed here is still the *card's* side and not the canvas's, so
    /// the disc is the size the reference draws whatever the caller's box is.
    #[allow(dead_code, reason = "`pages/user.rs:544` is the only caller that wants this")]
    pub fn shadowed_circle(bytes: &[u8], side: u32) -> Option<Icon> {
        let card = masked_as(bytes, side, true, true, button_bg(Gen::Dark))?;
        let picture = with_card_shadow(card, Shadow::of(Gen::Dark));
        Some(Icon {
            handle: Handle::from_pixels(picture.side, picture.side, picture.pixels),
        })
    }

    /// The picture, as something an `image` widget draws.
    ///
    /// A clone of a handle, not of the pixels: what travels is the id and a
    /// reference to the bytes the renderer already holds.
    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }
}

/// One icon as it arrived: the URL it answers, and the picture.
///
/// The URL travels beside the picture because that is the only key a card can look
/// an icon up by: a search's hits each carry an `icon_url` and nothing else about
/// their icon, so an answer keyed by anything but the URL would have to be matched
/// to a hit a second time, by a rule the fetch did not use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The `icon_url` this answers.
    pub url: String,
    /// The picture behind it.
    pub icon: Icon,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG of one colour, in memory, the way `skin.rs`'s tests make one.
    fn picture(width: u32, height: u32) -> Vec<u8> {
        let square =
            image::RgbaImage::from_pixel(width, height, image::Rgba([200, 40, 90, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(square)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("a PNG in memory");
        bytes
    }

    /// One pixel of a [`Picture`], as RGBA.
    fn at(picture: &Picture, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * picture.side + x) * 4) as usize;
        [
            picture.pixels[index],
            picture.pixels[index + 1],
            picture.pixels[index + 2],
            picture.pixels[index + 3],
        ]
    }

    /// A PNG of one colour in an arbitrary colour, for the tests that assert a
    /// blend rather than a copy.
    fn painted(width: u32, height: u32, colour: [u8; 4]) -> Vec<u8> {
        let square = image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba(colour),
        );
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(square)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("a PNG in memory");
        bytes
    }

    #[test]
    fn the_radius_is_the_reference_s_share_of_the_size() {
        // `calc(16 / 96 * size)`: 16 at the 96px avatar it was written for, and
        // the same share at every other size. 100 is `ProjectCard.vue`'s list row.
        assert_eq!(radius(96), 16);
        assert_eq!(radius(100), 17);
        assert_eq!(radius(64), 11);
        // The radius is rounded to whole pixels, and a box small enough rounds to
        // none at all -- CSS would give 2px a 0.33px radius, which a rasteriser
        // antialiases and this cannot.
        assert_eq!(radius(4), 1);
        assert_eq!(radius(2), 0);
        assert_eq!(radius(0), 0);
        // And it cannot exceed half the box, or the shape is not a rectangle any
        // more and the mask's own arithmetic would be asked to cut a circle.
        assert_eq!(radius(3), 1);
    }

    #[test]
    fn the_mask_clears_the_corners_and_keeps_the_middle() {
        let side = 64;
        let mask = mask(side);
        assert_eq!(mask.len(), (side * side) as usize);
        let at = |x: u32, y: u32| mask[(y * side + x) as usize];
        // Every corner of the box is outside the rounded rectangle.
        for (x, y) in [(0, 0), (side - 1, 0), (0, side - 1), (side - 1, side - 1)] {
            assert!(!at(x, y), "the corner {x},{y} must be clear");
        }
        // The middle and the straight edges are inside it.
        assert!(at(side / 2, side / 2));
        assert!(at(side / 2, 0), "the middle of an edge is inside");
        assert!(at(0, side / 2));
        // Symmetric about both axes: a mask that is not is a rounding that leans.
        for y in 0..side {
            for x in 0..side {
                assert_eq!(at(x, y), at(side - 1 - x, y), "not mirrored in x at {x},{y}");
                assert_eq!(at(x, y), at(x, side - 1 - y), "not mirrored in y at {x},{y}");
            }
        }
        // And most of a rounded square is kept: the corners are a few percent.
        let kept = mask.iter().filter(|kept| **kept).count();
        let total = (side * side) as usize;
        assert!(kept * 100 / total >= 96, "the mask cut {kept} of {total} pixels");
    }

    #[test]
    fn a_square_picture_is_scaled_to_the_box_and_its_corners_are_cleared() {
        // A 40x40 icon -- smaller than the box, so `contain` scales it *up*, which
        // is what the reference does with it.
        let picture = masked(&picture(40, 40), ICON_SIDE).expect("a PNG must decode");
        assert_eq!(picture.side, ICON_SIDE);
        assert_eq!(picture.pixels.len(), (ICON_SIDE * ICON_SIDE * 4) as usize);
        // The corners are clear and the colour is the image's own, which is the
        // whole claim: a rounded icon is the uploaded pixel with a cut corner.
        assert_eq!(at(&picture, 0, 0)[3], 0, "the top-left corner is cleared");
        assert_eq!(
            at(&picture, ICON_SIDE - 1, ICON_SIDE - 1)[3],
            0,
            "the bottom-right corner is cleared"
        );
        assert_eq!(
            at(&picture, ICON_SIDE / 2, ICON_SIDE / 2),
            [200, 40, 90, 255],
            "the middle is the image's own pixel"
        );
        assert_eq!(
            at(&picture, ICON_SIDE / 2, 0),
            [200, 40, 90, 255],
            "and so is the middle of an edge"
        );
    }

    #[test]
    fn a_picture_that_is_not_square_is_letterboxed_rather_than_stretched() {
        // A wide banner used as an icon: 200 by 40 in a 100 box, so `contain` fills
        // the width and leaves a band above and below -- the aspect is 5:1, so the
        // picture is 100 wide and 20 tall, centred on rows 40..59.
        let picture = masked(&picture(200, 40), ICON_SIDE).expect("a PNG must decode");
        assert_eq!(picture.side, ICON_SIDE);
        let (top, bottom) = ((ICON_SIDE - 20) / 2, (ICON_SIDE - 20) / 2 + 20);
        assert_eq!(at(&picture, ICON_SIDE / 2, ICON_SIDE / 2), [200, 40, 90, 255]);
        // The letterbox is the element's own background on both sides of it --
        // `Avatar.vue:286`, and what a browser paints there -- and the picture
        // reaches the box's full width where it is.
        let background = {
            let [red, green, blue, _] = theme_gen::ink_rgba(Gen::Dark, Ink::ButtonBg);
            [red, green, blue, u8::MAX]
        };
        assert_eq!(at(&picture, ICON_SIDE / 2, 0), background, "the letterbox is the button");
        assert_eq!(at(&picture, ICON_SIDE / 2, top - 1), background);
        assert_eq!(at(&picture, ICON_SIDE / 2, top), [200, 40, 90, 255]);
        assert_eq!(at(&picture, ICON_SIDE / 2, bottom - 1), [200, 40, 90, 255]);
        assert_eq!(at(&picture, ICON_SIDE / 2, bottom), background);
        assert_eq!(at(&picture, 1, ICON_SIDE / 2), [200, 40, 90, 255]);
        // And the box's own corner is still cut, because CSS clips the element's
        // box rather than its content: the fill does not reach past the radius.
        assert_eq!(at(&picture, 0, 0)[3], 0);
    }

    /// `Avatar.vue:286`'s `background-color` is what a letterboxed avatar's gutter
    /// shows, and without it the disc is two pixels narrower than the reference's.
    ///
    /// The case is `/user/FlameFire`'s own avatar: 300x307 in a 96 box, which
    /// `contain` fits to 94x96 and leaves one column of gutter down each side. On
    /// a transparent canvas the circle mask then clears those, and the disc
    /// measures 94 across the centre row where the reference's measures 96.
    #[test]
    fn a_letterboxed_avatar_is_the_full_width_of_its_box() {
        let bytes = picture(300, 307);
        let disc = masked_as(&bytes, 96, true, false, button_bg(Gen::Dark)).expect("a PNG");
        let row = 48;
        // Every pixel of the centre row survives the mask, and the two at its ends
        // are the gutter -- filled, not clear.
        assert_eq!(at(&disc, 0, row), [52, 54, 60, 255], "the gutter is button-bg");
        assert_eq!(at(&disc, 95, row), [52, 54, 60, 255]);
        let painted = (0..96).filter(|x| at(&disc, *x, row)[3] != 0).count();
        assert_eq!(painted, 96, "the disc is the box's full width");
        // One column in, the picture's own pixels are there: 300 * 96/307 = 93.8,
        // so `contain` puts the picture at x=1..94 and the gutter at 0 and 95.
        assert_eq!(at(&disc, 1, row), [200, 40, 90, 255]);
        assert_eq!(at(&disc, 94, row), [200, 40, 90, 255]);
        // The fill is the theme's, not a constant this module made up: the light
        // look's `--surface-4` is `#ffffff`.
        let light = masked_as(&bytes, 96, true, false, button_bg(Gen::Light)).expect("a PNG");
        assert_eq!(at(&light, 0, row), [255, 255, 255, 255]);
        // And the disc is still a disc: the fill does not reach the corners.
        assert_eq!(at(&disc, 0, 0)[3], 0, "the corner is clear");
        assert_eq!(at(&disc, 95, 95)[3], 0);
    }

    /// `Avatar.vue:284`'s `outline`, which `outline-offset: -1px` puts *inside* the
    /// box and CSS clips to the radius -- so on a circle it is a ring.
    #[test]
    fn the_outline_is_a_one_pixel_ring_of_fifteen_percent_white() {
        // The reference's own flat colour across the top of this disc, so the blend
        // below is the one the capture was measured with.
        let art = [52, 58, 63, 255];
        let disc = masked_as(&painted(300, 307, art), 96, true, true, button_bg(Gen::Dark))
            .expect("a PNG");
        let row = 48;
        // The reference measures `#52575b` at the top of the disc's centre column,
        // which is 15% of white over the picture's `#343a3f` in the truncating
        // arithmetic Chromium uses; and `#4f5156` at the disc's leftmost column,
        // where its own gutter shows through the outline -- 3 units under the same
        // blend over our `#34363c` gutter, which is the reference compositing its
        // 1px ring a little less than full coverage over that column.
        assert_eq!(at(&disc, 0, row), [82, 83, 89, 255], "the ring over the gutter");
        assert_eq!(at(&disc, row, 0), [82, 87, 91, 255], "the ring over the picture");
        assert_eq!(at(&disc, 95, row), [82, 83, 89, 255]);
        assert_eq!(at(&disc, row, 95), [82, 87, 91, 255]);
        // One pixel in, the picture is untouched -- a ring, not a wash.
        assert_eq!(at(&disc, 1, row), art);
        assert_eq!(at(&disc, row, 1), art);
        assert_eq!(at(&disc, 47, row), art);
        // And the ring is the whole of it, and one pixel deep: the annulus is
        // pi*(48^2 - 47^2) = 298 pixels and the lattice of pixel centres inside it
        // is 290, because a circle rasterised on whole pixels is a staircase.
        // (The cleared corners are counted out of it -- they are not the ring.)
        let ringed = (0..96)
            .flat_map(|y| (0..96).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let pixel = at(&disc, *x, *y);
                pixel[3] != 0 && pixel != art
            })
            .count();
        assert!(
            (280..=316).contains(&ringed),
            "a 1px ring is ~298 pixels, not {ringed}"
        );
        // Symmetric, because a ring that leaned would be a lopsided disc.
        for y in 0..96 {
            for x in 0..96 {
                assert_eq!(
                    at(&disc, x, y),
                    at(&disc, 95 - x, y),
                    "the ring is not mirrored in x at {x},{y}"
                );
                assert_eq!(
                    at(&disc, x, y),
                    at(&disc, x, 95 - y),
                    "the ring is not mirrored in y at {x},{y}"
                );
            }
        }
        // The project-icon shape is not given a ring: that outline is the 16/96
        // rounded rectangle, which `ui::icon_box` draws as a container border.
        let square = masked(&painted(64, 64, art), ICON_SIDE).expect("a PNG");
        let colours: std::collections::BTreeSet<[u8; 4]> = (0..square.side)
            .flat_map(|y| (0..square.side).map(move |x| (x, y)))
            .map(|(x, y)| at(&square, x, y))
            .collect();
        assert_eq!(
            colours,
            [art, [52, 58, 63, 0]].into_iter().collect(),
            "a project icon is its own pixels and a corner cut to nothing"
        );
    }

    /// `Avatar.vue:299`'s `box-shadow: var(--shadow-card)` is drawn, and these
    /// are its numbers: the canvas it needs, where the card sits inside it, and
    /// what the shadow reads as at each of the five depths off it.
    ///
    /// Measured off `ref/user-ref.png`, the profile header's own avatar, which is
    /// a 96 box at `x=88..183`, `y=72..167` on the page's `#16181c` and casts a
    /// shadow five rows below its last row and three columns past either edge of
    /// its centre row. So a 102 canvas, the card at `(3, 5)` and five rings of
    /// pre-composited ink -- and every one of the fifteen is read back out of the
    /// canvas rather than recomputed, so this fails if a reach or a depth moves.
    #[test]
    fn the_card_carries_the_shadow_the_reference_casts() {
        let side = 96u32;
        let shadow = Shadow::of(Gen::Dark);
        let card =
            masked_as(&picture(64, 64), side, true, true, button_bg(Gen::Dark)).expect("a PNG");
        let canvas = with_card_shadow(card, shadow);

        // The canvas is the shadow's own footprint -- the card's box moved down
        // the offset and grown by the reach -- so the card's corner is a pixel
        // *below* its top edge, and that is what leaves the capture's single row
        // above the disc above rather than a fifth row below it below.
        let (grown, at_x, at_y) = shadow_canvas(side);
        assert_eq!(shadow_canvas(2), (8, 3, 1), "the same padding at 2px");
        assert_eq!(
            shadow_canvas(100),
            (106, 3, 1),
            "and at `ProjectCard`'s size"
        );
        assert_eq!(canvas.side, grown);
        assert_eq!((at_x, at_y), (3, 1));
        assert_eq!(canvas.pixels.len(), (grown * grown * 4) as usize);
        // 96 becomes 102: six more pixels of canvas than the old note's three
        // sigma of Gaussian wanted, and 4,896 more bytes of RGBA.
        assert_eq!(SHADOW_REACH, 3.0, "`blur / 2` is 2 and the capture's is 3");

        // The five rings' inks, darkest first, and the reach they cover: the
        // outermost ends at the reach and each ring is a pixel of it narrower.
        let inks: Vec<[u8; 4]> = (0..SHADOW_DEPTHS.len())
            .map(|ring| shadow.ink(ring))
            .collect();
        assert_eq!(
            inks,
            vec![
                [18, 20, 23, 255],
                [19, 21, 24, 255],
                [20, 22, 25, 255],
                [21, 23, 26, 255],
                [21, 23, 27, 255],
            ],
            "`ref/user-ref.png`, x=136 y=168..172 over the page's #16181c"
        );
        assert_eq!(ring_radius(side, 4), 51.0, "48 + the reach");
        assert_eq!(
            ring_radius(side, 0),
            47.0,
            "one pixel inside the card's edge"
        );
        // The shadow's centre is the card's own centre and the offset below it,
        // and the rings land one pixel further out per ring: the card's last row
        // is 45.5 from that centre, so they are at 46.5 .. 50.5.
        let centre = (at_x + side / 2, at_y + SHADOW_OFFSET_Y as u32 + side / 2);
        for ring in 0..SHADOW_DEPTHS.len() {
            let row = centre.1 + 46 + ring as u32;
            assert_eq!(
                at(&canvas, centre.0, row),
                inks[ring],
                "ring {ring} is {row} rows down, {} from the centre",
                row as f32 + 0.5 - centre.1 as f32
            );
        }

        // And the twelve pixels the reference's own capture reads around that
        // card, at the capture's coordinates: the card is at `x=88`, `y=72`
        // there, so the canvas's `(0, 0)` is the capture's `(85, 71)`.
        let measured: [(u32, u32, [u8; 3]); 12] = [
            (51, 97, [18, 20, 23]), // x=136 y=168, the first row below the disc
            (51, 98, [19, 21, 24]),
            (51, 99, [20, 22, 25]),
            (51, 100, [21, 23, 26]),
            (51, 101, [21, 23, 27]),
            (2, 53, [20, 22, 25]), // y=120 x=87 and x=184, either side
            (1, 53, [21, 23, 26]),
            (0, 53, [21, 23, 27]),
            (99, 53, [20, 22, 25]),
            (100, 53, [21, 23, 26]),
            (101, 53, [21, 23, 27]),
            (51, 0, [21, 23, 27]), // x=136 y=71, the one row above the disc
        ];
        for (x, y, ink) in measured {
            assert_eq!(
                at(&canvas, x, y),
                [ink[0], ink[1], ink[2], 255],
                "the capture reads {ink:?} at canvas ({x}, {y})"
            );
        }
        // The four corners of the footprint carry nothing, which is what says the
        // reach is three and not the blur's nominal two: the capture has the page's
        // own `#16181c` at `x=84` and at `y=173`, and at `(89, 67)` beside the one
        // row above the disc.
        for (x, y) in [(0u32, 0u32), (0, 4), (4, 0), (101, 101), (97, 101)] {
            assert_eq!(
                at(&canvas, x, y),
                [0, 0, 0, 0],
                "({x}, {y}) is clear in the capture and here"
            );
        }
    }

    /// The card's own pixels are the bytes [`masked_as`] wrote, copied in whole.
    ///
    /// Which is the claim that the shadow changes nothing inside the disc: the
    /// fill, the picture, the outline and the radius are all still there, at the
    /// same bytes, and only the canvas round them is new. So the two composites
    /// are compared over the card's own box rather than over the shadow's.
    #[test]
    fn the_shadow_adds_pixels_and_changes_none() {
        for side in [8u32, 48, 96] {
            let art = [52, 58, 63, 255];
            let card = masked_as(
                &painted(300, 307, art),
                side,
                true,
                true,
                button_bg(Gen::Dark),
            )
            .expect("a PNG");
            let shadowed = with_card_shadow(card.clone(), Shadow::of(Gen::Dark));
            let (grown, at_x, at_y) = shadow_canvas(side);
            for y in 0..side {
                for x in 0..side {
                    assert_eq!(
                        at(&shadowed, x + at_x, y + at_y),
                        at(&card, x, y),
                        "the card moved at {x},{y} of a {side}px disc"
                    );
                }
            }
            // The disc's own corners are still clear, so the canvas has not filled
            // in behind them, and the shadow has not leaked past the reach.
            assert_eq!(at(&shadowed, at_x, at_y)[3], 0, "the corner is clear");
            assert_eq!(at(&shadowed, at_x + side - 1, at_y + side - 1)[3], 0);
            assert_eq!(at(&shadowed, 0, 0)[3], 0, "and nothing before the reach");
            assert_eq!(at(&shadowed, grown - 1, grown - 1)[3], 0);
            // Everything the shadow did land in is one of its own five inks.
            let inks: std::collections::BTreeSet<[u8; 4]> = (0..SHADOW_DEPTHS.len())
                .map(|r| Shadow::of(Gen::Dark).ink(r))
                .collect();
            let outside: std::collections::BTreeSet<[u8; 4]> = (0..grown)
                .flat_map(|y| (0..grown).map(move |x| (x, y)))
                .filter(|(x, y)| {
                    let inside = *x >= at_x && *x < at_x + side && *y >= at_y && *y < at_y + side;
                    !inside && at(&shadowed, *x, *y)[3] != 0
                })
                .map(|(x, y)| at(&shadowed, x, y))
                .collect();
            assert!(!outside.is_empty(), "a {side}px disc casts something");
            assert!(outside.is_subset(&inks), "and casts only its own rings");
        }
    }

    /// The shadow's own reach is outside the card's box, so the two constructors
    /// hand back handles of different sizes -- and that difference is the whole
    /// reason they are two constructors and not one flag.
    #[test]
    fn only_the_shadowed_circle_grows_its_canvas() {
        for side in [24u32, 36, 96] {
            let bytes = picture(64, 64);
            let plain = masked_as(&bytes, side, true, true, button_bg(Gen::Dark)).expect("a PNG");
            let shadowed = with_card_shadow(plain.clone(), Shadow::of(Gen::Dark));
            assert_eq!(plain.side, side, "the friends list's {side}px avatar");
            assert_eq!(shadowed.side, side + 6, "and the header's {side}px card");
            // The card is the same bytes either way; only the canvas round it is
            // new, and the disc lands at (3, 1) rather than at the origin.
            for y in 0..side {
                for x in 0..side {
                    assert_eq!(at(&shadowed, x + 3, y + 1), at(&plain, x, y));
                }
            }
        }
        // And the reference's own sizes are the two kinds this crate draws: eight
        // friends at `size="1.5rem"` round (`ServerListEmptyPreview.vue:47`),
        // Geometrically at `2.25rem` (`:100`) and the profile header at 96
        // (`UserPageHeader.vue:8`). Only the last one asks for a shadow.
        assert_eq!(
            shadow_canvas(24).0,
            30,
            "a friend would draw 30 if it had one"
        );
        assert_eq!(
            shadow_canvas(36).0,
            42,
            "the toast would draw 42 if it had one"
        );
        assert_eq!(shadow_canvas(96).0, 102, "the header's own canvas is 102");
    }

    /// `ProjectCard.vue:31` asks for `no-shadow` on a project icon, so
    /// [`Icon::of`]'s canvas is the box and nothing else -- which is what keeps
    /// the rail, the sidebar and a project's own cards free of a ring around a
    /// twenty-pixel avatar.
    #[test]
    fn a_project_icon_holds_its_box_and_no_shadow() {
        for side in [2u32, 8, 48, 96, 100] {
            let icon = masked(&picture(64, 64), side).expect("a PNG");
            assert_eq!(icon.side, side, "a {side}px icon");
            assert_eq!(
                icon.pixels.len(),
                (side * side * 4) as usize,
                "a {side}px icon holds {side}px of picture and no shadow around it"
            );
            // `ui::icon_box` lays the handle out at `side` too, so a project icon
            // is never squeezed by a shadow it was never given.
            assert!(
                Icon::of(&picture(64, 64), side)
                    .map(|icon| icon.handle().id())
                    .is_some(),
                "a {side}px icon has a handle"
            );
        }
    }

    /// The light look's `--shadow-card` (`variables.scss:140`) is the same shape
    /// with a tint and a tenth of the alpha, so the same geometry over the light
    /// page -- and the one shadow in this file that is *not* measured off a
    /// capture, because nothing in this crate paints the light look. It is
    /// derived from the token, and it is worth pinning because it says how little
    /// the reference's own light look would show: the deepest ring lands twelve
    /// units under the page and the faintest two.
    #[test]
    fn the_light_look_shadow_is_the_same_shape_over_a_tint() {
        let dark = Shadow::of(Gen::Dark);
        let light = Shadow::of(Gen::Light);
        assert_eq!(dark.page, [0x16, 0x18, 0x1c], "--surface-1 under a profile");
        assert_eq!(
            light.page,
            [0xeb, 0xeb, 0xeb],
            "and its own light equivalent"
        );
        assert_eq!(dark.alpha, 0.25, "`rgba(0, 0, 0, 0.25)`");
        assert_eq!(light.alpha, 0.1, "`rgba(50, 50, 100, 0.1)`");
        // `rgba(50, 50, 100, 0.1)` over `#ebebeb`: `235 * (1 - a) + tint * a`
        // truncated, which is 12 units down at the deepest ring and 2 at the
        // faintest, and blue is a unit under the other two throughout because the
        // tint's own blue is `100` and not `50`.
        assert_eq!(
            (0..SHADOW_DEPTHS.len())
                .map(|ring| light.ink(ring))
                .collect::<Vec<_>>(),
            vec![
                [223, 223, 226, 255],
                [226, 226, 228, 255],
                [229, 229, 230, 255],
                [232, 232, 232, 255],
                [233, 233, 234, 255],
            ]
        );
        // The geometry does not move with the ink: same canvas, same card corner,
        // same five radii, because both tokens declare the same `0px 2px 4px`.
        assert_eq!(shadow_canvas(96), (102, 3, 1));
        assert_eq!(ring_radius(96, 4), 51.0);
    }

    #[test]
    fn bytes_that_are_not_an_image_are_refused() {
        assert!(masked(b"not a png at all", ICON_SIDE).is_none());
        assert!(masked(&[], ICON_SIDE).is_none());
        assert!(masked(b"anything", 0).is_none());
        assert!(Icon::of(b"not a png at all", ICON_SIDE).is_none());
        assert!(Icon::of(&[], ICON_SIDE).is_none());
        assert!(Icon::of(&picture(64, 64), 0).is_none());
    }

    #[test]
    fn every_source_lands_on_the_box_the_card_asked_for() {
        // The composite is the box, not the picture: a 16px icon, a 64px one and a
        // 512px one are all drawn at the 100px the card asked for.
        for source in [16u32, 64, 512] {
            let bytes = picture(source, source);
            let picture = masked(&bytes, ICON_SIDE).expect("a PNG");
            assert_eq!(picture.side, ICON_SIDE, "a {source}px source");
            assert_eq!(picture.pixels.len(), (ICON_SIDE * ICON_SIDE * 4) as usize);
            // And the handle a card draws is that same picture.
            drop(Icon::of(&bytes, ICON_SIDE).expect("a PNG").handle());
        }
    }

    #[test]
    fn a_tiny_icon_is_scaled_as_blocks_rather_than_as_a_blur() {
        // `image-rendering: pixelated`, which `Avatar.vue` turns on for a source
        // narrower than 32 pixels: a 16px icon keeps its own pixels, so the canvas
        // has exactly one colour in it.
        let tiny = masked(&picture(16, 16), ICON_SIDE).expect("a PNG");
        assert_eq!(at(&tiny, ICON_SIDE / 2, ICON_SIDE / 2), [200, 40, 90, 255]);
        // Nearest-neighbour scaling of a one-colour source cannot have invented a
        // second colour; a smooth filter at a 6x scale would have moved the edge
        // pixels, and this is the assertion that notices.
        let colours: std::collections::BTreeSet<[u8; 4]> = (0..tiny.side)
            .flat_map(|y| (0..tiny.side).map(move |x| (x, y)))
            .map(|(x, y)| at(&tiny, x, y))
            .collect();
        assert_eq!(colours.len(), 2, "the icon's own colour and the cleared corners");
        // The same picture is what a card is handed, and the handle it draws with.
        let icon = Icon::of(&picture(16, 16), ICON_SIDE).expect("a PNG");
        drop(icon.handle());
    }
}
