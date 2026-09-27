//! Instance screenshots: finding them, and turning them into something the grid
//! can draw without exhausting memory.
//!
//! Minecraft writes a screenshot per F2 press into `<instance>/screenshots/`, and
//! a well-used instance has hundreds. Two consequences shape this module:
//!
//! * **Discovery is cheap; decoding is not.** The grid only needs a bounded
//!   number of pictures, so [`scan`] walks directories and reads metadata only —
//!   no decoding — and [`thumbnail`] shrinks the handful that are actually shown.
//! * **Full-size screenshots do not fit.** A 1920x1080 screenshot is 8.3 MB as
//!   RGBA. Handing sixty of those to the renderer would commit half a gigabyte,
//!   which on the machine this shell was reported against is most of the
//!   available memory. Thumbnails are sized by [`thumbnail_side`] to the pixels
//!   the grid will actually paint — see that function for why the size is a
//!   query rather than a constant — which keeps a full page of [`MAX_SHOWN`] of
//!   them under ten megabytes of pixels on a 100% display.
//!
//! Decoding runs on the same background worker the rest of the shell uses, never
//! on the UI thread: `image::open` on a 1080p PNG takes tens of milliseconds,
//! and doing that inside a frame would be visible as a stutter.

// The path-shape helpers below are what the tests read: the shell hands this
// module a path and gets an entry back.
#[cfg(test)]
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time::SystemTime;

/// Extensions the page treats as screenshots. Minecraft writes `.png`; the
/// others are here because people drop edited copies next to the originals.
#[cfg(test)]
const IMAGE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

/// Longest side of a generated thumbnail on a 100%-scaled display, in pixels.
///
/// This is the width the grid draws a picture at (`app::SHOT_TILE_WIDTH`), and
/// that is the point of the number: a thumbnail the size of the box it is drawn
/// into needs no resampling, so the renderer's sampler — the per-frame cost of
/// this page — has nothing to filter. It used to be 360, which fed 360x203
/// pixels to a 268x151 box and spent 1.8x the memory doing it.
///
/// A scaled display gets a proportionally bigger thumbnail ([`thumbnail_side`]),
/// so this is a floor rather than the size itself.
#[cfg(test)]
pub const THUMBNAIL_SIDE: u32 = 268;

/// The largest side [`thumbnail_side`] will ever ask for.
///
/// Cost is quadratic in this number, and the page holds [`MAX_SHOWN`] of them:
/// 448px on a 16:9 source is about 21 MB of pixels for the page, which is the
/// ceiling worth carrying on a machine that runs Minecraft. It exists for the
/// 200%-scaled display, where the tile really is 536 physical pixels wide; past
/// that the extra sharpness is not worth the memory.
#[cfg(test)]
pub const THUMBNAIL_SIDE_MAX: u32 = 448;

/// Longest side to decode a thumbnail at, given the logical width the tile is
/// drawn at and the display's scale factor.
///
/// The window is painted in physical pixels, so Windows at 125% draws that
/// 268-pixel tile across 335 of them: a thumbnail left at 268 would be visibly
/// soft there, and one always made at 360 would be sharp and 1.8x too big on the
/// 100% display that most of these machines run. Asking the display makes the
/// source match the destination at every scale.
#[cfg(test)]
pub fn thumbnail_side(tile_width: f32, scale_factor: f32) -> u32 {
    let scaled = (tile_width * scale_factor.max(1.0)).ceil();
    (scaled as u32).clamp(THUMBNAIL_SIDE, THUMBNAIL_SIDE_MAX)
}

/// How many screenshots the page will discover and draw.
///
/// A cap rather than a pager: the page's job is "show me what I took", and the
/// newest handful answers that. Anything older is still on disk and still
/// reachable through the instances folder.
#[cfg(test)]
pub const MAX_SHOWN: usize = 48;

/// One screenshot on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub struct Entry {
    /// The file itself.
    pub path: PathBuf,
    /// File name including extension, for the caption.
    pub name: String,
    /// Name of the instance it belongs to.
    pub instance: String,
    /// Last modification time, when the filesystem reports one.
    pub modified: Option<SystemTime>,
}

/// Whether a path looks like a screenshot this launcher will show.
///
/// Case-insensitive: Minecraft writes lower-case, but files copied from Windows
/// Explorer often are not.
#[cfg(test)]
pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let lower = extension.to_ascii_lowercase();
            IMAGE_EXTENSIONS.contains(&lower.as_str())
        })
        .unwrap_or(false)
}

/// The instance directories to search, as `(instance name, instance folder)`.
///
/// Built by the caller from the instance list, so this module needs no knowledge
/// of how instances are stored.
#[cfg(test)]
type InstanceDirs = [(String, PathBuf)];

/// Every screenshot under the given instances, newest first.
///
/// Ordered by modification time descending so the newest screenshot is the first
/// thing on the page. Ties — and files whose timestamp the filesystem will not
/// give up — fall back to the path, so the order is stable rather than dependent
/// on directory iteration, which is what makes it testable at all.
#[cfg(test)]
pub fn scan(instances: &InstanceDirs) -> Vec<Entry> {
    let mut found = Vec::new();
    for (instance, dir) in instances {
        let screenshots = dir.join("screenshots");
        let Ok(listing) = std::fs::read_dir(&screenshots) else {
            continue;
        };
        for entry in listing.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_image(&path) {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            found.push(Entry {
                path: path.clone(),
                name: name.to_string(),
                instance: instance.clone(),
                modified: entry.metadata().and_then(|meta| meta.modified()).ok(),
            });
        }
    }

    found.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.path.cmp(&b.path))
    });
    found.truncate(MAX_SHOWN);
    found
}

/// A decoded, shrunk screenshot: raw RGBA8 plus its dimensions.
///
/// Deliberately plain data rather than an iced `Handle`, so the decoding and
/// shrinking can be unit tested without a renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub struct Thumbnail {
    /// Width in pixels, after shrinking.
    pub width: u32,
    /// Height in pixels, after shrinking.
    pub height: u32,
    /// `width * height * 4` bytes of RGBA.
    pub pixels: Vec<u8>,
}

/// Decode `path` and shrink it so its longest side is at most `side` — the
/// value [`thumbnail_side`] worked out for the display this is being drawn on.
///
/// Shrinking only, never growing: `DynamicImage::thumbnail` scales *up* to fill
/// the box it is given, so a 64x48 screenshot — an old windowed capture, or a
/// test fixture — came back as 360x270. That wastes the decode, blurs the
/// picture and inflates the memory this module exists to bound.
///
/// `None` when the file is missing, unreadable or not an image format — a
/// screenshot that fails to decode must not take the page down with it.
#[cfg(test)]
pub fn thumbnail(path: &Path, side: u32) -> Option<Thumbnail> {
    let decoded = image::open(path).ok()?;
    let shrunk = if decoded.width().max(decoded.height()) <= side {
        decoded
    } else {
        decoded.thumbnail(side, side)
    };
    let rgba = shrunk.to_rgba8();
    Some(Thumbnail {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
    })
}

/// Load thumbnails for a batch of entries, skipping any that will not decode.
///
/// Returns them in the order given, so the grid stays newest-first.
#[cfg(test)]
pub fn thumbnails(entries: &[Entry], side: u32) -> Vec<(Entry, Thumbnail)> {
    entries
        .iter()
        .filter_map(|entry| thumbnail(&entry.path, side).map(|thumbnail| (entry.clone(), thumbnail)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a real PNG of the given size, so the decode path is exercised
    /// rather than mocked.
    fn write_png(path: &Path, width: u32, height: u32) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 200, 90, 255]));
        image.save(path).unwrap();
    }

    fn instance_dir(root: &Path, id: &str) -> (String, PathBuf) {
        let dir = root.join(id);
        std::fs::create_dir_all(dir.join("screenshots")).unwrap();
        (id.to_string(), dir)
    }

    #[test]
    fn only_image_extensions_are_screenshots() {
        assert!(is_image(Path::new("a/b/2026-09-12_20.11.03.png")));
        assert!(is_image(Path::new("SHOT.JPG")), "copied files are often upper-case");
        assert!(is_image(Path::new("x.jpeg")));
        assert!(!is_image(Path::new("options.txt")));
        assert!(!is_image(Path::new("screenshot")));
        assert!(!is_image(Path::new("shot.png.bak")));
    }

    #[test]
    fn nothing_found_is_an_empty_page_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan(&[]).is_empty());
        assert!(scan(&[("missing".to_string(), dir.path().join("nope"))]).is_empty());
        // An instance folder with no screenshots directory at all is normal.
        assert!(scan(&[("bare".to_string(), dir.path().to_path_buf())]).is_empty());
    }

    #[test]
    fn screenshots_are_collected_from_every_instance() {
        let root = tempfile::tempdir().unwrap();
        let first = instance_dir(root.path(), "survival");
        let second = instance_dir(root.path(), "creative");
        write_png(&first.1.join("screenshots/one.png"), 40, 30);
        write_png(&second.1.join("screenshots/two.png"), 40, 30);

        let found = scan(&[first, second]);
        assert_eq!(found.len(), 2);
        let names: Vec<&str> = found.iter().map(|entry| entry.name.as_str()).collect();
        assert!(names.contains(&"one.png") && names.contains(&"two.png"));
        // Each entry is labelled with the instance it came from.
        assert!(found.iter().any(|entry| entry.instance == "survival"));
        assert!(found.iter().any(|entry| entry.instance == "creative"));
    }

    #[test]
    fn non_images_and_directories_are_ignored() {
        let root = tempfile::tempdir().unwrap();
        let instance = instance_dir(root.path(), "survival");
        let shots = instance.1.join("screenshots");
        write_png(&shots.join("keep.png"), 20, 20);
        std::fs::write(shots.join("options.txt"), b"not a picture").unwrap();
        std::fs::create_dir_all(shots.join("older")).unwrap();

        let found = scan(&[instance]);
        assert_eq!(found.len(), 1, "only the PNG should be listed: {found:?}");
        assert_eq!(found[0].name, "keep.png");
    }

    #[test]
    fn the_page_is_bounded() {
        let root = tempfile::tempdir().unwrap();
        let instance = instance_dir(root.path(), "survival");
        for index in 0..(MAX_SHOWN + 12) {
            write_png(&instance.1.join(format!("screenshots/{index:03}.png")), 8, 8);
        }
        let found = scan(&[instance]);
        assert_eq!(
            found.len(),
            MAX_SHOWN,
            "discovery must respect the cap rather than building a page nobody can draw"
        );
    }

    #[test]
    fn the_newest_screenshot_comes_first() {
        let root = tempfile::tempdir().unwrap();
        let instance = instance_dir(root.path(), "survival");
        let shots = instance.1.join("screenshots");
        write_png(&shots.join("older.png"), 16, 16);
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_png(&shots.join("newer.png"), 16, 16);

        let found = scan(&[instance]);
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0].name, "newer.png",
            "the newest screenshot should lead the grid: {found:?}"
        );
    }

    #[test]
    fn thumbnails_are_shrunk_to_the_budget() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("big.png");
        // Rather than a true 1080p file, a small image with the same aspect: the
        // assertion is about the shrinking rule, not about codec throughput.
        write_png(&path, 800, 450);

        let side = THUMBNAIL_SIDE;
        let thumb = thumbnail(&path, side).expect("a real PNG must decode");
        assert!(
            thumb.width <= side && thumb.height <= side,
            "{}x{} exceeds the {side}px budget",
            thumb.width,
            thumb.height
        );
        assert_eq!(thumb.width, side, "the longest side should reach the budget");
        // 800x450 into a 268px longest side is 268x151; a pixel either way is
        // the resizer's rounding, a changed ratio is not.
        assert!(
            (thumb.height as i32 - 151).abs() <= 1,
            "aspect ratio must be preserved, got {}",
            thumb.height
        );
        assert_eq!(
            thumb.pixels.len(),
            (thumb.width * thumb.height * 4) as usize,
            "RGBA8 is four bytes per pixel"
        );
    }

    #[test]
    fn thumbnail_side_follows_the_display_scale() {
        // 100%: exactly the width the tile is drawn at, so the renderer has
        // nothing to resample.
        assert_eq!(thumbnail_side(268.0, 1.0), 268);
        // The two scalings Windows offers by default on most laptops, where an
        // unscaled thumbnail would be visibly soft.
        assert_eq!(thumbnail_side(268.0, 1.25), 335);
        assert_eq!(thumbnail_side(268.0, 1.5), 402);
        // A nonsense or absent scale factor falls back to the floor rather than
        // shrinking the picture below the size it is painted at.
        assert_eq!(thumbnail_side(268.0, 0.0), 268);
        assert_eq!(thumbnail_side(268.0, f32::NAN), 268, "NaN ignores, keeping the floor");
        // Past 200% the memory is not worth the sharpness, so it is clamped.
        assert_eq!(thumbnail_side(268.0, 2.0), THUMBNAIL_SIDE_MAX);
        assert_eq!(thumbnail_side(268.0, 4.0), THUMBNAIL_SIDE_MAX);
    }

    #[test]
    fn a_small_screenshot_is_not_blown_up() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("small.png");
        write_png(&path, 64, 48);
        let thumb = thumbnail(&path, THUMBNAIL_SIDE).unwrap();
        assert_eq!((thumb.width, thumb.height), (64, 48));
    }

    #[test]
    fn an_undecodable_file_is_skipped_rather_than_fatal() {
        let root = tempfile::tempdir().unwrap();
        let broken = root.path().join("broken.png");
        std::fs::write(&broken, b"\x89PNG not really").unwrap();
        assert!(thumbnail(&broken, THUMBNAIL_SIDE).is_none());
        assert!(thumbnail(&root.path().join("missing.png"), THUMBNAIL_SIDE).is_none());

        let entry = Entry {
            path: broken,
            name: "broken.png".to_string(),
            instance: "survival".to_string(),
            modified: None,
        };
        assert!(
            thumbnails(&[entry], THUMBNAIL_SIDE).is_empty(),
            "a broken file yields no tile"
        );
    }
}
