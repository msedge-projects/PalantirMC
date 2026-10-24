//! Scroll policy for the desktop shell: how much to draw, and how to move.
//!
//! iced 0.12's `Scrollable` consumes wheel events itself, and this module takes
//! them instead so the shell knows which named region moved and how far ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â a page
//! and the panel beside it, and up to nine regions on one screen.
//!
//! **A wheel moves the page at once.** There is no tween, no deadline and no frame
//! timer: [`ScrollAnim::wheel`] adds the notches to the position and
//! [`Glides::wheel`] answers with the one `scroll_to` that carries it. Two attempts
//! to make a tween feel fast ("the scrolling is still laggy", then "make it
//! lightweight and smooth") ended when it was measured as what it was ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â a gesture
//! spending frames travelling to a position the input had already named, with a
//! frame timer running while it did. A browser scrolls a wheel where the wheel says,
//! on the event; so does this now, and what is left of the policy is the part that
//! has to be right: which region, how far, and whether the offset a wheel reports is
//! this shell's own number or somebody else's (see [`Swept`]).
//!
//! What stays is the drawing half: the windowed list and the log cap, which decide
//! how many rows a long page hands the renderer at all.
//!
//! Deliberate limits, because each one is a way this can go wrong:
//!
//! * **Only the wheel is taken.** The scrollbar, dragging it, keyboard
//!   scrolling, touch and iced's own `scroll_to` (which the log page's
//!   autoscroll uses) all still work, because this does not replace
//!   `Scrollable` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â it sits inside one.
//! * **No timer at all.** Scrolling schedules no frame and owns no subscription, so
//!   an idle window and a scrolling one cost the same per frame: one redraw.
//! * **One command per event.** Where the wheel says, on the event. There is no
//!   state carried between events, so nothing can lag behind the input.

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
/// change how far a notch travels ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â only how it gets there.
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

/// Every region this shell has, in the order they were added.
///
/// One list because a region is two things at once -- an id on a widget and a key
/// in the shell's glide table -- and a name only one side knows is a region that
/// reports to nothing. The gate below holds three facts that were previously only
/// true by inspection: the nine names are distinct, each is a `scroll:` id, and
/// each one appears at a call site in this crate.
pub const REGIONS: [&str; 9] = [
    PAGE,
    CONTENT,
    PANEL,
    DIALOG,
    VERSIONS,
    BUILDS,
    GAME_VERSIONS,
    LOADER_BUILDS,
    PACK_VERSIONS,
];

/// The id a region's scrollable answers to, and the one the shell moves.
pub fn id(name: &'static str) -> iced::widget::scrollable::Id {
    iced::widget::scrollable::Id::new(name)
}

/// A command that puts the region `name` where the policy says it is.
///
/// `scroll_to` is the only way a program moves a `Scrollable` in iced -- the
/// widget owns its offset, and there is no setter -- so this is what a wheel's
/// answer is made of: one of these per event, carrying the whole of the notch. The x
/// offset is zero because every region here scrolls vertically; the content is as
/// wide as the region.
pub fn scroll_to<Message: 'static>(name: &'static str, offset: f32) -> iced::Command<Message> {
    iced::widget::scrollable::scroll_to(
        id(name),
        iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: offset },
    )
}







/// Below this many pixels from the target, the animation is over.
///
/// Without a floor, a re-clamped target would leave the page a fraction of a
/// pixel from where it belongs and keep asking for frames.
pub const SETTLED: f32 = 0.5;

/// How long a glide takes.
///
/// 150 ms: the band egui's own smooth scrolling uses (200 ms, which fastframe
/// helps toward) and short enough that a notch is done before the reader can
/// wonder whether it is still moving. This bounds a gesture. What makes the frames
/// that draw it cheap is `crate::icon`'s canvas cache, and until that landed they
/// were expensive enough that any animation read as a slideshow.
pub const DURATION: Duration = Duration::from_millis(150);

/// Ease-out over `progress` in 0..=1.
///
/// Cubic: three quarters of the distance in the first half of the time, which is
/// what makes the *end* of a gesture feel settled rather than abrupt. A linear
/// curve arrives, but does not look like it has.
fn ease_out(progress: f32) -> f32 {
    let inverse = 1.0 - progress.clamp(0.0, 1.0);
    1.0 - inverse * inverse * inverse
}

/// Where a page's scroll position is, and where it is going.
///
/// An eased move towards `target`, drawn by frames the shell asks for only while
/// one is running. The cost of such a frame is what decides whether the easing
/// reads as smoothness or as a slideshow, and that cost is `crate::icon`'s.
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

    /// Whether the offset is still moving, which decides whether the shell asks
    /// for a frame at all.
    pub fn animating(&self) -> bool {
        self.glide.is_some() || (self.target - self.offset).abs() > SETTLED
    }

    /// Take a wheel event, with the geometry it was reported in.
    ///
    /// The geometry comes from the same event rather than from a separate report,
    /// because iced only publishes viewport changes: a page nobody has scrolled
    /// yet has never reported one, and a wheel on it would otherwise move nothing.
    ///
    /// `now` is passed in rather than read here so that the whole policy is a
    /// function of its inputs, and a test can hold the clock still.
    pub fn wheel(&mut self, wheel: Wheel, now: Instant) {
        // Input arrives between frames. Sample the glide at the event's time
        // before retargeting, rather than starting from a stale frame.
        self.tick(now);
        self.observe(wheel.content_height, wheel.view_height);
        self.scroll_notches(wheel.notches);
        self.begin(now);
    }

    /// Move the target by `notches`, positive upward, and clamp it to the content.
    ///
    /// The offset follows in [`ScrollAnim::tick`]: the target is where the wheel
    /// asked for, the offset is where the page is.
    pub fn scroll_notches(&mut self, notches: f32) {
        self.target = self.clamp(self.target - notches * WHEEL_PIXELS_PER_NOTCH);
    }

    /// Start easing toward the target, unless a glide is already carrying one.
    ///
    /// **A gesture in flight keeps its clock.** Resetting `from` and `began` on
    /// every event -- which an earlier revision did -- makes a burst arriving faster
    /// than one event per [`DURATION`] impossible to finish: the page stays a fixed
    /// fraction of a second behind the wheel and settles only when the reader stops.
    fn begin(&mut self, now: Instant) {
        if self.glide.is_some() {
            return;
        }
        if !self.animating() {
            // Already there: a wheel clamped out of range at the end of a page must
            // not start a timer for a gesture that moves nothing.
            return;
        }
        self.glide = Some(Glide {
            from: self.offset,
            began: now,
            duration: DURATION,
        });
    }

    /// Advance to `now` and report whether there is more to animate.
    ///
    /// The position is a function of elapsed time against the glide's deadline, so a
    /// machine answering slowly draws fewer, larger steps and still finishes on
    /// time. Nothing here can make a gesture last longer than [`DURATION`].
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some(glide) = self.glide else {
            self.offset = self.target;
            return false;
        };
        let elapsed = now.saturating_duration_since(glide.began).as_secs_f32();
        let deadline = glide.duration.as_secs_f32();
        let progress = if deadline > 0.0 {
            (elapsed / deadline).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.offset = glide.from + (self.target - glide.from) * ease_out(progress);
        if progress >= 1.0 {
            // Land exactly on the target rather than asymptotically near it.
            self.offset = self.target;
            self.glide = None;
            return false;
        }
        true
    }

    /// Adopt an offset that came from somewhere else -- the scrollbar, a keyboard
    /// scroll, or `scroll_to` -- without fighting it.
    ///
    /// Both the offset and the target move, so the next notch adds to where the page
    /// really is, and the glide is dropped because the hand is somewhere else now.
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

/// One region: where it is, and the number this policy last said it should be at.
#[derive(Debug, Default)]
struct Region {
    anim: ScrollAnim,
    /// The offset the last `scroll_to` asked for, and what a later wheel's
    /// measured offset is compared against through [`Region::swept`] -- see
    /// [`Wheel::offset`].
    sent: f32,
    /// Every offset this policy has put the region at since it last measured it.
    swept: Swept,
}

/// The offsets a region's own commands can account for.
///
/// **A range rather than a point, because one command is not one measurement.**
/// `scroll_to` is an `AbsoluteOffset` (iced_widget-0.12.3's
/// `operation::scrollable::scroll_to`, applied at `State::scroll_to`), so the
/// command that lands *last* is the only one the widget ever sees ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â the ones
/// before it are overwritten, not composed. But a wheel event does not read the
/// widget: [`Wheel::offset`] is `viewport.y - bounds.y` off the *cached layout*
/// (scroll.rs's `WheelGuard::claimed`), and iced_winit only rebuilds that at the
/// end of its `AboutToWait` branch, after the messages in the batch have run.
/// A `Message::Tick` arrives as a winit `UserEvent` and is pushed straight into
/// the message queue (`iced_winit-0.12.2/src/application.rs:366-368`) while a
/// wheel goes through `user_interface.update` first (`application.rs:494-499`),
/// and the queue is then drained in arrival order (`application.rs:637-650`).
/// So a batch arriving as `[Tick, Wheel]` runs the tick's `scroll_to` and *then*
/// hands `Glides::wheel` a measurement taken before it.
///
/// Compared as a single number against the newest command, that measurement is a
/// whole frame of glide behind ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â tens of pixels, well past [`SETTLED`] ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â and reads
/// exactly like a scrollbar drag. [`Region::adopt`] then resyncs onto it, which
/// resets `ScrollAnim::target` to a position the reader never asked for and throws
/// away every notch accumulated since the gesture began; the frame answers
/// `scroll_to` with the stale offset as well, so the region visibly jumps back.
/// How many ticks get interleaved depends on the clock thread and on X
/// round-trip timing, which is why the same gesture travelled a different distance
/// on every run: 285, 272, 162, 165, 540, 416 and 415 pixels were measured from
/// the same binary for six, ten and twenty notches, where the arithmetic says
/// 360, 600 and 1200.
///
/// Recording the whole range each command swept makes that case
/// indistinguishable from the region being where it was told, which is what it
/// is, and it holds however many commands are outstanding rather than guessing a
/// depth. Widening a range is also what makes a repeated command mean the same
/// thing twice: `Region::send` only ever grows it, so re-issuing the last
/// `scroll_to` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â or issuing another while it is still in flight ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â can no longer
/// read as an outsider's move.
///
/// The `Default` is `Swept::at(0.0)` rather than a range of nothing: a region
/// nobody has commanded *is* at the top, and `Region::default` is what a name
/// this policy has never seen resolves to.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Swept {
    lo: f32,
    hi: f32,
}

impl Swept {
    /// A region nobody has commanded is at the top, which is where it starts.
    fn at(offset: f32) -> Self {
        Swept {
            lo: offset,
            hi: offset,
        }
    }

    /// Add an offset a command has put the region at.
    fn include(&mut self, offset: f32) {
        self.lo = self.lo.min(offset);
        self.hi = self.hi.max(offset);
    }

    /// Whether `offset` is one this policy could have put the region at.
    ///
    /// A tolerance rather than an equality, because iced clamps the offset it is
    /// given at the end of the content and because a frame and a wheel event do
    /// not have to fall on the same tick: half a pixel is this policy's own
    /// rounding, and anything past it is a drag, a keyboard scroll or a clamp.
    fn holds(&self, offset: f32) -> bool {
        offset >= self.lo - SETTLED && offset <= self.hi + SETTLED
    }
}

impl Region {
    /// Take the region over at the offset the wheel measured, unless this policy
    /// could have put it there itself -- which is the whole of what is left, once
    /// [`Region::swept`] has answered for the commands still in flight.
    ///
    /// The cost is a drag that lands *inside* the swept range, which is not
    /// adopted. That range is at most one frame of a glide wide and exists only
    /// while a gesture is running or has just finished, against the point-wide
    /// neighbourhood this replaced; a drag into it is left to the next wheel,
    /// which re-bases off whatever the region is at by then.
    fn adopt(&mut self, offset: f32) {
        if self.swept.holds(offset) {
            return;
        }
        self.anim.resync(offset);
        self.swept = Swept::at(offset);
    }

    /// Record that a `scroll_to` for `offset` has been issued, and answer with the
    /// offset that command carries.
    ///
    /// Both writers of `sent` go through here, so a commanded offset can never be
    /// recorded without widening [`Region::swept`] to cover it -- which is the
    /// whole reason a stale measurement is not read as somebody else's move.
    fn send(&mut self, offset: f32) -> f32 {
        self.sent = offset;
        self.swept.include(offset);
        self.sent
    }
}

impl Glides {
    /// Take a wheel on `name`, and answer with the command that moves it.
    ///
    /// One command, not a stream: the wheel moves the region and answers with the
    /// offset it moved it to.
    ///
    /// The notches go onto the **target**, not onto the measured offset, and that
    /// is deliberate rather than incidental: a burst of wheel events is delivered
    /// in one batch against one cached layout, so every event in the burst reports
    /// the same position. Accumulating on the target is the only way two events
    /// travel two notches. [`Region::adopt`] is what has to keep that accumulation
    /// -- see [`Swept`], and what happened when it did not.
    pub fn wheel<Message: 'static>(
        &mut self,
        name: &'static str,
        wheel: Wheel,
        now: Instant,
    ) -> iced::Command<Message> {
        let region = self.regions.entry(name).or_default();
        region.adopt(wheel.offset);
        region.anim.wheel(wheel, now);
        // One command per event, deliberately. A first attempt sent only the
        // event that started a glide and let the frame timer carry the rest, which
        // halved the commands -- and lost notches, because [`Region::adopt`] reads
        // the *commanded* offset as the proof that the region has not been moved by
        // somebody else. A command that is never sent is an offset the next wheel
        // cannot recognise, so the policy re-synced to a stale number and dropped
        // the notches in flight. `however_the_frames_and_the_wheel_events_interleave`
        // is the test that says so. What makes a burst cheap is the glide below,
        // not the count of commands: a notch is over in one frame now, so the offset
        // a command carries has already converged instead of trailing the wheel.
        let offset = region.send(region.anim.offset);
        scroll_to(name, offset)
    }

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
                let offset = region.send(region.anim.offset);
                moved.push(scroll_to(name, offset));
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
    /// ever needs the commands; this is how the tests ask a region what it did.
    #[cfg(test)]
    pub fn anim(&self, name: &'static str) -> ScrollAnim {
        self.regions
            .get(name)
            .map(|region| region.anim)
            .unwrap_or_default()
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
        self.regions
            .get(name)
            .map_or(0.0, |region| region.anim.offset)
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
    let view = if at.view_height > 0.0 {
        at.view_height
    } else {
        INITIAL_VIEW
    };
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
    /// the offsets it has *asked* for -- through [`Swept`], which is a range and
    /// not a point, because the measurement can be a frame behind a command that
    /// has already been issued. The two agree while a glide is in flight, so a
    /// wheel during a gesture adopts nothing and its notches accumulate, and a
    /// wheel after a drag adopts the drag. Adopting on every wheel is the mistake
    /// this field exists to avoid: each notch would re-aim at a position a frame
    /// behind the one just commanded, and a flick would travel far less than the
    /// hand asked for.
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
    iced::widget::scrollable(WheelGuard::new(content, move |wheel| on_wheel(name, wheel)))
        .id(id(name))
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
        WheelGuard {
            content: content.into(),
            on_wheel: Box::new(on_wheel),
        }
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
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.is_over(bounds) && wheel_notches(delta) != 0.0 =>
            {
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
        self.content
            .as_widget()
            .operate(tree, layout, renderer, operation);
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

    /// The nine regions are distinct, are ids, and are each drawn somewhere.
    ///
    /// The gate for what used to be an inspection: a region name is an id on a
    /// widget *and* a key in the shell's glide table, so a name nothing puts on a
    /// widget is a wheel that can never reach a glide, and two names that collide
    /// are one region drawn twice. The call sites are read from the five files
    /// that own them rather than from the whole crate, because a scan that
    /// matched `scroll:` strings would also match these names' own definitions.
    #[test]
    fn the_regions_are_nine_distinct_ids_and_every_one_is_drawn() {
        let mut seen = std::collections::BTreeSet::new();
        for name in REGIONS {
            assert!(seen.insert(name), "{name} is listed twice");
            assert!(
                name.starts_with("scroll:"),
                "{name} is not shaped like a region id"
            );
        }
        assert_eq!(seen.len(), 9, "the shell has nine regions");

        let sources = [
            include_str!("page.rs"),
            include_str!("shell.rs"),
            include_str!("instance_settings.rs"),
            include_str!("pages/discover.rs"),
            include_str!("pages/instance.rs"),
        ];
        // The constant each name is declared as, in `REGIONS` order: the gate is
        // about the spelling a call site uses, and `REGIONS` holds the values.
        let idents = [
            "PAGE",
            "CONTENT",
            "PANEL",
            "DIALOG",
            "VERSIONS",
            "BUILDS",
            "GAME_VERSIONS",
            "LOADER_BUILDS",
            "PACK_VERSIONS",
        ];
        for (name, ident) in REGIONS.iter().zip(idents) {
            let needle = format!("scroll::{ident}");
            assert!(
                sources.iter().any(|source| source.contains(&needle)),
                "{name} is in the table but no call site draws it"
            );
        }
    }

    fn page(content: f32, view: f32) -> ScrollAnim {
        let mut anim = ScrollAnim::default();
        anim.observe(content, view);
        anim
    }

    /// A wheel of `notches` on a region `content` tall showing `view` of itself,
    /// with the region currently at `offset` -- what a guard reports.
    fn wheel_at(notches: f32, content: f32, view: f32, offset: f32) -> Wheel {
        Wheel {
            notches,
            content_height: content,
            view_height: view,
            offset,
        }
    }
    /// A scroll region holding `len` rows of `row_height`, scrolled to `offset`
    /// in a `view_height` window.
    fn at(len: usize, row_height: f32, offset: f32, view_height: f32) -> std::ops::Range<usize> {
        window(
            len,
            row_height,
            Geometry {
                offset,
                view_height,
            },
        )
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
    fn content_that_shrinks_pulls_the_page_back_into_range() {
        let mut anim = page(4000.0, 600.0);
        anim.scroll_notches(-60.0);
        assert_eq!(anim.target, 3400.0);
        // The list got shorter while the page was scrolled to the bottom.
        anim.observe(900.0, 600.0);
        assert_eq!(
            anim.target, 300.0,
            "the target must be re-clamped, not left in space"
        );
    }

    #[test]
    fn a_wheel_delta_is_measured_in_notches() {
        // Windows sends notches ("lines"); touchpads send pixels. Both have to
        // arrive as the same unit or a touchpad would fly and a mouse would
        // crawl.
        let line = iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 };
        let pixel = iced::mouse::ScrollDelta::Pixels {
            x: 0.0,
            y: -WHEEL_PIXELS_PER_NOTCH,
        };
        assert_eq!(wheel_notches(&line), -1.0);
        assert_eq!(wheel_notches(&pixel), -1.0);
        let fractional = iced::mouse::ScrollDelta::Lines { x: 0.0, y: 0.5 };
        assert_eq!(wheel_notches(&fractional), 0.5);
    }

    #[test]
    fn only_a_wheel_over_the_content_is_claimed() {
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 1200.0,
        };
        // The visible slice: the scrollable's own height, at this offset.
        let viewport = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 400.0,
        };
        let inside = mouse::Cursor::Available(iced::Point::new(50.0, 50.0));
        let outside = mouse::Cursor::Available(iced::Point::new(150.0, 50.0));
        let wheel = Event::Mouse(mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
        });

        let claimed =
            WheelGuard::<(), Theme, iced::Renderer>::claimed(&wheel, inside, bounds, &viewport);
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
    fn horizontal_and_empty_wheel_events_do_not_restart_vertical_glides() {
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 1200.0,
        };
        let viewport = Rectangle {
            height: 400.0,
            ..bounds
        };
        let cursor = mouse::Cursor::Available(iced::Point::new(50.0, 50.0));
        for delta in [
            mouse::ScrollDelta::Pixels { x: 20.0, y: 0.0 },
            mouse::ScrollDelta::Lines { x: 0.0, y: 0.0 },
        ] {
            let event = Event::Mouse(mouse::Event::WheelScrolled { delta });
            assert!(WheelGuard::<(), Theme, iced::Renderer>::claimed(
                &event, cursor, bounds, &viewport,
            )
            .is_none());
        }
    }

    /// A notch eases over the glide and lands exactly on its deadline.
    ///
    /// The contract this replaces asserted that a notch landed on its own event --
    /// which is what the reader then described as "moves the page like in an
    /// instant, feels like 10 fps". What makes an eased notch affordable is the
    /// icon cache; what makes it *finish* is the deadline.
    #[test]
    fn a_notch_eases_and_lands_on_its_deadline() {
        let mut anim = page(4_000.0, 600.0);
        let began = Instant::now();
        anim.wheel(wheel_at(-1.0, 4_000.0, 600.0, 0.0), began);
        assert_eq!(anim.offset, 0.0, "the notch starts from where the page is");
        assert_eq!(anim.target, WHEEL_PIXELS_PER_NOTCH);
        assert!(anim.animating(), "and it is travelling");
        // Half way is somewhere between, judged against the eased curve rather
        // than the clock: what matters is that it moves before the deadline.
        anim.tick(began + DURATION / 3);
        assert!(
            anim.offset > 0.0 && anim.offset < WHEEL_PIXELS_PER_NOTCH,
            "part way at a third of the deadline: {}",
            anim.offset
        );
        // And on the deadline it is exactly there, with nothing left to animate.
        assert!(!anim.tick(began + DURATION), "the glide ends on its deadline");
        assert_eq!(anim.offset, WHEEL_PIXELS_PER_NOTCH);
    }

    /// Every notch a burst asks for lands, whatever offset each event reported.
    ///
    /// The accumulation-on-the-target rule: a batch against one cached layout
    /// reports the same position to every event, so two events must travel two
    /// notches. The glide is what carries them; the target is what they add to.
    #[test]
    fn however_many_events_arrive_the_notches_all_land() {
        let began = Instant::now();
        let mut glides = Glides::default();
        let mut stale = 0.0;
        for notch in 1..=6 {
            for _ in 0..2 {
                let _ = glides.wheel::<()>(
                    PAGE,
                    wheel_at(-1.0, 2_000.0, 500.0, stale),
                    began + Duration::from_millis(10 * notch as u64),
                );
            }
            assert_eq!(
                glides.anim(PAGE).target,
                (notch * 2) as f32 * WHEEL_PIXELS_PER_NOTCH,
                "{notch} clicks ask for the distance the wheel asked for"
            );
            stale = glides.offset(PAGE);
        }
        // One frame past the burst's deadline: every notch has arrived.
        let _ = glides.tick::<()>(began + DURATION + Duration::from_millis(20));
        assert_eq!(
            glides.anim(PAGE).offset,
            12.0 * WHEEL_PIXELS_PER_NOTCH,
            "and a dozen notches land"
        );
    }

    /// The region a wheel names moves, and the others do not.
    #[test]
    fn a_wheel_moves_the_region_it_names_and_leaves_the_others_alone() {
        let now = Instant::now();
        let mut glides = Glides::default();
        let _ = glides.wheel::<()>(PAGE, wheel_at(-1.0, 2_000.0, 500.0, 0.0), now);
        assert_eq!(glides.anim(PAGE).target, WHEEL_PIXELS_PER_NOTCH);
        assert_eq!(glides.anim(PANEL).target, 0.0, "the panel did not move");
        assert_eq!(glides.offset(PANEL), 0.0, "and nothing was asked of it");
    }

    /// A region a scrollbar or a key moved is adopted, not fought: the wheel adds
    /// to where the page really is.
    #[test]
    fn a_region_something_else_moved_is_taken_over_rather_than_dragged_back() {
        let now = Instant::now();
        let mut glides = Glides::default();
        // Somebody dragged the scrollbar to 300, and the wheel reports that.
        let _ = glides.wheel::<()>(PAGE, wheel_at(-1.0, 2_000.0, 500.0, 300.0), now);
        assert_eq!(
            glides.anim(PAGE).target,
            300.0 + WHEEL_PIXELS_PER_NOTCH,
            "the notch is added to where the page is, not to zero"
        );
        assert_eq!(glides.anim(PAGE).offset, 300.0, "and it starts from there");
    }

    /// A wheel at the end of the content moves nothing and asks for no frames.
    #[test]
    fn a_wheel_on_a_region_with_nothing_to_scroll_moves_nothing() {
        let now = Instant::now();
        let mut glides = Glides::default();
        // Content shorter than the window: the maximum offset is zero.
        let _ = glides.wheel::<()>(PAGE, wheel_at(-3.0, 400.0, 600.0, 0.0), now);
        assert_eq!(glides.anim(PAGE).offset, 0.0);
        assert_eq!(glides.anim(PAGE).target, 0.0);
        assert!(!glides.animating(), "so no frame timer starts");
    }
}
