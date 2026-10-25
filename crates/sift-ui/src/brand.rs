//! The product's mark: `logo512.png`, decoded once and uploaded as a texture.
//!
//! **Why the bytes are embedded.** The exe is the product. A launcher that reads its logo
//! from a path beside itself draws an empty plate the moment it is moved, copied into
//! another folder, started from a shortcut with a different working directory, or installed
//! by something that did not copy the art -- and that is exactly the bug this fixes: the
//! hero and the title bar carried a comment saying the mark was an asset "until the texture
//! is uploaded for it", so the plate the mark sits on was all a reader ever saw. `include_bytes!`
//! makes the drawing part of the binary, so the picture cannot be separated from the program
//! that draws it.
//!
//! **What it costs.** `logo512.png` is **148,607 bytes -- 145.1 KiB -- measured on the file**
//! that `tools/gen_brand.py` writes. `include_bytes!` places the PNG in the binary's
//! read-only data verbatim: the linker neither compresses nor re-encodes it, so that is the
//! whole cost of embedding. The 512-pixel copy rather than `icon256.png` (46,465 bytes)
//! because the About view draws the mark at 96 points, which is 192 px on a 2x display and
//! 288 px at 3x, where a 256-pixel source would be upscaled; the two files are the same
//! drawing, so the 512 is the one that survives every scale the window will ask for. At
//! runtime the cost is one decode and one upload, and the texture is 512x512x4 = 1 MiB of
//! GPU memory; the decoded `ColorImage` is not kept after the upload, so there is no second
//! CPU copy of the art.
//!
//! **Why `Context::load_texture` and not egui's image loaders.** `load_texture` takes a
//! decoded `ColorImage`, and this crate already carries `image` with its `png` feature for
//! project icons ([`crate::images`]). Adding `egui_extras` and `install_image_loaders` for
//! one embedded file would be a second image stack, a second cache and a URI scheme, where
//! this is the decode the icon cache already uses followed by the upload egui documents.
//!
//! The asset is Sift's own art -- `tools/gen_brand.py` derives it from
//! `assets/brand/sift-logo-source.png` -- so nothing here is adopted material and no notice
//! travels with it. The iced build embeds the same file from `crates/sift-desktop/src/brand.rs`;
//! the two crates do not share an asset pipeline, so each embeds the one file in the tree.

use std::sync::OnceLock;

use eframe::egui::{self, ColorImage, TextureHandle};

/// The mark, 512x512 RGBA, as `tools/gen_brand.py` writes it.
///
/// The path is the generated file's own: `assets/brand/logo512.png`,
/// which is where the generator and `--check` look for it. `src/brand.rs` is two
/// directories below the crate root, so the walk up is `../../`.
const LOGO_PNG: &[u8] = include_bytes!("../../../assets/brand/logo512.png");

/// The mark's texture, uploaded once for the life of the process.
///
/// The `Option` inside is the failure case *remembered*: a decode that failed is a fact
/// about a build-time asset, not something that can heal on the next frame, and re-decoding
/// 148,607 bytes per frame to find that out again would be the one cost this module exists
/// to avoid. `install` fills the cell at startup, so the first frame that draws the title
/// bar already has the mark.
static MARK: OnceLock<Option<TextureHandle>> = OnceLock::new();

/// Upload the mark, once.
///
/// Called from `main` beside [`crate::fonts::install`]: both are process-wide setup, and
/// doing this before the first frame means the window's first paint shows the mark rather
/// than the fallback plate. Idempotent -- a second call is a no-op through the `OnceLock`.
pub fn install(ctx: &eframe::egui::Context) {
    // The call is the installation; its answer is what `mark` hands back later.
    let _ = mark(ctx);
}

/// The mark's texture, if this process has one.
///
/// Loads on the first call when [`install`] was not called, which is what makes a test or a
/// second entry point work without startup order. A texture uploaded through one
/// `egui::Context` belongs to that context's texture manager, so the handle is meaningful
/// for as long as the context that produced it lives -- one window per process here, which
/// is why this can be a `OnceLock` at all.
pub fn mark(ctx: &eframe::egui::Context) -> Option<TextureHandle> {
    MARK.get_or_init(|| upload(ctx)).clone()
}

/// Whether a call site draws the mark, or the plate it would sit on.
///
/// The answer is the texture's existence and nothing else. The three call sites -- the title
/// bar's 20-point box, the hero's 64-point plate and the About view's 96 -- differ in size
/// and in what a missing mark would cost, and none of them differs in this rule. It is a
/// function rather than an `if let` per site so the fallback has one name and one test: a
/// context with no GPU, or the frame before the upload lands, must still draw *something*,
/// and "no texture" is the only case that draws the plate instead.
pub fn use_mark(mark: bool) -> bool {
    mark
}

/// Draw the mark into `rect`; `false` means there is none, and the caller draws its plate.
///
/// One function for the three sites because the alternative is the same `if let` three
/// times, and the picture's fit is part of the rule: the texture's own square fills the
/// rect, and the asset's transparent margin is inside it, so the caller's rect is the
/// framing rather than something the drawing second-guesses. The whole of the UV space is
/// asked for for the same reason -- cropping here would silently change the framing the
/// hero and the About view were measured at.
pub fn paint_mark(painter: &egui::Painter, ctx: &egui::Context, rect: egui::Rect) -> bool {
    let mark = mark(ctx);
    if !use_mark(mark.is_some()) {
        return false;
    }
    match mark {
        Some(texture) => {
            painter.image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            true
        }
        // `use_mark` has already answered this case, and the branch is kept honest rather
        // than made unreachable with an unwrap: the mark is a build asset, not a guarantee.
        None => false,
    }
}

/// Decode and upload, or say why not.
///
/// A failure is `None` rather than a panic: a mark that will not decode is a window with a
/// plate in it, and a launcher that refuses to start over its logo would be worse than one
/// that starts without it. The generation of the asset is gated in `tools/gen_brand.py`; a
/// failure here means the bytes in the binary are not a PNG this build can read.
fn upload(ctx: &eframe::egui::Context) -> Option<TextureHandle> {
    let image = decode(LOGO_PNG)?;
    // `load_texture` panics on a zero-sized image; the size comes from the decoded bytes
    // rather than from a constant, so this is the one check that keeps a bad asset from
    // taking the process down. 512x512 is far inside every backend's texture limit.
    if image.size[0] == 0 || image.size[1] == 0 {
        return None;
    }
    Some(ctx.load_texture("sift-brand-mark", image, egui::TextureOptions::LINEAR))
}

/// The bytes as a `ColorImage`, through the decoder the icon cache already uses.
///
/// `image` with its `png` feature is already a dependency for the project CDN's icons, so
/// this is the same `load_from_memory` + `to_rgba8` pair as [`crate::images`]'s worker --
/// one decoder in the crate rather than two.
fn decode(bytes: &[u8]) -> Option<ColorImage> {
    let decoded = image::load_from_memory(bytes).ok()?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded bytes are the generated PNG, at the size the module doc names.
    ///
    /// The length is asserted because the doc quotes it as the measured cost of embedding:
    /// a regenerated asset that changes the number has to change the paragraph above in the
    /// same commit, rather than leaving a figure that is no longer true in the one place a
    /// reviewer looks for it.
    #[test]
    fn the_embedded_mark_is_the_generated_png() {
        assert_eq!(&LOGO_PNG[..8], b"\x89PNG\r\n\x1a\n", "a PNG signature");
        assert_eq!(&LOGO_PNG[12..16], b"IHDR", "the first chunk is the header");
        let width = u32::from_be_bytes(LOGO_PNG[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(LOGO_PNG[20..24].try_into().unwrap());
        assert_eq!((width, height), (512, 512), "the mark is 512x512");
        assert_eq!(
            LOGO_PNG.len(),
            148_607,
            "the byte cost the module doc quotes"
        );
    }

    /// The bytes decode through the same path the texture upload takes, so a broken asset
    /// fails here -- in a test with no window -- rather than on somebody's first frame.
    ///
    /// This is a real decode of the real file: the `image` crate's PNG decoder is what
    /// `upload` calls, and the only parts it cannot exercise without a GPU are the upload
    /// and the texture itself.
    #[test]
    fn the_mark_decodes_to_one_texture_sized_square() {
        let image = decode(LOGO_PNG).expect("the generated mark decodes");
        assert_eq!(image.size, [512, 512]);
        assert_eq!(image.pixels.len(), 512 * 512, "four bytes per pixel, 512 square");
    }

    /// The fallback rule: a texture draws the mark, no texture draws the plate. Both
    /// answers are asserted because a function that always said "no" would still pass the
    /// first half of this.
    #[test]
    fn a_missing_mark_falls_back_to_the_plate() {
        assert!(use_mark(true), "a texture is what the mark is for");
        assert!(!use_mark(false), "no texture draws the plate instead");
    }
}
