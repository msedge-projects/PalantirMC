//! A Minecraft skin, drawn as the picture this launcher can paint.
//!
//! The reference draws the account's skin through a renderer of its own: a 3D
//! model, lit and turning, drawn off-screen by a Tauri plugin whose Rust is not in
//! this tree. What the *texture* is, though, is a PNG on `textures.minecraft.net`
//! in a layout Minecraft documents: every body part is a rectangle at a known
//! address, and the front of each part is cut from a known corner of its own box.
//!
//! So this module does the arithmetic -- the five parts of a front view are cut out
//! of the texture and laid beside each other, which is what a paper doll is -- and
//! three things are deliberately *not* drawn, each named rather than approximated:
//!
//! * **No 3D, no rotation, no lighting.** An [`iced::widget::image`] is a flat
//!   rectangle; the reference's model turns, and a reader comparing the two sees a
//!   doll against a turntable.
//! * **No second layer.** A modern skin's hat, jacket, sleeves and trousers are a
//!   second copy of the same parts at a different address, drawn over the first
//!   with alpha. They are not drawn here, so a skin whose look lives in its overlay
//!   is drawn bare.
//! * **Four-pixel arms for both variants.** A `SLIM` skin's arms are three pixels
//!   wide and its doll is a pixel narrower; mixing the two widths in one picture is
//!   more arithmetic than the page needs, and the departure is one pixel a side.
//!
//! A legacy texture -- 64x32, the shape every skin made before 1.8 has -- is read
//! by mirroring the right limbs into the left slots, which is what Minecraft itself
//! does with one arm's and one leg's worth of pixels.
//!
//! The same module holds the other direction: [`prepare`] turns a texture the reader
//! picked into the 64x64 PNG the upload service demands, padding a legacy one by
//! *writing* the left limbs' boxes from the right ones rather than reading them
//! mirrored. The two are the same claim about the format from either end, and a test
//! holds them together: the front view cut from a legacy texture is the front view cut
//! from the normalised one, pixel for pixel.

use iced::widget::image::Handle;
use palantir_net::{MinecraftCape, MinecraftSkin, MinecraftSkins};

/// The texture's width, which every version of the format shares.
pub const TEXTURE_WIDTH: u32 = 64;

/// A modern skin's height: the original parts, plus the extension that holds the
/// mirrored halves of everything and the second layer.
pub const TEXTURE_HEIGHT: u32 = 64;

/// A legacy skin's height: the original parts and nothing else.
pub const LEGACY_HEIGHT: u32 = 32;

/// The front view's width: a 4-pixel arm, an 8-pixel body, and the other arm.
pub const FRONT_WIDTH: u32 = 16;

/// The front view's height: the head's 8, the body's 12 and the legs' 12.
pub const FRONT_HEIGHT: u32 = 32;

/// One part of the front view, and where its pixels come from.
struct Part {
    /// The front's own corner in the texture: `(x, y)`.
    from: (u32, u32),
    /// Where the part lands in the view.
    to: (u32, u32),
    /// Its size, which is the same in both: `(width, height)`.
    size: (u32, u32),
    /// The part whose pixels stand in for this one in a legacy texture, as an index
    /// into [`PARTS`]. A 64x32 skin has no left arm and no left leg -- Minecraft
    /// draws both sides from the right limb's pixels, mirrored -- and this is how
    /// that is said here.
    legacy_stand_in: Option<usize>,
}

/// The six parts of the front view, in the texture's own order.
///
/// The addresses are the format's: the head's front face is the 8x8 box at
/// `(8, 8)`, the body's is the 8x12 at `(20, 20)`, the right arm's is the 4x12 at
/// `(44, 20)` and the right leg's is the 4x12 at `(4, 20)`. Each one is the *first*
/// thing in its own box, which is what "the front" means in Minecraft's layout --
/// the sides and the back follow it. The left arm and the left leg only exist in
/// the 64x64 extension, at `(36, 52)` and `(20, 52)`.
const PARTS: [Part; 6] = [
    Part { from: (8, 8), to: (4, 0), size: (8, 8), legacy_stand_in: None },
    Part { from: (44, 20), to: (0, 8), size: (4, 12), legacy_stand_in: None },
    Part { from: (20, 20), to: (4, 8), size: (8, 12), legacy_stand_in: None },
    Part { from: (36, 52), to: (12, 8), size: (4, 12), legacy_stand_in: Some(1) },
    Part { from: (4, 20), to: (4, 20), size: (4, 12), legacy_stand_in: None },
    Part { from: (20, 52), to: (8, 20), size: (4, 12), legacy_stand_in: Some(4) },
];

/// A skin's front view: the pixels, and the size they are laid out in.
///
/// The pixels are RGBA8, row by row from the top left, which is what
/// [`Handle::from_pixels`] takes. RGBA rather than RGB because a skin's texture is
/// partly transparent -- the gaps in a legacy layout, the unused corners of a
/// modern one -- and painting those black would put a black box around the doll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontView {
    /// [`FRONT_WIDTH`], and only here so the picture carries its own size.
    pub width: u32,
    /// [`FRONT_HEIGHT`].
    pub height: u32,
    /// `width * height * 4` bytes of RGBA.
    pub pixels: Vec<u8>,
}

impl FrontView {
    /// The view as something an `image` widget can draw.
    pub fn handle(&self) -> Handle {
        Handle::from_pixels(self.width, self.height, self.pixels.clone())
    }
}

/// Cut `texture` into the front view the Skins page draws.
///
/// `None` when the bytes are not an image, or are not a texture of either shape
/// Minecraft publishes: a skin that cannot be cut into its parts is a skin this
/// launcher cannot draw, and the page says so rather than drawing part of a
/// player. Everything the format *does* define is drawn -- including the parts a
/// legacy skin keeps no pixels for.
pub fn front_view(texture: &[u8]) -> Option<FrontView> {
    let decoded = image::load_from_memory(texture).ok()?.to_rgba8();
    let (width, height) = (decoded.width(), decoded.height());
    if width != TEXTURE_WIDTH || (height != TEXTURE_HEIGHT && height != LEGACY_HEIGHT) {
        return None;
    }
    let legacy = height == LEGACY_HEIGHT;
    let mut pixels = vec![0u8; (FRONT_WIDTH * FRONT_HEIGHT * 4) as usize];
    for part in &PARTS {
        let mirrored = legacy && part.legacy_stand_in.is_some();
        let source = if mirrored {
            // Safe by the table above: a stand-in is an index into it.
            PARTS.get(part.legacy_stand_in?).unwrap_or(part).from
        } else {
            part.from
        };
        blit(&decoded, &mut pixels, source, part.to, part.size, mirrored);
    }
    Some(FrontView { width: FRONT_WIDTH, height: FRONT_HEIGHT, pixels })
}

/// Copy one part's front from the texture into the view.
///
/// `mirrored` flips the part left to right as it is copied, which is the whole of
/// what a legacy skin's missing limbs need: the pixels are the right limb's own,
/// seen from the other side. Nothing here reads outside the texture: a part that
/// would is skipped, because a texture whose extension is shorter than the columns
/// this reads is one Minecraft would refuse too.
fn blit(
    texture: &image::RgbaImage,
    view: &mut [u8],
    from: (u32, u32),
    to: (u32, u32),
    size: (u32, u32),
    mirrored: bool,
) {
    let (source_x, source_y) = from;
    let (target_x, target_y) = to;
    let (part_width, part_height) = size;
    for row in 0..part_height {
        for column in 0..part_width {
            let x = source_x + if mirrored { part_width - 1 - column } else { column };
            let y = source_y + row;
            if x >= texture.width() || y >= texture.height() {
                continue;
            }
            let pixel = texture.get_pixel(x, y).0;
            let view_x = target_x + column;
            let view_y = target_y + row;
            let at = ((view_y * FRONT_WIDTH + view_x) * 4) as usize;
            if at + 4 > view.len() {
                continue;
            }
            view[at..at + 4].copy_from_slice(&pixel);
        }
    }
}

// ---- what an upload sends --------------------------------------------------

/// Which arm style a texture is drawn with.
///
/// Two, because Minecraft has two: the arm is four pixels wide or three. The
/// document spells them `CLASSIC` and `SLIM`, and the reference's own modal calls
/// them the wide and the slim arm style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// Four-pixel arms: the original shape, and what Minecraft falls back to.
    Classic,
    /// Three-pixel arms.
    Slim,
}

impl Model {
    /// The document's own word for this arm style.
    ///
    /// Capitalised as the profile document writes it; the service takes it
    /// lower-cased, and that is the wire's business rather than this type's
    /// (`palantir_net::skin_upload_body` lower-cases it on the way out).
    pub const fn variant(self) -> &'static str {
        match self {
            Model::Classic => "CLASSIC",
            Model::Slim => "SLIM",
        }
    }
}

/// A texture that is ready to upload: the modern PNG, and the model it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    /// The arm style the texture is drawn with.
    pub model: Model,
    /// A 64x64 PNG's bytes, which is the only shape the upload service takes.
    pub png: Vec<u8>,
}

/// One limb box: sixteen pixels on a side, four faces of four columns each.
///
/// The four-wide face is the classic arm's -- see [`MODEL_COLUMN`] -- and it is the
/// only one the fill below has to handle, because the fill runs only on a legacy
/// texture and the legacy format has no slim variant at all: the three-pixel arm
/// arrived in the same update as this layout did.
const LIMB_BOX: u32 = 16;
const FACE: u32 = 4;

/// One left limb's fill: the box to read, and the box to write it into.
///
/// A struct rather than a pair of pairs for the reason `Part` above is one: two
/// corners of the same shape read as a type puzzle as a tuple and as a sentence with
/// names.
struct LimbFill {
    /// The right limb's box corner, `(x, y)`, which is what is read.
    from: (u32, u32),
    /// The left limb's box corner, which is where it is written.
    to: (u32, u32),
}

/// The two boxes a legacy texture has nothing in: the left limbs, and the right
/// limb's own box each one is filled from.
///
/// The addresses are the format's, and they are the same four the drawing half cuts
/// from: the right leg's box is at `(0, 16)` and the left leg's at `(16, 48)`; the
/// right arm's is at `(40, 16)` and the left arm's at `(32, 48)`.
///
/// A whole box rather than a rectangle of pixels, because the fill is a per-face
/// mirror and the face boundaries are what the box's sixteen columns are divided
/// into. Filling the box as one flipped rectangle instead -- one flip of all sixteen
/// columns -- would put the right arm's *right* face where its front belongs, which is
/// a player whose left arm shows the wrong side of itself.
const LIMB_BOXES: [LimbFill; 2] = [
    LimbFill { from: (0, 16), to: (16, 48) },
    LimbFill { from: (40, 16), to: (32, 48) },
];

/// The rectangle the arm style is read from: `(x, y, width, height)`.
///
/// The reference's own `determineModelType` asks its question of exactly these
/// pixels, and they are the last two columns of the right arm's back face: a classic
/// arm's faces are four pixels wide, so those columns are skin, while a slim arm's
/// are three -- its right face is at 40, its front at 44, its left at 47 and its back
/// at 51 -- so the box's last two columns are padding in one texture and paint in the
/// other. Read from pixels rather than from a document because the file the reader
/// just picked has no document: nothing in a PNG says which model it is.
const MODEL_COLUMN: (u32, u32, u32, u32) = (54, 20, 2, 12);

/// Read the model and the modern form out of one texture.
///
/// The one call the upload path makes of this module, and it is one call rather than
/// the reference's two (`normalize_skin_texture` and `determineModelType`) for a
/// reason worth naming: the decode is the expensive part, both answers come out of the
/// same image, and a second public function that only this module's tests called would
/// be dead code in the binary -- which is a warning this tree counts, and a sign that
/// the function has no caller rather than that it is useful. It is still the
/// reference's normaliser: a 64x64 texture comes back with its pixels where they were,
/// and a legacy 64x32 one comes back padded, which is the whole of what this does to
/// the file the reader picked. Both answers are `None` together when the bytes are not
/// a texture of either shape, because a file this launcher cannot read is not a file it
/// can say the model of either.
pub fn prepare(texture: &[u8]) -> Option<Prepared> {
    let modern = to_modern(texture)?;
    // Read before the encode consumes the image. Either shape answers the same:
    // padding a legacy texture writes only the two limb boxes, and the rectangle the
    // model comes from is in the right arm's box.
    let model = model_of(&modern);
    let png = encode(modern)?;
    Some(Prepared { model, png })
}

/// The modern image a texture is: itself, or a legacy one padded into the modern
/// layout.
///
/// Padding is not decoration. The modern layout's left arm and left leg boxes have no
/// legacy counterpart at all, and a 64x64 canvas with them left transparent is a
/// player with two right limbs and nothing on the left -- so the boxes are filled
/// from the right limbs, and the fill is what makes the two layouts describe the same
/// player (see [`fill_limb`]).
fn to_modern(texture: &[u8]) -> Option<image::RgbaImage> {
    let decoded = image::load_from_memory(texture).ok()?.to_rgba8();
    let (width, height) = (decoded.width(), decoded.height());
    if width != TEXTURE_WIDTH || (height != TEXTURE_HEIGHT && height != LEGACY_HEIGHT) {
        return None;
    }
    if height == TEXTURE_HEIGHT {
        return Some(decoded);
    }
    // A legacy texture is the modern layout's top half, and the head, the body and the
    // right limbs are at the same addresses in both: the padding is a copy rather than
    // a rearrangement, and everything the modern layout adds beyond it starts
    // transparent.
    let mut modern =
        image::RgbaImage::from_pixel(TEXTURE_WIDTH, TEXTURE_HEIGHT, image::Rgba([0, 0, 0, 0]));
    image::imageops::replace(&mut modern, &decoded, 0, 0);
    for fill in LIMB_BOXES {
        fill_limb(&mut modern, fill.from, fill.to);
    }
    Some(modern)
}

/// Fill one left limb box from the right limb's own box, each of its four faces
/// mirrored in place.
///
/// In place rather than across the box, for [`LIMB_BOXES`]' reason: a face keeps its
/// own four columns and reverses within them, so the front stays the front and its
/// pixels are the mirror of the right limb's front. That is what the mirrored left
/// limb the format describes *is*, and it is the same claim `front_view`'s legacy
/// stand-ins make from the other direction -- which is why a test can hold the two
/// together (`a_normalised_legacy_texture_draws_the_same_doll_as_the_legacy_one`).
fn fill_limb(canvas: &mut image::RgbaImage, from: (u32, u32), to: (u32, u32)) {
    for face in 0..4 {
        for row in 0..LIMB_BOX {
            for column in 0..FACE {
                // The source is a right limb's box in a texture this function has
                // already accepted, so it is in bounds by construction: the legs sit
                // at rows 16..32, which even the legacy height has.
                let source = *canvas.get_pixel(from.0 + face * FACE + column, from.1 + row);
                let at = (to.0 + face * FACE + (FACE - 1 - column), to.1 + row);
                if at.0 >= canvas.width() || at.1 >= canvas.height() {
                    continue;
                }
                canvas.put_pixel(at.0, at.1, source);
            }
        }
    }
}

/// Whether the two columns the arm style lives in carry any paint at all.
///
/// `MODEL_COLUMN`'s rule, and `CLASSIC` whenever the rectangle cannot be read -- a
/// texture whose bytes stop short of it is one this launcher would not have accepted,
/// and the format's default is the safer answer of the two.
fn model_of(image: &image::RgbaImage) -> Model {
    let (x, y, width, height) = MODEL_COLUMN;
    for row in y..y + height {
        for column in x..x + width {
            if column >= image.width() || row >= image.height() {
                return Model::Classic;
            }
            if image.get_pixel(column, row).0[3] != 0 {
                return Model::Classic;
            }
        }
    }
    Model::Slim
}

/// The PNG bytes of an image, for the wire.
fn encode(image: image::RgbaImage) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .ok()?;
    Some(bytes)
}

/// The account's own appearance, ready for the Skins page to draw.
///
/// Three things from two reads and one cut: Minecraft's document for the name and
/// the two lists, the texture of the skin in force over the engine, and
/// [`front_view`] for the picture. The fetch and the cut can fail while the lists
/// are fine -- an account that owns skins still owns them on a machine with no
/// connection -- so a reason travels *beside* the picture rather than replacing the
/// whole answer, which is the difference between a page that draws what it has and
/// one that draws nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appearance {
    /// The name Minecraft knows the account by.
    pub username: String,
    /// The skins it owns, in the order Minecraft lists them.
    pub skins: Vec<MinecraftSkin>,
    /// The capes it owns.
    pub capes: Vec<MinecraftCape>,
    /// The front view of the skin in force, when one could be drawn.
    pub front: Option<FrontView>,
    /// Why there is no front view, when there is none.
    pub note: Option<String>,
}

impl Appearance {
    /// Put an appearance together from the document and the texture's own answer.
    ///
    /// This is the whole seam between the network and the page: `texture` is
    /// whatever came back for the skin in force -- the PNG's bytes, or the sentence
    /// explaining why there are none -- and what the page draws is decided here.
    /// That is what makes the page's half of this testable without a service: a test
    /// hands it the bytes a texture *would* be.
    pub fn of(
        username: impl Into<String>,
        owned: MinecraftSkins,
        texture: Result<Vec<u8>, String>,
    ) -> Appearance {
        let (front, note) = match texture {
            Ok(bytes) => match front_view(&bytes) {
                Some(view) => (Some(view), None),
                None => (
                    None,
                    Some("The skin in force is not a texture this launcher can draw.".to_string()),
                ),
            },
            Err(reason) => (None, Some(reason)),
        };
        Appearance {
            username: username.into(),
            skins: owned.skins,
            capes: owned.capes,
            front,
            note,
        }
    }

    /// The skin the account is wearing, if the document named one.
    pub fn equipped(&self) -> Option<&MinecraftSkin> {
        self.skins.iter().find(|skin| skin.equipped())
    }

    /// The cape it is wearing, if it is wearing one.
    pub fn equipped_cape(&self) -> Option<&MinecraftCape> {
        self.capes.iter().find(|cape| cape.equipped())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One rectangle of a test texture: its corner, its size, and the colour to
    /// paint it -- a struct rather than a tuple of tuples because the tuple reads as
    /// a type puzzle rather than as a rectangle.
    struct Paint {
        from: (u32, u32),
        size: (u32, u32),
        colour: [u8; 4],
    }

    /// The six source rectangles, in the order [`PARTS`] lists them, each painted
    /// its own colour so a pixel says which part it came from.
    const PAINTS: [Paint; 6] = [
        Paint { from: (8, 8), size: (8, 8), colour: [255, 0, 0, 255] },
        Paint { from: (44, 20), size: (4, 12), colour: [0, 255, 0, 255] },
        Paint { from: (20, 20), size: (8, 12), colour: [0, 0, 255, 255] },
        Paint { from: (36, 52), size: (4, 12), colour: [255, 255, 0, 255] },
        Paint { from: (4, 20), size: (4, 12), colour: [255, 0, 255, 255] },
        Paint { from: (20, 52), size: (4, 12), colour: [0, 255, 255, 255] },
    ];

    /// A real PNG of a real texture, painted part by part, so what the tests
    /// exercise is the decode and the cut rather than a mock of either.
    ///
    /// `height` picks the version: 64 paints all six parts, and 32 paints only the
    /// four the legacy format has -- which is the point of the second test.
    fn texture(height: u32) -> Vec<u8> {
        let mut image = image::RgbaImage::from_pixel(TEXTURE_WIDTH, height, image::Rgba([0, 0, 0, 0]));
        for paint in PAINTS {
            let (from, size) = (paint.from, paint.size);
            if from.1 + size.1 > height {
                continue;
            }
            for y in from.1..from.1 + size.1 {
                for x in from.0..from.0 + size.0 {
                    image.put_pixel(x, y, image::Rgba(paint.colour));
                }
            }
        }
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("a PNG in memory");
        bytes
    }

    /// The colour at one pixel of the front view.
    fn at(view: &FrontView, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * view.width + x) * 4) as usize;
        let mut pixel = [0u8; 4];
        pixel.copy_from_slice(&view.pixels[offset..offset + 4]);
        pixel
    }

    #[test]
    fn the_front_view_lays_the_six_parts_out_where_the_doll_wants_them() {
        let view = front_view(&texture(TEXTURE_HEIGHT)).expect("a 64x64 skin");
        assert_eq!((view.width, view.height), (FRONT_WIDTH, FRONT_HEIGHT));
        assert_eq!(view.pixels.len(), (FRONT_WIDTH * FRONT_HEIGHT * 4) as usize);
        // The head, centred over the body: its top-left corner is 4 in from the
        // left, because the arm is 4 wide.
        assert_eq!(at(&view, 4, 0), [255, 0, 0, 255], "the head");
        assert_eq!(at(&view, 11, 7), [255, 0, 0, 255], "and its far corner");
        // The right arm on the reader's left -- the player's own right -- and the
        // body beside it.
        assert_eq!(at(&view, 0, 8), [0, 255, 0, 255], "the right arm");
        assert_eq!(at(&view, 3, 19), [0, 255, 0, 255]);
        assert_eq!(at(&view, 4, 8), [0, 0, 255, 255], "the body");
        assert_eq!(at(&view, 11, 19), [0, 0, 255, 255]);
        // The left arm on the right, and the two legs under the body.
        assert_eq!(at(&view, 12, 8), [255, 255, 0, 255], "the left arm");
        assert_eq!(at(&view, 4, 20), [255, 0, 255, 255], "the right leg");
        assert_eq!(at(&view, 8, 20), [0, 255, 255, 255], "the left leg");
        // And the corners the doll does not fill stay as transparent as the
        // texture was: nothing here paints a background.
        assert_eq!(at(&view, 0, 0), [0, 0, 0, 0]);
        assert_eq!(at(&view, 15, 31), [0, 0, 0, 0]);
    }

    #[test]
    fn a_legacy_skin_has_its_left_limbs_mirrored_rather_than_left_empty() {
        let view = front_view(&texture(LEGACY_HEIGHT)).expect("a 64x32 skin");
        // The right arm and the right leg are the legacy format's own.
        assert_eq!(at(&view, 0, 8), [0, 255, 0, 255], "the right arm");
        assert_eq!(at(&view, 4, 20), [255, 0, 255, 255], "the right leg");
        // The left slots are the right limb's pixels, flipped: the right arm's
        // texture is painted one colour top-to-bottom here, so what the flip shows
        // is that the *columns* are copied from the right limb and not left empty.
        assert_eq!(at(&view, 12, 8), [0, 255, 0, 255], "the left arm");
        assert_eq!(at(&view, 8, 20), [255, 0, 255, 255], "the left leg");
        // Mirrored rather than copied: the right arm's *outer* column in the
        // texture (x=47, the last one of its 44..48 box) is the left arm's own
        // *inner* one on screen, and the other way round.
        let mut image =
            image::RgbaImage::from_pixel(TEXTURE_WIDTH, LEGACY_HEIGHT, image::Rgba([0, 0, 0, 0]));
        for y in 20..32 {
            image.put_pixel(44, y, image::Rgba([255, 0, 0, 255]));
            image.put_pixel(47, y, image::Rgba([0, 0, 255, 255]));
        }
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("a PNG in memory");
        let flipped = front_view(&bytes).expect("a 64x32 skin");
        assert_eq!(at(&flipped, 12, 8), [0, 0, 255, 255], "the texture's last column");
        assert_eq!(at(&flipped, 15, 8), [255, 0, 0, 255], "and its first, on the far side");
    }

    #[test]
    fn a_texture_that_is_not_a_skin_is_refused_rather_than_half_drawn() {
        // Not an image at all.
        assert!(front_view(b"this is not a PNG").is_none());
        // An image of the wrong shape: the two heights Minecraft publishes are the
        // only two this knows the layout of.
        for (width, height) in [(64, 63), (64, 34), (32, 64), (63, 64)] {
            let image = image::RgbaImage::from_pixel(width, height, image::Rgba([1, 2, 3, 255]));
            let mut bytes = Vec::new();
            image::DynamicImage::ImageRgba8(image)
                .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
                .expect("a PNG in memory");
            assert!(front_view(&bytes).is_none(), "{width}x{height} is not a skin");
        }
    }

    #[test]
    fn the_view_is_something_the_image_widget_can_draw() {
        let view = front_view(&texture(TEXTURE_HEIGHT)).expect("a 64x64 skin");
        // The handle is built from the pixels rather than from the encoded bytes:
        // the cut is the whole point, and handing the renderer the texture would
        // draw a 64x64 grid of body parts.
        drop(view.handle());
    }

    /// A decoded PNG, for the tests that ask what a transform did to the pixels
    /// rather than what it drew.
    fn decoded(bytes: &[u8]) -> image::RgbaImage {
        image::load_from_memory(bytes).expect("a PNG in memory").to_rgba8()
    }

    /// A 64x32 texture whose right arm's front face is painted one colour per column,
    /// so which way a mirror went is a pixel a test can name.
    fn four_colour_arm() -> Vec<u8> {
        let mut image =
            image::RgbaImage::from_pixel(TEXTURE_WIDTH, LEGACY_HEIGHT, image::Rgba([0, 0, 0, 0]));
        for (index, column) in (44..48).enumerate() {
            let colour = image::Rgba([10 * (index as u8 + 1), 0, 0, 255]);
            for y in 20..32 {
                image.put_pixel(column, y, colour);
            }
        }
        encode(image).expect("a PNG in memory")
    }

    #[test]
    fn a_legacy_texture_is_padded_to_the_modern_shape_with_both_left_limbs_filled() {
        let legacy = texture(LEGACY_HEIGHT);
        let prepared = prepare(&legacy).expect("a 64x32 skin");
        // The arm style is read from pixels and not from the format, so this texture
        // answers what its own pixels say: the six parts painted here do not include
        // the right arm's back face, and the two columns the model comes from are
        // inside that face. A legacy file painted the way a reader's own is -- all
        // sixteen columns of the arm's box -- reads classic, and the arm-style test
        // below is that case.
        assert_eq!(prepared.model, Model::Slim);
        let modern = decoded(&prepared.png);
        assert_eq!((modern.width(), modern.height()), (TEXTURE_WIDTH, TEXTURE_HEIGHT));
        // What the legacy layout already had is where it was, at the same addresses:
        // the head, the body, and the right limbs' fronts.
        assert_eq!(modern.get_pixel(8, 8).0, [255, 0, 0, 255], "the head");
        assert_eq!(modern.get_pixel(20, 20).0, [0, 0, 255, 255], "the body");
        assert_eq!(modern.get_pixel(44, 20).0, [0, 255, 0, 255], "the right arm's front");
        assert_eq!(modern.get_pixel(4, 20).0, [255, 0, 255, 255], "the right leg's front");
        // The left limbs' boxes are filled from the right ones: the arm's front face
        // at (36, 52) and the leg's at (20, 52), which are the modern layout's own
        // addresses for them.
        assert_eq!(modern.get_pixel(36, 52).0, [0, 255, 0, 255], "the left arm's front");
        assert_eq!(modern.get_pixel(39, 63).0, [0, 255, 0, 255], "to its far corner");
        assert_eq!(modern.get_pixel(20, 52).0, [255, 0, 255, 255], "the left leg's front");
        assert_eq!(modern.get_pixel(23, 63).0, [255, 0, 255, 255]);
        // The faces of the box that the legacy texture leaves empty stay empty, as
        // do the columns the fill does not reach -- the fill is the right limb's own
        // sixteen columns and not a set of pixels at all.
        assert_eq!(modern.get_pixel(44, 52).0, [0, 0, 0, 0], "the left arm's back face");
        assert_eq!(modern.get_pixel(36, 48).0, [0, 0, 0, 0], "and the strip above it");
        assert_eq!(modern.get_pixel(52, 52).0, [0, 0, 0, 0], "and the overlay box beside it");
        // The second layer is empty: a legacy texture has no jacket, no sleeves and no
        // trouser overlay, and painting them would be inventing pixels the reader never
        // drew.
        assert_eq!(modern.get_pixel(16, 32).0, [0, 0, 0, 0], "no jacket");
        assert_eq!(modern.get_pixel(40, 32).0, [0, 0, 0, 0], "no sleeve overlay");
        assert_eq!(modern.get_pixel(0, 32).0, [0, 0, 0, 0], "no trouser overlay");
        assert_eq!(modern.get_pixel(0, 48).0, [0, 0, 0, 0], "no left trouser overlay");
    }

    #[test]
    fn the_left_limbs_are_the_right_ones_mirrored_rather_than_copied() {
        // The direction of the flip, as four pixels: the right arm's front face is
        // painted 10, 20, 30, 40 from left to right, and the left arm's is the same
        // four in the other order. A copy would have put 10 where 40 is.
        let modern = decoded(&prepare(&four_colour_arm()).expect("a 64x32 skin").png);
        assert_eq!(modern.get_pixel(44, 20).0[0], 10, "the right arm's own front, leftmost column");
        assert_eq!(modern.get_pixel(47, 31).0[0], 40, "and its rightmost");
        assert_eq!(modern.get_pixel(36, 52).0[0], 40, "the left arm's front, mirrored");
        assert_eq!(modern.get_pixel(39, 63).0[0], 10, "to its other end");
    }

    #[test]
    fn a_normalised_legacy_texture_draws_the_same_doll_as_the_legacy_one() {
        // The property the transform exists for, and the one that pins the direction
        // of the mirror harder than any single pixel can: the front view cut from a
        // legacy texture -- which reads the right limb's own pixels flipped, through
        // `PARTS`' stand-ins -- has to be the front view cut from the normalised one,
        // which has no stand-in to read because its left boxes are real pixels now.
        // The two halves of this module agreeing is what "the same appearance" means.
        let legacy = texture(LEGACY_HEIGHT);
        let prepared = prepare(&legacy).expect("a 64x32 skin");
        let cut_from_legacy = front_view(&legacy).expect("a 64x32 skin");
        let cut_from_modern = front_view(&prepared.png).expect("the normalised texture");
        assert_eq!(cut_from_legacy, cut_from_modern);
        // And with the four-colour arm, where a wrong direction would be a different
        // picture rather than the same one: the doll's left arm is the right arm's
        // pixels reversed, in both readings.
        let coloured = four_colour_arm();
        let prepared = prepare(&coloured).expect("a 64x32 skin");
        assert_eq!(
            front_view(&coloured).expect("a 64x32 skin"),
            front_view(&prepared.png).expect("a normalised skin")
        );
    }

    #[test]
    fn a_modern_texture_comes_back_pixel_for_pixel() {
        let modern = texture(TEXTURE_HEIGHT);
        let prepared = prepare(&modern).expect("a 64x64 skin");
        assert_eq!(decoded(&prepared.png), decoded(&modern), "nothing is padded, nothing is lost");
        // The model is the two columns' answer and not the arm's: this texture paints
        // the arm's *front* face, which is not what the reference reads, so it is slim.
        assert_eq!(prepared.model, Model::Slim);
    }

    #[test]
    fn the_arm_style_is_read_from_the_two_columns_the_reference_reads() {
        // `MODEL_COLUMN`'s rectangle, which is the right arm's back face's last two
        // columns: paint there is a four-wide arm, and padding is a three-wide one.
        // Everything else in the arm's own box is painted first, so the answer cannot
        // be coming from somewhere else in it.
        let arm = |paint_the_two_columns: bool| {
            let mut image = image::RgbaImage::from_pixel(
                TEXTURE_WIDTH,
                TEXTURE_HEIGHT,
                image::Rgba([0, 0, 0, 0]),
            );
            for y in 16..32 {
                for x in 40..54 {
                    image.put_pixel(x, y, image::Rgba([1, 2, 3, 255]));
                }
                if paint_the_two_columns {
                    for x in 54..56 {
                        image.put_pixel(x, y, image::Rgba([1, 2, 3, 255]));
                    }
                }
            }
            encode(image).expect("a PNG in memory")
        };
        assert_eq!(prepare(&arm(false)).expect("a skin").model, Model::Slim);
        assert_eq!(prepare(&arm(true)).expect("a skin").model, Model::Classic);
        // One pixel of the twelve is enough, and it is the pixel the reference's own
        // loop stops on.
        let mut image = image::RgbaImage::from_pixel(
            TEXTURE_WIDTH,
            TEXTURE_HEIGHT,
            image::Rgba([0, 0, 0, 0]),
        );
        image.put_pixel(55, 31, image::Rgba([1, 2, 3, 255]));
        assert_eq!(prepare(&encode(image).expect("a PNG")).expect("a skin").model, Model::Classic);
        // The main test texture is the same case from the other side: the arm's *front*
        // face is painted and these two columns are not, so it is slim -- which is what
        // the reference's rule says too, and the reason it reads a face no artist
        // thinks of as the one that decides.
        assert_eq!(prepare(&texture(TEXTURE_HEIGHT)).expect("a skin").model, Model::Slim);
        // And the *format* does not decide it: a legacy file whose arm box is painted
        // the way a real one's is -- all sixteen columns, not just the front -- reads
        // classic through the same two columns, padded or not.
        let mut legacy =
            image::RgbaImage::from_pixel(TEXTURE_WIDTH, LEGACY_HEIGHT, image::Rgba([0, 0, 0, 0]));
        for y in 16..32 {
            for x in 40..56 {
                legacy.put_pixel(x, y, image::Rgba([1, 2, 3, 255]));
            }
        }
        assert_eq!(
            prepare(&encode(legacy).expect("a PNG in memory")).expect("a skin").model,
            Model::Classic,
            "a painted legacy arm box is a four-wide arm"
        );
    }

    #[test]
    fn a_texture_of_neither_shape_is_refused_by_the_upload_path_too() {
        // The upload path's refusals, which are the drawing path's: a file that cannot
        // be cut into a player cannot be sent as one either, and the page says so
        // rather than uploading a 32x64 rectangle the service would reject.
        assert!(prepare(b"this is not a PNG").is_none());
        for (width, height) in [(64, 63), (64, 34), (32, 64), (63, 64)] {
            let image = image::RgbaImage::from_pixel(width, height, image::Rgba([1, 2, 3, 255]));
            let bytes = encode(image).expect("a PNG in memory");
            assert!(prepare(&bytes).is_none(), "{width}x{height} is not a skin");
        }
    }

    /// What Minecraft's document says for the two tests below: one skin in force,
    /// one cape, and the name.
    fn owned() -> MinecraftSkins {
        MinecraftSkins {
            skins: vec![
                MinecraftSkin {
                    id: "skin-1".into(),
                    state: "ACTIVE".into(),
                    url: "http://textures.minecraft.net/texture/aaa".into(),
                    variant: "CLASSIC".into(),
                },
                MinecraftSkin {
                    id: "skin-2".into(),
                    state: "INACTIVE".into(),
                    url: "http://textures.minecraft.net/texture/bbb".into(),
                    variant: "SLIM".into(),
                },
            ],
            capes: vec![MinecraftCape {
                id: "cape-1".into(),
                state: "ACTIVE".into(),
                url: "http://textures.minecraft.net/texture/ccc".into(),
                alias: "Migrator".into(),
            }],
        }
    }

    #[test]
    fn an_appearance_is_the_document_s_lists_beside_the_skin_it_could_draw() {
        let appearance = Appearance::of("Steve", owned(), Ok(texture(TEXTURE_HEIGHT)));
        assert_eq!(appearance.username, "Steve");
        assert_eq!(appearance.skins.len(), 2);
        assert_eq!(appearance.equipped().map(|skin| skin.variant.as_str()), Some("CLASSIC"));
        assert_eq!(appearance.equipped_cape().map(|cape| cape.alias.as_str()), Some("Migrator"));
        assert!(appearance.front.is_some(), "the skin in force is drawn");
        assert!(appearance.note.is_none());
    }

    #[test]
    fn a_texture_that_will_not_come_back_leaves_the_lists_where_they_were() {
        // The two ways the picture can be missing, and neither of them is the
        // page's: a fetch that failed, and bytes that are not a skin.
        let refused = Appearance::of("Steve", owned(), Err("the network is down".into()));
        assert!(refused.front.is_none());
        assert_eq!(refused.note.as_deref(), Some("the network is down"));
        assert_eq!(refused.skins.len(), 2, "what the account owns does not change");
        assert_eq!(refused.equipped().map(|skin| skin.id.as_str()), Some("skin-1"));

        let not_a_skin = Appearance::of("Steve", owned(), Ok(b"not a PNG".to_vec()));
        assert!(not_a_skin.front.is_none());
        assert!(
            not_a_skin.note.as_deref().unwrap_or_default().contains("not a texture"),
            "got {:?}",
            not_a_skin.note
        );
        assert_eq!(not_a_skin.capes.len(), 1);
    }

    #[test]
    fn an_account_with_no_skin_in_force_is_drawn_without_a_picture() {
        let mut document = owned();
        document.skins.iter_mut().for_each(|skin| skin.state = "INACTIVE".into());
        let appearance = Appearance::of("Steve", document, Err("no skin is in force".into()));
        assert!(appearance.equipped().is_none());
        assert!(appearance.front.is_none());
        assert_eq!(appearance.skins.len(), 2, "both are still the account's own");
    }
}
