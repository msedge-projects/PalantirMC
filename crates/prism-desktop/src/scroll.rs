//! Scroll policy for the desktop shell: how much to draw, and how to move.
//!
//! iced 0.12's `Scrollable` consumes wheel events itself and applies each
//! notch as an instant 60-pixel jump. That is why scrolling a long page on
//! Windows feels steppy: the content teleports once per notch rather than
//! gliding. Nothing in iced exposes a friction/tween setting, but the
//! scrollable does hand the event to its *content* first and stand down if the
//! content captures it — so [`guard`] wraps a page's content, swallows the
//! wheel, and lets the shell own the offset. The shell then eases toward a
//! target, and only asks for frames while something is still moving.
//!
//! The tween is a function of **time**, and whether to have one at all is a
//! function of **this machine**. Both matter, and the second is the one that
//! took measuring:
//!
//! * A frame is not free. iced rebuilds the interface for every message it
//!   receives, rasterises the window and blits it, and with no usable GPU the
//!   rasteriser does that on the CPU. A gesture therefore costs one frame per
//!   frame of animation, so animating at a rate the machine cannot draw buys
//!   nothing and costs a pegged core.
//! * [`DURATION`] is a deadline rather than a frame count, so a machine that
//!   answers slowly draws fewer, larger steps instead of a longer animation.
//!   After the deadline the gesture is over, whatever else happened.
//! * [`SMOOTH_FRAME`] is where animating stops being worth it. A machine whose
//!   frames are slower than that is not asked to glide at all: the offset moves
//!   inside the frame the wheel event already paid for. That is both cheaper
//!   and *less* laggy than an animation drawn at 14 frames a second, which is
//!   what the fixed frame count produced on the machine this was measured on —
//!   16 ms per tick against a measured 69 ms frame: eleven frames, three
//!   quarters of a second of one core, to show a single wheel notch.
//!
//! Deliberate limits, because each one is a way this can go wrong:
//!
//! * **Only the wheel is taken.** The scrollbar, dragging it, keyboard
//!   scrolling, touch and iced's own `scroll_to` (which the log page's
//!   autoscroll uses) all still work, because this does not replace
//!   `Scrollable` — it sits inside one.
//! * **No timer while nothing moves.** The animation is a subscription that
//!   exists only while [`ScrollAnim::animating`] is true, so an idle window
//!   does no work at all — and a machine classified as too slow never starts
//!   one.
//! * **Bounded, not physical.** Easing is against a deadline, so a gesture
//!   cannot settle forever and cannot grow with the distance it covers.

use std::time::{Duration, Instant};

use iced::advanced::widget::{tree, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::{event, Element, Event, Length, Rectangle, Size, Vector};

/// Maximum number of rows rendered by a virtualized-ish log view.
///
/// The log buffer may retain many more lines for troubleshooting, but drawing
/// all of them makes wheel input expensive. The view renders only the newest
/// window, which bounds layout/draw work per frame.
pub const LOG_RENDER_CAP: usize = 500;

/// Convert a buffered-line count into the number of rows the log page should
/// build. This is deliberately O(1) and allocation-free.
pub const fn visible_log_lines(total: usize) -> usize {
    if total < LOG_RENDER_CAP {
        total
    } else {
        LOG_RENDER_CAP
    }
}

/// Pixels one wheel notch is worth.
///
/// Matches iced's own `movement * 60.0`, so taking the wheel over does not
/// change how far a notch travels — only how it gets there.
pub const WHEEL_PIXELS_PER_NOTCH: f32 = 60.0;

/// How often the tween asks for a frame while it is gliding.
///
/// 16 ms is 60 frames a second, the rate a machine that can hold a glide is
/// expected to. It is a *request* rather than a guarantee: the position is
/// driven by the clock, so a machine that answers late draws fewer, larger
/// steps rather than falling behind.
pub const FRAME: Duration = Duration::from_millis(16);

/// How long a glide takes, wall clock, on a machine that can draw one.
///
/// About ten frames at 60 fps, three at 20 fps, and the same fifth of a second
/// either way — which is the whole reason this is a duration. A frame count
/// makes the *length* of the gesture depend on how fast the machine happens to
/// be, and on a slow renderer that turns a scroll into a slideshow.
pub const DURATION: Duration = Duration::from_millis(160);

/// The frame interval above which this machine is not asked to glide.
///
/// 24 ms is about 40 frames a second. Below that an animation reads as motion;
/// above it the step between two frames is most of the likely distance, so the
/// eye sees a jump that cost three frames to draw. The policy is therefore not
/// "animate more cheaply" but "do not animate here": the offset moves in the
/// frame the wheel event already paid for, which is both the cheapest and the
/// *least* laggy answer on a machine that cannot keep up.
///
/// This is measured rather than assumed — see [`ScrollAnim::tick`] — so a
/// machine with a working GPU gets the glide and a software-rasterised one gets
/// the instant step, from the same binary and with no setting to get wrong.
pub const SMOOTH_FRAME: Duration = Duration::from_millis(24);

/// Below this many pixels from the target, the animation is over.
///
/// Without a floor, a re-clamped target would leave the page a fraction of a
/// pixel from where it belongs and keep asking for frames.
pub const SETTLED: f32 = 0.5;

/// Where a page's scroll position is, and where it is going.
///
/// Plain numbers and a clock rather than a reference to any widget state, so
/// the whole easing policy is unit tested without a renderer or a window.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScrollAnim {
    /// The offset the content is drawn at.
    pub offset: f32,
    /// The offset it is easing toward.
    pub target: f32,
    /// Height of the content, as last reported by the scrollable.
    pub content_height: f32,
    /// Height of the visible area, as last reported by the scrollable.
    pub view_height: f32,
    /// The glide in progress, if there is one.
    glide: Option<Glide>,
    /// When the previous frame was handled, for measuring frame cost.
    last_tick: Option<Instant>,
    /// The fastest recent frame interval, or `None` until one is measured.
    frame_cost: Option<Duration>,
}

/// One glide: where it started, when, and the deadline it must meet.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Glide {
    from: f32,
    began: Instant,
    duration: Duration,
}

impl ScrollAnim {
    /// The largest offset that still shows content: never negative, so a page
    /// shorter than the window cannot scroll at all.
    pub fn max_offset(&self) -> f32 {
        (self.content_height - self.view_height).max(0.0)
    }

    /// Clamp `offset` into the scrollable range.
    pub fn clamp(&self, offset: f32) -> f32 {
        offset.clamp(0.0, self.max_offset())
    }

    /// Whether the offset is still moving.
    ///
    /// True while a glide is running, and true when a target the offset has not
    /// reached is sitting there with no glide to carry it — which happens when
    /// content shrank and its re-clamped target is somewhere else. Either way
    /// the answer decides whether a frame timer exists at all.
    pub fn animating(&self) -> bool {
        self.glide.is_some() || (self.target - self.offset).abs() > SETTLED
    }

    /// Take a wheel event, with the geometry it was reported in.
    ///
    /// The geometry comes from the same event rather than from a separate
    /// report, because iced only publishes viewport changes — a page nobody has
    /// scrolled yet has never reported one, and a wheel on it would otherwise
    /// move nothing.
    ///
    /// `now` is passed in rather than read here so that the whole policy is a
    /// function of its inputs, and a test can hold the clock still.
    pub fn wheel(&mut self, wheel: Wheel, now: Instant) {
        self.observe(wheel.content_height, wheel.view_height);
        self.scroll_notches(wheel.notches);
        self.begin(now);
    }

    /// Move the target by `notches`, positive upward.
    ///
    /// The target is clamped to the content; the *offset* is not touched,
    /// because the whole point is that it catches up smoothly.
    pub fn scroll_notches(&mut self, notches: f32) {
        self.target = self.clamp(self.target - notches * WHEEL_PIXELS_PER_NOTCH);
    }

    /// Start easing toward the target — or move there now, if this machine was
    /// measured too slow to draw the easing.
    fn begin(&mut self, now: Instant) {
        self.glide = None;
        if !self.animating() {
            // Already there. A wheel clamped out of range at the end of a page
            // must not start a timer for a gesture that moves nothing.
            return;
        }
        let duration = Self::glide_duration(self.frame_cost);
        if duration.is_zero() {
            // A glide here would be three frames of work to show one step at
            // 14 fps. Arriving inside the frame the wheel already paid for is
            // both cheaper and lower-latency than animating it.
            self.offset = self.target;
            return;
        }
        self.glide = Some(Glide { from: self.offset, began: now, duration });
    }

    /// How long a glide may take on a machine whose frames cost `cost`.
    ///
    /// Pure, so the policy can be exercised without a clock. `None` — nothing
    /// measured yet — is the optimistic answer, and deliberately so: the first
    /// gesture of a session glides, and measuring *it* is what every gesture
    /// afterwards is answered with.
    pub fn glide_duration(cost: Option<Duration>) -> Duration {
        match cost {
            Some(cost) if cost >= SMOOTH_FRAME => Duration::ZERO,
            _ => DURATION,
        }
    }

    /// Advance to `now` and report whether there is more to animate.
    ///
    /// The position is a function of elapsed time against the glide's deadline,
    /// so a machine answering slowly draws fewer, larger steps and still
    /// finishes on time, while a fast one draws more, smaller ones. Nothing
    /// here can make a gesture last longer than [`DURATION`], which is the
    /// property the frame-counted tween could not offer: at 16 ms a tick
    /// against a 69 ms frame it asked for eleven frames and took three quarters
    /// of a second to move one notch.
    ///
    /// The interval between calls is also how the machine's cost is learned,
    /// which is why this is the only place that has to know.
    pub fn tick(&mut self, now: Instant) -> bool {
        if let Some(last) = self.last_tick {
            self.observe_cost(now.saturating_duration_since(last));
        }
        self.last_tick = Some(now);

        let Some(glide) = self.glide else {
            // No glide: either nothing was moving, or the machine was measured
            // too slow to have one and `begin` already arrived.
            self.offset = self.target;
            return false;
        };

        let elapsed = now.saturating_duration_since(glide.began).as_secs_f32();
        let deadline = glide.duration.as_secs_f32();
        let progress = if deadline > 0.0 { (elapsed / deadline).clamp(0.0, 1.0) } else { 1.0 };
        self.offset = glide.from + (self.target - glide.from) * ease_out(progress);

        if progress >= 1.0 {
            // Land exactly on the target rather than asymptotically near it,
            // which is what lets the timer stop instead of running forever.
            self.offset = self.target;
            self.glide = None;
            return false;
        }
        true
    }

    /// Fold one observed frame interval into the machine's classification.
    ///
    /// The *fastest* recent frame wins, because that is the honest measure of
    /// what the machine can draw: a one-off hitch (a page fault, a background
    /// scan, another window painting) must not demote it for the rest of the
    /// session. Recovery upward is capped at a quarter per frame, so a machine
    /// that has genuinely become slower is still reclassified within a few
    /// frames of the gesture that revealed it.
    fn observe_cost(&mut self, interval: Duration) {
        self.frame_cost = Some(match self.frame_cost {
            None => interval,
            Some(previous) => {
                let micros = previous.as_micros() as u64;
                interval.min(Duration::from_micros(micros + micros / 4))
            }
        });
    }

    /// Adopt an offset that came from somewhere else — the scrollbar, a
    /// keyboard scroll, or `scroll_to` — without fighting it.
    ///
    /// Both the offset and the target move, so the next frame does not drag the
    /// content back to where the wheel last asked for it. The glide is dropped
    /// for the same reason, and the frame clock with it: the gap between a
    /// drag and the next wheel is not a frame interval and must not be measured
    /// as one.
    pub fn resync(&mut self, offset: f32) {
        self.offset = offset;
        self.target = offset;
        self.glide = None;
        self.last_tick = None;
    }

    /// Record the scrollable's geometry.
    ///
    /// The target is re-clamped, because the content can shrink under it (a
    /// shorter instance list, a search that returns less) and an offset past the
    /// end would leave the page scrolled into empty space.
    pub fn observe(&mut self, content_height: f32, view_height: f32) {
        self.content_height = content_height;
        self.view_height = view_height;
        self.target = self.clamp(self.target);
        self.offset = self.clamp(self.offset);
    }

    /// Reset for a page the user has just opened.
    ///
    /// Position and geometry belong to the new page; the measured frame cost
    /// does not, because it belongs to the machine. Re-learning it per page
    /// would mean paying for one glide the machine cannot draw on every
    /// navigation, which is exactly the cost this policy exists to remove.
    pub fn restart(&mut self) {
        let frame_cost = self.frame_cost;
        *self = ScrollAnim { frame_cost, ..ScrollAnim::default() };
    }
}

/// Cubic ease-out: quick to leave, gentle to arrive.
///
/// A scroll is a *position* animation the hand asked for, not a decoration, so
/// it has to look like it is obeying — closing most of the distance early and
/// settling — rather than starting slowly the way an ease-in-out transition
/// would. `progress` is in `0.0..=1.0`.
fn ease_out(progress: f32) -> f32 {
    let remaining = 1.0 - progress;
    1.0 - remaining * remaining * remaining
}

/// One wheel event, with the geometry it happened in.
///
/// The guard reports both because they come from the same place: it is laid out
/// at the content's full height inside the scrollable, and the rectangle it is
/// asked to draw in is the visible slice of that content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wheel {
    /// Notches, positive upward.
    pub notches: f32,
    /// Full height of the content, in pixels.
    pub content_height: f32,
    /// Height of the part that is on screen, in pixels.
    pub view_height: f32,
}

/// Pixels for a wheel delta, matching the units [`ScrollAnim::scroll_notches`]
/// takes *off* the offset.
///
/// Windows sends whole notches; touchpads and precision mice send pixel deltas,
/// which are already the right unit. Lines are counted here and multiplied, so
/// a notch on one machine is the same distance as a notch on another.
pub fn wheel_notches(delta: &mouse::ScrollDelta) -> f32 {
    match delta {
        mouse::ScrollDelta::Lines { y, .. } => *y,
        mouse::ScrollDelta::Pixels { y, .. } => *y / WHEEL_PIXELS_PER_NOTCH,
    }
}

/// Wraps a page's content so the shell, not the scrollable, handles the wheel.
///
/// Transparent to layout and to painting: it delegates every one of them to its
/// content. Its only job is to report a wheel event it has seen and to claim
/// it, which is what stops `Scrollable` from applying the jump the animation is
/// supposed to replace.
pub fn guard<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    on_wheel: impl Fn(Wheel) -> Message + 'a,
) -> WheelGuard<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a,
{
    WheelGuard {
        content: content.into(),
        on_wheel: Box::new(on_wheel),
    }
}

/// See [`guard`].
pub struct WheelGuard<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a,
{
    content: Element<'a, Message, Theme, Renderer>,
    on_wheel: Box<dyn Fn(Wheel) -> Message + 'a>,
}

impl<'a, Message, Theme, Renderer> WheelGuard<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a,
{
    /// What this guard would report for an event, if it claims it.
    ///
    /// Split out so the claim rule is testable without a window: a wheel over
    /// the content is taken, everything else is left to the widget below.
    fn claimed(
        event: &Event,
        cursor: mouse::Cursor,
        bounds: Rectangle,
        viewport: &Rectangle,
    ) -> Option<Wheel> {
        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                // The part of the content that is on screen: iced hands the
                // clipped rectangle, so the intersection is the view height
                // even at the very top or bottom of the page.
                let view_height = bounds
                    .intersection(viewport)
                    .map_or(viewport.height, |visible| visible.height);
                Some(Wheel {
                    notches: wheel_notches(delta),
                    content_height: bounds.height,
                    view_height,
                })
            }
            _ => None,
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for WheelGuard<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget().layout(tree, renderer, limits)
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation<Message>,
    ) {
        self.content.as_widget().operate(tree, layout, renderer, operation);
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        // The content sees everything first: a click on a button inside the page
        // must not be swallowed by the scroll policy.
        let status = self.content.as_widget_mut().on_event(
            tree,
            event.clone(),
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if status == event::Status::Captured {
            return status;
        }

        if let Some(wheel) = Self::claimed(&event, cursor, layout.bounds(), viewport) {
            shell.publish((self.on_wheel)(wheel));
            return event::Status::Captured;
        }

        event::Status::Ignored
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<WheelGuard<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(guard: WheelGuard<'a, Message, Theme, Renderer>) -> Self {
        Element::new(guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Theme;

    fn page(content: f32, view: f32) -> ScrollAnim {
        let mut anim = ScrollAnim::default();
        anim.observe(content, view);
        anim
    }

    /// Run a gesture to completion on a clock the test owns, and report how many
    /// frames it took.
    fn frames_to_settle(anim: &mut ScrollAnim) -> usize {
        let began = Instant::now();
        anim.begin(began);
        let (mut frames, mut now) = (0, began);
        while anim.tick(now) {
            frames += 1;
            now += FRAME;
            assert!(frames < 60, "the animation must terminate");
        }
        frames
    }

    #[test]
    fn log_work_is_bounded() {
        assert_eq!(visible_log_lines(0), 0);
        assert_eq!(visible_log_lines(499), 499);
        assert_eq!(visible_log_lines(500), 500);
        assert_eq!(visible_log_lines(20_000), LOG_RENDER_CAP);
    }

    #[test]
    fn a_page_shorter_than_the_window_does_not_scroll() {
        let anim = page(300.0, 500.0);
        assert_eq!(anim.max_offset(), 0.0);
        assert_eq!(anim.clamp(120.0), 0.0);
        assert_eq!(anim.clamp(-40.0), 0.0);
    }

    #[test]
    fn one_notch_moves_one_notch() {
        // The whole point of taking the wheel over is that the distance stays
        // what iced would have moved it; only the path changes.
        let mut anim = page(2000.0, 500.0);
        let now = Instant::now();
        anim.wheel(Wheel { notches: -1.0, content_height: 2000.0, view_height: 500.0 }, now);
        assert_eq!(anim.target, WHEEL_PIXELS_PER_NOTCH);
        assert_eq!(anim.offset, 0.0, "the offset must ease, not jump");
        anim.wheel(Wheel { notches: -1.0, content_height: 2000.0, view_height: 500.0 }, now);
        assert_eq!(anim.target, 2.0 * WHEEL_PIXELS_PER_NOTCH);
    }

    #[test]
    fn the_first_wheel_event_measures_the_page_for_itself() {
        // A page nobody has scrolled has never reported a viewport, so the
        // geometry has to travel with the wheel event or nothing would move.
        let mut anim = ScrollAnim::default();
        assert_eq!(anim.max_offset(), 0.0);
        anim.wheel(
            Wheel { notches: -2.0, content_height: 3000.0, view_height: 700.0 },
            Instant::now(),
        );
        assert_eq!(anim.content_height, 3000.0);
        assert_eq!(anim.view_height, 700.0);
        assert_eq!(anim.target, 2.0 * WHEEL_PIXELS_PER_NOTCH);
    }

    #[test]
    fn the_target_cannot_leave_the_content() {
        let mut anim = page(1000.0, 400.0);
        assert_eq!(anim.max_offset(), 600.0);
        anim.scroll_notches(-50.0);
        assert_eq!(anim.target, 600.0, "scrolling past the end would show blank space");
        anim.scroll_notches(50.0);
        assert_eq!(anim.target, 0.0, "scrolling above the start is not allowed either");
    }

    #[test]
    fn the_offset_reaches_the_target_and_then_stops() {
        let mut anim = page(4000.0, 600.0);
        anim.scroll_notches(-5.0);
        let frames = frames_to_settle(&mut anim);
        assert_eq!(anim.offset, anim.target, "it must land exactly on the target");
        assert!(!anim.animating());
        // About ten frames at 60fps: a fifth of a second, which reads as one
        // movement. The window is wide on purpose -- the property that matters
        // is a bounded gesture, not an exact count -- but a curve that doubled
        // the count would fail here.
        assert!((5..=20).contains(&frames), "settled in {frames} frames");
        // And a settled animation costs nothing: no further frames are asked for.
        assert!(!anim.tick(Instant::now()));
    }

    #[test]
    fn a_long_flick_takes_the_same_time_as_a_short_one() {
        // Easing against a deadline means distance changes how far each frame
        // travels, never how long the gesture lasts. The frame-counted curve
        // this replaced took ten times the frames for a ten times longer flick,
        // which is how one wheel notch became three quarters of a second on a
        // slow renderer.
        let frames = |notches: f32| {
            let mut anim = page(8000.0, 600.0);
            anim.scroll_notches(notches);
            frames_to_settle(&mut anim)
        };
        assert_eq!(
            frames(-2.0),
            frames(-20.0),
            "a 10x flick must not cost ten times the frames"
        );
        assert!(frames(-20.0) <= 20, "a gesture is a fifth of a second at 60fps");
    }

    #[test]
    fn the_frame_budget_decides_whether_there_is_an_animation() {
        // Nothing measured yet: the optimistic answer, so the first gesture of a
        // session glides -- and is what measures the machine.
        assert_eq!(ScrollAnim::glide_duration(None), DURATION);
        // A machine holding 60fps, or just inside the threshold: the full glide.
        assert_eq!(ScrollAnim::glide_duration(Some(FRAME)), DURATION);
        assert_eq!(
            ScrollAnim::glide_duration(Some(SMOOTH_FRAME - Duration::from_millis(1))),
            DURATION
        );
        // At the threshold and beyond it: no animation at all.
        assert_eq!(ScrollAnim::glide_duration(Some(SMOOTH_FRAME)), Duration::ZERO);
        assert_eq!(
            ScrollAnim::glide_duration(Some(Duration::from_millis(69))),
            Duration::ZERO,
            "the interval this shell was profiled at must not animate"
        );
        assert_eq!(ScrollAnim::glide_duration(Some(Duration::from_millis(500))), Duration::ZERO);
    }

    #[test]
    fn a_machine_too_slow_to_animate_moves_instead_of_gliding() {
        let mut anim = page(4000.0, 600.0);
        // One measured frame at 69ms: about 14 frames a second, which is this
        // machine's software rasteriser.
        anim.observe_cost(Duration::from_millis(69));
        anim.wheel(
            Wheel { notches: -1.0, content_height: 4000.0, view_height: 600.0 },
            Instant::now(),
        );

        // The target moved and so did the offset. There is no animation left to
        // interrupt, so the frame timer is never started and the gesture costs
        // exactly the frame the wheel event already paid for.
        assert_eq!(anim.target, WHEEL_PIXELS_PER_NOTCH);
        assert_eq!(anim.offset, anim.target);
        assert!(!anim.animating(), "nothing to animate means no timer at all");
    }

    #[test]
    fn the_frame_cost_remembers_the_best_frame_not_the_worst() {
        let mut anim = page(4000.0, 600.0);
        anim.observe_cost(Duration::from_millis(16));
        anim.observe_cost(Duration::from_millis(200));
        assert_eq!(
            anim.frame_cost,
            Some(Duration::from_millis(16)),
            "one hitch must not permanently declare a fast machine slow"
        );
        // Recovery upward is capped at a quarter per frame, so a machine that
        // really has become slower is reclassified within a few frames rather
        // than never.
        anim.observe_cost(Duration::from_millis(500));
        assert_eq!(anim.frame_cost, Some(Duration::from_millis(20)));
    }

    #[test]
    fn a_page_opened_later_remembers_what_the_machine_costs() {
        let mut anim = page(4000.0, 600.0);
        anim.observe_cost(Duration::from_millis(69));
        anim.scroll_notches(-4.0);
        anim.restart();
        // The newly opened page starts at its top...
        assert_eq!(anim.offset, 0.0);
        assert_eq!(anim.target, 0.0);
        assert!(!anim.animating());
        // ...but the machine is still the machine, so it does not pay for
        // re-learning that with one more glide it cannot draw.
        assert_eq!(anim.frame_cost, Some(Duration::from_millis(69)));
    }

    #[test]
    fn an_idle_page_asks_for_no_frames() {
        let anim = page(4000.0, 600.0);
        assert!(!anim.animating(), "nothing moves until a wheel event says so");
    }

    #[test]
    fn a_scrollbar_drag_is_adopted_rather_than_fought() {
        // iced reports the new offset through `on_scroll`; if the target were
        // left behind, the next frame would drag the content back.
        let mut anim = page(4000.0, 600.0);
        anim.scroll_notches(-3.0);
        anim.begin(Instant::now());
        assert!(anim.animating());
        anim.resync(1200.0);
        assert_eq!(anim.offset, 1200.0);
        assert_eq!(anim.target, 1200.0);
        assert!(!anim.animating(), "a drag ends the glide rather than fighting it");
    }

    #[test]
    fn content_that_shrinks_pulls_the_page_back_into_range() {
        let mut anim = page(4000.0, 600.0);
        anim.scroll_notches(-60.0);
        assert_eq!(anim.target, 3400.0);
        // The list got shorter while the page was scrolled to the bottom.
        anim.observe(900.0, 600.0);
        assert_eq!(anim.target, 300.0, "the target must be re-clamped, not left in space");
    }

    #[test]
    fn a_wheel_delta_is_measured_in_notches() {
        // Windows sends notches ("lines"); touchpads send pixels. Both have to
        // arrive as the same unit or a touchpad would fly and a mouse would
        // crawl.
        let line = iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 };
        let pixel = iced::mouse::ScrollDelta::Pixels { x: 0.0, y: -WHEEL_PIXELS_PER_NOTCH };
        assert_eq!(wheel_notches(&line), -1.0);
        assert_eq!(wheel_notches(&pixel), -1.0);
        let fractional = iced::mouse::ScrollDelta::Lines { x: 0.0, y: 0.5 };
        assert_eq!(wheel_notches(&fractional), 0.5);
    }

    #[test]
    fn only_a_wheel_over_the_content_is_claimed() {
        let bounds = Rectangle { x: 0.0, y: 0.0, width: 100.0, height: 1200.0 };
        // The visible slice: the scrollable's own height, at this offset.
        let viewport = Rectangle { x: 0.0, y: 0.0, width: 100.0, height: 400.0 };
        let inside = mouse::Cursor::Available(iced::Point::new(50.0, 50.0));
        let outside = mouse::Cursor::Available(iced::Point::new(150.0, 50.0));
        let wheel = Event::Mouse(mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
        });

        let claimed = WheelGuard::<(), Theme, iced::Renderer>::claimed(
            &wheel, inside, bounds, &viewport,
        );
        assert_eq!(
            claimed,
            Some(Wheel { notches: -1.0, content_height: 1200.0, view_height: 400.0 }),
            "the content height and the visible height must both travel with the event"
        );
        assert_eq!(
            WheelGuard::<(), Theme, iced::Renderer>::claimed(&wheel, outside, bounds, &viewport),
            None,
            "a wheel outside the page belongs to whatever is under it"
        );
        // Non-wheel events are never claimed: clicks and keys stay with the
        // content, and the scrollable's own scrollbar keeps working.
        let click = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        assert_eq!(
            WheelGuard::<(), Theme, iced::Renderer>::claimed(&click, inside, bounds, &viewport),
            None
        );
    }

    #[test]
    fn the_view_height_is_what_is_on_screen_not_all_of_it() {
        // At the bottom of the page only part of the last screen is left, and a
        // guard that reported the whole content as visible would let the page
        // scroll its own end past the top of the window.
        let bounds = Rectangle { x: 0.0, y: -800.0, width: 100.0, height: 1200.0 };
        let viewport = Rectangle { x: 0.0, y: -800.0, width: 100.0, height: 400.0 };
        let inside = mouse::Cursor::Available(iced::Point::new(50.0, 10.0));
        let wheel = Event::Mouse(mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
        });
        let claimed = WheelGuard::<(), Theme, iced::Renderer>::claimed(
            &wheel, inside, bounds, &viewport,
        )
        .expect("a wheel over the visible part is claimed");
        assert_eq!(claimed.view_height, 400.0);
    }
}
