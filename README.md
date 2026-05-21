# DualSense Command Center

DualSense Command Center is a local Windows app for PlayStation DualSense and DualSense Edge controllers.

It runs a lightweight background agent, starts from a tray launcher, and serves a local web UI for controller status, profiles, adaptive trigger tuning, lightbar output, haptics, Steam Input helpers, and game telemetry adapters.

The first supported live telemetry adapter is Forza Data Out. The project is built around adapter modules for protocols and game modules for individual supported games, so future releases can add games without redesigning the app.

<img width="2095" height="1422" alt="image" src="https://github.com/user-attachments/assets/9a6c4e78-2c9a-40ba-bc5a-a658f13dadfb" />

## Current Release

- Version: `0.2.0`
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

Profile and controller settings are stored in the user's DSCC config directory, not in the install folder. During install or upgrade, the MSI backs up an existing `state.json` to `state.preinstall-<version>.json` before launching the updated tray app.

## Features

- DualSense and DualSense Edge discovery through `hidapi`.
- Local HTTP/WebSocket API served by the agent.
- Svelte web UI served from the installed app.
- Built-in Forza profiles with global and per-game profile create, edit, delete, import, and export support.
- Controller selection with persisted per-device display names backed by stable controller IDs.
- Rich supported-game selection from Steam metadata and artwork before tuning surfaces are shown.
- Global profile tuning for controller-level triggers, rumble, and lightbar behavior without game telemetry or RPM controls.
- Adaptive trigger curves, rumble rules, lightbar output, and controller status views.
- Game-specific Steam Input mapping mirror with guarded write-back to the selected controller layout.
- Forza Data Out parsing through the generic UDP adapter runtime, with live telemetry status and stale-telemetry fallback.
- Steam Input inspection and binding helper APIs.
- Controller glyph helper for supported Forza installs.
- Module model with protocol adapters and per-game contributions.
- CLI diagnostics for paths, devices, HID listing, mock devices, and agent status.
- Real hardware output is enabled by default; set `DSCC_DISABLE_HARDWARE_OUTPUT=1` or `DSCC_ENABLE_HARDWARE_OUTPUT=0` for diagnostics-only dry runs.
- LAN API or UDP adapter exposure requires explicit opt-in with `DSCC_ENABLE_LAN_API=1`; the current Forza adapter uses `DSCC_ENABLE_LAN_FORZA=1`.

## Supported Games

### Forza

DSCC currently focuses on Forza telemetry through the in-game Data Out / UDP Race Telemetry feature.

Forza Horizon 5, Forza Horizon 6, and Forza Motorsport are treated as separate supported game modules. They may share the same built-in `forza-data-out` adapter when their telemetry protocol is compatible.

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
  dscc-adapters/   Protocol/runtime telemetry adapters
  dscc-agent/      Local API, runtime state, profiles, and adapters
  dscc-tray/       Windows tray launcher
  dscc-cli/        Diagnostics and utility commands
web/               Svelte web UI
docs/              Public module format documentation
packaging/         Windows MSI packaging scripts
```

Draft public module format notes live in `docs/module-manifest-format.md`. Community modules are planned as data-only manifest packs; the installer/loader is not implemented yet. Native telemetry parsers stay built in until a parser sandbox/signing model exists.

Contributor docs live in `docs/contributing.md`, with a backend/frontend map in `docs/architecture.md`. Release readiness gates and unsigned-beta guidance live in `docs/production-readiness-plan.md`.

Local research notes, planning documents, assistant files, generated builds, release archives, `target/`, `web/dist/`, and `web/node_modules/` are intentionally ignored and should not be committed.

## Development

Install the Rust and Node.js toolchains, then install the web dependencies:

```powershell
npm.cmd --prefix web ci
```

Use the root package helpers for common local workflows:

```powershell
npm.cmd run check:web
npm.cmd run check:rust
npm.cmd run check
npm.cmd run dev:web
npm.cmd run dev:web:mock
npm.cmd run dev:agent
npm.cmd run dev
```

`check:web` runs the web typecheck, button-map performance guard, and web build. `check:rust` runs Rust fmt, tests, and clippy; on Windows it selects the installed GNU toolchain, while other platforms use normal `cargo`. `dev` starts the local agent and Vite UI together. `dev:web:mock` starts the UI with a stable in-browser fixture and does not require a running agent, controller, Steam, or Forza.

Manual equivalents:

```bash
cd web
npm ci
npm run typecheck
npm run test:button-map
npm run build
cd ..
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

On the current Windows development host, the default MSVC Rust toolchain fails because `link.exe` is not installed or not on `PATH`. Use the installed GNU toolchain for local verification:

```powershell
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
```

In PowerShell, use `npm.cmd` if script execution policy blocks `npm.ps1`:

```powershell
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run build
```

Run the local agent during development:

```powershell
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- serve --addr 127.0.0.1:43473
```

Build the Windows release binaries:

```bash
cargo build -p dscc-agent -p dscc-tray --release --target x86_64-pc-windows-gnu
```

Build the MSI:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File packaging\package-msi.ps1 -Version 0.2.0 -TargetTriple x86_64-pc-windows-gnu
```

## License

DualSense Command Center source code is licensed under the Apache License, Version 2.0. Third-party dependencies and bundled visual assets retain their own terms where applicable.
