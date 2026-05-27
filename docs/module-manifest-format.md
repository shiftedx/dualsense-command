# Module Manifest Format

Community modules are planned as data-only profile packs. The installer/loader
is not implemented yet, so contributors should test profile packs by importing
the included profiles through DSCC.

This format is separate from the current **Add Game** UI. Add Game only creates
local custom Steam entries for profile auto-load.

## What A Manifest Can Do

Allowed:

- Describe supported games.
- Reference exported `dev.dscc.profile.v1` profile files.
- Include metadata and licensed assets.
- Document which built-in adapter a profile expects.

Not allowed yet:

- Native code.
- Packet parsers.
- Process hooks.
- Filesystem writers.
- Runtime telemetry logic.

## Filename

```text
dscc-module.json
```

## Example

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

- `profileTemplates[].profile` points to an exported DSCC profile file.
- `signals` documents existing DSCC signals used by the profile. It must not
  define new packet fields or parser behavior.
- `games[].processNames` are declarative hints only. A manifest does not install
  a process scanner.
- `trusted` is not a manifest field. DSCC will decide trust from install source,
  signing, or review status.

Current API mapping:

- `ModuleSummary.kind`: `adapter` or `game`.
- `ModuleSummary.source`: currently `built_in` or `built_in_game`.
- `GameDetectionResponse.moduleId`: detected game module id.
- `GameDetectionResponse.adapterId`: telemetry adapter id.

## Review Rules

- Use public/approved sources or original experiments for app ids, process
  names, defaults, and assets.
- Do not copy constants, schemas, packet layouts, tuning defaults, comments, or
  structure from incompatible implementations.
- Do not bundle Sony, game, or third-party assets unless redistribution rights
  are documented.
- Target explicit game ids. Do not merge separate games into one module only
  because they share an adapter.

For a complete built-in example, see [Game Module Guide](game-module-contribution-guide.md).
