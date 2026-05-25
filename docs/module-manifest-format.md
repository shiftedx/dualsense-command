# DSCC Module Manifest Format

Community modules start as data-only manifest packs. This file is the draft public contract for those packs; a module installer/loader is not implemented yet. Until that exists, contributors should validate profile packs by importing the included profiles through the existing profile import flow.

This is separate from the current Add Game UI, which only stores local Steam game entries for profile auto-load and marks them as `custom / no telemetry adapter`.

DSCC uses two built-in module concepts:

- Adapter modules are protocol/runtime integrations. Today `forza-data-out` is the live UDP parser and `assetto-shared-memory` is the live Windows shared-memory reader; other catalog entries are metadata until their Rust runtimes exist.
- Game modules are one supported game each. A game module can depend on an adapter module instead of owning a parser itself.

Forza Horizon 5, Forza Horizon 6, and Forza Motorsport are separate built-in game modules that share `forza-data-out` when their public telemetry protocol is compatible. In the current app, FH5 and FH6 are Steam catalog entries; Forza Motorsport is process-detection metadata until correct catalog metadata is provenance-backed.

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

`signals` documents normalized DSCC signals consumed by the profiles; it must not declare new packet fields, parser layouts, or runtime behavior.

`games[].processNames` are declarative hints for review/future matching only. A manifest does not install a process scanner, hook a process, or enable telemetry parsing.

`trusted` is intentionally not a manifest field. DSCC derives trust from built-in status, future signing/review state, or install source.

Current API mapping:

- `ModuleSummary.kind` is `adapter` for protocol/runtime modules and `game` for per-game modules.
- `ModuleSummary.source` is currently `built_in` for adapter summaries and `built_in_game` for built-in game summaries; future community labels are derived by DSCC, not authored by the manifest.
- `ModuleSummary.profileTemplates` is currently a list of template display names, not embedded profile template objects.
- `GameDetectionResponse.moduleId` is the detected game module id.
- `GameDetectionResponse.adapterId` is the telemetry adapter id used by that game.
- Profile files referenced by `profileTemplates[].profile` use the `ExportedProfile` contract with `schema: "dev.dscc.profile.v1"`.

## Rules

- Community packs are metadata and profiles only.
- No executable code in first-wave community imports.
- Community modules cannot provide native telemetry parsers, process scanners, filesystem writers, or runtime hooks yet.
- Every bundled asset must include license metadata.
- Manifest data, profile defaults, process names, app ids, and assets must come from public/approved sources or original experiments recorded in `PROVENANCE.md`.
- Do not copy or derive constants, schemas, packet layouts, tuning defaults, comments, or structure from AGPL/incompatible implementations.
- Do not bundle Sony, game, or third-party assets unless redistribution rights and source/license metadata are recorded.
- Native packet parsers remain built-in adapter modules until a separate trust/sandbox/signing model exists.
- Game profile packs must target explicit game ids. Do not collapse distinct supported games into one community module just because they share an adapter.
- When the installer exists, bad manifests must fail validation with actionable errors and must not be partially installed.

## Contribution Paths

- Use a data-only profile pack when the game can use an existing built-in adapter and the contribution is metadata, profiles, labels, or licensed assets.
- Propose a built-in Rust adapter when a game needs new packet parsing, shared-memory access, filesystem access, process logic, or runtime behavior.
- Propose a built-in Rust game module when the game needs built-in detection logic, bundled presets, glyph helpers, or adapter binding before the community loader exists.

For a concrete built-in reference, see `docs/game-module-contribution-guide.md`. Assetto Corsa Rally demonstrates the current end-to-end path: a game module, a trusted shared-memory adapter, a built-in profile, process detection, detection-only lightbar output, normalized telemetry signals, and tests.

## Native Built-In Modules

Built-in adapter modules can contain Rust parser code and richer platform support. Built-in game modules currently bind one game id to detection hints, profiles, UI metadata, optional glyph helpers, and a single adapter dependency via `adapter_id`. Both expose manifest-like summaries through `/api/modules`; future installed modules should use the same shape, with source/trust labels derived by DSCC rather than authored by the manifest.
