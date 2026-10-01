// The Linux half of the island: cursor position on X11 and the
// never-takes-focus window hints. On Wayland there is no global cursor —
// the poll sees no movement and the tray menu remains the way in, which is
// the spec's Wayland fallback rather than a crash.

/// Cursor position in physical screen coordinates, X11 only.
/// Wayland reports no position; the caller treats `None` as "no movement".
pub fn cursor_physical() -> Option<(f64, f64)> {
    use mouse_position::mouse_position::Mouse;
    match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => Some((x as f64, y as f64)),
        Mouse::Error => None,
    }
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
