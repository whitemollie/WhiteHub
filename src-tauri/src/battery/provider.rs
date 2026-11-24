//! Common interface implemented by every device provider.
use super::DeviceKind;
use hidapi::HidApi;

/// The receiver a provider found during a poll.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Name reported by the receiver's USB descriptor, cleaned up.
    pub product: String,
    /// "VID:PID".
    pub hardware: String,
    /// Where the value comes from, e.g. "HID feature report, usage page FFFF".
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Battery { percent: u8, charging: bool, millivolts: Option<u16> },
    Asleep,
    Off,
    /// No matching receiver is plugged in.
    NotFound,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Poll {
    pub found: Option<Found>,
    pub outcome: Outcome,
}

impl Poll {
    pub fn not_found() -> Self {
        Self { found: None, outcome: Outcome::NotFound }
    }
}

/// A battery data source for one device. Providers are owned by the manager thread and never
/// shared, so they can keep caches without locking. `poll` must finish within ~2 seconds.
pub trait Provider: Send {
    fn id(&self) -> &'static str;
    fn default_name(&self) -> &'static str;
    fn kind(&self) -> DeviceKind;
    fn connection(&self) -> &'static str;
    /// Restricts discovery to one USB product ID (manual configuration); `None` = any supported.
    fn set_product_filter(&mut self, pid: Option<u16>);
    /// Cheap presence check over the already enumerated device list.
    fn is_present(&self, api: &HidApi) -> bool;
    /// Locates the receiver and reads the battery.
    fn poll(&mut self, api: &HidApi) -> Poll;
}

/// Removes vendor noise such as "RECEIVER" and repeated spaces from USB product strings.
pub fn clean_product(s: &str) -> String {
    let words: Vec<&str> = s.split_whitespace().filter(|w| !w.eq_ignore_ascii_case("receiver")).collect();
    words.join(" ")
}
