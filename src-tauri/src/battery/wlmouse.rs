//! WLmouse Beast G over its 2.4 GHz "8K" receiver (USB 36A7:A860).
//!
//! Protocol (read-only status query), byte-for-byte as sent by the official WL MOUSE HUB web app
//! (`getBatPer` in gm.wlmouse.gg) and verified on hardware:
//!
//! ```text
//! SET feature report 0 (65 bytes): 00 | 00 00 02 02 00 83 00 …
//! GET feature report 0:            00 | ST 00 02 02 00 83 CC NN …
//!   ST = A1 answer ready, A0 = still waiting for the mouse (it sleeps / is off)
//!   CC = 1 while charging, NN = battery percent
//! ```
//! The receiver relays the query to the mouse; a sleeping mouse never answers, so `A0` persists.
//! Other WLmouse receivers on VID 36A7 use the same query according to the hub code and
//! community tools, but only A860 has been tested.
use super::{
    hex,
    provider::{clean_product, Found, Outcome, Poll, Provider},
    DeviceKind,
};
use hidapi::{HidApi, HidDevice};
use std::{ffi::CString, thread::sleep, time::Duration};

pub const ID: &str = "wlmouse";
pub const VID: u16 = 0x36A7;
const VENDOR_PAGE: u16 = 0xFFFF;
const CMD_BATTERY: u8 = 0x83;
const ST_READY: u8 = 0xA1;
const QUERY_ATTEMPTS: usize = 16;
const QUERY_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug, PartialEq)]
pub enum Reply {
    Battery { percent: u8, charging: bool },
    /// The receiver has no answer from the mouse yet.
    Pending,
    /// Not a reply to the battery query (e.g. another app's command on the same channel).
    Foreign,
}

/// Decodes a feature report with or without the leading report-ID byte.
pub fn decode(r: &[u8]) -> Reply {
    let (st, sub, cmd, chg, lvl) = if r.len() >= 9 && r[0] == 0x00 && r[4] == 0x02 && r[6] == CMD_BATTERY {
        (r[1], r[4], r[6], r[7], r[8])
    } else if r.len() >= 8 && r[3] == 0x02 && r[5] == CMD_BATTERY {
        (r[0], r[3], r[5], r[6], r[7])
    } else {
        return Reply::Foreign;
    };
    debug_assert!(sub == 0x02 && cmd == CMD_BATTERY);
    match st {
        ST_READY if lvl > 100 => Reply::Foreign,
        // Level 0 while not charging is what the receiver reports for an unlinked mouse.
        ST_READY if lvl == 0 && chg == 0 => Reply::Pending,
        ST_READY => Reply::Battery { percent: lvl, charging: chg == 0x01 },
        // 0x00 = our request not processed yet; A0 = receiver waiting for the mouse.
        s if s != 0 && s < ST_READY => Reply::Pending,
        _ => Reply::Foreign,
    }
}

pub fn query_bytes() -> [u8; 65] {
    let mut q = [0u8; 65];
    q[3] = 0x02;
    q[4] = 0x02;
    q[6] = CMD_BATTERY;
    q
}

pub struct WlMouse {
    product_filter: Option<u16>,
    /// HID path of the collection that answered last time; tried first.
    last_path: Option<CString>,
}

impl WlMouse {
    pub fn new() -> Self {
        Self { product_filter: None, last_path: None }
    }

    fn matches(&self, vid: u16, pid: u16) -> bool {
        vid == VID && self.product_filter.map_or(true, |p| p == pid)
    }

    fn query(dev: &HidDevice) -> Result<Reply, String> {
        dev.send_feature_report(&query_bytes()).map_err(|e| e.to_string())?;
        let mut last = Reply::Foreign;
        for _ in 0..QUERY_ATTEMPTS {
            sleep(QUERY_DELAY);
            let mut buf = [0u8; 65];
            let n = dev.get_feature_report(&mut buf).map_err(|e| e.to_string())?;
            let reply = decode(&buf[..n]);
            log::debug!("[wlmouse] feature report: {}", hex(&buf[..n.min(12)]));
            match reply {
                Reply::Battery { .. } => return Ok(reply),
                Reply::Pending => last = Reply::Pending,
                Reply::Foreign => {}
            }
        }
        Ok(last)
    }
}

impl Provider for WlMouse {
    fn id(&self) -> &'static str {
        ID
    }
    fn default_name(&self) -> &'static str {
        "WLmouse Beast G"
    }
    fn kind(&self) -> DeviceKind {
        DeviceKind::Mouse
    }
    fn connection(&self) -> &'static str {
        "2.4 GHz receiver"
    }
    fn set_product_filter(&mut self, pid: Option<u16>) {
        self.product_filter = pid;
        self.last_path = None;
    }

    fn is_present(&self, api: &HidApi) -> bool {
        api.device_list().any(|d| self.matches(d.vendor_id(), d.product_id()) && d.usage_page() == VENDOR_PAGE)
    }

    fn poll(&mut self, api: &HidApi) -> Poll {
        let mut candidates: Vec<_> = api
            .device_list()
            .filter(|d| self.matches(d.vendor_id(), d.product_id()) && d.usage_page() == VENDOR_PAGE)
            .collect();
        if candidates.is_empty() {
            return Poll::not_found();
        }
        // Known-good collection first, then usage 0 (the one that accepts the query on A860).
        candidates.sort_by_key(|d| (Some(d.path()) != self.last_path.as_deref(), d.usage() != 0));
        let first = candidates[0];
        let found = Found {
            product: clean_product(first.product_string().unwrap_or("WLmouse")),
            hardware: format!("{:04X}:{:04X}", first.vendor_id(), first.product_id()),
            source: "HID feature report 0, vendor page FFFF, command 0x83".into(),
        };
        let mut errors = Vec::new();
        for info in candidates {
            let dev = match info.open_device(api) {
                Ok(d) => d,
                Err(e) => {
                    errors.push(e.to_string());
                    continue;
                }
            };
            match Self::query(&dev) {
                Ok(Reply::Battery { percent, charging }) => {
                    self.last_path = Some(info.path().to_owned());
                    return Poll { found: Some(found), outcome: Outcome::Battery { percent, charging, millivolts: None } };
                }
                Ok(Reply::Pending) => {
                    self.last_path = Some(info.path().to_owned());
                    return Poll { found: Some(found), outcome: Outcome::Asleep };
                }
                Ok(Reply::Foreign) => errors.push("unexpected reply".into()),
                Err(e) => errors.push(e),
            }
        }
        Poll { found: Some(found), outcome: Outcome::Failed(errors.join("; ")) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(lead: bool, head: &[u8]) -> Vec<u8> {
        let mut r = vec![0u8; 65];
        let o = lead as usize;
        r[o..o + head.len()].copy_from_slice(head);
        r
    }

    #[test]
    fn hardware_capture_ready() {
        // Captured from the Beast G receiver: 35 %, not charging.
        let r = frame(true, &[0xA1, 0x00, 0x02, 0x02, 0x00, 0x83, 0x00, 0x23]);
        assert_eq!(decode(&r), Reply::Battery { percent: 35, charging: false });
    }

    #[test]
    fn hardware_capture_asleep() {
        let r = frame(true, &[0xA0, 0x00, 0x02, 0x02, 0x00, 0x83, 0x00, 0x00]);
        assert_eq!(decode(&r), Reply::Pending);
    }

    #[test]
    fn without_report_id_and_charging() {
        let r = frame(false, &[0xA1, 0x00, 0x02, 0x02, 0x00, 0x83, 0x01, 0x32]);
        assert_eq!(decode(&r), Reply::Battery { percent: 50, charging: true });
    }

    #[test]
    fn rejects_foreign_and_invalid() {
        assert_eq!(decode(&query_bytes()), Reply::Foreign); // our own query read back
        assert_eq!(decode(&frame(true, &[0xA1, 0x00, 0x02, 0x01, 0x00, 0x85, 0x00, 0x02])), Reply::Foreign);
        assert_eq!(decode(&frame(true, &[0xA1, 0x00, 0x02, 0x02, 0x00, 0x83, 0x00, 0xC8])), Reply::Foreign);
        assert_eq!(decode(&frame(true, &[0xA3, 0x00, 0x02, 0x02, 0x00, 0x83, 0x00, 0x20])), Reply::Foreign);
        assert_eq!(decode(&[0x00, 0xA1]), Reply::Foreign);
    }

    #[test]
    fn zero_level_means_unlinked() {
        assert_eq!(decode(&frame(true, &[0xA1, 0x00, 0x02, 0x02, 0x00, 0x83, 0x00, 0x00])), Reply::Pending);
    }

    #[test]
    fn query_matches_official_hub() {
        // Hub: n[2]=2, n[3]=2, n[5]=131 on a 64-byte buffer sent as report 0.
        let q = query_bytes();
        assert_eq!(&q[..8], &[0x00, 0x00, 0x00, 0x02, 0x02, 0x00, 0x83, 0x00]);
        assert!(q[8..].iter().all(|&b| b == 0));
    }
}
