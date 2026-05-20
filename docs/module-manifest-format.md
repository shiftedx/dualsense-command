# DSCC Module Manifest Format

Community modules start as data-only packs. They should feel native in the UI, but first-alpha imports must not execute community code.

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
  "trusted": false,
  "platforms": ["windows"],
  "games": [
    {
      "id": "forza-horizon-5",
      "names": ["Forza Horizon 5"],
      "processNames": ["ForzaHorizon5.exe"]
    }
  ],
  "capabilities": {
    "profileTemplates": true,
    "telemetryParser": false,
    "nativeCode": false
  },
  "signals": [],
  "profileTemplates": []
}
```

## Rules

- Imported community packs are metadata and profiles only.
- No executable code in first-alpha community imports.
- Every bundled asset must include license metadata.
- Native packet parsers remain built-in Rust modules until a separate trust/sandbox/signing model exists.
- Bad manifests fail validation with actionable errors and are not partially installed.

## Native Built-In Modules

Built-in modules can contain Rust parser code and richer platform integration. They should still expose a manifest-like summary through `/api/modules` so users cannot tell whether a module is built in or installed except by source/trust labels.
