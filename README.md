# DualSense Command Center

DualSense Command Center is a local Windows app for PlayStation DualSense and DualSense Edge controllers.

It runs a lightweight background agent, starts from a tray launcher, and serves a local web UI for controller status, profiles, adaptive trigger tuning, lightbar output, haptics, Steam Input helpers, and game telemetry integrations.

The first supported live telemetry target is Forza Data Out. The project is built around modules so future releases can add other games that expose telemetry data without redesigning the app.

<img width="2095" height="1422" alt="image" src="https://github.com/user-attachments/assets/9a6c4e78-2c9a-40ba-bc5a-a658f13dadfb" />

## Current Release

- Version: `0.1.9`
- Platform: Windows x86_64
- Package: `.msi` installer from GitHub Releases
- License: Apache License 2.0

Download the latest installer from the [GitHub Releases page](https://github.com/shiftedx/dualsense-command/releases).

The installer adds:

- DualSense Command Center tray app
- Local DSCC agent
- Start menu shortcuts
- Optional startup entry
- Bundled web UI served locally by the agent

The MSI is currently unsigned, so Windows may show a SmartScreen or publisher warning during install.

## Features

- DualSense and DualSense Edge discovery through `hidapi`.
- Local HTTP/WebSocket API served by the agent.
- Svelte web UI served from the installed app.
- Built-in Forza profiles with profile create, edit, delete, import, and export support.
- Adaptive trigger curves, rumble rules, lightbar output, and controller status views.
- Forza Data Out parsing with live telemetry status and stale-telemetry fallback.
- Steam Input inspection and binding helper APIs.
- Controller glyph helper for supported Forza installs.
- CLI diagnostics for paths, devices, HID listing, mock devices, and agent status.
- `DSCC_DISABLE_HARDWARE_OUTPUT=1` for diagnostics-only runs.

## Supported Games

### Forza

DSCC currently focuses on Forza telemetry through the in-game Data Out / UDP Race Telemetry feature.

To connect Forza telemetry:

1. Open the game settings.
2. Enable Data Out / UDP Race Telemetry.
3. Set the target IP to `127.0.0.1`.
4. Set the target port to `5300`.
5. Start DualSense Command Center from the Start menu or tray app.

The local UI is served at:

```text
http://127.0.0.1:43473/
```

## Controller Glyphs

The Forza controller glyph toggle can install PlayStation-style button glyphs for supported Forza installs.

When the toggle is enabled, DSCC first saves the original `ControllerIcons.zip` files beside the game files as DSCC backups. When the toggle is disabled, DSCC restores those saved originals.

If DSCC cannot find a safe original file or backup, it refuses to overwrite the game icons and asks the user to verify the game files first. This prevents the app from leaving PlayStation glyphs installed without a path back to the original icons.

## Project Layout

```text
crates/
  dscc-core/       Core profile and effect evaluation model
  dscc-device/     HID device discovery and output transport
  dscc-telemetry/  Shared telemetry signal types
  dscc-adapters/   Game/source telemetry adapters
  dscc-agent/      Local API, runtime state, profiles, and integrations
  dscc-tray/       Windows tray launcher
  dscc-cli/        Diagnostics and utility commands
web/               Svelte web UI
docs/              Public module format documentation
packaging/         Windows MSI packaging scripts
```

Public module format notes live in `docs/module-manifest-format.md`.

Local research notes, planning documents, assistant files, generated builds, release archives, `target/`, `web/dist/`, and `web/node_modules/` are intentionally ignored and should not be committed.

## Development

Install the Rust and Node.js toolchains, then build the web UI and Rust workspace:

```bash
cd web
npm ci
npm run build
cd ..
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Run the local agent during development:

```bash
cargo run -p dscc-agent -- 127.0.0.1:43473
```

Build the Windows release binaries:

```bash
cargo build -p dscc-agent -p dscc-tray --release --target x86_64-pc-windows-gnu
```

Build the MSI:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File packaging\package-msi.ps1 -Version 0.1.9 -TargetTriple x86_64-pc-windows-gnu
```

## License

DualSense Command Center source code is licensed under the Apache License, Version 2.0. Third-party dependencies and bundled visual assets retain their own terms where applicable.
