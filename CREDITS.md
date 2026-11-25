# Credits

WhiteHub's device protocols build on the work of these projects.

| Project | Licence | Used for |
|---|---|---|
| [Wireless-Device-Battery-Monitor (BatteryHub)](https://github.com/ferloom/Wireless-Device-Battery-Monitor) by ferloom | GPL-3.0 | The WLmouse and HyperX providers (`src-tauri/src/battery/wlmouse.rs`, `hyperx.rs`) were written with its drivers as a reference and follow their structure. |
| [HyperHeadset](https://github.com/LennardKittner/HyperHeadset) by Lennard Kittner | MIT | Charge-state query and events of the Cloud III S |
| [hyperx-battery-logger](https://github.com/OZX-OG/hyperx-battery-logger) by OZX-OG | MIT | Battery query of the Cloud III S |
| [HyperX-Battery-Tray](https://github.com/GuacamoleGH/HyperX-Battery-Tray) by GuacamoleGH | MIT | Battery reply of the Cloud III S |
| [wlmouse-cli](https://github.com/mee7ya/wlmouse-cli) by mee7ya | none stated | Protocol facts about WLmouse receivers; no code was copied |

BatteryHub in turn credits [HaloBattery](https://github.com/HeyOkay/HaloBattery) (MIT) for the WLmouse protocol.

The battery query was additionally compared with the vendor's own web configurator and verified on hardware.

Main libraries: [Tauri](https://tauri.app), [SolidJS](https://www.solidjs.com), [hidapi](https://github.com/libusb/hidapi), [ab_glyph](https://github.com/alexheretic/ab-glyph), [windows-sys](https://github.com/microsoft/windows-rs).

Product names are trademarks of their owners and are used only to describe compatibility.
