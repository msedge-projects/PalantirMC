//! The pages, one module each.
//!
//! The shell draws the furniture -- title bar, rail, right panel -- and each page draws what
//! goes in the content column. That split is the reference's own: its shell is
//! `app/shell/{title-bar,sidebar}` and its pages are `pages/*`, and keeping the same seam
//! means a page can be ported, compared and signed off without touching the chrome.
//!
//! A page's signature is deliberately plain -- the rect it owns, the theme, and whichever
//! caches it needs -- rather than a handle to the shell. A page that could reach back into
//! the shell would be able to change the rail from inside a list, which is how a UI ends up
//! with navigation nobody can trace.

pub mod home;

use eframe::egui::Rect;

/// Which page the content column is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// The welcome screen: a hero when there is nothing installed, the instance list
    /// otherwise.
    Home,
    /// Browse and search projects.
    Discover,
}

impl Page {
    /// The name the reference prints beside the breadcrumb.
    pub fn title(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Discover => "Discover modpacks",
        }
    }

    /// The page's rect, from the shell's measured columns.
    pub fn rect(self) -> Rect {
        use crate::geometry;
        Rect::from_min_size(
            eframe::egui::pos2(
                geometry::CONTENT_LEFT,
                // The breadcrumb lives on the title bar; every page starts below it.
                geometry::TITLE_BAR,
            ),
            eframe::egui::vec2(
                geometry::PANEL_LEFT - geometry::CONTENT_LEFT - 16.0,
                geometry::WINDOW.1 - geometry::TITLE_BAR,
            ),
        )
    }
}
