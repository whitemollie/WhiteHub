# Project state

Checkpoint: 2026-11-25, about 21:30 local time. Version 0.2.1.

Evidence labels: **[checked]** was checked while writing this file, **[earlier]** was observed earlier in development and not repeated now.

## Repository

| Item | State |
|---|---|
| Branch | `main`: the 0.2.1 commit on top of `1107f7a` (0.2.0); not pushed, `origin/main` is at `1107f7a` |
| Working tree | Clean after the 0.2.1 commit; no stashes, no other branches |
| Tags | `v0.1.0` to `v0.1.9`, `v0.2.0`; no tag for 0.2.1 yet [checked] |
| GitHub | `whitemollie/WhiteHub`, private, GPL-3.0; no releases, issues or pull requests [checked] |
| Version | 0.2.1 in `package.json`, `Cargo.toml`, `tauri.conf.json` and both lock files [checked] |
| Tests | 55 passed, 0 failed, 1 ignored (`shell::render::tests::preview`, draws sample images by hand); clippy without warnings [checked] |
| Type check | `tsc --noEmit` without errors [checked] |
| CI | Run `45681696444` for `1107f7a` succeeded [earlier]; 0.2.1 has not run |
| Installer | `release/WhiteHub_0.2.1_x64-setup.exe`, 3,637,661 bytes, not in git [checked] |
| Installed copy | 0.2.0, installed over 0.1.7; the strip is embedded and all three devices reported a level (log) [checked] |

Files kept out of git on purpose: `release/`, `docs/RESEARCH.md`, `docs/FINAL_REPORT.md` (describes 0.1.0, partly outdated), `docs/research/`.

## 1. Implemented

- Batteries of WLmouse Beast G and HyperX Cloud III S Wireless, including charging.
- Automatic discovery: Bluetooth devices whose level Windows knows; Xbox controllers through XInput, in coarse steps.
- Dynamic device list: devices are remembered, can be forgotten, and their type can be corrected by hand.
- "My device is missing → Create file": a device request without serial numbers, addresses or Bluetooth names.
- Battery strip on the taskbar, left of the `^` button; tray icons as fallback.
- Fixed 640×560 window with Dashboard, Devices, Appearance, Settings and About; opens without a flash.
- Low battery notifications, themes, autostart, diagnostic export, log.
- Installer for the current user, with Microsoft's WebView2 bootstrapper embedded.
- Security hardening and tests, documentation in English and Russian, changelog, issue forms, CI.

Verified on hardware [earlier]:

- WLmouse 54 %, HyperX 47 %, Yoga Pro Mouse (Bluetooth) 50–51 %; the last one was found without setup.
- Disconnecting and reconnecting the Bluetooth mouse was noticed immediately.
- "Create file" works.
- In the background: 3.1 MB private memory, 10 threads, no CPU time over 60 s.

## 2. In development

Nothing. 0.2.1 is committed, not pushed: updates keep the autostart entry and Task Manager's flag (`hooks.nsh`). Checked with a small test installer using the same hooks, and the full installer builds; a real update between WhiteHub versions is not tested.

## 3. Unfinished

Waiting for a decision of the owner:

- GitHub release with the installer. A draft was offered and not answered.
- Making the repository public. Not approved.

Not tested:

- AirPods Pro 3 (could not be connected to the PC), and Bluetooth keyboards and headsets
- Xbox controllers
- HyperX dongle `03F0:02CC` and other WLmouse models
- HyperX headset switched off; its "full" charge state
- Strip of 0.2.0 with several devices on the real taskbar
- Sleep and hibernation; autostart after a reboot
- Light taskbar, several monitors, auto-hidden taskbar, left-aligned taskbar with many pinned apps
- Acrylic transparency
- Official WL MOUSE HUB open at the same time (unit test only)

A comparison of the percentages with the vendors' programs is not on record.

Ideas, not planned in detail and not approved: Logitech HID++ and more headsets (0.3 or later), a social preview image, download and star badges after the repository is public.

## 4. Errors and known issues

There is no failing test. Found by reading the code, not reproduced and not fixed:

- `app.rs`, `remember_devices`: the remembered type is the one chosen by the user, not the detected one. After a restart, resetting the type to automatic while the device is away keeps showing the chosen type until it connects.

Limits and loose ends:

- The README's download links lead to an empty Releases page until a release exists.
- A sleeping WLmouse does not report its level; the last value is shown dimmed.
- Costs of the chosen approach (taskbar strip, Bluetooth property, support per 2.4 GHz model): `DECISIONS.md`.
- The installer is not code-signed.
- Updating from 0.2.0 or older still removes the autostart entry and flag once, because the old uninstaller does it; WhiteHub restores the entry at its next start.
- Threads: 40 to 45 were seen with the installed 0.2.0, 43 after 10 minutes; 10 in the earlier measurement. Cause unknown; no CPU use while idle.
- `cargo audit` reports `ttf-parser` as unmaintained [earlier]; see `SECURITY.md`.

## 5. Technical decisions

See [DECISIONS.md](DECISIONS.md); structure: [ARCHITECTURE.md](ARCHITECTURE.md).

## 6. Next steps

1. Push 0.2.1 and tag it, when the owner agrees.
2. Owner installs `release/WhiteHub_0.2.1_x64-setup.exe` over 0.2.0.
3. Owner checks that settings survived, then the strip with all three devices, sleep and wake, and autostart after a reboot.
4. Connect AirPods in Windows. If Windows shows their level, WhiteHub should too; if not, use "Create file".
5. Fix what steps 3 and 4 reveal; otherwise no code change is needed.
6. After approval: create a draft release for `v0.2.1` with the installer and its SHA-256, then check the download link.
7. After approval: make the repository public, add the dynamic badges.
8. Choose the scope of 0.3.
