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
//! gates the frame clock in [`crate::shell`], so a shell whose switches have all
//! arrived asks for no frames at all.

use std::collections::HashMap;
use std::sync::Mutex;
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

// ---- The interaction clock ---------------------------------------------
//
// The reference's every control is `transition-[filter,transform] duration-150
// ease-out` (`ButtonFrame.vue`'s base classes), which means its hover and its
// press are *tweens*: the brightness lands on 1.25 over 150 ms, not in one
// frame. This shell had the right factors and the wrong clock — a hover was
// `filtered(role, 1.25)` on one frame and `filtered(role, 1.0)` on the next,
// which next to everything else reads as a flicker.
//
// The cost analysis that first skipped this conflated two events. A *pointer
// move* over a window arrives hundreds of times a second; a hover *change*
// arrives twice — once on enter, once on leave — and the shell already has
// both as messages (`SwitchHover`/`SwitchLeft` prove the pattern). Animating on
// enter/leave costs the same messages the snap already paid, plus 150 ms of
// frames from a subscription that only exists while something is moving.

/// How long a hover or a press takes to arrive, wall clock.
///
/// `ButtonFrame.vue`'s `duration-150`. The reference's `ease-out` is drawn as
/// the smoothstep below, which is what [`ease`] already is; the difference
/// between the two curves at this length is under a pixel of brightness.
pub const INTERACTION_DURATION: Duration = Duration::from_millis(150);

/// The shell's one interaction clock.
///
/// A process-wide handle rather than a field of the app, because the buttons
/// that read it carry it inside a stylesheet stored in the widget tree: they
/// are built from `&self` while `update` mutates everything else, so what a
/// style holds has to outlive every borrow the view takes. There is one
/// window, one pointer and one of these, which is also the honest model.
/// `OnceLock` rather than a `const` because a map cannot be built in a
/// constant context.
#[cfg(not(test))]
pub fn clock() -> &'static Mutex<Interactions> {
    static CLOCK: std::sync::OnceLock<Mutex<Interactions>> = std::sync::OnceLock::new();
    CLOCK.get_or_init(|| Mutex::new(Interactions::default()))
}

/// The same clock for a test: this thread's, which is this test's.
///
/// The window's clock is the process's, and a test binary runs its tests side
/// by side. That made it every test's clock too: a test that navigated -- which
/// forgets every crossing in the clock, the whole of `Shell::forget_pointer` --
/// could settle a crossing a test beside it was in the middle of asserting, and
/// the assertion failed about as often as the machine was fast. A test *is* its
/// own window, so this is the model made literal rather than a lock every test
/// has to remember to take.
///
/// The cell is leaked because callers hold a `&'static Mutex`: the stylesheets
/// do, on purpose, so the clock outlives every borrow a view takes. One mutex
/// per test thread is bounded by the number of tests the runner has in flight.
#[cfg(test)]
pub fn clock() -> &'static Mutex<Interactions> {
    thread_local! {
        static CLOCK: &'static Mutex<Interactions> =
            Box::leak(Box::new(Mutex::new(Interactions::default())));
    }
    CLOCK.with(|clock| *clock)
}

/// One control's hover/press tween, as the stylesheet sees it: the factor the
/// filter is currently multiplied by, in `1.0..=1.25` (dark) or `0.9..=1.0`
/// (light).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Tween {
    /// The factor the tween is heading for: 1.0 resting, `hover_brightness()`
    /// hovered, `PRESS_BRIGHTNESS` pressed.
    to: f32,
    /// The factor the tween started from.
    from: f32,
    /// The factor the control is drawn at this frame.
    progress: f32,
    /// When the tween began, or `None` once it has arrived.
    began: Option<Instant>,
    /// What the pointer last reported about this control.
    pointed: Pointed,
}

impl Tween {
    fn settled(factor: f32) -> Tween {
        Tween {
            to: factor,
            from: factor,
            progress: factor,
            began: None,
            pointed: Pointed::rest(),
        }
    }
}

/// The pointer's last report about one control, as the clock remembers it.
///
/// [`Interactions::factor`] does not need this: the shell it was written for
/// hands the pointer's state in from iced's own `Status`, which survives a
/// rebuild. A page's controls are `mouse_area`s that report a crossing as a
/// *message*, so the clock is the only thing that saw it — and it has to keep
/// it, because the view that draws the control is rebuilt from scratch every
/// frame and cannot be asked what the pointer is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pointed {
    /// The pointer is inside the control's bounds.
    hovered: bool,
    /// The left button went down on it and has not come back up.
    pressed: bool,
    /// The hover end this control was told, which is the `--hover-brightness`
    /// a card's scoped style may override.
    hover: f32,
}

impl Pointed {
    /// Nothing on this control.
    fn rest() -> Pointed {
        Pointed { hovered: false, pressed: false, hover: crate::theme::hover_brightness() }
    }
}

/// How far *through* its hover a factor is, for a control the clock remembers.
///
/// The same inversion [`Interactions::hover_progress_with_hover`] performs, on
/// the remembered state rather than on one handed in.
fn hover_fraction(factor: f32, pointed: Pointed) -> f32 {
    // A press is dimmer than rest and is not a position: the structure a hover
    // moved stays where it is while the control is held.
    if pointed.pressed {
        return if pointed.hovered { 1.0 } else { 0.0 };
    }
    // A theme whose hover is its rest state has nothing to invert, and nothing
    // that moves either: the fraction is the state itself.
    if (pointed.hover - 1.0).abs() < f32::EPSILON {
        return if pointed.hovered { 1.0 } else { 0.0 };
    }
    ((factor - 1.0) / (pointed.hover - 1.0)).clamp(0.0, 1.0)
}

/// Hover and press, eased, for every control that asks.
///
/// One clock for the whole shell rather than one per control, because the
/// controls a pointer can be over number one: the enter/leave messages move a
/// *single* key, and everything else is at rest and costs a map lookup. Keyed
/// by a stable string the view supplies, for [`SwitchAnim`]'s reason — a key
/// derived from tree position would animate whatever lands in the slot.
#[derive(Debug, Clone, Default)]
pub struct Interactions {
    tweens: HashMap<&'static str, Tween>,
}

impl Interactions {
    /// The factor the control `id` is drawn at, given its pointer state.
    ///
    /// A control that has never been touched is at rest — the first paint must
    /// not animate — and a control whose state matches its tween is simply at
    /// its end.
    pub fn factor(&self, id: &str, hovered: bool, pressed: bool) -> f32 {
        self.factor_with_hover(id, hovered, pressed, crate::theme::hover_brightness())
    }

    /// The factor the control is drawn at with a locally scoped hover end.
    ///
    /// Instance cards and checklist rows override the reference global
    /// hover brightness, so their scoped value travels with the style rather
    /// than leaking into every other control.
    pub fn factor_with_hover(
        &self,
        id: &str,
        hovered: bool,
        pressed: bool,
        hover_factor: f32,
    ) -> f32 {
        let target = Self::target(hovered, pressed, hover_factor);
        match self.tweens.get(id) {
            Some(tween) if tween.to == target && tween.began.is_none() => tween.progress,
            Some(tween) if tween.began.is_none() && tween.to != target => {
                // Stale but settled: the state changed and no one called set
                // yet (a fresh frame arrived before the update ran). Draw the
                // rest factor rather than the tween's — the next set will
                // start from here, so nothing jumps.
                target
            }
            Some(tween) => tween.progress,
            None => target,
        }
    }

    /// The factor a control with this pointer state is *supposed* to be at.
    fn target(hovered: bool, pressed: bool, hover_factor: f32) -> f32 {
        if pressed {
            crate::theme::PRESS_BRIGHTNESS
        } else if hovered {
            hover_factor
        } else {
            1.0
        }
    }

    /// Record the pointer's arrival on or departure from `id`, and start the
    /// tween if the target moved.
    pub fn set(&mut self, id: &'static str, hovered: bool, pressed: bool, now: Instant) {
        self.set_with_hover(id, hovered, pressed, now, crate::theme::hover_brightness());
    }

    /// Record pointer state with a locally scoped hover end.
    pub fn set_with_hover(
        &mut self,
        id: &'static str,
        hovered: bool,
        pressed: bool,
        now: Instant,
        hover_factor: f32,
    ) {
        let target = Self::target(hovered, pressed, hover_factor);
        let tween = self
            .tweens
            .entry(id)
            .or_insert_with(|| Tween::settled(1.0));
        // The remembered state is the pointer's report, which is new even when
        // the factor it implies is the one already drawn: a control the pointer
        // is on and one it has just left both sit at `1.0` in a light theme.
        tween.pointed = Pointed { hovered, pressed, hover: hover_factor };
        if tween.to == target && tween.began.is_none() {
            return;
        }
        // From where the control is drawn: a press inside a hover starts
        // from the hover brightness, and a fast hover-press-release reverses
        // from the pixel it is on.
        tween.from = if tween.began.is_none() { tween.to } else { tween.progress };
        // A tween whose ends are equal is already there.
        tween.to = target;
        tween.progress = tween.from;
        tween.began = if (target - tween.from).abs() < 0.001 { None } else { Some(now) };
    }

    /// How far along a hover is, `0.0..=1.0`.
    ///
    /// The clock's own number is a *brightness* — rest to
    /// [`crate::theme::hover_brightness`] — which is the right thing for a
    /// filter and the wrong thing for a position. A control whose hover moves
    /// something instead of tinting it (the settings switch's knob swells in
    /// its track) needs the fraction of the way there, and that is this: the
    /// same tween, its scale inverted.
    pub fn hover_progress(&self, id: &str, hovered: bool, pressed: bool) -> f32 {
        self.hover_progress_with_hover(id, hovered, pressed, crate::theme::hover_brightness())
    }

    /// How far along a hover is when its end factor is scoped to one control.
    pub fn hover_progress_with_hover(
        &self,
        id: &str,
        hovered: bool,
        pressed: bool,
        hover_factor: f32,
    ) -> f32 {
        hover_fraction(
            self.factor_with_hover(id, hovered, pressed, hover_factor),
            Pointed { hovered, pressed, hover: hover_factor },
        )
    }

    /// The factor and hover fraction a control is drawn at, from what the
    /// pointer last reported about it and nothing else.
    ///
    /// This is the reading a *page* makes, and the difference from
    /// [`Interactions::factor`] is where the pointer's state comes from: the
    /// shell this clock was written for hands it in from iced's own `Status`,
    /// which survives a rebuild, and a page's controls are `mouse_area`s whose
    /// crossing is a message. The messages came here, so here is where the
    /// answer is; a control nobody has reported is simply at rest.
    pub fn drawn(&self, id: &str) -> (f32, f32) {
        let Some(tween) = self.tweens.get(id) else {
            return (1.0, 0.0);
        };
        if tween.began.is_some() {
            return (tween.progress, hover_fraction(tween.progress, tween.pointed));
        }
        // Settled: what to draw is the state the pointer last reported, not the
        // factor the tween happened to end on. A control drawn from a stale
        // report is the failure this avoids, and it is why a page can be
        // rebuilt every frame without carrying a map of its own.
        let factor =
            Self::target(tween.pointed.hovered, tween.pointed.pressed, tween.pointed.hover);
        (factor, hover_fraction(factor, tween.pointed))
    }

    /// Forget what the pointer was doing with every control.
    ///
    /// Called when the pane changes: a control that was lit when the page left
    /// is not drawn on the new one, and a tween in the air belonged to the page
    /// that is gone. This is the gentler half of [`Interactions::clear`]: the
    /// names stay, settled at rest, rather than ceasing to exist under a reader
    /// that is holding them.
    pub fn forget_pointer(&mut self) {
        for tween in self.tweens.values_mut() {
            tween.pointed = Pointed::rest();
            tween.to = 1.0;
            tween.from = 1.0;
            tween.progress = 1.0;
            tween.began = None;
        }
    }

    /// Whether any control is mid-tween.
    pub fn animating(&self) -> bool {
        self.tweens.values().any(|tween| tween.began.is_some())
    }

    /// Advance every tween to `now`, returning whether any still moves.
    pub fn tick(&mut self, now: Instant) -> bool {
        let deadline = INTERACTION_DURATION.as_secs_f32();
        let mut moving = false;
        for tween in self.tweens.values_mut() {
            let Some(began) = tween.began else {
                continue;
            };
            let elapsed = now.saturating_duration_since(began).as_secs_f32();
            let progress = if deadline > 0.0 { (elapsed / deadline).clamp(0.0, 1.0) } else { 1.0 };
            tween.progress = tween.from + (tween.to - tween.from) * ease(progress);
            if progress >= 1.0 {
                tween.progress = tween.to;
                tween.began = None;
            } else {
                moving = true;
            }
        }
        moving
    }

    /// Forget every tween.
    ///
    /// Nothing in the shell calls this, and that is worth writing down rather
    /// than leaving as a gap: a page change used to clear the map, and the
    /// comment there claimed a stale entry would draw a control hovered with no
    /// pointer on it. It would not — [`Interactions::factor`] answers a settled
    /// tween whose end disagrees with the pointer's state with the *state*, so
    /// what a page change leaves behind is a settled entry that draws exactly
    /// what the fresh page would. The only caller is a test starting from a map
    /// it knows, and [`clock`] hands every test its own, so there is nobody
    /// else's tween here to pull the pixels out from under.
    pub fn clear(&mut self) {
        self.tweens.clear();
    }
}

// ---- The modal's arrival -----------------------------------------------
//
// The reference's modal arrives as an opacity on a backdrop and a transform on
// the dialog. `ui/src/components/modal/NewModal.vue` states both: the overlay is
// `opacity: 0` to `1` on `transition: all 0.2s ease-out`, and the dialog body
// sits at `scale: 0.97` with `opacity: 0` until `.shown` takes it to `scale: 1`
// on `transition: all 0.2s ease-in-out`. The shell had the backdrop pop from
// nothing to 64% black in one frame and the dialog appear with it, which is the
// single most visible way a shell says "I am not the reference". iced cannot
// transform, so the scale is drawn as the dialog arriving at its seat the way
// the press is — brightness — and the backdrop is drawn as what it actually is:
// an opacity.

/// How long the modal takes to arrive and to leave, wall clock.
///
/// The reference's own `0.2s` on both the overlay and the dialog body. The two
/// move together because the dialog's `scale`/`opacity` and the overlay's
/// `opacity` are started by the same class change.
pub const MODAL_DURATION: Duration = Duration::from_millis(200);

/// The modal's arrival, on the same deadline pattern as everything above.
///
/// `progress` is the backdrop's opacity fraction (`0..=1`) and the dialog's
/// arrival (`0` = the pressed dimness, `1` = seated). A modal that has never
/// been open reports settled at *closed*: the first frame of an opening modal
/// must be the first frame of its tween, not its end.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModalAnim {
    open: bool,
    progress: f32,
    began: Option<Instant>,
}

impl ModalAnim {
    /// The backdrop's opacity fraction right now.
    pub fn backdrop(&self) -> f32 {
        self.progress
    }

    /// The dialog's arrival factor, `0..=1`, for [`crate::theme::filtered`].
    ///
    /// The dialog arrives *from* the press's dimness — `0.8` — because that is
    /// what `scale: 0.97` reads as when the toolkit cannot scale a widget inside
    /// its own box: close enough to see it land, without a transform.
    pub fn dialog_factor(&self) -> f32 {
        crate::theme::PRESS_BRIGHTNESS
            + (1.0 - crate::theme::PRESS_BRIGHTNESS) * self.progress
    }

    /// Whether the modal is (still) on screen. `false` is what retires it.
    pub fn open(&self) -> bool {
        self.open || self.began.is_some()
    }

    /// Record that the modal opened or closed, and start its tween.
    pub fn set(&mut self, open: bool, now: Instant) {
        if self.open == open {
            return;
        }
        self.open = open;
        self.began = Some(now);
    }

    /// Whether the tween is moving.
    pub fn animating(&self) -> bool {
        self.began.is_some()
    }

    /// Advance to `now`, returning whether the tween still moves.
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some(began) = self.began else {
            return false;
        };
        let deadline = MODAL_DURATION.as_secs_f32();
        let elapsed = now.saturating_duration_since(began).as_secs_f32();
        let linear = if deadline > 0.0 { (elapsed / deadline).clamp(0.0, 1.0) } else { 1.0 };
        let target = if self.open { 1.0 } else { 0.0 };
        self.progress = self.progress + (target - self.progress) * ease(linear);
        if linear >= 1.0 {
            self.progress = target;
            self.began = None;
            return false;
        }
        true
    }
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

    #[test]
    fn an_untouched_control_is_at_rest() {
        let clock = Interactions::default();
        assert_eq!(clock.factor("a", false, false), 1.0);
        assert_eq!(clock.factor("a", true, false), crate::theme::hover_brightness());
        assert!(!clock.animating(), "nothing has been told anything");
    }

    #[test]
    fn finish_scoped_hover_factors_do_not_share_card_targets() {
        let mut clock = Interactions::default();
        let start = Instant::now();
        clock.set_with_hover("card-a", true, false, start, 1.1);
        clock.set_with_hover("card-b", true, false, start, 1.25);
        assert_eq!(clock.factor_with_hover("card-a", true, false, 1.1), 1.0);
        assert_eq!(clock.factor_with_hover("card-b", true, false, 1.25), 1.0);

        clock.tick(start + INTERACTION_DURATION);
        assert_eq!(clock.factor_with_hover("card-a", true, false, 1.1), 1.1);
        assert_eq!(clock.factor_with_hover("card-b", true, false, 1.25), 1.25);
    }
    #[test]
    fn a_hover_arrives_over_the_deadline_not_in_a_frame() {
        let mut clock = Interactions::default();
        let start = Instant::now();
        clock.set("a", true, false, start);
        assert!(clock.animating());
        assert_eq!(clock.factor("a", true, false), 1.0, "the tween begins at rest");

        // Halfway is strictly between rest and the hover factor.
        clock.tick(start + INTERACTION_DURATION / 2);
        let middle = clock.factor("a", true, false);
        let end = crate::theme::hover_brightness();
        assert!(middle > 1.0 && middle < end, "got {middle}, rest 1.0, hover {end}");

        // And the deadline ends it, exactly on the factor.
        assert!(!clock.tick(start + INTERACTION_DURATION));
        assert_eq!(clock.factor("a", true, false), end);
        assert!(!clock.animating(), "a settled tween must not hold frames open");
    }

    #[test]
    fn a_press_borrowed_from_a_hover_starts_from_the_hover() {
        // A press while hovered must not fall back to rest brightness first:
        // the tween starts from the factor the control is *drawn* at.
        let mut clock = Interactions::default();
        let start = Instant::now();
        clock.set("a", true, false, start);
        clock.tick(start + INTERACTION_DURATION);
        let hovered = clock.factor("a", true, false);

        clock.set("a", true, true, start);
        assert_eq!(clock.factor("a", true, true), hovered, "the press starts from the hover");
        clock.tick(start + INTERACTION_DURATION);
        assert_eq!(clock.factor("a", true, true), crate::theme::PRESS_BRIGHTNESS);
    }

    #[test]
    fn a_leave_turns_around_from_where_it_is() {
        let mut clock = Interactions::default();
        let start = Instant::now();
        clock.set("a", true, false, start);
        clock.tick(start + INTERACTION_DURATION / 2);
        let halfway = clock.factor("a", true, false);

        // The pointer leaves half a tween in: the return trip starts from the
        // halfway factor, and it is a full tween of its own.
        clock.set("a", false, false, start + INTERACTION_DURATION / 2);
        assert_eq!(clock.factor("a", false, false), halfway);
        assert!(clock.animating());
        clock.tick(start + INTERACTION_DURATION / 2 + INTERACTION_DURATION);
        assert_eq!(clock.factor("a", false, false), 1.0);
        assert!(!clock.animating());
    }

    #[test]
    fn controls_do_not_share_a_key() {
        let mut clock = Interactions::default();
        let start = Instant::now();
        clock.set("a", true, false, start);
        clock.tick(start + INTERACTION_DURATION);
        clock.set("b", true, false, start);
        assert_eq!(clock.factor("a", true, false), crate::theme::hover_brightness());
        assert_eq!(clock.factor("b", true, false), 1.0, "b's tween has just begun");
    }

    #[test]
    fn the_hover_fraction_runs_from_nothing_to_everything() {
        // What a control whose hover *moves* reads: the switch's knob grows by
        // this fraction, so it has to be 0 at rest, 1 when the hover has
        // arrived, and strictly between while the tween is in the air —
        // whichever way the theme's hover brightness points.
        let mut clock = Interactions::default();
        let start = at(0);
        assert_eq!(clock.hover_progress("a", false, false), 0.0, "at rest");
        clock.set("a", true, false, start);
        assert_eq!(clock.hover_progress("a", true, false), 0.0, "the tween starts at rest");
        clock.tick(start + INTERACTION_DURATION / 2);
        let middle = clock.hover_progress("a", true, false);
        assert!(middle > 0.0 && middle < 1.0, "got {middle}");
        clock.tick(start + INTERACTION_DURATION);
        assert_eq!(clock.hover_progress("a", true, false), 1.0);
        // A press is not a position: a held control keeps the hover's own
        // structure, whichever theme it is drawn in.
        clock.set("a", true, true, start);
        assert_eq!(clock.hover_progress("a", true, true), 1.0);
        // Leaving is the return trip, and it lands back at nothing.
        clock.set("a", false, false, start + INTERACTION_DURATION);
        clock.tick(start + 2 * INTERACTION_DURATION);
        assert_eq!(clock.hover_progress("a", false, false), 0.0);
    }

    #[test]
    fn a_control_nobody_reported_is_at_rest() {
        // What a page reads for a control before any crossing: rest, not a
        // hover the clock cannot know about.
        let clock = Interactions::default();
        assert_eq!(clock.drawn("nowhere"), (1.0, 0.0));
    }

    #[test]
    fn a_page_reads_the_hover_off_its_own_reports() {
        // The pages' whole reading, end to end: a crossing message arrives, the
        // tween is in the air, and the view asks the clock rather than holding
        // the state itself.
        let mut clock = Interactions::default();
        let start = Instant::now();
        let hover = crate::theme::hover_brightness();
        clock.set("page:save", true, false, start);
        assert_eq!(clock.drawn("page:save"), (1.0, 0.0), "the tween begins at rest");

        clock.tick(start + INTERACTION_DURATION / 2);
        let (factor, progress) = clock.drawn("page:save");
        assert!(factor > 1.0 && factor < hover, "got {factor}");
        assert!(progress > 0.0 && progress < 1.0, "got {progress}");

        assert!(!clock.tick(start + INTERACTION_DURATION));
        assert_eq!(clock.drawn("page:save"), (hover, 1.0), "arrived, and hovered");
        // The pointer leaves: the return trip, and it lands at rest.
        clock.set("page:save", false, false, start + INTERACTION_DURATION);
        clock.tick(start + 2 * INTERACTION_DURATION);
        assert_eq!(clock.drawn("page:save"), (1.0, 0.0));
        assert!(!clock.animating(), "a settled page control holds no frames");
    }

    #[test]
    fn forgetting_the_pointer_puts_every_control_back_at_rest() {
        // What a page change does: the control that was lit belonged to the page
        // that is gone, and a tween in the air goes with it.
        let mut clock = Interactions::default();
        let start = Instant::now();
        clock.set("page:save", true, false, start);
        assert!(clock.animating());
        clock.forget_pointer();
        assert_eq!(clock.drawn("page:save"), (1.0, 0.0));
        assert!(!clock.animating());
    }

    #[test]
    fn clear_forgets_every_key() {
        let mut clock = Interactions::default();
        let start = Instant::now();
        clock.set("a", true, false, start);
        clock.clear();
        assert!(!clock.animating());
        assert_eq!(clock.factor("a", true, false), crate::theme::hover_brightness());
    }

    #[test]
    fn a_clock_belongs_to_the_test_that_reads_it() {
        // The isolation the suite relies on. A test binary runs its tests side
        // by side, and one of them navigating forgets every pointer in the
        // clock it reads -- so a thread that has crossed nothing must not see
        // the crossing another test is in the middle of asserting. The clock a
        // test reads is the thread's own, which is that test's own window.
        let mine = clock();
        {
            let mut clock = mine.lock().expect("the clock");
            clock.clear();
            clock.set("probe", true, false, Instant::now());
        }
        assert!(mine.lock().expect("the clock").animating());

        let elsewhere = std::thread::spawn(|| clock().lock().expect("the clock").animating())
            .join()
            .expect("the thread");
        assert!(!elsewhere, "a thread that crossed nothing starts at rest");
        // And the tween this test started is still the one it was reading.
        assert!(mine.lock().expect("the clock").animating());
    }

    #[test]
    fn a_settled_tween_draws_the_pointer_state_not_its_own_end() {
        // The property that makes the map safe to leave alone for the life of
        // the process: a tween that has arrived is not a memory of where the
        // pointer was. Asked about a control the pointer is no longer on, it
        // answers at rest; asked about one it is on, it answers hovered — even
        // if nothing called `set` for the new state, which is what a page that
        // redraws a control without a fresh report looks like.
        let mut clock = Interactions::default();
        let start = Instant::now();
        clock.set("a", true, false, start);
        clock.tick(start + INTERACTION_DURATION);
        assert_eq!(clock.factor("a", true, false), crate::theme::hover_brightness());
        assert_eq!(clock.factor("a", false, false), 1.0, "left behind, drawn at rest");
        assert_eq!(clock.factor("a", false, true), crate::theme::PRESS_BRIGHTNESS);
        assert_eq!(clock.factor("nobody", true, false), crate::theme::hover_brightness());
    }

    #[test]
    fn a_modal_fades_in_over_the_deadline_and_lands_seated() {
        let mut modal = ModalAnim::default();
        let start = Instant::now();
        modal.set(true, start);
        assert!(modal.open(), "an opening modal is on screen even before its first tick");
        assert_eq!(modal.backdrop(), 0.0, "the fade begins from nothing");

        modal.tick(start + MODAL_DURATION / 2);
        let backdrop = modal.backdrop();
        assert!(backdrop > 0.05 && backdrop < 0.95, "got {backdrop}");
        let factor = modal.dialog_factor();
        assert!(factor > crate::theme::PRESS_BRIGHTNESS && factor < 1.0);

        assert!(!modal.tick(start + MODAL_DURATION));
        assert_eq!(modal.backdrop(), 1.0);
        assert_eq!(modal.dialog_factor(), 1.0);
        assert!(!modal.animating());
    }

    #[test]
    fn a_modal_fades_out_and_is_gone_only_at_the_end() {
        // The leave is why `open()` exists as a separate question from the
        // flag: a modal told to close must stay on screen *while it fades*, or
        // the fade-out would never be seen.
        let mut modal = ModalAnim::default();
        let start = Instant::now();
        modal.set(true, start);
        modal.tick(start + MODAL_DURATION);
        assert_eq!(modal.backdrop(), 1.0);

        modal.set(false, start);
        assert!(modal.open(), "a fading-out modal is still on screen");
        assert!(modal.animating());
        modal.tick(start + MODAL_DURATION);
        assert_eq!(modal.backdrop(), 0.0, "the backdrop ends at nothing");
        assert!(!modal.open(), "and the modal is gone");
        assert!(!modal.animating());
    }

    #[test]
    fn reopening_a_closing_modal_turns_it_around() {
        let mut modal = ModalAnim::default();
        let start = Instant::now();
        modal.set(true, start);
        modal.tick(start + MODAL_DURATION);
        modal.set(false, start);
        modal.tick(start + MODAL_DURATION / 2);
        let halfway = modal.backdrop();
        assert!(halfway > 0.0 && halfway < 1.0);

        modal.set(true, start + MODAL_DURATION / 2);
        assert_eq!(modal.backdrop(), halfway, "the turn keeps the position");
        modal.tick(start + MODAL_DURATION / 2 + MODAL_DURATION);
        assert_eq!(modal.backdrop(), 1.0);
    }
}
