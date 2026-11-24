//! IPC commands for the popup UI. The UI never touches devices directly.
use crate::{
    app::{self, AppState},
    battery::{manager::Cmd, DeviceSnapshot},
    logging,
    settings::Settings,
    window,
};
use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewWindow};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    config_path: String,
    log_dir: String,
    /// `null` while unknown, `false` if the taskbar strip could not be embedded.
    strip_available: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullState {
    devices: Vec<DeviceSnapshot>,
    settings: Settings,
    app: AppInfo,
    initial_page: Option<String>,
}

#[tauri::command]
pub fn get_state(app: AppHandle, state: State<'_, AppState>) -> FullState {
    FullState {
        devices: state.manager.snapshots(),
        settings: state.settings(),
        app: AppInfo {
            version: app.package_info().version.to_string(),
            config_path: state.store.path().display().to_string(),
            log_dir: state.log_dir.display().to_string(),
            strip_available: state.strip_available(),
        },
        initial_page: window::take_initial_page(),
    }
}

#[tauri::command]
pub fn update_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    app::apply_settings(&app, settings)
}

#[tauri::command]
pub fn default_settings() -> Settings {
    Settings::default()
}

#[tauri::command]
pub fn refresh(state: State<'_, AppState>, id: Option<String>) {
    state.manager.send(Cmd::Refresh(id));
}

#[tauri::command]
pub fn fit_window(window: WebviewWindow, height: f64) {
    window::fit_content(&window, height);
}

#[tauri::command]
pub fn close_window(app: AppHandle) {
    window::close(&app);
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    log::info!("[app] exit requested from UI");
    app.exit(0);
}

#[tauri::command]
pub fn open_logs_folder(state: State<'_, AppState>) -> Result<(), String> {
    std::process::Command::new("explorer.exe").arg(&state.log_dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Writes a plain-text diagnostics report to the desktop and selects it in Explorer.
#[tauri::command]
pub fn export_diagnostics(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let mut settings = state.settings();
    for d in settings.devices.values_mut() {
        if d.custom_icon.is_some() {
            d.custom_icon = Some("<image omitted>".into());
        }
    }
    let os = std::process::Command::new("cmd").args(["/c", "ver"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let report = format!(
        "WhiteHub diagnostics\nVersion: {}\nOS: {os}\nExecutable: {}\nAutostart registered: {}\nStrip embedded: {:?}\n\n== Devices ==\n{}\n\n== Settings ==\n{}\n\n== Log ==\n{}",
        app.package_info().version,
        std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default(),
        crate::autostart::is_enabled(),
        state.strip_available(),
        serde_json::to_string_pretty(&state.manager.snapshots()).unwrap_or_default(),
        serde_json::to_string_pretty(&settings).unwrap_or_default(),
        logging::read_all(),
    );
    let dir = app.path().desktop_dir().or_else(|_| app.path().download_dir()).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = dir.join(format!("WhiteHub-diagnostics-{stamp}.txt"));
    std::fs::write(&path, report).map_err(|e| e.to_string())?;
    log::info!("[app] diagnostics exported to {}", path.display());
    let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", path.display())).spawn();
    Ok(path.display().to_string())
}
