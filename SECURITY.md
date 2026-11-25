# Security

## Reporting a vulnerability

Please do not open a public issue. Use **Security → Report a vulnerability** on this repository's GitHub page. Only the latest release receives fixes.

## Design

- Runs as the current user, never needs administrator rights and installs no drivers.
- No network access in normal operation. The installer contains Microsoft's WebView2 bootstrapper, which downloads the runtime from Microsoft if it is missing.
- Sends fixed, read-only battery queries only to an allowlist of USB devices. Setting a product ID by hand replaces that list with the chosen product of the same vendor.
- Events pushed by a device are rate-limited.
- Device replies are validated (length, header, command, range). Text from USB descriptors is stripped of control characters.
- The window shows only bundled files: strict content security policy, no external navigation, frozen prototypes and a minimal set of permissions.
- Settings are validated on load. A broken file is set aside and replaced by defaults.
- Diagnostic reports replace the user's profile folder. They still contain device names, settings and the log, so read them before sharing.

## Known limits

- Releases are not code-signed.
- The taskbar strip is a child window of the Windows taskbar, which is not an official extension point.
- `cargo audit` reports `ttf-parser` as unmaintained. It parses only fonts from the Windows folder. The other warnings concern Linux-only dependencies that are not part of the Windows build.
