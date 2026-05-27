# Game Module PR Template

Use this template for a supported-game or profile-pack PR. Keep the PR small.
Split parser/runtime work from tuning-profile work when possible.

## Scope

- Game:
- Store IDs:
- Process names:
- Telemetry source:
- Profile IDs:
- Platforms tested:

## Files To Touch

Pick the smallest path that fits the work.

| Goal | Files |
| --- | --- |
| Add a game that reuses existing telemetry | `crates/dscc-agent/src/game_modules.rs`, `crates/dscc-agent/src/profiles.rs`, route/profile tests |
| Add local-app-only profile support | `crates/dscc-agent/src/game_detection/local_apps.rs`, profile tests |
| Add Steam discovery or art matching | `crates/dscc-agent/src/game_detection/steam.rs`, Steam/game detection tests |
| Add a UDP parser | `crates/dscc-adapters/src/lib.rs`, parser fixtures, provenance notes |
| Add shared-memory telemetry | A focused runtime module, platform-gated tests, provenance notes |
| Add haptic defaults only | `crates/dscc-agent/src/profiles.rs`, `crates/dscc-agent/src/effects/`, effect tests |

## Clean-Room Notes

- List every public source used for app IDs, process names, packet fields,
  shared-memory names, or protocol constants.
- State whether each value came from public docs, public user settings,
  original packet captures, or local hardware testing.
- Do not copy implementation details from incompatible projects.
- Do not include raw HID paths, serials, Bluetooth addresses, Steam account
  paths, or raw packet captures.

## Detection

- Module id:
- Display name:
- Adapter id:
- Default profile id:
- Process names:
- Steam app IDs:
- Install-folder hints:
- Detection-only lightbar color:

Detection must stay process-name and catalog based. Do not add hooks, private
APIs, or protected-game workarounds.

## Telemetry

- Source type: UDP / shared memory / SDK / none
- Runtime module:
- Freshness cutoff:
- Stale neutralization behavior:
- Signals produced:

Normalize to existing DSCC signals first. Add a new signal only when an
existing one cannot describe the data.

## Profile Defaults

- Trigger behavior:
- Body haptics:
- Lightbar:
- Stick/deadzone defaults:
- Button or paddle assumptions:

Defaults should be conservative. A user should be able to install the update,
open the game, and get useful feedback without aggressive effects.

## Tests

Required checks:

- Game module appears in the catalog.
- Detection resolves the expected profile.
- Missing telemetry stays safe.
- Stale telemetry neutralizes output.
- Parser rejects short or malformed packets.
- API responses redact private paths.
- UI still passes visual smoke if the PR changes game presentation.

Run:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace --all-features
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run check
```

## User-Facing Notes

- Setup steps:
- Known limitations:
- Hardware used:
- Telemetry source setting the user must enable:
- Screenshots or visual checks:

