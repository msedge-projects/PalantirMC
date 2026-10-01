//! The reference's `Avatar`: a project's icon, fitted into its box and rounded.
//!
//! A search hands out an `icon_url` per hit and the bytes behind it are a PNG
//! somebody uploaded. `Avatar.vue` puts those bytes in an `<img>` and rounds the
//! box with CSS:
//!
//! ```css
//! border-radius: calc(16 / 96 * var(--_override-size, var(--_size)));
//! object-fit: contain;
//! ```
//!
//! Two rules, and both are done here on the pixels rather than asked of the
//! toolkit. The radius is *proportional to the drawn size*, so the corner reads
//! the same at a 100px row icon and at a 64px one; and iced has no rounded image
//! -- a `container`'s border radius paints the container's own background and does
//! not clip what is drawn over it, so a rounded frame around a square image shows
//! square corners poking out of the frame's corners. So the picture is composited
//! onto a transparent canvas the size of the box, and the alpha outside the
//! rounded rectangle is cleared: [`crate::ui::icon_box`] then draws a handle that
//! already has the shape, over the box's own background and hairline.
//!
//! Because the canvas is the box, the three things `Avatar.vue` gets from CSS all
//! fall out of the one composite:
//!
//! * **`contain`**, so a picture that is not square is letterboxed and centred
//!   inside the box rather than stretched, and a picture *smaller* than the box is
//!   scaled up to it, which is what a browser does with `object-fit: contain`.
//! * **The corner clip**, which CSS applies to the element's box -- so the mask is
//!   the box's, and a letterboxed picture is clipped where it reaches a corner
//!   exactly as the reference clips it.
//! * **`image-rendering: pixelated`**, which `Avatar.vue` turns on when
//!   `naturalWidth < 32`: a tiny icon is scaled with nearest-neighbour and reads
//!   as blocks rather than as a blur.
//!
//! Three deliberate limits, each named rather than approximated:
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
//!
//! One thing this cannot do at all: an image whose bytes are not an image this
//! build's `image` crate reads is refused rather than drawn as a grey square, so a
//! broken icon is a missing icon.

use iced::widget::image::Handle;

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
fn masked(bytes: &[u8], side: u32) -> Option<Picture> {
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
    let mut canvas = image::RgbaImage::from_pixel(side, side, image::Rgba([0, 0, 0, 0]));
    let (width, height) = (fitted.width(), fitted.height());
    // `replace` rather than `overlay`: the composite is a copy, and an alpha
    // blend would multiply a translucent icon against the transparent canvas and
    // change the pixels it came with.
    image::imageops::replace(
        &mut canvas,
        &fitted,
        ((side - width) / 2) as i64,
        ((side - height) / 2) as i64,
    );
    let kept = mask(side);
    let mut pixels = canvas.into_raw();
    for (index, kept) in kept.iter().enumerate() {
        if !kept {
            pixels[index * 4 + 3] = 0;
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
#[derive(Debug, Clone)]
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
    pub fn of(bytes: &[u8], side: u32) -> Option<Icon> {
        let picture = masked(bytes, side)?;
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
#[derive(Debug, Clone)]
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
        // The letterbox is transparent on both sides of it, and the picture reaches
        // the box's full width where it is.
        assert_eq!(at(&picture, ICON_SIDE / 2, 0)[3], 0, "the letterbox is clear");
        assert_eq!(at(&picture, ICON_SIDE / 2, top - 1)[3], 0);
        assert_eq!(at(&picture, ICON_SIDE / 2, top), [200, 40, 90, 255]);
        assert_eq!(at(&picture, ICON_SIDE / 2, bottom - 1), [200, 40, 90, 255]);
        assert_eq!(at(&picture, ICON_SIDE / 2, bottom)[3], 0);
        assert_eq!(at(&picture, 1, ICON_SIDE / 2), [200, 40, 90, 255]);
        // And the box's own corner is still cut, because CSS clips the element's
        // box rather than its content.
        assert_eq!(at(&picture, 0, 0)[3], 0);
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
