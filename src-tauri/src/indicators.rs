//! Turns device snapshots into what the taskbar strip and the tray show.
use crate::{
    battery::{DeviceKind, DeviceSnapshot, Level, Status},
    settings::{Colorize, IconSet, Settings},
    shell::render::{StripItem, GLYPH_BLUETOOTH, GLYPH_GAMEPAD, GLYPH_HEADPHONE, GLYPH_HEADSET, GLYPH_KEYBOARD, GLYPH_MOUSE},
};

pub const DEFAULT_HIGH: [u8; 3] = [0x22, 0xc5, 0x5e];
pub const DEFAULT_MID: [u8; 3] = [0xea, 0xb3, 0x08];
pub const DEFAULT_LOW: [u8; 3] = [0xef, 0x44, 0x44];

fn parse_hex(c: &Option<String>) -> Option<[u8; 3]> {
    let h = c.as_deref()?.strip_prefix('#')?;
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some([v(0)?, v(2)?, v(4)?])
}

/// Colour for a battery level according to the configured thresholds.
pub fn level_color(s: &Settings, percent: u8) -> [u8; 3] {
    let a = &s.appearance;
    if percent >= a.high_threshold {
        parse_hex(&a.colors.high).unwrap_or(DEFAULT_HIGH)
    } else if percent >= a.low_threshold {
        parse_hex(&a.colors.mid).unwrap_or(DEFAULT_MID)
    } else {
        parse_hex(&a.colors.low).unwrap_or(DEFAULT_LOW)
    }
}

pub fn status_label(s: &DeviceSnapshot) -> String {
    match s.status {
        Status::Unknown => "Checking…".into(),
        Status::Online if s.charging => "Connected, charging".into(),
        Status::Online => "Connected".into(),
        Status::Asleep if s.discovered => "Connected, no battery level reported".into(),
        Status::Asleep => "Asleep, showing last known level".into(),
        Status::Off => "Device off or out of range".into(),
        Status::Disconnected if s.discovered => "Not connected".into(),
        Status::Disconnected => "Receiver not connected".into(),
        Status::Error => format!("Error: {}", s.error.as_deref().unwrap_or("unknown")),
        Status::Disabled => "Monitoring disabled".into(),
    }
}

/// Local wall-clock "HH:MM" for a Unix-millisecond timestamp.
pub fn local_time(ms: u64) -> String {
    use windows_sys::Win32::{
        Foundation::{FILETIME, SYSTEMTIME},
        System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime},
    };
    let ticks = (ms + 11_644_473_600_000) * 10_000; // 100 ns since 1601
    let ft = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    unsafe {
        let (mut utc, mut local): (SYSTEMTIME, SYSTEMTIME) = (std::mem::zeroed(), std::mem::zeroed());
        if FileTimeToSystemTime(&ft, &mut utc) == 0 || SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) == 0 {
            return "--:--".into();
        }
        format!("{:02}:{:02}", local.wHour, local.wMinute)
    }
}

/// Battery value as text, e.g. "64%" or "Low".
pub fn value(s: &DeviceSnapshot) -> Option<String> {
    s.percent.map(|p| format!("{p}%")).or_else(|| s.level.map(|l| l.label().to_owned()))
}

pub fn tooltip(s: &DeviceSnapshot) -> String {
    let level = value(s).map_or("Battery unknown".to_string(), |v| format!("Battery {v}"));
    let updated = s.last_update.map_or("never updated".to_string(), |t| format!("updated {}", local_time(t)));
    format!("{}\n{level}, {updated}\n{}", s.name, status_label(s))
}

/// Percentage for colours and bars; steps are placed where their colour fits.
fn approx(s: &DeviceSnapshot) -> Option<u8> {
    s.percent.or(s.level.map(|l| match l {
        Level::Empty => 0,
        Level::Low => 20,
        Level::Medium => 55,
        Level::Full => 100,
    }))
}

fn visible<'a>(settings: &'a Settings, snaps: &'a [DeviceSnapshot]) -> impl Iterator<Item = &'a DeviceSnapshot> {
    // Discovered devices disappear while away; fixed ones keep their last value.
    snaps.iter().filter(move |s| s.known && s.enabled && (!s.discovered || s.status != Status::Disconnected) && settings.device(&s.id).show_in_indicator)
}

pub fn strip_items(settings: &Settings, snaps: &[DeviceSnapshot]) -> Vec<StripItem> {
    visible(settings, snaps)
        .map(|s| {
            let glyph = match (settings.appearance.icon_set, s.kind) {
                (IconSet::Text, _) => None,
                (_, DeviceKind::Mouse) => Some(GLYPH_MOUSE),
                (_, DeviceKind::Keyboard) => Some(GLYPH_KEYBOARD),
                (IconSet::Alternate, DeviceKind::Headset) => Some(GLYPH_HEADSET),
                (_, DeviceKind::Headset) => Some(GLYPH_HEADPHONE),
                (_, DeviceKind::Gamepad) => Some(GLYPH_GAMEPAD),
                (_, DeviceKind::Other) => Some(GLYPH_BLUETOOTH),
            };
            let label = match s.kind {
                DeviceKind::Mouse => "M",
                DeviceKind::Keyboard => "K",
                DeviceKind::Headset => "H",
                DeviceKind::Gamepad => "G",
                DeviceKind::Other => "B",
            };
            let color = approx(s).and_then(|p| match settings.indicators.colorize {
                Colorize::Off => None,
                Colorize::LowOnly => (p < settings.appearance.low_threshold).then(|| level_color(settings, p)),
                Colorize::All => Some(level_color(settings, p)),
            });
            StripItem {
                glyph,
                label: label.into(),
                text: value(s).unwrap_or_else(|| "--".into()),
                color,
                dim: s.status != Status::Online,
                charging: s.charging,
                tooltip: tooltip(s),
            }
        })
        .collect()
}

/// Data for one fallback tray icon.
pub struct TrayLevel {
    pub id: String,
    pub text: String,
    pub level: Option<u8>,
    pub color: [u8; 3],
    pub dim: bool,
    pub tooltip: String,
    pub visible: bool,
}

pub fn tray_levels(settings: &Settings, snaps: &[DeviceSnapshot]) -> Vec<TrayLevel> {
    let shown: Vec<&str> = visible(settings, snaps).map(|s| s.id.as_str()).collect();
    snaps
        .iter()
        .filter(|s| s.known)
        .map(|s| TrayLevel {
            id: s.id.clone(),
            text: s.percent.map_or_else(|| s.level.map_or("--".into(), |l| l.label()[..1].into()), |p| p.to_string()),
            level: approx(s),
            color: approx(s).map_or(DEFAULT_MID, |p| level_color(settings, p)),
            dim: s.status != Status::Online,
            tooltip: tooltip(s),
            visible: shown.contains(&s.id.as_str()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(status: Status, percent: Option<u8>) -> DeviceSnapshot {
        DeviceSnapshot {
            id: "wlmouse".into(),
            name: "Mouse".into(),
            default_name: "Mouse".into(),
            known: true,
            discovered: false,
            kind: DeviceKind::Mouse,
            connection: "2.4 GHz receiver",
            status,
            percent,
            level: None,
            charging: false,
            millivolts: None,
            last_update: None,
            last_attempt: None,
            source: String::new(),
            hardware: None,
            product: None,
            error: None,
            enabled: true,
        }
    }

    #[test]
    fn colours_follow_thresholds() {
        let s = Settings::default();
        assert_eq!(level_color(&s, 70), DEFAULT_HIGH);
        assert_eq!(level_color(&s, 69), DEFAULT_MID);
        assert_eq!(level_color(&s, 30), DEFAULT_MID);
        assert_eq!(level_color(&s, 29), DEFAULT_LOW);
        let mut c = Settings::default();
        c.appearance.colors.low = Some("#0000ff".into());
        assert_eq!(level_color(&c, 1), [0, 0, 255]);
    }

    #[test]
    fn strip_shows_last_value_dimmed_and_never_invents() {
        let s = Settings::default();
        let items = strip_items(&s, &[snap(Status::Asleep, Some(35)), snap(Status::Disconnected, None)]);
        assert_eq!((items[0].text.as_str(), items[0].dim), ("35%", true));
        assert_eq!((items[1].text.as_str(), items[1].dim), ("--", true));
        let online = strip_items(&s, &[snap(Status::Online, Some(82))]);
        assert_eq!((online[0].text.as_str(), online[0].dim, online[0].color), ("82%", false, None));
    }

    #[test]
    fn colorize_modes() {
        let mut s = Settings::default();
        assert_eq!(strip_items(&s, &[snap(Status::Online, Some(10))])[0].color, Some(DEFAULT_LOW));
        s.indicators.colorize = Colorize::Off;
        assert_eq!(strip_items(&s, &[snap(Status::Online, Some(10))])[0].color, None);
        s.indicators.colorize = Colorize::All;
        assert_eq!(strip_items(&s, &[snap(Status::Online, Some(90))])[0].color, Some(DEFAULT_HIGH));
    }

    #[test]
    fn absent_and_unknown_devices_are_not_shown() {
        let s = Settings::default();
        let gone = DeviceSnapshot { discovered: true, ..snap(Status::Disconnected, Some(50)) };
        let never_seen = DeviceSnapshot { known: false, ..snap(Status::Disconnected, None) };
        assert!(strip_items(&s, &[gone, never_seen]).is_empty());
    }

    #[test]
    fn steps_are_shown_as_text() {
        let s = Settings::default();
        let pad = DeviceSnapshot { kind: DeviceKind::Gamepad, level: Some(Level::Low), ..snap(Status::Online, None) };
        let item = &strip_items(&s, &[pad])[0];
        assert_eq!((item.text.as_str(), item.color), ("Low", Some(DEFAULT_LOW)));
    }

    #[test]
    fn hidden_devices_are_skipped() {
        let mut s = Settings::default();
        s.devices.get_mut("wlmouse").unwrap().show_in_indicator = false;
        assert!(strip_items(&s, &[snap(Status::Online, Some(50))]).is_empty());
    }
}
