//! Xbox-compatible wireless controllers through XInput, which reports only coarse battery steps.
use super::{
    scanner::{Reading, Scanned, Scanner},
    DeviceKind, Level,
};
use windows_sys::Win32::UI::Input::XboxController::{
    XInputGetBatteryInformation, BATTERY_DEVTYPE_GAMEPAD, BATTERY_LEVEL_EMPTY, BATTERY_LEVEL_FULL, BATTERY_LEVEL_LOW, BATTERY_LEVEL_MEDIUM, BATTERY_TYPE_ALKALINE,
    BATTERY_TYPE_NIMH, XINPUT_BATTERY_INFORMATION, XUSER_MAX_COUNT,
};

fn decode(kind: u8, level: u8) -> Option<Level> {
    // Wired, disconnected and unknown types carry no usable level.
    if kind != BATTERY_TYPE_ALKALINE && kind != BATTERY_TYPE_NIMH {
        return None;
    }
    match u32::from(level) {
        l if l == BATTERY_LEVEL_EMPTY as u32 => Some(Level::Empty),
        l if l == BATTERY_LEVEL_LOW as u32 => Some(Level::Low),
        l if l == BATTERY_LEVEL_MEDIUM as u32 => Some(Level::Medium),
        l if l == BATTERY_LEVEL_FULL as u32 => Some(Level::Full),
        _ => None,
    }
}

pub struct XInput;

impl Scanner for XInput {
    fn prefix(&self) -> &'static str {
        "xinput:"
    }
    fn source(&self) -> &'static str {
        "Reported by Windows (XInput)"
    }
    fn scan(&mut self, taken: &[Scanned]) -> Vec<Scanned> {
        // A controller on Bluetooth already shows up there, with an exact percentage.
        if taken.iter().any(|d| d.kind == DeviceKind::Gamepad && d.connected && d.reading.is_some()) {
            return Vec::new();
        }
        (0..XUSER_MAX_COUNT)
            .filter_map(|i| {
                let mut info = XINPUT_BATTERY_INFORMATION { BatteryType: 0, BatteryLevel: 0 };
                let rc = unsafe { XInputGetBatteryInformation(i, BATTERY_DEVTYPE_GAMEPAD, &mut info) };
                let level = (rc == 0).then(|| decode(info.BatteryType, info.BatteryLevel))??;
                Some(Scanned {
                    id: format!("xinput:{i}"),
                    name: format!("Xbox controller {}", i + 1),
                    kind: DeviceKind::Gamepad,
                    connected: true,
                    reading: Some(Reading::Step(level)),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_battery_powered_controllers_count() {
        assert_eq!(decode(BATTERY_TYPE_NIMH, BATTERY_LEVEL_LOW), Some(Level::Low));
        assert_eq!(decode(BATTERY_TYPE_ALKALINE, BATTERY_LEVEL_FULL), Some(Level::Full));
        assert_eq!(decode(0x01, BATTERY_LEVEL_FULL), None, "wired");
        assert_eq!(decode(0xff, 0), None, "unknown");
        assert_eq!(decode(BATTERY_TYPE_NIMH, 9), None);
    }

    #[test]
    fn bluetooth_controllers_are_not_listed_twice() {
        let pad = Scanned { id: "bt:x".into(), name: "Pad".into(), kind: DeviceKind::Gamepad, connected: true, reading: Some(Reading::Percent(50)) };
        assert!(XInput.scan(&[pad]).is_empty());
    }
}
