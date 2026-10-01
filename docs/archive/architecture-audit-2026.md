# Architecture Extraction Review (2026)

Archived review of module boundaries against [CONTEXT.md](../../CONTEXT.md)
and [ADRs](../adr/). The original 2026-06-10 pass changed no code.
Use [Architecture](../architecture.md) for current ownership and
[Production Readiness](../production-readiness-plan.md) for release gates.

| Date | Evidence |
| --- | --- |
| 2026-06-10 | PRs #20/#21 completed six extractions below. App.svelte fell from about 2,700 to 2,336 lines; agent lib.rs from about 2,091 to 1,714. |
| 2026-07-17 | Later features grew App.svelte to about 2,741 lines. |
| 2026-09-30 | Confirmed extracted modules and remaining handler anchors in the working tree. Removed stale line ranges; historical counts above remain dated. |

## Completed extractions

| Owner | Concern | Module / anchor |
| --- | --- | --- |
| Web | Trigger curve geometry and pointer state | `web/src/app/triggerCurveEditor.ts`; `triggerCurveEditorContext` |
| Web | Forza tuning and effect state | `web/src/app/forzaEffectState.ts`; `createForzaEffectState` |
| Web | Profile resolution, CRUD, import/export | `web/src/app/profileManagement.ts`; `createProfileManagement` |
| Agent | Shift, clutch, suspension decisions | `crates/dscc-agent/src/effects/forza_runtime.rs`; `ForzaEffectRuntime` |
| Agent | Device scan, watchdog, hardware output loops | `crates/dscc-agent/src/runtime/{device_scan,output_watchdog,hardware_output}.rs` |
| Agent | Discovery cache and TTL policy | `crates/dscc-agent/src/game_detection_cache.rs`; `DiscoveryCache` / `CachedValue` |

The agent keeps typed output behind [ADR 0004](../adr/0004-use-typed-controller-output-boundary.md)
and separates game modules from telemetry adapters under
[ADR 0005](../adr/0005-separate-game-modules-and-telemetry-adapters.md).
`AgentState` holds the Forza runtime; `lib.rs` coordinates state and runtime wiring.

## Remaining web candidates

These handlers still live in `web/src/App.svelte`. Extract only when a change
needs the seam; file size alone does not justify another module.

| Concern | Existing seam / handler anchor | Remaining scope |
| --- | --- | --- |
| Controller selection and rename | `app/controllerSelection.ts`; `beginControllerRename` | Rename component state and handlers; selection helpers already extracted. |
| Add-game dialog and Steam library | `openAddGameDialog` | Library fetch and local-app validation/addition. |
| Lightbar / RGB | `setLightbarEnabled` | Preview and save state. |
| Input Bridge session | `startControllerInputBridge` | Mode save, start/stop requests, busy state. |
| App settings / LAN | `updateLanAccess` | Settings requests and feedback. |
| Support bundle / diagnostics | `app/supportBundle.ts`; `exportSupportBundle` | Export/copy I/O and feedback; bundle helpers already extracted. |

Keep shell lifecycle and small cross-cutting wiring in App.svelte. The original
audit proposed `gameAddition.ts`, `lightbarState.ts`, `inputBridgeSession.ts`,
`appSettingsState.ts`, and `supportBundleState.ts`; these were candidate names,
not implemented modules.

## Original no-action findings

The 2026-06-10 review found cohesive concerns in `config_model.rs` (data types),
`effects/runtime_profiles.rs` (effect rules), `agent_types.rs` (DTOs),
`web/src/lib/mock/api.ts` (dev fixtures), and `TelemetryRoutingPanel.svelte`
(feature UI). It also found consistent public domain terms: Controller, Target
Controller, Profile Resolution, Game Module, Telemetry Adapter, Hardware Output,
Edge Onboard Slot, and Runtime Live Effect. These are historical findings, not a
fresh whole-repo audit.

For a needed extraction, move one concern, preserve behavior and ADR boundaries,
and run the [contribution validation](../contributing.md#validation), including a
visual check for UI changes. Follow the completed modules' existing seams.
