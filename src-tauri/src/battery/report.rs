//! Text for a device request: what hardware is connected, without serial numbers, paths, addresses
//! or Bluetooth names (which often contain a person's name).
use super::{
    provider::clean_product,
    scanner::{Reading, Scanned},
};
use hidapi::{BusType, HidApi};
use std::collections::BTreeMap;

/// HID usage pages that vendors define themselves; battery channels live there.
fn is_vendor(page: u16) -> bool {
    page >= 0xFF00 || page == 0x01C0
}

/// Bluetooth devices can be renamed by their owner, so only USB product names are shown.
fn shown_name(usb: bool, name: Option<&str>) -> String {
    if usb {
        clean_product(name.unwrap_or("?"))
    } else {
        "(name left out)".into()
    }
}

pub fn describe_hid(api: &HidApi) -> String {
    let mut devices: BTreeMap<(u16, u16), (String, Vec<String>)> = BTreeMap::new();
    for d in api.device_list() {
        let e = devices.entry((d.vendor_id(), d.product_id())).or_insert_with(|| (shown_name(matches!(d.bus_type(), BusType::Usb) && d.interface_number() >= 0, d.product_string()), Vec::new()));
        let mark = if is_vendor(d.usage_page()) { "*" } else { "" };
        e.1.push(format!("if{} {:04X}/{:04X}{mark}", d.interface_number(), d.usage_page(), d.usage()));
    }
    let mut out = String::from("== USB / HID (VID:PID, name, usage page/usage, * = vendor-defined) ==\n");
    for ((vid, pid), (name, mut usages)) in devices {
        usages.sort();
        usages.dedup();
        out.push_str(&format!("{vid:04X}:{pid:04X}  {name}\n    {}\n", usages.join(", ")));
    }
    out
}

pub fn describe_scanned(found: &[Scanned]) -> String {
    let mut out = String::from("\n== Bluetooth and controllers (names left out) ==\n");
    for d in found {
        let level = match d.reading {
            Some(Reading::Percent(p)) => format!("{p}%"),
            Some(Reading::Step(l)) => l.label().into(),
            None => "no battery level".into(),
        };
        out.push_str(&format!("{:?}: {}, {level}\n", d.kind, if d.connected { "connected" } else { "not connected" }));
    }
    if found.is_empty() {
        out.push_str("none\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::DeviceKind;

    #[test]
    fn only_usb_names_are_shown() {
        assert_eq!(shown_name(true, Some("N-KEY Device")), "N-KEY Device");
        assert!(!shown_name(false, Some("Jane's Mouse")).contains("Jane"));
    }

    #[test]
    fn names_and_ids_are_left_out() {
        let d = Scanned { id: "bt:secret".into(), name: "Jane's Earbuds".into(), kind: DeviceKind::Headset, connected: true, reading: Some(Reading::Percent(80)) };
        let text = describe_scanned(&[d]);
        assert!(!text.contains("Jane") && !text.contains("secret"), "{text}");
        assert!(text.contains("Headset: connected, 80%"));
    }
}
