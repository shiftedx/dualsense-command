# Agent guide

DSCC is a local-first Rust agent and Svelte 5 UI for controller tuning, telemetry,
Steam Input, and an optional non-Steam Input Bridge. Keep changes narrow and
preserve user edits. Read this guide before running the app.

## Start here

1. Run `git status --short --ignored`; do not revert unrelated work.
2. Read [domain terms](CONTEXT.md) and the relevant [ADRs](docs/adr/).
3. Find the owner in [Architecture](docs/architecture.md); search with `rg`.
4. Use [Contributing](docs/contributing.md) for setup and change-specific checks.
5. Report changed behavior, verification, and remaining gaps. Do not claim
   hardware support from mock tests. Push or publish only when the user asks.

## Safety contracts

| Area | Preserve |
| --- | --- |
| Hardware output | Real writes default on. Set `DSCC_DISABLE_HARDWARE_OUTPUT=1` **before diagnostics or app smoke tests**. Use validated `ControllerOutputFrame` paths; no raw-byte write APIs. |
| Edge onboard profiles | Preserve protected default slot, confirmation, acknowledgement, readback, and local staging when writes are disabled. |
| Network | API and telemetry default to loopback. LAN requires explicit opt-in. Preserve same-origin mutation and WebSocket guards. |
| Steam Input | Guard canonical Steam roots and `controller_*.vdf` paths, the 256KB limit, backups, and dry-run behavior. Match slots using group, source, mode, and activator; `inputId` alone is insufficient. |
| Game glyphs | Guard trusted install roots, backups, replacement, and refusal on unbacked originals. Check redistribution status before packaging. |
| Persistence | Capture saves under the state lock; use ordered persistence helpers. Isolate test config directories. |
| Private data | Sanitize paths, Steam account identifiers, PnP values, serials, Bluetooth addresses, and raw reports. Keep lab captures and generated artifacts ignored. |

Before HID, telemetry, Steam Input, broker, or asset work, read the
[provenance policy](docs/provenance-policy.md) and [source ledger](docs/sources.md).
Consult local `PROVENANCE.md` if present; it is private and absent from fresh
clones. Add sanitized evidence to tracked docs. Do not inspect or derive code,
constants, layouts, schemas, comments, defaults, or structure from incompatible
implementations, including `Forza-Horizon-DualSense-Python`. Community modules
remain data-only.

## Implementation contracts

- This app is pre-1.0. Update producers and consumers together; do not add legacy
  routes, aliases, or compatibility shims without a requested migration.
- Use `moduleId` for Game Module identity and `adapterId` for Telemetry Adapter
  identity. Keep game-specific effect state outside the generic adapter runtime.
- Global Profile provides controller-only tuning. Hide game telemetry controls
  until a Game Profile is selected. Controller aliases never identify profiles.
- The UI uses plain Svelte 5 + Vite. Preserve teardown for timers, listeners,
  sockets, and polling. Keep filesystem discovery off render paths.
- Keep operational screens dense and accessible. Use Svelte event attributes and
  existing Lucide icons. Preserve Button Mapping's 0–100 coordinate model.
- Avoid speculative abstractions, dependencies, and file-size-only extractions.
  Keep validation and side effects with their owning module.

## Validation and timing

`npm run check` runs web and Rust checks; `npm run check:docs` checks local links.
On Windows, use `npm.cmd`; GNU Rust checks require a working native C compiler
and linker. See [Contributing](docs/contributing.md) for platform setup.
Use `rg --no-ignore` only when you need local handoff or lab notes.

| Runtime contract | Interval |
| --- | --- |
| Hardware output / telemetry processing | 33ms |
| Stale telemetry cutoff | 2s |
| Manual output refresh / keepalive | 250ms / 750ms |
| WebSocket invalidation / hardware game detection | 500ms |

Preserve these timings unless the task explicitly changes behavior. For UI
changes, run web checks and inspect the local app with hardware writes disabled.
Record actual commands and results, including unavailable checks.

## Agent skills

| Need | Canonical guide |
| --- | --- |
| Issues (`shiftedx/dualsense-command`, `gh`) | [Issue tracker](docs/agents/issue-tracker.md) |
| Issue category and state labels | [Triage labels](docs/agents/triage-labels.md) |
| Domain vocabulary and architecture decisions | [Domain docs](docs/agents/domain.md) |
| Release evidence and outstanding gates | [Production readiness](docs/production-readiness-plan.md) |

Keep this guide tracked and concise. Put workflow details in the linked owner
doc. Update it when tooling, ownership, or safety contracts change.
