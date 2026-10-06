//! The Win32 window behaviour iced 0.12 does not expose.
//!
//! Three things the shell needs and cannot get from iced:
//!
//! * **Resizing.** iced wraps winit's `drag_window` as `window::drag`, but
//!   there is no equivalent of `drag_resize_window` — the call that starts an
//!   OS resize loop. Without it an undecorated window cannot be resized from
//!   its edges at all. [`start_resize`] performs the same two steps winit uses
//!   internally (`ReleaseCapture`, then a `WM_NCLBUTTONDOWN` carrying the
//!   hit-test code for the grabbed edge), which is what puts Windows' own
//!   modal resize loop in charge.
//! * **The screen it has to fit on.** iced can resize a window but cannot say
//!   how big the monitor is, so [`primary_work_area`] asks Windows directly.
//!   This is the fix for a launcher that opened at 1280x820 on a 1366x768
//!   display, putting its own status bar and the bottom of its sidebar past
//!   the bottom edge where they could not be seen or grabbed.
//! * **A real window frame.** A window created without decorations has no
//!   non-client area at all, so Windows answers `HTCLIENT` for every pixel of
//!   it and the frame has to be rebuilt inside the client area. [`install_hit_test`]
//!   hands it back: the shim answers `WM_NCHITTEST` so Windows runs its own
//!   resize loop with its own cursors, and so the maximize button becomes the
//!   non-client region Windows 11 needs before it will offer Snap Layouts.
//!
//! Everything here is a no-op off Windows. The parts that are actually logic —
//! [`ResizeEdge`], [`fit_to_work_area`], [`hit_code`] and [`screen_point`] —
//! carry no `cfg` and are unit tested everywhere, so the only untested code is
//! the FFI call itself.

#[cfg(test)]
use iced::mouse;
#[cfg(windows)]
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::Mutex;

/// Thickness of the invisible grab band drawn around the window edge.
///
/// Wide enough to hit without aiming, narrow enough that it never eats a click
/// meant for the content. Windows' own frame is 4px; 6 matches the feel of a
/// desktop app without stealing anything.
///
/// This is the one number that has to agree in two places — the answer
/// [`hit_code`] gives Windows and the width of the bands the shell paints — so
/// the shell draws its bands from this constant rather than one of its own.
pub const RESIZE_BAND: f32 = 6.0;

/// Which edge or corner of the window the pointer grabbed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl ResizeEdge {
    /// Every edge and corner.
    #[cfg(test)]
    pub const ALL: [ResizeEdge; 8] = [
        ResizeEdge::NorthWest,
        ResizeEdge::North,
        ResizeEdge::NorthEast,
        ResizeEdge::East,
        ResizeEdge::SouthEast,
        ResizeEdge::South,
        ResizeEdge::SouthWest,
        ResizeEdge::West,
    ];

    /// A stable key for tests and tooltips.
    #[cfg(test)]
    pub const fn label(self) -> &'static str {
        match self {
            ResizeEdge::North => "north",
            ResizeEdge::South => "south",
            ResizeEdge::East => "east",
            ResizeEdge::West => "west",
            ResizeEdge::NorthEast => "north-east",
            ResizeEdge::NorthWest => "north-west",
            ResizeEdge::SouthEast => "south-east",
            ResizeEdge::SouthWest => "south-west",
        }
    }

    /// The `HT*` hit-test code Windows wants for this edge.
    ///
    /// The numbers are spelled out rather than imported so the mapping stays
    /// testable off Windows; the values are the ones in
    /// `Win32::UI::WindowsAndMessaging` (HTLEFT 10 … HTBOTTOMRIGHT 17).
    pub const fn hit_code(self) -> u32 {
        match self {
            ResizeEdge::West => 10,
            ResizeEdge::East => 11,
            ResizeEdge::North => 12,
            ResizeEdge::NorthWest => 13,
            ResizeEdge::NorthEast => 14,
            ResizeEdge::South => 15,
            ResizeEdge::SouthWest => 16,
            ResizeEdge::SouthEast => 17,
        }
    }

    /// The cursor to show while the pointer is in this band.
    ///
    /// iced has no diagonal resize cursors, so corners borrow the axis they
    /// mostly move along rather than showing nothing.
    #[cfg(test)]
    pub const fn interaction(self) -> mouse::Interaction {
        match self {
            ResizeEdge::North | ResizeEdge::South => mouse::Interaction::ResizingVertically,
            ResizeEdge::East | ResizeEdge::West => mouse::Interaction::ResizingHorizontally,
            ResizeEdge::NorthEast | ResizeEdge::SouthWest => mouse::Interaction::ResizingHorizontally,
            ResizeEdge::NorthWest | ResizeEdge::SouthEast => mouse::Interaction::ResizingVertically,
        }
    }
}

/// A monitor's work area, in the same logical units iced sizes windows in —
/// that is, with the display's DPI scaling already divided out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Shrink `desired` to fit `fraction` of the work area, never below `minimum`.
///
/// The window should open at the size it wants, unless the screen is too small
/// for that — then it opens as large as the screen comfortably allows. A
/// monitor smaller than the app's own floor is the one case where `minimum`
/// wins, because a window too cramped to use is worse than one that overflows.
pub fn fit_to_work_area(
    desired: (f32, f32),
    work: Option<WorkArea>,
    minimum: (f32, f32),
    fraction: f32,
) -> (f32, f32) {
    let Some(work) = work else {
        return desired;
    };
    let cap_width = (work.width * fraction).max(minimum.0);
    let cap_height = (work.height * fraction).max(minimum.1);
    (
        desired.0.min(cap_width).max(minimum.0),
        desired.1.min(cap_height).max(minimum.1),
    )
}

/// How much the primary display scales what the shell draws, where 1.0 is
/// 96 dpi.
///
/// Read by the screenshot pass, which generates thumbnails at the size they
/// will actually be painted: the same 268-pixel tile needs 268 pixels of source
/// on a 100% display, 335 on the 125% scaling Windows offers by default on many
/// laptops, and 402 at 150%. Generating one fixed size for every machine spends
/// too much on the common case and is soft on the others.
///
/// Deliberately the same query `primary_work_area` uses, so the size the window
/// opens at and the size the thumbnails are made for cannot disagree.
#[cfg(windows)]
pub fn system_scale_factor() -> f32 {
    use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;

    // SAFETY: no arguments and no pointers; the call only reads the process's
    // own DPI awareness and the primary monitor's DPI.
    let dpi = unsafe { GetDpiForSystem() };
    if dpi == 0 {
        1.0
    } else {
        dpi as f32 / 96.0
    }
}

/// Off Windows there is no system DPI to ask for; iced's winit backend scales
/// on its own and the thumbnail pass gets the 1.0 baseline.
#[cfg(not(windows))]
pub fn system_scale_factor() -> f32 {
    1.0
}

/// The primary monitor's work area, or `None` when it cannot be read.
#[cfg(windows)]
pub fn primary_work_area() -> Option<WorkArea> {
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;

    // windows-sys structs carry no `Default`, so the outgoing struct is built
    // field by field. `cbSize` is what the call validates first.
    let empty = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let mut info = MONITORINFO {
        cbSize: core::mem::size_of::<MONITORINFO>() as u32,
        rcMonitor: empty,
        rcWork: empty,
        dwFlags: 0,
    };

    // SAFETY: `info` is a correctly sized MONITORINFO and stays borrowed for
    // the length of each call; no pointer outlives its referent.
    let monitor = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    if monitor == 0 {
        return None;
    }
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }

    // iced's `window::Settings::size` is in logical pixels, so physical work
    // area has to be scaled back down or a 125%-scaled display would look like
    // a 1720x960 monitor and the fit would not happen.
    let dpi = unsafe { GetDpiForSystem() };
    let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };

    Some(WorkArea {
        x: info.rcWork.left as f32 / scale,
        y: info.rcWork.top as f32 / scale,
        width: (info.rcWork.right - info.rcWork.left) as f32 / scale,
        height: (info.rcWork.bottom - info.rcWork.top) as f32 / scale,
    })
}

/// The primary monitor's work area, or `None` off Windows.
#[cfg(not(windows))]
pub fn primary_work_area() -> Option<WorkArea> {
    None
}

/// An x coordinate past the right edge of every monitor on this desktop.
///
/// Where a `--shot` capture's window is born. Not an off-screen *hide*: the
/// window is created visible at a position no monitor covers, which keeps the
/// compositor and this process's frame loop doing exactly what they do for a
/// window in use -- the difference is only that nobody can see it. Hiding it
/// instead would stop it being drawn, which is the opposite of what a capture
/// needs.
///
/// Asked of the *virtual* desktop rather than the primary monitor, because a
/// second monitor to the right is still somebody's screen: the earlier version of
/// this put the window at the edge of the primary one, which on a two-monitor desk
/// is where the work is.
#[cfg(windows)]
pub fn beyond_every_monitor_x() -> f32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    };

    // SAFETY: `GetSystemMetrics` reads two process-wide integers and takes no
    // pointers.
    let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    if width <= 0 {
        // No monitor answered, which is the machine with no desktop at all: a
        // large constant still keeps the window away from 0, and a capture that
        // lands somewhere odd is better than one that covers the screen.
        return 4000.0;
    }
    // Logical pixels, like every other coordinate this shell hands iced -- see
    // `primary_work_area` for why the same division has to happen here.
    (left + width) as f32 / system_scale_factor() + 200.0
}

/// Off Windows there is no virtual desktop to ask about.
#[cfg(not(windows))]
pub fn beyond_every_monitor_x() -> f32 {
    4000.0
}

/// The launcher's own top-level window handle.
///
/// iced hands out its own `window::Id`, and 0.12 offers no conversion to the
/// Win32 handle, so the handle has to be found rather than asked for. It is
/// found by walking the top-level windows and keeping the visible, unowned one
/// that belongs to this process.
///
/// Asking `GetForegroundWindow` instead would be shorter and wrong: a press on
/// an inactive window's edge still has to resize it (that is how activating by
/// dragging a frame works), and the foreground window at that moment is not
/// necessarily ours. The process check means a failed walk does nothing rather
/// than resizing someone else's window.
///
/// `pub(crate)` for [`crate::pick`]'s one use: a file dialog wants this window as its
/// owner, or it opens unowned and can end up behind the launcher it belongs to.
#[cfg(windows)]
pub(crate) fn own_hwnd() -> Option<isize> {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };

    // `EnumWindows` wants a C callback, which cannot capture anything, so the
    // handle travels through this static instead. Only the UI thread touches
    // it, and it is reset before every walk.
    static FOUND: AtomicIsize = AtomicIsize::new(0);

    extern "system" fn visit(hwnd: HWND, _: LPARAM) -> BOOL {
        let mut pid = 0u32;
        // SAFETY: `pid` is a local out-parameter; `hwnd` comes from the OS.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid != std::process::id() {
            return TRUE;
        }
        // SAFETY: read-only queries on a handle the OS just handed us.
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return TRUE;
        }
        // Owned windows are dialogs and tool windows; the shell is unowned.
        if unsafe { GetWindow(hwnd, GW_OWNER) } != 0 {
            return TRUE;
        }
        FOUND.store(hwnd, Ordering::Relaxed);
        0 // stop walking
    }

    FOUND.store(0, Ordering::Relaxed);
    // SAFETY: `visit` lives for the whole call and only writes to a static.
    unsafe { EnumWindows(Some(visit), 0) };
    match FOUND.load(Ordering::Relaxed) {
        0 => None,
        hwnd => Some(hwnd),
    }
}

/// Hand the pointer to Windows and let it run a native resize loop from `edge`.
#[cfg(not(windows))]
pub fn start_resize(_edge: ResizeEdge) {
    // Nothing to do: on other platforms the window keeps its own decorations
    // and the OS handles resizing.
}

// ---------------------------------------------------------------------------
// The hit-test shim
// ---------------------------------------------------------------------------
//
// A borderless window's whole surface is client area, so Windows answers
// `WM_NCHITTEST` with `HTCLIENT` for every pixel of it. That is why the frame
// used to be rebuilt in the UI: iced drew grab bands around the edges and
// posted `WM_NCLBUTTONDOWN` itself to start each resize, and corners could only
// borrow an axis cursor because iced has no diagonal ones.
//
// Answering `WM_NCHITTEST` ourselves hands that back: returning `HTLEFT`,
// `HTTOPLEFT` and friends makes Windows run its own resize loop from its own hit
// test, so the pointer gets the real diagonal resize cursors, and resizing stops
// depending on a six-pixel band drawn inside the client area.
//
// **The frame's edges are the only thing claimed.** This used to answer
// `HTMAXBUTTON` over the title bar's maximize control as well, to buy Windows
// 11's Snap Layouts flyout, and that was a mistake: `HTMAXBUTTON` hands the
// click to Windows' caption-button loop, which is written for a window that has
// a caption, and this one is created with `decorations: false`. The result was a
// maximize button that could not be pressed — press it, drag north-east, and the
// window followed the pointer with a stray white system button drawn where the
// real one should be. The flyout is available from the keyboard (`Win` + `Z`)
// and a working button is worth more. See [`hit_code`].
//
// Only the hit test is taken over. Every other message is chained straight to
// the procedure winit installed, so nothing else about the window changes.

/// `WM_NCHITTEST`: the pointer is over ordinary client area, and iced should
/// handle it.
pub const HTCLIENT: u32 = 1;

/// `WM_NCHITTEST`: the pointer is over a real maximize button.
///
/// Never returned any more — see the note above — but kept, spelled out rather
/// than imported, because the test that holds the restore bug in place has to be
/// able to say "this is *not* what the control answers" by name.
#[cfg(test)]
pub const HTMAXBUTTON: u32 = 9;

/// `WM_MOUSELEAVE` and `WM_NCMOUSELEAVE` used to be named here, to un-stick the
/// maximize control's hover when the pointer left the window. That tracking is
/// gone with the non-client answer that needed it (see [`hit_code`]): the control
/// is client area, iced gets the pointer's exits itself, and a message the shim
/// no longer acts on is not worth an ABI constant and a `Win32_UI_Controls`
/// feature.

/// Where the title bar's maximize button sits, in logical pixels, measured from
/// the client area's top-right corner.
///
/// The UI layer publishes this because only the title bar knows its own layout;
/// the window procedure needs it to answer `WM_NCHITTEST`. Logical rather than
/// physical pixels because that is the unit iced lays out in, and the shim
/// divides by the window's DPI before comparing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptionTarget {
    /// Distance from the client's right edge to the button's right side.
    pub right_inset: f32,
    /// The button's width.
    pub width: f32,
    /// The button's top, measured down from the client's top edge.
    pub top: f32,
    /// The button's bottom, measured down from the client's top edge.
    pub bottom: f32,
}

impl CaptionTarget {
    /// The button's horizontal span for a client `client_width` wide.
    pub fn x_span(&self, client_width: f32) -> (f32, f32) {
        let right = client_width - self.right_inset;
        (right - self.width, right)
    }

    /// Whether a logical client point lands on the button.
    pub fn contains(&self, x: f32, y: f32, client_width: f32) -> bool {
        let (left, right) = self.x_span(client_width);
        x >= left && x < right && y >= self.top && y < self.bottom
    }
}

/// The published maximize button, once the UI layer has said where it is.
static CAPTION: Mutex<Option<CaptionTarget>> = Mutex::new(None);

/// This process's own top-level window, once the shim has found it.
#[cfg(windows)]
static OWN_WINDOW: AtomicIsize = AtomicIsize::new(0);

/// The window procedure the shim replaced, so it can chain to it.
#[cfg(windows)]
static PREVIOUS_PROC: AtomicIsize = AtomicIsize::new(0);

/// The far end of the pipe the window procedure reports state changes on.
///
/// The app registers a sender when its subscription starts; the procedure holds
/// the other end and posts into it when the window's own state changes — which
/// is now only whether the window is maximized. That is a thing Windows changes
/// without telling iced, and the app has to draw it (the caption's glyph is
/// maximize or restore).
///
/// **What used to be here, and why it is gone.** The procedure also reported the
/// maximize control's hover, because the shim answered `HTMAXBUTTON` over that
/// control and a non-client region never reaches iced — no pointer events, so no
/// hover the widget could paint. With the control answered as client area (see
/// [`hit_code`], and the restore bug that forced it) iced sees the pointer
/// normally and `Shell::control_button` carries its own `on_enter`/`on_exit`, so
/// this whole channel for hover is unnecessary.
///
/// The measurement that decided it, kept because it is the reason a repaint is
/// never the answer here and the next person who needs a window-driven redraw
/// will need it: `InvalidateRect` does not work, because iced rebuilds a view
/// only when a *message* arrives. A repaint redraws the widget tree the window
/// already holds, so an invalidated window paints the same pixels as before and
/// `view()` is never called again — instrumenting it showed two calls at startup
/// and not one more through hovers, a maximize, a restore and a resize. Hence
/// this channel: it is the one path that reaches `update`.
static WATCHER: Mutex<Option<futures::channel::mpsc::UnboundedSender<()>>> = Mutex::new(None);

/// Register the sender the window procedure reports state changes to.
///
/// The newest registration wins, so a restarted subscription is never left
/// sending into a receiver nobody holds.
pub fn watch_window_state(sender: futures::channel::mpsc::UnboundedSender<()>) {
    if let Ok(mut slot) = WATCHER.lock() {
        *slot = Some(sender);
    }
}

/// Tell the app that the window's own state changed.
///
/// Called from the window procedure, on the UI thread, so it neither blocks on
/// the app nor waits for one: the critical section is a pointer read, and a
/// send into an unbounded channel never waits for its receiver. With no watcher
/// — before the subscription starts, or after it is dropped — this is a no-op,
/// which is what makes it safe to call from a message as hot as a hit test.
pub fn window_state_changed() {
    let Ok(slot) = WATCHER.lock() else {
        return;
    };
    if let Some(sender) = slot.as_ref() {
        let _ = sender.unbounded_send(());
    }
}

/// How many messages the shim will look for the window for before giving up.
///
/// `install_hit_test` is cheap once it has succeeded — an atomic load — but
/// every attempt that has *not* succeeded walks the desktop looking for our
/// window, so a process that never owns one must not keep trying forever. In
/// practice the first message after startup finds the window; the margin is
/// here so a slow machine still installs, and small enough that a launcher
/// with no window is not left walking the window list on every update.
#[cfg(windows)]
const INSTALL_ATTEMPTS: u32 = 64;

/// How many times the shim has gone looking for the window.
#[cfg(windows)]
static INSTALL_ATTEMPTS_MADE: AtomicU32 = AtomicU32::new(0);

/// Publish where the title bar's maximize button is.
///
/// Idempotent, and cheap enough to call from a render if the layout ever needs
/// to move the button.
pub fn set_caption_target(target: CaptionTarget) {
    if let Ok(mut slot) = CAPTION.lock() {
        *slot = Some(target);
    }
}

/// The published maximize button, if there is one.
fn caption_target() -> Option<CaptionTarget> {
    // `try_lock` on purpose: this runs inside the window procedure, where
    // waiting on a lock another thread holds would wedge the message pump.
    // Losing the race just means the point is answered as client area, and the
    // next mouse move answers it properly.
    CAPTION.try_lock().ok().and_then(|slot| *slot)
}

/// The `WM_NCHITTEST` answer for a logical client point.
///
/// `None` means "no opinion" — the point is outside the client area, which a
/// maximized window's invisible frame is, and the answer should stay Windows'.
///
/// `zoomed` suppresses the resize edges: a maximized window has nothing to
/// resize, and its edges belong to the screen.
///
/// **The title bar's controls are deliberately not answered for.** This used to
/// return `HTMAXBUTTON` over the maximize control, and that is what broke it.
/// `HTMAXBUTTON` tells Windows the pixel belongs to a *real* caption button, so
/// Windows takes the click away from iced and runs its own caption-button loop
/// (`WM_NCLBUTTONDOWN` → `WM_SYSCOMMAND`/`SC_MAXIMIZE`) — a loop that is written
/// for a window that has a caption. This one does not: it is created with
/// `decorations: false`, so there is no caption for that loop to act on, and
/// what a reader got instead was the caption *drag* — press the restore button,
/// move the pointer north-east, and the window follows it, with a stray white
/// system button drawn where the real one should be. That is a control that
/// cannot be pressed, which is worse than one that is merely not native.
///
/// So every pixel inside the client area is `HTCLIENT`, and the frame's resize
/// edges are the only thing claimed. The cost is honest and worth stating:
/// Windows 11's Snap Layouts flyout needs a non-client maximize button, so it
/// does not appear. A working maximize/restore is worth more than a flyout, and
/// the flyout is reachable with the keyboard (`Win` + `Z`) regardless.
///
/// The `target` parameter stays because the shape of the problem is still worth
/// testing — it is what a caller would need to make the claim, and dropping the
/// capability without dropping the reasoning would make the next reader
/// rediscover it.
pub fn hit_code(
    x: f32,
    y: f32,
    client_width: f32,
    client_height: f32,
    target: Option<CaptionTarget>,
    zoomed: bool,
) -> Option<u32> {
    // Also rejects NaN, which would otherwise fall through every comparison.
    if !(0.0..client_width).contains(&x) || !(0.0..client_height).contains(&y) {
        return None;
    }

    if !zoomed {
        if let Some(code) = edge_hit_code(x, y, client_width, client_height) {
            return Some(code);
        }
    }

    // `target` is read so that an empty caption cannot be mistaken for a
    // deliberate one: the parameter is still part of the signature the shim
    // calls, and a caller that published a button and got `HTCLIENT` back is
    // told so by this branch rather than by silence.
    if let Some(target) = target {
        if target.contains(x, y, client_width) {
            return Some(HTCLIENT);
        }
    }

    Some(HTCLIENT)
}

/// Which frame edge a point within [`RESIZE_BAND`] of the client's edge belongs
/// to, if any.
///
/// The codes come from [`ResizeEdge::hit_code`] rather than being spelled out
/// again, so the mapping Windows is told and the one `start_resize` posts can
/// never disagree.
fn edge_hit_code(x: f32, y: f32, width: f32, height: f32) -> Option<u32> {
    let west = x < RESIZE_BAND;
    let east = x >= width - RESIZE_BAND;
    let north = y < RESIZE_BAND;
    let south = y >= height - RESIZE_BAND;

    Some(match (west, east, north, south) {
        (true, _, true, _) => ResizeEdge::NorthWest.hit_code(),
        (_, true, true, _) => ResizeEdge::NorthEast.hit_code(),
        (true, _, _, true) => ResizeEdge::SouthWest.hit_code(),
        (_, true, _, true) => ResizeEdge::SouthEast.hit_code(),
        (true, _, _, _) => ResizeEdge::West.hit_code(),
        (_, true, _, _) => ResizeEdge::East.hit_code(),
        (_, _, true, _) => ResizeEdge::North.hit_code(),
        (_, _, _, true) => ResizeEdge::South.hit_code(),
        _ => return None,
    })
}

/// The screen coordinates packed into a `WM_NCHITTEST` `lParam`: two signed
/// 16-bit halves, low word first.
///
/// The `GET_X_LPARAM`/`GET_Y_LPARAM` pair windows-sys does not generate. The
/// sign is the part that matters — a monitor placed to the left of the primary
/// one has negative screen coordinates, and a shim that read these as unsigned
/// would hit-test the wrong edge (or nothing) on such a setup.
pub fn screen_point(lparam: isize) -> (i32, i32) {
    let packed = lparam as u32;
    let x = (packed & 0xffff) as u16 as i16 as i32;
    let y = ((packed >> 16) & 0xffff) as u16 as i16 as i32;
    (x, y)
}

/// The hit test for a point Windows is asking about, in logical client
/// coordinates.
#[cfg(windows)]
fn hit_test_answer(hwnd: isize, lparam: isize) -> Option<u32> {
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetClientRect, IsZoomed};

    let (screen_x, screen_y) = screen_point(lparam);
    let mut point = POINT { x: screen_x, y: screen_y };
    let empty = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let mut client = empty;

    // SAFETY: read-only queries against our own window handle.
    unsafe {
        if ScreenToClient(hwnd, &mut point) == 0 {
            return None;
        }
        if GetClientRect(hwnd, &mut client) == 0 {
            return None;
        }
    }

    // The cursor arrives in physical pixels; the layout was measured in logical
    // ones. Comparing them raw would put the hit test out by the DPI scale on
    // every scaled display.
    let scale = dpi_scale(hwnd);
    // SAFETY: a read-only query on our own window handle.
    let zoomed = unsafe { IsZoomed(hwnd) } != 0;

    hit_code(
        point.x as f32 / scale,
        point.y as f32 / scale,
        (client.right - client.left) as f32 / scale,
        (client.bottom - client.top) as f32 / scale,
        caption_target(),
        zoomed,
    )
}

/// The window's DPI scale, defaulting to 1 when Windows will not say.
#[cfg(windows)]
fn dpi_scale(hwnd: isize) -> f32 {
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;

    // SAFETY: a read-only query on our own window handle.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        1.0
    } else {
        dpi as f32 / 96.0
    }
}

/// The window procedure shim.
///
/// It answers `WM_NCHITTEST` and chains everything else — including all the
/// work winit's own procedure does — so installing it cannot change how any
/// other message behaves.
#[cfg(windows)]
unsafe extern "system" fn hit_test_proc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, SIZE_MAXIMIZED, SIZE_RESTORED, WM_NCDESTROY, WM_NCHITTEST, WM_SIZE,
        WNDPROC,
    };

    if msg == WM_NCHITTEST {
        // Only the frame is claimed. Everything inside the client area is
        // `HTCLIENT`, so the answer is the frame's and nothing here has to
        // interpret it — and, in particular, no caption control can be turned
        // into a system button by accident.
        if let Some(code) = hit_test_answer(hwnd, lparam) {
            return code as isize;
        }
    }

    // Maximizing and restoring both arrive here, and neither reaches the app on
    // its own: the app's caption has to swap its glyph, and it cannot see the
    // button that starts it any more. The size itself is winit's business — it
    // is chained either way — but the app is told so it can redraw.
    if msg == WM_SIZE && (wparam == SIZE_MAXIMIZED as usize || wparam == SIZE_RESTORED as usize) {
        window_state_changed();
    }

    let previous = PREVIOUS_PROC.load(Ordering::Relaxed);
    if previous == 0 {
        // Only reachable if the shim were installed without its predecessor
        // being recorded, which `install_hit_test` does not do.
        return 0;
    }

    // SAFETY: `previous` is the procedure this shim replaced on our own window,
    // stored as a code address by `install_hit_test` and put back into its
    // original type here. Its signature is the one `SetWindowLongPtrW` expects.
    let previous: WNDPROC = unsafe { core::mem::transmute(previous) };
    // SAFETY: the arguments are the ones we were handed, untouched.
    let result = unsafe { CallWindowProcW(previous, hwnd, msg, wparam, lparam) };

    // The window is gone, so everything remembered about it is stale. Holding
    // on to a procedure address whose window no longer exists would send the
    // next message down a dead chain, and a launcher that opened a second
    // window would find the install looking like a repeat and never shim it.
    if msg == WM_NCDESTROY && hwnd == OWN_WINDOW.load(Ordering::Relaxed) {
        PREVIOUS_PROC.store(0, Ordering::Relaxed);
        OWN_WINDOW.store(0, Ordering::Relaxed);
        INSTALL_ATTEMPTS_MADE.store(0, Ordering::Relaxed);
    }

    result
}

/// Take over `WM_NCHITTEST` on this process's window.
///
/// Returns whether it took effect. Deferred until the window exists — the
/// procedure cannot be swapped before there is a window to swap it on, and
/// iced's `Application::new` runs before the window is built — and safe to call
/// again: a second successful call is a no-op.
#[cfg(windows)]
pub fn install_hit_test() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrW, GWLP_WNDPROC};

    if PREVIOUS_PROC.load(Ordering::Relaxed) != 0 {
        return true;
    }
    // A failed walk costs a trip through the desktop's window list, so it is
    // worth a bounded number of them and no more.
    if INSTALL_ATTEMPTS_MADE.load(Ordering::Relaxed) >= INSTALL_ATTEMPTS {
        return false;
    }
    INSTALL_ATTEMPTS_MADE.fetch_add(1, Ordering::Relaxed);

    let Some(hwnd) = own_hwnd() else {
        return false;
    };

    // The cast goes through a bare pointer on purpose: a function item cannot
    // be turned into an integer directly, and the procedure is addressed the
    // way `SetWindowLongPtrW` stores it — as an address, not a typed pointer.
    let shim = hit_test_proc as *const () as usize as isize;

    // SAFETY: our own window, on the thread that created it, and the shim
    // chains every message it does not answer itself.
    let previous = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, shim) };
    // A failed call leaves the existing procedure in place and returns zero,
    // so nothing is recorded and the window is untouched. The previous
    // procedure of a real window is never zero, which is what makes zero
    // unambiguous here.
    if previous == 0 {
        return false;
    }

    OWN_WINDOW.store(hwnd, Ordering::Relaxed);
    PREVIOUS_PROC.store(previous, Ordering::Relaxed);
    true
}

/// Take over `WM_NCHITTEST` on this process's window.
#[cfg(not(windows))]
pub fn install_hit_test() -> bool {
    // Other platforms keep their decorations, and the OS answers the hit test
    // for them.
    false
}

/// Whether the window is maximized, read from the window itself.
///
/// `None` when there is no window to ask — off Windows, or before the shim has
/// found one — in which case callers fall back to what they last saw.
///
/// Worth asking rather than tracking: maximizing happens through paths the app
/// never sees, from Windows' own caption button to Aero Snap to `Win`+`Up`, and
/// a title bar that shows the wrong glyph after any of them is worse than one
/// cheap query per frame.
#[cfg(windows)]
pub fn window_maximized() -> Option<bool> {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsZoomed;

    let hwnd = OWN_WINDOW.load(Ordering::Relaxed);
    if hwnd == 0 {
        return None;
    }
    // SAFETY: a read-only query on the handle `install_hit_test` recorded.
    Some(unsafe { IsZoomed(hwnd) } != 0)
}

/// Whether the window is maximized, read from the window itself.
#[cfg(not(windows))]
pub fn window_maximized() -> Option<bool> {
    None
}

// ---------------------------------------------------------------------------
// OS preferences
// ---------------------------------------------------------------------------

/// Where Windows keeps the per-user app appearance choice.
#[cfg(windows)]
const PERSONALIZE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
/// Where Windows keeps its own version numbers.
#[cfg(windows)]
#[cfg(test)]
const CURRENT_VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

/// A UTF-16, NUL-terminated copy of `text` for the `...W` Win32 entry points.
///
/// Shared with [`crate::pick`], which is the other module that hands a string to
/// Windows: the file dialog takes its title, its filter and its default suffix in the
/// same shape these calls do.
#[cfg(windows)]
pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Read a `REG_DWORD` from the registry, or `None` if it is not there.
///
/// Deliberately silent about failures: an unreadable preference is not an
/// error condition for a launcher, it just means the default applies.
#[cfg(windows)]
fn registry_dword(root: isize, subkey: &str, value: &str) -> Option<u32> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_DWORD};

    let subkey = wide(subkey);
    let value = wide(value);
    let mut data: u32 = 0;
    let mut size = core::mem::size_of::<u32>() as u32;
    // SAFETY: every pointer is to a live local of the type the call expects,
    // `size` describes `data`, and no pointer outlives this call.
    let status = unsafe {
        RegGetValueW(
            root,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            core::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    (status == ERROR_SUCCESS).then_some(data)
}

/// Read a `REG_SZ` from the registry, or `None` if it is not there.
#[cfg(windows)]
#[cfg(test)]
fn registry_string(root: isize, subkey: &str, value: &str) -> Option<String> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_SZ};

    let subkey = wide(subkey);
    let value = wide(value);
    // Two passes, because the call reports the required size when the buffer
    // is too small: the first gets that size, the second fills the buffer.
    let mut size: u32 = 0;
    // SAFETY: a size query with no buffer, which is the documented way to ask
    // for the length; the null data pointer is only read when the size fits.
    let probe = unsafe {
        RegGetValueW(
            root,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut size,
        )
    };
    if probe != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; (size as usize + 1) / 2];
    // SAFETY: `buffer` holds `size` bytes, which is what the previous call
    // asked for, and `size` is refreshed with the bytes actually written.
    let status = unsafe {
        RegGetValueW(
            root,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            core::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..end]))
}

/// Whether the operating system is set to a light app appearance.
///
/// Windows stores this per user, and it is what the reference client reads to
/// decide what "System" means. An unreadable or absent value answers `false`:
/// the shell's own identity is dark, so an unknown OS preference must not flip
/// someone who never asked into a light launcher.
#[cfg(windows)]
pub fn system_prefers_light() -> bool {
    use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;

    matches!(registry_dword(HKEY_CURRENT_USER, PERSONALIZE_KEY, "AppsUseLightTheme"), Some(light) if light != 0)
}

/// Off Windows there is no preference to read, so the answer is the app's own
/// dark default rather than a guess about the desktop environment.
#[cfg(not(windows))]
pub fn system_prefers_light() -> bool {
    false
}

/// Format the Windows version line, e.g. `Windows 10.0.19045`.
///
/// The build number is what actually distinguishes one Windows 10 from
/// another, so it belongs in the label. Pure enough to test without a registry.
#[cfg(test)]
pub fn version_label(major: u32, minor: u32, build: &str) -> String {
    format!("Windows {major}.{minor}.{build}")
}

/// The running Windows version, or `None` when it cannot be read.
#[cfg(windows)]
#[cfg(test)]
pub fn windows_version() -> Option<String> {
    use windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE;

    // Major/minor are DWORDs that only exist from Windows 10 on; the build
    // number is a string and is present on every release this launcher runs on.
    let build = registry_string(HKEY_LOCAL_MACHINE, CURRENT_VERSION_KEY, "CurrentBuildNumber")?;
    let major = registry_dword(HKEY_LOCAL_MACHINE, CURRENT_VERSION_KEY, "CurrentMajorVersionNumber")
        .unwrap_or(10);
    let minor = registry_dword(HKEY_LOCAL_MACHINE, CURRENT_VERSION_KEY, "CurrentMinorVersionNumber")
        .unwrap_or(0);
    Some(version_label(major, minor, build.trim()))
}

/// The running OS version, or `None` when the platform does not offer one.
#[cfg(not(windows))]
pub fn windows_version() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(width: f32, height: f32) -> Option<WorkArea> {
        Some(WorkArea { x: 0.0, y: 0.0, width, height })
    }

    #[test]
    fn the_version_label_names_the_build() {
        // The build number is the only part that distinguishes two Windows 10
        // machines, so it has to survive into the label.
        assert_eq!(version_label(10, 0, "19045"), "Windows 10.0.19045");
        assert_eq!(version_label(6, 1, "7601"), "Windows 6.1.7601");
    }

    #[test]
    fn the_running_version_looks_like_a_version() {
        // Platform-dependent by nature, so it only asserts the shape: a real
        // version on this machine, or an honest `None` where there is none.
        if let Some(label) = windows_version() {
            assert!(label.starts_with("Windows "), "unexpected label: {label}");
            assert!(
                label.trim_start_matches("Windows ").split('.').count() >= 3,
                "a version needs major.minor.build: {label}"
            );
        }
    }

    #[test]
    fn a_window_that_fits_is_left_alone() {
        let work = screen(2560.0, 1440.0);
        assert_eq!(
            fit_to_work_area((1280.0, 820.0), work, (980.0, 640.0), 0.92),
            (1280.0, 820.0)
        );
    }

    #[test]
    fn an_oversized_window_is_shrunk_to_the_screen() {
        // The bug this exists for: 1280x820 on a 1366x768 display, which put
        // the status bar past the bottom edge where it could not be reached.
        let work = screen(1366.0, 768.0);
        let (width, height) = fit_to_work_area((1280.0, 820.0), work, (980.0, 640.0), 0.92);
        assert!(width < 1366.0, "the window must fit across, got {width}");
        assert!(height < 768.0, "the window must fit down, got {height}");
        // Every clamped axis lands exactly on the allowed fraction, so the
        // usable screen is never given away to a margin nobody asked for.
        assert!((width - 1366.0 * 0.92).abs() < 0.01);
        assert!((height - 768.0 * 0.92).abs() < 0.01);
    }

    #[test]
    fn a_clamped_window_keeps_its_aspect_rather_than_going_squarish() {
        // Both axes shrink by the same rule, so a window that was too big in
        // one direction only loses what it must.
        let work = screen(1366.0, 2000.0);
        let (width, height) = fit_to_work_area((1280.0, 820.0), work, (980.0, 640.0), 0.92);
        assert!(width < 1280.0, "width should be clamped by the narrow screen");
        assert_eq!(height, 820.0, "a height that fits is left exactly alone");
    }

    #[test]
    fn a_tiny_screen_falls_back_to_the_minimum_size() {
        // 800x600 cannot hold a 980x640 window; the floor wins so the app stays
        // usable instead of collapsing.
        let (width, height) = fit_to_work_area((1280.0, 820.0), screen(800.0, 600.0), (980.0, 640.0), 0.92);
        assert_eq!((width, height), (980.0, 640.0));
    }

    #[test]
    fn an_unknown_screen_changes_nothing() {
        assert_eq!(
            fit_to_work_area((1280.0, 820.0), None, (980.0, 640.0), 0.92),
            (1280.0, 820.0)
        );
    }

    #[test]
    fn every_edge_maps_to_the_windows_hit_test_code() {
        // Spell the codes out here as well as in `hit_code`: a typo in either
        // place is then a failing test rather than a window that resizes from
        // the wrong edge.
        let expected = [
            (ResizeEdge::West, 10u32),
            (ResizeEdge::East, 11),
            (ResizeEdge::North, 12),
            (ResizeEdge::NorthWest, 13),
            (ResizeEdge::NorthEast, 14),
            (ResizeEdge::South, 15),
            (ResizeEdge::SouthWest, 16),
            (ResizeEdge::SouthEast, 17),
        ];
        for (edge, code) in expected {
            assert_eq!(edge.hit_code(), code, "hit code for {}", edge.label());
        }
    }

    #[test]
    fn edges_and_corners_are_all_distinct() {
        let mut codes: Vec<u32> = ResizeEdge::ALL.iter().map(|edge| edge.hit_code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), ResizeEdge::ALL.len(), "two edges share a hit code");

        let mut labels: Vec<&str> = ResizeEdge::ALL.iter().map(|edge| edge.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), ResizeEdge::ALL.len(), "two edges share a label");
    }

    #[test]
    fn every_grip_asks_for_a_resize_cursor() {
        for edge in ResizeEdge::ALL {
            let cursor = edge.interaction();
            assert!(
                matches!(
                    cursor,
                    mouse::Interaction::ResizingHorizontally | mouse::Interaction::ResizingVertically
                ),
                "{} should show a resize cursor, got {cursor:?}",
                edge.label()
            );
        }
    }

    #[test]
    fn the_grab_band_is_thick_enough_to_hit() {
        // Windows' own frame is 4px; anything thinner than that is a regression
        // in usability, anything much thicker starts eating content clicks.
        assert!((4.0..=8.0).contains(&RESIZE_BAND), "RESIZE_BAND is {RESIZE_BAND}");
    }

    // ---- the hit test ----------------------------------------------------

    /// The 1257x707 window the shell opens with on a 1366x768 screen, with the
    /// maximize button the title bar publishes for it: a 32px button inset 54px
    /// from the right edge, in the bar's 34px content band below the frame.
    ///
    /// These are the *title bar's* numbers, and the title bar's own tests are
    /// what keep them true; all this module needs is a target of the right
    /// shape to hit-test against.
    fn measured_window() -> (f32, f32, CaptionTarget) {
        (
            1257.0,
            707.0,
            CaptionTarget { right_inset: 54.0, width: 32.0, top: 17.0, bottom: 41.0 },
        )
    }

    #[test]
    fn the_hit_codes_are_the_ones_windows_defined() {
        // Spelled out again here so a typo in the constant is a failing test
        // rather than a window that resizes from the wrong edge.
        assert_eq!(HTCLIENT, 1);
        assert_eq!(HTMAXBUTTON, 9);
        assert_eq!(ResizeEdge::West.hit_code(), 10);
        assert_eq!(ResizeEdge::East.hit_code(), 11);
        assert_eq!(ResizeEdge::North.hit_code(), 12);
        assert_eq!(ResizeEdge::NorthWest.hit_code(), 13);
        assert_eq!(ResizeEdge::NorthEast.hit_code(), 14);
        assert_eq!(ResizeEdge::South.hit_code(), 15);
        assert_eq!(ResizeEdge::SouthWest.hit_code(), 16);
        assert_eq!(ResizeEdge::SouthEast.hit_code(), 17);
    }

    #[test]
    fn ordinary_content_is_client_area() {
        let (w, h, target) = measured_window();
        // Where the brand cluster is: it must stay client area, because that is
        // what lets iced keep dragging the window from it.
        assert_eq!(hit_code(112.0, 23.0, w, h, Some(target), false), Some(HTCLIENT));
        // And the middle of the window, and the middle of the content pane.
        assert_eq!(hit_code(600.0, 23.0, w, h, Some(target), false), Some(HTCLIENT));
        assert_eq!(hit_code(600.0, 400.0, w, h, Some(target), false), Some(HTCLIENT));
    }

    #[test]
    fn each_edge_and_corner_answers_with_its_resize_code() {
        let (w, h, target) = measured_window();
        let band = RESIZE_BAND;
        let cases = [
            (ResizeEdge::West, 1.0, 300.0),
            (ResizeEdge::East, w - 1.0, 300.0),
            (ResizeEdge::North, 600.0, 1.0),
            (ResizeEdge::South, 600.0, h - 1.0),
            (ResizeEdge::NorthWest, 1.0, 1.0),
            (ResizeEdge::NorthEast, w - 1.0, 1.0),
            (ResizeEdge::SouthWest, 1.0, h - 1.0),
            (ResizeEdge::SouthEast, w - 1.0, h - 1.0),
        ];
        for (edge, x, y) in cases {
            assert_eq!(
                hit_code(x, y, w, h, Some(target), false),
                Some(edge.hit_code()),
                "the {} corner/edge" , edge.label()
            );
        }

        // Just inside the band is content; just outside is the edge.
        assert_eq!(hit_code(band - 0.5, 300.0, w, h, Some(target), false), Some(ResizeEdge::West.hit_code()));
        assert_eq!(hit_code(band, 300.0, w, h, Some(target), false), Some(HTCLIENT));
        assert_eq!(hit_code(w - band, 300.0, w, h, Some(target), false), Some(ResizeEdge::East.hit_code()));
        assert_eq!(hit_code(w - band - 0.5, 300.0, w, h, Some(target), false), Some(HTCLIENT));
    }

    #[test]
    fn a_maximized_window_offers_no_resize_edges() {
        // Its edges are the screen's, and Windows would otherwise put the
        // resize cursor on them and refuse the drag.
        let (w, h, target) = measured_window();
        for (x, y) in [(1.0, 300.0), (w - 1.0, 300.0), (600.0, 1.0), (600.0, h - 1.0), (1.0, 1.0)] {
            assert_eq!(
                hit_code(x, y, w, h, Some(target), true),
                Some(HTCLIENT),
                "({x},{y}) must not resize a maximized window"
            );
        }
        // The restore control still works while maximized — it is what restores,
        // and it is iced's to answer because it is client area.
        assert_eq!(
            hit_code(w - 64.0, 23.0, w, h, Some(target), true),
            Some(HTCLIENT)
        );
    }

    #[test]
    fn the_restore_control_is_never_answered_as_a_system_caption_button() {
        // The regression this fixes. Answering `HTMAXBUTTON` here made Windows
        // believe the window had a real caption, and its caption-button loop on
        // a `decorations: false` window is a *drag*: pressing restore and moving
        // the pointer north-east dragged the window and drew a stray white
        // system button, instead of restoring. Every pixel of the control —
        // inside, on each edge, maximized or not — must be client area so the
        // click reaches iced's own button and its `window::maximize`.
        let (w, h, target) = measured_window();
        let (left, right) = target.x_span(w);
        for zoomed in [false, true] {
            for (x, y) in [
                (w - 64.0, 23.0),
                (left + 1.0, 23.0),
                (right - 1.0, 23.0),
                ((left + right) / 2.0, target.top + 1.0),
                ((left + right) / 2.0, target.bottom - 1.0),
            ] {
                let code = hit_code(x, y, w, h, Some(target), zoomed);
                assert_eq!(
                    code,
                    Some(HTCLIENT),
                    "({x},{y}) zoomed={zoomed}: a non-client answer here is the restore bug"
                );
                assert_ne!(code, Some(HTMAXBUTTON), "({x},{y}) must not be a system button");
            }
        }
    }

    #[test]
    fn the_frame_is_the_only_thing_the_shim_claims() {
        // Every pixel inside the client area that is not a resize edge is iced's,
        // whatever the title bar published and whether the window is maximized.
        // This is the whole claim of the module in one assertion, and it is what
        // makes the window draggable and its controls pressable.
        let (w, h, target) = measured_window();
        let mut client = 0usize;
        let mut non_client = 0usize;
        for y in (0..(h as usize)).step_by(7) {
            for x in (0..(w as usize)).step_by(7) {
                let edge = x < RESIZE_BAND as usize
                    || y < RESIZE_BAND as usize
                    || x >= w as usize - RESIZE_BAND as usize
                    || y >= h as usize - RESIZE_BAND as usize;
                match hit_code(x as f32, y as f32, w, h, Some(target), false) {
                    Some(HTCLIENT) if !edge => client += 1,
                    Some(_) if edge => non_client += 1,
                    other => panic!("({x},{y}) is neither client nor a claimed edge: {other:?}"),
                }
            }
        }
        assert!(client > 1000, "only {client} pixels were client area");
        assert!(non_client > 0, "the frame claimed nothing at all");
    }

    #[test]
    fn the_button_is_taken_over_only_when_it_has_been_published() {
        // Before the UI layer says where the button is, nothing is non-client
        // except the frame — so a window that never published still behaves.
        let (w, h, _) = measured_window();
        assert_eq!(hit_code(w - 64.0, 23.0, w, h, None, false), Some(HTCLIENT));
    }

    #[test]
    fn points_outside_the_client_area_keep_windows_answer() {
        // A maximized window's frame sits outside its client rect; the shim
        // must not claim those pixels.
        let (w, h, target) = measured_window();
        for (x, y) in [(-1.0, 23.0), (w, 23.0), (600.0, -1.0), (600.0, h), (-40.0, -40.0)] {
            assert_eq!(hit_code(x, y, w, h, Some(target), false), None, "({x},{y})");
        }
        // And a client area with no size at all has no opinion either.
        assert_eq!(hit_code(0.0, 0.0, 0.0, 0.0, Some(target), false), None);
        // NaN is not a point inside anything.
        assert_eq!(hit_code(f32::NAN, 23.0, w, h, Some(target), false), None);
    }

    #[test]
    fn screen_coordinates_are_read_as_signed_words() {
        // The packing `WM_NCHITTEST` uses: x in the low word, y in the high one.
        assert_eq!(screen_point(((600u32) | (400u32 << 16)) as isize), (600, 400));
        // Negative coordinates are normal on a monitor left of the primary one,
        // and must not read as 65000-odd.
        assert_eq!(screen_point(((-64i32 as u32) & 0xffff | (((-20i32 as u32) & 0xffff) << 16)) as isize), (-64, -20));
        assert_eq!(screen_point(0), (0, 0));
        // Only the low 32 bits are coordinates; whatever sits above them in the
        // pointer-sized `lParam` must not leak in.
        assert_eq!(screen_point(((600u32 | (400u32 << 16)) as isize) | (1isize << 40)), (600, 400));
    }

    #[test]
    fn the_window_procedure_reports_state_changes_to_the_app() {
        // The pipe the app subscribes to, exercised through the same two calls
        // the window procedure makes. No window is involved: what is tested is
        // that a change reaches the app at all, which is the part that was
        // missing — the shim used to repaint the window instead, and iced
        // rebuilt nothing.
        //
        // One test rather than three, because the registration is global and
        // tests in a binary run side by side: a second test reporting a change
        // would show up in this receiver and make it flaky.
        use futures::future::FutureExt;
        use futures::StreamExt;

        // Before any subscription exists, a report has nowhere to go and must
        // be harmless — the window procedure runs whenever Windows asks, which
        // can be before the app's subscription starts or after it stops.
        window_state_changed();

        let (sender, mut receiver) = futures::channel::mpsc::unbounded();
        watch_window_state(sender);
        assert!(receiver.next().now_or_never().is_none(), "nothing is reported before a change");

        window_state_changed();
        assert_eq!(receiver.next().now_or_never(), Some(Some(())));
        assert!(
            receiver.next().now_or_never().is_none(),
            "one report per change, not a standing request"
        );

        // A restarted subscription replaces the registration rather than being
        // added to it, so a report never goes to a receiver nobody holds. The
        // replaced channel is not merely silent, it is *closed* — its sender was
        // dropped — which is what `Some(None)` from a stream means.
        let (newer, mut newer_receiver) = futures::channel::mpsc::unbounded();
        watch_window_state(newer);
        assert_eq!(
            receiver.next().now_or_never(),
            Some(None),
            "the replaced registration was dropped, not kept alongside"
        );
        window_state_changed();
        assert_eq!(newer_receiver.next().now_or_never(), Some(Some(())));
    }

    #[test]
    fn the_published_caption_target_round_trips() {
        // The one test that touches the published slot, so it cannot race with
        // another that reads it.
        let (w, _, target) = measured_window();
        set_caption_target(target);
        assert_eq!(caption_target(), Some(target));
        // The slot still round-trips into the hit test, and the answer is client
        // area: publishing a caption target no longer buys a system button (see
        // `the_restore_control_is_never_answered_as_a_system_caption_button`).
        assert_eq!(hit_code(w - 64.0, 23.0, w, 707.0, caption_target(), false), Some(HTCLIENT));
    }
}
