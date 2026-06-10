# Architecture Audit

_Audit date: 2026-06-10. Report only — no code was changed in the original pass._

_Updated 2026-06-10: line numbers and counts refreshed to reflect the
post-decomposition state on `main` after PRs #20/#21 landed six slices. The
three App.svelte headline slices (trigger curve editor, Forza effect state,
profile management) and the three lib.rs slices (`ForzaEffectRuntime`,
background loops, game detection cache) are now extracted and marked completed
below._

This audit looks at DSCC through the lens of the project's own engineering
skills: the domain language in [CONTEXT.md](../CONTEXT.md) and the decisions in
[docs/adr/](adr/). It records the current shape of the two largest source files
and the highest-leverage decomposition opportunities, so the work can be picked
up later in safe, independent slices. Line ranges are paired with anchor symbols
(function/variable names) so they degrade gracefully as lines shift.

## Summary

The Rust agent is well separated: a typed controller-output boundary
([ADR 0004](adr/0004-use-typed-controller-output-boundary.md)), game modules
kept distinct from telemetry adapters
([ADR 0005](adr/0005-separate-game-modules-and-telemetry-adapters.md)), and a
consistent habit of extracting pure compute (hashing, diagnostics, shift
detection). The web UI carried the main smell: `web/src/App.svelte` is now a
2,336-line shell (down from ~2,700) that still bundles several concerns, though
its three largest have been extracted.

Domain-language hygiene is clean in both files. The CONTEXT.md `_Avoid_` terms
(Device, HID device, gamepad, plugin, backend, bus) do not leak into
user-facing or public API names; the code consistently uses Controller, Target
Controller, Profile Resolution, Game Module, Telemetry Adapter, Hardware
Output, Edge Onboard Slot, and Runtime Live Effect.

## web/src/App.svelte (2,336 lines)

A capable shell whose three heaviest concerns have been extracted to `app/`
modules. The remaining distinct concerns are listed with their current line
ranges and anchor symbols.

| Concern | Lines (anchors) | Nature | Module |
| --- | --- | --- | --- |
| App shell & lifecycle | 1–235 (`<script>` header), `onMount` 1952, markup from 1981 | framework boundary | keep |
| Trigger curve editing (geometry + pointer) | **Extracted** — `triggerCurveEditorContext` imported 118, wired 1019–1049; state 308–318 | pure compute + UI state | ✅ `app/triggerCurveEditor.ts` (292 lines) |
| Forza game tuning & effect state | **Extracted** — `createForzaEffectState` 991, handlers 1002–1011; `forzaTuning` 325 | game-module state | ✅ `app/forzaEffectState.ts` (141 lines) |
| Profile resolution, CRUD, import/export | **Extracted** — `createProfileManagement` 1653, handlers 1688–1700 | logic + I/O | ✅ `app/profileManagement.ts` (386 lines) |
| Target Controller selection & rename | state 239–243; `selectTargetController` 750, `pickAllControllers`/`pickControllerTarget` 809–815, `beginControllerRename`…`handleControllerRenameKeydown` 817–858 | logic + state | `app/controllerSelection.ts` (helpers already extracted; component handlers remain) |
| Add-game dialog & Steam library load | state 244–248; `openAddGameDialog`…`addLocalGameFromDialog` 761–807 | I/O + state | `app/gameAddition.ts` |
| Lightbar / RGB (Hardware Output) | state 333–336; `setLightbarEnabled`…`handleLightbarColorChange` 1429–1442; `previewLightbarColor`…`previewRpmColor` 1880–1916 | I/O + state | `app/lightbarState.ts` |
| DSCC Input Bridge session | `inputBridgeBusy` 270; `saveControllerInputMode` 860, `startControllerInputBridge`/`stopControllerInputBridge` 886–912 | I/O + state | `app/inputBridgeSession.ts` |
| App settings & LAN access | state 250–251; `setAppSettingsMessage` 1475; `updateLanAccess`/`updateForzaGlyphOverride` 1560–1603 | I/O + state | `app/appSettingsState.ts` |
| Support bundle & diagnostics | state 252–255; `diagnostics` derive 437; `setSupportBundleMessage`…`exportSupportBundle` 1488–1558 | logic + I/O | `app/supportBundleState.ts` |
| Update check, onboarding, toasts | state 276–281; `loadOnboardingPreference`…`dismissOnboarding` 603–615; `showToast` 661 | small cross-cutting | keep |

The three headline slices listed above are done. Their pure compute and state
machinery now live in the named `app/` modules; App.svelte holds only the thin
wiring (context creation, handler aliases, and markup). The Forza shape in
`app/forzaEffectState.ts` is the template future supported games (e.g. Assetto
Corsa Rally) should follow.

### Remaining order (App.svelte)

The completed slices took App.svelte from ~2,700 to 2,336 lines without touching
backend behavior. The remaining items below are smaller follow-ups, each
independently shippable:

1. **Controller selection & rename**: the pure workspace helpers
   (`deriveTargetControllerWorkspace`, `targetControllerSelection`,
   `singleProfileTargetSelection`) already live in `app/controllerSelection.ts`;
   what remains in App.svelte is the rename component state and handlers
   (`beginControllerRename`…`handleControllerRenameKeydown`, 817–858).
2. **Add-game dialog & Steam library load** (761–807): cohesive I/O for the
   Steam library fetch and local-app validation/addition.
3. **Lightbar / RGB**, **Input Bridge session**, **app settings & LAN**, and
   **support bundle & diagnostics**: small I/O + state follow-ups, each landable
   on its own.

## crates/dscc-agent/src/lib.rs (1,714 lines)

Better factored than the web shell — I/O is wrapped behind `spawn_blocking`,
caching has explicit TTLs, and the typed output boundary holds. The three
opportunities the original audit flagged have all been extracted, leaving lib.rs
as the agent-state and route-wiring core.

| Concern | Status | Module |
| --- | --- | --- |
| `ForzaEffectRuntime` (shift/clutch/suspension compute) | **Extracted** — held as `forza_effect_runtime: ForzaEffectRuntime` field (lib.rs:414), constructed lib.rs:626 | ✅ `effects/forza_runtime.rs` |
| Background loops: device scan, output watchdog, hardware output | **Extracted** — spawned from lib.rs (`output_watchdog_loop` 1653, `hardware_output_loop` 1657, `device_scan_loop` 1687) | ✅ `runtime/{device_scan,output_watchdog,hardware_output}.rs` |
| Game detection cache (TTL policy) | **Extracted** — `mod game_detection_cache` (lib.rs:74), re-exported lib.rs:169 | ✅ `game_detection_cache.rs` |

`ForzaEffectRuntime` is now pure decision logic in `effects/forza_runtime.rs`,
coupled to agent state only through the single field on `AgentState`. The three
background loops live under `runtime/` (declared by `runtime.rs`, re-exported
into lib.rs at line 202), cleanly separating "background work" from "state
queries". The game detection cache (`DiscoveryCache`/`CachedValue`) lives in
`game_detection_cache.rs` with its `AgentState` cache accessors. Together these
moves removed ~330 lines from lib.rs (~2,091 → 1,714).

## Other files (healthy — no action)

These are large but cohesive single-concern files and are left as-is:
`config_model.rs` (~1,272, data types), `effects/runtime_profiles.rs` (~1,170,
effect rules), `agent_types.rs` (~1,112, DTOs), `web/src/lib/mock/api.ts` (~965,
dev-only mock), and `web/src/lib/features/haptics/TelemetryRoutingPanel.svelte`
(~925, self-contained feature).

## How to act on this

Each remaining row in the tables above is an independent slice. Follow the
existing pattern in the history (e.g. "Decompose App.svelte into profile,
button-mapping, and edge-onboard workspaces", "Extract pure
compute_trigger_positions", and the recent trigger curve / Forza / profile
management extractions): extract one concern, keep behavior identical, land it
behind the existing validation set (`cargo test`, `npm run typecheck`,
`npm run build`, the visual smoke and source-audit checks), and repeat. None of
these require ADR changes; they deepen the structure the ADRs already describe.
