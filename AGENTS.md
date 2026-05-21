# AGENTS.md

This is the working guide for future agents in this repo. Keep it current when
tooling, module boundaries, validation commands, or safety constraints change.

## First Moves

- Start with `git status --short --ignored` and assume the worktree may already
  contain user edits. Do not revert unrelated changes.
- Search with `rg` or `rg --files`; use `rg --no-ignore` when checking ignored
  handoff, validation, or plan docs.
- Read `PROVENANCE.md` before implementing anything involving HID reports,
  Forza telemetry, Steam Input, Sony tooling, controller assets, packet layouts,
  schemas, or protocol constants.
- Keep changes narrow. This repo is being trimmed, cleaned, and optimized before
  a larger UI shift.

## Windows Host Toolchain

On this Windows host, plain `cargo` uses the MSVC toolchain, but `link.exe` is
not installed or not on `PATH`. If you run plain `cargo test`, `cargo clippy`,
or `cargo build`, failures may happen before project code is exercised:

```text
error: linker `link.exe` not found
```

Use the installed GNU toolchain for local Rust checks and builds:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-gnu build -p dscc-agent -p dscc-tray --release --target x86_64-pc-windows-gnu
```

In PowerShell, prefer `npm.cmd`; `npm.ps1` can be blocked by execution policy:

```powershell
npm.cmd --prefix web ci
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

CI still uses normal Rust stable plus Node 24 on GitHub runners. Linux builds
need `libudev-dev` for `hidapi`.

## Repo Map

- `Cargo.toml`: Rust workspace, edition 2021, rust-version 1.86.
- `.github/workflows/ci.yml`: CI contract for Rust and web checks.
- `crates/dscc-core`: Core domain types, profiles, effect rules, output frame
  model.
- `crates/dscc-telemetry`: Telemetry contracts.
- `crates/dscc-adapters`: Built-in telemetry catalog plus clean-room UDP parser
  registrations. The first live parser is Forza Data Out.
- `crates/dscc-device`: HID boundary, sanitized enumeration, registry,
  diagnostics, output encoding, and hidapi transport.
- `crates/dscc-agent`: Main runtime/API: state, persistence, generic UDP
  adapter runtime, profile resolution, Steam Input, routes, and WebSocket.
- `crates/dscc-cli`: Diagnostics commands such as paths, status, devices, HID
  listing, and probing.
- `crates/dscc-tray`: Windows tray launcher; starts the agent hidden and opens
  the local UI.
- `web`: Plain Svelte 5 + Vite app, not SvelteKit.
- `packaging/package-msi.ps1`: MSI staging/signing flow; expects release
  binaries and `web/dist`, downloads WiX into `target`.
- `docs/module-manifest-format.md`: Draft public module manifest format;
  community module installer/loader is not implemented yet.

## Clean-Room Rules

- Do not inspect, copy, or derive implementation details from AGPL/local clone
  projects such as `Forza-Horizon-DualSense-Python`.
- Do not copy code, constants, packet layouts, schemas, comments, defaults, or
  structure from incompatible implementations.
- Record new public sources, experiments, and hardware validation notes in
  `PROVENANCE.md` or `docs/hardware-validation.md`.
- Community modules stay data-only; do not accept executable module code.

## Backend And Security Boundaries

- This is a pre-1.0 app. Do not add compatibility shims, legacy input aliases,
  or old-shape endpoint handling unless the user explicitly asks for a migration.
  If the current modular model needs a different field or route contract, update
  the producer and consumers to the clean contract.
- The agent API and UDP adapter listeners are loopback-first. Defaults are API
  `127.0.0.1:43473` and the current Forza adapter on `127.0.0.1:5300`.
- LAN exposure requires explicit opt-in: `DSCC_ENABLE_LAN_API=1` for
  all-interface API binding and `DSCC_ENABLE_LAN_FORZA=1` for non-loopback
  binding of the current Forza UDP adapter.
- Mutating API routes rely on same-origin `Origin`/`Host` checks. Preserve
  cross-origin rejection for non-GET requests and WebSocket upgrades.
- Treat these as security-sensitive mutation surfaces: `/api/app-settings`,
  `/api/steam-input/bindings`, profile create/update/delete/import/activate,
  `/api/profile-resolution/override`, controller rename/config writes, Edge
  profile staging, and effect-test routes.
- Do not add raw HID-byte write APIs. Hardware output must flow through validated
  `ControllerOutputFrame` paths.
- Current code defaults to real hardware output unless
  `DSCC_DISABLE_HARDWARE_OUTPUT=1` or `DSCC_ENABLE_HARDWARE_OUTPUT=0` is set.
  Use dry-run env for diagnostics and verify current code before changing docs
  or tests around output safety.
- Output report encoding, clamping, and Bluetooth CRC live in
  `crates/dscc-device/src/output.rs`; HID write suppression lives in
  `crates/dscc-device/src/hidapi_transport.rs`.
- Edge onboard profile writes are staged only; hardware sync is intentionally
  disabled.
- Steam Input writes are allowed only through guarded `controller_*.vdf` paths.
  Preserve canonical Steam root checks, the 256KB file limit, backup creation,
  and dry-run behavior.
- Forza glyph install/restore modifies `ControllerIcons.zip` under a trusted FH6
  install root only. Preserve canonical root checks, backup files, temp-file
  replacement, and refusal on missing or unbacked originals.
- State persistence writes through `DSCC_CONFIG_DIR` or OS app config dirs. Tests
  that persist state should isolate with temp config dirs.
- Sanitize HID paths, Steam userdata paths, PnP values, serials, Bluetooth
  addresses, and raw report bytes in logs, docs, tests, and API output.

## Frontend Notes

- `web/src/main.ts` mounts `web/src/App.svelte`; Vite config is
  `web/vite.config.ts`.
- `web/src/App.svelte` owns most UI state and workflows.
- `web/src/lib/api.ts` contains browser API calls, snapshot normalization,
  WebSocket handling, and fallback polling.
- `web/src/lib/types.ts` defines UI-side DTO/domain types.
- The `#/games` entry view selects a stable target controller plus either Global
  Profile or a supported game. Global Profile is the default scope. Controller
  display names are editable aliases; never use them as identity for profile
  resolution.
- Global profile scope is controller-only tuning. Keep telemetry streams, RPM
  controls, adapter packet status, and game-signal routing hidden until a game
  profile is selected.
- Use Svelte 5 event attributes such as `onclick` and `oninput`; existing code
  also uses `$:` reactive declarations, `onMount`, and `vitePreprocess`.
- Use `@lucide/svelte` for icons when adding controls.
- Clean up timers, listeners, and polling in `stopAppRuntime` or the relevant
  component teardown path.
- Dev server: `npm.cmd --prefix web run dev`, usually on `127.0.0.1:5173`, with
  `/api` proxied to the Rust agent on `127.0.0.1:43473`.

## Button Mapping View

- The route is hash-based: `#/button-mapping`.
- Button mapping logic and the extracted view live in `web/src/lib/features/buttonMapping`.
- The p95 guard lives in `web/scripts/button-mapping-p95.mjs`.
- Mapping assets live under `web/public/dualsense/...`.
- Global styling is in `web/src/styles/app.css`; `Tooltip.svelte` has local
  tooltip styles.
- The mapping view is a Steam Input mirror for the selected game/controller
  layout. Preserve Steam source/group identity when reading and writing:
  `inputId` alone is not enough because D-pad and center-trackpad swipes can
  share ids such as `dpad_north`; use `groupId`, source, source mode, and
  activator in slot matching and write requests.
- The mapping stage uses a 0-100 coordinate system. `.dm-mapping-stage` has
  `aspect-ratio: 2 / 1`, the controller art is about 54% width, and chips use
  `--chip-x` / `--chip-y`.
- Preserve the dense, console-like app feel. Avoid marketing-page structure while
  this remains an operational tool.
- Keep Steam/Input filesystem scans off hot UI paths. Discovery is cached,
  layout scans are bounded, and parsed bindings are truncated at 64.

## Module Boundaries

- Game detection uses `moduleId` for the detected game module id and `adapterId`
  for the telemetry adapter id. Do not overload one field to mean both.
- Adapter runtime state tracks listener binding, packet health, parse errors,
  and packet rate by adapter id.
- Game/effect-specific runtime state, such as Forza shift-event latching, must
  stay outside the generic adapter runtime.

## Performance And Validation

Button mapping performance guard:

```powershell
npm.cmd --prefix web run test:button-map
```

Current budgets in `web/scripts/button-mapping-p95.mjs`:

- `lookup <= 8ms`
- `chipModel <= 8ms`
- `parse <= 2ms`
- 300 samples

Common local validation set:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

Runtime timing assumptions to preserve unless deliberately changing behavior:

- Hardware output loop: 33ms
- UDP adapter telemetry processing: 33ms
- Stale packet cutoff: 2s
- Manual output refresh: 250ms
- Keepalive: 750ms
- WebSocket invalidation: 500ms

For UI-impacting changes, run the web build and button-map p95 guard, then open
the local app against a running agent. Use the Browser plugin for local visual
verification when the change affects layout or interaction.

## Local Run Commands

Run the agent through the CLI when you want an explicit bind address:

```powershell
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- serve --addr 127.0.0.1:43473
```

Or set the agent env var used by `dscc-agent`:

```powershell
$env:DSCC_AGENT_ADDR='127.0.0.1:43473'
cargo +stable-x86_64-pc-windows-gnu run -p dscc-agent
```

Diagnostics examples:

```powershell
$env:DSCC_DISABLE_HARDWARE_OUTPUT='1'
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- devices diagnose --json
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- devices list-hid --experimental --json --mock
```

Start the web app:

```powershell
npm.cmd --prefix web run dev
```

## Worktree And Artifact Norms

- `AGENTS.md`, `WINDOWS_HANDOFF_PROMPT.md`, `designspec.md`, many
  research/validation docs, `target/`, `web/dist/`, and `web/node_modules/` may
  be ignored/local.
- Do not commit generated builds, release artifacts, MSI files, private lab
  captures, raw HID paths, serials, Bluetooth addresses, or raw report payloads.
- If the user wants committed agent guidance, first update `.gitignore` or move
  this guidance into a tracked doc intentionally.
