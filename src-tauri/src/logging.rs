//! Small file logger for the `log` facade: one file, rotated to `.1` at 512 KiB.
//! Info level by default, debug level in Debug Mode. Writes are unbuffered appends; the app
//! logs a handful of lines per poll, so this costs nothing measurable.
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

const MAX_BYTES: u64 = 512 * 1024;

struct FileLogger {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

static LOGGER: std::sync::OnceLock<FileLogger> = std::sync::OnceLock::new();

fn timestamp() -> String {
    let t = unsafe {
        let mut t = std::mem::zeroed();
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t);
        t
    };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds)
}

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::max_level() && (m.target().starts_with("whitehub") || m.level() <= log::Level::Warn)
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!("{} {:<5} {}\n", timestamp(), r.level(), r.args());
        #[cfg(debug_assertions)]
        eprint!("{line}");
        let Ok(mut guard) = self.file.lock() else { return };
        if guard.as_ref().and_then(|f| f.metadata().ok()).is_some_and(|m| m.len() > MAX_BYTES) {
            *guard = None;
            let _ = fs::rename(&self.path, self.path.with_extension("log.1"));
        }
        if guard.is_none() {
            *guard = OpenOptions::new().create(true).append(true).open(&self.path).ok();
        }
        if let Some(f) = guard.as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {}
}

pub fn init(dir: &Path, debug: bool) {
    let _ = fs::create_dir_all(dir);
    let logger = LOGGER.get_or_init(|| FileLogger { path: dir.join("whitehub.log"), file: Mutex::new(None) });
    if log::set_logger(logger).is_ok() {
        set_debug(debug);
    }
}

pub fn set_debug(debug: bool) {
    log::set_max_level(if debug { log::LevelFilter::Debug } else { log::LevelFilter::Info });
}

pub fn log_file() -> Option<PathBuf> {
    LOGGER.get().map(|l| l.path.clone())
}

/// Current log plus the rotated one, oldest first.
pub fn read_all() -> String {
    let Some(path) = log_file() else { return String::new() };
    let old = fs::read_to_string(path.with_extension("log.1")).unwrap_or_default();
    old + &fs::read_to_string(&path).unwrap_or_default()
}
