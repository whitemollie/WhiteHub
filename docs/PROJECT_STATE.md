# Project state

Version 0.2.3, November 2026. Structure: [ARCHITECTURE.md](ARCHITECTURE.md). Reasons: [DECISIONS.md](DECISIONS.md). History: [CHANGELOG.md](../CHANGELOG.md).

## Working

- Battery and charging of WLmouse Beast G and HyperX Cloud III S Wireless.
- Automatic discovery of Bluetooth devices whose level Windows knows, and of Xbox controllers in coarse steps.
- Device list that remembers devices; they can be renamed, forgotten, and given another type.
- Device request file without serial numbers, addresses or Bluetooth names.
- Battery strip on the taskbar with tray icons as fallback; the logo as tray icon.
- Window with Dashboard, Devices, Appearance, Settings and About.
- Low battery notifications, themes, autostart, diagnostic export, log.
- Installer for the current user; updates keep autostart.

## Tested on hardware

- WLmouse Beast G, HyperX Cloud III S Wireless and a Bluetooth mouse: level, connecting and disconnecting.
- Device request file.
- Installing 0.2.0 over 0.1.7 and 0.2.1 over 0.2.0.

## Not tested

- AirPods, Bluetooth keyboards and headsets
- Xbox controllers
- HyperX dongle `03F0:02CC` and other WLmouse models
- HyperX headset switched off; its "full" charge state
- Autostart kept by an update from 0.2.1 or later; autostart after a reboot
- Tray icon with the logo on the real taskbar
- Sleep and hibernation
- Light taskbar, several monitors, auto-hidden taskbar, left-aligned taskbar with many pinned apps
- Acrylic transparency
- The vendor's WL MOUSE HUB open at the same time (unit test only)

## Known issues

- A sleeping WLmouse does not report its level; the last value is shown dimmed.
- Updating from 0.2.0 or older removes the autostart entry once, because the old uninstaller does it; WhiteHub restores it at its next start.
- About 40 threads were seen in 0.2.0, 10 in an earlier measurement. The cause is unknown; there is no CPU use while idle.
- The installer is not code-signed.
- `ttf-parser` is unmaintained, see [SECURITY.md](../SECURITY.md).

## Next

1. Test the points above.
2. Logitech HID++ and more headsets.
