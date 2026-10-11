use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer, widget};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector, event};

pub fn modal<'a, M: Clone + 'a>(
    base: Element<'a, M>,
    dialog: Element<'a, M>,
    close: M,
) -> Element<'a, M> {
    Element::new(Modal {
        base,
        dialog,
        close,
    })
}
struct Modal<'a, M> {
    base: Element<'a, M>,
    dialog: Element<'a, M>,
    close: M,
}
impl<M: Clone> Widget<M, Theme, Renderer> for Modal<'_, M> {
    fn size(&self) -> Size<Length> {
        self.base.as_widget().size()
    }
    fn children(&self) -> Vec<widget::Tree> {
        vec![
            widget::Tree::new(&self.base),
            widget::Tree::new(&self.dialog),
        ]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(&[&self.base, &self.dialog]);
    }
    fn layout(
        &self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.base
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.base.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }
    // Only the overlay receives interaction while a dialog is visible.
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        _translation: Vector,
    ) -> Option<overlay::Element<'a, M, Theme, Renderer>> {
        Some(overlay::Element::new(Box::new(Dialog {
            content: &mut self.dialog,
            tree: &mut tree.children[1],
            close: self.close.clone(),
        })))
    }
    fn operate(
        &self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation<M>,
    ) {
        // Focus traversal belongs to the dialog, not the dimmed background. Overlay
        // operation supplies the dialog's actual layout through Dialog::operate.
        operation.container(None, layout.bounds(), &mut |_operation| {});
        let _ = (tree, renderer);
    }
}
struct Dialog<'a, 'b, M> {
    content: &'a mut Element<'b, M>,
    tree: &'a mut widget::Tree,
    close: M,
}
impl<M: Clone> overlay::Overlay<M, Theme, Renderer> for Dialog<'_, '_, M> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(
            Size::ZERO,
            Size::new(
                (bounds.width - 32.0).max(0.0),
                (bounds.height - 32.0).max(0.0),
            ),
        );
        let child = self
            .content
            .as_widget()
            .layout(self.tree, renderer, &limits);
        let size = child.size();
        layout::Node::with_children(
            bounds,
            vec![child.move_to(Point::new(
                (bounds.width - size.width) / 2.0,
                (bounds.height - size.height) / 2.0,
            ))],
        )
    }
    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        use iced::advanced::Renderer as _;
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                ..Default::default()
            },
            iced::Color::from_rgba(0.03, 0.05, 0.08, 0.85),
        );
        if let Some(child) = layout.children().next() {
            self.content.as_widget().draw(
                self.tree,
                renderer,
                theme,
                style,
                child,
                cursor,
                &layout.bounds(),
            );
        }
    }
    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation<M>,
    ) {
        if let Some(child) = layout.children().next() {
            self.content
                .as_widget()
                .operate(self.tree, child, renderer, operation);
        }
    }
    fn on_event(
        &mut self,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
    ) -> event::Status {
        if let Some(child) = layout.children().next() {
            if matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            ) && cursor.is_over(layout.bounds())
                && !cursor.is_over(child.bounds())
            {
                shell.publish(self.close.clone());
                return event::Status::Captured;
            }
            self.content.as_widget_mut().on_event(
                self.tree,
                event,
                child,
                cursor,
                renderer,
                clipboard,
                shell,
                &layout.bounds(),
            );
        }
        event::Status::Captured
    }
    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        layout
            .children()
            .next()
            .map(|child| {
                self.content
                    .as_widget()
                    .mouse_interaction(self.tree, child, cursor, viewport, renderer)
            })
            .unwrap_or_default()
    }
    fn overlay<'a>(
        &'a mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'a, M, Theme, Renderer>> {
        let child = layout.children().next()?;
        self.content
            .as_widget_mut()
            .overlay(self.tree, child, renderer, Vector::ZERO)
    }
}
