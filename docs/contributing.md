# Contributing

Thanks for helping DSCC. Keep changes focused, avoid broad rewrites, and do not
copy implementation details from incompatible projects.

## Before You Start

```powershell
git status --short --ignored
```

- Use `rg` or `rg --files` when searching.
- Do not revert unrelated local changes.
- Do not commit private notes, raw captures, build output, MSI files, or local
  agent instructions.
- If you touch HID reports, telemetry packet layouts, controller assets, Sony
  tooling, Steam Input, or protocol constants, document the public source or
  original experiment in the PR.

## Setup

Install Rust, Node.js 24, and the Windows GNU Rust target/toolchain used by the
project. On Linux, install `libudev-dev` for `hidapi`.

```powershell
npm.cmd --prefix web ci
```

Useful root commands:

```powershell
npm.cmd run dev
npm.cmd run check
npm.cmd run check:web
npm.cmd run check:rust
```

PowerShell note: use `npm.cmd`; `npm.ps1` may be blocked by execution policy.

On this Windows host, plain `cargo` may fail because MSVC `link.exe` is not on
`PATH`. Use:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
```

## Local Modes

Run the full local app:

```powershell
npm.cmd run dev
```

Run only the agent:

```powershell
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- serve --addr 127.0.0.1:43473
```

Run UI-only mock mode:

```powershell
npm.cmd --prefix web run dev:mock
```

Mock mode is for development only. Production builds ignore mock toggles and do
not include the fixture bundle.

Run without writing to real controller hardware:

```powershell
$env:DSCC_DISABLE_HARDWARE_OUTPUT='1'
# or
$env:DSCC_ENABLE_HARDWARE_OUTPUT='0'
```

## LAN Policy

Normal users enable LAN Access in the app. Direct agent launches that bind to a
non-loopback address require explicit opt-in:

```powershell
$env:DSCC_ENABLE_LAN_API='1'
$env:DSCC_ENABLE_LAN_FORZA='1'
```

The tray may pass `DSCC_ENABLE_LAN_API=1` so the UI can save the LAN setting,
but the saved `listenOnAllInterfaces` setting still controls actual exposure.

## Frontend Changes

- Keep `App.svelte` as the shell and state coordinator.
- Put feature code in `web/src/lib/features/<feature>/` when possible.
- Keep API calls in `web/src/lib/api.ts`.
- Keep shared UI types in `web/src/lib/types.ts`.
- Use Svelte 5 event attributes such as `onclick` and `oninput`.
- Use `@lucide/svelte` icons when adding controls.
- Clean up timers, sockets, listeners, and polling.
- Keep expensive Steam or filesystem work out of render paths.
- Preserve the dense app UI style. This is an operational tool, not a landing
  page.

Global Profile is controller-only tuning. Do not show telemetry streams, RPM
controls, adapter packet status, or game-signal routing until a supported game
profile is selected.

## Backend/API Changes

- Add typed request/response structs.
- Validate input before touching state, hardware, or the filesystem.
- Keep mutating routes behind the same-origin guard.
- Persist durable changes with existing state helpers.
- Add route tests for success and failure paths.
- Add cross-origin rejection tests for security-sensitive mutations.

Do not add raw HID-byte routes. Hardware output routes must accept high-level
intent and use typed output/profile paths.

## Game Or Telemetry Changes

DSCC has two module layers:

- **Game modules** identify games and profiles.
- **Adapter modules** read telemetry.

Use a profile pack when the game already works through an existing adapter. Add
a built-in Rust adapter when new parsing, shared memory, filesystem access, or
runtime behavior is needed.

Rules:

- Keep `moduleId` as the game module id and `adapterId` as the telemetry adapter
  id.
- Forza Horizon 5, Forza Horizon 6, and Forza Motorsport remain separate game
  modules even when they share `forza-data-out`.
- Assetto Corsa Rally uses `assetto-shared-memory`.
- Community modules are data-only until DSCC has a sandbox/signing model.

See [Game Module Contribution Guide](game-module-contribution-guide.md).

## Controller Output Changes

Controller output has a hard boundary:

- Frame model: `crates/dscc-core`
- Encoding/clamping: `crates/dscc-device/src/output.rs`
- HID transport: `crates/dscc-device/src/hidapi_transport.rs`
- Runtime write path: `ControllerOutputManager` and agent output loops

Keep these promises:

- No raw report bytes in the API.
- Manual tests are time-limited.
- Stale/no-telemetry game state keeps triggers and rumble neutral.
- Supported-game detection may emit lightbar-only output.
- DualSense Edge onboard writes are USB-only and staged locally when hardware
  sync is unavailable.

## Steam Input Changes

Steam Input writes touch user files. Preserve these guards:

- Write only guarded `controller_*.vdf` files.
- Never write `controller_base*.vdf`.
- Keep canonical Steam root checks.
- Keep the 256 KB layout file limit.
- Honor `dryRun`.
- Create backups before real writes.
- Preserve `groupId`, source, source mode, input id, and activator identity.

Run the button mapping guard when changing this area:

```powershell
npm.cmd --prefix web run test:button-map
```

## Validation

For docs-only changes, inspect the diff. For code changes, run the smallest
validation set that covers the risk:

```powershell
npm.cmd run check:web
npm.cmd run check:rust
npm.cmd run check
```

For UI changes, also open the local app and verify the affected screen.
