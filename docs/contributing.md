# Contributing To DSCC

This repo is being narrowed and cleaned before a larger UI shift. Keep changes scoped, read the existing feature before editing, and do not revert unrelated work in the tree.

## First Moves

```powershell
git status --short --ignored
```

Use `rg` or `rg --files` for searches. Use `rg --no-ignore` only when checking ignored handoff, validation, or local planning docs.

Before touching HID reports, Forza telemetry, Steam Input, Sony tooling, controller assets, packet layouts, schemas, or protocol constants, read `PROVENANCE.md` and record any new public source or hardware experiment there before implementation depends on it.

## Setup And One-Command Workflows

Install Rust, the Windows GNU Rust target/toolchain used locally, and Node.js. Then install web dependencies:

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

- `check:web`: runs Svelte typecheck, button mapping p95 guard, and Vite build.
- `check:rust`: runs Rust fmt, workspace tests, and clippy. On Windows this selects the installed GNU toolchain; on other platforms it uses normal `cargo`.
- `check`: runs both web and Rust checks.
- `dev:agent`: runs `dscc-cli serve --addr 127.0.0.1:43473`.
- `dev:web`: starts Vite on `127.0.0.1:5173` with `/api` proxied to the local agent.
- `dev:web:mock`: starts Vite with stable in-browser mock data and does not require the agent, controller, Steam, or Forza.
- `dev`: starts the agent and Vite UI together.

On this Windows host, plain `cargo` may fail before project code runs because MSVC `link.exe` is not installed or not on `PATH`. Use the GNU toolchain for manual Rust checks:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
```

In PowerShell, prefer `npm.cmd`; `npm.ps1` can be blocked by execution policy.

## Mock And Dev Modes

For UI-only work without a running agent:

```powershell
npm.cmd --prefix web run dev:mock
```

The mock API is implemented in `web/src/lib/mock` and is dev-only. It can be toggled while running Vite dev with `?mock=1` or `?mock=0`, which persists the choice in `localStorage`, or by setting `VITE_DSCC_MOCK_API=1` / `VITE_DSCC_MOCK=1` before starting Vite. Production builds ignore all mock toggles and do not include the mock fixture bundle.

Use dry-run hardware output for diagnostics or demos that must not write to a controller:

```powershell
$env:DSCC_DISABLE_HARDWARE_OUTPUT='1'
# or
$env:DSCC_ENABLE_HARDWARE_OUTPUT='0'
```

Current agent builds default to real hardware output when the HID backend opens successfully, so set one of those env vars before running effect tests without hardware writes.

Useful local modes:

```powershell
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- mock-devices --json
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- devices list-hid --experimental --json --mock
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- devices diagnose --json
```

For game detection fixtures, either force a process list or disable scanning:

```powershell
$env:DSCC_PROCESS_SCAN_FIXTURE='ForzaHorizon6.exe'
# or
$env:DSCC_DISABLE_PROCESS_SCAN='1'
```

For isolated state while testing persistence:

```powershell
$env:DSCC_CONFIG_DIR="$env:TEMP\dscc-dev-config"
```

For Steam Input fixture roots:

```powershell
$env:DSCC_STEAM_ROOT='C:\path\to\steam-fixture'
```

For alternate local bind addresses:

```powershell
$env:DSCC_AGENT_ADDR='127.0.0.1:43473'
$env:DSCC_FORZA_BIND_ADDR='127.0.0.1:5300'
```

Non-loopback API or UDP adapter binding requires explicit opt-in: `DSCC_ENABLE_LAN_API=1` for the API and `DSCC_ENABLE_LAN_FORZA=1` for the current Forza adapter.

## Add An API Route

1. Find the closest existing handler and DTO in `crates/dscc-agent/src/lib.rs`. Add a small module only if the surrounding ownership is already being extracted.
2. Define typed request/response structs with serde casing that matches the existing API style.
3. Add the handler using axum extractors such as `State`, `Path`, `Query`, and `Json`.
4. Register the route in `app(state)`.
5. For mutations, keep the route under the existing router so `reject_cross_origin_mutations` still applies.
6. Validate inputs before touching state, filesystem, or hardware boundaries.
7. Persist state changes with the existing snapshot helpers when the changed state is durable.
8. Add or update tests using `app(AgentState::mock())`. Cover success, not found/conflict paths, and cross-origin rejection when the surface is security-sensitive.
9. If the UI consumes the route, add the fetch function in `web/src/lib/api.ts` and the UI type in `web/src/lib/types.ts`.

Do not add raw HID-byte routes. Output routes must accept high-level, validated intent and produce a `ControllerOutputFrame`.

## Add A Svelte Feature

1. Add UI contracts to `web/src/lib/types.ts` if the feature crosses the API boundary.
2. Add fetch/normalization code to `web/src/lib/api.ts`.
3. Put feature-local views, pure transforms, layout models, parsers, and performance-sensitive helpers in `web/src/lib/features/<feature>/`.
4. Export feature entry points from `web/src/lib/features/<feature>/index.ts`.
5. Wire the view through `App.svelte` only for shell state, hash routing, snapshot data, and app-level actions.
6. Use Svelte 5 event attributes such as `onclick` and `oninput`.
7. Use `@lucide/svelte` for icons when adding controls.
8. Clean up timers, listeners, sockets, and polling in `stopAppRuntime` or component teardown.
9. Keep the dense operational UI style. Avoid marketing-page structure and avoid expensive discovery work on hot render paths.

For UI-impacting changes, run the web checks and visually verify the local app against a running agent.

Global profile scope is controller-only tuning. Do not show telemetry streams, RPM controls, adapter packet status, or game-signal routing until a supported game profile is selected. Controller display names are editable labels only; profile resolution must use the stable controller id returned by the API.

## Add Or Update A Module

Keep module ownership explicit:

- Adapter modules own protocol/runtime plumbing. A runnable UDP adapter owns parser/runtime glue for one protocol, such as Forza Data Out.
- Game modules are one supported game each, with their own game id, detection hints, profiles, labels, and adapter dependencies.
- Game detection responses use `moduleId` for the game module and `adapterId` for the telemetry adapter. Do not overload one as the other.
- Forza Horizon 5, Forza Horizon 6, and Forza Motorsport should remain separate game modules even when they share the `forza-data-out` adapter.
- Community modules are currently data-only manifest packs. They may contribute metadata and profile templates, but not native parsers, executable code, process scanners, filesystem writers, or runtime hooks.

See `docs/game-module-contribution-guide.md` for the contribution checklist and the built-in Assetto Corsa Rally reference module.

Choose the contribution path before editing:

1. Use a data-only profile pack when the game can use an existing built-in adapter and the contribution is profiles, metadata, or licensed assets. The loader is not implemented yet, so validate by importing each included `dev.dscc.profile.v1` profile through the profile import API/UI.
2. Propose a built-in Rust adapter when new telemetry parsing, shared-memory access, filesystem access, process logic, or runtime behavior is needed.
3. Propose a built-in Rust game module when adding first-party game detection, bundled presets, glyph helpers, or an adapter binding before the community loader exists.

Built-in game module metadata lives in `crates/dscc-agent/src/game_modules.rs`. Built-in adapter metadata, UDP parser registrations, and parser implementations live in `crates/dscc-adapters`. The agent starts registered runnable UDP adapters and routes normalized `SignalUpdate`s into snapshots. `/api/modules` exposes both adapter modules and built-in game modules.

Native parser contributions must stay built into Rust until DSCC has a parser sandbox/signing model. See `docs/module-manifest-format.md` for the public manifest shape.

Because DSCC is still pre-1.0, prefer clean contracts over compatibility shims. If a route, DTO, or UI caller expects an old adapter-shaped field, update that caller to the current game-module/adapter-module model instead of translating around it.

## Add A Controller Output Feature

Controller output has a hard boundary:

- High-level frame model: `crates/dscc-core/src/lib.rs`.
- Encoding and clamping: `crates/dscc-device/src/output.rs`.
- HID write suppression: `crates/dscc-device/src/hidapi_transport.rs`.
- Runtime write path: `ControllerOutputManager` and agent output loops.

When adding an output feature:

1. Model the feature as a typed `ControllerOutputFrame` field or existing `TriggerOutput`, `LightbarOutput`, `PlayerLedsOutput`, or `RumbleOutput` variant.
2. Clamp and encode in `dscc-device`; do not leak raw report bytes through the API.
3. Preserve `OutputMode::DryRunHid` and `OutputMode::HardwareOutput` behavior.
4. Route previews and tests through `/api/controllers/:id/test-effect` or another high-level route.
5. Add USB/Bluetooth encoder tests and dry-run write tests with `MockTransport`.
6. Use `DSCC_DISABLE_HARDWARE_OUTPUT=1` or `DSCC_ENABLE_HARDWARE_OUTPUT=0` for local diagnostics that must not write to hardware.
7. Record hardware validation notes in `docs/hardware-validation.md` or `PROVENANCE.md` when behavior depends on a new observation.

## Add A Steam Input Binding Feature

Steam Input discovery and writes are guarded because they touch user files:

- Status API: `GET /api/steam-input`.
- Write API: `POST /api/steam-input/bindings`.
- UI feature: `#/button-mapping` and `web/src/lib/features/buttonMapping`.

When changing this area:

1. Keep filesystem scans cached and bounded; do not move Steam scans into hot UI paths.
2. Preserve canonical Steam root checks and `DSCC_STEAM_ROOT` fixture support.
3. Write only guarded `controller_*.vdf` files, never `controller_base*.vdf`.
4. Keep the 256 KB layout file limit.
5. Validate binding kinds and targets before writing.
6. Honor request `dryRun`.
7. Create a timestamped backup before a real write.
8. Return sanitized layout paths in API responses.
9. Update button mapping pure helpers and the p95 guard when changing slot lookup, chip models, or binding parsing.

The button mapping UI should mirror the selected game's Steam Input layout, not a generic translated layout. Preserve `groupId`, source, source mode, and activator through reads and writes; several Steam sources reuse input ids such as `click` or `dpad_north`, so matching on `inputId` alone can update the wrong control.

## Add A Forza Effect

Forza effect work touches telemetry, profiles, controller output, and clean-room rules.

1. Read `PROVENANCE.md` first. If a new signal, tuning rule, packet field, or public reference is needed, record the source or experiment before using it.
2. Add parser signals to the `forza-data-out` adapter only from approved public documentation or original experiments.
3. Use lowercase dotted signal names through `dscc-telemetry`.
4. Keep game identity and detection metadata attached to the relevant game module. Effect tuning presets still live in the agent preset helpers until that layer is extracted.
5. Add UI metadata in `web/src/App.svelte` only until that feature is extracted; keep UI-side route/type additions in `web/src/lib/types.ts`.
6. Implement trigger rules in `forza_runtime_profile` when the output is rule-driven.
7. Implement body rumble, lightbar, or player LED layers in `apply_forza_output_enhancements` and its helpers.
8. Preserve stale/menu behavior: stale Forza streams keep safe baseline tension while the game is still detected, and neutralize after the game exits.
9. Add tests for live telemetry, stale telemetry, disabled effect behavior, routing, and profile preset defaults.

Timing assumptions to preserve unless the task explicitly changes them:

- Hardware output loop: `33ms`.
- UDP adapter telemetry processing: `33ms`.
- Stale packet cutoff: `2s`.
- Manual output refresh: `250ms`.
- Keepalive: `750ms`.
- WebSocket invalidation debounce: `500ms`.

## Button Mapping P95 Guard

Run the guard when changing button mapping logic, Steam binding parsing, chip model creation, or the mapping view data path:

```powershell
npm.cmd --prefix web run test:button-map
```

The guard lives at `web/scripts/button-mapping-p95.mjs` and currently samples 300 iterations with these budgets:

- lookup p95: `<= 8ms`
- chip model p95: `<= 8ms`
- parse p95: `<= 2ms`

Keep hot-path helpers pure and bounded so the guard remains stable on local Windows hardware and in CI.

## Validation Checklist

For docs-only changes, inspect the diff. For code changes, use the smallest validation set that covers the risk:

```powershell
npm.cmd run check:web
npm.cmd run check:rust
npm.cmd run check
```

For UI changes, run the agent and web app, then verify the affected screen locally:

```powershell
npm.cmd run dev
```
