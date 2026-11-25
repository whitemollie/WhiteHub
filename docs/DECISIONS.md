# Decisions

Only decisions with a lasting effect. The version says where a decision is in effect from. How things work: [ARCHITECTURE.md](ARCHITECTURE.md). Add new entries at the end; mark replaced ones instead of deleting them.

### 1. Battery strip inside the taskbar (0.1.0)
Tray icons can be hidden under `^`; the level should always be visible. Windows 11 has no official extension point, so the strip is a child window of the taskbar. **Cost:** a Windows update can break it, so tray icons stay as the fallback.

### 2. Destroy the window on close (0.1.0)
WebView2 needs about 170 MB while open (measured with 0.1.0). **Cost:** every opening starts WebView2 again.

### 3. One worker thread, no async runtime (0.1.0)
One thread owns the HID API, so providers need no locking and idle costs no CPU. `panic = "unwind"` is required, because panics of a provider are caught. **Cost:** polls run one after another and must be short.

### 4. Own autostart code (0.1.0)
The autostart plugin wrote to `HKLM`. WhiteHub must work without administrator rights, so it uses the `HKCU` Run key.

### 5. Tray icons are never recreated (0.1.0)
Windows remembers pinning per icon ID, assigned in creation order. Icons are created once and then shown or hidden.

### 6. Draw the strip in software with system fonts (0.1.0, look final in 0.1.7)
Memory-mapped system fonts cost almost no private memory and nothing has to be bundled. Chosen look: Segoe Fluent Icons with Segoe UI Black digits.

### 7. Cloak the window and fade it ourselves (0.1.5)
WebView2 paints its first frame late, which showed as a white flash. **Cost:** translucent windows skip the fade, because layered windows cannot show acrylic.

### 8. One window size for every page (0.1.5)
Sizing the window to its content caused flicker. Longer pages scroll.

### 9. GPL-3.0 (0.1.8)
The project builds on GPL-3.0 sources. The credits in [CREDITS.md](../CREDITS.md) must stay.

### 10. Query only an allowlist of USB IDs, read-only (0.1.9)
Vendors use their own protocols, and probing unknown hardware is risky. Other WLmouse models need a product ID entered by the user. **Cost:** every other 2.4 GHz device needs its own support.

### 11. Embed the WebView2 bootstrapper (0.1.9)
The installer carries Microsoft's signed bootstrapper instead of downloading it. **Cost:** a larger installer.

### 12. Scanners next to providers (0.2.0)
Providers describe one known device; Bluetooth and controllers bring an unknown number. A second trait keeps the two simple and needs no setup by the user.

### 13. Read Bluetooth levels from Windows (0.2.0)
Windows already knows the level, so nothing is sent to devices and no pairing code is needed. The container ID joins the nodes of one device and is stable across restarts. **Cost:** the property is undocumented, and only devices whose level Windows knows can work.

### 14. Discovered devices leave the indicators when away (0.2.0)
Otherwise every device ever seen would fill the taskbar. Fixed devices keep their last value, because a sleeping mouse does not report one.

### 15. The app owns the list of known devices (0.2.0)
`DeviceSettings.known` is written only by the backend; saving from the interface keeps it, so an old copy in the window cannot overwrite it.

### 16. Nothing personal in exports (0.1.8, device requests 0.2.0)
Device requests contain no serial numbers, addresses or Bluetooth names, because owners put their own name into those. Diagnostics hide the profile folder.

### 17. CI runs on `main` are not cancelled (after 0.1.9)
A cancelled run shows as failing on the badge.

### 18. The uninstaller tells an update from a real uninstall by its own path (0.2.1)
An update runs the old uninstaller in place and without `/UPDATE`, so `$UpdateMode` does not help. A real uninstall runs from a temporary copy. **Cost:** relies on how NSIS and Tauri's template work; check it when updating Tauri.

### 19. The tray icon is the full logo (0.2.2)
Black tile included, as everywhere else. It looks the same on dark and light taskbars, so it does not follow the theme. It is drawn from the shapes of `assets/app-icon.svg` to be sharp at every size. **Cost:** the battery in it is small at 16 pixels.
