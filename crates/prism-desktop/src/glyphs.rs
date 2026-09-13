//! Vector glyphs for the launcher chrome, drawn in-process.
//!
//! Every UI symbol in the shell — the rail, the create dialog's rows, the small
//! inline buttons — is a tiny `canvas` program rather than an embedded bitmap.
//! Three reasons, in order of how much they matter:
//!
//! * **Tinting.** A bitmap is painted with the colours baked into it, so a
//!   themed button cannot recolour it. That is exactly why the carved-PNG
//!   icons looked identical whether a rail entry was selected or not, and why
//!   Prism's coloured artwork sat in an otherwise flat, monochrome shell. A
//!   canvas glyph takes its colour as a parameter, so one glyph serves the
//!   idle, hover and active states.
//! * **Crispness.** Strokes are rasterised at the size they are drawn, so a
//!   14px inline icon and a 22px rail icon are both sharp instead of a
//!   resampled bitmap.
//! * **Provenance.** The rail used to carve its icons out of the Prism
//!   Launcher binary. These are drawn from coordinates, so the launcher's own
//!   chrome needs no third-party art at all. (Prism's *instance* icons stay
//!   bitmap art on purpose: they are Minecraft-themed and users recognise
//!   them.)
//!
//! Glyphs are authored on a 24x24 grid — the usual icon grid — and scaled to
//! whatever square the caller asks for. Unknown names fall back to [`Glyph::Info`]
//! rather than panicking, so a typo in a view shows a neutral dot instead of
//! taking the window down.

use iced::widget::canvas::path::arc::{Arc, Elliptical};
use iced::widget::canvas::{
    self, Canvas, Frame, Geometry, LineCap, LineDash, LineJoin, Path, Stroke, Style,
};
use iced::{
    mouse::Cursor, Color, Element, Length, Point, Radians, Rectangle, Renderer, Theme, Vector,
};

use crate::app::Message;

/// The icon grid every glyph is authored on.
const GRID: f32 = 24.0;

/// A drawn UI symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// Play (home, "launch").
    Play,
    /// Compass (browse Modrinth).
    Compass,
    /// Isometric cube (mods).
    Cube,
    /// Globe (worlds).
    Globe,
    /// Terminal window (logs).
    Terminal,
    /// Framed picture (screenshots).
    Image,
    /// Slider bank (settings).
    Sliders,
    /// Head and shoulders (accounts).
    Person,
    /// Circled "i" (about).
    Info,
    /// Plus (create).
    Plus,
    /// Folder.
    Folder,
    /// Circular arrow (refresh / rescan).
    Refresh,
    /// Cross (close / cancel).
    Close,
    /// Single bar (minimise the window).
    Minimize,
    /// Hollow square (maximise the window).
    Maximize,
    /// Two offset squares (restore a maximised window).
    Restore,
    /// Tick (done / enabled).
    Check,
    /// Waste bin (delete).
    Trash,
    /// Pencil (edit).
    Edit,
    /// Two sheets (copy).
    Copy,
    /// Downward chevron (a dropdown).
    Chevron,
    /// Magnifier (search).
    Search,
    /// Arrow leaving a tray (upload).
    Upload,
    /// Arrow entering a tray (download / import).
    Download,
    /// Filled square (stop).
    Stop,
}

impl Glyph {
    /// Map an icon key to a glyph.
    ///
    /// Keys are the short names used by the views (`"play"`, `"gear"`, …) plus
    /// their obvious synonyms, so callers can keep passing the strings they
    /// already use.
    pub fn from_name(name: &str) -> Glyph {
        match name {
            "play" | "home" | "run" => Glyph::Play,
            "compass" | "browse" | "search-projects" | "update" => Glyph::Compass,
            "cube" | "mods" | "box" => Glyph::Cube,
            "globe" | "worlds" | "map" => Glyph::Globe,
            "terminal" | "logs" | "console" => Glyph::Terminal,
            "image" | "screenshots" | "screenshot" | "picture" | "photo" => Glyph::Image,
            "sliders" | "gear" | "settings" | "options" => Glyph::Sliders,
            "person" | "account" | "steve" | "user" => Glyph::Person,
            "info" | "about" | "help" => Glyph::Info,
            "plus" | "create" | "add" => Glyph::Plus,
            "folder" | "directory" => Glyph::Folder,
            "refresh" | "sync" | "reload" => Glyph::Refresh,
            "close" | "cancel" | "x" => Glyph::Close,
            "minimize" | "minimise" => Glyph::Minimize,
            "maximize" | "maximise" => Glyph::Maximize,
            "restore" | "unmaximize" => Glyph::Restore,
            "check" | "done" | "enabled" => Glyph::Check,
            "trash" | "delete" | "remove" => Glyph::Trash,
            "edit" | "pencil" | "rename" => Glyph::Edit,
            "copy" | "duplicate" => Glyph::Copy,
            "chevron" | "dropdown" => Glyph::Chevron,
            "search" | "find" => Glyph::Search,
            "upload" => Glyph::Upload,
            "download" | "import" => Glyph::Download,
            "stop" | "kill" | "minus" => Glyph::Stop,
            _ => Glyph::Info,
        }
    }

    /// Every key [`Glyph::from_name`] understands (used by the tests below).
    #[cfg(test)]
    pub fn names() -> [&'static str; 28] {
        [
            "play", "compass", "cube", "globe", "terminal", "sliders", "person", "info", "plus",
            "folder", "refresh", "close", "minimize", "maximize", "restore", "check", "trash",
            "edit", "copy", "chevron", "search", "upload", "download", "stop", "gear", "help",
            "update", "image",
        ]
    }
}

/// A size-and-colour bound glyph, ready to drop into a view.
pub fn glyph(name: &str, side: f32, color: Color) -> Element<'static, Message> {
    Canvas::new(Icon { glyph: Glyph::from_name(name), color })
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .into()
}

/// The canvas program behind [`glyph`].
struct Icon {
    glyph: Glyph,
    color: Color,
}

impl<Message> canvas::Program<Message> for Icon {
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
        // A square canvas: the glyph scales with the smaller side so a
        // non-square layout never stretches it.
        let side = bounds.width.min(bounds.height);
        draw(&mut frame, self.glyph, Ink { color: self.color, k: side / GRID });
        vec![frame.into_geometry()]
    }
}

/// Drawing surface: the grid-to-pixel scale plus the ink colour.
#[derive(Debug, Clone, Copy)]
struct Ink {
    color: Color,
    k: f32,
}

impl Ink {
    /// A grid point in pixels.
    fn p(&self, x: f32, y: f32) -> Point {
        Point::new(x * self.k, y * self.k)
    }

    /// A stroke of `width` grid units, round-capped so corners look drawn
    /// rather than cut. Never thinner than one pixel: an icon that vanishes is
    /// worse than an icon that is slightly heavy.
    fn pen(&self, width: f32) -> Stroke<'static> {
        Stroke {
            style: Style::Solid(self.color),
            width: (width * self.k).max(1.0),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            line_dash: LineDash::default(),
        }
    }

    /// Stroke an open polyline through `points`.
    fn poly(&self, frame: &mut Frame, width: f32, points: &[(f32, f32)]) {
        let ink = *self;
        let path = Path::new(move |builder| {
            let mut iter = points.iter();
            if let Some((x, y)) = iter.next() {
                builder.move_to(ink.p(*x, *y));
            }
            for (x, y) in iter {
                builder.line_to(ink.p(*x, *y));
            }
        });
        frame.stroke(&path, self.pen(width));
    }

    /// Fill the closed polygon through `points`.
    fn fill(&self, frame: &mut Frame, points: &[(f32, f32)]) {
        let ink = *self;
        let path = Path::new(move |builder| {
            let mut iter = points.iter();
            if let Some((x, y)) = iter.next() {
                builder.move_to(ink.p(*x, *y));
            }
            for (x, y) in iter {
                builder.line_to(ink.p(*x, *y));
            }
            builder.close();
        });
        frame.fill(&path, self.color);
    }

    /// Stroke a circle outline.
    fn ring(&self, frame: &mut Frame, width: f32, cx: f32, cy: f32, radius: f32) {
        let path = Path::circle(self.p(cx, cy), radius * self.k);
        frame.stroke(&path, self.pen(width));
    }

    /// Fill a circle.
    fn dot(&self, frame: &mut Frame, cx: f32, cy: f32, radius: f32) {
        let path = Path::circle(self.p(cx, cy), radius * self.k);
        frame.fill(&path, self.color);
    }

    /// Stroke a circular arc, clockwise from `from` to `to` (degrees).
    fn arc(&self, frame: &mut Frame, width: f32, cx: f32, cy: f32, radius: f32, from: f32, to: f32) {
        let ink = *self;
        let path = Path::new(move |builder| {
            builder.arc(Arc {
                center: ink.p(cx, cy),
                radius: radius * ink.k,
                start_angle: Radians(from.to_radians()),
                end_angle: Radians(to.to_radians()),
            });
        });
        frame.stroke(&path, self.pen(width));
    }

    /// Stroke a full ellipse outline (a globe's meridian).
    fn ellipse(&self, frame: &mut Frame, width: f32, cx: f32, cy: f32, rx: f32, ry: f32) {
        let ink = *self;
        let path = Path::new(move |builder| {
            builder.ellipse(Elliptical {
                center: ink.p(cx, cy),
                radii: Vector::new(rx * ink.k, ry * ink.k),
                rotation: Radians(0.0),
                start_angle: Radians(0.0),
                end_angle: Radians(std::f32::consts::TAU),
            });
        });
        frame.stroke(&path, self.pen(width));
    }

    /// A grid point on a circle around `(cx, cy)`.
    fn on_circle(cx: f32, cy: f32, radius: f32, degrees: f32) -> (f32, f32) {
        let radians = degrees.to_radians();
        (cx + radius * radians.cos(), cy + radius * radians.sin())
    }

    /// An arrowhead at `degrees` on a clockwise arc: the tip points the way the
    /// sweep travels, so it reads as rotation rather than a stray triangle.
    fn arrow_head(&self, frame: &mut Frame, cx: f32, cy: f32, radius: f32, degrees: f32) {
        let radians = degrees.to_radians();
        let (px, py) = Self::on_circle(cx, cy, radius, degrees);
        // Tangent (the direction of travel) and the radial direction, which is
        // perpendicular to it and therefore the base of the head.
        let (tx, ty) = (-radians.sin(), radians.cos());
        let (rx, ry) = (radians.cos(), radians.sin());
        const LENGTH: f32 = 4.4;
        const HALF_BASE: f32 = 2.9;
        self.fill(
            frame,
            &[
                (px + tx * LENGTH, py + ty * LENGTH),
                (px + rx * HALF_BASE, py + ry * HALF_BASE),
                (px - rx * HALF_BASE, py - ry * HALF_BASE),
            ],
        );
    }
}

/// Draw one glyph. Every shape is expressed on the 24x24 grid.
fn draw(frame: &mut Frame, glyph: Glyph, ink: Ink) {
    match glyph {
        Glyph::Play => ink.fill(frame, &[(8.0, 5.0), (19.2, 12.0), (8.0, 19.0)]),
        Glyph::Compass => {
            ink.ring(frame, 1.8, 12.0, 12.0, 8.6);
            ink.fill(frame, &[(12.0, 6.4), (14.8, 13.6), (12.0, 17.6), (9.2, 10.4)]);
        }
        Glyph::Cube => {
            ink.poly(
                frame,
                1.7,
                &[
                    (12.0, 2.8),
                    (20.0, 7.4),
                    (20.0, 16.6),
                    (12.0, 21.2),
                    (4.0, 16.6),
                    (4.0, 7.4),
                    (12.0, 2.8),
                ],
            );
            ink.poly(frame, 1.7, &[(4.0, 7.4), (12.0, 12.2), (20.0, 7.4)]);
            ink.poly(frame, 1.7, &[(12.0, 12.2), (12.0, 21.2)]);
        }
        Glyph::Globe => {
            ink.ring(frame, 1.7, 12.0, 12.0, 8.6);
            ink.ellipse(frame, 1.7, 12.0, 12.0, 4.2, 8.6);
            ink.poly(frame, 1.7, &[(3.4, 12.0), (20.6, 12.0)]);
        }
        Glyph::Terminal => {
            ink.poly(
                frame,
                1.7,
                &[(3.6, 5.2), (20.4, 5.2), (20.4, 18.8), (3.6, 18.8), (3.6, 5.2)],
            );
            ink.poly(frame, 1.7, &[(7.6, 10.0), (10.4, 12.6), (7.6, 15.2)]);
            ink.poly(frame, 1.7, &[(13.0, 15.2), (17.0, 15.2)]);
        }
        Glyph::Image => {
            // A frame, a sun and two hills: the same picture-in-a-box every
            // desktop reads as "image" without a caption.
            ink.poly(
                frame,
                1.7,
                &[(3.6, 5.4), (20.4, 5.4), (20.4, 18.6), (3.6, 18.6), (3.6, 5.4)],
            );
            ink.dot(frame, 8.8, 9.8, 1.5);
            ink.poly(
                frame,
                1.7,
                &[(4.6, 17.2), (9.8, 12.2), (12.8, 15.2), (15.4, 12.6), (19.4, 17.2)],
            );
        }
        Glyph::Sliders => {
            ink.poly(frame, 1.7, &[(4.2, 7.0), (19.8, 7.0)]);
            ink.poly(frame, 1.7, &[(4.2, 12.0), (19.8, 12.0)]);
            ink.poly(frame, 1.7, &[(4.2, 17.0), (19.8, 17.0)]);
            ink.dot(frame, 9.0, 7.0, 2.3);
            ink.dot(frame, 15.4, 12.0, 2.3);
            ink.dot(frame, 8.2, 17.0, 2.3);
        }
        Glyph::Person => {
            ink.ring(frame, 1.8, 12.0, 8.0, 3.9);
            let ink2 = ink;
            let path = Path::new(move |builder| {
                builder.move_to(ink2.p(4.6, 20.4));
                builder.quadratic_curve_to(ink2.p(12.0, 13.0), ink2.p(19.4, 20.4));
            });
            frame.stroke(&path, ink.pen(1.8));
        }
        Glyph::Info => {
            ink.ring(frame, 1.8, 12.0, 12.0, 8.6);
            ink.dot(frame, 12.0, 7.6, 1.5);
            ink.poly(frame, 2.0, &[(12.0, 11.2), (12.0, 16.8)]);
        }
        Glyph::Plus => {
            ink.poly(frame, 2.0, &[(12.0, 5.4), (12.0, 18.6)]);
            ink.poly(frame, 2.0, &[(5.4, 12.0), (18.6, 12.0)]);
        }
        Glyph::Folder => ink.poly(
            frame,
            1.7,
            &[
                (3.6, 6.6),
                (9.8, 6.6),
                (11.8, 9.6),
                (20.4, 9.6),
                (20.4, 18.4),
                (3.6, 18.4),
                (3.6, 6.6),
            ],
        ),
        Glyph::Refresh => {
            // A ring opened at the upper right, with the head closing the gap.
            ink.arc(frame, 1.8, 12.0, 12.0, 7.4, 20.0, 320.0);
            ink.arrow_head(frame, 12.0, 12.0, 7.4, 320.0);
        }
        Glyph::Close => {
            ink.poly(frame, 2.0, &[(6.4, 6.4), (17.6, 17.6)]);
            ink.poly(frame, 2.0, &[(17.6, 6.4), (6.4, 17.6)]);
        }
        Glyph::Minimize => ink.poly(frame, 2.0, &[(6.4, 12.0), (17.6, 12.0)]),
        Glyph::Maximize => ink.poly(
            frame,
            2.0,
            &[(6.6, 6.6), (17.4, 6.6), (17.4, 17.4), (6.6, 17.4), (6.6, 6.6)],
        ),
        Glyph::Restore => {
            // The two squares of the Windows "restore" icon, with only the
            // visible edges of the back one drawn so they do not overlap.
            ink.poly(frame, 1.8, &[(9.0, 4.8), (19.2, 4.8), (19.2, 15.0)]);
            ink.poly(
                frame,
                1.8,
                &[(4.8, 9.0), (15.0, 9.0), (15.0, 19.2), (4.8, 19.2), (4.8, 9.0)],
            );
        }
        Glyph::Check => ink.poly(frame, 2.0, &[(5.0, 12.6), (10.0, 17.6), (19.2, 6.8)]),
        Glyph::Trash => {
            ink.poly(frame, 1.7, &[(4.2, 7.0), (19.8, 7.0)]);
            ink.poly(
                frame,
                1.7,
                &[(6.4, 7.0), (7.4, 20.2), (16.6, 20.2), (17.6, 7.0)],
            );
            ink.poly(frame, 1.6, &[(9.6, 4.2), (14.4, 4.2)]);
            ink.poly(frame, 1.6, &[(10.2, 10.4), (10.6, 17.0)]);
            ink.poly(frame, 1.6, &[(13.8, 10.4), (13.4, 17.0)]);
        }
        Glyph::Edit => ink.poly(
            frame,
            1.7,
            &[
                (4.0, 20.0),
                (5.4, 14.6),
                (15.0, 5.0),
                (19.0, 9.0),
                (9.4, 18.6),
                (4.0, 20.0),
            ],
        ),
        Glyph::Copy => {
            ink.poly(
                frame,
                1.7,
                &[(9.0, 4.2), (19.8, 4.2), (19.8, 15.0), (9.0, 15.0), (9.0, 4.2)],
            );
            ink.poly(
                frame,
                1.7,
                &[(4.2, 9.0), (15.0, 9.0), (15.0, 19.8), (4.2, 19.8), (4.2, 9.0)],
            );
        }
        Glyph::Chevron => ink.poly(frame, 2.0, &[(6.2, 9.6), (12.0, 15.4), (17.8, 9.6)]),
        Glyph::Search => {
            ink.ring(frame, 1.8, 10.6, 10.6, 6.4);
            ink.poly(frame, 2.0, &[(15.4, 15.4), (20.4, 20.4)]);
        }
        Glyph::Upload => {
            ink.poly(frame, 1.9, &[(12.0, 20.0), (12.0, 6.0)]);
            ink.poly(frame, 1.9, &[(6.8, 11.4), (12.0, 5.8), (17.2, 11.4)]);
            ink.poly(frame, 1.7, &[(4.6, 20.4), (19.4, 20.4)]);
        }
        Glyph::Download => {
            ink.poly(frame, 1.9, &[(12.0, 4.6), (12.0, 17.0)]);
            ink.poly(frame, 1.9, &[(6.8, 11.8), (12.0, 17.4), (17.2, 11.8)]);
            ink.poly(frame, 1.7, &[(4.6, 20.4), (19.4, 20.4)]);
        }
        Glyph::Stop => ink.fill(
            frame,
            &[(7.0, 7.0), (17.0, 7.0), (17.0, 17.0), (7.0, 17.0)],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_name_maps_to_its_glyph() {
        let expected = [
            ("play", Glyph::Play),
            ("compass", Glyph::Compass),
            ("cube", Glyph::Cube),
            ("globe", Glyph::Globe),
            ("terminal", Glyph::Terminal),
            ("gear", Glyph::Sliders),
            ("person", Glyph::Person),
            ("info", Glyph::Info),
            ("plus", Glyph::Plus),
            ("folder", Glyph::Folder),
            ("refresh", Glyph::Refresh),
            ("close", Glyph::Close),
            ("minimize", Glyph::Minimize),
            ("maximize", Glyph::Maximize),
            ("check", Glyph::Check),
            ("trash", Glyph::Trash),
            ("edit", Glyph::Edit),
            ("copy", Glyph::Copy),
            ("chevron", Glyph::Chevron),
            ("search", Glyph::Search),
            ("upload", Glyph::Upload),
            ("download", Glyph::Download),
            ("stop", Glyph::Stop),
        ];
        for (name, glyph) in expected {
            assert_eq!(Glyph::from_name(name), glyph, "key '{name}'");
        }
        // Everything advertised in `names()` must resolve to a real glyph; the
        // table above is what actually checks it, so this only guards against
        // a name silently disappearing from `names()`.
        let advertised: Vec<Glyph> = Glyph::names().iter().map(|n| Glyph::from_name(n)).collect();
        assert!(advertised.contains(&Glyph::Info));
        assert!(advertised.contains(&Glyph::Stop));
    }

    #[test]
    fn rail_keys_are_distinct() {
        // The rail must not show the same symbol twice (Prism's keys used to
        // collapse several of these onto one fallback bitmap).
        let rail = ["play", "compass", "cube", "globe", "terminal", "gear", "person", "info"];
        let mut seen = Vec::new();
        for name in rail {
            let glyph = Glyph::from_name(name);
            assert!(!seen.contains(&glyph), "'{name}' collides with another rail glyph");
            seen.push(glyph);
        }
    }

    #[test]
    fn synonyms_and_strangers_are_handled() {
        assert_eq!(Glyph::from_name("gear"), Glyph::Sliders);
        assert_eq!(Glyph::from_name("settings"), Glyph::Sliders);
        assert_eq!(Glyph::from_name("kill"), Glyph::Stop);
        // Window controls have no bitmap ancestors: they were unicode text
        // (`—`, `▢`, `✕`) until the glyph set grew these three.
        assert_eq!(Glyph::from_name("minimize"), Glyph::Minimize);
        assert_eq!(Glyph::from_name("maximize"), Glyph::Maximize);
        assert_eq!(Glyph::from_name("restore"), Glyph::Restore);
        assert_ne!(Glyph::from_name("minimize"), Glyph::from_name("maximize"));
        // Restore is its own mark, not the maximize square again: a maximize
        // button that does not change when the window is maximized is the bug
        // this exists for.
        assert_ne!(Glyph::from_name("restore"), Glyph::from_name("maximize"));
        assert_ne!(Glyph::from_name("minimize"), Glyph::Stop);
        // Unknown input is a neutral dot, never a panic.
        assert_eq!(Glyph::from_name(""), Glyph::Info);
        assert_eq!(Glyph::from_name("nonsense"), Glyph::Info);
    }

    #[test]
    fn arrow_head_sits_on_the_circle() {
        let (x, y) = Ink::on_circle(12.0, 12.0, 8.0, 0.0);
        assert!((x - 20.0).abs() < 0.001 && (y - 12.0).abs() < 0.001);
    }
}
