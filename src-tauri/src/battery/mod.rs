//! Battery monitoring: device providers and the polling manager.
pub mod bluetooth;
pub mod hyperx;
pub mod manager;
pub mod provider;
pub mod report;
pub mod scanner;
pub mod wlmouse;
pub mod xinput;

use serde::{Deserialize, Serialize};

/// Declaration order is display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    Mouse,
    Keyboard,
    Headset,
    Gamepad,
    #[default]
    Other,
}

/// Battery step of devices that do not report a percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Empty,
    Low,
    Medium,
    Full,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Empty => "Empty",
            Level::Low => "Low",
            Level::Medium => "Med",
            Level::Full => "Full",
        }
    }
    pub fn is_low(self) -> bool {
        matches!(self, Level::Empty | Level::Low)
    }
}

/// What WhiteHub currently knows about a device's link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// Not polled yet.
    Unknown,
    /// Battery read successfully on the last attempt.
    Online,
    /// Receiver present, but the device did not answer because it is sleeping (or switched off;
    /// the WLmouse receiver cannot tell these apart).
    Asleep,
    /// Receiver present, device reported or appears to be powered off.
    Off,
    /// Receiver/dongle not plugged in.
    Disconnected,
    /// Communication failed.
    Error,
    /// Monitoring disabled by the user.
    Disabled,
}

impl Status {
    /// Whether the device itself (not just its receiver) is reachable. `None` = undecided.
    pub fn linked(self) -> Option<bool> {
        match self {
            Status::Online => Some(true),
            Status::Off | Status::Disconnected => Some(false),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSnapshot {
    pub id: String,
    pub name: String,
    /// Name without the user's override.
    pub default_name: String,
    /// Has been present at least once; unknown devices are not shown.
    pub known: bool,
    /// Found by a scanner (Bluetooth, controllers) rather than by a fixed provider.
    pub discovered: bool,
    pub kind: DeviceKind,
    pub connection: &'static str,
    pub status: Status,
    /// Last known battery level; kept while the device is unreachable.
    pub percent: Option<u8>,
    /// Battery step, for devices without a percentage.
    pub level: Option<Level>,
    pub charging: bool,
    pub millivolts: Option<u16>,
    /// Unix milliseconds of the last successful battery read.
    pub last_update: Option<u64>,
    /// Unix milliseconds of the last attempt (successful or not).
    pub last_attempt: Option<u64>,
    /// Human-readable description of where the value comes from.
    pub source: String,
    /// USB VID:PID of the receiver currently used.
    pub hardware: Option<String>,
    /// Product name reported by the receiver.
    pub product: Option<String>,
    pub error: Option<String>,
    pub enabled: bool,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}
