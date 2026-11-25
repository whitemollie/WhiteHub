//! Sources that discover any number of devices by themselves, in contrast to `Provider`, which
//! knows one specific device.
use super::{DeviceKind, Level};

/// A device as a scanner sees it right now.
#[derive(Clone, Debug, PartialEq)]
pub struct Scanned {
    /// Stable across restarts; starts with the scanner's prefix.
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub connected: bool,
    pub reading: Option<Reading>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reading {
    Percent(u8),
    /// The device reports only coarse steps.
    Step(Level),
}

pub trait Scanner: Send {
    fn prefix(&self) -> &'static str;
    /// Where the values come from, shown to the user.
    fn source(&self) -> &'static str;
    /// `taken` lists devices other scanners already found, to avoid duplicates.
    fn scan(&mut self, taken: &[Scanned]) -> Vec<Scanned>;
}

/// Connection label of a scanned device ID; `None` for IDs of fixed providers.
pub fn connection_of(id: &str) -> Option<&'static str> {
    match id.split_once(':')?.0 {
        "bt" => Some("Bluetooth"),
        "xinput" => Some("Xbox wireless"),
        _ => None,
    }
}
