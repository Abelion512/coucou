//! The little bit of Unix the relay needs: where the socket is, and who is on
//! the other end.
//!
//! This mirrors `win.rs`. On Windows the pipe name carries the SID because the
//! named-pipe namespace is machine-wide; on Unix the socket lives under
//! `$XDG_RUNTIME_DIR` — a per-user, per-boot directory only the owner can walk —
//! and once connected we still ask the kernel who the peer is via
//! `SO_PEERCRED` before sending anything. Failing to vouch for the server
//! refuses one hook event; trusting it could hand another local account the
//! contents of every tool call.

use std::os::unix::net::UnixStream;

/// `$XDG_RUNTIME_DIR/coucou/coucou.sock`, falling back to `/tmp/coucou-<uid>.sock`.
///
/// Two guards, both from the spec:
///   * a Unix `sun_path` is 108 bytes including the NUL, so anything longer
///     would be silently truncated by `connect()` — fall back to the short
///     `/tmp` path instead;
///   * `/tmp` is world-writable, so the fallback relies on the peer check below
///     and on the server binding with 0600.
pub fn socket_path() -> String {
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let path = std::path::Path::new(&runtime).join("coucou").join("coucou.sock");
        let text = path.to_string_lossy().into_owned();
        if text.len() <= 107 {
            return text;
        }
    }
    format!("/tmp/coucou-{}.sock", nix_uid())
}

fn nix_uid() -> u32 {
    // std has no getuid; libc is already a dependency for SO_PEERCRED.
    unsafe { libc::getuid() }
}

/// True when the process listening on the other end of `stream` runs as the
/// same user we do.
pub fn peer_is_same_user(stream: &UnixStream) -> bool {
    use std::os::fd::AsRawFd;
    #[repr(C)]
    struct Ucred {
        pid: libc::pid_t,
        uid: libc::uid_t,
        gid: libc::gid_t,
    }
    let mut cred = Ucred { pid: 0, uid: u32::MAX, gid: 0 };
    let mut len = std::mem::size_of::<Ucred>() as libc::socklen_t;
    let ok = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::from_mut(&mut cred).cast(),
            &mut len as *mut libc::socklen_t,
        ) == 0
    };
    ok && cred.uid == unsafe { libc::getuid() }
}
