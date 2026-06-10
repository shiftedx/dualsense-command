# Architecture Audit

_Audit date: 2026-06-10. Report only — no code was changed in this pass._

This audit looks at DSCC through the lens of the project's own engineering
skills: the domain language in [CONTEXT.md](../CONTEXT.md) and the decisions in
[docs/adr/](adr/). It records the current shape of the two largest source files
and the highest-leverage decomposition opportunities, so the work can be picked
up later in safe, independent slices.

## Summary

The Rust agent is well separated: a typed controller-output boundary
([ADR 0004](adr/0004-use-typed-controller-output-boundary.md)), game modules
kept distinct from telemetry adapters
([ADR 0005](adr/0005-separate-game-modules-and-telemetry-adapters.md)), and a
consistent habit of extracting pure compute (hashing, diagnostics, shift
detection). The web UI carries the main smell: `web/src/App.svelte` is a single
~2,700-line shell that bundles 20+ concerns.

Domain-language hygiene is clean in both files. The CONTEXT.md `_Avoid_` terms
(Device, HID device, gamepad, plugin, backend, bus) do not leak into
user-facing or public API names; the code consistently uses Controller, Target
Controller, Profile Resolution, Game Module, Telemetry Adapter, Hardware
Output, Edge Onboard Slot, and Runtime Live Effect.

## web/src/App.svelte (~2,700 lines)

A capable but monolithic shell. Distinct concerns currently bundled together,
with approximate line ranges:

| Concern | Approx. lines | Nature | Suggested module |
| --- | --- | --- | --- |
| App shell & lifecycle | 1–350 | framework boundary | keep |
| Trigger curve editing (geometry + pointer) | 690–1300 | pure compute + UI state | `app/triggerCurveEditor.ts` |
| Forza game tuning & effect state | 1004–1076, 2020–2202 | game-module state | `app/forzaEffectState.ts` |
| Profile resolution, CRUD, import/export | 401–533, 1314–1546, 2204–2244 | logic + I/O | `app/profileManagement.ts` |
| Target Controller selection & rename | 234–371, 722–765, 825–866 | logic + state | `app/controllerSelection.ts` |
| Add-game dialog & Steam library load | 242–246, 769–815 | I/O + state | `app/gameAddition.ts` |
| Lightbar / RGB (Hardware Output) | 330–333, 1586–1598, 2246–2282 | I/O + state | `app/lightbarState.ts` |
| DSCC Input Bridge session | 268, 868–920 | I/O + state | `app/inputBridgeSession.ts` |
| App settings & LAN access | 248–249, 1717–1760 | I/O + state | `app/appSettingsState.ts` |
| Support bundle & diagnostics | 250–253, 1651–1715 | logic + I/O | `app/supportBundleState.ts` |
| Update check, onboarding, toasts | 274–279, 589–669 | small cross-cutting | keep |

Several pure functions already live here and are extraction-safe today (they
depend only on their arguments): `profileConfigSignature` (`App.svelte:983`),
`curveGraphPointFromPointer` (`App.svelte:1110`), and `applyEditableConfig`
(`App.svelte:1315`).

### Recommended order (App.svelte)

1. **Trigger curve editor** (~550 lines): the largest, most self-contained, and
   the most testable (curve geometry and point math are pure). Extract state and
   handlers into `app/triggerCurveEditor.ts`, keep the panel component thin.
2. **Forza effect state** (~400 lines): isolates Game Module-specific tuning and
   makes room for future supported games (e.g. Assetto Corsa Rally) to follow
   the same shape.
3. **Profile management** (~300 lines): cohesive Profile Resolution workflows
   (select, create, rename, delete, save, import/export, preview).

Landing these three would take App.svelte under ~1,000 lines without touching
backend behavior. Items 4–8 (controller selection, game addition, lightbar,
input bridge, app settings) are smaller follow-ups, each independently
shippable.

## crates/dscc-agent/src/lib.rs (~2,100 lines)

Better factored than the web shell — I/O is wrapped behind `spawn_blocking`,
caching has explicit TTLs, and the typed output boundary holds. The remaining
opportunities are about making the runtime's shape explicit rather than fixing a
god-object.

| Concern | Approx. lines | Suggested module |
| --- | --- | --- |
| `ForzaEffectRuntime` (shift/clutch/suspension compute) | 469–629 | `effects/forza_runtime.rs` |
| Background loops: device scan, output watchdog, hardware output | 2004–2091 | `runtime/{device_scan,output_watchdog,hardware_output}.rs` |
| Game detection cache (TTL policy) | 1515–1588 | `game_detection_cache.rs` |

`ForzaEffectRuntime` (`lib.rs:469`) is the cleanest first move: it is mostly
pure decision logic and has minimal coupling to agent state. Moving the three
background loops (`device_scan_loop` at `lib.rs:2004`, `output_watchdog_loop` at
`lib.rs:2033`, and the hardware output loop that follows) into a `runtime/`
submodule separates "background work" from "state queries" and would clarify the
file's responsibilities. Together these are ~330 lines (~16%).

## Other files (healthy — no action)

These are large but cohesive single-concern files and are left as-is:
`config_model.rs` (~1,270, data types), `effects/runtime_profiles.rs` (~1,170,
effect rules), `agent_types.rs` (~1,110, DTOs), `web/src/lib/mock/api.ts` (~965,
dev-only mock), and `web/src/lib/features/haptics/TelemetryRoutingPanel.svelte`
(~925, self-contained feature).

## How to act on this

Each row in the tables above is an independent slice. Follow the existing
pattern in the history (e.g. "Decompose App.svelte into profile, button-mapping,
and edge-onboard workspaces", "Extract pure compute_trigger_positions"): extract
one concern, keep behavior identical, land it behind the existing validation set
(`cargo test`, `npm run typecheck`, `npm run build`, the visual smoke and
source-audit checks), and repeat. None of these require ADR changes; they
deepen the structure the ADRs already describe.
