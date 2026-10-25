//! The content column's search field: a plate, a magnifier, a placeholder and a caret.
//!
//! It is what the search row in [`crate::shell`] used to paint by hand -- the rounded
//! plate, the ring glyph and the `Search modpacks...` text -- with the difference that the
//! plate now takes the keyboard, holds what is typed and reports it as a query.
//!
//! **Why a field drawn by hand rather than [`egui::TextEdit`].** The reference's field is
//! not a widget: it is a plate in `--color-raised-bg` at `--radius-sm`, with its text
//! inset 34 points from the left, a magnifier glyph at 18, and a clear `X` at the right
//! that exists only once there is something to clear. `TextEdit` arrives with its own
//! frame, its own margin, its own text inset, its own spacing below the field, and its own
//! opinion about when a button belongs beside the text. Matching those numbers means
//! overriding `TextEdit`'s frame, margin and text colour in every state it has -- idle,
//! hovered, focused, disabled -- and that override is not just more code than the four
//! draws below: it is a patch that silently un-matches the day egui changes a default,
//! with a capture as the only thing that would notice.
//!
//! The tradeoff is real and it is the honest half of the decision: `TextEdit` also brings
//! selection, clipboard, IME pre-edit, word-wise motion, horizontal scrolling and a caret
//! that is positioned by the shaper. This field has none of those yet. It appends, deletes
//! one character, gives the keyboard back and blinks. Those gaps are owed work, not
//! arguments that the hand-rolled field is finished.
//!
//! **The one widget in this crate that keeps the frame loop alive.** Every other widget
//! here reports "nothing in flight" and the window stops drawing, which is the idle-CPU
//! budget in `docs/LIGHTNESS.md`. A caret cannot behave that way: its blink changes with
//! wall-clock time and no input event will arrive to announce the change, so the only way
//! to see the second half of the cycle is to draw the next frame. [`Search::ui`] therefore
//! returns whether a frame is still **owed** -- `true` exactly while the field has focus,
//! `false` otherwise -- and the caller turns a `true` into a repaint request. It is a
//! deliberate exception, it is bounded by the field's focus, and it stops the moment the
//! field lets go of the keyboard. (The cheaper refinement, when it is worth doing, is
//! `Context::request_repaint_after` aimed at the next half-period rather than a frame per
//! vsync; the return value is the seam that would change, not the widget.)

use egui::{Id, Rect, Sense, Stroke, Vec2};

use crate::fonts;
use crate::icons::{self, Glyph};
use crate::theme::Theme;

/// The hint an empty field shows. The reference's own string, ellipsis included.
const PLACEHOLDER: &str = "Search modpacks...";

/// A single-line search field, as the content column's search row draws it.
///
/// The state is the text and the caret's phase. Everything else -- the plate, the glyph,
/// the placeholder, the clear button -- is a function of those two, the theme and the
/// pointer, which is why none of it is stored.
#[derive(Default)]
pub struct Search {
    /// The query as typed, untrimmed.
    ///
    /// The leading and trailing spaces are kept on purpose: dropping them as they arrive
    /// would eat the space between two words the reader is still typing, and
    /// [`Search::query`] is where the trimming belongs, once, at the point of use.
    pub text: String,
    /// Whether the field is taking keystrokes. This is also the frame debt -- see the
    /// module documentation for why focus and "owes a frame" are the same fact.
    focused: bool,
    /// Seconds since the field last took focus, used only as a blink **phase**:
    /// `caret.fract() < 0.5` is the visible half of the cycle.
    caret: f32,
}

impl Search {
    /// Draw the field and take its input. Returns whether a frame is still owed.
    ///
    /// `rect` is the whole control -- the plate the shell measured for the search row --
    /// in the same coordinate space as `ui`'s painter. A `true` return means the caller
    /// must request a repaint: the caret's blink is a function of time, so nothing else in
    /// the frame will ask for the next one.
    pub fn ui(&mut self, ui: &mut egui::Ui, rect: Rect, theme: &Theme) -> bool {
        // `click_and_drag` rather than `click`: a field is a place to put a caret, and a
        // press that turns into a drag across the plate should still take the keyboard
        // rather than be abandoned into the list underneath.
        let response = ui.interact(rect, Id::new("search-field"), Sense::click_and_drag());
        if response.clicked() {
            self.focus();
        }

        // A press anywhere else gives the keyboard back. Read from the pointer rather than
        // from egui's focus system because this field is not an `egui` widget with an id
        // in that system: the single field in the window needs no focus graph, and adding
        // one would be a second source of truth for the same bool.
        let pressed_outside = ui.input(|i| {
            i.pointer.any_pressed() && i.pointer.hover_pos().is_some_and(|at| !rect.contains(at))
        });
        if pressed_outside {
            self.focused = false;
        }

        if self.focused {
            // The phase advances with the frame clock, not with input: a blink is the one
            // thing here that has to happen while the reader does nothing at all.
            self.caret += ui.input(|i| i.stable_dt);

            // The event list is cloned rather than drained with `input_mut` + `retain`:
            // swallowing events here would hide the keystrokes of whatever widget draws
            // later in the frame, and this plate is one control in a row of them.
            //
            // Events are replayed in the order they arrived, which is the order the keys
            // were pressed: a fast typist's two characters land in one frame's list, and
            // applying them in list order is what keeps the field equal to the keyboard.
            for event in ui.input(|i| i.events.clone()) {
                match event {
                    // Composed characters arrive as text, which is why the character is
                    // appended rather than a key being mapped: a dead-key `é` is one
                    // `Text` event and no key at all.
                    egui::Event::Text(text) => self.apply(Some(text.as_str()), false),
                    // `pressed` is required for the editing keys: egui sends a release as
                    // well, and deleting two characters per press is a bug nobody looks
                    // for twice.
                    egui::Event::Key {
                        key: egui::Key::Backspace,
                        pressed: true,
                        ..
                    } => self.apply(None, true),
                    // Enter leaves the text alone and gives the keyboard back. The query is
                    // already the field's whole answer -- there is nothing to submit -- but
                    // the list under it should stop taking edits once the reader is done.
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        pressed: true,
                        ..
                    } => {
                        self.focused = false;
                        // The rest of this frame's events belong to whatever has the
                        // keyboard next; replaying them into a field that just let go
                        // would strand characters where nobody can see them.
                        break;
                    }
                    // Escape matches its release as well as its press, which is harmless:
                    // letting go twice is idempotent, and a release whose press went to
                    // another window should still be allowed to end the edit. Once
                    // unfocused, the same `break` reasoning applies.
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        ..
                    } => {
                        self.focused = false;
                        break;
                    }
                    _ => {}
                }
            }
        }

        // The clear button: a 16-point target at the plate's right end, drawn and
        // interacted with only while there is something to clear *and* the pointer is on
        // the field. An always-present invisible target that empties the field is a trap,
        // and the reference hides it for the same reason.
        let clear = Rect::from_center_size(
            egui::pos2(rect.right() - 18.0, rect.center().y),
            Vec2::splat(16.0),
        );
        let clear_shown = !self.text.is_empty() && response.hovered();
        let mut clear_hovered = false;
        if clear_shown {
            let hit = ui.interact(clear, Id::new("search-clear"), Sense::click());
            clear_hovered = hit.hovered();
            if hit.clicked() {
                self.text.clear();
                // Clearing is a step in typing the next query, not the end of it: a field
                // that dropped the keyboard here would need a second click to be used
                // again. `focus` also restarts the blink, so the caret is visible at the
                // start of the empty field it was just emptied into.
                self.focus();
            }
        }

        let painter = ui.painter();

        // The plate. One fill at every state, on purpose: the reference gives this control
        // no hover or focus wash. What tells a reader where the keyboard is, is the caret.
        painter.rect_filled(rect, crate::theme::RADIUS_SM, theme.RAISED_BG);

        // The magnifier, where the painted placeholder used to draw its ring: a 12-point
        // glyph centred 18 points in from the plate's left edge.
        icons::draw(
            painter,
            Glyph::Search,
            Rect::from_center_size(
                egui::pos2(rect.left() + 18.0, rect.center().y),
                Vec2::splat(12.0),
            ),
            theme.SECONDARY,
        );

        // The text's own left inset: 34 is where the reference's placeholder starts, which
        // is 18 for the glyph's centre, 6 for its radius, and a 10-point gap.
        let text_left = rect.left() + 34.0;
        let text_right = if clear_shown {
            // The clear button's target plus a gap, so a long query does not run under the
            // X that would delete it.
            clear.left() - 8.0
        } else {
            rect.right() - 12.0
        };
        // Clipped rather than truncated or wrapped: this is a one-line control, and text
        // that outgrows it should disappear at the plate's edge, exactly as the reference's
        // input does, rather than spill over the sort selects beside it.
        let text_painter = painter.with_clip_rect(Rect::from_min_max(
            egui::pos2(text_left, rect.top()),
            egui::pos2(text_right, rect.bottom()),
        ));

        let (contents, ink) = if self.text.is_empty() {
            (PLACEHOLDER, theme.SECONDARY)
        } else {
            (self.text.as_str(), theme.BASE)
        };
        let galley = text_painter.layout_no_wrap(contents.to_owned(), fonts::body(13.0), ink);
        text_painter.galley(
            egui::pos2(text_left, rect.center().y - galley.size().y / 2.0),
            galley.clone(),
            ink,
        );

        if self.focused && self.caret.fract() < 0.5 {
            // Measured from the value, not from what is on screen: with an empty field the
            // placeholder is a hint, and a caret drawn after the hint would sit where no
            // character can ever be.
            let typed = if self.text.is_empty() { 0.0 } else { galley.size().x };
            // One point inside the text's right limit, so a caret at the end of a full
            // field is still drawn instead of being clipped away with the overflow.
            let x = (text_left + typed).min(text_right - 1.0);
            text_painter.vline(
                x,
                (rect.center().y - 8.0)..=(rect.center().y + 8.0),
                Stroke::new(1.0, theme.CONTRAST),
            );
        }

        if clear_shown {
            // The X is a control with its own hover state: it brightens to the contrast ink
            // under the pointer, which is the only feedback the reference gives it.
            icons::draw(
                painter,
                Glyph::X,
                Rect::from_center_size(clear.center(), Vec2::splat(12.0)),
                if clear_hovered { theme.CONTRAST } else { theme.SECONDARY },
            );
        }

        self.owes_frame()
    }

    /// The query to filter with, or `None` when the field asks for nothing.
    ///
    /// Whitespace is not a query. A single space is what a reader leaves behind after
    /// deleting a word, and treating it as one would empty the catalogue instead of
    /// restoring it, so the trim happens here -- once, at the point of use -- and not in
    /// the stored text.
    pub fn query(&self) -> Option<&str> {
        let trimmed = self.text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// Whether this field still wants a frame. `ui` returns exactly this, so the crate's
    /// "an idle window draws nothing" rule can be read and tested without a painter.
    fn owes_frame(&self) -> bool {
        self.focused
    }

    /// Take the keyboard, starting a fresh blink.
    ///
    /// Both ways in -- a click on the plate and a click on the clear button -- come through
    /// here, so the phase can never disagree with the focus: a caret that arrived in its
    /// dark half would look like a field that had not been clicked at all.
    fn focus(&mut self) {
        self.focused = true;
        self.caret = 0.0;
    }

    /// Apply one edit to the text.
    ///
    /// Split out from [`Search::ui`] and kept pure -- no painter, no clock, no pointer --
    /// so the field's text rules can be tested without building a `Ui`. It is the same
    /// function the frame loop calls, which is the point: a test of `apply` is a test of
    /// what typing does, not of a copy of it.
    ///
    /// `event_text` is the character(s) one event carried and `backspace` is one press of
    /// the key; both can be set in one call, because they are independent edits rather than
    /// two variants of one.
    fn apply(&mut self, event_text: Option<&str>, backspace: bool) {
        if let Some(text) = event_text {
            // Control characters are dropped rather than drawn: a paste can carry a newline
            // or a tab, and a one-line field has no line to put either on. This is also the
            // whole of the "printable only" filter -- a character that is not a control is
            // a character the reader meant to type.
            self.text.extend(text.chars().filter(|c| !c.is_control()));
        }
        if backspace {
            // `pop` on an empty string is a no-op, which is the behaviour a field needs: a
            // backspace at the start of the text does nothing rather than moving focus or
            // reaching for the character before it.
            self.text.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Typing appends and a backspace removes one character, through the same `apply` the
    /// frame loop calls.
    #[test]
    fn typing_appends_and_backspace_removes() {
        let mut field = Search::default();
        field.apply(Some("fab"), false);
        field.apply(Some("ric"), false);
        assert_eq!(field.text, "fabric", "typed text accumulates in order");

        field.apply(None, true);
        assert_eq!(field.text, "fabri", "one backspace is one character");

        // A backspace with nothing to delete is a no-op rather than a panic: this is the
        // path a reader takes by holding the key at the start of an empty field.
        let mut empty = Search::default();
        empty.apply(None, true);
        assert_eq!(empty.text, "");
    }

    /// A paste can carry control characters; a one-line field has nowhere to put them.
    #[test]
    fn control_characters_never_reach_the_field() {
        let mut field = Search::default();
        field.apply(Some("opti\nfine\t"), false);
        assert_eq!(field.text, "optifine");
    }

    /// Whitespace is not a query: an empty search means "everything", and a filter that
    /// took a trailing space seriously would show nothing at all.
    #[test]
    fn a_whitespace_query_is_no_query() {
        let mut field = Search::default();
        assert_eq!(field.query(), None, "an empty field asks for nothing");

        field.apply(Some("   "), false);
        assert_eq!(field.query(), None, "spaces are not a query");

        field.apply(Some("  fabric  "), false);
        assert_eq!(field.query(), Some("fabric"), "the query is trimmed at the point of use");
        assert_eq!(field.text, "     fabric  ", "the stored text keeps what was typed");
    }

    /// A field nobody has clicked owes no frame, which is the crate's rule read from this
    /// widget's side: `ui` returns `owes_frame()`, and an idle window draws nothing.
    #[test]
    fn a_fresh_field_is_unfocused_and_owes_no_frame() {
        let field = Search::default();
        assert!(!field.focused, "a field that has not been clicked is not taking keys");
        assert!(!field.owes_frame(), "an unfocused field asks for no frame");
        assert_eq!(field.caret, 0.0, "the blink has not started");
        assert_eq!(field.query(), None);
    }
}
