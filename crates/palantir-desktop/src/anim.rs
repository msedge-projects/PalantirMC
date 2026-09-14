//! The slide a switch makes when it changes state.
//!
//! The reference client's switches are `transition-all duration-200`: over
//! 200 ms the knob travels from one end of its track to the other and the track
//! recolours behind it. iced's stock `Toggler` jumps between the two positions
//! inside a single frame, which next to the rest of the shell reads as a glitch
//! rather than a control — every other transition the reference draws is eased.
//!
//! So the slide is drawn here instead, and the split is the same one
//! [`crate::scroll`] makes: this module owns *the numbers and the clock*, and
//! the widget only paints what it is handed. Nothing in here knows about iced,
//! which is what lets the easing be tested without a window.
//!
//! One deliberate difference from [`crate::scroll::ScrollAnim`], because the
//! cost is different. A scroll animates for 160 ms under a wheel event that can
//! arrive thirty times a second, over a distance of hundreds of pixels, and on a
//! software rasteriser that was measured at 69 ms a frame it spent three
//! quarters of a second of a core to show one notch — so a machine that slow is
//! not asked to glide at all. A switch animates once per click, for 200 ms,
//! across 24 pixels: on that same slow machine it is three frames and it is
//! over. The deadline bounds it either way, so this animation is always worth
//! drawing, and skipping it would cost the polish while saving nothing.
//!
//! The mechanism that keeps it free at rest is the same: [`SwitchAnim::animating`]
//! gates the frame subscription in [`crate::app`], so a shell whose switches have
//! all arrived asks for no frames at all.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long a switch takes to arrive, wall clock.
///
/// The reference's `duration-200`, and a *deadline* rather than a frame count
/// for [`crate::scroll`]'s reason: a machine that answers late draws fewer,
/// larger steps and still finishes on time.
pub const DURATION: Duration = Duration::from_millis(200);

/// One switch's position, and the slide carrying it there.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Knob {
    /// The value the setting has: 0.0 off, 1.0 on.
    to: f32,
    /// Where the current slide started from.
    from: f32,
    /// Where the knob is drawn, in `0.0..=1.0`.
    progress: f32,
    /// When the slide began, or `None` once it has arrived.
    began: Option<Instant>,
}

impl Knob {
    /// A knob already at `value` and going nowhere.
    fn settled(value: f32) -> Knob {
        Knob { to: value, from: value, progress: value, began: None }
    }
}

/// Every switch's position, keyed by a stable id.
///
/// Keyed by `&'static str` rather than by position, because the position of a
/// switch in the tree is exactly what changes when a pane is switched or a
/// section is reordered — and a slide that follows the tree would then be drawn
/// on whatever landed in that slot.
#[derive(Debug, Clone, Default)]
pub struct SwitchAnim {
    knobs: HashMap<&'static str, Knob>,
}

impl SwitchAnim {
    /// Where to draw the switch `id`, whose setting is `on`.
    ///
    /// A switch that has never been touched is simply at its value: the first
    /// paint of a pane must not animate every switch on it into place, which is
    /// what reading an absent entry as `0.0` would do.
    pub fn progress(&self, id: &str, on: bool) -> f32 {
        match self.knobs.get(id) {
            Some(knob) => knob.progress,
            None => if on { 1.0 } else { 0.0 },
        }
    }

    /// Record that the setting behind `id` has moved from `from` to `to`, and
    /// start its slide.
    ///
    /// Called from `update`, where the message that changed the setting is
    /// handled — not from the view, which cannot know whether the value it is
    /// drawing is new. Both ends travel because a switch that has never been
    /// touched has no entry here, and the only thing that says where it *was*
    /// is the value it just left. Guessing that from `to` alone would make a
    /// first click the one click that jumps instead of sliding.
    pub fn set(&mut self, id: &'static str, from: bool, to: bool, now: Instant) {
        let target = if to { 1.0 } else { 0.0 };
        // A value that did not actually change starts no slide.
        let knob = self
            .knobs
            .entry(id)
            .or_insert_with(|| Knob::settled(if from { 1.0 } else { 0.0 }));
        if knob.to == target {
            return;
        }
        // From where the knob is *drawn*, not from the value it had: clicking
        // twice quickly reverses a half-finished slide instead of snapping it
        // back to the far end first.
        knob.from = knob.progress;
        knob.to = target;
        knob.began = Some(now);
    }

    /// Whether any knob is still travelling.
    ///
    /// This is what holds the frame subscription open, so it has to be exact:
    /// reporting `true` once too often leaves a shell asking for frames forever.
    pub fn animating(&self) -> bool {
        self.knobs.values().any(|knob| knob.began.is_some())
    }

    /// Advance every slide to `now`, returning whether any is still moving.
    pub fn tick(&mut self, now: Instant) -> bool {
        let deadline = DURATION.as_secs_f32();
        let mut moving = false;
        for knob in self.knobs.values_mut() {
            let Some(began) = knob.began else {
                continue;
            };
            let elapsed = now.saturating_duration_since(began).as_secs_f32();
            let progress = if deadline > 0.0 { (elapsed / deadline).clamp(0.0, 1.0) } else { 1.0 };
            knob.progress = knob.from + (knob.to - knob.from) * ease(progress);
            if progress >= 1.0 {
                // Land exactly, so the animation stops rather than approaching
                // its target forever and holding the frame subscription open.
                knob.progress = knob.to;
                knob.began = None;
            } else {
                moving = true;
            }
        }
        moving
    }
}

/// CSS `ease`, near enough to be indistinguishable at 24 pixels.
///
/// A control that changed state is a *decoration*, not a position the hand
/// asked for, so it leaves slowly, crosses quickly and settles — the opposite
/// emphasis to [`crate::scroll`]'s ease-out, and the reason these are two
/// functions rather than one shared helper. `progress` is in `0.0..=1.0`.
///
/// The real curve is `cubic-bezier(0.25, 0.1, 0.25, 1)`; smoothstep is within
/// two points of it across the whole range, and unlike a bezier it is three
/// multiplications with no solver.
fn ease(progress: f32) -> f32 {
    progress * progress * (3.0 - 2.0 * progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(millis: u64) -> Instant {
        // A fixed origin: the tests are about elapsed time, not about now.
        Instant::now() + Duration::from_millis(millis)
    }

    #[test]
    fn an_untouched_switch_is_at_its_value() {
        let anim = SwitchAnim::default();
        assert_eq!(anim.progress("a", false), 0.0);
        assert_eq!(anim.progress("a", true), 1.0);
        assert!(!anim.animating(), "nothing has moved, so nothing is moving");
    }

    #[test]
    fn a_click_starts_a_slide_and_arrives_on_the_deadline() {
        let mut anim = SwitchAnim::default();
        let start = Instant::now();
        anim.set("a", false, true, start);
        assert!(anim.animating());
        assert_eq!(anim.progress("a", true), 0.0, "a slide begins where the knob was");

        // Halfway is strictly between the ends — a jump would be at 1.0 here.
        assert!(anim.tick(start + DURATION / 2));
        let middle = anim.progress("a", true);
        assert!(middle > 0.05 && middle < 0.95, "got {middle}");

        // And the deadline ends it, exactly on the value.
        assert!(!anim.tick(start + DURATION));
        assert_eq!(anim.progress("a", true), 1.0);
        assert!(!anim.animating(), "a settled slide must not hold frames open");
    }

    #[test]
    fn the_slide_is_a_function_of_time_not_of_frame_count() {
        // The property the frame-counted tween could not offer: three late
        // frames and thirty early ones both finish at the same wall clock time.
        let start = Instant::now();
        let mut slow = SwitchAnim::default();
        slow.set("a", false, true, start);
        for step in 1..=3 {
            slow.tick(start + DURATION.mul_f32(step as f32 / 3.0));
        }
        assert_eq!(slow.progress("a", true), 1.0);
        assert!(!slow.animating());

        // Thirty steps of a hair over 6.6 ms each, so the last one lands past
        // the deadline and the slide is over — the count is not what ends it.
        let mut fast = SwitchAnim::default();
        fast.set("a", false, true, start);
        for step in 1..=30 {
            fast.tick(start + Duration::from_micros(step * 6_800));
        }
        assert_eq!(fast.progress("a", true), 1.0);
        assert!(!fast.animating());
    }

    #[test]
    fn a_reversed_click_turns_around_from_where_it_is() {
        // Clicking twice in a hurry must not snap the knob to the far end and
        // start over: it reverses from the pixel it is on. The second click
        // here lands half a slide in, and the return trip then gets a full
        // slide of its own — so a tick that is halfway through the *first*
        // deadline cannot be the end of it.
        let mut anim = SwitchAnim::default();
        let start = Instant::now();
        anim.set("a", false, true, start);
        anim.tick(start + DURATION / 2);
        let halfway = anim.progress("a", true);
        assert!(halfway > 0.0 && halfway < 1.0);

        anim.set("a", true, false, start + DURATION / 2);
        assert_eq!(anim.progress("a", true), halfway, "the turn-around keeps the position");
        anim.tick(start + DURATION);
        let back = anim.progress("a", false);
        assert!(back > 0.0 && back < halfway, "got {back} back from {halfway}");
        assert!(anim.animating(), "the return trip is not over yet");

        anim.tick(start + DURATION / 2 + DURATION);
        assert_eq!(anim.progress("a", false), 0.0);
        assert!(!anim.animating());
    }

    #[test]
    fn setting_the_value_it_already_has_starts_nothing() {
        let mut anim = SwitchAnim::default();
        let start = Instant::now();
        anim.set("a", false, false, start);
        assert!(!anim.animating(), "off to off is not a transition");
        anim.set("a", false, true, start);
        assert!(anim.animating());
        anim.tick(start + DURATION);
        anim.set("a", true, true, start + DURATION);
        assert!(!anim.animating(), "on to on is not a transition either");
    }

    #[test]
    fn switches_do_not_share_a_position() {
        let mut anim = SwitchAnim::default();
        let start = Instant::now();
        anim.set("a", false, true, start);
        anim.set("b", true, false, start);
        anim.tick(start + DURATION);
        assert_eq!(anim.progress("a", true), 1.0);
        assert_eq!(anim.progress("b", false), 0.0);
    }

    #[test]
    fn the_easing_is_monotonic_and_pinned_at_both_ends() {
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(1.0), 1.0);
        let mut previous = -1.0;
        for step in 0..=100 {
            let value = ease(step as f32 / 100.0);
            assert!(value >= previous, "eased backwards at {step}");
            previous = value;
        }
        // Slow to leave and slow to settle, which is what makes it read as a
        // transition rather than a jump.
        assert!(ease(0.1) < 0.1, "should leave gently");
        assert!(ease(0.9) > 0.9, "should settle gently");
    }
}
