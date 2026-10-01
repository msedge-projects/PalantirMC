//! PalantirMC branding: the launcher logo and the window/taskbar icon.
//!
//! The artwork lives in `assets/brand/` and is embedded with `include_bytes!`
//! so the GUI never touches the disk to paint the logo:
//!
//! * `palantirmc.png` — the original 1254x1254 source art (kept for reference,
//!   not embedded).
//! * `logo512.png` — the emblem, square-cropped with a small margin so it stays
//!   centered, 512x512. This is what the welcome hero and the title bar paint.
//! * `icon256.png` — the same crop at 256x256, for the window/taskbar icon.
//!
//! Both derived files are RGBA with the source's pure-black backdrop turned
//! into real transparency, so the emblem blends into the dark UI and the
//! taskbar instead of sitting on a black tile. Alpha is the pixel's brightest
//! channel (the backdrop is `#000000`, so black is exactly zero), resampled
//! premultiplied and normalized so the emblem's core stays fully opaque;
//! pixels below 8/255 are cleared outright, which drops the resampler's
//! ringing and keeps the darkest glow from turning into grey haze.
//!
//! Both derived files come from `tools/gen_brand.py`, which is that recipe in
//! code: it measures the mark's ink box, cuts a square centred on it with the
//! mark filling 91% of the side -- 309x464 of the 512 tile, so the hero and the
//! taskbar get the same framing -- and resamples that once per output in
//! premultiplied space. Regenerate with `python tools/gen_brand.py`; `--check`
//! fails if the files on disk are not what it writes.

use iced::widget::image::Handle;
use iced::window::Icon;
use std::sync::OnceLock;

/// Product name shown in the title bar, the hero card and the About page.
///
/// [`palantir_core::PRODUCT_NAME`] rather than a second literal: the name also
/// travels into the game through the launch script, and two spellings of one
/// product name is how a window ends up labelled with a different launcher.
pub const APP_NAME: &str = palantir_core::PRODUCT_NAME;

/// Embedded hero logo (512x512 RGBA PNG, transparent backdrop).
const LOGO_PNG: &[u8] = include_bytes!("../assets/brand/logo512.png");

/// Embedded window icon (256x256 RGBA PNG, transparent backdrop).
const ICON_PNG: &[u8] = include_bytes!("../assets/brand/icon256.png");

/// Version reported by the About page (`Cargo.toml`).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// `PalantirMC 0.1.0` — used on the About page and in logs.
#[cfg(test)]
pub fn full_name() -> String {
    format!("{APP_NAME} {}", version())
}

/// Window title.
pub fn window_title() -> String {
    APP_NAME.to_string()
}

/// Longest side the embedded hero logo is decoded at, in pixels.
///
/// The largest the mark is ever drawn is the welcome tile's 112 logical pixels;
/// the About badge asks for 64. Handing the renderer the 512x512 original costs
/// a megabyte of RGBA for a picture a tenth that size, so it is shrunk once at
/// startup instead. 256 is a little over 2x the largest drawn size, which keeps
/// the mark sharp on a 200%-scaled display without growing the cache again.
const LOGO_SIDE: u32 = 256;

/// Handle for the hero logo. Built once per process; `Handle` clones are
/// `Arc` bumps, so per-frame cost is nil -- the same caching argument
/// [`crate::icon`] makes for the reference's geometry.
///
/// The pixels are decoded here rather than left to the renderer so that what
/// gets cached is [`LOGO_SIDE`] square rather than the full original: on this
/// mark that is 262 KB instead of 1 MB, and it gives the renderer's sampler a
/// source close to the size of the thing it paints.
pub fn logo_handle() -> Handle {
    static HANDLE: OnceLock<Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| {
            shrunk_logo_pixels()
                .map(|(width, height, pixels)| Handle::from_pixels(width, height, pixels))
                // Falling back to the encoded bytes keeps a surprise in the
                // asset — a re-export in a format `image` cannot read — from
                // turning the logo into a panic on startup.
                .unwrap_or_else(|| Handle::from_memory(LOGO_PNG))
        })
        .clone()
}

/// The embedded logo, shrunk to [`LOGO_SIDE`]: `(width, height, RGBA8)`.
///
/// `None` when the embedded art will not decode, which is the caller's cue to
/// fall back to handing the renderer the encoded bytes.
fn shrunk_logo_pixels() -> Option<(u32, u32, Vec<u8>)> {
    let decoded = image::load_from_memory(LOGO_PNG).ok()?;
    let shrunk = if decoded.width().max(decoded.height()) <= LOGO_SIDE {
        decoded
    } else {
        // Shrinking only: `thumbnail` scales up to fill the box it is given.
        decoded.thumbnail(LOGO_SIDE, LOGO_SIDE)
    };
    let rgba = shrunk.to_rgba8();
    Some((rgba.width(), rgba.height(), rgba.into_raw()))
}

/// Decode the embedded icon into an iced window icon. Decoding happens once
/// (it is only consulted while building the window settings) and returns
/// `None` rather than panicking if an exotic platform rejects the pixels.
pub fn window_icon() -> Option<Icon> {
    static ICON: OnceLock<Option<Icon>> = OnceLock::new();
    ICON.get_or_init(|| iced::window::icon::from_file_data(ICON_PNG, None).ok())
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    #[test]
    fn embedded_art_is_png_with_png_magic() {
        assert_eq!(&LOGO_PNG[..8], &PNG_MAGIC);
        assert_eq!(&ICON_PNG[..8], &PNG_MAGIC);
        assert!(LOGO_PNG.len() > 1024, "logo must be real art, not a stub");
        assert!(ICON_PNG.len() > 512, "icon must be real art, not a stub");
    }

    #[test]
    fn logo_handle_is_stable_across_calls() {
        assert_eq!(logo_handle().id(), logo_handle().id());
    }

    #[test]
    fn logo_is_decoded_at_the_drawn_size_not_the_original() {
        let (width, height, pixels) = shrunk_logo_pixels().expect("embedded logo must decode");
        assert_eq!((width, height), (LOGO_SIDE, LOGO_SIDE));
        assert_eq!(pixels.len(), (LOGO_SIDE * LOGO_SIDE * 4) as usize);
        // Shrinking must not lose the transparency the mark depends on: the
        // square crop's corners stay clear.
        assert!(pixels.chunks_exact(4).any(|px| px[3] == 0), "backdrop must stay transparent");

        // The bar is relative rather than a fixed threshold, and the artwork is
        // why: the mark this test was written against was a soft radial glow, of
        // which at 512x512 exactly two pixels reached alpha 255 and only 102
        // cleared 240, so a fixed bar measured where the brightest pixel landed
        // on the sampling grid rather than whether the shrink preserved the art
        // — 239 is the honest result of averaging that spike into a 2x2 window.
        // The mark that ships now is a stroke, with 1,423 pixels at alpha 255 in
        // the same tile; a fixed bar would flatter that one instead. Measuring
        // against the source is the claim that holds for either.
        let source = image::load_from_memory(LOGO_PNG)
            .expect("embedded logo must decode")
            .to_rgba8();
        let source_alpha = source.as_raw();
        let source_peak = source_alpha.chunks_exact(4).map(|px| px[3]).max().unwrap_or(0);
        let source_ink: u64 = source_alpha.iter().skip(3).step_by(4).map(|&a| u64::from(a)).sum();

        let peak = pixels.chunks_exact(4).map(|px| px[3]).max().unwrap_or(0);
        assert!(
            u32::from(peak) * 100 >= u32::from(source_peak) * 90,
            "the glow must not be dimmed by the shrink: peak {peak} at {LOGO_SIDE}px \
             against {source_peak} at 512px"
        );

        // Ink is what a bad resample destroys: dimming every pixel keeps the
        // shape while washing the mark out, and clipping brightens it. A 2:1
        // downsample should carry the same total alpha into a quarter of the
        // pixels. The 5% band absorbs the filter's fractional edge windows and
        // its per-pixel integer truncation.
        let ink: u64 = pixels.iter().skip(3).step_by(4).map(|&a| u64::from(a)).sum();
        let expected = source_ink / 4;
        let drift = ink.abs_diff(expected) * 100;
        assert!(
            drift <= expected * 5,
            "the shrink changed the mark's total ink: {ink} against {expected} expected \
             ({drift}% of it)"
        );
    }

    #[test]
    fn window_icon_decodes_to_256_square() {
        let icon = window_icon().expect("embedded icon must decode");
        let (rgba, size) = icon.into_raw();
        assert_eq!(size.width, 256);
        assert_eq!(size.height, 256);
        assert_eq!(rgba.len(), 256 * 256 * 4);
        // The backdrop was made transparent: some pixels must be fully clear
        // (the square crop's corners) while the emblem stays opaque.
        assert!(rgba.chunks_exact(4).any(|px| px[3] == 0));
        assert!(rgba.chunks_exact(4).any(|px| px[3] == 255));
        // A clear pixel must not keep a colour, or a compositor that ignores
        // alpha at small sizes would paint the resampler's ringing.
        assert!(rgba.chunks_exact(4).filter(|px| px[3] == 0).all(|px| px[..3] == [0, 0, 0]));
        // The emblem is centred, not jammed into a corner.
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (256usize, 0usize, 256usize, 0usize);
        for (index, px) in rgba.chunks_exact(4).enumerate() {
            if px[3] < 8 {
                continue;
            }
            let (x, y) = (index % 256, index / 256);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let (dx, dy) = (max_x + min_x, max_y + min_y);
        assert!((dx as i32 - 255).abs() <= 4, "logo is off-centre horizontally: {min_x}..{max_x}");
        assert!((dy as i32 - 255).abs() <= 4, "logo is off-centre vertically: {min_y}..{max_y}");
    }

    #[test]
    fn hero_logo_is_transparent() {
        // Decoded through the very path the window icon uses, so no extra
        // image dependency is needed to check the pixels.
        let icon = iced::window::icon::from_file_data(LOGO_PNG, None)
            .expect("embedded hero logo must decode");
        let (rgba, size) = icon.into_raw();
        assert_eq!((size.width, size.height), (512, 512));
        assert_eq!(rgba.len(), 512 * 512 * 4);
        assert!(rgba.chunks_exact(4).any(|px| px[3] == 0), "backdrop must be transparent");
        assert!(rgba.chunks_exact(4).any(|px| px[3] == 255), "emblem must be opaque");
    }

    #[test]
    fn names_are_stable() {
        assert_eq!(APP_NAME, "PalantirMC");
        assert_eq!(window_title(), "PalantirMC");
        assert!(full_name().starts_with("PalantirMC "));
        assert!(full_name().ends_with(version()));
    }
}
