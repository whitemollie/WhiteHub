# Security

## Reporting a vulnerability

Please do not open a public issue. Use **Security → Report a vulnerability** on this repository's GitHub page. Only the latest release receives fixes.

## Design

- Runs as the current user, never needs administrator rights and installs no drivers.
- No network access in normal operation. If the WebView2 runtime is missing, the installer downloads it from Microsoft.
- Sends fixed, read-only battery queries only to listed HyperX dongles and to devices with the WLmouse USB vendor ID. Setting a product ID by hand narrows the WLmouse search, or points the HyperX query at another device of the same vendor.
- Device replies are validated (length, header, command, range). Text from USB descriptors is stripped of control characters.
- The window shows only bundled files: strict content security policy, no external navigation, frozen prototypes and a minimal set of permissions.
- Settings are validated on load. A broken file is set aside and replaced by defaults.
- Diagnostic reports replace the user's profile folder. They still contain device names, settings and the log, so read them before sharing.

## Known limits

- Releases are not code-signed.
- The taskbar strip is a child window of the Windows taskbar, which is not an official extension point.
- `cargo audit` reports `ttf-parser` as unmaintained. It parses only fonts from the Windows folder. The other warnings concern Linux-only dependencies that are not part of the Windows build.
