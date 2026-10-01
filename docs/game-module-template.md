# Game Module PR Template

Use for supported-game/profile-pack PRs. Find owners in the
[contributor map](game-module-contribution-guide.md#contributor-map); split
parser/runtime work from tuning when practical.

## Scope and Detection

- Game / `moduleId` / display name:
- Store ids / process names / install hints:
- `adapterId` / telemetry source / runtime owner:
- Default profile / profile ids:
- Detection lightbar:
- Changed files and responsibilities:
- Platforms tested:

Detection stays process/catalog based; no hooks, private APIs or protected-game
workarounds. Keep game and adapter identities separate.

## Clean-Room Notes

- Public source or sanitized original experiment for each id, process, field,
  shared-memory name, protocol value, tuning default and asset:
- Source-ledger entry / asset redistribution permission:

Apply [Provenance Policy](provenance-policy.md). Exclude incompatible
implementation details and raw captures, paths, serials, Bluetooth addresses or
account identifiers.

## Telemetry and Defaults

- Source: UDP / shared memory / SDK / none
- Existing normalized signals used / new signals justified:
- Freshness cutoff (preserve 2s) / missing-stale neutralization:
- Trigger / body haptics / lightbar defaults:
- Stick/deadzone / button-paddle assumptions:

Use conservative defaults. Detection alone may set the lightbar; game triggers
and rumble require fresh telemetry.

## Verification

| Changed behavior | Command / observed result / gap |
| --- | --- |
| Catalog and detection/profile resolution | |
| Adapter status; missing/stale telemetry | |
| Short/malformed parser input | |
| Private-data redaction | |
| Changed UI / visual smoke | |
| Physical controller/game run, if applicable | |

Use [Contributing](contributing.md#validation) for commands and output suppression.
Mocks do not prove physical support.

## User-Facing Notes

- Setup / game telemetry setting:
- Known limitations / remaining checks:
- Hardware and transport tested:
- Screenshots or visual checks:
