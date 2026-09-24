//! Drawing one of the reference's icons into a widget.
//!
//! [`crate::icons_gen`] holds the geometry; this is the twelve lines that put
//! it on screen, and it is a module of its own because it is the one place that
//! knows the two facts a caller cannot guess:
//!
//! * **The scale goes in two places, not one.** The geometry is in the icon's
//!   own view box, so the frame's transform has to scale the *positions*; but
//!   neither of iced's two geometry backends scales a stroke width with the
//!   frame -- both transform the path and then tessellate it at the width they
//!   were handed. So the width is pre-scaled by [`crate::icons_gen::parts`],
//!   and a caller that scaled only the frame would draw a 16px icon with a
//!   24px icon's 2px stroke: every edge a third too heavy.
//! * **The reference is always `xMidYMid meet`.** None of the 313 vendored SVGs
//!   declares `preserveAspectRatio`, so all of them take the SVG default, which
//!   is a uniform scale that fits and centres. [`fit`] is that rule.

use iced::widget::canvas::{self, Canvas, Frame, Geometry};
use iced::{mouse::Cursor, Color, Element, Length, Rectangle, Renderer, Theme, Vector};

use crate::icons_gen::{self, Glyph, Paint};

/// Where a glyph's view box lands inside a `size`-pixel box: uniform scale,
/// centred, the whole icon visible.
///
/// Public because a caller laying a row of icons out may want the icon's real
/// footprint rather than the box it was given -- an icon whose view box is not
/// square draws narrower than its box, and the gap beside it should include
/// that.
pub fn fit(glyph: Glyph, size: f32) -> (f32, Vector) {
    let (width, height) = glyph.view_box();
    if width <= 0.0 || height <= 0.0 || size <= 0.0 {
        return (1.0, Vector::new(0.0, 0.0));
    }
    let scale = (size / width).min(size / height);
    let (drawn_width, drawn_height) = (width * scale, height * scale);
    (
        scale,
        Vector::new((size - drawn_width) * 0.5, (size - drawn_height) * 0.5),
    )
}

/// One icon, filling a `size`-pixel box, drawn in `ink`.
///
/// `ink` is what the reference's `currentColor` resolves to, so an icon takes
/// the colour of the control it sits in. It is a parameter rather than a
/// property of the widget for the reason the icons are geometry and not
/// bitmaps: a bitmap has one colour and one size.
pub fn icon<'a, Message: 'a>(glyph: Glyph, size: f32, ink: Color) -> Element<'a, Message> {
    Canvas::new(Picture { glyph, size, ink })
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}

/// The canvas program behind [`icon`].
struct Picture {
    glyph: Glyph,
    size: f32,
    ink: Color,
}

impl<Message> canvas::Program<Message> for Picture {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        // Drawn against the box that was *given*, not the one asked for: a row
        // that ran out of space hands a canvas fewer pixels than it wanted, and
        // an icon drawn at its requested size would spill out of its slot.
        let size = bounds.width.min(bounds.height).min(self.size);
        let (scale, offset) = fit(self.glyph, size);
        let mut frame = Frame::new(renderer, bounds.size());
        frame.translate(offset);
        frame.scale(scale);
        for (path, paint) in icons_gen::parts(self.glyph, scale, self.ink) {
            match paint {
                Paint::Stroke(stroke) => frame.stroke(&path, stroke),
                Paint::Fill(color) => frame.fill(&path, color),
            }
        }
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pixel of slack: `f32` division of 16 by 24 is not exactly two thirds.
    const SLACK: f32 = 1e-4;

    #[test]
    fn an_icon_fits_its_box_and_touches_it() {
        // The SVG default, checked over all 313 rather than asserted for one:
        // the icon is never drawn larger than the box in either direction, and
        // it is never drawn smaller than the box in *both* -- `meet` means the
        // icon touches one pair of edges and has margins on the other.
        for glyph in icons_gen::ALL.iter().map(|(_, glyph)| *glyph) {
            let (width, height) = glyph.view_box();
            for size in [16.0, 20.0, 24.0, 48.0] {
                let (scale, offset) = fit(glyph, size);
                let (drawn_width, drawn_height) = (width * scale, height * scale);
                assert!(drawn_width <= size + SLACK, "{} wide in {size}", glyph.name());
                assert!(drawn_height <= size + SLACK, "{} tall in {size}", glyph.name());
                let touches = (drawn_width - size).abs() < SLACK || (drawn_height - size).abs() < SLACK;
                assert!(touches, "{} does not reach the edge of {size}", glyph.name());
                // Centred: the margins on the two axes add up to what is left.
                assert!(
                    (offset.x * 2.0 - (size - drawn_width)).abs() < SLACK,
                    "{} is not centred horizontally in {size}",
                    glyph.name()
                );
                assert!(
                    (offset.y * 2.0 - (size - drawn_height)).abs() < SLACK,
                    "{} is not centred vertically in {size}",
                    glyph.name()
                );
                assert!(offset.x >= -SLACK && offset.y >= -SLACK, "{} is off its box", glyph.name());
            }
        }
    }

    #[test]
    fn a_twenty_four_unit_icon_is_drawn_identity_at_twenty_four_pixels() {
        // The common case, and the one every rail icon is: 305 of the 313 SVGs
        // declare `viewBox="0 0 24 24"`, and the rail draws them at 24px. Scale
        // 1 and offset 0 is what makes the stroke widths in the generated table
        // the reference's own numbers rather than something scaled.
        let (scale, offset) = fit(Glyph::Home, 24.0);
        assert_eq!(scale, 1.0);
        assert_eq!(offset, Vector::new(0.0, 0.0));
        // Two thirds at 16px, which is the head's chevrons.
        let (scale, offset) = fit(Glyph::Home, 16.0);
        assert!((scale - 2.0 / 3.0).abs() < SLACK);
        assert_eq!(offset, Vector::new(0.0, 0.0));
    }

    #[test]
    fn every_icon_has_something_to_draw() {
        // Not a formality: `parts` walks the generated element table, so this
        // visits all 313 icons and every one of their 1105 elements, and would
        // fail on an icon the generator emitted as empty or on a path builder
        // that panicked. `Paint` has no `PartialEq`, so the check is a
        // destructure rather than a comparison.
        let mut strokes = 0;
        let mut fills = 0;
        // Which icons are drawn filled rather than stroked, and with how many
        // filled elements each. Nine of the 313 are filled and two of those
        // nine carry two filled elements rather than one, which is where 11
        // fill paints over 9 icons comes from -- so a count on its own would
        // not say which icon grew a fill. The names are what the generator was
        // fixed to find (`x.svg` inherits its fill from the root `<svg>` and
        // `spinner.svg` fills its ring and its head separately), and a count
        // would let one of them lose its fill while another gained one.
        let mut filled: Vec<(&'static str, usize)> = Vec::new();
        for (name, glyph) in icons_gen::ALL {
            let parts = icons_gen::parts(*glyph, 1.0, Color::WHITE);
            assert!(!parts.is_empty(), "{name} draws nothing");
            assert_eq!(parts.len(), glyph.elements().len(), "{name} lost an element");
            let mut icon_fills = 0;
            for (_, paint) in parts {
                match paint {
                    Paint::Stroke(stroke) => {
                        assert!(stroke.width > 0.0, "{name} strokes at width zero");
                        strokes += 1;
                    }
                    Paint::Fill(_) => {
                        fills += 1;
                        icon_fills += 1;
                    }
                }
            }
            if icon_fills > 0 {
                filled.push((name, icon_fills));
            }
        }
        assert_eq!(strokes + fills, 1105, "the element count moved");
        assert_eq!(fills, 11, "eleven elements are filled");
        assert_eq!(filled.len(), 9, "over nine icons");
        let mut names: Vec<&str> = filled.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            vec![
                "cloud",
                "cog",
                "cube",
                "images",
                "online-indicator",
                "radio-button",
                "radio-button-checked",
                "spinner",
                "x",
            ]
        );
        let mut doubled: Vec<&str> = filled
            .iter()
            .filter(|(_, count)| *count == 2)
            .map(|(name, _)| *name)
            .collect();
        doubled.sort_unstable();
        assert_eq!(doubled, vec!["online-indicator", "radio-button-checked"]);
    }

    #[test]
    fn the_stroke_width_is_scaled_by_the_caller_and_not_by_the_frame() {
        // The reason `fit`'s scale is handed to `parts` as well as to the
        // frame. This is a statement about iced's two backends rather than
        // about the reference: both transform the path and then tessellate it
        // at the width they were given, so a stroke declared at 2 draws at 2
        // whatever the frame is scaled to. Asserting it here keeps the two
        // halves of the scaling from being consolidated into one by a later
        // tidy-up, which would draw every small icon too heavy.
        let at_one = icons_gen::parts(Glyph::Home, 1.0, Color::WHITE);
        let at_two_thirds = icons_gen::parts(Glyph::Home, 2.0 / 3.0, Color::WHITE);
        let width = |parts: &[(iced::widget::canvas::Path, Paint)]| match &parts[0].1 {
            Paint::Stroke(stroke) => stroke.width,
            Paint::Fill(_) => panic!("the first element of an icon is a stroke"),
        };
        assert!((width(&at_two_thirds) - width(&at_one) * 2.0 / 3.0).abs() < SLACK);
    }
}
