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
//! * **No box-shadow.** `Avatar.vue:299` gives every avatar that is not
//!   `.no-shadow` a `box-shadow: var(--shadow-card)`, and a profile's header
//!   avatar is one -- but the shadow is *behind* the picture, and iced cannot
//!   paint one behind an `image` widget. It could be drawn into a canvas grown to
//!   hold it, and the arithmetic says not to: see [`masked_as`] for the pixels
//!   that buys (a 96 box becomes a 108x108 canvas, +2,448 pixels and +9.8KB per
//!   avatar) against what it costs (a 1.6% dip in one channel over five rows).
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
/// # Why there is no shadow
///
/// `Avatar.vue:299`'s `box-shadow: var(--shadow-card)` is the one rule of the
/// component's left undrawn, and the arithmetic is the reason rather than the
/// toolkit being asked for too little. The token is
/// `rgba(0, 0, 0, 0.25) 0px 2px 4px 0px` (`variables.scss:368`): a Gaussian of
/// `blur/2 = 2px` sigma, shifted down 2, so painting it into a bitmap means
/// growing the canvas by the offset plus three sigma in every direction a blurred
/// edge can reach -- 96 becomes 108x108, which is 11,664 pixels against 9,216, or
/// +2,448 pixels and +9.8KB of RGBA for every avatar this crate decodes. What it
/// buys is measured on the reference: rows y=168..172 under a disc whose last row
/// is y=167, going from the page's `#16181c` through `#121417`, `#131518`,
/// `#141619` and `#15171b` back to `#16181c` -- a peak alpha of 0.25 (which is the
/// token's own) and a largest 8-bit step of 4/255 = 1.6% in red. And it is not
/// free of layout: the callers draw a bare `image` at a fixed side inside a
/// `row`, so a 108x108 handle in a 96 slot would stretch the disc to 112.5% and
/// the row would grow by 12x8 -- a change in `pages/user.rs` and `pages/servers.rs`,
/// which are not this file. A 1.6% dip over five rows is not worth either.
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
    pub fn circle(bytes: &[u8], side: u32) -> Option<Icon> {
        let picture = masked_as(bytes, side, true, true, button_bg(Gen::Dark))?;
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

    /// The box-shadow is deliberately not drawn, and this is what holds that line:
    /// the canvas is the box and nothing else, so there is nowhere in it for a
    /// shadow's blur to reach.
    ///
    /// `--shadow-card` is `rgba(0, 0, 0, 0.25) 0px 2px 4px 0px`
    /// (`variables.scss:368`), so a bitmap of it needs the box plus the 2px offset
    /// plus three times the 2px sigma in each direction: 108x108, 2,448 pixels and
    /// 9.8KB more per avatar than the 9,216 it has. What it buys is a 0.25 peak
    /// alpha spread over five rows, the largest step 4/255 = 1.6% -- and a caller
    /// change in a file this one does not own, since a 108x108 handle in a 96 slot
    /// stretches the disc by 12.5%. See [`masked_as`].
    #[test]
    fn the_canvas_is_the_box_and_never_the_box_plus_a_shadow() {
        for side in [2u32, 8, 48, 96, 100] {
            let disc = masked_as(&picture(64, 64), side, true, true, button_bg(Gen::Dark))
                .expect("a PNG");
            assert_eq!(disc.side, side, "a {side}px avatar");
            assert_eq!(
                disc.pixels.len(),
                (side * side * 4) as usize,
                "a {side}px avatar holds {side}px of picture and no shadow around it"
            );
        }
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
