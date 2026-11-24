//! Application lifecycle: wiring of settings, battery manager, shell integration, tray and window.
use crate::{
    autostart,
    battery::{hyperx::HyperX, manager::{Cmd, Manager}, wlmouse::WlMouse, DeviceSnapshot},
    commands, indicators, logging, notifications,
    settings::{IndicatorMode, Settings, Store},
    shell::{self, Shell, ShellEvent, StripModel},
    tray::{self, TrayState},
    window,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager as _, RunEvent, WindowEvent};

pub const AUTOSTART_ARG: &str = "--autostart";

pub struct AppState {
    settings: Mutex<Settings>,
    pub store: Store,
    pub manager: Manager,
    pub shell: Shell,
    pub log_dir: PathBuf,
    /// `None` until the shell reports whether embedding worked.
    strip_available: Mutex<Option<bool>>,
    light_taskbar: AtomicBool,
}

impl AppState {
    pub fn settings(&self) -> Settings {
        self.settings.lock().map(|s| s.clone()).unwrap_or_default()
    }
    pub fn strip_available(&self) -> Option<bool> {
        self.strip_available.lock().ok().and_then(|s| *s)
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| window::open(app, page_arg(&args).as_deref())))
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::update_settings,
            commands::default_settings,
            commands::refresh,
            commands::fit_window,
            commands::close_window,
            commands::export_diagnostics,
            commands::open_logs_folder,
            commands::quit,
        ])
        .setup(|app| {
            setup(app.handle())?;
            Ok(())
        })
        .on_window_event(|w, e| {
            if let WindowEvent::Destroyed = e {
                log::info!("[window] closed, WebView released");
                trim_memory_later(Duration::from_secs(3));
                let _ = w;
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build WhiteHub")
        .run(|app, event| match event {
            // Closing the popup must not end the app; only an explicit exit (code set) does.
            RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            RunEvent::Exit => shutdown(app),
            _ => {}
        });
}

fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = app.path().app_config_dir()?;
    let log_dir = app.path().app_log_dir()?;
    logging::init(&log_dir, false);
    let store = Store::new(&config_dir);
    let settings = store.load();
    logging::set_debug(settings.debug_logging);
    log::info!("[app] WhiteHub {} starting (config {})", app.package_info().version, store.path().display());

    sync_autostart(settings.autostart);

    let h = app.clone();
    let shell = Shell::start(Box::new(move |e| on_shell_event(&h, e)));
    let h = app.clone();
    let manager = Manager::start(
        vec![Box::new(WlMouse::new()), Box::new(HyperX::new())],
        &settings,
        Box::new(move |snaps, events| on_devices(&h, snaps, events)),
    );
    let light = shell::taskbar_is_light();
    let trays = tray::create(app, light)?;
    app.manage(TrayState(Mutex::new(Some(trays))));
    app.manage(AppState {
        settings: Mutex::new(settings),
        store,
        manager,
        shell,
        log_dir,
        strip_available: Mutex::new(None),
        light_taskbar: AtomicBool::new(light),
    });
    refresh_indicators(app);
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == AUTOSTART_ARG) {
        window::open(app, page_arg(&args).as_deref());
    }
    trim_memory_later(Duration::from_secs(10));
    Ok(())
}

/// `--page <dashboard|devices|appearance|settings|about>` opens the window on that page.
fn page_arg(args: &[String]) -> Option<String> {
    let i = args.iter().position(|a| a == "--page")?;
    args.get(i + 1).filter(|p| ["dashboard", "devices", "appearance", "settings", "about"].contains(&p.as_str())).cloned()
}

fn shutdown(app: &AppHandle) {
    log::info!("[app] shutting down");
    if let Some(s) = app.try_state::<AppState>() {
        s.shell.shutdown();
        s.manager.shutdown();
    }
    log::info!("[app] stopped");
}

pub fn sync_autostart(wanted: bool) {
    if autostart::is_enabled() != wanted {
        match autostart::set(wanted) {
            Ok(()) => log::info!("[app] autostart {}", if wanted { "enabled" } else { "disabled" }),
            Err(e) => log::error!("[app] cannot change autostart: {e}"),
        }
    }
}

fn on_shell_event(app: &AppHandle, e: ShellEvent) {
    match e {
        ShellEvent::Clicked => window::open(app, None),
        ShellEvent::DevicesChanged => with_state(app, |s| s.manager.send(Cmd::DevicesChanged)),
        ShellEvent::Resumed => with_state(app, |s| s.manager.send(Cmd::Resumed)),
        ShellEvent::ThemeChanged(light) => {
            with_state(app, |s| s.light_taskbar.store(light, Ordering::Relaxed));
            refresh_indicators(app);
        }
        ShellEvent::StripAvailable(ok) => {
            with_state(app, |s| {
                if let Ok(mut a) = s.strip_available.lock() {
                    *a = Some(ok);
                }
            });
            refresh_indicators(app);
        }
    }
}

fn with_state(app: &AppHandle, f: impl FnOnce(&AppState)) {
    if let Some(s) = app.try_state::<AppState>() {
        f(&s);
    }
}

fn on_devices(app: &AppHandle, snaps: &[DeviceSnapshot], events: Vec<crate::battery::manager::Event>) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let settings = state.settings();
    notifications::send(app, &settings.notifications, &events);
    let _ = app.emit_to(window::LABEL, "devices", snaps);
    refresh_indicators_with(app, &settings, snaps);
}

pub fn refresh_indicators(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let settings = state.settings();
    let snaps = state.manager.snapshots();
    refresh_indicators_with(app, &settings, &snaps);
}

/// Updates the taskbar strip and the tray icons for the current snapshots and settings.
fn refresh_indicators_with(app: &AppHandle, settings: &Settings, snaps: &[DeviceSnapshot]) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let taskbar_mode = settings.indicators.mode == IndicatorMode::Taskbar;
    state.shell.set_model(StripModel { enabled: taskbar_mode, items: indicators::strip_items(settings, snaps) });
    // Tray icons stand in when the user chose them or the strip could not be embedded.
    let show_tray = !taskbar_mode || state.strip_available() == Some(false);
    let light = state.light_taskbar.load(Ordering::Relaxed);
    let levels = indicators::tray_levels(settings, snaps);
    let summary = summary(snaps);
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        tray::with(&app2, |t| {
            t.set_theme(light);
            t.set_summary(&summary);
            t.set_levels(&app2, &levels, show_tray, light);
        });
    });
}

fn summary(snaps: &[DeviceSnapshot]) -> String {
    let mut s = String::from("WhiteHub");
    for d in snaps.iter().filter(|d| d.enabled) {
        s.push_str(&format!("\n{}: {}", d.name, d.percent.map_or("--".into(), |p| format!("{p}%"))));
    }
    s.chars().take(120).collect()
}

/// Applies new settings: validate, persist, and push to every subsystem.
pub fn apply_settings(app: &AppHandle, mut new: Settings) -> Result<Settings, String> {
    let state = app.try_state::<AppState>().ok_or("not ready")?;
    for fix in new.validate() {
        log::warn!("[settings] {fix}");
    }
    let old = state.settings();
    state.store.save(&new).map_err(|e| format!("cannot save settings: {e}"))?;
    if let Ok(mut s) = state.settings.lock() {
        *s = new.clone();
    }
    if old.debug_logging != new.debug_logging {
        logging::set_debug(new.debug_logging);
        log::info!("[app] debug logging {}", if new.debug_logging { "on" } else { "off" });
    }
    if old.autostart != new.autostart {
        sync_autostart(new.autostart);
    }
    if old.poll_interval_minutes != new.poll_interval_minutes || old.devices != new.devices {
        state.manager.send(Cmd::Configure(Box::new(new.clone())));
    } else {
        refresh_indicators(app);
    }
    if old.indicators.mode != new.indicators.mode {
        log::info!("[app] indicator mode {:?}", new.indicators.mode);
    }
    Ok(new)
}

/// Returns pages the process no longer touches (e.g. after the WebView closed) to the OS.
fn trim_memory_later(after: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(after);
        unsafe {
            use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
            SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
        }
    });
}
