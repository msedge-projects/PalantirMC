//! What every page is built from: the state of what it asked for, and the blocks
//! it draws when the answer is not ready.
//!
//! Nineteen routes in the reference all do the same three things: they ask for
//! something, they draw it when it arrives, and they show *something honest* when
//! it has not. The third is the part a port usually skips -- a spinner that never
//! resolves, or an empty list that means "the request failed". [`Load`] and
//! [`draw`] are that third part, written once so no page can forget it, and every
//! page's gate asserts all four arms.
//!
//! Geometry is the reference's, quoted where it is used:
//!
//! | Number | Source |
//! | --- | --- |
//! | 24px page inset | `p-6` on `Index.vue`'s library page and `Browse.vue`'s body |
//! | 12px between blocks | `gap-3` on the library page, `gap-2` (8px) elsewhere |
//! | 8px block padding step | Tailwind's 4px scale, which `theme_gen` carries |

#![allow(dead_code)]

use iced::widget::{column, container, Space};
use iced::{Alignment, Element, Font, Length, Padding};

use crate::icon;
use crate::icons_gen::Glyph;
use crate::style::{heading, medium, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Ink, Theme as Gen};
use crate::ui::text;

/// `p-6` on the pages that use the library layout.
pub const INSET: f32 = 24.0;
/// `gap-3`, between the blocks of a page's body.
pub const GAP: f32 = 12.0;
/// `gap-2`, between the parts of a row.
pub const ROW_GAP: f32 = 8.0;
/// `gap-4`, between the cards of a grid.
pub const GRID_GAP: f32 = 16.0;

/// The state of something a page asked for.
///
/// `Empty` is deliberately not `Ready(vec![])`: the reference draws a title and a
/// sentence for "there is nothing here", and a page that had to tell the two
/// apart by looking inside its data would eventually draw the wrong one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Load<T> {
    /// Nothing has been asked for yet: the page has not been shown.
    #[default]
    Idle,
    /// Asked for, not answered.
    Loading,
    /// Answered, and there is nothing to show.
    Empty,
    /// Answered, with something to show.
    Ready(T),
    /// Failed, with the reason the reader is shown.
    Failed(String),
}

impl<T> Load<T> {
    /// The value, once there is one.
    pub fn ready(&self) -> Option<&T> {
        match self {
            Load::Ready(value) => Some(value),
            _ => None,
        }
    }

    /// Whether the answer has arrived, either way.
    pub const fn settled(&self) -> bool {
        matches!(self, Load::Empty | Load::Ready(_) | Load::Failed(_))
    }

    /// Whether the page is still waiting.
    pub const fn waiting(&self) -> bool {
        matches!(self, Load::Idle | Load::Loading)
    }

    /// The same load with its value mapped.
    pub fn map<U>(self, convert: impl FnOnce(T) -> U) -> Load<U> {
        match self {
            Load::Idle => Load::Idle,
            Load::Loading => Load::Loading,
            Load::Empty => Load::Empty,
            Load::Ready(value) => Load::Ready(convert(value)),
            Load::Failed(reason) => Load::Failed(reason),
        }
    }

    /// The reason the request failed, if it did.
    pub fn failure(&self) -> Option<&str> {
        match self {
            Load::Failed(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Draw whatever a load is holding, and the right block for the arms that hold
/// nothing.
///
/// This is the one place a page's four states are turned into pixels, which is
/// what makes "every page has an empty state" a property of the toolkit rather
/// than a promise about nineteen files. `what` names the thing being asked for,
/// so the loading block can say it.
pub fn draw<'a, T, Message: 'a>(
    theme: Gen,
    load: &'a Load<T>,
    what: &str,
    ready: impl FnOnce(&'a T) -> Element<'a, Message>,
) -> Element<'a, Message> {
    match load {
        Load::Ready(value) => ready(value),
        Load::Empty => empty(theme, Key::BrowseNoResults),
        Load::Failed(reason) => failed(theme, reason),
        Load::Idle | Load::Loading => waiting(theme, what),
    }
}

/// A page's body: the inset, the spacing, and a scroll region.
///
/// The reference's pages are `flex flex-col gap-* p-6` inside `.app-viewport`,
/// which scrolls. Same here: one scroll region per page, and the inset inside it
/// so the text does not slide under the pane's rounded corner as it scrolls.
///
/// `on_wheel` is how a page tells the shell that the pointer scrolled in it.
/// [`crate::scroll::region`] takes the wheel before iced's own handling does, and
/// what happens next is not the page's: iced moves a scrollable with
/// `scroll_to`, which is a command, and a command is an update's to return. So a
/// page carries one message variant for the wheel and knows nothing else about
/// scrolling -- see [`crate::scroll`] for what the glide is and why it is not
/// drawn as a plain wheel notch.
///
/// The name comes back out of the region rather than being written here, so a page
/// hands its own message constructor in and nothing else: which region the wheel
/// happened in is a fact about the widget, and the shell keys its glides by it.
pub fn body<'a, Message: 'a>(
    blocks: Vec<Element<'a, Message>>,
    gap: f32,
    on_wheel: impl Fn(&'static str, crate::scroll::Wheel) -> Message + 'a,
) -> Element<'a, Message> {
    let mut items = column![].spacing(gap).width(Length::Fill);
    for block in blocks {
        items = items.push(block);
    }
    crate::scroll::region(
        crate::scroll::PAGE,
        container(items)
            .width(Length::Fill)
            .padding(Padding { top: INSET, right: INSET, bottom: INSET, left: INSET }),
        on_wheel,
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// A page heading: `text-2xl font-extrabold text-contrast`.
pub fn title<'a, Message: 'a>(theme: Gen, key: Key) -> Element<'a, Message> {
    text(key.message())
        .size(24.0)
        .font(heading())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)))
        .into()
}

/// A section heading with the count the reference puts beside it.
pub fn section<'a, Message: 'a>(theme: Gen, key: Key, count: Option<usize>) -> Element<'a, Message> {
    let mut row = iced::widget::row![]
        .align_items(Alignment::Center)
        .spacing(ROW_GAP)
        .push(
            text(key.message())
                .size(16.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        );
    if let Some(count) = count {
        row = row.push(
            text(count.to_string())
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        );
    }
    row.into()
}

/// What a page says while it is waiting for its own data.
pub fn waiting<'a, Message: 'a>(theme: Gen, what: &str) -> Element<'a, Message> {
    sentence(theme, &format!("Loading {what}…"))
}

/// What a page says when the request failed.
///
/// The reason is the store's, printed rather than swallowed: a failed request that
/// looks like an empty list is the failure mode this whole module exists to
/// prevent.
pub fn failed<'a, Message: 'a>(theme: Gen, reason: &str) -> Element<'a, Message> {
    sentence(theme, reason)
}

/// What a page says when there is nothing to show.
///
/// `browse.no-results` is the reference's own sentence for it, so this block and
/// the reference agree word for word.
pub fn empty<'a, Message: 'a>(theme: Gen, key: Key) -> Element<'a, Message> {
    sentence(theme, key.message())
}

/// Something a page could not do, with the control that dismisses it.
///
/// Every page has one of these and it is the same shape everywhere, which is the
/// point: the reference's `Admonition` has no close button, and a notice that
/// cannot be dismissed is a sentence that follows the reader around. The control
/// is here rather than in [`crate::ui::admonition`] because dismissing is page
/// state and the widget kit has none -- a page passes the message that means
/// "this is read now".
pub fn notice<'a, Message: Clone + crate::ui::Hovered + 'a>(
    theme: Gen,
    header: Key,
    body: &str,
    dismiss: Message,
) -> Element<'a, Message> {
    iced::widget::row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Start)
        .push(crate::ui::admonition(theme, crate::ui::Severity::Info, header.message(), body))
        .push(Space::with_width(Length::Fill))
        // One dismiss control on the page, and every page's notice names it the
        // same way: the tween it gets is the crossing this page's family routes.
        .push(crate::ui::icon_button(theme, "page:notice:dismiss", Glyph::X, 16.0, dismiss))
        .into()
}

/// The sentence, centred, in the quiet ink.
fn sentence<'a, Message: 'a>(theme: Gen, sentence: &str) -> Element<'a, Message> {
    container(
        text(sentence.to_string())
            .size(14.0)
            .font(medium())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
    )
    .width(Length::Fill)
    .padding(24.0)
    .center_x()
    .into()
}

/// A horizontal rule, `bg-surface-5`.
pub fn rule<'a, Message: 'a>(theme: Gen) -> Element<'a, Message> {
    container(Space::with_height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Appearance {
            background: Some(iced::Background::Color(theme_gen::ink(theme, Ink::Surface5))),
            ..container::Appearance::default()
        })
        .into()
}

/// An icon in the quiet ink, at the size the reference's chrome uses.
pub fn glyph<'a, Message: 'a>(
    theme: Gen,
    glyph: Glyph,
    size: f32,
) -> Element<'a, Message> {
    icon::icon(glyph, size, theme_gen::ink(theme, INK_DEFAULT))
}

/// The face a page's own metadata is set in, for a caller that needs the font
/// rather than an element.
pub const fn meta() -> Font {
    medium()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme_gen::Theme as Gen;

    /// Draw a load the way a page does, for one theme.
    ///
    /// The element borrows the load, which is why this is a loop body rather than
    /// a function in the test below: a returned element would outlive the value it
    /// borrowed.
    fn draw_for_every_theme(load: &Load<u32>) {
        for theme in Gen::ALL {
            let ready = |value: &u32| -> Element<'_, ()> { text(value.to_string()).into() };
            drop(draw(*theme, load, "something", ready));
        }
    }

    #[test]
    fn every_arm_of_a_load_draws_something() {
        // The gate the whole module exists for: four arms, four answers, and the
        // failure carrying its own reason rather than looking like an empty list.
        let arms = [
            Load::Idle,
            Load::Loading,
            Load::Empty,
            Load::Ready(7),
            Load::Failed("the store is not wired yet".to_string()),
        ];
        for load in &arms {
            draw_for_every_theme(load);
        }
    }

    #[test]
    fn a_load_knows_whether_it_is_waiting_settled_or_failed() {
        assert!(Load::<u32>::Idle.waiting());
        assert!(Load::<u32>::Loading.waiting());
        assert!(!Load::<u32>::Empty.waiting());
        assert!(!Load::<u32>::Idle.settled());
        assert!(Load::<u32>::Empty.settled());
        assert!(Load::<u32>::Ready(1).settled());
        assert!(Load::<u32>::Failed("x".into()).settled());
        assert_eq!(Load::Ready(1u32).ready(), Some(&1));
        assert_eq!(Load::<u32>::Empty.ready(), None);
        assert_eq!(Load::<u32>::Ready(1).failure(), None);
        assert_eq!(Load::<u32>::Failed("why".into()).failure(), Some("why"));
        // The default is `Idle`, so a page's struct can be `Default` and cannot
        // accidentally start out claiming to have data.
        assert_eq!(Load::<Vec<u32>>::default(), Load::<Vec<u32>>::Idle);
    }

    #[test]
    fn mapping_a_load_leaves_the_arms_that_hold_nothing_alone() {
        assert_eq!(Load::<u32>::Loading.map(|value| value + 1), Load::Loading);
        assert_eq!(Load::<u32>::Empty.map(|value| value + 1), Load::Empty);
        assert_eq!(
            Load::<u32>::Failed("why".into()).map(|value| value + 1),
            Load::<u32>::Failed("why".into())
        );
        assert_eq!(Load::Ready(1u32).map(|value| value + 1), Load::Ready(2));
        // The mapping happens once, on the value that is there.
        let mut calls = 0;
        let mapped = Load::Ready(1u32).map(|value| {
            calls += 1;
            value
        });
        assert_eq!(calls, 1);
        assert_eq!(mapped, Load::Ready(1));
    }

    #[test]
    fn the_furniture_is_the_reference_s_own_spacing() {
        // `p-6`, `gap-3`, `gap-2`, `gap-4` at Tailwind's 4px step.
        assert_eq!(INSET, 24.0);
        assert_eq!(GAP, 12.0);
        assert_eq!(ROW_GAP, 8.0);
        assert_eq!(GRID_GAP, 16.0);
        assert_eq!(INSET, 6.0 * 4.0);
        assert_eq!(GAP, 3.0 * 4.0);
        assert_eq!(ROW_GAP, 2.0 * 4.0);
        assert_eq!(GRID_GAP, 4.0 * 4.0);
        // And a body of any length builds, for every theme: a page whose blocks
        // are drawn in a theme with no ink for one of them is a page that panics
        // on someone's machine.
        for theme in Gen::ALL {
            let blocks: Vec<Element<'_, ()>> = vec![
                title(*theme, Key::AppNavigationHome),
                section(*theme, Key::AppNavigationHome, Some(3)),
                rule(*theme),
                glyph(*theme, Glyph::Play, 20.0),
            ];
            // A message constructor that is never published: what this draws in
            // is the region, and a region with nothing to publish is still a
            // region.
            drop(body(blocks, GAP, |_, _| ()));
        }
    }
}
