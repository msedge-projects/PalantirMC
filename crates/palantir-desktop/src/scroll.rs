//! Scroll policy for the desktop shell: how much to draw, and how to move.
//!
//! iced 0.12's `Scrollable` consumes wheel events itself and applies each
//! notch as an instant 60-pixel jump. That is why scrolling a long page on
//! Windows feels steppy: the content teleports once per notch rather than
//! gliding. Nothing in iced exposes a friction/tween setting, but the
//! scrollable does hand the event to its *content* first and stand down if the
//! content captures it — so [`WheelGuard`] wraps a page's content, swallows the
//! wheel, and lets the shell own the offset. The shell then eases toward a
//! target, and only asks for frames while something is still moving.
//!
//! The tween is a function of **time**, and the deadline is the whole of the
//! policy:
//!
//! * A frame is not free. iced rebuilds the interface for every message it
//!   receives, rasterises the window and blits it, and with no usable GPU the
//!   rasteriser does that on the CPU. A gesture therefore costs one frame per
//!   frame of animation, so the number of frames a gesture draws is the thing
//!   to bound.
//! * [`DURATION`] is a deadline rather than a frame count, so a machine that
//!   answers slowly draws fewer, larger steps instead of a longer animation.
//!   After the deadline the gesture is over, whatever else happened -- a machine
//!   slow enough to draw two steps gets two steps, not a slideshow.
//!
//! An earlier revision went further and *classified* the machine, taking the
//! glide away from one whose frames were slower than 24 ms. It was removed after
//! the reference's own scrolling was measured frame by frame (`GATES.md` G140):
//! the official client glides a wheel over two to six frames at ~30 fps, while
//! ours arrived in a single frame, 14 wheel events out of 14 -- and the
//! classifier is what produced that, because 24 ms is faster than both the rate
//! the reference draws at (~33 ms) and the rate this box's software rasteriser
//! was profiled at (69 ms). Two frames in a row above the threshold set the
//! cost, the cost was read only when a glide *started*, and a demoted machine
//! started none -- so no frame ran again to measure it, and the demotion lasted
//! for the session. The interval between two gestures was the same mistake seen
//! from the other side: the frame timer exists only while something moves, so a
//! gesture's first tick was measured against the previous gesture's last tick,
//! which is the reader's reading time rather than a frame. `tick` measures
//! nothing now, and the deadline bounds what a slow machine pays: fewer, larger
//! steps, never more of them.
//!
//! Deliberate limits, because each one is a way this can go wrong:
//!
//! * **Only the wheel is taken.** The scrollbar, dragging it, keyboard
//!   scrolling, touch and iced's own `scroll_to` (which the log page's
//!   autoscroll uses) all still work, because this does not replace
//!   `Scrollable` — it sits inside one.
//! * **No timer while nothing moves.** The animation is a subscription that
//!   exists only while [`ScrollAnim::animating`] is true, so an idle window
//!   does no work at all.
//! * **Bounded, not physical.** Easing is against a deadline, so a gesture
//!   cannot settle forever and cannot grow with the distance it covers.

use std::time::{Duration, Instant};

use iced::advanced::widget::{tree, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::{event, Element, Event, Length, Rectangle, Size, Vector};

/// Maximum number of rows rendered by a virtualized-ish log view.
///
/// The log buffer may retain many more lines for troubleshooting, but drawing
/// all of them makes wheel input expensive. The view renders only the newest
/// window, which bounds layout/draw work per frame.
#[cfg(test)]
pub const LOG_RENDER_CAP: usize = 500;

/// Convert a buffered-line count into the number of rows the log page should
/// build. This is deliberately O(1) and allocation-free.
#[cfg(test)]
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

// ---- The regions ---------------------------------------------------------
//
// Every scroll region in this shell, named. A name rather than a type because
// that is what a region is to the two sides that share it: the call site puts
// it on the scrollable as an `iced::widget::scrollable::Id`, and the shell keys
// its glide table by it -- and the identity has to be stable across frames,
// because the command that moves a region is a lookup by that id.
//
// Two regions are on screen at once -- a page, and the panel beside it -- so the
// wheel has to name the one it happened in. The lists inside a dialog are
// regions of their own for the same reason one level down: a wheel over a
// version list is not a wheel over the sheet it sits in, and the version and
// build lists of one dialog are two regions, visible together.

/// A page's own body: [`crate::page::body`], which every page draws through.
pub const PAGE: &str = "scroll:page";
/// An instance page's tab body, which is its own region under the pinned head.
pub const CONTENT: &str = "scroll:content";
/// The right panel's sections, beside whichever page is up.
pub const PANEL: &str = "scroll:panel";
/// An open dialog's body: one modal is up at a time, so one region is enough.
pub const DIALOG: &str = "scroll:dialog";
/// The create dialog's list of game versions.
pub const VERSIONS: &str = "scroll:versions";
/// The create dialog's list of loader builds, opened by the *Other* chip.
pub const BUILDS: &str = "scroll:builds";
/// The instance-settings modal's list of game versions.
pub const GAME_VERSIONS: &str = "scroll:game-versions";
/// The same modal's list of loader builds.
pub const LOADER_BUILDS: &str = "scroll:loader-builds";
/// The same modal's list of the linked pack's versions.
pub const PACK_VERSIONS: &str = "scroll:pack-versions";

/// The id a region's scrollable answers to, and the one the shell moves.
pub fn id(name: &'static str) -> iced::widget::scrollable::Id {
    iced::widget::scrollable::Id::new(name)
}

/// A command that puts the region `name` where the policy says it is.
///
/// `scroll_to` is the only way a program moves a `Scrollable` in iced -- the
/// widget owns its offset, and there is no setter -- so a glide is a stream of
/// these, one per frame, for as long as [`ScrollAnim::animating`] is true. The x
/// offset is zero because every region here scrolls vertically; the content is as
/// wide as the region.
pub fn scroll_to<Message: 'static>(name: &'static str, offset: f32) -> iced::Command<Message> {
    iced::widget::scrollable::scroll_to(
        id(name),
        iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: offset },
    )
}

/// How often the tween asks for a frame while it is gliding.
///
/// 16 ms is 60 frames a second, the rate a machine that can hold a glide is
/// expected to. It is a *request* rather than a guarantee: the position is
/// driven by the clock, so a machine that answers late draws fewer, larger
/// steps rather than falling behind.
#[cfg(test)]
pub const FRAME: Duration = Duration::from_millis(16);

/// How long a glide takes, wall clock, on a machine that can draw one.
///
/// About ten frames at 60 fps, three at 20 fps, and the same fifth of a second
/// either way — which is the whole reason this is a duration. A frame count
/// makes the *length* of the gesture depend on how fast the machine happens to
/// be, and on a slow renderer that turns a scroll into a slideshow.
pub const DURATION: Duration = Duration::from_millis(160);

/// Below this many pixels from the target, the animation is over.
///
/// Without a floor, a re-clamped target would leave the page a fraction of a
/// pixel from where it belongs and keep asking for frames.
pub const SETTLED: f32 = 0.5;

/// Where a page's scroll position is, and where it is going.
///
/// Plain numbers and a clock rather than a reference to any widget state, so
/// the whole easing policy is unit tested without a renderer or a window.
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
    /// The glide in progress, if there is one.
    glide: Option<Glide>,
}

/// One glide: where it started, when, and the deadline it must meet.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Glide {
    from: f32,
    began: Instant,
    duration: Duration,
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
    ///
    /// True while a glide is running, and true when a target the offset has not
    /// reached is sitting there with no glide to carry it — which happens when
    /// content shrank and its re-clamped target is somewhere else. Either way
    /// the answer decides whether a frame timer exists at all.
    pub fn animating(&self) -> bool {
        self.glide.is_some() || (self.target - self.offset).abs() > SETTLED
    }

    /// Take a wheel event, with the geometry it was reported in.
    ///
    /// The geometry comes from the same event rather than from a separate
    /// report, because iced only publishes viewport changes — a page nobody has
    /// scrolled yet has never reported one, and a wheel on it would otherwise
    /// move nothing.
    ///
    /// `now` is passed in rather than read here so that the whole policy is a
    /// function of its inputs, and a test can hold the clock still.
    pub fn wheel(&mut self, wheel: Wheel, now: Instant) {
        // Where the region is is the caller's business, not this one's: only the
        // caller knows whether the offset the wheel measured is this policy's own
        // number or somebody else's. See [`Wheel::offset`] and [`Glides::wheel`].
        self.observe(wheel.content_height, wheel.view_height);
        self.scroll_notches(wheel.notches);
        self.begin(now);
    }

    /// Move the target by `notches`, positive upward.
    ///
    /// The target is clamped to the content; the *offset* is not touched,
    /// because the whole point is that it catches up smoothly.
    pub fn scroll_notches(&mut self, notches: f32) {
        self.target = self.clamp(self.target - notches * WHEEL_PIXELS_PER_NOTCH);
    }

    /// Start easing toward the target.
    ///
    /// Every machine gets the same gesture: the deadline is what bounds the
    /// cost, so a slow one draws fewer steps of it rather than none.
    fn begin(&mut self, now: Instant) {
        self.glide = None;
        if !self.animating() {
            // Already there. A wheel clamped out of range at the end of a page
            // must not start a timer for a gesture that moves nothing.
            return;
        }
        self.glide = Some(Glide { from: self.offset, began: now, duration: DURATION });
    }

    /// Advance to `now` and report whether there is more to animate.
    ///
    /// The position is a function of elapsed time against the glide's deadline,
    /// so a machine answering slowly draws fewer, larger steps and still
    /// finishes on time, while a fast one draws more, smaller ones. Nothing
    /// here can make a gesture last longer than [`DURATION`], which is the
    /// property the frame-counted tween could not offer: at 16 ms a tick
    /// against a 69 ms frame it asked for eleven frames and took three quarters
    /// of a second to move one notch.
    ///
    /// What this deliberately does *not* measure is the interval between calls.
    /// The frame timer exists only while something is moving, so the first tick
    /// of a gesture is separated from the last tick of the one before it by
    /// however long the reader spent reading -- seconds, usually -- and a policy
    /// that read that gap as a frame cost would classify the machine by the
    /// reader's browsing.
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some(glide) = self.glide else {
            // No glide: nothing was moving, and `begin` found nothing to do.
            self.offset = self.target;
            return false;
        };

        let elapsed = now.saturating_duration_since(glide.began).as_secs_f32();
        let deadline = glide.duration.as_secs_f32();
        let progress = if deadline > 0.0 { (elapsed / deadline).clamp(0.0, 1.0) } else { 1.0 };
        self.offset = glide.from + (self.target - glide.from) * ease_out(progress);

        if progress >= 1.0 {
            // Land exactly on the target rather than asymptotically near it,
            // which is what lets the timer stop instead of running forever.
            self.offset = self.target;
            self.glide = None;
            return false;
        }
        true
    }

    /// Adopt an offset that came from somewhere else — the scrollbar, a
    /// keyboard scroll, or `scroll_to` — without fighting it.
    ///
    /// Both the offset and the target move, so the next frame does not drag the
    /// content back to where the wheel last asked for it. The glide is dropped
    /// for the same reason: the hand is somewhere else now, and a tween that
    /// kept going would finish a gesture the reader has already overridden.
    pub fn resync(&mut self, offset: f32) {
        self.offset = offset;
        self.target = offset;
        self.glide = None;
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

    /// Reset for a page the user has just opened.
    ///
    /// Position and geometry belong to the new page; there is nothing else to
    /// carry across, because nothing about the machine is measured any more.
    ///
    /// Nothing in the shell calls it yet: a region keeps iced's own offset across
    /// a navigation, so resetting the policy alone would put the two sides at odds.
    /// It is here, and under test, for the change that resets both.
    #[cfg(test)]
    pub fn restart(&mut self) {
        self.offset = 0.0;
        self.target = 0.0;
        self.content_height = 0.0;
        self.view_height = 0.0;
        self.glide = None;
    }
}

// ---- Who moves them -------------------------------------------------------

/// Every region's glide, keyed by the name the region was built with.
///
/// The shell's half of the policy. A region reports a wheel (see [`region`]),
/// the shell asks this what that wheel should have moved it to, and then asks it
/// again on every frame until nothing is moving. The regions are a map keyed by
/// name rather than a field each because a region *is* a name here -- the same
/// nine names are spread over eight pages, a panel, a dialog and a modal, and no
/// two of the ones that share a glide are ever on screen together.
///
/// A region nobody has moved is not an error and not a special case: it is at
/// rest at the top, which is what [`Glides::anim`] answers for a name it has
/// never seen, and where a wheel that arrives on that name starts from.
#[derive(Debug, Default)]
pub struct Glides {
    regions: std::collections::HashMap<&'static str, Region>,
}

/// One region: its glide, and the number this policy last said it should be at.
#[derive(Debug, Default)]
struct Region {
    anim: ScrollAnim,
    /// The offset the last `scroll_to` asked for. What a later wheel's measured
    /// offset is compared against, to tell this policy's own number from somebody
    /// else's -- see [`Wheel::offset`].
    sent: f32,
}

impl Region {
    /// Take the region over at the offset the wheel measured, if that offset is
    /// not the one this policy last asked for.
    ///
    /// A tolerance rather than an equality, because iced clamps the offset it is
    /// given at the end of the content and because a frame and a wheel event do
    /// not have to fall on the same tick: half a pixel is this policy's own
    /// rounding, and anything past it is a drag, a keyboard scroll or a clamp.
    fn adopt(&mut self, offset: f32) {
        if (offset - self.sent).abs() > SETTLED {
            self.anim.resync(offset);
        }
    }
}

impl Glides {
    /// Take a wheel on `name`, and answer with the command that moves it.
    ///
    /// One command, not a stream: the wheel starts the tween and answers with the
    /// offset it starts from, and every frame after this one is [`Glides::tick`]'s.
    pub fn wheel<Message: 'static>(
        &mut self,
        name: &'static str,
        wheel: Wheel,
        now: Instant,
    ) -> iced::Command<Message> {
        let region = self.regions.entry(name).or_default();
        region.adopt(wheel.offset);
        region.anim.wheel(wheel, now);
        region.sent = region.anim.offset;
        scroll_to(name, region.sent)
    }

    /// Advance every region by one frame, and answer with the commands for the
    /// ones that moved.
    ///
    /// Called from the window's frame timer, which exists only while
    /// [`Glides::animating`] is true -- so this is the last frame of a gesture as
    /// often as not, and the frame that lands an offset exactly on its target is
    /// the one that has to be sent. Hence the comparison rather than the flag
    /// [`ScrollAnim::tick`] returns: what matters here is whether the frame
    /// *changed* the picture, not whether another one is coming.
    pub fn tick<Message: 'static>(&mut self, now: Instant) -> iced::Command<Message> {
        let mut moved = Vec::new();
        for (name, region) in &mut self.regions {
            let name = *name;
            let before = region.anim.offset;
            region.anim.tick(now);
            if region.anim.offset != before {
                region.sent = region.anim.offset;
                moved.push(scroll_to(name, region.sent));
            }
        }
        iced::Command::batch(moved)
    }

    /// Whether any region is still moving, which is what keeps the one frame
    /// timer every region shares alive.
    pub fn animating(&self) -> bool {
        self.regions.values().any(|region| region.anim.animating())
    }

    /// Where one region is, or the resting state for a name nothing has moved.
    ///
    /// What a glide is made of is the region's own business, and the shell only
    /// ever needs the commands; this is how the tests ask a region what it did.
    #[cfg(test)]
    pub fn anim(&self, name: &'static str) -> ScrollAnim {
        self.regions.get(name).map(|region| region.anim).unwrap_or_default()
    }

    /// The offset this policy last put `name` at, or zero for a name nothing has
    /// moved.
    ///
    /// **The offset that was commanded, not a measurement of the widget**, and
    /// that is the honest reading: iced publishes no scrollable's position to the
    /// application except through the wheel report this policy already takes
    /// ([`Wheel::offset`], which is measured from the region's own layout), and a
    /// position that only arrived with a wheel would be a frame behind every
    /// gesture after the first. What this answers is the number the last
    /// [`scroll_to`] carried, so it is where the region is to within the frame a
    /// command takes to land -- and a region nobody has moved is at the top,
    /// which is the resting state the tests read through `anim` as well.
    ///
    /// It exists for one caller: the hosting page's invite toast, which is a
    /// layer of the pane's own [`crate::pages::overlay::Stack`] rather than a
    /// child of the page it is drawn against, and so has to be told how far the
    /// page has been scrolled or it stays where it was while everything around it
    /// moves. See [`crate::shell::Shell::page_overlay`].
    pub(crate) fn offset(&self, name: &'static str) -> f32 {
        self.regions.get(name).map_or(0.0, |region| region.anim.offset)
    }
}

// ---- How many rows a frame draws -----------------------------------------

/// How many rows beyond the visible ones a frame draws, on each side.
///
/// The reference's `bufferSize` in `ui/src/composables/virtual-scroll.ts`, and
/// the same reasoning: a wheel notch lands as a report of where the region *is*,
/// and the frame that draws it is built from that report, so without a margin
/// the edge of the window would be a line the reader can see arriving. Five
/// rows is more than one notch's travel at any row height this interface draws.
pub const OVERSCAN: usize = 5;

/// The fewest rows a frame draws before the scroll region has reported where it
/// is.
///
/// The reference's `initialItemCount`. The floor exists for the row that is
/// tall: an assumption about the height of a window (below) is a row count only
/// once a row height is known, and a list whose rows are taller than that budget
/// still owes the reader a screenful rather than a row and a half.
pub const INITIAL_ROWS: usize = 20;

/// The height of the window assumed before the scroll region has reported its
/// own, in pixels.
///
/// **This is where the port has to differ from the reference, and the reason is
/// that the reference can measure and this cannot.** `useScrollViewport` attaches
/// to the scrolling ancestor on mount and reads `clientHeight` then, which is why
/// twenty items are enough for it: they are one frame's worth, before the ref
/// exists. iced publishes a scrollable's viewport only from an *event* -- wheel,
/// touch, a scrollbar drag -- so a tab the reader has opened and not yet scrolled
/// would draw twenty rows and then a band of nothing on any window taller than
/// that. A height rather than a count is what makes the fallback work for rows of
/// every size this page draws: 4,000px of them is a taller window than this shell
/// can be drawn at, so what it draws always covers the visible slice, and it is
/// still a constant -- a list of five thousand costs what a list of ninety costs,
/// until the first event says exactly what to draw.
pub const INITIAL_VIEW: f32 = 4_000.0;

/// Where a scroll region is, in pixels, as `Scrollable::on_scroll` reports it.
///
/// Plain numbers rather than the widget's own viewport, so the rule below is a
/// function of its inputs and is unit tested without a window -- the same reason
/// [`ScrollAnim`] carries numbers instead of a reference to widget state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Geometry {
    /// Pixels the content has been scrolled by, from its top.
    pub offset: f32,
    /// Height of the part of it that is on screen.
    pub view_height: f32,
}

impl Geometry {
    /// The geometry a scrollable reported.
    ///
    /// `absolute_offset` rather than the relative one: the rows are placed from
    /// the content's own top (see [`crate::pages::instance`]), and a percentage
    /// would have to be turned back into pixels against the same content height
    /// the caller already has.
    pub fn of(viewport: iced::widget::scrollable::Viewport) -> Geometry {
        Geometry {
            offset: viewport.absolute_offset().y,
            view_height: viewport.bounds().height,
        }
    }
}

/// The rows one frame draws, out of a list `len` long whose rows are all
/// `row_height` tall.
///
/// A port of the reference's `visibleRange`, including both of its guards, and
/// the reason this is a *policy* rather than a loop: the rows outside the range
/// are the ones that cost nothing, so what a frame costs stops depending on how
/// long the list is. `crate::scale` is where that is measured at five thousand.
///
/// The two guards are worth naming. A region that has not reported its height
/// yet gets [`INITIAL_VIEW`] rather than "all of them" or "none", and at least
/// [`INITIAL_ROWS`]. A range is never shorter than the window, so at the very end
/// of a list the range slides rather than shrinking -- which is what keeps the
/// count of drawn rows constant while a reader scrolls, and therefore the frame
/// cost constant too.
pub fn window(len: usize, row_height: f32, at: Geometry) -> std::ops::Range<usize> {
    if len == 0 || row_height <= 0.0 {
        return 0..len;
    }
    let view = if at.view_height > 0.0 { at.view_height } else { INITIAL_VIEW };
    let visible = (view / row_height).ceil() as usize;
    let size = (visible + OVERSCAN * 2).max(INITIAL_ROWS);
    // `saturating_sub` for the list shorter than one window: there is no offset
    // that would show `size` rows of it, and `0` is the only range that makes
    // sense.
    let start = ((at.offset / row_height).floor().max(0.0) as usize)
        .saturating_sub(OVERSCAN)
        .min(len.saturating_sub(size));
    start..(start + size).min(len)
}

/// Cubic ease-out: quick to leave, gentle to arrive.
///
/// A scroll is a *position* animation the hand asked for, not a decoration, so
/// it has to look like it is obeying — closing most of the distance early and
/// settling — rather than starting slowly the way an ease-in-out transition
/// would. `progress` is in `0.0..=1.0`.
fn ease_out(progress: f32) -> f32 {
    let remaining = 1.0 - progress;
    1.0 - remaining * remaining * remaining
}

/// One wheel event, with the geometry it happened in.
///
/// The guard reports all of it because it comes from the same place: it is laid
/// out at the content's full height inside the scrollable, the rectangle it is
/// asked to draw in is the visible slice of that content, and the gap between
/// the two is where the region currently is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wheel {
    /// Notches, positive upward.
    pub notches: f32,
    /// Full height of the content, in pixels.
    pub content_height: f32,
    /// Height of the part that is on screen, in pixels.
    pub view_height: f32,
    /// Where the region is right now, in pixels from the content's top.
    ///
    /// **Measured rather than remembered, because the scrollbar is not this
    /// policy.** iced's own scrollbar drag, a keyboard scroll and a `scroll_to`
    /// from anywhere else all move a region without a wheel, and each of them
    /// leaves the policy's idea of the offset behind. A glide started from that
    /// stale offset does not slide from where the reader is looking: it jumps
    /// there first, and then slides.
    ///
    /// What reads this is [`Glides`], and what it compares the number against is
    /// the offset it last *asked* for. The two agree during a glide -- iced
    /// applies what it is told and hands it back in the next event's viewport --
    /// so a wheel during a gesture adopts nothing and its notches accumulate,
    /// while a wheel after a drag adopts the drag. Adopting on every wheel is the
    /// mistake this field exists to avoid: each notch would re-aim at a position
    /// a frame behind the one just commanded, and a flick would travel far less
    /// than the hand asked for.
    pub offset: f32,
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

/// A scroll region whose bar is not drawn.
///
/// The reference's own scroll containers show no bar in either capture -- its
/// stylesheet paints one only under the pointer -- while iced's stock appearance
/// puts a low-alpha thumb at the region's right edge whether or not anybody is
/// there, which in the settings pane is a grey band across the second theme card's
/// corner. Nothing but the bar changes: the wheel still works, and a region's own
/// layout does not read this appearance.
///
/// A site that wants it asks with [`no_bar`]; a site that wants the stock bar
/// omits the call.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoBar;

impl iced::widget::scrollable::StyleSheet for NoBar {
    type Style = iced::Theme;

    fn active(&self, _theme: &iced::Theme) -> iced::widget::scrollable::Appearance {
        iced::widget::scrollable::Appearance {
            container: iced::widget::container::Appearance::default(),
            scrollbar: iced::widget::scrollable::Scrollbar {
                background: None,
                border: iced::Border::default(),
                scroller: iced::widget::scrollable::Scroller {
                    color: iced::Color::TRANSPARENT,
                    border: iced::Border::default(),
                },
            },
            gap: None,
        }
    }

    fn hovered(
        &self,
        theme: &iced::Theme,
        _is_mouse_over_scrollbar: bool,
    ) -> iced::widget::scrollable::Appearance {
        self.active(theme)
    }
}

/// The style a scroll region that wants no bar hands to `Scrollable::style`.
///
/// [`NoBar`] is a stylesheet, while `Scrollable::style` takes the style enum of
/// the theme it draws in, so the conversion lives here rather than at every call
/// site: a site writes `.style(crate::scroll::no_bar())`. A region that wants
/// iced's stock bar omits the call.
pub fn no_bar() -> iced::theme::Scrollable {
    iced::theme::Scrollable::custom(NoBar)
}

/// A scroll region: `content` wrapped so that the wheel over it is reported
/// rather than applied, inside the scrollable that carries the region's id.
///
/// The one call a region's site makes, rather than three that have to agree: the
/// guard reports the region's name, the scrollable answers to it, and both come
/// from the same argument. What the wheel is *not* given to is iced's own
/// handling, and that is the whole reason this exists: a `Scrollable` hands the
/// event to its content first and stands down if the content claims it, so a
/// notch that the guard takes is one iced does not apply as an instant 60-pixel
/// jump. What the caller gets back is the `Scrollable`, so a site can still say
/// its height, its width or its own `on_scroll`.
pub fn region<'a, Message, Theme, Renderer>(
    name: &'static str,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    on_wheel: impl Fn(&'static str, Wheel) -> Message + 'a,
) -> iced::widget::Scrollable<'a, Message, Theme, Renderer>
where
    Message: 'a,
    // `Scrollable` itself is not one of the bounds `Widget` asks for, so it has
    // to be named: the struct is generic over a theme that can draw a scrollbar.
    Theme: iced::widget::scrollable::StyleSheet + 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    iced::widget::scrollable(WheelGuard::new(content, move |wheel| on_wheel(name, wheel))).id(id(name))
}

/// A region's content, guarded, before the scrollable is built around it.
///
/// [`region`] is what a call site uses; this is the wrapper itself, which a
/// caller that needs to place the guarded content in a scrollable of its own
/// (with its own id) can hold.
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
    /// Wrap `content` so that a wheel over it is reported to `on_wheel`.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_wheel: impl Fn(Wheel) -> Message + 'a,
    ) -> Self {
        WheelGuard { content: content.into(), on_wheel: Box::new(on_wheel) }
    }

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
                    // `bounds` is this content laid out from its own top, and
                    // `viewport` is the visible slice of it -- both in the
                    // window's coordinates, because iced positions layout nodes
                    // absolutely. So the distance between their tops is exactly
                    // how far the content has been scrolled, with no state of
                    // this policy in the answer.
                    offset: viewport.y - bounds.y,
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

    /// A wheel of `notches` on a region `content` tall showing `view` of itself,
    /// with the region currently at `offset` -- what a guard reports.
    fn wheel_at(notches: f32, content: f32, view: f32, offset: f32) -> Wheel {
        Wheel { notches, content_height: content, view_height: view, offset }
    }

    /// Run a gesture to completion on a clock the test owns, and report how many
    /// frames it took.
    fn frames_to_settle(anim: &mut ScrollAnim) -> usize {
        let began = Instant::now();
        anim.begin(began);
        let (mut frames, mut now) = (0, began);
        while anim.tick(now) {
            frames += 1;
            now += FRAME;
            assert!(frames < 60, "the animation must terminate");
        }
        frames
    }

    /// A scroll region holding `len` rows of `row_height`, scrolled to `offset`
    /// in a `view_height` window.
    fn at(len: usize, row_height: f32, offset: f32, view_height: f32) -> std::ops::Range<usize> {
        window(len, row_height, Geometry { offset, view_height })
    }

    #[test]
    fn a_window_holds_the_visible_rows_and_a_margin_either_side() {
        // 66px rows in a 600px region: nine and a bit rows are on screen, and
        // nine are drawn beside them -- five above and five below, which is what
        // keeps a partly-scrolled row from arriving as a blank line.
        assert_eq!(at(5_000, 66.0, 0.0, 600.0).len(), 10 + OVERSCAN * 2);
        // At the top the margin cannot go above row zero.
        assert_eq!(at(5_000, 66.0, 0.0, 600.0).start, 0);
        // A hundred rows in: the window has moved with the reader, and its size
        // has not changed.
        let middle = at(5_000, 66.0, 6_600.0, 600.0);
        assert_eq!(middle.start, 100 - OVERSCAN);
        assert_eq!(middle.len(), 10 + OVERSCAN * 2);
    }

    #[test]
    fn the_window_slides_at_the_end_of_the_list_rather_than_shrinking() {
        // The constant size is the point: a range that shrank at the bottom would
        // make the last screenful the cheapest frame and the one before it the
        // most expensive, for no reason a reader could see.
        let end = at(5_000, 66.0, 400_000.0, 600.0);
        assert_eq!(end.start, 5_000 - (10 + OVERSCAN * 2));
        assert_eq!(end.end, 5_000, "there is nothing past the end to draw");
        assert_eq!(end.len(), 10 + OVERSCAN * 2);
    }

    #[test]
    fn a_region_that_has_not_reported_itself_draws_a_window_taller_than_a_window() {
        // Nothing published yet. The whole list would be the cost this rule
        // exists to remove, and twenty rows would be a band of nothing on a tall
        // window at this row height -- so the fallback is a height, and it is
        // more than any window this shell can be drawn in.
        let first = at(5_000, 66.0, 0.0, 0.0);
        assert!(first.len() > INITIAL_ROWS, "{}", first.len());
        assert!(first.len() < 100, "and it is still a window, not the list");
        assert_eq!(first.start, 0, "nothing has been scrolled, so it starts at the top");
        // And a list shorter than that draws all of itself.
        assert_eq!(at(7, 66.0, 0.0, 0.0), 0..7);
        // A row taller than the assumed window still owes a screenful: the floor
        // is what makes the fallback work for rows of every size.
        assert_eq!(at(5_000, 500.0, 0.0, 0.0).len(), INITIAL_ROWS);
    }

    #[test]
    fn a_list_shorter_than_its_window_is_all_of_itself() {
        assert_eq!(at(3, 66.0, 0.0, 600.0), 0..3);
        // A row taller than the region is still one row: `ceil` rounds a part of
        // a row up, and a window of zero rows would draw nothing at all. (The
        // count is the floor's, not the row's -- the floor is above one row.)
        assert!(at(5_000, 900.0, 0.0, 600.0).contains(&0));
        // An empty listing, and a row height of zero, which is the shape a
        // divide by zero would take if this were written as a division.
        assert_eq!(at(0, 66.0, 0.0, 600.0), 0..0);
        assert_eq!(at(9, 0.0, 0.0, 600.0), 0..9);
        // The floor is a floor: a short list is still all of itself, however few
        // rows its region could hold.
        assert_eq!(at(3, 900.0, 0.0, 600.0), 0..3);
    }

    #[test]
    fn the_window_keeps_the_scrollbar_where_it_was_by_never_passing_the_end() {
        // The page draws a spacer above the first row and one below the last, so
        // the content's height is the whole list's whatever is drawn. What this
        // checks is the other half: the range is always inside the list, so the
        // rows that are drawn are always at the offset they claim to be at.
        for offset in [0.0, 1.0, 6_599.0, 6_600.0, 329_999.0, 400_000.0] {
            let range = at(5_000, 66.0, offset, 600.0);
            assert!(range.end <= 5_000 && range.start <= range.end, "{offset}: {range:?}");
            assert_eq!(range.len(), 10 + OVERSCAN * 2, "{offset}: {range:?}");
        }
        // A negative offset is the top: a region cannot report one, and a rule
        // that trusted it would panic on the cast rather than draw.
        assert_eq!(at(5_000, 66.0, -120.0, 600.0).start, 0);
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
        let now = Instant::now();
        anim.wheel(wheel_at(-1.0, 2000.0, 500.0, 0.0), now);
        assert_eq!(anim.target, WHEEL_PIXELS_PER_NOTCH);
        assert_eq!(anim.offset, 0.0, "the offset must ease, not jump");
        anim.wheel(wheel_at(-1.0, 2000.0, 500.0, 0.0), now);
        assert_eq!(anim.target, 2.0 * WHEEL_PIXELS_PER_NOTCH);
    }

    #[test]
    fn the_first_wheel_event_measures_the_page_for_itself() {
        // A page nobody has scrolled has never reported a viewport, so the
        // geometry has to travel with the wheel event or nothing would move.
        let mut anim = ScrollAnim::default();
        assert_eq!(anim.max_offset(), 0.0);
        anim.wheel(wheel_at(-2.0, 3000.0, 700.0, 0.0), Instant::now());
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
        let frames = frames_to_settle(&mut anim);
        assert_eq!(anim.offset, anim.target, "it must land exactly on the target");
        assert!(!anim.animating());
        // About ten frames at 60fps: a fifth of a second, which reads as one
        // movement. The window is wide on purpose -- the property that matters
        // is a bounded gesture, not an exact count -- but a curve that doubled
        // the count would fail here.
        assert!((5..=20).contains(&frames), "settled in {frames} frames");
        // And a settled animation costs nothing: no further frames are asked for.
        assert!(!anim.tick(Instant::now()));
    }

    #[test]
    fn a_long_flick_takes_the_same_time_as_a_short_one() {
        // Easing against a deadline means distance changes how far each frame
        // travels, never how long the gesture lasts. The frame-counted curve
        // this replaced took ten times the frames for a ten times longer flick,
        // which is how one wheel notch became three quarters of a second on a
        // slow renderer.
        let frames = |notches: f32| {
            let mut anim = page(8000.0, 600.0);
            anim.scroll_notches(notches);
            frames_to_settle(&mut anim)
        };
        assert_eq!(
            frames(-2.0),
            frames(-20.0),
            "a 10x flick must not cost ten times the frames"
        );
        assert!(frames(-20.0) <= 20, "a gesture is a fifth of a second at 60fps");
    }

    #[test]
    fn a_gap_between_two_gestures_is_not_a_frame() {
        // The frame timer exists only while something is moving, so the first
        // tick after a gesture begins is separated from the last tick of the one
        // before it by however long the reader spent reading -- seconds, not
        // milliseconds. An earlier revision folded that gap into a frame-cost
        // measurement, which made it a "frame" of four seconds: the probe was
        // wrong about what it was measuring, whatever it did with the number.
        // What this asserts is that the number is not taken at all.
        let mut anim = page(4000.0, 600.0);
        anim.wheel(wheel_at(-1.0, 4000.0, 600.0, 0.0), Instant::now());
        assert!(anim.animating(), "the first gesture of a session must glide");
        let first = frames_to_settle(&mut anim);
        assert!(first > 1, "it glided rather than teleporting: {first} frames");
        // A minute of reading later, the same wheel on the same page.
        anim.wheel(wheel_at(-1.0, 4000.0, 600.0, anim.target), Instant::now());
        assert!(anim.animating(), "the gap between two gestures is not a measurement");
        assert_eq!(
            frames_to_settle(&mut anim),
            first,
            "the second gesture must glide exactly like the first"
        );
    }

    #[test]
    fn a_slow_machine_glides_too_and_lands_on_the_deadline() {
        // The machine the removed demotion existed for: a software rasteriser
        // measured at 69 ms a frame, above the 24 ms the probe called slow, so
        // two frames took its glide away for the rest of the session. What
        // bounds it instead is the deadline -- three steps of 160 ms rather than
        // eleven frames over three quarters of a second -- and three large steps
        // is what the reference draws at ~30 fps (G140).
        let mut anim = page(4000.0, 600.0);
        let began = Instant::now();
        anim.wheel(wheel_at(-1.0, 4000.0, 600.0, 0.0), began);
        assert!(anim.animating(), "a slow machine is still asked to glide");
        let (mut frames, mut now) = (0, began);
        while anim.tick(now) {
            frames += 1;
            now += Duration::from_millis(69);
        }
        assert!((2..=4).contains(&frames), "it moved in steps: {frames} of them");
        assert_eq!(anim.offset, anim.target, "and still landed on the target");
        assert!(!anim.animating());
    }

    #[test]
    fn a_frame_that_arrives_late_lands_the_gesture_rather_than_extending_it() {
        // The other half of a deadline: a machine that stalls mid-gesture does
        // not get a longer animation, it gets the end of this one. There is also
        // nothing left for the stall to be re-learned as.
        let mut anim = page(4000.0, 600.0);
        let began = Instant::now();
        anim.wheel(wheel_at(-1.0, 4000.0, 600.0, 0.0), began);
        assert!(anim.tick(began), "the first frame has the whole glide ahead of it");
        assert!(
            !anim.tick(began + Duration::from_secs(2)),
            "two seconds later the gesture is over"
        );
        assert_eq!(anim.offset, anim.target);
        assert!(!anim.animating());
    }

    #[test]
    fn a_page_opened_later_starts_at_its_top_and_still_glides() {
        let mut anim = page(4000.0, 600.0);
        anim.scroll_notches(-4.0);
        anim.restart();
        // The newly opened page starts at its top...
        assert_eq!(anim.offset, 0.0);
        assert_eq!(anim.target, 0.0);
        assert!(!anim.animating());
        // ...and glides like every other one. Nothing about the machine is
        // carried across a navigation, because nothing about it is measured.
        anim.wheel(wheel_at(-1.0, 4000.0, 600.0, 0.0), Instant::now());
        assert!(frames_to_settle(&mut anim) > 1, "a fresh page must glide too");
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
        anim.begin(Instant::now());
        assert!(anim.animating());
        anim.resync(1200.0);
        assert_eq!(anim.offset, 1200.0);
        assert_eq!(anim.target, 1200.0);
        assert!(!anim.animating(), "a drag ends the glide rather than fighting it");
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
            Some(wheel_at(-1.0, 1200.0, 400.0, 0.0)),
            "the content height, the visible height and the offset must all travel with the event"
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

    #[test]
    fn the_offset_a_wheel_reports_is_the_region_s_position_and_not_this_policy_s() {
        // The content is laid out from the top of the region and the visible
        // slice is translated by the scroll; the gap between the two tops is the
        // offset, read off the event rather than remembered. Scrolled 300px: the
        // slice starts that much below the content.
        let bounds = Rectangle { x: 12.0, y: 40.0, width: 100.0, height: 1200.0 };
        let viewport = Rectangle { x: 12.0, y: 340.0, width: 100.0, height: 400.0 };
        let inside = mouse::Cursor::Available(iced::Point::new(50.0, 100.0));
        let wheel = Event::Mouse(mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
        });
        let claimed = WheelGuard::<(), Theme, iced::Renderer>::claimed(
            &wheel, inside, bounds, &viewport,
        )
        .expect("a wheel over the visible part is claimed");
        assert_eq!(claimed.offset, 300.0);
    }

    // ---- The shell's side of the policy ---------------------------------

    /// Run a wheel on `name`, with the region reported at `at`.
    fn glide(glides: &mut Glides, name: &'static str, at: f32, now: Instant) {
        let _ = glides.wheel::<()>(name, wheel_at(-1.0, 2000.0, 500.0, at), now);
    }

    #[test]
    fn a_wheel_moves_the_region_it_names_and_leaves_the_others_alone() {
        let now = Instant::now();
        let mut glides = Glides::default();
        glide(&mut glides, PAGE, 0.0, now);
        assert_eq!(glides.anim(PAGE).target, WHEEL_PIXELS_PER_NOTCH);
        assert_eq!(glides.anim(PANEL).target, 0.0, "a region nobody wheeled is still at the top");
    }

    #[test]
    fn a_flick_accumulates_the_way_the_hand_asked_for_it() {
        // The case the sent-offset comparison exists for. Every notch is answered
        // before the frame carrying the previous one has been drawn, so the region
        // reports where it was a frame ago -- and a policy that adopted that report
        // would re-aim at it, losing most of the flick.
        let began = Instant::now();
        let mut glides = Glides::default();
        glide(&mut glides, PAGE, 0.0, began);
        let _ = glides.tick::<()>(began + FRAME);
        let sent = glides.anim(PAGE).offset;
        assert!(sent > 0.0 && sent < WHEEL_PIXELS_PER_NOTCH, "one frame of a glide: {sent}");
        // The second notch arrives with the region still reporting the position
        // the first frame asked for, because that is what it is at.
        glide(&mut glides, PAGE, sent, began + FRAME);
        assert_eq!(
            glides.anim(PAGE).target,
            2.0 * WHEEL_PIXELS_PER_NOTCH,
            "two notches must travel two notches"
        );
    }

    #[test]
    fn a_region_something_else_moved_is_taken_over_rather_than_dragged_back() {
        // A scrollbar drag moves the region without a wheel, and iced clamps the
        // end of a shortened page the same way. A glide started from the policy's
        // stale idea of the offset would jump the page there first.
        let now = Instant::now();
        let mut glides = Glides::default();
        glide(&mut glides, PAGE, 0.0, now);
        assert_eq!(glides.anim(PAGE).offset, 0.0);
        glide(&mut glides, PAGE, 900.0, now);
        assert_eq!(glides.anim(PAGE).offset, 900.0, "the drag is where the glide starts");
        assert_eq!(
            glides.anim(PAGE).target,
            900.0 + WHEEL_PIXELS_PER_NOTCH,
            "and the notch is measured from there"
        );
    }

    #[test]
    fn the_regions_stop_asking_for_frames_once_they_have_arrived() {
        // What keeps the frame timer from running forever: `animating` is what the
        // shell's subscription asks, so a gesture that has landed must say so.
        let began = Instant::now();
        let mut glides = Glides::default();
        glide(&mut glides, PAGE, 0.0, began);
        assert!(glides.animating(), "a wheel that can move a region starts the frame timer");
        let (mut frames, mut now) = (0, began);
        while glides.animating() {
            let _ = glides.tick::<()>(now);
            now += FRAME;
            frames += 1;
            assert!(frames < 60, "the gesture must terminate");
        }
        assert!(frames > 1, "it glided rather than arriving in one step: {frames}");
        assert_eq!(glides.anim(PAGE).offset, WHEEL_PIXELS_PER_NOTCH, "exactly on the target");
    }

    #[test]
    fn a_wheel_on_a_region_with_nothing_to_scroll_starts_no_timer() {
        // A page shorter than its window clamps the target back to the top, so the
        // wheel moves nothing and must not wake a clock for 160ms.
        let mut glides = Glides::default();
        let _ = glides.wheel::<()>(PAGE, wheel_at(-1.0, 400.0, 600.0, 0.0), Instant::now());
        assert!(!glides.animating());
    }

    #[test]
    fn an_overlay_can_ask_where_the_page_under_it_is() {
        // The hosting page's toast is a layer of the *pane's* stack, so the one
        // number it cannot see for itself is how far the page has been scrolled.
        // What it must read is the offset the policy is at, frame by frame -- a
        // toast that read the wheel's own report instead would sit still through
        // the glide and then jump.
        let began = Instant::now();
        let mut glides = Glides::default();
        assert_eq!(glides.offset(PAGE), 0.0, "a region nobody has moved is at the top");
        glide(&mut glides, PAGE, 0.0, began);
        assert_eq!(glides.offset(PAGE), 0.0, "and the frame the wheel lands in has not moved it");
        let mut seen = Vec::new();
        let mut now = began;
        while glides.animating() {
            let _ = glides.tick::<()>(now);
            now += FRAME;
            seen.push(glides.offset(PAGE));
        }
        assert!(
            seen.windows(2).all(|pair| pair[1] > pair[0]),
            "every frame of a glide moves the overlay with it: {seen:?}"
        );
        assert_eq!(glides.offset(PAGE), WHEEL_PIXELS_PER_NOTCH, "and it lands on the target");
    }
}
