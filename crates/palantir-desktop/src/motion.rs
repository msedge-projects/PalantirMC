//! Motion: the reference's own numbers, and the one curve solver a native
//! toolkit does not get for free.
//!
//! A browser animates a CSS transition by interpolating the target property
//! through a *timing function*, which is a cubic Bézier from `(0, 0)` to
//! `(1, 1)` whose two middle control points the stylesheet names. iced has no
//! such thing: it has per-widget easing presets and `time::every`, so the
//! numbers and the maths both have to be carried across by hand. This module
//! carries them.
//!
//! Every number comes from [`crate::theme_gen`], which is generated from the
//! reference's own stylesheets, so a duration here is a citation rather than a
//! choice: [`Timing::declared`] looks a row up in the generated table and
//! returns nothing if the reference does not declare that pair. The one place
//! a caller may name a duration outright is [`Timing::cited`], for the
//! transitions `tools/gen_theme.py` deliberately keeps verbatim -- it refuses
//! to split `opacity 0.25s var(--ease-out-expo), scale 0.25s var(--ease-out-
//! expo)` because a naive split of a CSS transition shorthand produces a
//! plausible wrong number, so the shell cites the row instead.
//!
//! The solver is WebKit's `UnitBezier` algorithm -- eight Newton-Raphson steps
//! with a bisection fallback -- because it is the algorithm inside the engine
//! the reference ships in, and the tests compare its answers against Chromium's
//! own, sampled a quarter of the way through each of the reference's curves.

use std::time::Duration;

use crate::theme_gen::{self, Curve};

/// How close to the answer a solve has to get.
///
/// WebKit uses 1/200 for painted transitions; a thousandth of a pixel is not
/// worth ten more iterations in a window that redraws every 16ms, and 1e-6 is
/// still five orders of magnitude finer than that. Below this the answer would
/// differ from the browser's by less than a pixel at any size this shell draws.
const EPSILON: f32 = 1e-6;

/// One coordinate of a cubic Bézier from `(0, 0)` to `(1, 1)`.
///
/// `a` is the first control point, `b` the second, and `1.0` the end point,
/// which is what the expanded polynomial `3(1-t)²t·a + 3(1-t)t²·b + t³` is.
fn bezier(t: f32, a: f32, b: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
}

/// The derivative of [`bezier`] with respect to `t`, which Newton's method
/// needs.
fn bezier_slope(t: f32, a: f32, b: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
}

/// The `t` at which the curve's *x* reaches `x`.
///
/// A CSS timing function is defined the other way round from the way it is
/// used: the input is progress along the x axis and the output is the eased
/// fraction on y, and x is not `t`, it is a cubic in `t`. So the fraction is
/// not a number the curve is sampled at, it is one the curve is *inverted* for,
/// which is the whole reason this is not a one-liner.
fn solve_for_t(control: [f32; 4], x: f32) -> f32 {
    let (x1, _, x2, _) = (control[0], control[1], control[2], control[3]);

    // Newton-Raphson: usually three or four steps. Starting from `x` is
    // WebKit's own guess, and it is exact for the identity curve.
    let mut t = x;
    for _ in 0..8 {
        let error = bezier(t, x1, x2) - x;
        if error.abs() < EPSILON {
            return t;
        }
        let slope = bezier_slope(t, x1, x2);
        // A flat derivative cannot be stepped along; the bisection below takes
        // it from here.
        if slope.abs() < 1e-6 {
            break;
        }
        t -= error / slope;
    }

    // Bisection: slower and unconditional. The reference's curves are all
    // monotone in x (checked in the tests), so a bracket always narrows.
    let (mut low, mut high) = (0.0f32, 1.0f32);
    if t < low {
        return low;
    }
    if t > high {
        return high;
    }
    while low < high {
        let estimate = bezier(t, x1, x2);
        if (estimate - x).abs() < EPSILON {
            return t;
        }
        if x > estimate {
            low = t;
        } else {
            high = t;
        }
        t = (high - low) * 0.5 + low;
    }
    t
}

/// Evaluate a CSS timing function: progress in, eased fraction out.
///
/// Both ends are exact (`0` maps to `0`, `1` to `1`) and the result is *not*
/// clamped in between: the reference has curves whose control points sit
/// outside the unit square -- `cubic-bezier(0.15, 1.4, 0.64, 0.96)`, on
/// `FloatingActionBar` and on `App.vue`'s own pop transition -- and the
/// overshoot past the target is the effect, not an error. Callers that draw
/// the value rather than the fraction are responsible for whatever clamping
/// their own widget needs.
pub fn ease(control: [f32; 4], progress: f32) -> f32 {
    if progress <= 0.0 {
        return 0.0;
    }
    if progress >= 1.0 {
        return 1.0;
    }
    let (_, y1, _, y2) = (control[0], control[1], control[2], control[3]);
    bezier(solve_for_t(control, progress), y1, y2)
}

/// A transition the reference declares: how long it takes and what it moves
/// on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// Milliseconds, as the reference writes them.
    pub millis: u32,
    /// The curve, resolved through the generated table.
    pub curve: Curve,
}

impl Timing {
    /// The rail's selection plate, which is the shell's signature animation.
    ///
    /// `NavButton.vue` declares it as
    /// `opacity 0.25s var(--ease-out-expo), scale 0.25s var(--ease-out-expo)`
    /// on the `::before` circle that grows behind the icon: opacity and scale
    /// both run for 250ms on `--ease-out-expo`, which is
    /// `cubic-bezier(0.16, 1, 0.3, 1)`. The row is one of the ones
    /// `tools/gen_theme.py` keeps verbatim rather than splitting -- see
    /// `theme_gen::MOTION_VERBATIM`, which holds this exact string.
    pub const NAV_PLATE: Timing = Timing { millis: 250, curve: Curve::EaseOutExpo };

    /// The circle the plate grows *from*: `scale: 0.4` in the same rule, which
    /// is why pressing a rail button reads as a pop rather than a fade.
    pub const NAV_PLATE_FROM_SCALE: f32 = 0.4;

    /// The row of the reference's motion table this pair is, or `None`.
    ///
    /// Returning `None` is the useful answer: it means the reference does not
    /// declare a transition with that property and that duration, so a caller
    /// asking for one has invented a number and should be made to notice.
    pub fn declared(property: &str, millis: u32) -> Option<Timing> {
        let index = theme_gen::MOTION
            .binary_search_by_key(&(property, millis), |motion| (motion.property, motion.millis))
            .ok()?;
        // The table is sorted by property, millis *and* curve, and rows share
        // the first two while differing in the third: the reference declares
        // `transition: all 200ms` in three different files on three different
        // curves, and the generator keeps all three because all three are real
        // rules. So a lookup by property and duration alone takes the first row
        // in table order, and `declared_rows` is there for a caller that needs
        // to see every row that pair owns.
        Some(Timing {
            millis,
            curve: theme_gen::MOTION
                .get(index)
                .map_or(Curve::EaseInOut, |motion| motion.curve),
        })
    }

    /// Every row the reference declares for a property, as `(millis, curve,
    /// source)`, in table order.
    pub fn declared_rows(property: &str) -> Vec<(u32, Curve, &'static str)> {
        theme_gen::MOTION
            .iter()
            .filter(|motion| motion.property == property)
            .map(|motion| (motion.millis, motion.curve, motion.source))
            .collect()
    }

    /// A timing the caller names outright, for a transition the generator kept
    /// verbatim.
    ///
    /// Not a licence to pick a duration: the value belongs in the comment
    /// beside the call, as a quote from the reference, and the constant above
    /// is the example.
    pub fn cited(millis: u32, curve: Curve) -> Timing {
        Timing { millis, curve }
    }

    /// The curve's four control-point coordinates, from the generated table.
    pub fn control(self) -> [f32; 4] {
        theme_gen::curve(self.curve)
    }

    /// How long the transition runs.
    pub fn duration(self) -> Duration {
        Duration::from_millis(self.millis as u64)
    }
}

/// A single value moving from one number to another over the reference's own
/// timing.
///
/// Deliberately one value rather than a scene graph: the shell keeps a `Tween`
/// per animated property of the thing it is drawing, and asking whether any of
/// them is still running is what decides whether the frame clock stays awake.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    from: f32,
    to: f32,
    timing: Timing,
    elapsed: Duration,
}

impl Tween {
    /// A value that is already at `value` and not moving.
    pub fn at(value: f32) -> Tween {
        Tween { from: value, to: value, timing: Timing { millis: 0, curve: Curve::Linear }, elapsed: Duration::ZERO }
    }

    /// A value starting at `from` and heading for `to`.
    pub fn new(from: f32, to: f32, timing: Timing) -> Tween {
        Tween { from, to, timing, elapsed: Duration::ZERO }
    }

    /// Whether this still has somewhere to go.
    pub fn is_running(&self) -> bool {
        self.elapsed < self.timing.duration()
    }

    /// Move the clock on. Returns whether it is still running afterwards, which
    /// is the only thing a caller needs to decide whether to keep ticking.
    pub fn advance(&mut self, delta: Duration) -> bool {
        if delta.is_zero() {
            return self.is_running();
        }
        self.elapsed = self.elapsed.saturating_add(delta);
        if self.elapsed > self.timing.duration() {
            self.elapsed = self.timing.duration();
        }
        self.is_running()
    }

    /// Jump to the end, whatever the clock says. What a caller does when a
    /// window loses focus or a user asks for reduced motion.
    pub fn finish(&mut self) {
        self.elapsed = self.timing.duration();
    }

    /// Linear progress, 0 to 1.
    pub fn progress(&self) -> f32 {
        if self.timing.millis == 0 {
            return 1.0;
        }
        (self.elapsed.as_secs_f32() * 1000.0 / self.timing.millis as f32).clamp(0.0, 1.0)
    }

    /// The eased fraction: [`ease`] of [`Tween::progress`].
    pub fn eased(&self) -> f32 {
        ease(self.timing.control(), self.progress())
    }

    /// Where the value is now.
    ///
    /// A tween that has stopped lands *exactly* on its target rather than a
    /// float-ulp short of it. The difference between `0.4` and `0.39999998` is
    /// invisible on its own and very visible when two controls that should line
    /// up are drawn from the same number.
    pub fn value(&self) -> f32 {
        if !self.is_running() {
            return self.to;
        }
        self.from + (self.to - self.from) * self.eased()
    }

    /// Where it is heading.
    pub fn target(&self) -> f32 {
        self.to
    }

    /// Where the current leg started.
    pub fn origin(&self) -> f32 {
        self.from
    }

    /// The timing the current leg runs on.
    pub fn timing(&self) -> Timing {
        self.timing
    }

    /// Aim at a new target, starting from wherever the value is right now and
    /// running for the full duration again.
    ///
    /// This is what a browser does when the property a transition is running on
    /// changes mid-flight, and it is why hovering a rail button and leaving it
    /// again does not snap: the outgoing transition starts from the part-grown
    /// plate, not from where the incoming one began. Changing the target to the
    /// one already in flight is a no-op, so a re-render cannot restart an
    /// animation.
    pub fn retarget(&mut self, to: f32) {
        if to == self.to {
            return;
        }
        self.from = self.value();
        self.to = to;
        self.elapsed = Duration::ZERO;
    }

    /// The same, with a different timing: the reference's own files use
    /// different durations for enter and leave in places -- a 0.2s fade in and
    /// a 0.3s fade out, on `PopupNotificationPanel` -- so the leg's timing is
    /// part of the retarget rather than a property of the value's life.
    pub fn retarget_on(&mut self, to: f32, timing: Timing) {
        let value = self.value();
        let was_aimed = to == self.to && timing == self.timing;
        if was_aimed {
            return;
        }
        self.from = value;
        self.to = to;
        self.timing = timing;
        self.elapsed = Duration::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chromium's own answers, for the tenths after zero.
    ///
    /// Measured, not remembered: `tools/curve_samples.html` runs each of the
    /// reference's curves as a real CSS animation in the engine the reference
    /// ships inside, pauses it at each tenth of the way through and reads the
    /// computed style back, which is the fraction a browser would paint. The
    /// table is that page's output. It is written down here rather than
    /// computed in this file because the point of the test is to compare two
    /// independent implementations -- if this module re-derived the numbers,
    /// the test would only be asking whether arithmetic is deterministic.
    ///
    /// A cross-check ran the same algorithm in Python over the same samples and
    /// agreed to 1.3e-6, which is why the tolerance below is where it is: an
    /// f32 evaluation of the same polynomial, not a licence to be sloppy.
    const BROWSER_SAMPLES: &[(Curve, [f32; 9])] = &[
        (
            Curve::Linear,
            [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9],
        ),
        (
            Curve::Ease,
            [0.094_796_3, 0.295_244, 0.513_315, 0.682_54, 0.802_403,
             0.885_229, 0.940_765, 0.975_625, 0.994_316],
        ),
        (
            Curve::EaseOut,
            [0.160_572, 0.308_366, 0.445_186, 0.570_88, 0.684_643,
             0.785_139, 0.870_423, 0.937_718, 0.982_973],
        ),
        (
            Curve::EaseInOut,
            [0.019_722_5, 0.081_659_9, 0.187_396, 0.331_884, 0.5,
             0.668_116, 0.812_604, 0.918_34, 0.980_278],
        ),
        (
            Curve::EaseOutExpo,
            [0.494_391, 0.752_126, 0.877_174, 0.939_756, 0.971_779,
             0.987_965, 0.995_678, 0.998_891, 0.999_878],
        ),
    ];

    #[test]
    fn easing_agrees_with_the_browser_the_reference_runs_in() {
        for (curve, samples) in BROWSER_SAMPLES {
            let control = theme_gen::curve(*curve);
            for (index, expected) in samples.iter().enumerate() {
                let progress = (index + 1) as f32 / 10.0;
                let computed = ease(control, progress);
                assert!(
                    (computed - expected).abs() < 1e-5,
                    "{curve:?} at {progress}: this solver says {computed}, Chromium says {expected}"
                );
            }
        }
    }

    #[test]
    fn both_ends_of_every_curve_are_exact() {
        // A transition starts at its start and finishes at its finish; no
        // solver tolerance is allowed to show up as a value that never quite
        // arrives.
        for curve in theme_gen::ALL_CURVE {
            assert_eq!(ease(theme_gen::curve(*curve), 0.0), 0.0, "{curve:?}");
            assert_eq!(ease(theme_gen::curve(*curve), 1.0), 1.0, "{curve:?}");
            assert_eq!(ease(theme_gen::curve(*curve), -1.0), 0.0, "{curve:?}");
            assert_eq!(ease(theme_gen::curve(*curve), 2.0), 1.0, "{curve:?}");
        }
    }

    #[test]
    fn every_curve_the_reference_declares_is_a_legal_timing_function() {
        // CSS requires the two x coordinates to lie within [0, 1] or the curve
        // is not a function of progress at all; y is free, and the overshoot
        // curves rely on that. A generated table with an x outside the range
        // would be invalid CSS that a browser rejects, so it is a gate here.
        for curve in theme_gen::ALL_CURVE {
            let control = theme_gen::curve(*curve);
            assert!(
                (0.0..=1.0).contains(&control[0]) && (0.0..=1.0).contains(&control[2]),
                "{curve:?} has an x outside the unit interval: {control:?}"
            );
            // Monotone in x is what makes the inversion well defined, and what
            // the bisection fallback relies on.
            let mut previous = 0.0;
            for step in 0..=64 {
                let x = bezier(step as f32 / 64.0, control[0], control[2]);
                assert!(x >= previous, "{curve:?} is not monotone in x");
                previous = x;
            }
        }
    }

    #[test]
    fn the_motion_table_agrees_with_itself() {
        // Every row of the generated table must be findable by the pair the
        // shell looks rows up with, and every row's curve must resolve. This is
        // the gate that would have caught the generator reading `200ms` as
        // 200000ms a second time: the shell would have asked for a two-hundred
        // second fade and silently got nothing.
        for motion in theme_gen::MOTION {
            let Some(timing) = Timing::declared(motion.property, motion.millis) else {
                panic!(
                    "{} at {}ms is in the table but not findable in it",
                    motion.property, motion.millis
                )
            };
            // Same pair, possibly a different row: the reference declares
            // `all 200ms` on three curves in three files and the generator
            // keeps all three, so the lookup cannot be unique. What has to hold
            // is that the curve it answered with belongs to one of the rows
            // that pair owns.
            let curves: Vec<Curve> = Timing::declared_rows(motion.property)
                .into_iter()
                .filter(|(millis, _, _)| *millis == motion.millis)
                .map(|(_, curve, _)| curve)
                .collect();
            assert!(
                curves.contains(&timing.curve),
                "{} at {}ms resolved to {:?}, which no row declares",
                motion.property,
                motion.millis,
                timing.curve
            );
            assert!(
                motion.millis <= 2000,
                "{} declares {}ms, longer than any transition in the reference",
                motion.property,
                motion.millis
            );
        }
        // And the duplication is a fact about the reference rather than a
        // defect in the generated table: `NewModal.vue` declares `all 200ms
        // ease-out` and `all 200ms ease-in-out`, and `FileNavbar.vue` declares
        // `all 200ms ease`. If a future regeneration loses two of the three,
        // the assertion above stops being able to tell them apart and this one
        // says so.
        let duplicated = Timing::declared_rows("all")
            .into_iter()
            .filter(|(millis, _, _)| *millis == 200)
            .map(|(_, curve, _)| curve)
            .collect::<Vec<Curve>>();
        assert!(
            duplicated.len() >= 2 && duplicated.iter().any(|curve| *curve != duplicated[0]),
            "the reference declares `all 200ms` on more than one curve: {duplicated:?}"
        );
    }

    #[test]
    fn a_row_can_be_cited_with_its_source() {
        // The property names are the reference's CSS properties, and the source
        // is where a porter goes to read the rule.
        let rows = Timing::declared_rows("transform");
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|(_, _, source)| source.ends_with(".scss") || source.ends_with(".vue")));
        assert!(Timing::declared("filter", 100).is_some());
        // A pair the reference does not declare is not invented.
        assert!(Timing::declared("filter", 1234).is_none());
        assert!(Timing::declared("not-a-property", 200).is_none());
    }

    #[test]
    fn the_rail_plate_is_the_references_own_pop() {
        // `NavButton.vue`: `scale: 0.4` on the `::before` circle,
        // `transition: opacity 0.25s var(--ease-out-expo), scale 0.25s ...`.
        let plate = Timing::NAV_PLATE;
        assert_eq!(plate.millis, 250);
        assert_eq!(plate.curve, Curve::EaseOutExpo);
        assert_eq!(plate.control(), [0.16, 1.0, 0.3, 1.0]);
        assert!(
            theme_gen::MOTION_VERBATIM
                .iter()
                .any(|(source, value)| source.ends_with("NavButton.vue") && value.contains("0.25s")),
            "the row this cites must still be in the generated verbatim table"
        );
        // `SmartClickable` declares the hover fade as a single property and a
        // single duration, so it survives as a structured row: 125ms ease-out,
        // and that is what the plate's opacity uses on the way in.
        assert_eq!(
            Timing::declared("opacity", 125).map(|timing| timing.curve),
            Some(Curve::EaseOut)
        );
    }

    #[test]
    fn a_tween_starts_where_it_is_put_and_ends_exactly_on_its_target() {
        let mut tween = Tween::new(0.0, 1.0, Timing::NAV_PLATE);
        assert_eq!(tween.value(), 0.0);
        assert!(tween.is_running());
        // Half a duration is not the halfway value on a curve like this one:
        // `ease-out-expo` is most of the way there at 50%.
        assert!(tween.advance(Duration::from_millis(125)), "half way is still running");
        assert!(tween.value() > 0.9, "ease-out-expo at half way: {}", tween.value());
        assert!(!tween.advance(Duration::from_millis(200)), "past the end it has stopped");
        assert_eq!(tween.value(), 1.0, "a finished tween is exactly at its target");
        assert_eq!(tween.progress(), 1.0);
        assert!(!tween.advance(Duration::from_millis(16)));
    }

    #[test]
    fn a_tween_retargeted_mid_flight_carries_on_from_where_it_is() {
        // The plate grows in and is then un-hovered a third of the way there.
        // A browser starts the outgoing transition from the *computed* value at
        // that instant, so the shape of the first leg is preserved in the
        // second rather than snapping back to 0.4.
        let mut tween = Tween::new(Timing::NAV_PLATE_FROM_SCALE, 1.0, Timing::NAV_PLATE);
        tween.advance(Duration::from_millis(80));
        let partial = tween.value();
        assert!(partial > Timing::NAV_PLATE_FROM_SCALE && partial < 1.0);
        tween.retarget(Timing::NAV_PLATE_FROM_SCALE);
        assert_eq!(tween.origin(), partial, "the new leg starts at the value on screen");
        assert_eq!(tween.progress(), 0.0);
        // Leaving at 0.25s per leg is what makes the two directions symmetric.
        tween.advance(Duration::from_millis(250));
        assert_eq!(tween.value(), Timing::NAV_PLATE_FROM_SCALE);
    }

    #[test]
    fn retargeting_to_the_target_already_in_flight_does_not_restart_it() {
        // A shell rebuilds its whole element tree every frame, so a hover
        // message can arrive many times a second. Without this, the plate would
        // never finish growing.
        let mut tween = Tween::new(0.0, 1.0, Timing::NAV_PLATE);
        tween.advance(Duration::from_millis(100));
        let progress = tween.progress();
        tween.retarget(1.0);
        assert_eq!(tween.progress(), progress);
        // A different timing for the same target, though, is a new leg, which
        // is how the reference's asymmetric enter/leave pairs work.
        tween.retarget_on(1.0, Timing::cited(100, Curve::EaseOut));
        assert_eq!(tween.progress(), 0.0);
        assert_eq!(tween.timing().millis, 100);
    }

    #[test]
    fn an_overshooting_curve_passes_its_target_and_comes_back() {
        // `App.vue`'s own pop: `all 0.5s cubic-bezier(0.15, 1.4, 0.64, 0.96)`,
        // declared in the generated verbatim table. y1 = 1.4 is why the result
        // is not clamped: the value genuinely goes past the target.
        let control = [0.15, 1.4, 0.64, 0.96];
        assert!(theme_gen::MOTION_VERBATIM
            .iter()
            .any(|(_, value)| value.contains("0.15, 1.4, 0.64, 0.96")));
        let mut peak = 0.0f32;
        for step in 0..=200 {
            peak = peak.max(ease(control, step as f32 / 200.0));
        }
        assert!(peak > 1.0, "the overshoot is the effect: peak {peak}");
        let tween = Tween::new(0.0, 100.0, Timing::cited(500, Curve::EaseOutExpo));
        assert_eq!(tween.value(), 0.0);
        // And the endpoint is still exact, so the overshoot cannot strand the
        // value past the target.
        let mut tween = tween;
        tween.advance(Duration::from_millis(500));
        assert_eq!(tween.value(), 100.0);
    }

    #[test]
    fn a_zero_length_transition_is_already_over() {
        // `transition-duration: 0s` is legal CSS and appears in the reference's
        // own `none` rows. It must not divide by zero or spin the frame clock.
        let tween = Tween::new(0.0, 1.0, Timing::cited(0, Curve::Linear));
        assert!(!tween.is_running());
        assert_eq!(tween.value(), 1.0);
    }

    #[test]
    fn a_tween_is_either_moving_or_at_rest() {
        let resting = Tween::at(0.5);
        assert!(!resting.is_running());
        assert_eq!(resting.value(), 0.5);
        let mut tween = Tween::new(0.0, 1.0, Timing::NAV_PLATE);
        assert!(tween.is_running());
        tween.finish();
        assert!(!tween.is_running());
        assert_eq!(tween.value(), 1.0);
    }

    #[test]
    fn advancing_halfway_twice_is_not_advancing_all_the_way_once() {
        // The clock is additive, not a step count: a shell that ticks at 16ms
        // and a shell that ticks at 8ms must reach the same value at the same
        // wall-clock moment, or motion would depend on the frame rate.
        let mut coarse = Tween::new(0.0, 1.0, Timing::NAV_PLATE);
        coarse.advance(Duration::from_millis(32));
        let mut fine = Tween::new(0.0, 1.0, Timing::NAV_PLATE);
        fine.advance(Duration::from_millis(16));
        fine.advance(Duration::from_millis(16));
        assert_eq!(coarse.value(), fine.value());
    }
}
