# Module Manifest Format

**Draft, not loadable.** Community modules are planned data-only profile packs;
test profiles through DSCC's profile import. **Add Game** registers custom
Steam/local-app profile auto-load, not manifests or telemetry.

## What A Manifest Can Do

| Allowed | Not allowed yet |
| --- | --- |
| Game descriptions, metadata, licensed assets | Native code, process hooks, filesystem writers |
| Exported `dev.dscc.profile.v1` files and expected built-in adapters | Packet parsers, runtime telemetry logic |

## Example

Proposed filename: `dscc-module.json`. This example describes the draft pack
format, not a supported API or runtime loader.

```json
{
  "schema": "dev.dscc.module.v1",
  "id": "forza-horizon-track-pack",
  "name": "Forza Horizon Track Pack",
  "version": "1.0.0",
  "author": "Example Author",
  "license": "CC0-1.0",
  "homepage": "https://example.invalid",
  "kind": "profile_pack",
  "platforms": ["windows"],
  "games": [
    {
      "id": "forza-horizon-5",
      "names": ["Forza Horizon 5"],
      "adapterId": "forza-data-out",
      "processNames": ["ForzaHorizon5.exe"]
    }
  ],
  "capabilities": {
    "profileTemplates": true,
    "telemetryParser": false,
    "nativeCode": false
  },
  "signals": [],
  "profileTemplates": [
    {
      "id": "fh5-balanced",
      "name": "FH5 Balanced",
      "gameId": "forza-horizon-5",
      "profile": "profiles/fh5-balanced.dscc-profile.json"
    }
  ],
  "assets": [
    {
      "path": "assets/fh5-banner.webp",
      "kind": "banner",
      "license": "CC-BY-4.0",
      "source": "https://example.invalid/fh5-banner"
    }
  ]
}
```

## Field Notes

- `profileTemplates[].profile`: exported DSCC profile file.
- `signals`: existing normalized signals; no packet definitions or parsers.
- `games[].processNames`: declarative hints; no installed scanner.
- `trusted`: absent; trust/signing/review policy remains future work.

Current API uses `ModuleSummary.kind` = `adapter` / `game` and `source` =
`built_in` / `built_in_game`. Detection distinguishes `moduleId` (game) from
`adapterId` (telemetry). Draft manifest ids do not replace those API fields.

## Review Rules

Follow [Provenance Policy](provenance-policy.md): approved sources/original
experiments, no incompatible implementation details and documented asset
redistribution rights. Target distinct game ids even when sharing an adapter.
Use the [Game Module Guide](game-module-contribution-guide.md) for built-in work.
