<div align="center">

<img src="assets/app-icon.svg" width="112" alt="WhiteHub logo">

# WhiteHub

**Battery levels of your wireless gear, right on the Windows 11 taskbar.**

No vendor apps in the background. No drivers. No network access.

![Release](https://img.shields.io/badge/release-v0.1.9-0078D4?style=flat-square)
![Platform](https://img.shields.io/badge/platform-Windows%2011-0078D4?style=flat-square)
![License](https://img.shields.io/badge/license-GPL--3.0-22C55E?style=flat-square)
[![CI](../../actions/workflows/ci.yml/badge.svg)](../../actions/workflows/ci.yml)
<br>
![Rust](https://img.shields.io/badge/Rust-stable-CE422B?style=flat-square&logo=rust&logoColor=white)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white)
![SolidJS](https://img.shields.io/badge/SolidJS-1.9-2C4F7C?style=flat-square&logo=solid&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-5.9-3178C6?style=flat-square&logo=typescript&logoColor=white)

**English** | [Русский](README.ru.md)

### [⬇️ Download for Windows](../../releases) · [What's new](CHANGELOG.md) · [Report an issue](../../issues/new/choose)

[Highlights](#highlights) · [Preview](#preview) · [Quick start](#quick-start) · [Devices](#devices) · [FAQ](#faq) · [Privacy](#privacy)

<br>

<img src="assets/screenshots/strip.png" width="430" alt="Battery strip on the taskbar">

</div>

## Highlights

| | |
|---|---|
| 🔋 **Always in sight** | Battery levels sit next to the clock, so you never have to open another app |
| ⚡ **Charging status** | A bolt appears when a device is on its cable |
| 🔔 **Timely alerts** | Low battery, connected and disconnected, with do not disturb |
| 🎨 **Your look** | Dark and light themes, custom colours, three icon styles |
| 🪶 **Lightweight** | One background process; the window is loaded only while it is open |
| 🔒 **Private** | No telemetry, accounts or network requests |

## Preview

<div align="center">

| Dashboard | Light theme |
|:---:|:---:|
| <img src="assets/screenshots/dashboard.png" width="400" alt="Dashboard"> | <img src="assets/screenshots/dashboard-light.png" width="400" alt="Dashboard in the light theme"> |
| **Devices** | **Appearance** |
| <img src="assets/screenshots/devices.png" width="400" alt="Devices page"> | <img src="assets/screenshots/appearance.png" width="400" alt="Appearance page"> |

</div>

## Quick start

1. Download `WhiteHub_x.y.z_x64-setup.exe` from [Releases](../../releases).
2. Run it. It installs for your user only and needs no administrator rights.
3. Plug in a [supported device](#devices). The battery appears on the taskbar.

> [!NOTE]
> The installer is not code-signed, so Windows SmartScreen may warn about an unknown publisher.

## Devices

| Device | USB receiver | Status |
|---|---|---|
| WLmouse Beast G | `36A7:A860`, cable `36A7:A861` | ✅ Tested |
| HyperX Cloud III **S** Wireless | `03F0:06BE` | ✅ Tested |
| HyperX Cloud III S Wireless, other dongle | `03F0:02CC` | ⚪ Untested |
| Other WLmouse mice | Product ID entered by hand | ⚪ Untested |

> [!IMPORTANT]
> WhiteHub is not universal: every vendor uses its own protocol. Other mice, headsets, keyboards and Bluetooth devices are not supported yet. Missing yours? [Request a device](../../issues/new?template=device-request.yml).

## FAQ

<details>
<summary><b>Why does my mouse show an old value?</b></summary>

A sleeping mouse does not report its battery. WhiteHub shows the last known value dimmed and tries again. Move the mouse and press **Refresh now**.
</details>

<details>
<summary><b>The WhiteHub tray icon is hidden. Where is it?</b></summary>

Windows hides new icons under `^`. Turn it on in *Settings → Personalization → Taskbar → Other system tray icons*.
</details>

<details>
<summary><b>Is the taskbar strip an official Windows feature?</b></summary>

No. It is a small window placed inside the taskbar. If a Windows update breaks that, WhiteHub switches to tray icons.
</details>

<details>
<summary><b>What has not been tested?</b></summary>

Sleep and hibernation, several monitors, a light taskbar and an auto-hiding taskbar. Only the primary taskbar is used.
</details>

<details>
<summary><b>How much memory does it use?</b></summary>

About 7 MB of private memory in the background once the window has been used (measured with 0.1.7). While the window is open, WebView2 adds about 165 MB (measured with 0.1.0).
</details>

<details>
<summary><b>How do I uninstall it?</b></summary>

*Settings → Apps → WhiteHub*. The uninstaller asks whether to delete settings and logs.
</details>

## Privacy

WhiteHub sends only read-only battery queries, and only to the USB devices listed above or to one you choose by its product ID. Settings live in `%APPDATA%\com.whitehub.desktop`, logs in `%LOCALAPPDATA%\com.whitehub.desktop`. Vulnerabilities: [SECURITY.md](SECURITY.md).

## For developers

Users only need the installer. Building from source and adding devices: [CONTRIBUTING.md](CONTRIBUTING.md). Device protocols: [docs/PROTOCOLS.md](docs/PROTOCOLS.md).

## Licence

[GPL-3.0](LICENSE), © 2026 whitemollie. Protocol sources and credits: [CREDITS.md](CREDITS.md).

<sub>WhiteHub is an independent project. It is not affiliated with or endorsed by WLmouse, HyperX or HP.</sub>
