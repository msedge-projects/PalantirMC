//! A control that reports the pointer crossing its bounds, as a message.
//!
//! The reference's controls are `transition-[filter,transform] duration-150
//! ease-out` (`ButtonFrame.vue`'s base classes), which means a hover is a
//! *tween*: the brightness arrives over 150 ms rather than on the next frame.
//! [`crate::anim::Interactions`] already holds that tween, and a stylesheet can
//! read it — but *starting* one is a sharper constraint than it looks, and this
//! widget is the whole reason it is a module.
//!
//! iced re-tracks a program's subscriptions in exactly one place: right after a
//! batch of messages is handled, and — this is the part that matters — *before*
//! the view runs (`iced_winit-0.12`'s `application::update`, which ends with
//! `runtime.track(application.subscription())`). The frame subscription that
//! advances a tween is gated on `Interactions::animating`, so a tween that
//! starts while the view is being built — which is when a stylesheet first sees
//! `Status::Hovered`, the only moment iced volunteers the pointer's position —
//! is one beat too late for the subscription that would have carried it. The
//! hover would paint its first frame and then sit there: no frames are
//! requested, no message arrives, and the subscription is never re-evaluated.
//! A tween started in the view is a tween that never moves.
//!
//! So the pointer's arrival has to *be* a message, published before the view
//! runs. `MouseArea` publishes enter/leave too, but it cannot wrap a button:
//! it never hands events to its content, so the button inside would be inert —
//! and the press would have to move into the `MouseArea`, taking iced's
//! release-inside semantics with it. This wrapper is the other half of that
//! trade: the content sees every event first and its status is returned
//! untouched, so a `button` inside keeps its own press, release and click
//! behaviour, and what is added is the one thing a widget cannot otherwise
//! say — the moment the pointer crosses its bounds.
//!
//! Deliberate limits:
//!
//! * **Twice per visit, not once per move.** A `CursorMoved` is compared
//!   against the side the pointer was last on and publishes only at the
//!   boundary, so the message traffic is two messages per hover rather than
//!   one per pixel of travel. The press is the same shape: two more messages
//!   per *click*, on the button going down and coming up, not one per event a
//!   held button generates.
//! * **Every event, not just moves.** The crossing is checked on every event
//!   the widget is handed, which costs a rectangle test and covers the cases a
//!   move-only report misses: the pointer leaving the window (`CursorLeft`
//!   takes the cursor away entirely, and a control that was lit must go out),
//!   and a control that appears or moves under a stationary pointer.
//! * **Never captures.** The content's status is returned as-is, so a wrapper
//!   around a button changes nothing about it.

use iced::advanced::widget::{tree, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::{event, Element, Event, Length, Rectangle, Size, Vector};

/// A two-state thing that changed, as the state it changed to.
///
/// Both reports this widget makes are boundary reports — the pointer's side of
/// a rectangle, and whether the left button is held — and both exist so that a
/// message is published per *change* rather than per event.
fn changed(was: bool, is: bool) -> Option<bool> {
    if was == is {
        None
    } else {
        Some(is)
    }
}

/// Which side of a control's bounds the pointer moved to, if it moved.
///
/// Split out so the rule is testable without a window: `Some(true)` on
/// arrival, `Some(false)` on departure, `None` while the pointer stays on the
/// side it was already on — which is what keeps a move from publishing
/// anything at all.
fn crossing(was_over: bool, is_over: bool) -> Option<bool> {
    changed(was_over, is_over)
}

/// Where the pointer is on a control: over it, and holding it down.
///
/// It has to live in the tree rather than in the widget: a view is rebuilt
/// from scratch on every frame, so a field on the struct would read `false`
/// again each time and every frame would be an arrival.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Pointer {
    /// The pointer is inside the control's bounds.
    over: bool,
    /// The left button went down on it and has not come back up there.
    down: bool,
}

/// Whether a mouse event changes what the control is drawn as, and to what.
///
/// A press is reported by the button *going down*, not by iced's `on_press`
/// (which is a release-inside, and a beat too late to be the look): the
/// reference's `active:` state is on for as long as the button is held.
///
/// Three deliberate refusals:
///
/// * **Left button only.** A right-click is a context menu elsewhere in the
///   shell, and giving it a dimming of its own would be inventing a look the
///   reference does not have.
/// * **A press outside must not report.** A control cannot be held by a button
///   that went down somewhere else.
/// * **A release only if it was held here.** Anything else is a release that
///   belongs to another widget — the pointer left this one, or the press was
///   never ours — and reporting it would light a control the hand is not on.
///   The hover's departure is what undoes a press that left, so this refusal
///   is also what keeps the two reports from disagreeing.
fn pressing(state: Pointer, event: &Event) -> Option<bool> {
    if !state.over {
        return None;
    }
    match event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => changed(state.down, true),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => changed(state.down, false),
        _ => None,
    }
}

/// A control that reports when the pointer crosses it.
pub struct Report<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a,
{
    content: Element<'a, Message, Theme, Renderer>,
    on_cross: Box<dyn Fn(bool) -> Message + 'a>,
    on_press: Box<dyn Fn(bool) -> Message + 'a>,
}

impl<'a, Message, Theme, Renderer> Report<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a,
{
    /// Wrap `content`, reporting `true` when the pointer arrives on it and
    /// `false` when it leaves, and the same pair for the left button going
    /// down on it and coming back up.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_cross: impl Fn(bool) -> Message + 'a,
        on_press: impl Fn(bool) -> Message + 'a,
    ) -> Self {
        Report {
            content: content.into(),
            on_cross: Box::new(on_cross),
            on_press: Box::new(on_press),
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Report<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Pointer>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Pointer::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
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
        self.content
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation<Message>,
    ) {
        self.content.as_widget().operate(
            &mut tree.children[0],
            layout,
            renderer,
            operation,
        );
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
        // Both readings are taken *before* the event is handed on, because the
        // content takes it by value — and they are readings of what the pointer
        // did, not of what the content did with it.
        let over = cursor.is_over(layout.bounds());
        let state = *tree.state.downcast_ref::<Pointer>();
        let crossed = crossing(state.over, over);
        let pressed = pressing(Pointer { over, ..state }, &event);

        // The content answers first and its answer is the one returned: a
        // click on a button inside must not be swallowed by the report, and
        // the report must not claim an event it did not use.
        let status = self.content.as_widget_mut().on_event(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state = tree.state.downcast_mut::<Pointer>();
        if let Some(over) = crossed {
            state.over = over;
            // Leaving with the button still down takes the press with it: a
            // release outside the control is a release, and the report is the
            // *hover's* to undo — one call moves the clock back to rest, which
            // is why no press message is sent from here.
            state.down = false;
            shell.publish((self.on_cross)(over));
        }
        if let Some(down) = pressed {
            state.down = down;
            shell.publish((self.on_press)(down));
        }

        status
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
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
            .overlay(&mut tree.children[0], layout, renderer, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<Report<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(report: Report<'a, Message, Theme, Renderer>) -> Self {
        Element::new(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crossing_is_reported_only_when_the_side_changes() {
        assert_eq!(crossing(false, true), Some(true), "an arrival is reported");
        assert_eq!(crossing(true, false), Some(false), "a departure is reported");
        assert_eq!(crossing(false, false), None, "moving inside must not report");
        assert_eq!(crossing(true, true), None, "moving outside must not report");
    }

    #[test]
    fn a_visit_reports_exactly_twice_however_many_events_arrive() {
        // The cost this widget exists to avoid: a pointer travelling across a
        // control arrives with dozens of events, and the tween only wants the
        // two that are boundaries.
        let mut was_over = false;
        let mut reports = Vec::new();
        // Five moves inside, then one that leaves, then four outside.
        for is_over in [true, true, true, true, true, false, false, false, false] {
            if let Some(over) = crossing(was_over, is_over) {
                reports.push(over);
                was_over = is_over;
            }
        }
        assert_eq!(reports, vec![true, false]);
    }

    #[test]
    fn leaving_the_window_is_a_departure() {
        // A cursor that has left the window is no longer over anything, and a
        // control it was last seen on has to go out — the one case a report
        // keyed to `CursorMoved` alone would never hear about.
        assert_eq!(crossing(true, false), Some(false));
    }

    fn press() -> Event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    }

    fn release() -> Event {
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
    }

    #[test]
    fn a_press_is_reported_on_the_way_down_and_the_way_up() {
        let over = Pointer { over: true, down: false };
        assert_eq!(pressing(over, &press()), Some(true), "down on the control");
        let held = Pointer { over: true, down: true };
        assert_eq!(pressing(held, &release()), Some(false), "and up on it");
        // The events in between are the moves a held button generates, and
        // none of them is a change.
        assert_eq!(pressing(held, &press()), None, "a repeat is not a press");
        assert_eq!(pressing(over, &release()), None, "a release that was never ours");
        assert_eq!(
            pressing(Pointer { over: false, down: false }, &press()),
            None,
            "a press outside the control is not its press"
        );
    }

    #[test]
    fn only_the_left_button_is_a_press() {
        // A right-click belongs to the shell's context menus, and a look of
        // its own here would be invented.
        for button in [
            mouse::Button::Right,
            mouse::Button::Middle,
            mouse::Button::Other(4),
        ] {
            let over = Pointer { over: true, down: false };
            assert_eq!(pressing(over, &Event::Mouse(mouse::Event::ButtonPressed(button))), None);
        }
    }

    #[test]
    fn a_pointer_that_leaves_while_held_lets_go() {
        // The press is undone by the *hover* leaving — one call to the clock
        // puts the control back at rest — so the wrapper drops the hold rather
        // than publishing a release that would light a control the hand is not
        // on. After the leave, the release that follows is not ours.
        let mut state = Pointer { over: true, down: true };
        if let Some(over) = crossing(state.over, false) {
            state.over = over;
            state.down = false;
        }
        assert_eq!(state, Pointer { over: false, down: false });
        assert_eq!(pressing(state, &release()), None);
    }
}
