//! Small interactive pieces the pages share.
//!
//! A widget lives here when more than one page needs it and when it owns state: the search
//! field keeps focus and a blink phase, which is exactly the kind of thing that turns into
//! two subtly different copies if each page keeps its own.

pub mod search;
