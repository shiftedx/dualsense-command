# Contributing

Start with `git status --short --ignored`; preserve unrelated edits. Read
[AGENTS](../AGENTS.md) for safety/timing contracts and [Architecture](architecture.md#change-entry-points)
for source owners. Search with `rg` / `rg --files`.

Before HID, telemetry, Steam Input, broker or asset changes, read the
[provenance policy](provenance-policy.md) and [source ledger](sources.md).
Record public references or sanitized experiments; keep private notes, captures,
builds, installers and assistant state out of commits.

## Setup and local modes

Install Rust and Node.js 24. Windows uses the GNU Rust toolchain plus native
C compiler/linker; Linux needs `libudev-dev` for `hidapi`. In PowerShell use
`npm.cmd` to avoid `npm.ps1` execution-policy blocks. Root scripts select GNU
Rust automatically; direct commands use `cargo +stable-x86_64-pc-windows-gnu`.

**Real output defaults on. Set the write-disable flag before diagnostics or
smoke tests, in every terminal that launches the agent:**

```powershell
$env:DSCC_DISABLE_HARDWARE_OUTPUT='1'
npm.cmd --prefix web ci
npm.cmd run dev
```

Alternative write-disable flag: `DSCC_ENABLE_HARDWARE_OUTPUT=0`.

| Mode | Command |
| --- | --- |
| Agent + web | `npm.cmd run dev` |
| UI-only fixtures | `npm.cmd --prefix web run dev:mock` |
| Separate agent terminal | `cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- serve --addr 127.0.0.1:43473` |
| Separate web terminal | `npm.cmd --prefix web run dev` |

Production builds ignore mock toggles and exclude fixtures. For browser checks,
install Chromium: `npm.cmd --prefix web exec playwright install chromium`.

## Boundary checklist

| Change | Preserve |
| --- | --- |
| UI | Svelte 5 event attributes, existing Lucide icons, lifecycle teardown, dense accessible screens; filesystem scans off render paths. |
| Profiles | Global controller-only tuning; game telemetry controls require Game Profile selection. Controller aliases are display names. |
| API / persistence | Typed validation before side effects, same-origin guards, locked snapshot capture and ordered atomic persistence. |
| Output | Typed frames/profiles → output manager → device encoding/clamping → guarded HID transport. Time-limited tests; stale game telemetry neutralizes triggers/rumble. |
| Edge | Protected default slot, confirmation, acknowledgement/readback, local staging when writes are unavailable. |
| Steam Input | Canonical roots; `controller_*.vdf` only, reject `controller_base*.vdf`; 256KB limit, dry-run and backups. Match group, source, mode, input and activator. |
| Modules | Separate `moduleId` / `adapterId`; FH5/FH6/Motorsport remain distinct. Community packs are data-only. |

Reuse existing adapters for profile packs. New parsers, shared-memory readers or
runtime behavior belong in built-in Rust adapters: [contribution guide](game-module-contribution-guide.md),
[PR template](game-module-template.md). Keep docs factual and active; use tables
for comparisons and diagrams for flows.

## Validation

Run the matching suite while editing, then the full suite before a PR.
For docs-only changes, inspect the diff, verify commands and run `check:docs`.

| Suite | Command | Coverage |
| --- | --- | --- |
| Full | `npm.cmd run check` | Docs, release, web and Rust gates. |
| Docs | `npm.cmd run check:docs` | Repository-local links; rejects ignored targets. |
| Release | `npm.cmd run check:release` | Metadata tests, versions, assets and dependency notices. |
| Rust | `npm.cmd run check:rust` | Format, all-feature workspace tests, Clippy. |
| Web | `npm.cmd run check:web` | Types, source audit, mapping/DTO/haptics, build, size, visual smoke, curve drag. |
| Performance | `npm.cmd run check:perf` | Rust perf guards and button-map p95 budget. |

| Changed area | Focused check / evidence |
| --- | --- |
| Steam Input / mapping | `npm.cmd --prefix web run test:button-map` |
| Snapshot DTOs | `npm.cmd --prefix web run test:snapshot-map` |
| Haptics math | `npm.cmd --prefix web run test:haptics-graph` |
| Telemetry / detection | Rust: malformed packets, stale output, profile resolution. |
| Persistence / filesystem | Rust: isolated temp paths, ordering, replacement, path guards. |
| HID / Edge / Bridge | Rust + sanitized physical evidence; mocks do not establish hardware support. |
| API / diagnostics / packaging / provider copy | `npm.cmd --prefix web run test:source-audit` |
| UI / layout | Inspect the affected screen with writes disabled; `npm.cmd --prefix web run test:visual-smoke`. |
| Curves | `npm.cmd --prefix web run test:curve-drag`: 240 moves, frame p95 ≤50ms, mutations/move ≤32. |

Record actual commands/results and unavailable checks. Release acceptance is
tracked in [Production readiness](production-readiness-plan.md).
