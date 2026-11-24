//! The popup window: created on demand above the notification area and *destroyed* when closed,
//! so the WebView2 processes (~150 MB) exist only while the user looks at it.
use crate::{app::AppState, battery::manager::Cmd};
use std::time::Duration;
use tauri::{
    window::{Effect, EffectsBuilder},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use windows_sys::Win32::{
    Foundation::POINT,
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::GetCursorPos,
    },
};

pub const LABEL: &str = "main";
const WIDTH: f64 = 640.0;
const MIN_HEIGHT: f64 = 360.0;
const MAX_HEIGHT: f64 = 720.0;
const MARGIN: f64 = 12.0;
/// Data older than this is refreshed when the popup opens.
const STALE_AFTER: Duration = Duration::from_secs(60);

struct WorkArea {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    scale: f64,
}

fn work_area_from(monitor: windows_sys::Win32::Graphics::Gdi::HMONITOR) -> Option<WorkArea> {
    unsafe {
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut mi) == 0 {
            return None;
        }
        let (mut dx, mut dy) = (96u32, 96u32);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        let r = mi.rcWork;
        Some(WorkArea { left: r.left, top: r.top, right: r.right, bottom: r.bottom, scale: f64::from(dx.max(96)) / 96.0 })
    }
}

/// Work area of the monitor under the cursor (the user just clicked the taskbar there).
fn cursor_work_area() -> Option<WorkArea> {
    unsafe {
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        work_area_from(MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST))
    }
}

fn window_work_area(w: &WebviewWindow) -> Option<WorkArea> {
    let hwnd = w.hwnd().ok()?;
    unsafe { work_area_from(MonitorFromWindow(hwnd.0 as _, MONITOR_DEFAULTTONEAREST)) }
}

pub fn open(app: &AppHandle, page: Option<&str>) {
    let app2 = app.clone();
    let page = page.map(str::to_owned);
    // Window creation must happen on the main thread.
    let _ = app.run_on_main_thread(move || open_on_main(&app2, page));
}

fn open_on_main(app: &AppHandle, page: Option<String>) {
    let Some(state) = app.try_state::<AppState>() else { return };
    state.manager.send(Cmd::RefreshIfStale(STALE_AFTER));
    if let Some(w) = app.get_webview_window(LABEL) {
        if let Some(p) = &page {
            let _ = w.emit("navigate", p);
        }
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let settings = state.settings();
    let page = page.unwrap_or_else(|| settings.ui.last_page.clone());
    let translucent = settings.appearance.opacity < 0.999;
    let height = LAST_HEIGHT.lock().map(|h| *h).unwrap_or(DEFAULT_HEIGHT);
    if let Ok(mut p) = INITIAL_PAGE.lock() {
        *p = Some(page.clone());
    }
    let mut b = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("WhiteHub")
        .inner_size(WIDTH, height)
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .visible(false)
        .focused(true)
        .transparent(translucent);
    if translucent {
        b = b.effects(EffectsBuilder::new().effect(Effect::Acrylic).build());
    }
    match b.build() {
        Ok(w) => {
            // Place by the real outer size (it includes the frame used for the shadow), then show
            // right away: a hidden WebView2 does not lay out, so it could not measure itself.
            if let (Some(a), Ok(outer)) = (cursor_work_area(), w.outer_size()) {
                let m = (MARGIN * a.scale) as i32;
                let x = (a.right - outer.width as i32 - m).max(a.left);
                let y = (a.bottom - outer.height as i32 - m).max(a.top);
                let _ = w.set_position(PhysicalPosition::new(x, y));
                log::debug!(
                    "[window] work area {},{}-{},{} scale {} outer {}x{} -> {x},{y}; now at {:?}",
                    a.left, a.top, a.right, a.bottom, a.scale, outer.width, outer.height, w.outer_position()
                );
            }
            let _ = w.show();
            let _ = w.set_focus();
            log::info!("[window] opened ({page})");
        }
        Err(e) => log::error!("[window] cannot create window: {e}"),
    }
}

/// Page the next window should start on; consumed by the UI's first `get_state`.
static INITIAL_PAGE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn take_initial_page() -> Option<String> {
    INITIAL_PAGE.lock().ok().and_then(|mut p| p.take())
}

const DEFAULT_HEIGHT: f64 = 520.0;
/// Last content height, so the next window opens at its final size without a visible jump.
static LAST_HEIGHT: std::sync::Mutex<f64> = std::sync::Mutex::new(DEFAULT_HEIGHT);

/// Resizes the window to its content, keeping the bottom edge anchored above the taskbar.
pub fn fit_content(w: &WebviewWindow, css_height: f64) {
    let Some(area) = window_work_area(w) else { return };
    let scale = w.scale_factor().unwrap_or(area.scale);
    let (Ok(pos), Ok(outer), Ok(inner)) = (w.outer_position(), w.outer_size(), w.inner_size()) else { return };
    let frame_h = outer.height.saturating_sub(inner.height) as i32;
    let max = (MAX_HEIGHT * scale).min(f64::from(area.bottom - area.top) - 2.0 * MARGIN * scale);
    let h = (css_height.clamp(MIN_HEIGHT, MAX_HEIGHT) * scale).min(max).round() as i32;
    if (h - inner.height as i32).abs() <= 1 {
        return;
    }
    if let Ok(mut last) = LAST_HEIGHT.lock() {
        *last = f64::from(h) / scale;
    }
    // Anchor: the current bottom edge (keeps a dragged window where the user put it).
    let bottom = pos.y + outer.height as i32;
    let new_outer_h = h + frame_h;
    let x = pos.x.clamp(area.left, (area.right - outer.width as i32).max(area.left));
    let y = (bottom - new_outer_h).clamp(area.top, (area.bottom - new_outer_h).max(area.top));
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.set_size(PhysicalSize::new(inner.width, h as u32));
}

pub fn close(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
    }
}
