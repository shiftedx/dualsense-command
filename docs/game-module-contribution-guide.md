# Game Module Contribution Guide

DSCC has two module layers:

- **Game modules** identify one supported game: names, process hints, Steam ids, install directories, default profile, profile templates, and detection lightbar color.
- **Adapter modules** read telemetry: UDP, shared memory, SDK, or another trusted built-in runtime, then publish normalized DSCC signals such as `input.brake`, `vehicle.rpm_ratio`, and `wheel.slip.max`.

Community modules are data-only until DSCC has a parser sandbox/signing model. If a game needs packet parsing, shared-memory reads, filesystem access, or runtime hooks, contribute it as a built-in Rust adapter plus a built-in Rust game module.

## Choose The Contribution Path

1. **Profile pack only**
   Use this when the game already works through an existing adapter and the PR only adds metadata, profiles, labels, or licensed assets.

2. **Built-in game module**
   Use this when DSCC should detect the game, show it as supported, assign a default profile, or light the controller when the process is detected.

3. **Built-in adapter module**
   Use this when DSCC must parse telemetry, read shared memory, bind UDP, or use a game SDK. Native parser code must stay in the Rust workspace.

## Built-In Game Checklist

1. Record public sources in `PROVENANCE.md` before using process names, app ids, packet layouts, shared-memory page names, or telemetry fields.
2. Add adapter metadata to `crates/dscc-adapters/src/lib.rs` if the game needs a new telemetry source.
3. Add game metadata to `crates/dscc-agent/src/game_modules.rs`.
4. Add a built-in profile or map the game to an existing profile in `crates/dscc-agent/src/lib.rs`.
5. Normalize telemetry into existing signal names whenever possible.
6. Add tests for process detection, module metadata, adapter status, profile resolution, waiting-for-telemetry behavior, and parser normalization.
7. Keep hardware output gated: detection may write a lightbar-only frame, but trigger tension and rumble require fresh telemetry.
8. Run Rust and web validation before opening the PR.

## Assetto Corsa Rally Example

Assetto Corsa Rally is the reference first-party shared-memory module:

- Game module id: `assetto-corsa-rally`
- Steam app id: `3917090`
- Process hint: `acr.exe`
- Adapter id: `assetto-shared-memory`
- Default profile id: `assetto-corsa-rally`
- Telemetry transport: read-only Windows shared memory
- Detection behavior: red lightbar as soon as the supported process is detected; triggers remain neutral until fresh physics telemetry is live

The adapter reads the public Assetto shared-memory physics prefix and publishes normalized racing signals, so the existing haptic engine can reuse brake, throttle, RPM, slip, shift, and surface cues.

## Required Validation

Use the local GNU toolchain on Windows:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace --target x86_64-pc-windows-gnu
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
```

For UI or mapping changes, also run:

```powershell
npm.cmd --prefix web run test:button-map
```
