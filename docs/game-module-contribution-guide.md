# Game Module Guide

Start with the [PR template](game-module-template.md) and
[provenance policy](provenance-policy.md). A **Game Module** identifies/detects a
game (`moduleId`); a **Telemetry Adapter** reads a source and publishes normalized
signals (`adapterId`). Sharing an adapter does not merge game identity.

## Pick The Right Path

| Need | Path |
| --- | --- |
| Tuning/metadata/licensed assets using an existing adapter | Data-only profile pack; [draft manifest](module-manifest-format.md). |
| Built-in detection, default profile or detection lightbar | Declarative Game Module. |
| New parser, shared memory, SDK or telemetry runtime | Built-in Rust Telemetry Adapter. |
| Custom Steam/local-app profile auto-load | **Add Game** UI; adds no telemetry or manifest loader. |

Live adapters are `forza-data-out` and `assetto-shared-memory`. Catalog metadata
alone starts no parser/listener. Community modules remain data-only under
[ADR 0006](adr/0006-keep-community-modules-data-only.md).

## Contributor Map

| Task | Owner |
| --- | --- |
| Game metadata | `crates/dscc-agent/src/game_modules.rs` (`GameModule` fields/types). |
| Profile defaults/effects | `crates/dscc-agent/src/built_in_presets.rs`, `profiles.rs`, `effects/`. |
| Local-app / Steam discovery | `crates/dscc-agent/src/game_detection/local_apps.rs` / `steam.rs`. |
| UDP parsing | `crates/dscc-adapters/src/lib.rs`. |
| Windows shared memory | `crates/dscc-agent/src/assetto_shared_memory.rs`; platform-gated tests. |

Keep catalog entries declarative: ids, names, processes, store ids, adapter,
profile and presentation. Parsing, filesystem access and effect state belong in
adapter/runtime owners. Detection uses process/catalog metadata, never injection,
hooks or memory scanning.

## Checklist

1. Record approved public sources/original experiments for ids, processes,
   telemetry fields and assets in the [source ledger](sources.md) and PR.
2. Reuse normalized signals/adapters where possible; register runtime and metadata
   together when adding a source.
3. Add distinct game metadata and conservative profile defaults.
4. Test detection/profile resolution, metadata/status and malformed/short data.
5. Verify missing/stale telemetry keeps triggers/rumble neutral; detection may
   set only the lightbar. Preserve the 2s stale cutoff.
6. Run [change-specific validation](contributing.md#validation); record actual
   commands/results and separate physical evidence from mocks.

Do not derive code, schemas, layouts, defaults, comments or structure from
incompatible implementations. Exclude raw captures/private identifiers; document
asset redistribution rights before bundling.

## Assetto Corsa Rally Example

| Field | Value |
| --- | --- |
| Game / default profile | `assetto-corsa-rally` |
| Steam app / process hint | `3917090` / `acr.exe` |
| Adapter / source | `assetto-shared-memory` / read-only Windows shared memory |
| Signals | Brake, throttle, RPM, slip, shift and surface cues. |

This illustrates built-in ownership; catalog presence alone does not establish
current physical/game validation.
