# Architecture

This is a quick map for contributors. It explains where the main pieces live
and which boundaries should stay intact.

## Big Picture

DSCC has three visible parts:

- **Tray app**: starts/stops the local agent and opens the UI.
- **Local agent**: owns controllers, profiles, telemetry, safety gates, and API
  routes.
- **Web UI**: the Svelte app users interact with in the browser.

The app is local-first. The normal UI/API address is `127.0.0.1:43473`.
Forza telemetry listens on `127.0.0.1:5300`.

## Rust Crates

| Crate | Purpose |
| --- | --- |
| `dscc-core` | Profiles, effect rules, telemetry value sources, and typed controller output frames. |
| `dscc-telemetry` | Shared signal names, snapshots, adapter status, and adapter traits. |
| `dscc-adapters` | Built-in adapter catalog and telemetry parsers, including Forza Data Out. |
| `dscc-device` | HID discovery, diagnostics, output encoding, input reads, and guarded device writes. |
| `dscc-agent` | Local API, persistence, profile resolution, game detection, Steam Input, telemetry runtimes, and hardware output loops. |
| `dscc-tray` | Windows tray launcher. The binary entrypoint is `src/main.rs`; the Windows implementation lives in `src/windows_tray.rs` with focused `health`, `menu`, `painting`, and test submodules. |
| `dscc-cli` | Diagnostics and local helper commands. |

## Agent Modules

Useful entry points:

- `crates/dscc-agent/src/main.rs`: agent binary.
- `crates/dscc-agent/src/lib.rs`: state construction, runtime coordination, and
  module wiring.
- `crates/dscc-agent/src/routes.rs`: API/static route table.
- `crates/dscc-agent/src/api/`: focused route handlers.
- `crates/dscc-agent/src/runtime_constants.rs`: loop timing, cache TTLs,
  trigger-force constants, and timestamp helpers.
- `crates/dscc-agent/src/built_in_presets.rs`: built-in racing profile
  presets and default trigger curves.
- `crates/dscc-agent/src/runtime_paths.rs`: tracing setup and OS app
  config/data/log path discovery.
- `crates/dscc-agent/src/effects/`: effect materialization, runtime profile
  output, output-frame enhancement, and manual effect-test helpers.
- `crates/dscc-agent/src/bind_addr.rs`: loopback/LAN binding policy.
- `crates/dscc-agent/src/env_policy.rs`: hardware output env policy.
- `crates/dscc-agent/src/game_modules.rs`: built-in supported games.
- `crates/dscc-agent/src/game_detection/`: Steam, local app, process scan, and
  catalog detection helpers.
- `crates/dscc-agent/src/forza_glyphs.rs`: guarded Forza Horizon 6 glyph install/restore.
- `crates/dscc-agent/src/http_security.rs`: same-origin mutation guard.

## Device Boundary

- `crates/dscc-device/src/output.rs`: output-manager sessions, guarded writes,
  input reads, and Edge onboard profile dispatch.
- `crates/dscc-device/src/output/input.rs`: normalized DualSense input report
  parsing for sticks, triggers, and buttons.
- `crates/dscc-device/src/output/encoding.rs`: typed DualSense USB/Bluetooth
  output report construction and CRC.

Important routes include status, snapshots, controllers, controller input,
profiles, Edge onboard profiles, adapters, Steam Input, Steam library/custom
games, game art, modules, game detection, telemetry, update checks, logs,
diagnostics, and `/api/ws`.

Update checks are link-only: the agent checks GitHub Releases, the web UI can
show a download banner, and the tray opens the latest release page. DSCC does
not auto-install updates.

## Runtime Flow

1. The tray starts the agent, or a developer starts `dscc-cli serve`.
2. The agent scans controllers through `hidapi`.
3. Telemetry runtimes start for registered sources:
   - `forza-data-out`: UDP, default `127.0.0.1:5300`.
   - `assetto-shared-memory`: Windows shared memory.
4. Profile resolution chooses Global Profile or a supported game profile.
5. The effect engine turns profile rules into a typed controller output frame.
6. Hardware output writes only after safety gates pass.
7. The web UI receives state through `/api/snapshot` and `/api/ws`.

Supported-game detection may set the lightbar before telemetry arrives. Trigger
and rumble effects require fresh telemetry or a manual test.

Hardware output compares stable encoded-report fingerprints before writing. If
two typed frames encode to the same controller report, DSCC suppresses the
redundant write until the keepalive interval. This keeps current haptics intact
while reducing unnecessary USB/Bluetooth output traffic.

## Web UI

The UI is Svelte 5 + Vite, not SvelteKit.

| Path | Purpose |
| --- | --- |
| `web/src/main.ts` | Mounts the app. |
| `web/src/App.svelte` | App shell, hash routing, snapshot lifecycle, and shared state. |
| `web/src/lib/api.ts` | API calls, DTO normalization, WebSocket setup, fallback polling. |
| `web/src/lib/types.ts` | UI-side DTOs and shared types. |
| `web/src/app/` | Navigation, runtime, selection, profile-draft, haptics-state, polling, update-state, toast, onboarding, partial-error, and support-bundle helpers. |
| `web/src/lib/features/haptics/HapticsView.svelte` | Adaptive triggers and haptics view. |
| `web/src/lib/features/buttonMapping` | Steam Input mirror view and p95-tested helpers. |
| `web/src/components/ControllerCard.svelte` | Games page controller panel. |
| `web/src/lib/features/games/AddGameDialog.svelte` | Steam and local-app registration. |
| `web/src/lib/mock` | Dev-only mock API. Production builds ignore mock toggles. |

Primary routes:

- `#/games`
- `#/controllers`
- `#/adaptive-triggers-haptics`
- `#/button-mapping`

## Game And Adapter Modules

- Game modules identify a supported game.
- Adapter modules read telemetry and publish normalized signals.
- Game detection uses `moduleId` for the game and `adapterId` for the telemetry
  adapter. Do not collapse those fields.

Current live telemetry adapters:

- `forza-data-out`
- `assetto-shared-memory`

Catalog-only adapter entries exist for future work, but metadata alone does not
start a parser or listener.

Community modules are still draft data-only manifest/profile packs. They cannot
add native parsers, process hooks, filesystem writers, or executable code.

## DualSense Edge Onboard Slots

Edge onboard profile support is typed and guarded:

- USB or Bluetooth Edge controllers can read onboard slot state when the host
  exposes HID feature-report access.
- `Fn + Circle`, `Fn + Cross`, and `Fn + Square` are editable.
- `Fn + Triangle` remains the default/read-only slot.
- Enabled hardware output can write supported static profile data over guarded
  USB or Bluetooth HID feature reports after acknowledgement and readback.
- Unavailable hardware paths stage changes locally.
- Live telemetry effects are not stored onboard.

## Safety Rules

- Default API binding is loopback.
- LAN API exposure requires user opt-in in app settings.
- Direct `dscc-agent` non-loopback binding requires `DSCC_ENABLE_LAN_API=1`.
- Forza non-loopback UDP binding requires `DSCC_ENABLE_LAN_FORZA=1`.
- Mutating HTTP requests and WebSocket upgrades must keep same-origin checks.
- Do not add raw HID-byte API routes.
- Hardware output must flow through typed frame/profile paths.
- Steam Input writes stay under guarded `controller_*.vdf` paths with backups.
- Forza glyph writes stay under trusted game roots with backups.
- Logs and API output must not expose raw HID paths, serials, Bluetooth
  addresses, Steam account paths, or raw report bytes.
