//! JSON configuration: defaults, validation and atomic persistence.
//!
//! Every field has a default (`#[serde(default)]`), so missing or older files load cleanly; values
//! are clamped by [`Settings::validate`]; a file that cannot be parsed is kept as
//! `settings.json.bad` and replaced by defaults instead of crashing the app.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const CURRENT_VERSION: u32 = 1;
pub const POLL_INTERVALS: [u32; 5] = [5, 10, 15, 30, 60];
pub const DEVICE_IDS: [&str; 2] = [crate::battery::wlmouse::ID, crate::battery::hyperx::ID];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum IndicatorMode {
    /// Text strip embedded in the taskbar, left of the tray.
    #[default]
    Taskbar,
    /// One notification-area icon per device.
    Tray,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Colorize {
    Off,
    #[default]
    LowOnly,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum IconSet {
    /// Segoe Fluent mouse + headphones.
    #[default]
    Fluent,
    /// Segoe Fluent mouse + headset with microphone.
    Alternate,
    /// Letters "M" / "H" instead of icons.
    Text,
}

/// Optional colour overrides; `None` means "use the theme default".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Colors {
    pub accent: Option<String>,
    pub background: Option<String>,
    pub card: Option<String>,
    pub text: Option<String>,
    pub text_muted: Option<String>,
    pub high: Option<String>,
    pub mid: Option<String>,
    pub low: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub mode: ThemeMode,
    pub colors: Colors,
    /// Window background opacity, 0.6–1.0.
    pub opacity: f32,
    pub icon_set: IconSet,
    /// Percentages at or above `high` use the "high" colour, below `low` the "low" colour.
    pub high_threshold: u8,
    pub low_threshold: u8,
}

impl Default for Appearance {
    fn default() -> Self {
        Self { mode: ThemeMode::Dark, colors: Colors::default(), opacity: 1.0, icon_set: IconSet::Fluent, high_threshold: 70, low_threshold: 30 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Indicators {
    pub mode: IndicatorMode,
    pub colorize: Colorize,
}

impl Default for Indicators {
    fn default() -> Self {
        Self { mode: IndicatorMode::Taskbar, colorize: Colorize::LowOnly }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Notifications {
    pub do_not_disturb: bool,
    pub on_connect: bool,
    pub on_disconnect: bool,
    pub on_low: bool,
}

impl Default for Notifications {
    fn default() -> Self {
        Self { do_not_disturb: false, on_connect: true, on_disconnect: true, on_low: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DeviceSettings {
    pub enabled: bool,
    /// Display name override.
    pub name: Option<String>,
    /// Restrict discovery to this USB product ID (hex, e.g. "A860").
    pub product_id: Option<String>,
    pub notify_low: bool,
    pub low_threshold: u8,
    pub show_in_indicator: bool,
    /// Custom image (data URL, PNG/SVG/JPEG) shown on the dashboard card.
    pub custom_icon: Option<String>,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self { enabled: true, name: None, product_id: None, notify_low: true, low_threshold: 20, show_in_indicator: true, custom_icon: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Ui {
    pub last_page: String,
}

impl Default for Ui {
    fn default() -> Self {
        Self { last_page: "dashboard".into() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub poll_interval_minutes: u32,
    pub autostart: bool,
    pub debug_logging: bool,
    pub appearance: Appearance,
    pub indicators: Indicators,
    pub notifications: Notifications,
    pub devices: BTreeMap<String, DeviceSettings>,
    pub ui: Ui,
}

impl Default for Settings {
    fn default() -> Self {
        let devices = DEVICE_IDS.iter().map(|id| (id.to_string(), DeviceSettings::default())).collect();
        Self {
            version: CURRENT_VERSION,
            poll_interval_minutes: 5,
            autostart: false,
            debug_logging: false,
            appearance: Appearance::default(),
            indicators: Indicators::default(),
            notifications: Notifications::default(),
            devices,
            ui: Ui::default(),
        }
    }
}

const MAX_ICON_BYTES: usize = 512 * 1024;
const PAGES: [&str; 5] = ["dashboard", "devices", "appearance", "settings", "about"];

fn valid_color(c: &Option<String>) -> Option<String> {
    c.as_ref().filter(|s| {
        let h = s.strip_prefix('#').unwrap_or("");
        (h.len() == 6 || h.len() == 8) && h.chars().all(|c| c.is_ascii_hexdigit())
    }).map(|s| s.to_ascii_lowercase())
}

impl Settings {
    /// Clamps or resets every value that is out of range. Returns human-readable fixes.
    pub fn validate(&mut self) -> Vec<String> {
        let mut fixes = Vec::new();
        if !POLL_INTERVALS.contains(&self.poll_interval_minutes) {
            fixes.push(format!("poll interval {} min is not allowed, using 5", self.poll_interval_minutes));
            self.poll_interval_minutes = 5;
        }
        let a = &mut self.appearance;
        if !(0.6..=1.0).contains(&a.opacity) || a.opacity.is_nan() {
            fixes.push(format!("opacity {} out of range", a.opacity));
            a.opacity = a.opacity.clamp(0.6, 1.0);
            if a.opacity.is_nan() { a.opacity = 1.0; }
        }
        if a.low_threshold >= a.high_threshold || a.high_threshold > 100 || a.low_threshold < 1 {
            fixes.push("colour thresholds reset to 30/70".into());
            a.low_threshold = 30;
            a.high_threshold = 70;
        }
        let c = &mut a.colors;
        for (name, slot) in [
            ("accent", &mut c.accent), ("background", &mut c.background), ("card", &mut c.card), ("text", &mut c.text),
            ("textMuted", &mut c.text_muted), ("high", &mut c.high), ("mid", &mut c.mid), ("low", &mut c.low),
        ] {
            let v = valid_color(slot);
            if slot.is_some() && v.is_none() { fixes.push(format!("invalid colour for {name}")); }
            *slot = v;
        }
        self.devices.retain(|k, _| DEVICE_IDS.contains(&k.as_str()));
        for id in DEVICE_IDS {
            let d = self.devices.entry(id.to_string()).or_default();
            d.low_threshold = d.low_threshold.clamp(5, 50);
            if let Some(n) = &d.name {
                let t: String = n.trim().chars().filter(|c| !c.is_control()).take(40).collect();
                d.name = (!t.is_empty()).then_some(t);
            }
            if let Some(p) = &d.product_id {
                let t = p.trim().trim_start_matches("0x").to_ascii_uppercase();
                d.product_id = (t.len() == 4 && u16::from_str_radix(&t, 16).is_ok()).then_some(t);
                if d.product_id.is_none() { fixes.push(format!("invalid product id for {id}")); }
            }
            if d.custom_icon.as_ref().is_some_and(|s| s.len() > MAX_ICON_BYTES || !s.starts_with("data:image/")) {
                fixes.push(format!("custom icon for {id} rejected"));
                d.custom_icon = None;
            }
        }
        if !PAGES.contains(&self.ui.last_page.as_str()) {
            self.ui.last_page = "dashboard".into();
        }
        self.version = CURRENT_VERSION;
        fixes
    }

    pub fn device(&self, id: &str) -> DeviceSettings {
        self.devices.get(id).cloned().unwrap_or_default()
    }
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(dir: &Path) -> Self {
        Self { path: dir.join("settings.json") }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads settings; never fails. Corrupt files are preserved as `settings.json.bad`.
    pub fn load(&self) -> Settings {
        let raw = match fs::read_to_string(&self.path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::info!("[settings] no configuration file, using defaults");
                return Settings::default();
            }
            Err(e) => {
                log::error!("[settings] cannot read {}: {e}", self.path.display());
                return Settings::default();
            }
        };
        // Editors such as Notepad or PowerShell 5 may prepend a UTF-8 byte-order mark.
        match serde_json::from_str::<Settings>(raw.trim_start_matches('\u{feff}')) {
            Ok(mut s) => {
                for fix in s.validate() {
                    log::warn!("[settings] {fix}");
                }
                s
            }
            Err(e) => {
                log::error!("[settings] configuration is corrupt ({e}); keeping a copy and using defaults");
                let _ = fs::copy(&self.path, self.path.with_extension("json.bad"));
                Settings::default()
            }
        }
    }

    /// Writes to a temp file and renames it over the old one, so a crash never leaves a torn file.
    pub fn save(&self, s: &Settings) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
        fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("whitehub-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn defaults_are_valid() {
        let mut s = Settings::default();
        assert!(s.validate().is_empty());
        assert_eq!(s.poll_interval_minutes, 5);
        assert_eq!(s.devices.len(), 2);
    }

    #[test]
    fn invalid_values_are_clamped() {
        let mut s = Settings::default();
        s.poll_interval_minutes = 7;
        s.appearance.opacity = 0.1;
        s.appearance.low_threshold = 80;
        s.appearance.colors.accent = Some("red".into());
        s.appearance.colors.card = Some("#AABBCC".into());
        s.devices.get_mut(crate::battery::wlmouse::ID).unwrap().product_id = Some("zz".into());
        s.devices.insert("unknown".into(), DeviceSettings::default());
        let fixes = s.validate();
        assert!(!fixes.is_empty());
        assert_eq!(s.poll_interval_minutes, 5);
        assert_eq!(s.appearance.opacity, 0.6);
        assert_eq!((s.appearance.low_threshold, s.appearance.high_threshold), (30, 70));
        assert_eq!(s.appearance.colors.accent, None);
        assert_eq!(s.appearance.colors.card.as_deref(), Some("#aabbcc"));
        assert_eq!(s.devices.len(), 2);
        assert_eq!(s.device(crate::battery::wlmouse::ID).product_id, None);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = serde_json::from_str(r#"{"pollIntervalMinutes": 30, "appearance": {"mode": "light"}}"#).unwrap();
        assert_eq!(s.poll_interval_minutes, 30);
        assert_eq!(s.appearance.mode, ThemeMode::Light);
        assert_eq!(s.appearance.high_threshold, 70);
        assert!(s.notifications.on_low);
    }

    #[test]
    fn corrupt_file_is_preserved_and_defaults_used() {
        let dir = temp_dir("corrupt");
        let store = Store::new(&dir);
        fs::write(store.path(), "{ not json").unwrap();
        let s = store.load();
        assert_eq!(s, Settings::default());
        assert!(dir.join("settings.json.bad").exists());
    }

    #[test]
    fn byte_order_mark_is_accepted() {
        let dir = temp_dir("bom");
        let store = Store::new(&dir);
        fs::write(store.path(), "\u{feff}{\"pollIntervalMinutes\": 10}").unwrap();
        assert_eq!(store.load().poll_interval_minutes, 10);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = temp_dir("roundtrip");
        let store = Store::new(&dir);
        let mut s = Settings::default();
        s.poll_interval_minutes = 15;
        s.appearance.mode = ThemeMode::Light;
        store.save(&s).unwrap();
        assert_eq!(store.load(), s);
        assert!(!dir.join("settings.json.tmp").exists());
    }
}
