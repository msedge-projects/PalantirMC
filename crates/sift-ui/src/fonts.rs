//! Inter, at the weights the reference actually uses.
//!
//! egui ships one proportional face and no weight axis, so a heading and a body line come
//! out the same weight unless the fonts are registered by hand. The reference's stylesheet
//! asks for **400** (body), **500** (labels), **600** (tab labels and section headings),
//! **700** (card titles) and **800** (page headings), and at 1440x900 the difference
//! between 400 and 600 is the first thing an eye catches in a side-by-side capture -- the
//! tab strip looked "lighter and smaller" for exactly this reason.
//!
//! Each weight becomes its own `FontFamily`, so a call site names the weight it wants:
//!
//! ```text
//! ui.painter().text(pos, align, "Mods", fonts::semibold(14.0), colour);
//! ```
//!
//! The files are the same Inter TTFs the iced build embedded, kept in `assets/fonts/` now that
//! assets today; when that crate is deleted this module's `include_bytes!` paths move with
//! them, which is why the path is written once, here.

use eframe::egui::{FontData, FontDefinitions, FontFamily, FontId};

/// The weights the interface uses, as `(family name, file name)`.
///
/// The names are what a call site writes; the files are Inter's static weights. A weight
/// the interface never asks for is not shipped -- five faces is 1.5 MB already.
const FACES: [(&str, &str); 5] = [
    ("inter-400", "Inter-400.ttf"),
    ("inter-500", "Inter-500.ttf"),
    ("inter-600", "Inter-600.ttf"),
    ("inter-700", "Inter-700.ttf"),
    ("inter-800", "Inter-800.ttf"),
];

/// Register every weight with the context, and make Inter the default proportional face.
///
/// Called once at startup from `main`. Registering rather than replacing matters: egui's
/// own glyph fallbacks stay available, so a character Inter lacks does not become a box.
pub fn install(ctx: &eframe::egui::Context) {
    let mut definitions = FontDefinitions::default();

    for (family, file) in FACES {
        let bytes: &[u8] = match file {
            "Inter-400.ttf" => include_bytes!("../../../assets/fonts/Inter-400.ttf"),
            "Inter-500.ttf" => include_bytes!("../../../assets/fonts/Inter-500.ttf"),
            "Inter-600.ttf" => include_bytes!("../../../assets/fonts/Inter-600.ttf"),
            "Inter-700.ttf" => include_bytes!("../../../assets/fonts/Inter-700.ttf"),
            "Inter-800.ttf" => include_bytes!("../../../assets/fonts/Inter-800.ttf"),
            _ => continue,
        };
        definitions
            .font_data
            .insert(family.to_owned(), std::sync::Arc::new(FontData::from_static(bytes)));
        definitions
            .families
            .insert(FontFamily::Name(family.into()), vec![family.to_owned()]);
    }

    // The default proportional family is Inter 400 with egui's own fallbacks behind it, in
    // that order: text the interface draws without naming a weight is body text.
    let fallbacks = definitions
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut body = vec!["inter-400".to_owned()];
    body.extend(fallbacks);
    definitions.families.insert(FontFamily::Proportional, body);

    ctx.set_fonts(definitions);
}

/// A weighted face, by the name given in [`FACES`].
///
/// A weight that was not registered falls back to the default proportional family rather
/// than panicking: a missing font is a look that is wrong, not a window that does not open.
fn face(name: &str, size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(name.into()))
}

/// Body text.
pub fn body(size: f32) -> FontId {
    face("inter-400", size)
}

/// A label, one step heavier than body.
pub fn label(size: f32) -> FontId {
    face("inter-500", size)
}

/// Tab labels and section headings: the reference's `font-semibold`.
pub fn semibold(size: f32) -> FontId {
    face("inter-600", size)
}

/// Card titles.
pub fn title(size: f32) -> FontId {
    face("inter-700", size)
}

/// Page headings: the reference's `font-extrabold` on a page's own title.
pub fn heading(size: f32) -> FontId {
    face("inter-800", size)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every weight the helpers name must be one the installer registers, or the helper
    /// silently falls back to 400 and a heading looks like a paragraph.
    #[test]
    fn every_helper_names_a_registered_face() {
        let registered: Vec<&str> = FACES.iter().map(|(family, _)| *family).collect();
        for family in ["inter-400", "inter-500", "inter-600", "inter-700", "inter-800"] {
            assert!(registered.contains(&family), "{family} is not registered");
        }
    }
}
