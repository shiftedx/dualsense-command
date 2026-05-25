# Game Module Contribution Guide

DSCC has two module layers:

- **Game modules** identify one supported game: names, process hints, Steam ids, install directories, default profile, profile templates, and detection lightbar color.
- **Adapter modules** describe telemetry integrations. Some are live runtimes; today the parser-backed/runtime-backed adapters are `forza-data-out` and `assetto-shared-memory`. Other first-wave catalog entries are setup-ready metadata until a Rust runtime/parser is added.

Community modules are data-only until DSCC has a parser sandbox/signing model. If a game needs packet parsing, shared-memory reads, filesystem access, or runtime hooks, contribute it as a built-in Rust adapter plus a built-in Rust game module.

The current Add Game UI creates local `custom-*` Steam entries with profile auto-load only. Those entries are not community module manifests and do not add telemetry adapters.

## Choose The Contribution Path

1. **Profile pack only**
   Use this when the game already works through an existing adapter and the PR only adds metadata, profiles, labels, or licensed assets.

2. **Built-in game module**
   Use this when DSCC should detect the game, show it as supported, assign a default profile, or light the controller when the process is detected.

3. **Built-in adapter module**
   Use this when DSCC must parse telemetry, read shared memory, bind UDP, or use a game SDK. Native parser code must stay in the Rust workspace.

## Built-In Game Checklist

1. Record public sources in `PROVENANCE.md` before using process names, app ids, packet layouts, shared-memory page names, or telemetry fields.
2. If the game needs a new telemetry source, add adapter metadata to `crates/dscc-adapters/src/lib.rs` and add the live runtime registration too: `built_in_udp_adapters()` for UDP parsers, or an agent-side/shared-memory loop like the Assetto reader. Catalog metadata alone does not parse packets or start a listener.
3. Add game metadata to `crates/dscc-agent/src/game_modules.rs`.
4. Set `steam_catalog: true` only when the Steam app id and install directory are provenance-backed and should appear in the Games view. Process-detection-only modules can remain out of the Steam catalog.
5. Add a built-in profile or map the game to an existing profile in `crates/dscc-agent/src/lib.rs`.
6. Normalize telemetry into existing signal names whenever possible.
7. Add tests for process detection, module metadata, adapter status, profile resolution, waiting-for-telemetry behavior, and parser normalization.
8. Keep hardware output gated: detection may write a lightbar-only frame, but trigger tension and rumble require fresh telemetry.
9. Run Rust and web validation before opening the PR.

Do not inspect or derive packet layouts, profile defaults, schemas, tuning values, comments, or structure from incompatible implementations such as `Forza-Horizon-DualSense-Python`. Record public sources or original experiments in `PROVENANCE.md` before depending on them.

## Assetto Corsa Rally Example

Assetto Corsa Rally is the built-in shared-memory reference module:

- Game module id: `assetto-corsa-rally`
- Steam app id: `3917090`
- Process hint: `acr.exe`
- Adapter id: `assetto-shared-memory`
- Default profile id: `assetto-corsa-rally`
- Telemetry transport: read-only Windows shared memory
- Detection behavior: red lightbar as soon as the supported process is detected; triggers remain neutral until fresh physics telemetry is live

The adapter reads public Assetto-compatible shared-memory pages (`acpmf_*`, with `acevo_pmf_*` compatibility), uses the physics prefix plus optional graphics/static data, and publishes normalized racing signals so the existing haptic engine can reuse brake, throttle, RPM, slip, shift, and surface cues.

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
