# For developers

You do not need any of this to use WhiteHub: download the installer from [Releases](../../releases).

## Build

Requires Windows 11 x64, Rust (stable, MSVC) and Node.js 22+.

```powershell
npm ci
npm run tauri dev                                  # development
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build                                # release build and installer
```

Release builds are made with `RUSTFLAGS="--remap-path-prefix=$env:USERPROFILE\.cargo=cargo"`, so that they contain no local folder names.

## Add a device

Protocols of the supported devices: [docs/PROTOCOLS.md](docs/PROTOCOLS.md).

1. Implement `battery::provider::Provider` in a new module under `src-tauri/src/battery`.
2. Register it in `app.rs` and add its ID to `settings::DEVICE_IDS`. For a source that finds many devices by itself, implement `battery::scanner::Scanner` instead.
3. Match exact USB IDs, send read-only queries only, and validate every reply. Never probe unknown devices.
4. Add unit tests with replies captured from real hardware, without serial numbers.

Not a programmer? [Request a device](../../issues/new?template=device-request.yml) instead.
