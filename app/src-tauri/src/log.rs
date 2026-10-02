// Small append-only log at $XDG_DATA_HOME/coucou/coucou.log — the equivalent of
// nbLog() in HookServer.swift. Nothing leaves the machine.

use std::io::Write;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use crate::settings;

pub fn line(message: impl AsRef<str>) {
    let stamp = timestamp();
    let dir = settings::local_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("coucou.log");
    // Keep it from growing forever: start fresh past ~1 MB.
    if std::fs::metadata(&path).map(|m| m.len() > 1_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        // 0600 from the moment it exists. Creating it 0644 and chmod-ing after
        // `open` left a window where hook events — tool names, project paths —
        // were world-readable.
        .mode(0o600)
        .open(&path)
    {
        let _ = writeln!(file, "{stamp} {}", message.as_ref());
    }
}

/// Local wall-clock time.
fn timestamp() -> String {
    let secs = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    let mut buf = [0u8; 20];
    let len = unsafe {
        libc::strftime(
            buf.as_mut_ptr().cast(),
            buf.len(),
            c"%Y-%m-%d %H:%M:%S".as_ptr(),
            &tm,
        )
    };
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn timestamp_shape() {
        let t = super::timestamp();
        assert_eq!(t.len(), 19, "YYYY-MM-DD HH:MM:SS, got {t}");
    }
}
