//! Battery levels of Bluetooth devices as Windows itself knows them.
//!
//! Windows reads the level (Bluetooth LE battery service, or the hands-free profile of headsets)
//! and publishes it as a device property; it is the number shown in the Windows settings. Nothing
//! is sent to the devices. The battery and connection properties are not officially documented.
use super::{
    provider::clean_product,
    scanner::{Reading, Scanned, Scanner},
    DeviceKind,
};
use std::collections::{BTreeMap, HashSet};
use windows_sys::{
    core::GUID,
    Win32::Devices::{
        DeviceAndDriverInstallation::{
            CM_Get_DevNode_PropertyW, CM_Get_Device_ID_ListW, CM_Get_Device_ID_List_SizeW, CM_Locate_DevNodeW, CM_GETIDLIST_FILTER_CLASS,
            CM_GETIDLIST_FILTER_ENUMERATOR, CM_GETIDLIST_FILTER_PRESENT, CM_LOCATE_DEVNODE_NORMAL, CR_SUCCESS,
        },
        Properties::{DEVPROPKEY, DEVPROP_TYPE_BOOLEAN, DEVPROP_TYPE_BYTE, DEVPROP_TYPE_GUID, DEVPROP_TYPE_STRING, DEVPROP_TYPE_UINT32},
    },
};

const KEY_BATTERY: DEVPROPKEY = key(0x104EA319_6EE2_4701_BD47_8DDBF425BBE5, 2);
const KEY_CONNECTED: DEVPROPKEY = key(0x83DA6326_97A6_4088_9453_A1923F573B29, 15);
const KEY_NAME: DEVPROPKEY = key(0xB725F130_47EF_101A_A5F1_02608C9EEBAC, 10);
const KEY_CONTAINER: DEVPROPKEY = key(0x8C7ED206_3F8A_4827_B3AB_AE9E1FAEFC6C, 2);
const KEY_CLASS_OF_DEVICE: DEVPROPKEY = key(0x2BD67D8B_8BEB_48D5_87E0_6CDA3428040A, 10);
const CLASS_MOUSE: &str = "{4d36e96f-e325-11ce-bfc1-08002be10318}";
const CLASS_KEYBOARD: &str = "{4d36e96b-e325-11ce-bfc1-08002be10318}";
/// Container shared by the computer's own parts.
const SYSTEM_CONTAINER: &str = "00000000-0000-0000-ffff-ffffffffffff";

const fn key(guid: u128, pid: u32) -> DEVPROPKEY {
    DEVPROPKEY { fmtid: GUID::from_u128(guid), pid }
}

/// What one Bluetooth device node reports.
#[derive(Clone, Debug, Default)]
struct Node {
    container: String,
    /// The node of the device itself, as opposed to one of its services.
    is_device: bool,
    name: Option<String>,
    connected: Option<bool>,
    battery: Option<u8>,
    class_of_device: Option<u32>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn instance_ids(filter: &str, flags: u32) -> Vec<String> {
    let filter = wide(filter);
    let flags = flags | CM_GETIDLIST_FILTER_PRESENT;
    // The list can grow between the two calls; try again once with the new size.
    for _ in 0..2 {
        let mut len = 0u32;
        if unsafe { CM_Get_Device_ID_List_SizeW(&mut len, filter.as_ptr(), flags) } != CR_SUCCESS || len == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u16; len as usize];
        if unsafe { CM_Get_Device_ID_ListW(filter.as_ptr(), buf.as_mut_ptr(), len, flags) } == CR_SUCCESS {
            return buf.split(|&c| c == 0).filter(|s| !s.is_empty()).map(String::from_utf16_lossy).collect();
        }
    }
    Vec::new()
}

fn locate(id: &str) -> Option<u32> {
    let id = wide(id);
    let mut inst = 0u32;
    (unsafe { CM_Locate_DevNodeW(&mut inst, id.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) } == CR_SUCCESS).then_some(inst)
}

/// Reads a property if it exists and has the expected type.
fn property(inst: u32, key: &DEVPROPKEY, expected: u32) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; 1024];
    let (mut kind, mut size) = (0u32, buf.len() as u32);
    let rc = unsafe { CM_Get_DevNode_PropertyW(inst, key, &mut kind, buf.as_mut_ptr(), &mut size, 0) };
    if rc != CR_SUCCESS || kind != expected || size as usize > buf.len() {
        return None;
    }
    buf.truncate(size as usize);
    Some(buf)
}

fn text(inst: u32, key: &DEVPROPKEY) -> Option<String> {
    let raw = property(inst, key, DEVPROP_TYPE_STRING)?;
    let units: Vec<u16> = raw.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).take_while(|&c| c != 0).collect();
    let name = clean_product(&String::from_utf16_lossy(&units));
    (!name.is_empty()).then_some(name)
}

fn container(inst: u32) -> Option<String> {
    let b: [u8; 16] = property(inst, &KEY_CONTAINER, DEVPROP_TYPE_GUID)?.try_into().ok()?;
    let h = |r: std::ops::Range<usize>, rev: bool| -> String {
        let mut v = b[r].to_vec();
        if rev {
            v.reverse();
        }
        v.iter().map(|x| format!("{x:02x}")).collect()
    };
    let id = format!("{}-{}-{}-{}-{}", h(0..4, true), h(4..6, true), h(6..8, true), h(8..10, false), h(10..16, false));
    (id != SYSTEM_CONTAINER).then_some(id)
}

fn read_node(id: &str) -> Option<Node> {
    let inst = locate(id)?;
    Some(Node {
        container: container(inst)?,
        is_device: id.split('\\').nth(1).is_some_and(|p| p.to_ascii_uppercase().starts_with("DEV_")),
        name: text(inst, &KEY_NAME),
        connected: property(inst, &KEY_CONNECTED, DEVPROP_TYPE_BOOLEAN).map(|v| v.first().is_some_and(|&b| b != 0)),
        battery: property(inst, &KEY_BATTERY, DEVPROP_TYPE_BYTE).and_then(|v| v.first().copied()),
        class_of_device: property(inst, &KEY_CLASS_OF_DEVICE, DEVPROP_TYPE_UINT32).and_then(|v| v.try_into().ok().map(u32::from_le_bytes)),
    })
}

fn containers_of_class(class: &str) -> HashSet<String> {
    instance_ids(class, CM_GETIDLIST_FILTER_CLASS).iter().filter_map(|id| container(locate(id)?)).collect()
}

/// Device type from the Bluetooth class of device; `None` if it does not say.
fn kind_from_class(cod: u32) -> Option<DeviceKind> {
    let (major, minor) = ((cod >> 8) & 0x1f, (cod >> 2) & 0x3f);
    match major {
        0x04 => Some(DeviceKind::Headset),
        0x05 => match (minor >> 4, minor & 0x0f) {
            (_, 1 | 2) => Some(DeviceKind::Gamepad),
            (1, _) => Some(DeviceKind::Keyboard),
            (2 | 3, _) => Some(DeviceKind::Mouse),
            _ => None,
        },
        _ => None,
    }
}

/// Merges the nodes of each physical device.
fn merge(nodes: Vec<Node>, mice: &HashSet<String>, keyboards: &HashSet<String>) -> Vec<Scanned> {
    let mut groups: BTreeMap<String, Vec<Node>> = BTreeMap::new();
    for n in nodes {
        groups.entry(n.container.clone()).or_default().push(n);
    }
    groups
        .into_iter()
        .filter_map(|(id, nodes)| {
            let main = nodes.iter().find(|n| n.is_device)?;
            let battery = nodes.iter().find_map(|n| n.battery).filter(|&b| b <= 100);
            let kind = nodes
                .iter()
                .find_map(|n| n.class_of_device.and_then(kind_from_class))
                .or_else(|| mice.contains(&id).then_some(DeviceKind::Mouse))
                .or_else(|| keyboards.contains(&id).then_some(DeviceKind::Keyboard))
                .unwrap_or(DeviceKind::Other);
            Some(Scanned {
                id: format!("bt:{id}"),
                name: main.name.clone().unwrap_or_else(|| "Bluetooth device".into()),
                kind,
                connected: nodes.iter().any(|n| n.is_device && n.connected == Some(true)),
                reading: battery.map(Reading::Percent),
            })
        })
        .collect()
}

pub struct Bluetooth;

impl Scanner for Bluetooth {
    fn prefix(&self) -> &'static str {
        "bt:"
    }
    fn source(&self) -> &'static str {
        "Reported by Windows"
    }
    fn scan(&mut self, _: &[Scanned]) -> Vec<Scanned> {
        let ids = ["BTHENUM", "BTHLE"].iter().flat_map(|e| instance_ids(e, CM_GETIDLIST_FILTER_ENUMERATOR));
        let nodes: Vec<Node> = ids.filter_map(|id| read_node(&id)).collect();
        if nodes.is_empty() {
            return Vec::new();
        }
        merge(nodes, &containers_of_class(CLASS_MOUSE), &containers_of_class(CLASS_KEYBOARD))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(container: &str, is_device: bool) -> Node {
        Node { container: container.into(), is_device, ..Default::default() }
    }

    #[test]
    fn nodes_of_one_device_are_merged() {
        // A headset: connection on the device node, battery on its hands-free service node.
        let dev = Node { name: Some("Earbuds".into()), connected: Some(true), class_of_device: Some(0x240418), ..node("a", true) };
        let hfp = Node { battery: Some(80), ..node("a", false) };
        let got = merge(vec![hfp, dev], &HashSet::new(), &HashSet::new());
        assert_eq!(got, [Scanned { id: "bt:a".into(), name: "Earbuds".into(), kind: DeviceKind::Headset, connected: true, reading: Some(Reading::Percent(80)) }]);
    }

    #[test]
    fn implausible_levels_and_orphans_are_dropped() {
        let dev = Node { connected: Some(false), battery: Some(255), ..node("a", true) };
        let got = merge(vec![dev, Node { battery: Some(50), ..node("b", false) }], &HashSet::from(["a".to_string()]), &HashSet::new());
        assert_eq!(got.len(), 1, "a service without its device is ignored");
        assert_eq!((got[0].reading, got[0].connected, got[0].kind), (None, false, DeviceKind::Mouse));
    }

    #[test]
    fn class_of_device() {
        assert_eq!(kind_from_class(0x240404), Some(DeviceKind::Headset));
        assert_eq!(kind_from_class(0x002540), Some(DeviceKind::Keyboard));
        assert_eq!(kind_from_class(0x002580), Some(DeviceKind::Mouse));
        assert_eq!(kind_from_class(0x002508), Some(DeviceKind::Gamepad));
        assert_eq!(kind_from_class(0x5A020C), None, "phone");
    }

    #[test]
    fn scanning_this_computer_does_not_fail() {
        for d in Bluetooth.scan(&[]) {
            assert!(d.id.starts_with("bt:") && d.id.len() == 39, "{}", d.id);
        }
    }
}
