# DualSense Command Center

DualSense Command Center is a local Rust and TypeScript app for PlayStation DualSense and DualSense Edge controllers.

It runs a lightweight background agent, opens from a tray launcher, and serves a local web UI for controller status, adaptive trigger tuning, lightbar output, haptics, profiles, Steam Input helpers, and game telemetry integrations.

The first live telemetry target is Forza Data Out. The project is intentionally module-oriented so later releases can add other games that expose telemetry streams without reshaping the whole app.

## Current Status

The current runtime includes:

- Windows/Linux background agent with a local HTTP/WebSocket API.
- Svelte web UI served by the agent from `web/dist`.
- DualSense and DualSense Edge discovery through `hidapi`, with mock transport support for tests.
- Forza Data Out parsing, profile resolution, adaptive trigger effects, rumble output, lightbar output, and stale-telemetry fallback.
- Built-in Forza profiles plus profile CRUD and import/export scaffolding.
- Steam Input inspection and binding helper APIs.
- CLI diagnostics for paths, devices, HID listing, mock devices, and agent status.
- Guarded hardware output, with `DSCC_DISABLE_HARDWARE_OUTPUT=1` available for diagnostics-only runs.

## Repository Layout

```text
crates/
  dscc-core/
  dscc-device/
  dscc-telemetry/
  dscc-adapters/
  dscc-agent/
  dscc-tray/
  dscc-cli/
web/
docs/
```

Public module format notes live in `docs/module-manifest-format.md`. Local research notes, planning documents, assistant files, generated builds, and release archives are intentionally ignored.

## Using a Release Build

Download the Windows x86_64 ZIP from GitHub Releases, extract it, and run `dscc-tray.exe`. The tray starts the local agent and opens the UI at:

```text
http://127.0.0.1:43473/
```

For Forza, enable Data Out / UDP Race Telemetry in the game settings, set the target IP to `127.0.0.1`, and use port `5300`.

## Development

Useful local commands:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd web && npm ci && npm run build
cargo run -p dscc-agent -- 127.0.0.1:43473
```

Build the web UI before running a packaged agent:

```bash
cd web && npm ci && npm run build
```

## License

DualSense Command Center source code is licensed under the Apache License, Version 2.0. Third-party dependencies and bundled visual assets retain their own terms where applicable.
