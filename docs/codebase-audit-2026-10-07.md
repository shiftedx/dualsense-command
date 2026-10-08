# DSCC codebase audit October 7 2026

DSCC has clear subsystem ownership, useful agent guidance, and a healthy measured
web baseline. This audit identified 21 concrete defects in output lifetime,
stale telemetry, data preservation, UI state consistency and one dependency.
The implementation record below tracks their corrections and verification.

Audit target: version **0.5.1**, commit
`c31bd35f4738c05ab2d47775caf1d9211fa0a484`. The tracked working tree was clean
at the start. Findings and the initial verification record describe that commit;
source line links point to that immutable baseline. This remediation contains
the audit corrections only. The separate UX rework and controller asset
proposals are excluded; the existing presentation is retained. No release is included.

## Scope and evidence

The review covered the eight Rust crates, Svelte shell and feature workflows,
API and DTO boundaries, persistence, telemetry and output loops, Steam writers,
glyph handling, support sanitation, broker lifecycle, tray ownership,
validation scripts, CI, dependency manifests, and contributor documentation.
Review depth focused on production paths and their tests rather than claiming
line-by-line proof of every branch.

**Source trace** means the review followed callers, state changes, and consumers
but did not reproduce the failure in a running agent. **Executed probe** means
an isolated frontend module or compiler check reproduced the specific condition.
**Advisory scan** means the dependency scanner reported an affected locked
version. None of these establishes physical controller behavior.

Hardware output was disabled for diagnostics using
`DSCC_DISABLE_HARDWARE_OUTPUT=1`. No real controller output, Steam file edits,
glyph replacements, driver installation, release publishing, or private capture
inspection formed part of this audit.

## Implementation record

The following corrections retain the existing safety boundaries and runtime
intervals. Deferred browser responses, isolated filesystem fixtures, recording
output seams and controlled-time tests establish software behavior. They do not
establish physical controller or real provider compatibility.

| Finding | Correction | Regression owner |
| --- | --- | --- |
| 1 | Controller-scoped output ownership and expiry retry; old generations cannot clear replacements. | [Manual tests](../crates/dscc-agent/src/tests/effects/manual_tests.rs), [watchdog](../crates/dscc-agent/src/runtime/output_watchdog.rs) |
| 2 | Producer sample identity controls freshness; unchanged pages expire, advancing/wrapped/restarted samples recover. | [Telemetry tests](../crates/dscc-agent/src/tests/telemetry.rs) |
| 3 | Complete-copy/sync staging publishes originals exclusively; failed or interrupted staging cannot become restorable. Switching replacements requires restore first. | [Glyph tests](../crates/dscc-agent/src/tests/forza_glyphs.rs) |
| 4 | Distinct load failures, exclusive recovery copies and startup I/O mutation rejection before side effects. | [Persistence tests](../crates/dscc-agent/src/persistence.rs) |
| 5 | Per-canonical-target lock spans the Steam read/edit/backup/commit transaction; external content rechecked. | [Steam tests](../crates/dscc-agent/src/tests/steam_input.rs) |
| 6 | Exclusive unique backup/staging paths retain every rapid-write preimage. | [Steam tests](../crates/dscc-agent/src/tests/steam_input.rs) |
| 7 | Opaque path fields and drive/UNC/POSIX sanitation preserve HTTP(S) spans; identifiers still redact. | [API tests](../crates/dscc-agent/src/tests/api.rs) |
| 8 | Full mapping context invalidates selection, drafts, messages and optimistic state. Monotonic context/request ownership rejects late Steam, Bridge and paddle responses, refresh cleanup, retained callbacks and teardown work. | [Workflow fixtures](../web/scripts/profile-workflows.mjs), [browser regressions](../web/scripts/profile-browser.mjs) |
| 9 | Request/context generations discard late profile/controller success and failures, including teardown. | [Browser regressions](../web/scripts/profile-browser.mjs) |
| 10 | Edge controller/generation checks cover success, error, finally and teardown. | [Browser regressions](../web/scripts/profile-browser.mjs) |
| 11 | Save captures the acknowledged config and original targets before awaits; late context completions are discarded and newer same-context drafts stay dirty. | [Workflow fixtures](../web/scripts/profile-workflows.mjs), [browser regressions](../web/scripts/profile-browser.mjs) |
| 12 | Import preserves canonical `game_id`; obsolete duplicate export aliases removed. | [Workflow fixtures](../web/scripts/profile-workflows.mjs), [profile tests](../crates/dscc-agent/src/tests/profiles.rs) |
| 13 | Explicit reactive draft snapshot drives Save/keyboard eligibility after failed live PUT. | [Browser regressions](../web/scripts/profile-browser.mjs) |
| 14 | Agent export materializes complete canonical stock configuration; production UI preset tables removed. | [Profile tests](../crates/dscc-agent/src/tests/profiles.rs), [browser regressions](../web/scripts/profile-browser.mjs) |
| 15 | Bounded independent provider-health probes publish cached status without holding the operational backend mutex. | [Bridge tests](../crates/dscc-agent/src/input_bridge.rs) |
| 16 | Adapter disable closes fresh telemetry eligibility and associated output immediately. | [Telemetry tests](../crates/dscc-agent/src/tests/telemetry.rs) |
| 17 | Source/session transitions reset gear, clutch and event history before the next sample. | [Telemetry tests](../crates/dscc-agent/src/tests/telemetry.rs) |
| 18 | Required group/source/mode/input/activator selectors must resolve one unique parsed and physical slot. | [Steam tests](../crates/dscc-agent/src/tests/steam_input.rs), [mapping session](../web/src/app/buttonMappingSession.ts) |
| 19 | Already-current paddles preserve complete bytes, revision and backup count, including UI-shaped requests. | [Steam tests](../crates/dscc-agent/src/tests/steam_input.rs) |
| 20 | Locked source-map-js updated to patched 1.2.2; notices/inventory regenerated. | [Web lockfile](../web/package-lock.json), advisory scans |
| 21 | Saved feel preview requests 3,000ms with its own lifetime; pending start/refresh cannot replace it, and competing previews await controller-owned stops. Normal manual test remains 30,000ms. | [Browser regressions](../web/scripts/profile-browser.mjs) |

Additional audit recommendations implemented: native environment preflight,
broker PR build, locked production Rust configuration, focused workflow/browser
CI checks and diagnostics that report the actual bound IP independently of
hardware output. The existing UI layout and artwork are retained.

The corrections passed independent task reviews, including deferred-save,
manual-preview/stop races and interrupted glyph backup publication. The audit-only
extraction retained all backend files exactly and passed a new code review.
That review reproduced a late mapping response overwriting another game's editor;
the correction now rejects obsolete completions after switch, leave-and-return
and runtime teardown. Independent actual-module probes and mounted-App regressions
verify the fix. Original findings and initial checks remain historical.

Remaining boundaries: Steam's external-content recheck is not an OS
compare-and-swap against arbitrary writers. Generic post-startup disk failures
still log warnings; this change does not redesign all mutations into durable
acknowledgement transactions. Glyph backup publication requires same-directory
hard-link support and fails before target replacement when unavailable; failure
tests model interrupted staging, not power-loss durability. Physical output, onboard writes, actual provider
operation, installer startup, signing and Linux hardware gates still require
their [release evidence](production-readiness-plan.md).

## Current verification October 7

The complete check ran again after the final mapping ownership correction on
`codex/audit-only` and passed. Node 24.15.0, GNU Cargo 1.95.0, MinGW GCC 16.2.0
and .NET SDK 10.0.401 were available. All diagnostics and app smoke processes used
`DSCC_DISABLE_HARDWARE_OUTPUT=1`; an ignored Cargo target preserved the original
invalid build cache. Measurements below describe this audit-only tree.

| Command / inspection | Audit-only integration result |
| --- | --- |
| `npm.cmd run check` | Passed environment, docs, release metadata, complete web suite, Rust formatting/all-feature tests/Clippy with warnings denied, locked production feature check and broker Release build. |
| Web behavioral checks | Svelte 0 errors/0 warnings; 68 actual-module workflow cases and 55 compiled-App browser cases pass. Deferred mapping cases cover Steam/Bridge/paddles, request supersession, navigation, leave-and-return, teardown, rejection and refresh cleanup. |
| Rust workspace / broker | 401 tests passed, 0 failed, 2 existing manual tests ignored. Broker Release compilation: 0 warnings, 0 errors. |
| `npm.cmd run check:perf` | Four native guards pass. Mapping p95: lookup 0.053ms, chips 0.047ms, parse 0.003ms over 300 samples. |
| Bundle / curve drag / visual smoke | 38 files; 786.1 KiB raw, 438.7 KiB computed gzip, 111.6 KiB JS gzip. 240 drag moves: 16.8ms frame p95 and 13.7 mutations/move, within unchanged 50ms/32 limits. Four routes across three viewports pass. Gzip estimates are not measured HTTP transfers; synthetic drag is not native HID latency. |
| Advisory / distribution checks | npm and Cargo scans report zero known vulnerabilities; Cargo scanned 224 dependencies against 1,294 advisories. Exact asset/lock distribution checks and dependency notices pass for 220 packages. |
| Installer preflight | Preflight/retention regressions pass; no MSI installation performed. |
| Actual production agent / Chromium | Optimized default-feature CLI served the production bundle on isolated loopback/configuration paths. Diagnostics confirm loopback-only and hardware output disabled. With no readable connected controller, Tuning redirects to Status and Controller details shows no selected controller, with no page errors or horizontal overflow. |
| Presentation scope | Styles, presentation components, artwork and product contract match the base branch. App markup differs only in the three SavedRail bindings needed for the corrected preview lifetime. |
| Hardware / actual provider / MSI installation | Not performed by the audit-only integration; outstanding release gates remain open. |

## Prioritized findings

P1 items affect the hardware output safety contract and should precede further
feature work. P2 items affect reliability, privacy, data preservation, or an
identified performance boundary. P3 is a smaller consistency correction.

### 1 P1 Overlapping controller tests can skip expiry cleanup

**Evidence: source trace.** [Manual override generation](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/lib.rs#L736)
is global. Starting a test on controller B supersedes controller A's generation;
A's [refresh task returns](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/api/effects.rs#L118) before
its neutralization and session release. Both runtime loops pause while B's
override remains active. After B expires, normal profile resolution selects the
first connected controller, while the watchdog accepts that controller's
permitted output or lighting. A second controller's old effect can remain.

Scope test generation and deadlines to the controller, or neutralize the
previous target before replacing a global test. Validate generation inside the
serialized write path too. Regress overlapping tests with A second in registry
order, same-controller replacement, explicit stop, and failed replacement.
Use a recording output seam: the existing dry-run agent mock bypasses the timed
hardware branch and cannot prove emitted neutralization.

### 2 P1 A stalled shared memory producer never becomes stale

**Evidence: source trace.** The Assetto reader reads `packet_id` but manufactures
a [new local sequence on each 33ms read](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/assetto_shared_memory.rs#L356).
It applies unchanged pages and refreshes
[last_packet_at](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/adapter_runtime.rs#L128). If the game
stalls with a readable mapping and its last state is driving, the two-second
freshness gate never closes.

Refresh live freshness when the source sample identity changes. Handle wrapping
and restarted packet IDs. Regress repeated identical pages beyond two seconds,
neutral output at expiry, and recovery when the producer advances.

### 3 P2 Changing glyph archives can overwrite the original backup

**Evidence: source trace.** [Install planning](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/forza_glyphs.rs#L133)
treats a target that differs from the currently configured archive as an
original. The subsequent copy overwrites an existing backup. Installing pack A,
then selecting pack B and reinstalling, replaces the original Xbox backup with
A. Restore under B then restores A.

Preserve an existing verified original. Track the installed replacement's
identity, or reject ambiguous replacement pending game-file verification.
Regress original to A to B to restore, asserting byte-for-byte original recovery.

### 4 P2 Failed settings loads silently become writable defaults

**Evidence: source trace.** [Agent construction](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/lib.rs#L583)
discards read/deserialization failures through `.ok().unwrap_or_default()`.
The next settings mutation can replace the unreadable original with defaults,
losing profiles and recovery evidence.

Distinguish absence from failed reads. Expose the load failure and preserve the
original before allowing replacement saves. Regress malformed JSON, unreadable
state, and a subsequent mutation; the original must remain recoverable.

### 5 P2 Concurrent Steam edits can lose accepted changes

**Evidence: source trace.** API calls dispatch
[independent blocking writers](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/api.rs#L70). Each reads
the full VDF and later [writes its own result](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/steam_input/writer.rs#L428)
without serializing the transaction or checking for changes since its read.
Two tabs can receive success while one edit erases the other. Steam can also
change the file between DSCC's read and write.

Serialize the complete read, validate, modify, backup, and replacement operation.
Recheck contents before committing and reject stale external edits. Prefer
atomic replacement to truncating the live VDF. Regress concurrent changes to
different bindings and an external modification before commit.

### 6 P2 Steam backup names collide within one second

**Evidence: source trace.** [Backup names](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/steam_input/writer.rs#L423)
use second-resolution timestamps and `fs::copy` permits replacement. Rapid
successful edits replace the first pre-edit backup with an intermediate layout.

Create unique backups with exclusive creation. Regress multiple edits under the
same clock timestamp and assert that each original backup remains intact.

### 7 P2 Support bundles retain UNC and custom POSIX paths

**Evidence: source trace.** [Sanitation](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/support_bundle.rs#L463)
recognizes drive paths and four environment roots. Its
[path recognizer](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/support_bundle.rs#L536) leaves ordinary
UNC, extended UNC, and non-home POSIX paths intact. Config/web asset overrides
and diagnostic text can therefore expose private paths in a bundle that claims
sanitation.

Replace known path fields with opaque labels and extend free-text redaction for
supported path forms. Regress synthetic UNC paths, extended UNC, and custom
POSIX overrides without using private user values.

### 8 P2 Button Mapping carries drafts across games

**Evidence: executed probe.** A
[context change](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/app/buttonMappingSession.ts#L214) clears optimistic
bindings and hover state but retains draft/selection identity. When game B has
the same binding key as game A, the
[draft reload condition](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/app/buttonMappingSession.ts#L260) does not
run. Applying the editor can write A's action to B's layout.

An isolated call to the real session module produced context
`game-b|game-b|ctrl|DualSense` with draft `key_press A, , A`, while B's binding
was `key_press B, , B`.

Reset draft keys, selection, and draft contents when layout/game/controller
context changes, then initialize from the new binding. Add a session-level
fixture with identical slot identity and different actions across two games.

### 9 P2 Obsolete custom profile responses replace the current editor

**Evidence: source trace.** [Profile loading](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/App.svelte#L1589)
awaits a custom export, then applies it and captures a baseline without checking
the current controller, selected profile, scope, or request generation. Selecting
A then B can display A's late response under B's selection.

Capture selection and request identity before awaiting. Discard obsolete
completions before applying configuration or baselines. Regress deferred A and
B responses completed in reverse order, including switching controller/scope.

### 10 P2 Obsolete Edge reads replace slots after controller selection changes

**Evidence: source trace.** [Edge loading](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/App.svelte#L1510)
unconditionally updates profiles, errors, and loading on completion. A read for
Edge A that finishes after selection of B can populate B's view with A's slots
and capabilities; `loadedFor` can still identify B. Rust's protected-slot
enforcement remains in place, but the UI presents the wrong controller state.

Guard success, failure, and finally handling by controller and request generation.
Regress reverse-order reads and a late failure after a successful newer read.

### 11 P2 Saving can capture a different discard baseline

**Evidence: source trace.** [Profile save](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/app/profileManagement.ts#L262)
captures the request config and saves its signature. The
[baseline setter](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/App.svelte#L1983) rebuilds the config from the draft
after awaiting the request. Editing during the save can produce a saved signature
for A and a saved/discard object for unsaved B.

Pass the captured config into the baseline setter and clone that object.
Regress editing while a save is pending; saved rows and Discard must reflect the
acknowledged request config.

### 12 P2 Game Profile export and import lose game association

**Evidence: executed probe and source trace.** The agent export includes
`game_id`, but [profileImportPayload](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/lib/features/profiles/profileSelection.ts#L157)
and the API import type omit game identity. A Game Profile imported through the
UI becomes a Global Profile.

The actual helper returned only id, schema, name, and config for an export with
`game_id: "forza-horizon-6"`. Update producer/consumer types together and preserve
the current exported identifier through the import request. Regress a complete
Game Profile round trip and a Global Profile round trip.

### 13 P2 Save eligibility does not react to draft edits

**Evidence: compiler probe.** The
[dirty expression](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/App.svelte#L1166) reads draft values through an
opaque helper call. Svelte's generated dependency function includes only
`currentControllerConfig` and `profileSaveBaselineSignature`. A successful live
PUT happens to trigger reevaluation; a failed PUT can leave edited rows with
Save disabled.

Derive dirty state from the existing explicit `profileDraftSnapshot`, or expose
the required draft dependencies. Regress immediate edit-to-dirty behavior and a
failed live update, including keyboard Save eligibility.

### 14 P2 Stock preset configuration has two conflicting owners

**Evidence: executed frontend probe and source trace.**
[UI preset reconstruction](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/App.svelte#L1563) uses local Base or
Immersive tables. [UI rev-limiter intensities](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/app/hapticsState.ts#L258)
are 55 and 62; the [agent values](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/built_in_presets.rs#L54)
are 85 and 95. Assetto Rally also receives the UI Base table despite a distinct
Rust preset. The editor captures these reconstructions as saved values.

Have the agent materialize canonical stock configurations through the existing
profile boundary. Remove duplicated production preset truth; keep mock fixtures
separate. Regress selection, reset, and copy for every stock profile and compare
the complete configuration, not just curve constants.

### 15 P2 Snapshot status queries contend with Input Bridge forwarding

**Evidence: source trace; latency impact requires measurement.** Each
[snapshot](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/lib.rs#L1636) awaits live backend status through
the same [serialized operation queue](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/input_bridge.rs#L88)
as virtual input submission. Status performs broker I/O with a two-second
response timeout. A slow provider can delay the entire snapshot and the 8ms
forwarding loop. `spawn_blocking` avoids blocking Tokio workers but retains
queue contention.

Serve cached provider status in snapshots. Refresh health independently at a
bounded cadence without occupying the forwarding queue for a provider response.
Measure snapshot and submit latency with a slow/failing fake provider before
and after the change.

### 16 P2 Disabling a live adapter leaves its output gate open

**Evidence: source trace.** [Adapter disable](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/api.rs#L38)
changes enabled/presentation state but retains live freshness. The
[runtime output gate](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/effects/materialization.rs#L343)
does not check enabled state, so cached effects can continue until the two-second
stale cutoff despite disabled status.

Include enabled state in runtime eligibility and clear active-source live
freshness when disabling. Regress disabling immediately after a driving packet;
the next materialized trigger/rumble frame must be neutral.

### 17 P2 Racing effect history crosses source boundaries

**Evidence: source trace.** Both racing adapters feed one
[effect history](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/lib.rs#L1247). Replacing the signal
snapshot on a source change does not reset previous gear, clutch interpretation,
or latched events. Switching Forza to Assetto can synthesize a shift from the
previous game's gear and inherit its clutch history.

Reset effect history before processing a changed source/session identity. Keep
the effect state with its effect owner. Regress a source switch with different
gear/clutch values; the first new-source sample should establish a baseline.

### 18 P2 Incomplete Steam selectors allow ambiguous first match writes

**Evidence: source trace and existing fixture.**
[Writer matching](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/steam_input/writer.rs#L266) treats
missing group and activator selectors as wildcards. An input-only request can
edit the first of several matching slots. The
[existing duplicate-input fixture](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/tests/steam_input.rs#L371)
has `dpad_north` in groups 9 and 14.

Require full slot identity and validate against the current parsed layout, or
reject requests resolving to multiple bindings. Carry group, source, mode, input,
and activator through both DTOs. Missing source/mode fields alone did not prove
a valid same-group collision; the established failure is the ambiguous wildcard
request. Regress omitted selectors and stale layout identity.

### 19 P2 Paddle presets reject already correct bindings

**Evidence: source trace.** [Paddle preset handling](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/crates/dscc-agent/src/steam_input/paddle_preset.rs#L83)
treats an unchanged replacement as a conflict, although the shared helper's
`None` means no edit was needed. Reapplying a preset fails, and one already
correct paddle prevents updating the other.

Retain the original contents for unchanged replacements and preserve accurate
changed flags. Regress neither, one, and both paddles already matching, in both
dry-run and guarded-write modes using isolated test files.

### 20 P2 The locked build dependency has a current security advisory

**Evidence: advisory scan.** `npm.cmd --prefix web audit --json` reports one
high-severity advisory for `source-map-js` 1.2.1, locked in
[package-lock.json](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/package-lock.json#L1150). It enters through
Vite to PostCSS. The [reviewed advisory](https://github.com/advisories/GHSA-68fv-2mgg-jv7q)
identifies indexed source-map input that can block the Node event loop and names
1.2.2 as patched.

Update the transitive lock entry to a patched compatible version and rerun build,
checks, and the advisory scan. This concerns build tooling; the scan does not
establish that the shipped browser app or Rust agent exposes the vulnerable
source-map parser to remote input. Refresh applicable dependency records when
the lockfile changes.

### 21 P3 The displayed preview duration differs from the request

**Evidence: source trace.** [SavedRail](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/lib/features/tuning/SavedRail.svelte#L124)
promises `3s`, but its callback uses the
[30,000ms base-feel test](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/App.svelte#L243). Give this preview a distinct
three-second duration or update the displayed promise. Verify the actual request
duration with a fixture, then inspect the affected UI with hardware writes
disabled.

## Performance baseline and next measurements

| Check | October 7 result | Interpretation |
| --- | --- | --- |
| Mapping p95, 300 synthetic samples | Lookup 0.045ms; chips 0.028ms; parse 0.002ms | Current helper costs are small on this host. |
| Curve drag, 240 moves | Frame p95 16.7ms; 13.1 mutations/move | Passes 50ms and 32 mutation budgets. |
| Production web assets | 38 files; 784.3 KiB raw; 438.0 KiB computed gzip | Passes current budgets. |
| JavaScript | 110.9 KiB computed gzip | No evidence that broad code splitting is the first priority. |
| Full hardware runtime latency | Unavailable | Requires native build tools, a recording backend, and physical validation. |

The current runtime already caches prepared effect profiles, separates preview
and hardware temporal state, uses borrowed normalized curves and fixed encoding
buffers, suppresses duplicate reports with keepalive, and expires hardware
detection independently of slow filesystem discovery. Preserve those paths.

The highest-value performance change is finding 15. After that, record packet
receipt to output submission p95/p99, state-lock wait/hold time, materialization
duration, snapshot latency, and forwarding latency under provider contention.
Measure at realistic packet rates and with multiple controllers. Existing
[perf guards](../crates/dscc-agent/src/tests/perf_guards.rs) mostly time helpers;
the full materialization timing test is ignored. Helper throughput cannot prove
the 33ms runtime deadline.

Other candidates need measurements before implementation:

- Full game-detection/signal snapshot clones and repeated candidate sorts may
  consume CPU under load. Profile these costs before changing ownership or data
  structures.
- Hidden pages still apply complete socket snapshots. Compare hidden CPU and
  mutations before retaining only the latest snapshot until visibility returns.
- The socket falls back to five-second polling after failure. A bounded reconnect
  would restore push responsiveness after an agent restart; test outage recovery
  before adding it.
- Fonts account for much of the bundle. Preserve supported glyph coverage; reduce
  shipped subsets only if measured startup or distribution size warrants it.
- The gzip budget computes potential compression. The current static route uses
  ServeDir without an explicit compression layer or precompressed sidecars;
  measure actual response bytes before using gzip figures as transfer evidence.

## Organization documentation and agent friendliness

The root agent guide, [Architecture](architecture.md),
[Contributing](contributing.md), [CONTEXT](../CONTEXT.md), and the ADRs give agents
clear owners, vocabulary, prohibited sources, timing contracts, and targeted
checks. Documentation link checks pass. The current production-readiness guide
also separates historical engineering results from outstanding hardware,
installer, Bridge, Linux, startup, and signing gates.

Keep the eight-crate division and feature owners. App.svelte and agent lib.rs
remain large coordination points, but line count alone does not justify more
modules. The concrete seams to improve are async editor request ownership,
saved-baseline capture, canonical preset materialization, Steam transactions,
and per-controller output lifetime.

The agent crate uses broad parent imports and root re-exports throughout its
submodules. Prefer explicit imports for boundaries being changed so agents can
identify dependencies locally; avoid a wholesale mechanical rewrite.

Improve the working guides and checks in these narrow ways:

| Improvement | Purpose and acceptance |
| --- | --- |
| Add a small environment preflight | Check Node, GNU Rust, gcc/dlltool, browser availability, and optional .NET 10 SDK. Give exact setup guidance before starting long gates. |
| Document focused regression commands | Map the findings above to Rust test modules and frontend session/async fixtures; retain the current owner map. |
| Build the C# broker in PR CI | Current ordinary CI covers Rust/web; broker publishing occurs in release CI. Catch broker compile failures before tagging and test protocol/provider failure behavior. |
| Check production Rust feature configuration | Keep all-feature tests, and also validate default/release paths where mock/debug gates differ. |
| Keep advisory scans current | Dependabot updates are useful; document a supported scanner version and run npm/Cargo advisory gates with maintained tools. |
| Clarify diagnostics loopback semantics | Agent diagnostics currently derive `loopback_only` from the hardware-output flag. Network exposure should follow the effective bind address, or the field should explicitly describe output mode. |

The existing typecheck, source audit, DTO/math fixtures, bundle budgets, and
visual smoke tests are valuable. Add behavioral fixtures for session context
switches, deferred requests, save-baseline capture, profile round trips, and
controller expiry. A broader source-regex checklist would not catch these bugs.

## Complexity reduction pass

No unused Cargo dependency emerged from `cargo machete`. The review found no
reason to add a state library, plugin framework, generic cache layer, or arbitrary
component extraction.

- `delete:` duplicated production preset truth in the UI. Replace it with agent-materialized configuration as part of finding 14. [hapticsState.ts](https://github.com/shiftedx/dualsense-command/blob/c31bd35f4738c05ab2d47775caf1d9211fa0a484/web/src/app/hapticsState.ts#L258)
- `shrink:` repeated browser-test port/server lifecycle helpers. Reuse one small helper if those scripts next change together. [visual-smoke.mjs](../web/scripts/visual-smoke.mjs), [curve-drag-budget.mjs](../web/scripts/curve-drag-budget.mjs)

net: approximately -80 to -140 lines, -0 dependencies possible. This is a static
estimate before implementation, not a measured patch saving. Correctness and
safety checks must survive any reduction.

## Initial verification record

| Command or probe | Result |
| --- | --- |
| `git status --short --ignored` | Tracked working tree clean before the audit; local captures/builds/notes ignored. |
| `npm.cmd run check` | Docs, release metadata, and the complete web suite passed; root command exited 1 at Rust because Cargo was absent from the session PATH. |
| `npm.cmd run check:rust` with Cargo added to session PATH | Format check passed; compilation stopped at missing `dlltool.exe`. Workspace tests and Clippy did not complete. |
| `npm.cmd run check:perf` with Cargo added to session PATH | Blocked by missing `dlltool.exe` and `gcc.exe`. Standalone web mapping budgets passed within the full web suite. |
| `node tools/check-release.mjs --distribution` | Passed version, asset inventory/hash, redistribution, and notice-lock checks. |
| `powershell -NoProfile -ExecutionPolicy Bypass -File packaging/windows-installer-smoke.test.ps1` | Installer preflight and retention regression checks passed; this does not test an MSI installation. |
| `npm.cmd --prefix web audit --json` | Exited 1: one high source-map-js advisory, finding 20. |
| `cargo audit --file Cargo.lock` | Installed cargo-audit 0.21.2 failed to parse a CVSS 4.0 advisory. No current Rust advisory conclusion is available. |
| `cargo machete` | Passed; no unused Cargo dependencies reported. |
| `dotnet --list-sdks` | No SDK installed; .NET 8 runtimes exist. Broker targets .NET 10 and could not be built here. |
| Svelte compile and actual frontend module probes | Confirmed findings 8, 12, 13, and the frontend values in 14. No app state or external files changed. |

The web suite reported zero typecheck errors/warnings; source audit, snapshot
mapping, haptics parity, build, release size, 12 route/viewport smoke cases, and
drag budget passed. Those checks do not cover the identified async and lifetime
failures. The dated September 30 Rust test/advisory results in the readiness
guide are historical and were not substituted for this run.

## Original recommended implementation order

1. Close findings 1 and 2 with recording/controlled-time regressions; preserve
   shutdown ordering and validate physical neutralization separately.
2. Preserve data and privacy in findings 3 through 7. Keep changes with the owning
   persistence, filesystem, and support modules.
3. Correct frontend scope, async completion, baseline, and dirty-state behavior
   in findings 8 through 14 using deferred-response and round-trip fixtures.
4. Remove Bridge status contention and close adapter/source eligibility gaps in
   findings 15 through 17 without changing the documented runtime cadence.
5. Complete Steam targeting/idempotency, update the advisory dependency, and fix
   preview copy in findings 18 through 21.
6. Run the full gates with native tools, then collect end-to-end latency and the
   outstanding [release evidence](production-readiness-plan.md). Use measurements
   to choose any further optimization.
