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
//! Deliberate limits, because each one is a way this can go wrong:
//!
//! * **Only the wheel is taken.** The scrollbar, dragging it, keyboard
//!   scrolling, touch and iced's own `scroll_to` (which the log page's
//!   autoscroll uses) all still work, because this does not replace
//!   `Scrollable` — it sits inside one.
//! * **No timer while nothing moves.** The animation is a subscription that
//!   exists only while [`ScrollAnim::animating`] is true, so an idle window
//!   still does no work at all.
//! * **Bounded, not physical.** An exponential approach with a fixed factor
//!   reaches the target in a fixed number of frames; there is no velocity to
//!   integrate and no way to end up settling forever.

use std::time::Duration;

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

/// How long one animation frame is. 60 frames a second: the display's own
/// rate on the machines this shell runs on, and the rate the tween's factor is
/// calibrated for.
pub const FRAME: Duration = Duration::from_millis(16);

/// Fraction of the remaining distance covered per frame.
///
/// 0.45 reaches the [`SETTLED`] floor in about eight frames for a single notch
/// and eleven or twelve for a long flick — 130 to 200 ms, which reads as one
/// smooth movement rather than as two.
///
/// The number is a cost decision as much as a feel decision. Every frame of a
/// tween is a published message, and in iced 0.12 a message is what makes the
/// shell rebuild: `iced_winit` runs the application's `update` and `view` only
/// when dispatch produced a message, and `Program::State::update` re-runs
/// `view` — twice, when there are messages. The frame then has to be drawn and
/// the window blitted. At 0.28 the same single notch took fifteen frames and a
/// flick up to twenty-two; this curve spends roughly half of that while the
/// distance covered per frame still falls monotonically, which is what the eye
/// reads as smoothness.
pub const EASE: f32 = 0.45;

/// Below this many pixels from the target, the animation is over.
///
/// Without a floor, an exponential approach never quite arrives and the timer
/// would run forever.
pub const SETTLED: f32 = 0.5;

/// Where a page's scroll position is, and where it is going.
///
/// Plain numbers rather than a reference to any widget state, so the whole
/// easing policy is unit tested without a renderer or a window.
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
    pub fn animating(&self) -> bool {
        (self.target - self.offset).abs() > SETTLED
    }

    /// Take a wheel event, with the geometry it was reported in.
    ///
    /// The geometry comes from the same event rather than from a separate
    /// report, because iced only publishes viewport changes — a page nobody has
    /// scrolled yet has never reported one, and a wheel on it would otherwise
    /// move nothing.
    pub fn wheel(&mut self, wheel: Wheel) {
        self.observe(wheel.content_height, wheel.view_height);
        self.scroll_notches(wheel.notches);
    }

    /// Move the target by `notches`, positive upward.
    ///
    /// The target is clamped to the content; the *offset* is not touched,
    /// because the whole point is that it catches up smoothly.
    pub fn scroll_notches(&mut self, notches: f32) {
        self.target = self.clamp(self.target - notches * WHEEL_PIXELS_PER_NOTCH);
    }

    /// Advance one frame and report whether there is more to animate.
    ///
    /// The step is proportional to what is left, so a flick of many notches at
    /// once still settles in the same number of frames instead of crawling, and
    /// the offset always lands exactly on the target rather than asymptotically
    /// near it — which is what lets the timer stop.
    pub fn tick(&mut self) -> bool {
        self.offset += (self.target - self.offset) * EASE;
        if (self.target - self.offset).abs() <= SETTLED {
            self.offset = self.target;
            return false;
        }
        true
    }

    /// Adopt an offset that came from somewhere else — the scrollbar, a
    /// keyboard scroll, or `scroll_to` — without fighting it.
    ///
    /// Both the offset and the target move, so the next frame does not drag the
    /// content back to where the wheel last asked for it.
    pub fn resync(&mut self, offset: f32) {
        self.offset = offset;
        self.target = offset;
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
        anim.wheel(Wheel { notches: -1.0, content_height: 2000.0, view_height: 500.0 });
        assert_eq!(anim.target, WHEEL_PIXELS_PER_NOTCH);
        assert_eq!(anim.offset, 0.0, "the offset must ease, not jump");
        anim.wheel(Wheel { notches: -1.0, content_height: 2000.0, view_height: 500.0 });
        assert_eq!(anim.target, 2.0 * WHEEL_PIXELS_PER_NOTCH);
    }

    #[test]
    fn the_first_wheel_event_measures_the_page_for_itself() {
        // A page nobody has scrolled has never reported a viewport, so the
        // geometry has to travel with the wheel event or nothing would move.
        let mut anim = ScrollAnim::default();
        assert_eq!(anim.max_offset(), 0.0);
        anim.wheel(Wheel { notches: -2.0, content_height: 3000.0, view_height: 700.0 });
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
        let mut frames = 0;
        while anim.tick() {
            frames += 1;
            assert!(frames < 120, "the animation must terminate");
        }
        assert_eq!(anim.offset, anim.target, "it must land exactly on the target");
        assert!(!anim.animating());
        // 5 notches is 300px: an exponential approach covers 99% of it in about
        // fifteen frames, so a quarter of a second at 60fps.
        // 5 notches is 300px: this curve covers it in about eleven frames, a
        // fifth of a second at 60fps. The window is wide on purpose — the
        // property that matters is "a bounded number of frames", not an exact
        // one — but a curve that doubled the count would fail here.
        assert!((5..=20).contains(&frames), "settled in {frames} frames");
        // And a settled animation costs nothing: no further frames are asked for.
        assert!(!anim.tick());
    }

    #[test]
    fn the_settling_time_grows_logarithmically_not_with_distance() {
        // A proportional step means distance costs frames only through the
        // 0.5px floor: ten times the flick needs a few more frames, not ten
        // times as many. Either way the whole gesture is under half a second,
        // which is the property that matters to the hand.
        let frames = |notches: f32| {
            let mut anim = page(8000.0, 600.0);
            anim.scroll_notches(notches);
            let mut count = 0;
            while anim.tick() {
                count += 1;
                assert!(count < 120, "the animation must terminate");
            }
            count
        };
        let (short, long) = (frames(-2.0), frames(-20.0));
        assert!(long <= short + 10, "a 10x flick took {long} frames against {short}");
        assert!(long <= 30, "a long flick should settle inside half a second, took {long}");
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
        anim.resync(1200.0);
        assert_eq!(anim.offset, 1200.0);
        assert_eq!(anim.target, 1200.0);
        assert!(!anim.animating());
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
