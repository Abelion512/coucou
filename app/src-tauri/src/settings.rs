// Preferences, stored as plain JSON in $XDG_CONFIG_HOME/coucou. No secret ever
// lands here — API keys live in the Secret Service keyring.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub sound_enabled: bool,
    pub sound_volume: f64,
    pub auto_close_interval: f64,
    pub absence_interval: f64,
    pub active_integrations: Vec<String>,
    /// "primary" = the main display, "cursor" = whichever display the mouse is on.
    pub screen: String,
    pub autostart: bool,
    pub hooks_installed: bool,
    /// Claude model used by the chat. Changeable in the settings window.
    /// Defaulted explicitly so a settings.json written by an older build still loads.
    #[serde(default = "default_model")]
    pub model: String,
    /// Base URL of a Messages-compatible relay (Chinese model relay, LiteLLM,
    /// corporate gateway). Empty = the official Anthropic API, which then also
    /// requires the API key.
    #[serde(default)]
    pub api_base: String,
    /// Model ids typed by hand, newest first. A relay like 9router lists a
    /// thousand-plus models, useless in a dropdown and slow to render, so the list
    /// stays what the user actually uses and the relay is only asked whether those
    /// still exist.
    #[serde(default)]
    pub recent_models: Vec<String>,
}

fn default_model() -> String {
    crate::claude::DEFAULT_MODEL.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            sound_volume: 0.5,
            auto_close_interval: 15.0,
            absence_interval: 180.0,
            active_integrations: vec![
                "integration_resend".into(),
                "integration_n8n".into(),
                "integration_vercel".into(),
                "integration_github".into(),
            ],
            screen: "primary".into(),
            autostart: false,
            hooks_installed: false,
            model: default_model(),
            api_base: String::new(),
            recent_models: Vec::new(),
        }
    }
}

/// $XDG_CONFIG_HOME/coucou.
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".config"))
                .unwrap_or_else(|| PathBuf::from("."))
        });
    base.join("coucou")
}

/// $XDG_DATA_HOME/coucou — where the hook binary and the log live.
pub fn local_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".local/share"))
                .unwrap_or_else(|| PathBuf::from("."))
        });
    base.join("coucou")
}

pub fn hook_exe_path() -> PathBuf {
    local_dir().join("bin").join("coucou-hook")
}

/// $XDG_CACHE_HOME/coucou — regenerated data. Currently the GStreamer registry,
/// which has to stay out of the shared one (see `prepare_media_environment`).
pub fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("coucou"))
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn load() -> Settings {
    match std::fs::read(settings_path()) {
        Ok(bytes) => {
            let mut s: Settings = serde_json::from_slice(&bytes).unwrap_or_default();
            // Volume was capped at 0.2 while the slider now goes to 1.0, so an
            // older stored value is still valid — but out of range it would be
            // accepted by Web Audio as a gain above 1 and clip.
            s.sound_volume = s.sound_volume.clamp(0.0, 1.0);
            s
        }
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_vec_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(settings_path(), json)
}
