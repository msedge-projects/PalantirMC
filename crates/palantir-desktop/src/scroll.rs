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

#[cfg(test)]
use std::time::{Duration, Instant};

use iced::advanced::widget::{tree, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::{event, Element, Event, Length, Rectangle, Size, Vector};

/// Maximum number of rows rendered by a virtualized-ish log view.
///
/// The log buffer may retain many more lines for troubleshooting, but drawing
/// all of them makes wheel input expensive. The view renders only the newest
/// window, which bounds layout/draw work per frame.
#[cfg(test)]
pub const LOG_RENDER_CAP: usize = 500;

/// Convert a buffered-line count into the number of rows the log page should
/// build. This is deliberately O(1) and allocation-free.
#[cfg(test)]
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
#[cfg(test)]
pub const FRAME: Duration = Duration::from_millis(16);

/// How long a glide takes, wall clock, on a machine that can draw one.
///
/// About ten frames at 60 fps, three at 20 fps, and the same fifth of a second
/// either way — which is the whole reason this is a duration. A frame count
/// makes the *length* of the gesture depend on how fast the machine happens to
/// be, and on a slow renderer that turns a scroll into a slideshow.
#[cfg(test)]
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
#[cfg(test)]
pub const SMOOTH_FRAME: Duration = Duration::from_millis(24);

/// Below this many pixels from the target, the animation is over.
///
/// Without a floor, a re-clamped target would leave the page a fraction of a
/// pixel from where it belongs and keep asking for frames.
#[cfg(test)]
pub const SETTLED: f32 = 0.5;

/// How many consecutive slow frames it takes to stop animating on a machine.
///
/// The two directions of this measurement are not equally costly, so they are
/// not treated equally. Believing a fast machine is slow turns the glide off
/// for the rest of the session; believing a slow one is fast costs a few wasted
/// frames, and the next gesture corrects it. So one fast frame is enough to
/// resume animating, while a single hitch -- a page fault, another window
/// painting, a background scan -- is not enough to stop it.
#[cfg(test)]
const SLOW_FRAMES_TO_DEMOTE: u8 = 2;

/// Where a page's scroll position is, and where it is going.
///
/// Plain numbers and a clock rather than a reference to any widget state, so
/// the whole easing policy is unit tested without a renderer or a window.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg(test)]
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
    /// The frame interval this machine was last measured at: a fast one the
    /// moment it is seen, a slow one only after [`SLOW_FRAMES_TO_DEMOTE`] of
    /// them in a row. `None` until a glide has run.
    frame_cost: Option<Duration>,
    /// How many consecutive frames have now been seen at or above
    /// [`SMOOTH_FRAME`].
    slow_frames: u8,
}

/// One glide: where it started, when, and the deadline it must meet.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg(test)]
struct Glide {
    from: f32,
    began: Instant,
    duration: Duration,
}

#[cfg(test)]
impl ScrollAnim {
    /// The largest offset that still shows content: never negative, so a page
    /// shorter than the window cannot scroll at all.
    #[cfg(test)]
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
    /// Asymmetric on purpose, and the asymmetry is the whole design: see
    /// [`SLOW_FRAMES_TO_DEMOTE`]. A frame this machine can animate in is its
    /// answer immediately, and two slow frames in a row are needed to take the
    /// animation away -- so a hitch neither stops the glide nor, once the
    /// machine really has become slow, leaves it running for long.
    fn observe_cost(&mut self, interval: Duration) {
        if interval < SMOOTH_FRAME {
            self.frame_cost = Some(interval);
            self.slow_frames = 0;
            return;
        }
        self.slow_frames = self.slow_frames.saturating_add(1);
        if self.slow_frames >= SLOW_FRAMES_TO_DEMOTE {
            self.frame_cost = Some(interval);
        }
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
        self.offset = 0.0;
        self.target = 0.0;
        self.content_height = 0.0;
        self.view_height = 0.0;
        self.glide = None;
        self.last_tick = None;
        self.slow_frames = 0;
        // `frame_cost` is deliberately left alone: it describes the machine, so
        // re-learning it once per page would mean paying for one glide the
        // machine cannot draw on every navigation.
    }
}

// ---- How many rows a frame draws -----------------------------------------

/// How many rows beyond the visible ones a frame draws, on each side.
///
/// The reference's `bufferSize` in `ui/src/composables/virtual-scroll.ts`, and
/// the same reasoning: a wheel notch lands as a report of where the region *is*,
/// and the frame that draws it is built from that report, so without a margin
/// the edge of the window would be a line the reader can see arriving. Five
/// rows is more than one notch's travel at any row height this interface draws.
pub const OVERSCAN: usize = 5;

/// The fewest rows a frame draws before the scroll region has reported where it
/// is.
///
/// The reference's `initialItemCount`. The floor exists for the row that is
/// tall: an assumption about the height of a window (below) is a row count only
/// once a row height is known, and a list whose rows are taller than that budget
/// still owes the reader a screenful rather than a row and a half.
pub const INITIAL_ROWS: usize = 20;

/// The height of the window assumed before the scroll region has reported its
/// own, in pixels.
///
/// **This is where the port has to differ from the reference, and the reason is
/// that the reference can measure and this cannot.** `useScrollViewport` attaches
/// to the scrolling ancestor on mount and reads `clientHeight` then, which is why
/// twenty items are enough for it: they are one frame's worth, before the ref
/// exists. iced publishes a scrollable's viewport only from an *event* -- wheel,
/// touch, a scrollbar drag -- so a tab the reader has opened and not yet scrolled
/// would draw twenty rows and then a band of nothing on any window taller than
/// that. A height rather than a count is what makes the fallback work for rows of
/// every size this page draws: 4,000px of them is a taller window than this shell
/// can be drawn at, so what it draws always covers the visible slice, and it is
/// still a constant -- a list of five thousand costs what a list of ninety costs,
/// until the first event says exactly what to draw.
pub const INITIAL_VIEW: f32 = 4_000.0;

/// Where a scroll region is, in pixels, as `Scrollable::on_scroll` reports it.
///
/// Plain numbers rather than the widget's own viewport, so the rule below is a
/// function of its inputs and is unit tested without a window -- the same reason
/// [`ScrollAnim`] carries numbers instead of a reference to widget state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Geometry {
    /// Pixels the content has been scrolled by, from its top.
    pub offset: f32,
    /// Height of the part of it that is on screen.
    pub view_height: f32,
}

impl Geometry {
    /// The geometry a scrollable reported.
    ///
    /// `absolute_offset` rather than the relative one: the rows are placed from
    /// the content's own top (see [`crate::pages::instance`]), and a percentage
    /// would have to be turned back into pixels against the same content height
    /// the caller already has.
    pub fn of(viewport: iced::widget::scrollable::Viewport) -> Geometry {
        Geometry {
            offset: viewport.absolute_offset().y,
            view_height: viewport.bounds().height,
        }
    }
}

/// The rows one frame draws, out of a list `len` long whose rows are all
/// `row_height` tall.
///
/// A port of the reference's `visibleRange`, including both of its guards, and
/// the reason this is a *policy* rather than a loop: the rows outside the range
/// are the ones that cost nothing, so what a frame costs stops depending on how
/// long the list is. `crate::scale` is where that is measured at five thousand.
///
/// The two guards are worth naming. A region that has not reported its height
/// yet gets [`INITIAL_VIEW`] rather than "all of them" or "none", and at least
/// [`INITIAL_ROWS`]. A range is never shorter than the window, so at the very end
/// of a list the range slides rather than shrinking -- which is what keeps the
/// count of drawn rows constant while a reader scrolls, and therefore the frame
/// cost constant too.
pub fn window(len: usize, row_height: f32, at: Geometry) -> std::ops::Range<usize> {
    if len == 0 || row_height <= 0.0 {
        return 0..len;
    }
    let view = if at.view_height > 0.0 { at.view_height } else { INITIAL_VIEW };
    let visible = (view / row_height).ceil() as usize;
    let size = (visible + OVERSCAN * 2).max(INITIAL_ROWS);
    // `saturating_sub` for the list shorter than one window: there is no offset
    // that would show `size` rows of it, and `0` is the only range that makes
    // sense.
    let start = ((at.offset / row_height).floor().max(0.0) as usize)
        .saturating_sub(OVERSCAN)
        .min(len.saturating_sub(size));
    start..(start + size).min(len)
}

/// Cubic ease-out: quick to leave, gentle to arrive.
///
/// A scroll is a *position* animation the hand asked for, not a decoration, so
/// it has to look like it is obeying — closing most of the distance early and
/// settling — rather than starting slowly the way an ease-in-out transition
/// would. `progress` is in `0.0..=1.0`.
#[cfg(test)]
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

    /// A scroll region holding `len` rows of `row_height`, scrolled to `offset`
    /// in a `view_height` window.
    fn at(len: usize, row_height: f32, offset: f32, view_height: f32) -> std::ops::Range<usize> {
        window(len, row_height, Geometry { offset, view_height })
    }

    #[test]
    fn a_window_holds_the_visible_rows_and_a_margin_either_side() {
        // 66px rows in a 600px region: nine and a bit rows are on screen, and
        // nine are drawn beside them -- five above and five below, which is what
        // keeps a partly-scrolled row from arriving as a blank line.
        assert_eq!(at(5_000, 66.0, 0.0, 600.0).len(), 10 + OVERSCAN * 2);
        // At the top the margin cannot go above row zero.
        assert_eq!(at(5_000, 66.0, 0.0, 600.0).start, 0);
        // A hundred rows in: the window has moved with the reader, and its size
        // has not changed.
        let middle = at(5_000, 66.0, 6_600.0, 600.0);
        assert_eq!(middle.start, 100 - OVERSCAN);
        assert_eq!(middle.len(), 10 + OVERSCAN * 2);
    }

    #[test]
    fn the_window_slides_at_the_end_of_the_list_rather_than_shrinking() {
        // The constant size is the point: a range that shrank at the bottom would
        // make the last screenful the cheapest frame and the one before it the
        // most expensive, for no reason a reader could see.
        let end = at(5_000, 66.0, 400_000.0, 600.0);
        assert_eq!(end.start, 5_000 - (10 + OVERSCAN * 2));
        assert_eq!(end.end, 5_000, "there is nothing past the end to draw");
        assert_eq!(end.len(), 10 + OVERSCAN * 2);
    }

    #[test]
    fn a_region_that_has_not_reported_itself_draws_a_window_taller_than_a_window() {
        // Nothing published yet. The whole list would be the cost this rule
        // exists to remove, and twenty rows would be a band of nothing on a tall
        // window at this row height -- so the fallback is a height, and it is
        // more than any window this shell can be drawn in.
        let first = at(5_000, 66.0, 0.0, 0.0);
        assert!(first.len() > INITIAL_ROWS, "{}", first.len());
        assert!(first.len() < 100, "and it is still a window, not the list");
        assert_eq!(first.start, 0, "nothing has been scrolled, so it starts at the top");
        // And a list shorter than that draws all of itself.
        assert_eq!(at(7, 66.0, 0.0, 0.0), 0..7);
        // A row taller than the assumed window still owes a screenful: the floor
        // is what makes the fallback work for rows of every size.
        assert_eq!(at(5_000, 500.0, 0.0, 0.0).len(), INITIAL_ROWS);
    }

    #[test]
    fn a_list_shorter_than_its_window_is_all_of_itself() {
        assert_eq!(at(3, 66.0, 0.0, 600.0), 0..3);
        // A row taller than the region is still one row: `ceil` rounds a part of
        // a row up, and a window of zero rows would draw nothing at all. (The
        // count is the floor's, not the row's -- the floor is above one row.)
        assert!(at(5_000, 900.0, 0.0, 600.0).contains(&0));
        // An empty listing, and a row height of zero, which is the shape a
        // divide by zero would take if this were written as a division.
        assert_eq!(at(0, 66.0, 0.0, 600.0), 0..0);
        assert_eq!(at(9, 0.0, 0.0, 600.0), 0..9);
        // The floor is a floor: a short list is still all of itself, however few
        // rows its region could hold.
        assert_eq!(at(3, 900.0, 0.0, 600.0), 0..3);
    }

    #[test]
    fn the_window_keeps_the_scrollbar_where_it_was_by_never_passing_the_end() {
        // The page draws a spacer above the first row and one below the last, so
        // the content's height is the whole list's whatever is drawn. What this
        // checks is the other half: the range is always inside the list, so the
        // rows that are drawn are always at the offset they claim to be at.
        for offset in [0.0, 1.0, 6_599.0, 6_600.0, 329_999.0, 400_000.0] {
            let range = at(5_000, 66.0, offset, 600.0);
            assert!(range.end <= 5_000 && range.start <= range.end, "{offset}: {range:?}");
            assert_eq!(range.len(), 10 + OVERSCAN * 2, "{offset}: {range:?}");
        }
        // A negative offset is the top: a region cannot report one, and a rule
        // that trusted it would panic on the cast rather than draw.
        assert_eq!(at(5_000, 66.0, -120.0, 600.0).start, 0);
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
        // Two measured frames at 69ms: about 14 frames a second, which is this
        // machine's software rasteriser. Two rather than one because a single
        // slow frame is not yet evidence -- see `SLOW_FRAMES_TO_DEMOTE` -- and
        // the point of this test is the machine that has been classified, not
        // the frame that classified it.
        anim.observe_cost(Duration::from_millis(69));
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
    fn one_hitch_does_not_declare_a_fast_machine_slow() {
        let mut anim = page(4000.0, 600.0);
        anim.observe_cost(Duration::from_millis(16));
        anim.observe_cost(Duration::from_millis(200));
        assert_eq!(
            anim.frame_cost,
            Some(Duration::from_millis(16)),
            "a single bad frame must not take the glide away"
        );
        // And the other direction, which is the same measurement read from the
        // other side: a machine that is slow every frame is classified as slow
        // within the gesture that revealed it.
        let mut slow = page(4000.0, 600.0);
        slow.observe_cost(Duration::from_millis(69));
        assert_eq!(
            slow.frame_cost, None,
            "the first slow frame is still only a suspicion"
        );
        slow.observe_cost(Duration::from_millis(69));
        assert_eq!(slow.frame_cost, Some(Duration::from_millis(69)));
        assert_eq!(ScrollAnim::glide_duration(slow.frame_cost), Duration::ZERO);
    }

    #[test]
    fn a_page_opened_later_remembers_what_the_machine_costs() {
        let mut anim = page(4000.0, 600.0);
        anim.observe_cost(Duration::from_millis(69));
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
