# Windows Hardware Matrix

Release candidate: `0.5.1`; updated 2026-09-30. Use this matrix for public
controller/transport claims. **Implemented** means the app path exists;
**verified** requires the full current-candidate physical checklist. Mock tests
and historical runs do not complete a cell.

## Current Windows Matrix

| Controller | USB | Bluetooth |
| --- | --- | --- |
| DualSense | Implemented; physical pass pending | Implemented; physical pass pending |
| DualSense Edge | Implemented; physical pass pending | Earlier partial evidence; candidate pass pending |

Edge onboard sync uses guarded feature reports when the host permits access.
The default `Fn + Triangle` slot is protected; assignable-slot sync requires
acknowledgement and matching typed readback. Unavailable paths stage locally.

## Evidence So Far

| Run | Observed | Limit |
| --- | --- | --- |
| September 30, `0.5.0` working-tree binary, Edge Bluetooth | User confirmed L2, R2, rumble, lightbar and neutralization on graceful exit; exit during R2 completed in 10ms. | Partial runtime run; not an exact `0.5.1` pass. Remaining checklist and onboard retest pending. |
| May 2026, Edge Bluetooth | Enumeration, permissions, battery/config, slot reads, profile resolution, triggers, lightbar, rumble, manual tests and bounded `Fn + Square` no-op write with matching readback. | Historical evidence; not a current-candidate pass. |
| Rust tests | Typed output, encoding/decoding, safety gates and guarded onboard behavior. | Software evidence; no physical support claim. |

## Validation Checklist

Run each cell with the current artifact; record Pass / Fail / Not run using the
[validation template](hardware-validation-template.md). Real output is required
only for deliberate physical tests; use output suppression for diagnostics per
[Contributing](contributing.md#setup-and-local-modes).

| Step | Check | Pass condition |
| --- | --- | --- |
| 1 | Clean MSI install | Complete [installer smoke](windows-installer-smoke.md); retain evidence. |
| 2 | Start-menu launch | Tray and agent start. |
| 3 | Connect target transport | Correct controller is available. |
| 4 | Controller details/input | Family, transport, available battery and live sticks/triggers/buttons are correct; private identifiers are hidden. |
| 5 | L2/R2 preview | Both effects are felt. |
| 6 | Preview completion | Responsive start; triggers return to neutral. |
| 7 | Lightbar and rumble preview | Both update without API errors; rumble ends. |
| 8 | No supported game | Global Profile; no game telemetry trigger/rumble output. |
| 9 | Start supported game | Expected Game Profile resolves. |
| 10 | Detection lightbar | Expected color applies. |
| 11 | Driving telemetry | Fresh packets and physical trigger/body effects. |
| 12 | Stop telemetry/game | Triggers/rumble neutralize within the 2s stale cutoff. |
| 13 | Disconnect/reconnect | Recovery without reinstall. |
| 14 | Support bundle | No raw paths, serials, Bluetooth addresses, account ids or reports. |
| 15 | Tray quit during brief effect | Neutralization and process exit. |
| 16 | Tray restart during brief effect | Neutralization before restart. |
| 17 | Tray termination / Windows session end | Record each observation separately. Forced agent/OS kill cannot guarantee a final write; timeout is not confirmed cleanup. |

## DualSense Edge Onboard Extras

Repeat over USB and Bluetooth:

1. Read available Fn slots; verify `Fn + Triangle` cannot be overwritten.
2. Confirm a safe, identity-mapped test profile for an assignable slot.
3. Write, re-read and compare supported static settings.
4. Confirm hardware-synced status appears only after acknowledgement and matching
   readback; disabled/unavailable writes remain staged locally.

## Production-Ready Gate

Mark a cell **verified** only after every applicable check passes on the current
candidate. Document failures as limitations or linked issues before publishing;
do not describe an incomplete cell as fully verified. Edge USB/Bluetooth claims
also require onboard checks and explicit default-slot protection wording.
See [Production Readiness](production-readiness-plan.md) for release gates.

## Helping With Matrix Validation

Report version/artifact, OS, controller/transport, failed step, reconnect/restart
result and a sanitized support bundle via [Troubleshooting](troubleshooting.md#reporting-a-problem).
Keep raw captures and private identifiers out of public evidence.
