//! Drawing one element over another at a chosen offset.
//!
//! The reference reaches for `position: absolute` inside a `position: relative`
//! box in places where this port's widget kit has no equivalent, and both of the
//! places it matters are structural rather than decorative: an element drawn
//! *inside* another element's extent rather than after it. `pages::servers` and
//! `pages::user` share one implementation because a second copy of iced's
//! `Widget` trait is a second copy of its diffing, and a diffed tree that is
//! built two ways is a tree that desynchronises.

#![allow(dead_code)]

use iced::advanced::widget::{tree, Tree};
use iced::advanced::{layout, mouse, renderer, Clipboard, Layout, Shell, Widget};
use iced::{event, Element, Event, Length, Rectangle, Vector};

// ---- Layers over one another --------------------------------------------

/// Elements drawn at the same place, each at an offset, the later over the
/// earlier.
///
/// The one thing it stands in for is `position: absolute` inside a
/// `position: relative` box. The reference needs that in two places with nothing
/// else in common: `ServerListEmptyPreview.vue` puts its fade and its toast over
/// a panel, and `ProjectCard.vue`'s list layout puts a grid item in a named area
/// that *spans* rows -- the icon column runs a card's full height while the tags
/// sit on the card's last row, which is inside the icon's span rather than after
/// it. A column cannot say that, because the tags are not below the icon; they
/// are beside its last eight pixels.
///
/// iced 0.12 has no `stack` (it arrives in 0.13) and no positioned children, so
/// this is the whole of it. Four rules, three from CSS and one from iced:
///
/// * **The first layer sets the box.** Its layout node *is* the stack's, so a
///   layer placed at an offset that runs past it is the caller's to clip -- which
///   is what the toast does, and what the reference's own clipped viewport does
///   with it (`left-[32%]` of a 400 box is 128, and 128 + 336 = 464).
/// * **The last layer takes the pointer.** Events, hover and cursor walk the
///   layers from the last to the first and stop at the first that claims the
///   event, which is what CSS does with two `position: absolute` boxes over one
///   another and what `App.vue:2554-2564`'s `absolute bottom-[250px] ... z-10`
///   link over the sidebar's own column is asking for. [`mouse_interaction`]
///   walks the same layers in the same order, because a hit test that disagrees
///   with delivery is worse than either of them alone.
/// * **A layer that is a picture is not asked.** The second point is only true of
///   the layers that *are* things: a decorative layer drawn over live content
///   must not be able to swallow what is under it, or a fade would become a
///   click shield. So [`Stack::over`] declares its layer inert and only
///   [`Stack::over_control`] puts one back into the walk.
/// * **A layer is diffed, never rebuilt.** Nothing about that is CSS; it is what
///   `iced_widget::Row` does and what every widget in iced is written against,
///   and it is what keeps a control's *state* -- which lives in the tree and in
///   nowhere else -- from being rebuilt out from under it every frame. See
///   [`Stack::diff`].
///
/// The third rule is what the reference's own `inert` is. `ServerListEmptyPreview.vue`
/// is `inert aria-hidden` -- a picture of the invite dialog, not the dialog --
/// so the panel, its buttons, the friend rows, the invite link and the toast
/// beside it are all drawn and none of them are hit targets
/// (`crates::pages::servers::preview`). Every other stack here is a picture over
/// something live for the same reason, which is why the pane's own furniture --
/// the reserved scrollbar gutter, the toast, the inset shadow and the rule --
/// goes on with [`Stack::over`] and takes no input: `Shell::pane` has one live
/// layer and it is the page.
pub struct Stack<'a, Message, Theme, Renderer> {
    /// Painted in this order, the last one over the rest.
    layers: Vec<Layer<'a, Message, Theme, Renderer>>,
}

/// One of a [`Stack`]'s layers: where it sits, what it is, and whether it takes
/// input.
struct Layer<'a, Message, Theme, Renderer> {
    /// From the stack's own origin, in the window's coordinates.
    at: Vector,
    element: Element<'a, Message, Theme, Renderer>,
    /// Whether this layer is asked for events, cursor and focus at all.
    ///
    /// Set by the constructor rather than guessed from the element: only the
    /// call site knows whether a layer drawn over live content is a picture of a
    /// control or the control itself.
    input: bool,
}

impl<'a, Message, Theme, Renderer> Stack<'a, Message, Theme, Renderer> {
    /// The layer the stack's own box is taken from, and the one the stack is
    /// *for*: the content the overlays are decorations on top of.
    ///
    /// It takes input, because the reference's `position: relative` box takes
    /// input wherever nothing above it does.
    pub fn at(
        offset: Vector,
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Stack { layers: vec![Layer { at: offset, element: element.into(), input: true }] }
    }

    /// One more layer, over everything laid down so far, drawn and not asked.
    ///
    /// The default because every stack in this crate is mostly this: a picture of
    /// a presence dot over an avatar, a fade over a panel, the pane's rule over
    /// the page. A layer that cannot be reached cannot take a click meant for the
    /// content under it, which is the whole reason this is not [`Stack::over_control`]
    /// for all of them.
    pub fn over(
        mut self,
        offset: Vector,
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.layers.push(Layer { at: offset, element: element.into(), input: false });
        self
    }

    /// One more layer over everything laid down so far, which is *not* a
    /// picture: it takes the pointer before the layers under it.
    ///
    /// For the reference's own case of a control positioned over content: the
    /// `*Upgrade to Modrinth Plus*` link at `App.vue:2554-2564` is
    /// `absolute bottom-[250px] ... z-10` inside `.app-sidebar`, so it is above
    /// the column it is drawn over and a click on its words is the link's rather
    /// than the column's. See [`Shell::panel`](crate::shell::Shell::panel) for
    /// the two layers that are the other half of that block.
    pub fn over_control(
        mut self,
        offset: Vector,
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.layers.push(Layer { at: offset, element: element.into(), input: true });
        self
    }
}

/// The layers, laid out, in one indexed place for the three methods that walk
/// them.
///
/// A `Layout` is two words and `Copy`, and `layout.children()` yields the layers
/// in paint order, so collecting it is how a walk can run backwards: a
/// topmost-first hit order is the whole point, and an iterator that only went
/// forwards would make the order an argument to every call instead.
fn layer_layouts(layout: Layout<'_>) -> Vec<Layout<'_>> {
    layout.children().collect()
}

impl<'a, Message, Theme, Renderer> From<Stack<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(stack: Stack<'a, Message, Theme, Renderer>) -> Self {
        Element::new(stack)
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Stack<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn tag(&self) -> tree::Tag {
        self.layers.first().map_or(tree::Tag::stateless(), |base| base.element.as_widget().tag())
    }

    fn state(&self) -> tree::State {
        self.layers.first().map_or(tree::State::None, |base| base.element.as_widget().state())
    }

    fn children(&self) -> Vec<Tree> {
        self.layers
            .iter()
            .map(|layer| Tree::new(layer.element.as_widget()))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        // iced's own idiom, and the whole difference between a tree that is
        // diffed and one that is rebuilt: `diff_children` reconciles each layer
        // against the tree last frame's, so the **state** inside a layer survives
        // the frame. `Tree::new` per layer and a swap would throw that state away
        // every frame, and the state is not the widget's to hold: a `mouse_area`
        // remembers `is_hovered` there, a `text_input` its text, a `scrollable`
        // its offset. What a rebuild costs is measurable in the interface rather
        // than in a number -- a control that lights up under the pointer and
        // never goes out again, because the second half of a crossing needs the
        // first half's memory to still be there when the pointer leaves.
        let layers: Vec<&dyn Widget<Message, Theme, Renderer>> =
            self.layers.iter().map(|layer| layer.element.as_widget()).collect();
        tree.diff_children(&layers);
    }

    fn size(&self) -> iced::Size<Length> {
        self.layers.first().map_or(iced::Size::new(Length::Shrink, Length::Shrink), |base| {
            base.element.as_widget().size()
        })
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let mut nodes: Vec<layout::Node> = Vec::with_capacity(self.layers.len());
        for (index, layer) in self.layers.iter().enumerate() {
            // `diff` keeps `tree.children` as long as `layers`, so the two only
            // fall out of step if this runs before it -- and then there is nothing
            // to draw anyway.
            let Some(child) = tree.children.get_mut(index) else { break };
            nodes.push(layer.element.as_widget().layout(child, renderer, limits).translate(layer.at));
        }
        let size = nodes.first().map_or(iced::Size::ZERO, layout::Node::size);
        layout::Node::with_children(size, nodes)
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
        let mut children = layout.children();
        for (index, layer) in self.layers.iter().enumerate() {
            let (Some(child), Some(node)) = (children.next(), tree.children.get(index)) else {
                continue;
            };
            layer.element.as_widget().draw(node, renderer, theme, style, child, cursor, viewport);
        }
    }

    /// The stack's own hook for an operation that walks the tree: focus
    /// traversal, and the clipboard commands.
    ///
    /// See the body for why it cannot be left empty.
    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation<Message>,
    ) {
        let layouts = layer_layouts(layout);
        // The hook every container has to call, and the reason this body is not
        // optional: an operation that walks the tree -- `focus_next`, `copy`,
        // `paste`, `select_all` -- reaches a widget by way of
        // `Operation::container`, and a widget that never calls it *ends* the
        // walk. See `iced_widget-0.12.3/src/container.rs:194-212`, which is the
        // stock example, and `iced_core-0.12.3/src/widget/operation.rs:33-46`
        // for the hook itself. Without this, no text input inside a page could be
        // focused and no clipboard command could reach one.
        operation.container(None, layout.bounds(), &mut |operation| {
            // In paint order, which is reading order: `focus_next` counts the
            // focusable widgets it walks past, and the next field on a page is the
            // next one down it rather than the topmost one.
            for (index, layer) in self.layers.iter().enumerate() {
                if !layer.input {
                    continue;
                }
                let (Some(child), Some(child_layout)) =
                    (tree.children.get_mut(index), layouts.get(index).copied())
                else {
                    break;
                };
                layer.element.as_widget().operate(child, child_layout, renderer, operation);
            }
        });
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
        let layouts = layer_layouts(layout);
        // The last layer first, and the first layer that claims the event ends
        // it: the reference hands a click to the topmost box under the pointer
        // and lets a box under it have the event only if the one over it did not
        // take it, which is CSS's own rule and what the two live overlays in this
        // crate are (`App.vue:2554-2564`'s `z-10` link over the sidebar column).
        //
        // Not every layer is asked. A layer built with [`Stack::over`] is a
        // picture of a control and is skipped whole, so an overlay cannot be a
        // click shield -- the pane's reserved gutter is 10 pixels of the page's
        // own background and a full-height rule stands over every control on
        // every page, and both are drawn there.
        //
        // Every layer is asked unconditionally rather than only the one the
        // pointer is inside, because the widgets underneath bounds-check
        // themselves: `mouse_area` returns `Ignored` for a press outside its own
        // box (`iced_widget-0.12.3/src/mouse_area.rs:333`), and the stock
        // `container` does the same by answering with its content's status. A
        // keyboard event has no position to test against at all, and this is how
        // `iced_widget::Container` forwards one.
        for index in (0..self.layers.len()).rev() {
            if !self.layers[index].input {
                continue;
            }
            let (Some(child), Some(child_layout)) =
                (tree.children.get_mut(index), layouts.get(index).copied())
            else {
                break;
            };
            let status = self.layers[index].element.as_widget_mut().on_event(
                child,
                event.clone(),
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
            if status == event::Status::Captured {
                return status;
            }
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
        let layouts = layer_layouts(layout);
        // The same walk as `on_event` -- same layers, same order, same skipping
        // -- because the two answering differently is the failure that cannot be
        // seen: a control that shows the pointer and eats nothing looks like a
        // dead button, and one that eats the click without showing the pointer
        // looks like a bug in the picture.
        for index in (0..self.layers.len()).rev() {
            if !self.layers[index].input {
                continue;
            }
            let (Some(child), Some(child_layout)) =
                (tree.children.get(index), layouts.get(index).copied())
            else {
                break;
            };
            match self.layers[index].element.as_widget().mouse_interaction(
                child,
                child_layout,
                cursor,
                viewport,
                renderer,
            ) {
                // `Idle` is the default and means this layer has nothing to say
                // about the pointer here, which is not the same as refusing it:
                // the layer under this one is asked, as the reference's own hit
                // test falls through to the box beneath.
                mouse::Interaction::Idle => {}
                claimed => return claimed,
            }
        }
        mouse::Interaction::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::{container, mouse_area, Space};

    /// A renderer that draws nothing, which is all a test needs: nothing here
    /// calls it.
    ///
    /// [`iced::advanced::Renderer`] is four methods -- the ones
    /// `iced_core-0.12.3/src/renderer.rs:13-43` declares -- so a test can name a
    /// type that satisfies it without a GPU, a window, or this machine's wgpu.
    #[derive(Debug, Default)]
    struct Null;

    impl iced::advanced::Renderer for Null {
        fn with_layer(&mut self, _bounds: Rectangle, f: impl FnOnce(&mut Self)) {
            f(self)
        }

        fn with_transformation(
            &mut self,
            _transformation: iced::Transformation,
            f: impl FnOnce(&mut Self),
        ) {
            f(self)
        }

        fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<iced::Background>) {}

        fn clear(&mut self) {}
    }

    /// What these tests build: a stack whose layers answer a press with the name
    /// they were built with, so "did the event reach *this* layer" is a question
    /// about a message rather than about a pixel.
    type Test = Stack<'static, &'static str, iced::Theme, Null>;

    /// The box every layer here is made of when it is a control.
    fn pressable(label: &'static str) -> Element<'static, &'static str, iced::Theme, Null> {
        mouse_area(Space::new(Length::Fixed(200.0), Length::Fixed(100.0)))
            .interaction(mouse::Interaction::Pointer)
            .on_press(label)
            .into()
    }

    /// The same box when it is a control that also reports its crossings, so a
    /// test can tell an enter from an exit.
    fn crossed(label: &'static str) -> Element<'static, &'static str, iced::Theme, Null> {
        mouse_area(Space::new(Length::Fixed(200.0), Length::Fixed(100.0)))
            .on_enter("enter")
            .on_exit("exit")
            .on_press(label)
            .into()
    }

    /// The same box when it is a picture: it answers nothing and offers no cursor.
    fn blank() -> Element<'static, &'static str, iced::Theme, Null> {
        Space::new(Length::Fixed(200.0), Length::Fixed(100.0)).into()
    }

    /// Lay `stack` out and hand it to `ask`, which is the thing under test.
    ///
    /// The two steps are the ones `iced_runtime-0.12.1`'s `UserInterface::build`
    /// takes before it has anything to ask (`user_interface.rs:96-102`): diff the
    /// tree, then lay the widget out against it. Doing both in one function is
    /// also what keeps the `Layout` alive -- it borrows the node, so it cannot be
    /// returned beside it.
    fn with_layout<T>(mut stack: Test, ask: impl FnOnce(&mut Test, &mut Tree, Layout<'_>) -> T) -> T {
        let widget: &dyn Widget<&'static str, iced::Theme, Null> = &stack;
        let mut tree = Tree::new(widget);
        stack.diff(&mut tree);
        let renderer = Null;
        let node = stack.layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(iced::Size::ZERO, iced::Size::new(400.0, 300.0)),
        );
        ask(&mut stack, &mut tree, Layout::new(&node))
    }

    fn at(x: f32, y: f32) -> Vector {
        Vector::new(x, y)
    }

    /// Where the pointer is put: inside every 200x100 box these tests build, and
    /// well inside it -- `mouse_area` asks `cursor.is_over` rather than
    /// `contains` (`iced_widget-0.12.3/src/mouse_area.rs:333`), so a point on a
    /// box's edge is outside it.
    fn pointer() -> mouse::Cursor {
        mouse::Cursor::Available(iced::Point::new(100.0, 50.0))
    }

    fn viewport() -> Rectangle {
        Rectangle::new(iced::Point::ORIGIN, iced::Size::new(400.0, 300.0))
    }

    /// The one event these tests send: a left press at [`pointer`].
    fn press_event() -> Event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    }

    /// That press, delivered to the stack, and what it did.
    fn press(stack: Test) -> (event::Status, Vec<&'static str>) {
        with_layout(stack, |stack, tree, layout| {
            let mut messages: Vec<&'static str> = Vec::new();
            let mut shell = Shell::new(&mut messages);
            let status = stack.on_event(
                tree,
                press_event(),
                layout,
                pointer(),
                &Null,
                &mut iced::advanced::clipboard::Null,
                &mut shell,
                &viewport(),
            );
            (status, messages)
        })
    }

    #[test]
    fn a_press_reaches_the_layer_that_is_a_control() {
        let (status, messages) = press(
            Stack::at(at(0.0, 0.0), pressable("base"))
                .over_control(at(0.0, 0.0), pressable("over")),
        );
        assert_eq!(messages, vec!["over"], "the topmost control takes the press");
        assert_eq!(status, event::Status::Captured);
    }

    #[test]
    fn a_layer_that_is_a_picture_is_not_asked_at_all() {
        // The pane's own case: a full-height rule is drawn over every control on
        // every page, and so is a ten-pixel strip of the page's own background.
        // If either were asked, this press would be theirs.
        let (status, messages) = press(
            Stack::at(at(0.0, 0.0), pressable("base"))
                .over(at(0.0, 0.0), pressable("picture"))
                .over(at(0.0, 0.0), pressable("rule")),
        );
        assert_eq!(messages, vec!["base"], "the content under a picture is what is pressed");
        assert_eq!(status, event::Status::Captured);
    }

    #[test]
    fn a_stack_of_pictures_takes_nothing_at_all() {
        // No live layer, so nothing is delivered and nothing is claimed: the
        // answer for a page that is one picture over another, which is what the
        // hosting page's `inert aria-hidden` preview is.
        let (status, messages) = press(
            Stack::<&'static str, iced::Theme, Null>::at(at(0.0, 0.0), blank())
                .over(at(0.0, 0.0), pressable("picture")),
        );
        assert!(messages.is_empty(), "a picture is not a control: {messages:?}");
        assert_eq!(status, event::Status::Ignored);
    }

    #[test]
    fn a_control_above_an_inert_layer_still_takes_the_press() {
        // Which is the pane's shape with the pane's rule removed: the hit order
        // is the paint order, so an inert layer between a control and the layer
        // under it changes nothing.
        let (status, messages) = press(
            Stack::at(at(0.0, 0.0), pressable("base"))
                .over(at(0.0, 0.0), blank())
                .over_control(at(0.0, 0.0), pressable("link")),
        );
        assert_eq!(messages, vec!["link"]);
        assert_eq!(status, event::Status::Captured);
    }

    #[test]
    fn the_cursor_agrees_with_the_delivery() {
        // Over a live control: the pointer is offered and the press is taken.
        let (interaction, (status, messages)) = with_layout(
            Stack::at(at(0.0, 0.0), blank()).over_control(at(0.0, 0.0), pressable("over")),
            |stack, tree, layout| {
                let cursor = stack.mouse_interaction(
                    tree,
                    layout,
                    pointer(),
                    &viewport(),
                    &Null,
                );
                let mut published: Vec<&'static str> = Vec::new();
                let mut shell = Shell::new(&mut published);
                let status = stack.on_event(
                    tree,
                    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    layout,
                    pointer(),
                    &Null,
                    &mut iced::advanced::clipboard::Null,
                    &mut shell,
                    &viewport(),
                );
                (cursor, (status, published))
            },
        );
        assert_eq!(interaction, mouse::Interaction::Pointer);
        assert_eq!(messages, vec!["over"]);
        assert_eq!(status, event::Status::Captured);

        // The same layer as a picture: neither the cursor nor the press.
        let (interaction, (status, messages)) = with_layout(
            Stack::at(at(0.0, 0.0), blank()).over(at(0.0, 0.0), pressable("over")),
            |stack, tree, layout| {
                let cursor = stack.mouse_interaction(
                    tree,
                    layout,
                    pointer(),
                    &viewport(),
                    &Null,
                );
                let mut published: Vec<&'static str> = Vec::new();
                let mut shell = Shell::new(&mut published);
                let status = stack.on_event(
                    tree,
                    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    layout,
                    pointer(),
                    &Null,
                    &mut iced::advanced::clipboard::Null,
                    &mut shell,
                    &viewport(),
                );
                (cursor, (status, published))
            },
        );
        assert_eq!(interaction, mouse::Interaction::Idle, "a picture offers no cursor");
        assert!(messages.is_empty(), "{messages:?}");
        assert_eq!(status, event::Status::Ignored);
    }

    #[test]
    fn the_cursor_falls_through_to_the_layer_underneath() {
        // The reference's own rule for two boxes over one another: a layer that
        // says nothing about the pointer here hands the question to the one below
        // it rather than answering for it.
        let interaction = with_layout(
            Stack::at(at(0.0, 0.0), pressable("base"))
                .over(at(0.0, 0.0), blank()),
            |stack, tree, layout| {
                stack.mouse_interaction(tree, layout, pointer(), &viewport(), &Null)
            },
        );
        assert_eq!(
            interaction,
            mouse::Interaction::Pointer,
            "an inert layer over a control must not hide the control's cursor"
        );
    }

    #[test]
    fn a_layer_keeps_its_state_across_a_frame() {
        // The state that makes a hover a hover lives in the tree, not in the
        // widget: `mouse_area` remembers `is_hovered` there, and the second half
        // of a crossing -- the pointer leaving a control it was on -- is
        // published only if that memory survived the frame. So this is the test
        // for a stack that *rebuilds* its layers instead of diffing them, and it
        // is the difference between a control that lights up under the pointer
        // and one that latches on the first visit.
        let limits = || layout::Limits::new(iced::Size::ZERO, iced::Size::new(400.0, 300.0));
        let on = || mouse::Cursor::Available(iced::Point::new(100.0, 50.0));
        let off = || mouse::Cursor::Available(iced::Point::new(-50.0, -50.0));
        let moved = |cursor: mouse::Cursor| Event::Mouse(mouse::Event::CursorMoved { position: match cursor {
            mouse::Cursor::Available(point) => point,
            mouse::Cursor::Unavailable => iced::Point::ORIGIN,
        } });
        let mut messages: Vec<&'static str> = Vec::new();

        let mut stack = Stack::at(at(0.0, 0.0), crossed("base"));
        let widget: &dyn Widget<&'static str, iced::Theme, Null> = &stack;
        let mut tree = Tree::new(widget);
        stack.diff(&mut tree);
        let node = stack.layout(&mut tree, &Null, &limits());
        let mut shell = Shell::new(&mut messages);
        stack.on_event(
            &mut tree,
            moved(on()),
            Layout::new(&node),
            on(),
            &Null,
            &mut iced::advanced::clipboard::Null,
            &mut shell,
            &viewport(),
        );
        assert_eq!(messages, vec!["enter"], "the pointer arrived on a control");

        // The next frame builds the same stack again and hands it the tree the
        // frame before left behind. Everything in it is a *new* widget value --
        // there is no other way to build one -- so the only thing that can carry
        // the crossing across is a diff.
        let mut next = Stack::at(at(0.0, 0.0), crossed("base"));
        let widget: &dyn Widget<&'static str, iced::Theme, Null> = &next;
        tree.diff(widget);
        let node = next.layout(&mut tree, &Null, &limits());
        let mut shell = Shell::new(&mut messages);
        next.on_event(
            &mut tree,
            moved(off()),
            Layout::new(&node),
            off(),
            &Null,
            &mut iced::advanced::clipboard::Null,
            &mut shell,
            &viewport(),
        );
        assert_eq!(
            messages,
            vec!["enter", "exit"],
            "and the pointer leaving it is still a crossing"
        );
    }

    /// Counts the `container` calls an operation makes, which is how a walk is
    /// counted: `Operation::container` is the hook a container calls to hand the
    /// walk on to what it holds (`iced_core-0.12.3/src/widget/operation.rs:33`).
    #[derive(Debug, Default)]
    struct Counting(usize);

    impl iced::advanced::widget::Operation<&'static str> for Counting {
        fn container(
            &mut self,
            _id: Option<&iced::advanced::widget::Id>,
            _bounds: Rectangle,
            operate_on_children: &mut dyn FnMut(
                &mut dyn iced::advanced::widget::Operation<&'static str>,
            ),
        ) {
            self.0 += 1;
            operate_on_children(self);
        }
    }

    #[test]
    fn focus_reaches_the_live_layers_and_stops_at_the_pictures() {
        // Four layers over a base, alternating picture and control.
        let layered = || {
            Test::at(at(0.0, 0.0), container(blank()))
                .over(at(0.0, 0.0), container(blank()))
                .over_control(at(0.0, 0.0), container(blank()))
                .over(at(0.0, 0.0), container(blank()))
                .over_control(at(0.0, 0.0), container(blank()))
        };
        let counted = with_layout(layered(), |stack, tree, layout| {
            let mut operation = Counting::default();
            stack.operate(tree, layout, &Null, &mut operation);
            operation.0
        });
        assert_eq!(
            counted, 4,
            "the stack, its base and its two controls -- the two pictures are not walked"
        );

        // And the bare case: a stack with nothing over it still hands the walk on.
        let counted = with_layout(
            Test::at(at(0.0, 0.0), container(blank())),
            |stack, tree, layout| {
                let mut operation = Counting::default();
                stack.operate(tree, layout, &Null, &mut operation);
                operation.0
            },
        );
        assert_eq!(counted, 2, "the stack and its base");
    }
}
