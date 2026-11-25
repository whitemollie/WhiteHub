# Project state

Version 0.2.3, November 2026. Structure: [ARCHITECTURE.md](ARCHITECTURE.md). Reasons: [DECISIONS.md](DECISIONS.md). History: [CHANGELOG.md](../CHANGELOG.md).

## Working

- Battery and charging of the models listed in the [README](../README.md#devices).
- Automatic discovery of Bluetooth devices whose level Windows knows, and of Xbox controllers in coarse steps.
- Device list that remembers devices; they can be renamed, forgotten, and given another type.
- Device request file without serial numbers, addresses or Bluetooth names.
- Battery strip on the taskbar with tray icons as fallback; the logo as tray icon.
- Window with Dashboard, Devices, Appearance, Settings and About.
- Low battery notifications, themes, autostart, diagnostic export, log.
- Installer for the current user; updates keep autostart.

## Tested on hardware

- The models marked as tested in the README and a Bluetooth mouse: level, connecting and disconnecting.
- Device request file.
- Updates from 0.1.7 to 0.2.3. From 0.2.1 on the autostart entry survived them (log).

## Not tested

- Bluetooth keyboards, headsets and earbuds
- Xbox controllers
- The models marked as untested in the README
- Headset switched off; its "full" charge state
- Autostart after a reboot
- Tray icon with the logo on the real taskbar
- Sleep and hibernation
- Light taskbar, several monitors, auto-hidden taskbar, left-aligned taskbar with many pinned apps
- Acrylic transparency
- A vendor's program open at the same time (unit test only)

## Known issues

- A sleeping mouse on its receiver does not report its level; the last value is shown dimmed.
- Updating from 0.2.0 or older removes the autostart entry once, because the old uninstaller does it; WhiteHub restores it at its next start.
- 40 to 48 threads were seen since 0.2.0, 10 in an earlier measurement. The cause is unknown; there is no CPU use while idle.
- The installer is not code-signed.
- `ttf-parser` is unmaintained, see [SECURITY.md](../SECURITY.md).

## Next

1. Test the points above.
2. More 2.4 GHz devices.
