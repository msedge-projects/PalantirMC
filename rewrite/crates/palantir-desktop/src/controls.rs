use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, renderer, widget};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, event};

pub fn action<'a, M: Clone + 'a>(
    caption: &str,
    message: M,
    style: crate::design::Button,
) -> Element<'a, M> {
    let button = iced::widget::button(crate::design::label(caption))
        .padding([10, 14])
        .style(style)
        .on_press(message.clone());
    content(button.into(), message)
}
pub fn content<'a, M: Clone + 'a>(content: Element<'a, M>, message: M) -> Element<'a, M> {
    Element::new(Action {
        content,
        message: Some(message),
        previous: None,
        next: None,
    })
}
pub fn toggle<'a, M: Clone + 'a>(
    caption: &str,
    checked: bool,
    on_toggle: fn(bool) -> M,
) -> Element<'a, M> {
    let checkbox = iced::widget::checkbox(caption, checked).on_toggle(on_toggle);
    content(checkbox.into(), on_toggle(!checked))
}
pub fn picker<'a, T, M>(
    options: impl IntoIterator<Item = T>,
    selected: Option<T>,
    on_select: fn(T) -> M,
) -> Element<'a, M>
where
    T: Clone + ToString + PartialEq + 'a,
    M: Clone + 'a,
{
    let options: Vec<T> = options.into_iter().collect();
    let index = selected
        .as_ref()
        .and_then(|s| options.iter().position(|o| o == s))
        .unwrap_or(0);
    let previous = options.get(index.saturating_sub(1)).cloned().map(on_select);
    let next = options
        .get((index + 1).min(options.len().saturating_sub(1)))
        .cloned()
        .map(on_select);
    let content = iced::widget::pick_list(options, selected, on_select)
        .text_shaping(iced::widget::text::Shaping::Advanced)
        .style(crate::design::selector())
        .padding(12)
        .into();
    Element::new(Action {
        content,
        message: None,
        previous,
        next,
    })
}
struct Action<'a, M> {
    content: Element<'a, M>,
    message: Option<M>,
    previous: Option<M>,
    next: Option<M>,
}
#[derive(Default)]
struct Focus(bool);
impl widget::operation::Focusable for Focus {
    fn is_focused(&self) -> bool {
        self.0
    }
    fn focus(&mut self) {
        self.0 = true;
    }
    fn unfocus(&mut self) {
        self.0 = false;
    }
}
impl<M: Clone> Widget<M, Theme, Renderer> for Action<'_, M> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<Focus>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(Focus::default())
    }
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(&[&self.content]);
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn layout(
        &self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &self,
        tree: &mut widget::Tree,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn widget::Operation<M>,
    ) {
        operation.focusable(tree.state.downcast_mut::<Focus>(), None);
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        if tree.state.downcast_ref::<Focus>().0 {
            use iced::advanced::Renderer as _;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: iced::Border {
                        color: theme.palette().primary,
                        width: 2.0,
                        radius: 9.0.into(),
                    },
                    ..Default::default()
                },
                iced::Color::TRANSPARENT,
            );
        }
    }
    fn on_event(
        &mut self,
        tree: &mut widget::Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) -> event::Status {
        if matches!(
            &event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) {
            tree.state.downcast_mut::<Focus>().0 = cursor.is_over(layout.bounds());
        }
        if tree.state.downcast_ref::<Focus>().0
            && matches!(
                &event,
                Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::Enter | iced::keyboard::key::Named::Space
                    ),
                    ..
                })
            )
        {
            if let Some(message) = &self.message {
                shell.publish(message.clone());
            } else {
                return self.content.as_widget_mut().on_event(
                    &mut tree.children[0],
                    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    layout,
                    mouse::Cursor::Available(layout.bounds().center()),
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            }
            return event::Status::Captured;
        }
        if tree.state.downcast_ref::<Focus>().0 {
            if let Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(key),
                ..
            }) = &event
            {
                use iced::keyboard::key::Named;
                let selection = match key {
                    Named::ArrowUp | Named::ArrowLeft => self.previous.as_ref(),
                    Named::ArrowDown | Named::ArrowRight => self.next.as_ref(),
                    _ => None,
                };
                if let Some(message) = selection {
                    shell.publish(message.clone());
                    return event::Status::Captured;
                }
            }
        }
        self.content.as_widget_mut().on_event(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }
    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
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
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'a, M, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, translation)
    }
}
