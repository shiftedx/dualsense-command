# Game Module Guide

Use this guide when adding support for a game or profile pack.

## The Two Pieces

- **Game module**: names the game, detects it, assigns profiles, and tells the
  UI what to show.
- **Adapter module**: reads telemetry and publishes normalized signals.

Current live adapters:

- `forza-data-out`
- `assetto-shared-memory`

Other adapter entries are catalog metadata until a Rust runtime/parser is added.

## Pick The Right Path

1. **Profile pack**
   Use this when the game already works through a built-in adapter and you only
   want to add profiles, labels, metadata, or licensed assets.

2. **Built-in game module**
   Use this when DSCC should detect the game, show it as supported, assign a
   default profile, or set the lightbar when the process is detected.

3. **Built-in adapter module**
   Use this when DSCC must parse new UDP packets, read shared memory, call an
   SDK, or run new telemetry logic.

The current **Add Game** UI only creates local custom Steam entries for profile
auto-load. It does not add telemetry support.

## Checklist

1. Record public sources or original experiments in the PR before using process
   names, app ids, packet layouts, shared-memory names, or telemetry fields.
2. Add adapter metadata and runtime registration if a new telemetry source is
   needed. Metadata alone does not parse packets.
3. Add game metadata in `crates/dscc-agent/src/game_modules.rs`.
4. Add or map a built-in profile in the agent.
5. Normalize telemetry into existing signals whenever possible.
6. Add tests for detection, metadata, adapter status, profile resolution, stale
   telemetry, and parser behavior.
7. Keep output gated: detection may set the lightbar, but triggers and rumble
   require fresh telemetry.
8. Run Rust and web validation before opening a PR.

Do not inspect or derive packet layouts, tuning values, comments, or code from
incompatible implementations.

## Assetto Corsa Rally Example

Assetto Corsa Rally is the built-in shared-memory reference module:

- Game module id: `assetto-corsa-rally`
- Steam app id: `3917090`
- Process hint: `acr.exe`
- Adapter id: `assetto-shared-memory`
- Default profile id: `assetto-corsa-rally`
- Telemetry: read-only Windows shared memory

The adapter reads public Assetto-compatible shared-memory pages and publishes
signals the existing haptic engine can use: brake, throttle, RPM, slip, shift,
and surface cues.

## Validation

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
```

For UI or button-mapping changes:

```powershell
npm.cmd --prefix web run test:button-map
```
