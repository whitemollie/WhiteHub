//! IPC commands for the popup UI. The UI never touches devices directly.
use crate::{
    app::{self, AppState},
    battery::{manager::Cmd, DeviceSnapshot},
    logging,
    settings::Settings,
    system, window,
};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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
pub fn forget_device(app: AppHandle, id: String) -> Result<Settings, String> {
    app::forget_device(&app, &id)
}

/// Writes a description of the connected hardware to the desktop, for a device request.
/// Async so that waiting for a poll in progress does not block the window.
#[tauri::command(async)]
pub fn export_device_request(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    state.manager.send(Cmd::Report(tx));
    let body = rx.recv_timeout(std::time::Duration::from_secs(10)).map_err(|_| "the devices did not answer in time")?;
    let text = format!(
        "WhiteHub device request\nVersion: {}\nWindows build: {}\n\nDevice model (please fill in): \n\n{body}\nThis file contains no serial numbers, addresses or Bluetooth names.\nCheck it, then attach it to a device request.\n",
        app.package_info().version,
        system::windows_build()
    );
    save_to_desktop(&app, "WhiteHub-device-request", &text)
}

/// The page has painted its first frame; the window can be shown.
#[tauri::command]
pub fn window_ready(app: AppHandle) {
    window::reveal(&app, "ready");
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
    explorer().arg(&state.log_dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Explorer by absolute path: a bare name would be searched in the application folder first.
fn explorer() -> std::process::Command {
    std::process::Command::new(system::windows_dir().join("explorer.exe"))
}

/// Replaces the user's profile folder, so a report can be shared without revealing the user name.
fn redact(text: &str, profile: &str) -> String {
    if profile.len() < 4 {
        return text.to_owned();
    }
    // JSON escapes backslashes, so cover both spellings.
    text.replace(&profile.replace('\\', "\\\\"), "%USERPROFILE%").replace(profile, "%USERPROFILE%")
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
    let report = format!(
        "WhiteHub diagnostics\nVersion: {}\nWindows build: {}\nExecutable: {}\nAutostart registered: {}\nStrip embedded: {:?}\n\n== Devices ==\n{}\n\n== Settings ==\n{}\n\n== Log ==\n{}",
        app.package_info().version,
        system::windows_build(),
        std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default(),
        crate::autostart::is_enabled(),
        state.strip_available(),
        serde_json::to_string_pretty(&state.manager.snapshots()).unwrap_or_default(),
        serde_json::to_string_pretty(&settings).unwrap_or_default(),
        logging::read_all(),
    );
    save_to_desktop(&app, "WhiteHub-diagnostics", &report)
}

/// Saves a text file to the desktop (profile folder redacted) and selects it in Explorer.
fn save_to_desktop(app: &AppHandle, name: &str, text: &str) -> Result<String, String> {
    let profile = app.path().home_dir().map(|p| p.display().to_string()).unwrap_or_default();
    let dir = app.path().desktop_dir().or_else(|_| app.path().download_dir()).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = dir.join(format!("{name}-{stamp}.txt"));
    std::fs::write(&path, redact(text, &profile)).map_err(|e| e.to_string())?;
    log::info!("[app] {name} exported");
    let _ = explorer().arg(format!("/select,{}", path.display())).spawn();
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn report_hides_the_profile_folder() {
        let profile = r"C:\Users\Jane";
        let text = "log C:\\Users\\Jane\\AppData json \"C:\\\\Users\\\\Jane\\\\x\"";
        let out = redact(text, profile);
        assert!(!out.contains("Jane"), "{out}");
        assert_eq!(out.matches("%USERPROFILE%").count(), 2);
        assert_eq!(redact("abc", ""), "abc");
    }
}
