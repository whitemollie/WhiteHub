//! HyperX Cloud III S Wireless over its 2.4 GHz dongle (USB 03F0:06BE; 03F0:02CC is the same
//! headset on another dongle revision according to community tools, untested).
//!
//! Protocol as documented by HyperX-Battery-Tray, hyperx-battery-logger and HyperHeadset (MIT)
//! and verified on hardware. Vendor collection usage page 01C0 / usage 0001, 64-byte reports 0x0C:
//!
//! ```text
//! battery  (output) 0C 02 03 01 00 06 | 00 …
//!          (input)  0C 02 03 01 00 06 NN VH VL …   NN percent (>100: headset off), VH:VL mV
//! charging (output) 0C 02 03 01 00 48 | 00 …
//!          (input)  0C 02 03 01 00 48 SS …         SS 0 not charging, 1 charging, 2 full
//! events   (input)  0D 02 03 00 0A xx  charging cable 1/0
//!                   0D 02 03 00 0C xx  headset on 1 / off 0
//! ```
//! Byte 3 = 01 marks a read ("GET") command; nothing is ever written to the headset.
//!
//! Written with the driver of ferloom/Wireless-Device-Battery-Monitor (GPL-3.0) as a reference.
use super::{
    hex,
    provider::{clean_product, signature_of, EventSource, Found, Outcome, Poll, Provider},
    DeviceKind,
};
use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

pub const ID: &str = "hyperx";
pub const VID: u16 = 0x03F0;
pub const PIDS: [u16; 2] = [0x06BE, 0x02CC];
const PAGE: u16 = 0x01C0;
const USAGE: u16 = 0x0001;
const HEADER: [u8; 5] = [0x0C, 0x02, 0x03, 0x01, 0x00];
const CMD_BATTERY: u8 = 0x06;
const CMD_CHARGE_STATE: u8 = 0x48;
const EVENT: [u8; 4] = [0x0D, 0x02, 0x03, 0x00];
const EV_CHARGER: u8 = 0x0A;
const EV_HEADSET: u8 = 0x0C;
const BATTERY_TIMEOUT: Duration = Duration::from_millis(1500);
const MAX_STALE_REPORTS: usize = 256;
const CHARGE_TIMEOUT: Duration = Duration::from_millis(600);

#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    /// Battery reply: `None` = headset off.
    Battery(Option<u8>, Option<u16>),
    /// Charge-state reply: 0 not charging, 1 charging, 2 fully charged.
    ChargeState(u8),
    Charger(bool),
    Headset(bool),
}

pub fn decode(r: &[u8]) -> Option<Frame> {
    if r.len() >= 7 && r[..5] == HEADER && r[5] == CMD_BATTERY {
        // All-zero payload looks exactly like our own request; a real reply always carries a voltage.
        if r[6..r.len().min(9)].iter().all(|&b| b == 0) {
            return None;
        }
        let level = (r[6] <= 100).then_some(r[6]);
        let mv = (r.len() >= 9 && level.is_some()).then(|| u16::from(r[7]) << 8 | u16::from(r[8])).filter(|&v| v > 0);
        return Some(Frame::Battery(level, mv));
    }
    if r.len() >= 7 && r[..5] == HEADER && r[5] == CMD_CHARGE_STATE && r[6] <= 2 {
        return Some(Frame::ChargeState(r[6]));
    }
    if r.len() >= 6 && r[..4] == EVENT && r[5] <= 1 {
        return match r[4] {
            EV_CHARGER => Some(Frame::Charger(r[5] == 1)),
            EV_HEADSET => Some(Frame::Headset(r[5] == 1)),
            _ => None,
        };
    }
    None
}

/// Reports pushed by the dongle that warrant a fresh poll.
fn is_change_event(r: &[u8]) -> bool {
    matches!(decode(r), Some(Frame::Charger(_) | Frame::Headset(_)))
}

pub fn query_bytes(cmd: u8) -> [u8; 64] {
    let mut q = [0u8; 64];
    q[..5].copy_from_slice(&HEADER);
    q[5] = cmd;
    q
}

pub struct HyperX {
    product_filter: Option<u16>,
}

/// What one request produced: the matching reply, or that the headset reported itself off.
enum Answer {
    Reply(Frame),
    Off,
    Timeout,
}

impl HyperX {
    pub fn new() -> Self {
        Self { product_filter: None }
    }

    fn matches(&self, vid: u16, pid: u16) -> bool {
        vid == VID && self.product_filter.map_or(PIDS.contains(&pid), |p| p == pid)
    }

    fn request(dev: &HidDevice, cmd: u8, timeout: Duration) -> Result<Answer, String> {
        let mut buf = [0u8; 64];
        dev.write(&query_bytes(cmd)).map_err(|e| e.to_string())?;
        let deadline = Instant::now() + timeout;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let n = dev.read_timeout(&mut buf, left.as_millis().max(1) as i32).map_err(|e| e.to_string())?;
            if n == 0 {
                continue;
            }
            log::debug!("[hyperx] input report: {}", hex(&buf[..n.min(12)]));
            match decode(&buf[..n]) {
                Some(f @ Frame::Battery(Some(_), _)) if cmd == CMD_BATTERY => return Ok(Answer::Reply(f)),
                Some(f @ Frame::ChargeState(_)) if cmd == CMD_CHARGE_STATE => return Ok(Answer::Reply(f)),
                Some(Frame::Battery(None, _)) | Some(Frame::Headset(false)) => return Ok(Answer::Off),
                _ => {}
            }
        }
        Ok(Answer::Timeout)
    }

    fn query(dev: &HidDevice) -> Result<Outcome, String> {
        let mut buf = [0u8; 64];
        // Drop stale input queued on this handle (bounded, whatever the device sends).
        for _ in 0..MAX_STALE_REPORTS {
            if dev.read_timeout(&mut buf, 0).map_err(|e| e.to_string())? == 0 {
                break;
            }
        }
        let (percent, millivolts) = match Self::request(dev, CMD_BATTERY, BATTERY_TIMEOUT)? {
            Answer::Reply(Frame::Battery(Some(p), mv)) => (p, mv),
            // The dongle stays enumerated while the headset is off and simply never answers.
            _ => return Ok(Outcome::Off),
        };
        // Charging cable plugged (1) or charged and still plugged (2) both show the bolt.
        let charging = match Self::request(dev, CMD_CHARGE_STATE, CHARGE_TIMEOUT)? {
            Answer::Reply(Frame::ChargeState(s)) => s != 0,
            _ => false,
        };
        Ok(Outcome::Battery { percent, charging, millivolts })
    }

    fn find<'a>(&self, api: &'a HidApi) -> Option<&'a hidapi::DeviceInfo> {
        api.device_list().find(|d| self.matches(d.vendor_id(), d.product_id()) && d.usage_page() == PAGE && d.usage() == USAGE)
    }
}

impl Provider for HyperX {
    fn id(&self) -> &'static str {
        ID
    }
    fn default_name(&self) -> &'static str {
        "HyperX Cloud III S Wireless"
    }
    fn kind(&self) -> DeviceKind {
        DeviceKind::Headset
    }
    fn connection(&self) -> &'static str {
        "2.4 GHz dongle"
    }
    fn set_product_filter(&mut self, pid: Option<u16>) {
        self.product_filter = pid;
    }

    fn signature(&self, api: &HidApi) -> String {
        signature_of(api, |d| self.matches(d.vendor_id(), d.product_id()) && d.usage_page() == PAGE)
    }

    fn poll(&mut self, api: &HidApi) -> Poll {
        let Some(info) = self.find(api) else {
            return Poll::not_found();
        };
        let found = Found {
            product: clean_product(info.product_string().unwrap_or("HyperX headset")),
            hardware: format!("{:04X}:{:04X}", info.vendor_id(), info.product_id()),
            source: "HID output/input report 0x0C, usage page 01C0, commands 0x06 / 0x48".into(),
            connection: None,
        };
        let outcome = match info.open_device(api) {
            Ok(dev) => Self::query(&dev).unwrap_or_else(Outcome::Failed),
            Err(e) => Outcome::Failed(e.to_string()),
        };
        Poll { found: Some(found), outcome }
    }

    /// The dongle pushes cable and power events; a dedicated handle lets WhiteHub react at once.
    fn event_source(&self, api: &HidApi) -> Option<EventSource> {
        let device = self.find(api)?.open_device(api).ok()?;
        Some(EventSource { device, is_change: is_change_event })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn report(head: &[u8]) -> Vec<u8> {
        let mut r = vec![0u8; 64];
        r[..head.len()].copy_from_slice(head);
        r
    }

    #[test]
    fn hardware_capture() {
        // Captured from the 06BE dongle: 46 %, 3776 mV.
        assert_eq!(decode(&report(&[0x0C, 0x02, 0x03, 0x01, 0x00, 0x06, 0x2E, 0x0E, 0xC0])), Some(Frame::Battery(Some(46), Some(3776))));
    }

    #[test]
    fn headset_off() {
        assert_eq!(decode(&report(&[0x0C, 0x02, 0x03, 0x01, 0x00, 0x06, 0xFF])), Some(Frame::Battery(None, None)));
    }

    #[test]
    fn events() {
        assert_eq!(decode(&report(&[0x0D, 0x02, 0x03, 0x00, 0x0A, 0x01])), Some(Frame::Charger(true)));
        assert_eq!(decode(&report(&[0x0D, 0x02, 0x03, 0x00, 0x0C, 0x00])), Some(Frame::Headset(false)));
        assert_eq!(decode(&report(&[0x0D, 0x02, 0x03, 0x00, 0x03, 0x01])), None);
    }

    #[test]
    fn ignores_other_frames() {
        assert_eq!(decode(&report(&[0x0C, 0x02, 0x03, 0x01, 0x00, 0x02, 0x02])), None);
        assert_eq!(decode(&[0x0C, 0x02, 0x03, 0x01, 0x00, 0x06]), None);
        assert_eq!(decode(&query_bytes(CMD_BATTERY)), None);
    }

    #[test]
    fn charge_state_captures() {
        // Captured from the 06BE dongle: cable unplugged, then plugged.
        assert_eq!(decode(&report(&[0x0C, 0x02, 0x03, 0x01, 0x00, 0x48, 0x00])), Some(Frame::ChargeState(0)));
        assert_eq!(decode(&report(&[0x0C, 0x02, 0x03, 0x01, 0x00, 0x48, 0x01])), Some(Frame::ChargeState(1)));
        assert_eq!(decode(&report(&[0x0C, 0x02, 0x03, 0x01, 0x00, 0x48, 0x07])), None);
        assert!(is_change_event(&report(&[0x0D, 0x02, 0x03, 0x00, 0x0A, 0x01])));
        assert!(!is_change_event(&report(&[0x0C, 0x02, 0x03, 0x01, 0x00, 0x06, 0x2E, 0x0E, 0xC0])));
    }
}
