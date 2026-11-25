# WhiteHub

Battery levels of wireless peripherals, right on the Windows 11 taskbar. [Русская версия](README.ru.md)

WhiteHub reads the battery straight from the USB receivers through the standard Windows HID driver and shows it next to the notification area. It needs no vendor software, no extra drivers and no administrator rights, and it makes no network requests.

## Supported devices

WhiteHub is **not universal**: every vendor uses its own protocol, and only these devices are supported today.

| Device | USB receiver | Status |
|---|---|---|
| WLmouse Beast G | `36A7:A860` (receiver), `36A7:A861` (cable) | Tested on hardware |
| HyperX Cloud III **S** Wireless | `03F0:06BE` | Tested on hardware |
| HyperX Cloud III S Wireless, other dongle | `03F0:02CC` | Untested |
| Other WLmouse mice (USB vendor `36A7`) | — | Detected, untested; some models use a different protocol |

Other mice, headsets, keyboards and Bluetooth devices are not supported yet. Without a supported device WhiteHub starts and shows `--`. [Request a device](../../issues/new?template=device-request.yml).

## Features

- Battery strip on the taskbar, with tray icons as a fallback
- Charging indicator, low-battery and connection notifications
- Dark and light themes with custom colours
- Polling every 5 to 60 minutes, optional start with Windows
- Small footprint: one background process, about 7 MB of private memory once the window has been used; the window itself (WebView2) is loaded only while it is open

## Install

Requires Windows 11 x64.

1. Download `WhiteHub_x.y.z_x64-setup.exe` from [Releases](../../releases).
2. Run it. It installs for the current user only. The installer is not code-signed, so SmartScreen may warn about an unknown publisher.
3. To keep the WhiteHub tray icon visible: *Settings → Personalization → Taskbar → Other system tray icons*.

To uninstall: *Settings → Apps → WhiteHub*. The uninstaller asks whether to delete settings and logs.

## Good to know

- **A sleeping mouse does not report its battery.** WhiteHub keeps showing the last known value, dimmed, and retries.
- **The taskbar strip is not an official Windows feature.** It is a window placed inside the taskbar. If a Windows update breaks that, WhiteHub switches to tray icons.
- Only the primary taskbar is used. Sleep/hibernation, multiple monitors and a light taskbar have not been tested.

## Privacy and security

No telemetry, accounts or network access. WhiteHub sends only read-only battery queries, and only to the HyperX dongles listed above and to USB devices of the vendor WLmouse (`36A7`). Settings live in `%APPDATA%\com.whitehub.desktop`, logs in `%LOCALAPPDATA%\com.whitehub.desktop`. To report a vulnerability, see [SECURITY.md](SECURITY.md).

## Build

Requires Rust (stable, MSVC) and Node.js 22+.

```powershell
npm ci
npm run tauri dev                      # development
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build                    # release build and installer
```

Device protocols: [docs/PROTOCOLS.md](docs/PROTOCOLS.md). To add a device, implement `battery::provider::Provider`, register it in `app.rs` and add its ID to `settings::DEVICE_IDS`.

## Licence

[GPL-3.0](LICENSE), © 2026 whitemollie. Protocol sources and credits: [CREDITS.md](CREDITS.md).

WhiteHub is an independent project. It is not affiliated with or endorsed by WLmouse, HyperX or HP.
