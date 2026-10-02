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
use iced::advanced::{layout, mouse, renderer, Layout, Widget};
use iced::{Element, Length, Rectangle, Vector};

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
/// this is the whole of it. Two rules, both from CSS:
///
/// * **The first layer sets the box.** Its layout node *is* the stack's, so a
///   layer placed at an offset that runs past it is the caller's to clip -- which
///   is what the toast does, and what the reference's own clipped viewport does
///   with it (`left-[32%]` of a 400 box is 128, and 128 + 336 = 464).
/// * **Later layers take the pointer.** Events, hover and cursor are the first
///   layer's. Where every layer is inside something the reference marks
///   `inert aria-hidden` that is the only defensible answer; where a later layer
///   is a real control over a picture, the caller orders them so the layer that
///   wants the click is first.
pub struct Stack<'a, Message, Theme, Renderer> {
    /// `(offset from the stack's origin, the element)`, painted in this order.
    layers: Vec<(Vector, Element<'a, Message, Theme, Renderer>)>,
}

impl<'a, Message, Theme, Renderer> Stack<'a, Message, Theme, Renderer> {
    /// The layer the stack's own box is taken from.
    pub fn at(
        offset: Vector,
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Stack { layers: vec![(offset, element.into())] }
    }

    /// One more layer, over everything laid down so far.
    pub fn over(
        mut self,
        offset: Vector,
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.layers.push((offset, element.into()));
        self
    }
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
        self.layers.first().map_or(tree::Tag::stateless(), |(_, base)| base.as_widget().tag())
    }

    fn state(&self) -> tree::State {
        self.layers.first().map_or(tree::State::None, |(_, base)| base.as_widget().state())
    }

    fn children(&self) -> Vec<Tree> {
        self.layers
            .iter()
            .map(|(_, element)| Tree::new(element.as_widget()))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        // iced's own idiom: build the children this frame, diff each into the
        // matching one from last frame, then swap. Dropping the rest is what
        // retires a layer that is no longer there.
        let mut fresh: Vec<Tree> = self
            .layers
            .iter()
            .map(|(_, element)| Tree::new(element.as_widget()))
            .collect();
        for (old, (_, element)) in tree.children.iter_mut().zip(self.layers.iter()) {
            element.as_widget().diff(old);
        }
        std::mem::swap(&mut tree.children, &mut fresh);
    }

    fn size(&self) -> iced::Size<Length> {
        self.layers.first().map_or(iced::Size::new(Length::Shrink, Length::Shrink), |(_, base)| {
            base.as_widget().size()
        })
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let mut nodes: Vec<layout::Node> = Vec::with_capacity(self.layers.len());
        for (index, (offset, element)) in self.layers.iter().enumerate() {
            // `diff` keeps `tree.children` as long as `layers`, so the two only
            // fall out of step if this runs before it -- and then there is nothing
            // to draw anyway.
            let Some(child) = tree.children.get_mut(index) else { break };
            nodes.push(element.as_widget().layout(child, renderer, limits).translate(*offset));
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
        for (index, (_, element)) in self.layers.iter().enumerate() {
            let (Some(child), Some(node)) = (children.next(), tree.children.get(index)) else {
                continue;
            };
            element.as_widget().draw(node, renderer, theme, style, child, cursor, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        // The base layer's, and nothing else: every stack on this page is a
        // picture of something inert, and an overlay that swallowed the pointer
        // would make the picture feel like a control.
        let (Some(base), Some(node)) = (self.layers.first(), tree.children.first()) else {
            return mouse::Interaction::default();
        };
        let Some(child) = layout.children().next() else {
            return mouse::Interaction::default();
        };
        base.1.as_widget().mouse_interaction(node, child, cursor, viewport, renderer)
    }

    fn operate(
        &self,
        _state: &mut Tree,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        _operation: &mut dyn iced::advanced::widget::Operation<Message>,
    ) {
    }
}
