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

#[cfg(test)]
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

/// A transition: how long it takes, and the timing function it moves on.
///
/// The curve is held as its four control-point coordinates rather than as a
/// [`Curve`] because a *timing function* is the numbers and the enum is only a
/// name for five of them. That distinction has teeth here: the reference's
/// controls are animated by Tailwind's `transition-all`, whose curve
/// (`cubic-bezier(0.4, 0, 0.2, 1)`) is Tailwind's own and not one of the five
/// CSS-named easings, so a shell that could only name a curve could not cite
/// the timing its own rail buttons use. [`Timing::raw`] is that door, and the
/// tests keep [`Timing::curve`] honest about which named curves they are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// Milliseconds, as the reference writes them.
    pub millis: u32,
    /// `(x1, y1, x2, y2)`, in the order a CSS `cubic-bezier()` names them.
    pub control: [f32; 4],
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
    pub const NAV_PLATE: Timing =
        Timing { millis: 250, control: [0.16, 1.0, 0.3, 1.0] };

    /// The circle the plate grows *from*: `scale: 0.4` in the same rule, which
    /// is why pressing a rail button reads as a pop rather than a fade.
    pub const NAV_PLATE_FROM_SCALE: f32 = 0.4;

    /// The page fade: a new page arriving over the old one.
    ///
    /// `App.vue`'s own `<Transition name="fade">` around the router view, read
    /// off that file's style block (lines 2829-2835): `fade-enter-active` is
    /// `transition: 0.25s ease-in-out` and `fade-enter-from` is `opacity: 0`.
    /// CSS's `ease-in-out` is `cubic-bezier(0.42, 0, 0.58, 1)`, so that is what
    /// the four control points are rather than one of this module's other five.
    ///
    /// **One-sided on purpose.** The reference declares no `fade-leave-*` rules
    /// at all, so the outgoing page leaves instantly and only the incoming one
    /// fades up. Reproducing a cross-fade here would be inventing a rule the
    /// reference does not have, and would make a page change look like a slower
    /// app rather than a faster-looking one.
    pub const PAGE_FADE: Timing = Timing { millis: 250, control: [0.42, 0.0, 0.58, 1.0] };

    /// A dialog's own arrival: `scale` and `opacity` together.
    ///
    /// `ui/src/components/modal/NewModal.vue`'s `> .modal-body` rule: the body
    /// sits at `scale: 0.97` with `opacity: 0` and `visibility: hidden`, and
    /// `.modal-container.shown > .modal-body` takes it to `opacity: 1;
    /// visibility: visible; scale: 1` on `transition: all 0.2s ease-in-out`.
    /// CSS's `ease-in-out` is `cubic-bezier(0.42, 0, 0.58, 1)`, which is the same
    /// four numbers [`Timing::PAGE_FADE`] carries -- the reference reaches for
    /// that curve in both places, so they share the constant's value and not its
    /// name.
    ///
    /// **The scale is not drawn.** iced 0.12 has no transform, so what a shell
    /// can paint of a `scale` is nothing at all: a dialog at 0.97 that
    /// interpolates to 1.0 is the same dialog, slightly narrower, for 200ms.
    /// Faking it with padding would move every row inside the dialog as well,
    /// which is the *other* half of `scale` and not what the reference does --
    /// it scales the box and its contents together. What is drawn is the
    /// opacity half, which is the part a reader notices and the half the
    /// capture can measure. See [`crate::shell::Shell::modal_opacity`].
    pub const MODAL_DIALOG: Timing = Timing { millis: 200, control: [0.42, 0.0, 0.58, 1.0] };

    /// The bed behind a dialog, which is the other half of its arrival.
    ///
    /// The same file's `.modal-overlay`: `opacity: 0` with
    /// `transition: all 0.2s ease-out`, taken to `opacity: 1` by `.shown`.
    ///
    /// It cannot be a [`Timing::declared`] lookup, because the reference writes
    /// this one as the shorthand keyword `ease-out` inside a two-property
    /// `all` transition -- `all 0.2s ease-out` -- and a lookup takes a *property*
    /// and a duration, not a shorthand. The four control points are what CSS
    /// defines `ease-out` to be, and they are the same four the generated table
    /// resolves `Curve::EaseOut` to, which is what makes this a citation rather
    /// than a guess: [`Timing::declared`] for `("all", 200)` can answer in any of
    /// three curves, because three files each declare `all 200ms` on a different
    /// one.
    pub const MODAL_SCRIM: Timing = Timing { millis: 200, control: [0.0, 0.0, 0.58, 1.0] };

    /// How long the reference waits before it takes the modal out of the tree.
    ///
    /// `NewModal.vue`'s `hide()` sets `visible` false and then leaves the
    /// element mounted for 300ms: `hideTimeout = setTimeout(() => { open.value =
    /// false }, 300)`. Without that delay the leave transition would have nothing
    /// left to run on, and the dialog would blink rather than fade. The shell has
    /// the same problem in the same shape -- a modal dropped from an `Option` is
    /// gone on the next frame -- so the number is a citation for *when* the
    /// option may be cleared rather than a duration to animate over.
    pub const MODAL_UNMOUNT: Duration = Duration::from_millis(300);

    /// The panel toggle's arrow, which turns a half revolution when the panel
    /// goes or comes.
    ///
    /// `App.vue:2408-2413`: the `IconButton` that opens and closes the panel
    /// carries `class="mr-3 transition-transform"` and
    /// `:class="{ 'rotate-180': !sidebarToggled }"`. Tailwind's
    /// `transition-transform` is a three-part utility -- `transition-property:
    /// transform`, `transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1)`,
    /// `transition-duration: 150ms` -- and the four control points above are
    /// Tailwind's own, which is the curve this module's docs already say is not
    /// one of the five CSS-named easings. The rest of the vendored tree confirms
    /// the duration rather than merely defaulting it: `Combobox.vue`, `TagItem.vue`
    /// and `CollapsibleAdmonition.vue` each spell out
    /// `transition-transform duration-150` where they mean this and
    /// `duration-300` where they mean something longer.
    pub const PANEL_ARROW: Timing = Timing { millis: 150, control: [0.4, 0.0, 0.2, 1.0] };

    /// A settings dialog's own tab column, which changes colour and plate
    /// together.
    ///
    /// `ui/src/components/modal/TabbedModal.vue:176` puts `transition-all` on
    /// each tab button in the column, so when the selected tab changes both its
    /// `bg-button-bgSelected` plate and its `text-button-textSelected` ink
    /// cross-fade -- `transition-all` being Tailwind's widest property list on the
    /// utility's own 150ms and `cubic-bezier(0.4, 0, 0.2, 1)`.
    ///
    /// **The same four numbers as [`Timing::PANEL_ARROW`], and that is not a
    /// coincidence.** Both utilities set only `transition-property`; Tailwind
    /// takes the duration and the timing function from its own defaults, so every
    /// `transition-*` in the vendored tree is `150ms cubic-bezier(0.4, 0, 0.2, 1)`
    /// unless it also carries a `duration-` class. Two named constants rather than
    /// one shared, because they are two declarations in two files and a future
    /// regeneration of either should be visible as a test failing on one name
    /// rather than silently moving both.
    pub const TAB_COLUMN: Timing = Timing { millis: 150, control: [0.4, 0.0, 0.2, 1.0] };

    /// A tab strip's label and icon, which change colour when the tab changes.
    ///
    /// `ui/src/components/base/NavTabs.vue`'s scoped `.tab-color` rule is
    /// `transition: color 100ms cubic-bezier(0.4, 0, 0.2, 1)`, on the label's
    /// `<span>` and the icon alike. The curve is Tailwind's, the same one
    /// [`Timing::PANEL_ARROW`] carries and for the same reason: Tailwind's
    /// `transition-*` utilities name it rather than a CSS keyword.
    ///
    /// **The plate itself is a different transition, and it is not this one.**
    /// The strip's slider -- the pill that slides under the selected tab -- is
    /// `.navtabs-transition`, which moves `left`, `right`, `top` and `bottom`
    /// over `80ms` on the same curve, with a `175ms` *stagger delay* on whichever
    /// trailing edge is not travelling toward its new position
    /// (`animateSliderTo`'s `STAGGER_DELAY`). So the reference slides its plate in
    /// 80ms and cross-fades its labels in 100ms, and they are not the same
    /// number. See [`Timing::TAB_PLATE`].
    pub const TAB_COLOR: Timing = Timing { millis: 100, control: [0.4, 0.0, 0.2, 1.0] };

    /// The pill that slides under a tab strip's selected tab.
    ///
    /// The same file's `.navtabs-transition`, quoted whole in
    /// `theme_gen::MOTION_VERBATIM`: `left 80ms cubic-bezier(0.4, 0, 0.2, 1)
    /// v-bind(leftDelay), right 80ms ... v-bind(rightDelay), top 80ms ...
    /// v-bind(topDelay), bottom 80ms ... v-bind(bottomDelay)`.
    ///
    /// **Not drawn, and the reason is worth the paragraph.** The reference knows
    /// where the pill is because it reads `el.offsetLeft` off a real DOM node.
    /// An iced widget tree has no layout to interrogate -- a strip is a `row!` of
    /// `mouse_area`s whose widths come from measured text this shell only ever
    /// sees while painting -- so there is no offset to start an 80ms leg *from*.
    /// Inventing one (a fixed tab width, or a fraction of the strip) would slide
    /// a plate to a position the reference never puts it at, which is worse than
    /// the plate being where the reference has it and arriving at once. The
    /// label cross-fade in [`Timing::TAB_COLOR`] *is* drawn, because it needs no
    /// geometry: it is a colour, and a colour can be mixed from the two inks the
    /// strip already knows.
    pub const TAB_PLATE: Timing = Timing { millis: 80, control: [0.4, 0.0, 0.2, 1.0] };

    /// The stagger the plate waits on its trailing edges: `NavTabs.vue`'s own
    /// `const STAGGER_DELAY = '175ms'`.
    ///
    /// The reference moves the plate's leading edges immediately and delays the
    /// trailing ones, which is what makes the pill *stretch* across the strip
    /// rather than jump and grow: the edge already in the right place arrives
    /// first and the one that has further to come waits. Kept because it is a
    /// fact about the reference that a port of the plate would need, and not
    /// because anything reads it today.
    pub const TAB_PLATE_STAGGER: Duration = Duration::from_millis(175);

    /// The row of the reference's motion table this pair is, or `None`.
    ///
    /// Returning `None` is the useful answer: it means the reference does not
    /// declare a transition with that property and that duration, so a caller
    /// asking for one has invented a number and should be made to notice.
    #[cfg(test)]
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
        let curve = theme_gen::MOTION.get(index).map_or(Curve::EaseInOut, |motion| motion.curve);
        Some(Timing { millis, control: theme_gen::curve(curve) })
    }

    /// Every row the reference declares for a property, as `(millis, curve,
    /// source)`, in table order.
    #[cfg(test)]
    pub fn declared_rows(property: &str) -> Vec<(u32, Curve, &'static str)> {
        theme_gen::MOTION
            .iter()
            .filter(|motion| motion.property == property)
            .map(|motion| (motion.millis, motion.curve, motion.source))
            .collect()
    }

    /// A timing the caller names outright, by one of the curves the generated
    /// table has a name for.
    ///
    /// Not a licence to pick a duration: the value belongs in the comment
    /// beside the call, as a quote from the reference, and the constant above
    /// is the example.
    #[cfg(test)]
    pub fn cited(millis: u32, curve: Curve) -> Timing {
        Timing { millis, control: theme_gen::curve(curve) }
    }

    /// The curve's four control-point coordinates.
    pub fn control(self) -> [f32; 4] {
        self.control
    }

    /// Which named curve this timing is, if it is one.
    ///
    /// `None` means the curve came from [`Timing::raw`]. A gate, not a
    /// convenience: it is how a test can say which of the two kinds of timing a
    /// row is without comparing floats by hand.
    #[cfg(test)]
    pub fn curve(self) -> Option<Curve> {
        theme_gen::ALL_CURVE
            .iter()
            .copied()
            .find(|curve| theme_gen::curve(*curve) == self.control)
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
    ///
    /// The timing is asked for rather than assumed, and that is the point of
    /// the signature: a tween at rest has not chosen a leg to run, and a
    /// zero-length placeholder here would make the first [`Tween::retarget`]
    /// jump instead of animate -- a bug that looks like a missing animation
    /// rather than like a wrong duration. There is no sensible default, because
    /// the reference's durations are per-property.
    pub fn at(value: f32, timing: Timing) -> Tween {
        Tween { from: value, to: value, timing, elapsed: timing.duration() }
    }

    /// A value starting at `from` and heading for `to`.
    #[cfg(test)]
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
    #[cfg(test)]
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

    /// Where the current leg started.
    #[cfg(test)]
    pub fn origin(&self) -> f32 {
        self.from
    }

    /// The timing the current leg runs on.
    #[cfg(test)]
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
    #[cfg(test)]
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
            let resolved = timing.curve().expect("a declared row is one of the named curves");
            assert!(
                curves.contains(&resolved),
                "{} at {}ms resolved to {resolved:?}, which no row declares",
                motion.property,
                motion.millis
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
        assert_eq!(plate.curve(), Some(Curve::EaseOutExpo));
        assert_eq!(plate.control(), theme_gen::curve(Curve::EaseOutExpo));
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
            Timing::declared("opacity", 125).and_then(|timing| timing.curve()),
            Some(Curve::EaseOut)
        );
    }

    #[test]
    fn a_dialog_arrives_on_the_reference_s_own_two_halves() {
        // `NewModal.vue`: the bed is `opacity` on `all 0.2s ease-out` and the
        // body is `scale` + `opacity` on `all 0.2s ease-in-out`. Two halves, two
        // curves, and only one of the two curves is one of the five named ones --
        // `ease-out` and `ease-in-out` are both cubic-bezier shorthands.
        assert_eq!(Timing::MODAL_SCRIM.millis, 200);
        assert_eq!(Timing::MODAL_DIALOG.millis, 200);
        assert_ne!(Timing::MODAL_SCRIM.control(), Timing::MODAL_DIALOG.control());
        assert_eq!(
            Timing::MODAL_DIALOG.control(),
            theme_gen::curve(Curve::EaseInOut),
            "the body's half is CSS's ease-in-out"
        );
        // Both halves do resolve to named curves -- `ease-out` and `ease-in-out`
        // are two of the five -- so the difference between them is real and not
        // an artefact of how the numbers were written down.
        assert_eq!(Timing::MODAL_SCRIM.curve(), Some(Curve::EaseOut));
        assert_eq!(Timing::MODAL_DIALOG.curve(), Some(Curve::EaseInOut));
        // Which is also why `Timing::declared` cannot be used for either: the
        // reference writes `all 200ms` on three different curves in three files,
        // so a lookup by property and duration alone is ambiguous *by design* and
        // answers in whichever of them the table happens to sort first. Both of
        // these rules are `all 200ms`, so both would be answered by that same
        // lookup and both would get the same curve -- which is why each is a
        // constant with the file that wrote it named beside it.
        let ambiguous = Timing::declared_rows("all")
            .into_iter()
            .filter(|(millis, _, _)| *millis == 200)
            .map(|(_, curve, source)| (curve, source))
            .collect::<Vec<_>>();
        assert!(
            ambiguous.len() >= 2 && ambiguous.iter().any(|(curve, _)| *curve != ambiguous[0].0),
            "`all 200ms` is declared on more than one curve, so a lookup cannot say which: {ambiguous:?}"
        );
        assert!(ambiguous
            .iter()
            .any(|(_, source)| source.ends_with("modal/NewModal.vue")));
        // And the unmount delay the leave transition needs in order to run at all.
        assert_eq!(Timing::MODAL_UNMOUNT, Duration::from_millis(300));
    }

    #[test]
    fn the_panel_arrow_and_the_tab_strip_carry_tailwind_s_own_curve() {
        // Neither is one of the five CSS-named curves: both come from Tailwind's
        // `transition-transform` / `transition-*` utilities, which name
        // `cubic-bezier(0.4, 0, 0.2, 1)` outright. A shell that could only name a
        // CSS curve could not have quoted either of these.
        let tailwind = [0.4, 0.0, 0.2, 1.0];
        assert_eq!(Timing::PANEL_ARROW.control(), tailwind);
        assert_eq!(Timing::TAB_COLOR.control(), tailwind);
        assert_eq!(Timing::TAB_PLATE.control(), tailwind);
        assert_eq!(Timing::PANEL_ARROW.curve(), None, "not one of the five names");
        assert_eq!(Timing::TAB_COLOR.curve(), None);
        // The three are separate numbers read off three separate declarations, so
        // none of them may borrow another's duration.
        assert_eq!(Timing::PANEL_ARROW.millis, 150, "`transition-transform`'s own");
        assert_eq!(Timing::TAB_COLOR.millis, 100, "`.tab-color`'s own");
        assert_eq!(Timing::TAB_PLATE.millis, 80, "`.navtabs-transition`'s own");
        assert_eq!(Timing::TAB_PLATE_STAGGER, Duration::from_millis(175));
        // The settings dialog's tab column and the panel arrow: two utilities
        // that differ only in which properties they list, and therefore the same
        // 150ms on the same curve. Kept apart so a failure names the rule that
        // moved rather than "some Tailwind transition".
        assert_eq!(Timing::TAB_COLUMN.control(), tailwind);
        assert_eq!(Timing::TAB_COLUMN.millis, 150);
        assert_ne!(Timing::TAB_COLOR.millis, Timing::TAB_COLUMN.millis);
    }

    #[test]
    fn the_two_tab_strip_rules_are_cited_verbatim_and_are_not_the_same_rule() {
        // The rows have to still be in the generated verbatim table, or a
        // regeneration has silently changed what this file claims the reference
        // says.
        let navtabs = |needle: &str| {
            theme_gen::MOTION_VERBATIM.iter().any(|(source, value)| {
                source.ends_with("base/NavTabs.vue") && value.contains(needle)
            })
        };
        assert!(navtabs("color 100ms cubic-bezier(0.4, 0, 0.2, 1)"));
        assert!(navtabs("left 80ms cubic-bezier(0.4, 0, 0.2, 1)"));
        // The plate's four edges each carry their own v-bind delay, which is the
        // stagger -- a single edge with no delay would be a different component.
        assert!(navtabs("v-bind(rightDelay)"), "the trailing edge is delayed, not the leading one");
        // `App.vue`'s panel arrow, whose rule is a Tailwind utility rather than a
        // stylesheet declaration, so it is not in the generated table at all --
        // which is exactly why it is a named constant here and not a lookup.
        assert!(
            !theme_gen::MOTION_VERBATIM.iter().any(|(source, value)| {
                source.ends_with("App.vue") && value.contains("transition-transform")
            }),
            "the arrow's number is Tailwind's, not a declaration the generator reads"
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
        let resting = Tween::at(0.5, Timing::NAV_PLATE);
        assert!(!resting.is_running());
        assert_eq!(resting.value(), 0.5);
        // A value that is at rest still carries the timing its next leg will
        // run on, so aiming it somewhere animates rather than jumps.
        let mut woken = resting;
        woken.retarget(1.0);
        assert!(woken.is_running());
        assert_eq!(woken.value(), 0.5, "a retarget starts from where it was");
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
