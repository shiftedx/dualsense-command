# DSCC Module Manifest Format

Community modules start as data-only manifest packs. This file is the draft public contract for those packs; a module installer/loader is not implemented yet. Until that exists, contributors should validate profile packs by importing the included profiles through the existing profile import flow.

DSCC uses two built-in module concepts:

- Adapter modules are protocol/runtime adapters, such as a UDP telemetry parser or shared-memory reader.
- Game modules are one supported game each. A game module can depend on an adapter module instead of owning a parser itself.

Forza Horizon 5, Forza Horizon 6, and Forza Motorsport should be separate game modules. They may share the built-in `forza-data-out` adapter when their public telemetry protocol is compatible.

## File

Preferred filename:

```text
dscc-module.json
```

TOML can be added later if profile packs settle on TOML.

## Required Fields

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

`profileTemplates[].profile` points to an exported DSCC profile file using the existing `dev.dscc.profile.v1` profile import/export shape. Embedded profile configs can be added later, but file references keep first-wave packs simple and reviewable.

`games[].processNames` are declarative detection hints only. They are not process scanners or runtime hooks; DSCC decides whether and how to use them.

`trusted` is intentionally not a manifest field. DSCC derives trust from built-in status, future signing/review state, or install source.

Current API mapping:

- `ModuleSummary.kind` is `adapter` for protocol/runtime modules and `game` for per-game modules.
- `GameDetectionResponse.moduleId` is the detected game module id.
- `GameDetectionResponse.adapterId` is the telemetry adapter id used by that game.
- Profile files referenced by `profileTemplates[].profile` use the `ExportedProfile` contract with `schema: "dev.dscc.profile.v1"`.

## Rules

- Community packs are metadata and profiles only.
- No executable code in first-alpha community imports.
- Community modules cannot provide native telemetry parsers, process scanners, filesystem writers, or runtime hooks yet.
- Every bundled asset must include license metadata.
- Native packet parsers remain built-in adapter modules until a separate trust/sandbox/signing model exists.
- Game profile packs must target explicit game ids. Do not collapse distinct supported games into one community module just because they share an adapter.
- When the installer exists, bad manifests must fail validation with actionable errors and must not be partially installed.

## Contribution Paths

- Use a data-only profile pack when the game can use an existing built-in adapter and the contribution is metadata, profiles, labels, or licensed assets.
- Propose a built-in Rust adapter when a game needs new packet parsing, shared-memory access, filesystem access, process logic, or runtime behavior.
- Propose a built-in Rust game module when the game needs first-party detection logic, bundled presets, glyph helpers, or adapter binding before the community loader exists.

For a concrete built-in reference, see `docs/game-module-contribution-guide.md`. Assetto Corsa Rally demonstrates the current end-to-end path: a game module, a trusted shared-memory adapter, a built-in profile, process detection, detection-only lightbar output, normalized telemetry signals, and tests.

## Native Built-In Modules

Built-in adapter modules can contain Rust parser code and richer platform support. Built-in game modules bind one game id to detection hints, profiles, UI metadata, optional glyph helpers, and one or more adapter dependencies. Both expose manifest-like summaries through `/api/modules`; future installed modules should use the same shape, with source/trust labels derived by DSCC rather than authored by the manifest.
