# DSCC Architecture

This map reflects the backend and frontend structure after the contributor-ease cleanup waves. It is meant to help contributors find the extension point before opening a broad refactor.

## Backend Crates

- `crates/dscc-core`: Domain model for controllers, profiles, effect rules, value sources, and validated `ControllerOutputFrame` output. New output concepts should start here only when they are part of DSCC's public internal model.
- `crates/dscc-telemetry`: Shared signal names, signal values, snapshots, adapter status, and adapter traits. Telemetry signals are lowercase dotted names such as `input.brake` or `vehicle.rpm_ratio`.
- `crates/dscc-adapters`: Built-in adapter catalog plus clean-room telemetry parsers. Adapter modules own protocol/runtime plumbing, such as Forza Data Out, and are not game identity containers.
- `crates/dscc-device`: Hardware boundary. It owns sanitized HID enumeration, device registry reconciliation, diagnostics, mock transport, `hidapi` transport, output report encoding, input report reads, and guarded writes through `ControllerOutputManager`.
- `crates/dscc-agent`: Runtime and local API. Address policy, output env policy, HTTP mutation security, Forza glyph filesystem operations, and the built-in game module registry are now split into small modules, but `src/lib.rs` still owns most state, routes, persistence, process scan orchestration, Steam Input, the generic UDP adapter runtime, snapshots, WebSocket invalidation, and hardware output loops.
- `crates/dscc-cli`: Diagnostics and local helper commands, including agent status, app paths, mock devices, sanitized HID listing, and device diagnosis.
- `crates/dscc-tray`: Windows tray launcher that starts the local agent hidden and opens the bundled UI.

## Agent Module Map

- `crates/dscc-agent/src/main.rs`: Binary entry point.
- `crates/dscc-agent/src/bind_addr.rs`: Default bind addresses, `DSCC_AGENT_ADDR`, the current Forza adapter bind env (`DSCC_FORZA_BIND_ADDR`), and explicit LAN opt-in gates.
- `crates/dscc-agent/src/env_policy.rs`: Hardware output mode from `DSCC_DISABLE_HARDWARE_OUTPUT` and `DSCC_ENABLE_HARDWARE_OUTPUT`.
- `crates/dscc-agent/src/forza_glyphs.rs`: Trusted Forza Horizon 6 install path resolution plus PlayStation glyph install/restore with canonical root checks, backups, and refusal rules.
- `crates/dscc-agent/src/game_modules.rs`: Built-in game module registry. Each supported game owns one game id, display name, process detection hints, Steam metadata, adapter binding, default profile id, profile template labels, aliases, and supported-game summary helpers. External community module loading is not implemented yet.
- `crates/dscc-agent/src/http_security.rs`: Same-origin `Origin`/`Host` guard for non-GET API requests and WebSocket upgrades.
- `crates/dscc-agent/src/lib.rs`: Application state, API DTOs, route handlers, persistence, profile resolution, process scanning, Steam Input layout discovery/writes, generic UDP adapter loop, Forza effect materialization, output watchdog, hardware output loop, static web serving, and most API tests.

The main local API router is `app(state)` in `dscc-agent/src/lib.rs`. It registers `/api/status`, app settings, controllers, profiles, adapters, Steam Input, modules, game detection, effects, profile resolution, telemetry, logs, diagnostics, and `/api/ws`, then applies `reject_cross_origin_mutations`.

## Runtime Flow

1. `dscc-cli serve` or `dscc-agent` resolves the loopback API address and starts the axum router.
2. `hid_agent_state()` opens `HidApiTransport`, chooses the output mode, creates `DeviceManager` and `ControllerOutputManager`, and starts periodic device scanning.
3. The generic UDP adapter runtime starts registered runnable UDP adapters, currently `forza-data-out`, parses packets through `dscc-adapters`, and applies normalized `SignalUpdate`s into the agent telemetry snapshot.
4. Profile resolution picks the active profile from controller/game override, game detection, game assignment, global override, or global default.
5. `EffectEngine` evaluates a runtime `Profile` into a `ControllerOutputFrame`.
6. Forza-specific enhancements add telemetry rumble, lightbar, and player LEDs when live driving telemetry is present.
7. `ControllerOutputManager` encodes USB or Bluetooth reports and writes only when hardware output mode allows it.
8. `/api/snapshot` and `/api/ws` carry normalized runtime state to the Svelte UI.

Controller display names are persisted as aliases keyed by the stable controller id. The alias can change in the UI, but profile resolution and controller-specific overrides must continue to use the stable id.

## Frontend Structure Target

The current UI is a plain Svelte 5 + Vite app, not SvelteKit:

- `web/src/main.ts`: Mounts the app.
- `web/src/App.svelte`: App shell, hash-view routing, snapshot lifecycle, controller/game selection, cross-feature state, and still much of the haptics/profile UI.
- `web/src/lib/api.ts`: Browser API calls, DTO normalization, snapshot WebSocket handling, and fallback polling support.
- `web/src/lib/types.ts`: UI-side DTOs and domain types.
- `web/src/lib/features/buttonMapping`: First extracted feature folder. It owns the Steam Input mirror view, pure source-aware slot/binding helpers, and feature exports used by the p95 guard.
- `web/src/lib/mock`: Stable in-browser mock API and fixture data for contributor UI work without a running agent, controller, Steam, or Forza.
- `web/src/styles/app.css`: Global styling for the dense operational UI.

For new frontend work, prefer this target shape:

- Keep `App.svelte` as the app shell and state coordinator.
- Keep the `#/games` view as the low-complexity entry point. It selects the target controller plus either Global Profile or a supported Steam game before revealing tuning surfaces; Global Profile is the default scope.
- Preserve profile scope explicitly: `gameId: null`/missing is global, and a concrete `gameId` is a per-game profile.
- Treat global scope as controller-only tuning. Do not show telemetry streams, RPM controls, adapter packet status, or game-signal routing unless a game profile is selected.
- Keep controller display names as editable labels only. Stable controller ids remain the identity used for profile loading, overrides, and persistence.
- Put feature views and feature-local helpers in `web/src/lib/features/<featureName>/`.
- Export feature entry points from `web/src/lib/features/<featureName>/index.ts`.
- Put shared UI-side contracts in `web/src/lib/types.ts`.
- Put API fetch functions and DTO normalization in `web/src/lib/api.ts`.
- Keep timers, socket handles, event listeners, and polling cleanup in `stopAppRuntime` or the component teardown that created them.
- Keep expensive filesystem or Steam discovery work out of render-time and hot UI paths; consume cached API results instead.
- Keep Steam Input mapping UI source-aware. Game-specific layouts can reuse raw input ids across D-pad, trackpad, trigger, and joystick groups, so matching and writes must preserve `groupId`, source, source mode, and activator.

## Built-In Modules

`/api/modules` should present adapter modules and game modules as first-class contribution units:

- Adapter modules own protocol/runtime plumbing. They parse telemetry, expose adapter status, and publish shared signals.
- Game modules own one supported game each. They bind game detection, profiles, UI labels, optional glyph helpers, and adapter dependencies.
- Game detection keeps game module identity and adapter identity separate: `moduleId` is the game module id, while `adapterId` is the protocol adapter id.
- `/api/modules` exposes both through one summary shape with `kind: "adapter"` or `kind: "game"`. Adapter summaries do not own game profile templates.

Forza Horizon 5, Forza Horizon 6, and Forza Motorsport should be separate game modules. They may share the `forza-data-out` adapter when their public Data Out / UDP Race Telemetry protocol is compatible.

`dscc-adapters` currently exposes these built-in adapter summaries:

- `forza-data-out`: UDP, default port `5300`, active parser hosted by the generic UDP adapter runtime and shared by supported Forza game modules.
- `ea-f1-udp`: UDP catalog entry for F1 telemetry.
- `ea-wrc-udp`: UDP catalog entry for EA SPORTS WRC.
- `beamng`: UDP catalog entry for BeamNG.drive protocols.
- `live-for-speed`: UDP catalog entry for LFS InSim/OutGauge.
- `raceroom`: Shared-memory catalog entry.

Community modules remain draft data-only manifest packs; the installer/loader is not implemented yet. Native parser code stays in built-in adapter modules until a native parser sandbox/signing model exists.

## Security And Safety Boundaries

- The API and UDP adapter listeners are loopback-first. LAN binding requires explicit opt-in; the current Forza adapter uses `DSCC_ENABLE_LAN_FORZA=1`.
- Non-GET mutations must keep same-origin `Origin`/`Host` rejection.
- Raw HID-byte write APIs are not allowed. All hardware output must flow through validated `ControllerOutputFrame` paths.
- Steam Input writes must stay under canonical Steam roots, target guarded `controller_*.vdf` files, respect the file-size limit, create backups before real writes, and support dry-run validation.
- Forza glyph writes must stay under a trusted Forza install root, keep backups, and refuse unbacked originals.
- HID paths, Steam userdata paths, PnP values, serials, Bluetooth addresses, and raw report bytes must stay sanitized in logs, docs, tests, and API output.
