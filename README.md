# DualSense Command Center

DualSense Command Center is a local-first control center for PlayStation DualSense and DualSense Edge controllers. It runs a lightweight Rust agent, a Windows tray launcher, and a Svelte web UI for profiles, adaptive triggers, haptics, lightbar control, Steam Input helpers, controller diagnostics, and racing telemetry.

The app is built around two extension concepts:

- **Adapters** own protocol/runtime plumbing, such as Forza Data Out UDP or Assetto shared memory.
- **Game modules** own one supported game, including detection hints, profile defaults, labels, and adapter bindings.

<img width="2095" height="1422" alt="DualSense Command Center haptics UI" src="https://github.com/user-attachments/assets/a3481779-af9d-46dd-bbc6-c544573d807e" />

## Current Status

- Latest release: `0.2.8`
- Primary platform: Windows x86_64
- Package: unsigned Windows MSI from GitHub Releases
- Linux status: experimental raw binaries only
- License: Apache License 2.0
- App maturity: pre-1.0 beta with real hardware output enabled by default

Download the latest build from the [GitHub Releases page](https://github.com/shiftedx/dualsense-command/releases/latest).

The Windows installer includes:

- DSCC tray app
- Local DSCC agent
- Bundled web UI served by the agent
- Start menu shortcuts
- Optional startup entry

The MSI is unsigned, so Windows SmartScreen or managed endpoint policy may warn during install. Published release assets include SHA256 checksum files.

Profiles, controller aliases, app settings, and staged Edge slot data are stored in the user config directory, not in the install folder. During install or upgrade, the MSI backs up the existing `state.json` to `state.preinstall-<version>.json` before launching the updated tray app.

## What Works Today

### Controller Runtime

- Discovers DualSense and DualSense Edge controllers through `hidapi`.
- Tracks stable controller ids separately from editable display aliases.
- Shows connection, battery, permission, diagnostics, and capability state.
- Reads live trigger input for tuning/test workflows.
- Encodes controller output through validated `ControllerOutputFrame` paths; DSCC does not expose raw HID-byte write APIs.
- Supports USB and Bluetooth output paths where the HID backend can open the controller.

Hardware output is enabled by default. Use one of these for diagnostics-only dry runs:

```powershell
$env:DSCC_DISABLE_HARDWARE_OUTPUT='1'
# or
$env:DSCC_ENABLE_HARDWARE_OUTPUT='0'
```

### Profiles And Tuning

- Global profile scope for controller-only trigger, rumble, and lightbar tuning.
- Per-game profiles for supported game modules.
- Profile create, edit, rename, delete, import, export, save-as, and activation flows.
- Built-in Forza and Assetto Corsa Rally profile templates.
- 4-8 point adaptive trigger curve editor for L2 and R2.
- Live graph preview aligned with the backend force model for telemetry profiles.
- Manual trigger and body-rumble tests that run only during the requested test phase.
- Custom lightbar color, brightness, RPM color, and player LED behavior.
- Forza body-rumble mode that defaults to native passthrough and adds DSCC event cues for short effects such as shift and landing thumps.

Global Profile is the default until DSCC detects a supported game with a matching profile. Triggers and rumble are not driven by game effects until fresh telemetry is available.

### Racing Telemetry

DSCC currently has two live telemetry paths:

- `forza-data-out`: UDP adapter for supported Forza Data Out / Race Telemetry variants, default bind `127.0.0.1:5300`.
- `assetto-shared-memory`: Windows shared-memory reader for Assetto Corsa Rally's public Assetto telemetry pages.

Telemetry can drive adaptive triggers, body haptics, lightbar/RPM output, player LEDs, and effect routing. Stale telemetry falls back safely, and hardware output waits for supported-game detection plus fresh telemetry before applying game effects.

### Button Mapping And Steam Input

- `#/button-mapping` mirrors the selected game's Steam Input controller layout.
- Reads Steam Input binding summaries from guarded `controller_*.vdf` files.
- Preserves Steam source, source mode, group id, input id, and activator identity so similarly named controls do not overwrite each other.
- Provides DSCC default mapping overlays when a layout is missing bindings.
- Rejects writes to synthetic/default-only mappings until a real Steam Input layout is available.
- Uses dry-run validation and creates backups before real Steam Input writes.

### DualSense Edge Onboard Slots

The app exposes an experimental DualSense Edge onboard memory panel for assignable Fn slots:

- `Fn + Circle`
- `Fn + Cross`
- `Fn + Square`

DSCC can stage typed static settings locally, including supported trigger, stick, lightbar, vibration, and button settings. When a DualSense Edge is connected over USB and hardware writes are enabled, the API has a guarded write path for supported static onboard profile data. Bluetooth or unavailable hardware paths fall back to staged local state and clearly report that the controller memory was not written.

Live telemetry effects are not stored on the controller. They require DSCC to be running.

### Forza Glyph Helper

Forza Horizon 6 includes an optional PlayStation glyph helper. When enabled, DSCC installs bundled PlayStation-style controller glyph files under a trusted Forza Horizon 6 install root.

Safety rules:

- DSCC only writes under the trusted FH6 install root.
- Original `ControllerIcons.zip` files are backed up before replacement.
- Restore uses the saved originals.
- If originals or backups are missing, DSCC refuses the operation and asks the user to verify game files.

### Updates And LAN

- The app checks GitHub Releases for updates.
- The tray opens the local UI at `http://127.0.0.1:43473/`.
- LAN Access is off by default.
- In installed builds, the tray grants the agent permission to offer the in-app LAN toggle.
- Selecting **Web UI Location -> LAN Access** persists the user opt-in and requires restart; after restart the agent binds to `0.0.0.0:43473`.
- Direct non-loopback agent launches still require `DSCC_ENABLE_LAN_API=1`.
- The Forza UDP adapter remains loopback-first; non-loopback Forza binding requires `DSCC_ENABLE_LAN_FORZA=1`.

## Supported Games

### Forza Horizon 5, Forza Horizon 6, And Forza Motorsport

Forza support uses the in-game Data Out / UDP Race Telemetry feature.

Forza setup:

1. Open the game's HUD/gameplay telemetry settings.
2. Enable Data Out / UDP Race Telemetry.
3. Set target IP to `127.0.0.1`.
4. Set target port to `5300`.
5. Start DualSense Command Center from the Start menu or tray app.
6. Select the detected Forza game/profile in the DSCC UI.

Supported Forza effects include brake pressure, ABS/front slip, handbrake wall, throttle load, paddle shift thump, rev limiter buzz, road texture, rumble strips, tire slip, puddle drag, suspension/impact thumps, RGB/RPM lightbar behavior, and player LEDs.

### Assetto Corsa Rally

Assetto Corsa Rally is the first shared-memory game module. DSCC detects `acr.exe`, watches Assetto-compatible shared-memory pages on Windows, and feeds normalized racing signals into the same haptic/profile runtime used by the rest of the app.

To use it, launch Assetto Corsa Rally, enter a driving session, and select the detected game/profile in DSCC. No Forza UDP port setup is required.

### Adapter Catalog

The repo also catalogs additional adapter directions such as EA F1 UDP, EA SPORTS WRC UDP, BeamNG.drive, Live for Speed, and RaceRoom. These are contribution targets and metadata/catalog entries unless a live parser/runtime is explicitly listed above.

## Safety Model

DSCC is intentionally loopback-first and hardware-gated:

- The local API defaults to `127.0.0.1:43473`.
- The Forza UDP adapter defaults to `127.0.0.1:5300`.
- Mutating API routes reject cross-origin requests.
- Hardware output flows through typed frame models and encoder clamps.
- Game haptics require a supported detected game, an active profile, and fresh telemetry.
- Manual effect tests bypass game gating only for the requested test duration.
- Steam Input writes are limited to guarded `controller_*.vdf` files under trusted Steam roots.
- HID paths, Steam userdata paths, PnP values, serials, Bluetooth addresses, and raw reports are sanitized from API/log/test surfaces.

## Project Layout

```text
crates/
  dscc-core/       Profiles, effect rules, value sources, and output frame model
  dscc-device/     HID discovery, diagnostics, registry, output encoding, transport
  dscc-telemetry/  Shared telemetry signals, snapshots, adapter contracts
  dscc-adapters/   Built-in adapter catalog and telemetry parsers
  dscc-agent/      Local API, state, persistence, profiles, adapters, output loops
  dscc-tray/       Windows tray launcher and agent starter
  dscc-cli/        Diagnostics and utility commands
web/               Svelte 5 + Vite web UI
docs/              Public architecture, contribution, and module docs
packaging/         Windows MSI packaging scripts
```

Useful docs:

- `docs/architecture.md`: Backend/frontend map and runtime flow.
- `docs/contributing.md`: Contributor workflow and safety rules.
- `docs/game-module-contribution-guide.md`: How to add or propose game modules.
- `docs/module-manifest-format.md`: Draft data-only community module format.
- `docs/production-readiness-plan.md`: Release and beta-readiness notes.

Community modules are currently data-only manifest/profile packs. The external installer/loader is not implemented yet. Native telemetry parsers remain built into Rust until DSCC has a parser sandbox/signing model.

## Development

Install Rust, Node.js, and web dependencies:

```powershell
npm.cmd --prefix web ci
```

Common root commands:

```powershell
npm.cmd run check:web
npm.cmd run check:rust
npm.cmd run check
npm.cmd run dev:web
npm.cmd run dev:web:mock
npm.cmd run dev:agent
npm.cmd run dev
```

- `check:web`: Svelte typecheck, button-mapping p95 guard, and Vite build.
- `check:rust`: Rust fmt, workspace tests, and clippy. On this Windows host it uses the installed GNU toolchain.
- `check`: web and Rust checks.
- `dev:agent`: starts `dscc-cli serve --addr 127.0.0.1:43473`.
- `dev:web`: starts Vite on `127.0.0.1:5173` with `/api` proxied to the local agent.
- `dev:web:mock`: starts the dev-only mock UI fixture with no agent, controller, Steam, or game required.
- `dev`: starts the local agent and Vite UI together.

Manual validation set:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

On this Windows development host, plain `cargo` may fail because MSVC `link.exe` is not installed or not on `PATH`. Use the GNU toolchain shown above.

Run the local agent:

```powershell
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- serve --addr 127.0.0.1:43473
```

Build Windows release binaries:

```powershell
cargo +stable-x86_64-pc-windows-gnu build -p dscc-agent -p dscc-tray --release --target x86_64-pc-windows-gnu
```

Build the Windows MSI:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File packaging\package-msi.ps1 -Version 0.2.8 -TargetTriple x86_64-pc-windows-gnu
```

## Clean-Room Policy

DSCC is a clean-room implementation. Do not copy code, constants, packet layouts, schemas, comments, defaults, or structure from incompatible projects. Before changing HID reports, Forza telemetry, Steam Input, Sony tooling, controller assets, packet layouts, schemas, or protocol constants, read `PROVENANCE.md` and record any new public source or hardware experiment there before code depends on it.

## License

DualSense Command Center source code is licensed under the Apache License, Version 2.0. Third-party dependencies and bundled visual assets retain their own terms where applicable.
