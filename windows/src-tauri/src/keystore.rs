// Keyring plumbing that differs between the two platforms. `secrets.rs` keeps
// the key policy (which names may exist) and stays platform-neutral; only the
// `Entry` construction and its failure mode are OS-specific.
//
// On Windows the Credential Manager is always available. On Linux the
// Secret Service needs a running D-Bus session (GNOME Keyring, KWallet); when
// it is absent the platform degrades to "keys cannot be stored" instead of
// crashing — exactly what the spec's Linux table asks for.

#[cfg(windows)]
pub fn entry(service: &str, key: &str) -> Option<keyring::Entry> {
    keyring::Entry::new(service, key).ok()
}

#[cfg(unix)]
pub fn entry(service: &str, key: &str) -> Option<keyring::Entry> {
    // On Linux, Entry::new() itself fails without a Secret Service backend.
    match keyring::Entry::new(service, key) {
        Ok(e) => Some(e),
        // We report the cause once, then let the caller's Option path degrade.
        Err(err) => {
            crate::log::line(format!("keyring unavailable: {err}"));
            None
        }
    }
}
