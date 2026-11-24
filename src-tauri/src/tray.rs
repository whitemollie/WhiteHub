//! Notification-area icons: the WhiteHub icon (always) and per-device fallback icons (shown when
//! the taskbar strip is off or unavailable).
//!
//! Windows remembers "always show" per (executable, icon UID). tray-icon assigns UIDs in creation
//! order, so icons are created once in a fixed order and afterwards only shown/hidden — never
//! removed and recreated — to keep the user's pinning.
use crate::{indicators::TrayLevel, shell::render, window};
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use windows_sys::Win32::UI::{
    HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi},
    WindowsAndMessaging::SM_CXSMICON,
};

pub struct Trays {
    app_icon: TrayIcon,
    levels: Vec<(&'static str, TrayIcon)>,
}

pub struct TrayState(pub Mutex<Option<Trays>>);

pub fn icon_size() -> u32 {
    unsafe { GetSystemMetricsForDpi(SM_CXSMICON as i32, GetDpiForSystem()).clamp(16, 64) as u32 }
}

pub fn fg(light_taskbar: bool) -> [u8; 3] {
    if light_taskbar { [0x1a, 0x1a, 0x1a] } else { [0xff, 0xff, 0xff] }
}

fn app_image(light: bool) -> Image<'static> {
    let size = icon_size();
    Image::new_owned(render::render_app_icon(size, fg(light)), size, size)
}

pub fn create(app: &AppHandle, light: bool) -> tauri::Result<Trays> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", "Open", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?,
            &MenuItem::with_id(app, "devices", "Devices", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?,
        ],
    )?;
    let app_icon = TrayIconBuilder::with_id("whitehub")
        .icon(app_image(light))
        .tooltip("WhiteHub")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id().as_ref() {
            "open" => window::open(app, None),
            "settings" => window::open(app, Some("settings")),
            "devices" => window::open(app, Some("devices")),
            "exit" => {
                log::info!("[app] exit requested from tray menu");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                window::open(tray.app_handle(), None);
            }
        })
        .build(app)?;
    Ok(Trays { app_icon, levels: Vec::new() })
}

impl Trays {
    pub fn set_theme(&self, light: bool) {
        let _ = self.app_icon.set_icon(Some(app_image(light)));
    }

    pub fn set_summary(&self, text: &str) {
        let _ = self.app_icon.set_tooltip(Some(text));
    }

    /// Shows per-device icons (`show = true`) or hides them.
    pub fn set_levels(&mut self, app: &AppHandle, levels: &[TrayLevel], show: bool, light: bool) {
        if !show {
            for (_, t) in &self.levels {
                let _ = t.set_visible(false);
            }
            return;
        }
        if self.levels.is_empty() {
            for l in levels {
                let built = TrayIconBuilder::with_id(format!("level-{}", l.id))
                    .on_tray_icon_event(|tray, e| {
                        if let TrayIconEvent::Click { button_state: MouseButtonState::Up, .. } = e {
                            window::open(tray.app_handle(), None);
                        }
                    })
                    .build(app);
                match built {
                    Ok(t) => self.levels.push((l.id, t)),
                    Err(e) => log::error!("[tray] cannot create icon for {}: {e}", l.id),
                }
            }
        }
        let size = icon_size();
        let fgc = fg(light);
        for (id, t) in &self.levels {
            let Some(l) = levels.iter().find(|l| l.id == *id) else { continue };
            let text_color = if l.dim { [0x8a, 0x8a, 0x8a] } else { fgc };
            if let Some(px) = render::render_level_icon(size, &l.text, l.level, text_color, l.color) {
                let _ = t.set_icon(Some(Image::new_owned(px, size, size)));
            }
            let _ = t.set_tooltip(Some(&l.tooltip));
            let _ = t.set_visible(l.visible);
        }
    }
}

pub fn with<R>(app: &AppHandle, f: impl FnOnce(&mut Trays) -> R) -> Option<R> {
    let state = app.try_state::<TrayState>()?;
    let mut guard = state.0.lock().ok()?;
    guard.as_mut().map(f)
}
