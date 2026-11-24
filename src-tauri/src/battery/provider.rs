//! Common interface implemented by every device provider.
use super::DeviceKind;
use hidapi::{HidApi, HidDevice};

/// The receiver (or cable connection) a provider found during a poll.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Name reported by the USB descriptor, cleaned up.
    pub product: String,
    /// "VID:PID".
    pub hardware: String,
    /// Where the value comes from, e.g. "HID feature report, usage page FFFF".
    pub source: String,
    /// Overrides the provider's default connection label (e.g. "USB cable").
    pub connection: Option<&'static str>,
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

/// A HID handle that pushes unsolicited reports, plus a filter that says which reports mean
/// "something changed, poll again" (e.g. a charging cable was plugged in).
pub struct EventSource {
    pub device: HidDevice,
    pub is_change: fn(&[u8]) -> bool,
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
    /// Cheap identity of the matching USB devices currently present (empty = none). A change
    /// (receiver plugged, cable attached) triggers an immediate poll.
    fn signature(&self, api: &HidApi) -> String;
    /// Locates the receiver and reads the battery.
    fn poll(&mut self, api: &HidApi) -> Poll;
    /// Optional handle for change notifications pushed by the device.
    fn event_source(&self, _api: &HidApi) -> Option<EventSource> {
        None
    }
}

/// Removes vendor noise such as "RECEIVER" and repeated spaces from USB product strings.
pub fn clean_product(s: &str) -> String {
    let words: Vec<&str> = s.split_whitespace().filter(|w| !w.eq_ignore_ascii_case("receiver")).collect();
    words.join(" ")
}

/// Sorted, de-duplicated "VID:PID" list of devices accepted by `matches`.
pub fn signature_of(api: &HidApi, matches: impl Fn(&hidapi::DeviceInfo) -> bool) -> String {
    let mut ids: Vec<String> = api.device_list().filter(|d| matches(d)).map(|d| format!("{:04X}:{:04X}", d.vendor_id(), d.product_id())).collect();
    ids.sort();
    ids.dedup();
    ids.join(",")
}
