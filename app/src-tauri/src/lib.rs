// Coucou for Linux — app wiring and the commands the island calls.

mod agents;
mod claude;
mod files;
mod hooks;
mod integrations;
mod island;
mod keystore;
mod log;
mod secrets;
mod settings;
mod socket;
mod tray;

use std::process::Command;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};

use claude::{Chat, ChatContext, ChatReply};
use files::DroppedFile;
use hooks::{HookPreview, HookStatus};
use island::{PollGate, ScreenInfo};
use socket::Pending;

/// The hook relay (Unix socket). `relay` keeps the call sites readable and
/// mirrors the old pipe/socket naming without the cfg split.
use socket as relay;
use settings::Settings;

pub struct Shared {
    pub settings: Mutex<Settings>,
    pub gate: Arc<PollGate>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootInfo {
    settings: Settings,
    screen: ScreenInfo,
    version: String,
    hook_path: String,
}

#[tauri::command]
fn boot(app: AppHandle, shared: State<Shared>) -> BootInfo {
    let mut settings = shared.settings.lock().unwrap().clone();
    // The real state of ~/.claude/settings.json wins over whatever we stored.
    settings.hooks_installed = hooks::status().installed;
    let screen = island::screen_info(&app, &settings.screen);
    BootInfo {
        settings,
        screen,
        version: env!("CARGO_PKG_VERSION").to_string(),
        hook_path: settings::hook_exe_path().to_string_lossy().to_string(),
    }
}

#[tauri::command]
fn save_settings(app: AppHandle, shared: State<Shared>, settings: Settings) {
    let (screen_changed, autostart_changed) = {
        let mut current = shared.settings.lock().unwrap();
        let screen_changed = current.screen != settings.screen;
        let autostart_changed = current.autostart != settings.autostart;
        *current = settings.clone();
        (screen_changed, autostart_changed)
    };
    if let Err(err) = settings::save(&settings) {
        eprintln!("[coucou] could not save settings: {err}");
    }
    if autostart_changed {
        let manager = app.autolaunch();
        let result = if settings.autostart { manager.enable() } else { manager.disable() };
        if let Err(err) = result {
            eprintln!("[coucou] autostart: {err}");
        }
    }
    if screen_changed {
        let collapsed = shared.gate.collapsed.load(Ordering::Relaxed);
        island::apply_geometry(&app, &settings.screen, collapsed);
    }
    // Keep the other window in step (island ⇄ settings window).
    let _ = app.emit("settings-changed", settings);
}

/// Hidden island → shrink the window to the invisible wake strip and park the
/// cursor poll; anything else → full panel and 60 Hz polling.
#[tauri::command]
fn set_collapsed(app: AppHandle, shared: State<Shared>, collapsed: bool) {
    let pref = shared.settings.lock().unwrap().screen.clone();
    shared.gate.collapsed.store(collapsed, Ordering::Relaxed);
    island::apply_geometry(&app, &pref, collapsed);
    // The wake strip must always take the mouse, and a resize invalidates the flag.
    island::set_ignore_cursor(&app, false);
    shared.gate.forget_ignore_state();
    shared.gate.set_active(!collapsed);
}

/// The front end pushes the island shape; Rust decides click-through from it.
#[tauri::command]
fn set_island_rect(shared: State<Shared>, x: f64, y: f64, width: f64, height: f64) {
    shared.gate.set_rect(island::IslandRect { x, y, w: width, h: height });
}

/// The webview's real CSS-pixel size, measured in JS.
///
/// The island lays itself out in CSS pixels, so the window has to be big enough
/// to hand back exactly the panel the front end was designed around. GTK's
/// reported scale factor is not reliable at startup (see island::apply_geometry),
/// so the webview reports what it actually got and Rust corrects the window once.
#[tauri::command]
fn report_viewport(app: AppHandle, shared: State<Shared>, width: f64, height: f64) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    island::note_viewport(width, height);
    let pref = shared.settings.lock().unwrap().screen.clone();
    let collapsed = shared.gate.collapsed.load(Ordering::Relaxed);
    island::apply_geometry(&app, &pref, collapsed);
}

#[tauri::command]
fn focus_window(app: AppHandle, focused: bool) {
    let Some(win) = island::window(&app) else { return };
    island::set_activating(&win, focused);
    if focused {
        let _ = win.set_focus();
    }
}

#[tauri::command]
fn reposition(app: AppHandle, shared: State<Shared>) {
    let pref = shared.settings.lock().unwrap().screen.clone();
    let collapsed = shared.gate.collapsed.load(Ordering::Relaxed);
    island::apply_geometry(&app, &pref, collapsed);
}

#[tauri::command]
fn open_url(url: String) {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return;
    }
    open_url_impl(&url);
}

/// Linux opens URLs through the portals / xdg-open.
fn open_url_impl(url: &str) {
    let _ = Command::new("xdg-open").arg(url).spawn();
}

/// "Open terminal"/"Open editor" — the folder goes to the first editor on
/// PATH. `code` is only first because it used to be the only one; the fallback
/// ends at the file manager so the button still does something useful.
#[tauri::command]
fn open_in_editor(path: Option<String>) -> bool {
    if let Some(p) = path.as_deref().filter(|p| !p.is_empty()) {
        for launcher in ["code", "codium", "antigravity", "zed", "kate", "gedit"] {
            if Command::new(launcher).arg(p).spawn().is_ok() {
                return true;
            }
        }
        let _ = Command::new("xdg-open").arg(p).spawn();
        return true;
    }
    false
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Tray → Pause. Paused means paused: the pollers stop talking to the network,
/// not just the island stopping showing things.
#[tauri::command]
fn set_paused(paused: bool) {
    integrations::set_paused(paused);
}

// ── Claude Code hooks ─────────────────────────────────────────────────────────

#[tauri::command]
fn hooks_status() -> HookStatus {
    hooks::status()
}

/// Returns the diff the user has to look at before anything is written.
#[tauri::command]
fn hooks_preview(install: bool) -> Result<HookPreview, String> {
    hooks::preview(install)
}

/// Only ever called from an explicit click in the settings window.
#[tauri::command]
fn hooks_apply(
    app: AppHandle,
    shared: State<Shared>,
    install: bool,
    fingerprint: String,
) -> Result<String, String> {
    // The fingerprint comes from the preview the user actually looked at, so a
    // settings.json that changed in between is refused rather than overwritten.
    let backup = hooks::write(install, &fingerprint)?;
    let updated = {
        let mut current = shared.settings.lock().unwrap();
        current.hooks_installed = install;
        let _ = settings::save(&current);
        current.clone()
    };
    let _ = app.emit("settings-changed", updated);
    Ok(backup)
}

#[tauri::command]
fn approval_decision(app: AppHandle, request_id: String, decision: String) {
    relay::answer(&app, &request_id, &decision);
}

/// The island has the card on screen, so the long wait for a human may begin.
/// Until this arrives the relay only waits a few hundred milliseconds, which is
/// what stops a paused or unresponsive island from freezing Claude Code.
#[tauri::command]
fn approval_ack(app: AppHandle, request_id: String) {
    relay::acknowledge(&app, &request_id);
}

/// Nobody can act on this request — the island is paused, or another card is
/// already up. Claude Code falls back to asking in the terminal immediately.
#[tauri::command]
fn approval_decline(app: AppHandle, request_id: String) {
    relay::decline(&app, &request_id);
}

// ── Chat, files and secrets ───────────────────────────────────────────────────

/// One chat turn. The API key and any file bytes stay on the Rust side.
#[tauri::command]
async fn chat_send(
    shared: State<'_, Shared>,
    chat: State<'_, Chat>,
    query: String,
    context: Option<ChatContext>,
) -> Result<ChatReply, String> {
    let (model, api_base) = {
        let s = shared.settings.lock().unwrap();
        (s.model.clone(), s.api_base.clone())
    };
    claude::send(&chat, &model, Some(&api_base), query, context).await
}

#[tauri::command]
fn chat_reset(chat: State<Chat>) {
    chat.reset();
}

/// Copies a dropped file into the inbox and reports its name back.
#[tauri::command]
fn ingest_file(path: String) -> Result<DroppedFile, String> {
    files::ingest(&path)
}

/// The island may only ask whether a key exists — never read it.
#[tauri::command]
fn secret_present(key: String) -> bool {
    secrets::present(&key)
}

#[tauri::command]
fn secret_set(key: String, value: String) -> Result<(), String> {
    secrets::set(&key, &value)
}

#[tauri::command]
fn secret_clear(key: String) -> Result<(), String> {
    secrets::clear(&key)
}

/// Opens the configured n8n instance — the URL lives in the Secret Service keyring.
#[tauri::command]
fn open_n8n() {
    if let Some(url) = secrets::get("n8n-url") {
        open_url(url);
    }
}

/// Refresh buttons in the integration cards.
#[tauri::command]
async fn refresh_integration(app: AppHandle, id: String) {
    integrations::poll_once(app, &id).await;
}

/// The three agents' liveness, for the settings window.
///
/// The island deliberately shows nothing for an agent that is down — a permanent
/// empty pill would be noise. Settings is where "why is it missing?" belongs, so
/// this is the one surface that answers it.
#[tauri::command]
fn agents_status() -> Vec<agents::AgentStatus> {
    agents::status()
}

/// Lets the island write to the same log as the Rust side.
#[tauri::command]
fn log_line(message: String) {
    log::line(format!("ui  {message}"));
}

// ── Settings window ───────────────────────────────────────────────────────────

/// WebView2 allows exactly one browser environment per app, and its options are
/// fixed by whichever webview is created first. Every window must therefore ask
/// for the *same* arguments as the island (see `additionalBrowserArgs` in
/// tauri.conf.json) — a mismatch makes the second window come up blank, with no
/// error anywhere.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required";

/// In a dev build the pages are served by Vite, so the second window needs the
/// absolute dev URL; a bundled build resolves it inside the app bundle.
fn settings_page_url(app: &AppHandle) -> WebviewUrl {
    #[cfg(dev)]
    if let Some(mut base) = app.config().build.dev_url.clone() {
        base.set_path("/settings.html");
        return WebviewUrl::External(base);
    }
    let _ = app;
    WebviewUrl::App("settings.html".into())
}

/// The settings window is created hidden at launch and only ever shown and
/// hidden afterwards. A WebView2 window created later — on the main thread or
/// not — silently comes up blank in this app, so the window that works is the
/// one that exists before the island's webview does.
fn create_settings_window(app: &AppHandle) {
    let url = settings_page_url(app);
    match WebviewWindowBuilder::new(app, "settings", url)
        .additional_browser_args(BROWSER_ARGS)
        .title("Settings — Coucou")
        .inner_size(560.0, 680.0)
        .min_inner_size(460.0, 480.0)
        .resizable(true)
        .visible(false)
        .center()
        .build()
    {
        Ok(win) => {
            // Closing it must only hide it, or it could never be reopened.
            let hidden = win.clone();
            win.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = hidden.hide();
                }
            });
        }
        Err(err) => log::line(format!("settings window failed: {err}")),
    }
}

pub fn show_settings_window(app: &AppHandle) {
    let Some(win) = app.get_webview_window("settings") else {
        log::line("settings window missing");
        return;
    };
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
    // The window is never destroyed, so this is the only honest moment for the
    // front end to re-check something that can change while it is closed: which of
    // the model ids this machine uses are still on the relay.
    let _ = app.emit_to("settings", "settings-opened", ());
}

#[tauri::command]
fn open_settings_window(app: AppHandle) {
    show_settings_window(&app);
}

/// What the relay currently offers, judged only against the ids we already know.
///
/// A relay can list a thousand models — 9router lists 1013 — which is neither
/// renderable in a dropdown nor useful to read. So nothing is downloaded into the
/// UI: the cached ids go out, and what comes back is which of them are still
/// there, plus a count so the user can see how big the catalogue is. A model the
/// relay dropped is marked, never silently removed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelCheck {
    reachable: bool,
    total: usize,
    present: Vec<String>,
    missing: Vec<String>,
}

#[tauri::command]
async fn chat_models_check(shared: State<'_, Shared>, known: Vec<String>) -> Result<ModelCheck, ()> {
    let api_base = shared.settings.lock().unwrap().api_base.trim().to_string();
    if api_base.is_empty() {
        return Ok(ModelCheck { reachable: false, total: 0, present: known, missing: Vec::new() });
    }
    // A relay that needs a key would answer 401 here; the key is in the Secret
    // Service and is deliberately not read for a list, so "unreachable" covers it
    // rather than prompting for a credential that may not be needed.
    let text = {
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                return Ok(ModelCheck { reachable: false, total: 0, present: Vec::new(), missing: known })
            }
        };
        match client
            .get(format!("{}/models", api_base.trim_end_matches('/')))
            .send()
            .await
        {
            Ok(r) => match r.text().await {
                Ok(t) => t,
                Err(_) => {
                    return Ok(ModelCheck {
                        reachable: false,
                        total: 0,
                        present: Vec::new(),
                        missing: known,
                    })
                }
            },
            Err(_) => {
                return Ok(ModelCheck { reachable: false, total: 0, present: Vec::new(), missing: known })
            }
        }
    };
    let ids: Vec<String> = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("data").and_then(|d| d.as_array()).cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let present: Vec<String> = known.iter().filter(|k| ids.contains(k)).cloned().collect();
    let missing: Vec<String> = known.iter().filter(|k| !ids.contains(k)).cloned().collect();
    Ok(ModelCheck { reachable: true, total: ids.len(), present, missing })
}

/// Points GStreamer at the system's plugins and gives it its own registry file.
///
/// Must run before any thread or webview starts, because GStreamer reads these
/// once at init.
///
/// Inside the AppImage, `libgstreamer` is bundled but the *plugins* are not — so
/// WebKitGTK failed with "GStreamer element autoaudiosink not found" and the 28
/// sounds never played. The system's plugins are the complete set, so the AppImage
/// uses those. The registry gets its own file because the AppImage is mounted at a
/// new path every launch: sharing the system's would rewrite it with plugin paths
/// that vanish on exit.
fn prepare_media_environment() {
    if std::env::var_os("APPIMAGE").is_none() {
        return;
    }
    if let Some(plugins) = system_gstreamer_plugins() {
        // Prepend rather than replace: a plugin the system has but the AppImage
        // does not is exactly what we need, and one only the AppImage has would be
        // rare enough not to matter.
        let existing = std::env::var("GST_PLUGIN_SYSTEM_PATH_1_0").unwrap_or_default();
        let path = if existing.is_empty() { plugins } else { format!("{plugins}:{existing}") };
        std::env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", path);
    }
    if std::env::var_os("GST_REGISTRY").is_none() {
        if let Some(cache) = settings::cache_dir() {
            if std::fs::create_dir_all(&cache).is_ok() {
                std::env::set_var("GST_REGISTRY", cache.join("gstreamer-registry.bin"));
            }
        }
    }
}

/// Where this distro keeps its GStreamer plugins.
///
/// Debian/Ubuntu put them in the multiarch dir (`/usr/lib/x86_64-linux-gnu/…`),
/// which is not under `$XDG_DATA_DIRS`, so both layouts are checked. Returns every
/// directory that exists — GStreamer takes a colon-separated list, and picking the
/// wrong single one would leave the sounds silent again.
fn system_gstreamer_plugins() -> Option<String> {
    fn plugin_dir(base: &Path) -> Option<String> {
        let dir = base.join("gstreamer-1.0");
        dir.is_dir().then(|| dir.to_string_lossy().into_owned())
    }

    let mut found: Vec<String> = Vec::new();
    // The multiarch directory: named by dpkg, so ask it rather than guessing.
    if let Some(arch) = std::env::var_os("DEB_HOST_MULTIARCH") {
        if let Some(d) = plugin_dir(&PathBuf::from("/usr/lib").join(arch)) {
            found.push(d);
        }
    }
    // Otherwise find it under /usr/lib — the arch triplet is the only child there
    // that has a gstreamer-1.0 inside.
    if found.is_empty() {
        if let Ok(entries) = std::fs::read_dir("/usr/lib") {
            for entry in entries.flatten() {
                if let Some(d) = plugin_dir(&entry.path()) {
                    found.push(d);
                }
            }
        }
    }
    // And the shared locations, which is where non-Debian distros keep them.
    let dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in dirs.split(':') {
        if let Some(p) = plugin_dir(&PathBuf::from(d)) {
            found.push(p);
        }
    }

    (!found.is_empty()).then(|| found.join(":"))
}

pub fn run() {
    prepare_media_environment();
    let loaded = settings::load();
    let gate = Arc::new(PollGate::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let _ = app.emit_to(island::WINDOW_LABEL, "tray", "open".to_string());
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .manage(Shared {
            settings: Mutex::new(loaded.clone()),
            gate: gate.clone(),
        })
        .manage(Pending::default())
        .manage(Chat::default())
        .invoke_handler(tauri::generate_handler![
            boot,
            save_settings,
            set_collapsed,
            set_island_rect,
            report_viewport,
            focus_window,
            reposition,
            open_url,
            open_in_editor,
            quit_app,
            hooks_status,
            hooks_preview,
            hooks_apply,
            approval_decision,
            approval_ack,
            approval_decline,
            log_line,
            chat_send,
            chat_reset,
            ingest_file,
            secret_present,
            secret_set,
            secret_clear,
            refresh_integration,
            agents_status,
            chat_models_check,
            open_n8n,
            open_settings_window,
            set_paused,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            tray::build(&handle)?;
            // Before the island: see create_settings_window.
            create_settings_window(&handle);

            if let Some(win) = island::window(&handle) {
                island::make_non_activating(&win);
                island::apply_geometry(&handle, &loaded.screen, false);
                let _ = win.show();
            }
            gate.collapsed.store(false, Ordering::Relaxed);
            gate.set_active(true);
            island::spawn_cursor_poll(handle.clone(), gate.clone());

            log::line(format!("--- Coucou {} started ---", env!("CARGO_PKG_VERSION")));
            hooks::ensure_hook_exe(&handle);
            // The hook relay (Unix socket).
            relay::start(handle.clone());
            integrations::start(handle.clone());
            // The other agents: OpenCode, Hermes, Freebuff/Codebuff. Their
            // events funnel through the bus and reach the island as `agent`
            // events; the bus consumer owns the receiving end.
            let (bus, mut rx) = agents::AgentBus::new();
            let bus_app = handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    agents::emit(&bus_app, &event);
                }
            });
            agents::start(handle.clone(), &bus);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Coucou");
}

#[cfg(test)]
mod media_tests {
    use super::system_gstreamer_plugins;

    /// The AppImage bundles libgstreamer but none of its plugins, so without this
    /// the sounds are silent in the packaged build while working in dev.
    #[test]
    fn finds_the_system_gstreamer_plugins() {
        let found = system_gstreamer_plugins().expect("no gstreamer-1.0 found");
        assert!(found.contains("/usr/"), "expected an absolute system path, got {found}");
        for part in found.split(':') {
            assert!(std::path::Path::new(part).is_dir(), "{part} does not exist");
        }
    }
}
