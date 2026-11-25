//! The popup window: created on demand above the notification area and *destroyed* when closed,
//! so the WebView2 processes (~150 MB) exist only while the user looks at it.
//!
//! WebView2 needs a visible window to render, but takes ~0.5 s to paint its first frame. The window
//! is therefore first shown *cloaked* (DWM keeps it off screen) and revealed only once the page
//! reports that it has painted. It then fades in as a whole (frame, shadow and content together)
//! with a short upward slide, and fades out the same way. A translucent (acrylic) window skips the
//! fade, because layered windows cannot show the acrylic backdrop.
use crate::{
    app::AppState,
    battery::manager::Cmd,
    settings::{Settings, ThemeMode},
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{
    window::{Color, Effect, EffectsBuilder},
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use windows_sys::Win32::{
    Foundation::{HWND, POINT},
    Graphics::{
        Dwm::{DwmFlush, DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_TRANSITIONS_FORCEDISABLED},
        Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    },
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::{
            GetCursorPos, GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, LWA_ALPHA, SWP_ASYNCWINDOWPOS,
            SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_LAYERED,
        },
    },
};

pub const LABEL: &str = "main";
/// One size for every page; longer pages scroll.
const WIDTH: f64 = 640.0;
const HEIGHT: f64 = 560.0;
const MARGIN: f64 = 12.0;
/// Data older than this is refreshed when the popup opens.
const STALE_AFTER: Duration = Duration::from_secs(60);
/// Reveal the window even if the page never reports its first paint.
const REVEAL_FALLBACK: Duration = Duration::from_millis(2500);
/// The page reports its first frame slightly before DWM has it; measured gap < 60 ms.
const PAINT_SETTLE: Duration = Duration::from_millis(100);
const FADE_IN: Duration = Duration::from_millis(160);
/// Upward slide while fading in, in logical pixels.
const SLIDE: f64 = 8.0;
const FADE_OUT: Duration = Duration::from_millis(120);

const NONE: u8 = 0;
const LOADING: u8 = 1;
const SHOWN: u8 = 2;
const CLOSING: u8 = 3;
static PHASE: AtomicU8 = AtomicU8::new(NONE);
/// Identifies a window instance, so a late timer never touches a newer window.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Whether the current window fades (opaque) or uses the acrylic backdrop.
static FADES: AtomicBool = AtomicBool::new(true);
static OPENED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Page the next window should start on; consumed by the UI's first `get_state`.
static INITIAL_PAGE: Mutex<Option<String>> = Mutex::new(None);

struct WorkArea {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    scale: f64,
}

/// Work area of the monitor under the cursor (the user just clicked the taskbar there).
fn cursor_work_area() -> Option<WorkArea> {
    unsafe {
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        let monitor = MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST);
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

fn set_dwm_flag(w: &WebviewWindow, attribute: i32, on: bool) {
    let Ok(hwnd) = w.hwnd() else { return };
    let value: i32 = on.into();
    unsafe { DwmSetWindowAttribute(hwnd.0 as _, attribute as u32, (&value as *const i32).cast(), 4) };
}

fn set_alpha(w: &WebviewWindow, alpha: u8) {
    let Ok(hwnd) = w.hwnd() else { return };
    unsafe { SetLayeredWindowAttributes(hwnd.0 as _, 0, alpha, LWA_ALPHA) };
}

fn background(s: &Settings) -> Color {
    let custom = s.appearance.colors.background.as_deref().and_then(|c| {
        let v = |i: usize| u8::from_str_radix(c.get(i..i + 2)?, 16).ok();
        Some(Color(v(1)?, v(3)?, v(5)?, 255))
    });
    custom.unwrap_or(match s.appearance.mode {
        ThemeMode::Dark => Color(0x0a, 0x0a, 0x0a, 255),
        ThemeMode::Light => Color(0xff, 0xff, 0xff, 255),
    })
}

pub fn take_initial_page() -> Option<String> {
    INITIAL_PAGE.lock().ok().and_then(|mut p| p.take())
}

fn set_initial_page(page: String) {
    if let Ok(mut p) = INITIAL_PAGE.lock() {
        *p = Some(page);
    }
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
        let loading = PHASE.load(Ordering::SeqCst) == LOADING;
        if let Some(p) = page {
            let _ = w.emit("navigate", &p);
            if loading {
                set_initial_page(p);
            }
        }
        // Still loading: it reveals itself. Closing: reuse the window that is fading out.
        if !loading {
            PHASE.store(SHOWN, Ordering::SeqCst);
            if FADES.load(Ordering::SeqCst) {
                set_alpha(&w, 255);
            }
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
        return;
    }
    let settings = state.settings();
    let page = page.unwrap_or_else(|| settings.ui.last_page.clone());
    let translucent = settings.appearance.opacity < 0.999;
    let area = cursor_work_area();
    let height = area.as_ref().map_or(HEIGHT, |a| HEIGHT.min(f64::from(a.bottom - a.top) / a.scale - 2.0 * MARGIN));
    set_initial_page(page.clone());
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
    b = if translucent {
        b.effects(EffectsBuilder::new().effect(Effect::Acrylic).build())
    } else {
        b.background_color(background(&settings))
    };
    let w = match b.build() {
        Ok(w) => w,
        Err(e) => return log::error!("[window] cannot create window: {e}"),
    };
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    PHASE.store(LOADING, Ordering::SeqCst);
    FADES.store(!translucent, Ordering::SeqCst);
    if let Ok(mut t) = OPENED_AT.lock() {
        *t = Some(Instant::now());
    }
    // Place by the real outer size: it includes the invisible frame that carries the shadow.
    if let (Some(a), Ok(outer)) = (area, w.outer_size()) {
        let m = (MARGIN * a.scale) as i32;
        let x = (a.right - outer.width as i32 - m).max(a.left);
        let y = (a.bottom - outer.height as i32 - m).max(a.top);
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    set_dwm_flag(&w, DWMWA_TRANSITIONS_FORCEDISABLED, true);
    set_dwm_flag(&w, DWMWA_CLOAK, true);
    let _ = w.show();
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(REVEAL_FALLBACK);
        if GENERATION.load(Ordering::SeqCst) == generation {
            reveal(&app2, "timeout");
        }
    });
    log::info!("[window] opened ({page})");
}

/// Fades the loaded window in. Does nothing if it is already shown.
pub fn reveal(app: &AppHandle, reason: &'static str) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(PAINT_SETTLE);
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            let Some(w) = app2.get_webview_window(LABEL) else { return };
            if PHASE.compare_exchange(LOADING, SHOWN, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                return;
            }
            let (Ok(hwnd), Ok(pos)) = (w.hwnd(), w.outer_position()) else { return };
            let slide = (SLIDE * w.scale_factor().unwrap_or(1.0)).round() as i32;
            let raw = hwnd.0 as isize;
            let fades = FADES.load(Ordering::SeqCst);
            if fades {
                let h = raw as HWND;
                unsafe { SetWindowLongPtrW(h, GWL_EXSTYLE, GetWindowLongPtrW(h, GWL_EXSTYLE) | WS_EX_LAYERED as isize) };
                set_alpha(&w, 0);
            }
            set_dwm_flag(&w, DWMWA_TRANSITIONS_FORCEDISABLED, false);
            set_dwm_flag(&w, DWMWA_CLOAK, false);
            let _ = w.set_focus();
            let ms = OPENED_AT.lock().ok().and_then(|t| t.map(|t| t.elapsed().as_millis())).unwrap_or(0);
            log::info!("[window] revealed after {ms} ms ({reason})");
            if fades {
                let generation = GENERATION.load(Ordering::SeqCst);
                std::thread::spawn(move || {
                    animate(FADE_IN, SHOWN, generation, |t| {
                        let eased = 1.0 - (1.0 - t).powi(3);
                        let y = pos.y + (f64::from(slide) * (1.0 - eased)).round() as i32;
                        unsafe {
                            SetWindowPos(raw as HWND, std::ptr::null_mut(), pos.x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
                            SetLayeredWindowAttributes(raw as HWND, 0, (eased * 255.0).round() as u8, LWA_ALPHA);
                        }
                    })
                });
            }
        });
    });
}

/// Calls `step` with progress 0..=1 once per screen refresh. Returns early (false) if the window
/// was replaced or left `phase`, e.g. closed while fading in.
fn animate(duration: Duration, phase: u8, generation: u64, step: impl Fn(f64)) -> bool {
    let start = Instant::now();
    loop {
        if GENERATION.load(Ordering::SeqCst) != generation || PHASE.load(Ordering::SeqCst) != phase {
            return false;
        }
        let t = (start.elapsed().as_secs_f64() / duration.as_secs_f64()).min(1.0);
        step(t);
        if t >= 1.0 {
            return true;
        }
        unsafe { DwmFlush() };
    }
}

/// Fades the window out, then destroys it to release WebView2.
pub fn close(app: &AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(w) = app2.get_webview_window(LABEL) else { return };
        let was = PHASE.swap(CLOSING, Ordering::SeqCst);
        if was == CLOSING {
            return; // already fading out
        }
        let Ok(hwnd) = w.hwnd() else { return };
        let raw = hwnd.0 as isize;
        let fades = FADES.load(Ordering::SeqCst) && was == SHOWN;
        let generation = GENERATION.load(Ordering::SeqCst);
        std::thread::spawn(move || {
            let duration = if fades { FADE_OUT } else { Duration::from_millis(1) };
            let done = animate(duration, CLOSING, generation, |t| {
                if fades {
                    unsafe { SetLayeredWindowAttributes(raw as HWND, 0, ((1.0 - t) * 255.0).round() as u8, LWA_ALPHA) };
                }
            });
            if !done {
                return; // reopened meanwhile
            }
            let app3 = app2.clone();
            let _ = app2.run_on_main_thread(move || {
                if GENERATION.load(Ordering::SeqCst) == generation && PHASE.load(Ordering::SeqCst) == CLOSING {
                    if let Some(w) = app3.get_webview_window(LABEL) {
                        let _ = w.destroy();
                    }
                }
            });
        });
    });
}

pub fn on_destroyed() {
    PHASE.store(NONE, Ordering::SeqCst);
}
