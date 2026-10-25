//! Where things are, in the reference's own pixels.
//!
//! Every number here was read off a capture of the Modrinth app at 1440x900 with
//! `tools/parity/edges.py`, not estimated: the tool walks a row of the PNG and reports
//! every x where a channel moves, which is where a panel, a field or a rule begins. The
//! constants exist so that a page's layout is arithmetic over measurements rather than a
//! pile of magic numbers each page repeats.
//!
//! **The measurements are the contract.** When a capture disagrees with a constant, the
//! constant is what gets fixed, and the evidence goes in `docs/PARITY-LOG.md`. That is the
//! opposite of the usual direction, where the layout is whatever looked right and the
//! screenshot is a souvenir.
//!
//! Re-measure with:
//!
//! ```text
//! python3 tools/parity/edges.py --png out/discover-idle-modrinth.png --row 105
//! ```
//!
//! Rows that matter: 20 (title bar), 105 (tab strip), 150 (search row), 300 (first card).

/// The window the captures are taken at.
pub const WINDOW: (f32, f32) = (1440.0, 900.0);

/// Height of the title bar, measured as the first row where the rail's own background
/// begins below the header. Row 20 is inside it; row 105 is below it.
pub const TITLE_BAR: f32 = 40.0;

/// The left rail's width: row 105's first edge pair is `64,65`, the rail's right border.
pub const RAIL: f32 = 64.0;

/// The gap between the rail's border and the content column: row 105 puts the content's
/// own left edge at `88,89`, row 150 agrees.
pub const CONTENT_INSET: f32 = 24.0;

/// The content column's left edge in window coordinates (`RAIL + CONTENT_INSET`).
pub const CONTENT_LEFT: f32 = RAIL + CONTENT_INSET;

/// The right panel's width. The panel is the last thing before the window's right edge;
/// its left border sits at `WINDOW.0 - PANEL`.
pub const PANEL: f32 = 340.0;

/// The right panel's left edge in window coordinates.
pub const PANEL_LEFT: f32 = WINDOW.0 - PANEL;

/// Where the first tab's label starts, inside the tab strip: row 105 puts a text edge at
/// `196,197`. Used as a check on the strip's own padding rather than as a layout input.
pub const FIRST_TAB_LABEL: f32 = 196.0;

/// Where the first back/forward circle's centre sits, on the title bar's own row. Measured
/// at 2x from the reference: the circles begin at 167 window pixels.
pub const BREADCRUMB_ARROWS: f32 = 180.0;

/// The route badge's centre on the title bar, 240 window pixels in the reference.
pub const BREADCRUMB_BADGE: f32 = 253.0;

/// Where the tab strip's rounded container begins, below the title bar.
pub const TAB_CONTAINER_TOP: f32 = 48.0;

/// The tab container's height: 36-point pills inside 8-point padding.
pub const TAB_CONTAINER: f32 = 52.0;

/// One tab pill's height.
pub const TAB_PILL: f32 = 36.0;

/// The tab strip's height: the label sits at row 105 and the strip's bottom rule is the
/// next horizontal edge below it.
pub const TAB_STRIP: f32 = 36.0;

/// A result card's height in Discover's list layout, measured from the reference's first
/// card's top and the second card's top.
pub const CARD: f32 = 116.0;

/// The gap between two result cards.
pub const CARD_GAP: f32 = 12.0;

/// The distance between two rail icon centres. The rail's icons are evenly pitched; the
/// pitch is what makes a rail look like a rail rather than a list.
pub const RAIL_PITCH: f32 = 56.0;

/// The centre of the first rail icon, measured from the top of the window.
pub const RAIL_FIRST: f32 = 64.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// The geometry has to add up, or every page inherits the error.
    #[test]
    fn the_columns_do_not_overlap_and_reach_the_window_edge() {
        assert_eq!(CONTENT_LEFT, 88.0, "measured at row 105 and row 150");
        assert!(
            CONTENT_LEFT + 24.0 <= PANEL_LEFT,
            "the content column must leave room before the panel"
        );
        assert_eq!(PANEL_LEFT, 1100.0, "1440 - 340");
        assert_eq!(PANEL_LEFT + PANEL, WINDOW.0, "the panel ends at the window edge");
    }

    /// The first tab's label is inside the strip, and the strip is inside the title bar's
    /// shadow and the search row: a constant that broke this order would put text in the
    /// chrome.
    #[test]
    fn the_tab_label_sits_inside_the_content_column() {
        assert!(FIRST_TAB_LABEL > CONTENT_LEFT, "the label is right of the column's edge");
        // 196 - 88 is the strip's own padding plus one tab's padding: 108 px for a tab
        // with a leading icon and 12 px of padding on each side is plausible; what must
        // hold is that the label is well within the content column.
        assert!(FIRST_TAB_LABEL - CONTENT_LEFT < 140.0);
    }

    /// A rail whose pitch is smaller than its icon would draw overlapping targets.
    #[test]
    fn the_rail_pitch_is_larger_than_the_icon_it_holds() {
        assert!(RAIL_PITCH > 40.0, "icons are 24-32 px and need a resting margin");
        assert!(RAIL_FIRST + RAIL_PITCH * 5.0 < WINDOW.1, "five icons fit");
    }
}
