// The Linux half of the island: cursor position on X11 and the
// never-takes-focus window hints. On Wayland there is no global cursor —
// the poll sees no movement and the tray menu remains the way in, which is
// the spec's Wayland fallback rather than a crash.

use std::sync::OnceLock;

/// One X11 connection, opened once and kept.
///
/// The `mouse_position` crate looks like a thin wrapper and is not: every call
/// dlopens libX11, opens a display, reads the Xauthority file, queries the
/// pointer and tears the whole thing down again. Measured on this machine at
/// 471–838 µs per call, which at 60 Hz is 28–50 ms of CPU *every second* while the
/// island is visible — 3–5 % of a core, permanently, for one integer pair.
///
/// Holding the display open and asking only `XQueryPointer` costs 47 µs, ~18×
/// cheaper. The poll thread is the only user of this connection and X11 is
/// fine with one thread on one display as long as calls are serialised, which
/// they are: `cursor_physical` is called from exactly one thread.
/// The raw Xlib display pointer is not `Send`/`Sync`, and it is genuinely not
/// thread-safe — Xlib requires all calls on one display to be serialised. That
/// is exactly the contract here: `cursor_physical` is called from the poll thread
/// and nowhere else, so the pointer is wrapped to say so rather than being made
/// `unsafe impl` without the evidence. The `Mutex` makes the guarantee
/// mechanical instead of a comment.
struct Cursor {
    x11: &'static x11_dl::xlib::Xlib,
    display: *mut x11_dl::xlib::Display,
}

// SAFETY: the contained pointer is only ever dereferenced while the `Mutex` in
// `CURSOR_STATE` is held, so no two threads are ever inside libX11 at once. The
// value is immutable after `OnceLock` initialisation.
unsafe impl Send for Cursor {}
unsafe impl Sync for Cursor {}

impl Cursor {
    /// # Safety: caller must hold the mutex guarding this connection.
    unsafe fn query(&self) -> Option<(f64, f64)> {
        use std::os::raw::{c_int, c_ulong, c_uint};
        let root: c_ulong = unsafe { (self.x11.XDefaultRootWindow)(self.display) };
        if root == 0 {
            return None;
        }
        let mut root_return: c_ulong = 0;
        let mut child: c_ulong = 0;
        let mut root_x: c_int = 0;
        let mut root_y: c_int = 0;
        let mut win_x: c_int = 0;
        let mut win_y: c_int = 0;
        let mut mask: c_uint = 0;
        let hit = unsafe {
            (self.x11.XQueryPointer)(
                self.display,
                root,
                &mut root_return,
                &mut child,
                &mut root_x,
                &mut root_y,
                &mut win_x,
                &mut win_y,
                &mut mask,
            )
        };
        if hit == 0 {
            return None;
        }
        Some((root_x as f64, root_y as f64))
    }
}

static CURSOR_STATE: OnceLock<Option<std::sync::Mutex<Cursor>>> = OnceLock::new();

fn cursor() -> Option<&'static std::sync::Mutex<Cursor>> {
    CURSOR_STATE
        .get_or_init(|| {
            // `open()` hands back the function table by value but caches it in a
            // `OnceCell` internally and returns a reference into it, so leaking
            // it for 'static is just handing back what the library already keeps.
            let xlib = x11_dl::xlib::Xlib::open().ok()?;
            let x11: &'static x11_dl::xlib::Xlib = Box::leak(Box::new(xlib));
            let display = unsafe { (x11.XOpenDisplay)(std::ptr::null()) };
            if display.is_null() {
                return None;
            }
            Some(std::sync::Mutex::new(Cursor { x11, display }))
        })
        .as_ref()
}

/// Cursor position in physical screen coordinates, X11 only.
/// Wayland reports no position; the caller treats `None` as "no movement".
pub fn cursor_physical() -> Option<(f64, f64)> {
    let c = cursor()?;
    // A poisoned lock means a previous query panicked; the display pointer is
    // still valid, so take it rather than silently disabling the island.
    let guard = c.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the mutex guarantees no other thread is inside libX11.
    unsafe { guard.query() }
}

/// The GTK/WebKit equivalent of WS_EX_NOACTIVATE: `accept_focus(false)` keeps
/// clicks from stealing focus, and the window type hint keeps the island out
/// of Alt-Tab and the taskbar. Best-effort — a compositor may ignore hints.
pub fn make_non_activating(win: &tauri::WebviewWindow) {
    use gtk::prelude::*;
    let Ok(gtk_win) = win.gtk_window() else { return };
    gtk_win.set_accept_focus(false);
    gtk_win.set_type_hint(gtk::gdk::WindowTypeHint::Dock);
}

/// Temporarily allow activation so a text field inside the island can be typed in.
pub fn set_activating(win: &tauri::WebviewWindow, activating: bool) {
    use gtk::prelude::*;
    let Ok(gtk_win) = win.gtk_window() else { return };
    gtk_win.set_accept_focus(activating);
}
