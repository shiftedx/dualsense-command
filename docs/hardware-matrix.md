# Windows Hardware Matrix

Last updated: 2026-05-25

This page is the public source for DSCC controller support claims on Windows.
It separates implemented support from physical validation evidence so release
notes, issues, and README claims stay honest.

## Support Language

- **Supported** means DSCC intentionally supports this controller/transport on
  Windows for normal app use.
- **Verified** means a physical controller has passed the checklist below on the
  current release candidate or a recent build with equivalent hardware paths.
- **Pending physical pass** means the app path exists, but this public matrix
  still needs a release-candidate hardware run.
- **Edge onboard profiles** means DualSense Edge Fn-slot settings stored on the
  controller. DSCC reads slots over USB or Bluetooth when the host exposes HID
  feature-report access. Controller-memory sync is supported over USB and
  Bluetooth, with default-slot protection and readback verification.

## Current Windows Matrix

| Controller | USB | Bluetooth | Public claim |
| --- | --- | --- | --- |
| DualSense | Supported; pending physical pass | Supported; pending physical pass | Supported on Windows, with final release-candidate validation still needed |
| DualSense Edge | Supported; pending physical pass for current release candidate | Supported for runtime profiles and onboard-slot read/write; verified with a bounded Bluetooth onboard no-op write on 2026-05-25, current release-candidate full-matrix re-test recommended | Fully supported on Windows for DSCC runtime profiles, adaptive triggers, haptics, lightbar, diagnostics, supported-game telemetry, and typed Edge onboard profile sync over USB or Bluetooth. |

## Helping With Matrix Validation

If you are reporting a controller support issue, please include:

- DSCC version and installer/archive file name if you know it.
- Controller model and whether you used USB or Bluetooth.
- The checklist step number that failed.
- Whether the result changed after reconnecting the controller and restarting
  DSCC.
- A sanitized support bundle from the DSCC Support panel when possible.

Do not include raw HID paths, serial numbers, Bluetooth addresses, or private
Steam account paths.

## Evidence So Far

- DualSense Edge Bluetooth has sanitized Windows validation covering
  enumeration, open permission, battery/config reporting, onboard slot reads,
  profile resolution, adaptive trigger output, lightbar output, rumble output,
  manual effect tests, and a bounded Fn + Square no-op onboard write with typed
  readback match.
- DualSense Edge onboard profile support is implemented through typed, guarded
  profile paths. The default Fn profile is protected from overwrite, assignable
  slots use USB/Bluetooth HID feature reports when available, and encode/decode
  behavior is covered by Rust tests.
- Standard DualSense runtime support shares the same typed output, safety gate,
  trigger input, lightbar, rumble, and telemetry profile paths, but it still
  needs a public physical matrix pass before release notes should imply the same
  evidence level as Edge.

## Validation Checklist

Run this checklist for each matrix cell before marking it **Verified**.

1. Install the current MSI on a clean Windows user profile or a clean test VM.
2. Launch DSCC from the Start menu and confirm the tray icon appears.
3. Connect the controller using the target transport.
4. Confirm the controller appears in the web UI with the correct family,
   transport, battery state when available, and no raw HID path or serial shown.
5. Open the haptics view and run L2/R2 manual trigger preview.
6. Confirm manual preview starts quickly, feels responsive, and returns to
   neutral when the test ends.
7. Preview lightbar color and confirm the controller updates without API errors.
8. With no supported game running, confirm DSCC stays on Global Profile and does
   not apply game telemetry trigger/rumble output.
9. Start a supported game and confirm game detection selects the expected game
   profile.
10. Confirm the lightbar updates when the game is detected.
11. Confirm telemetry becomes live and adaptive trigger/body haptic effects are
    felt during driving.
12. Stop telemetry or exit the game and confirm triggers/rumble neutralize within
    the stale telemetry window.
13. Disconnect and reconnect the controller, then confirm DSCC recovers without
    requiring a full reinstall.
14. Copy or export a support bundle and confirm it is sanitized.

## DualSense Edge Onboard Extras

Run these additional checks for DualSense Edge over USB and Bluetooth:

1. Open the Edge onboard profile UI.
2. Read all available Fn slots.
3. Confirm the default Fn + Triangle profile cannot be overwritten.
4. Write a simple assignable-slot test profile with a safe name and identity
   button mapping.
5. Re-read the slot and confirm the supported static settings match.
6. Confirm the UI reports hardware-synced only after acknowledgement and
   readback match.

## Production-Ready Gate

DualSense Edge can be described as fully hardware-verified on Windows when:

- Edge USB passes the current release-candidate checklist.
- Edge Bluetooth passes a current release-candidate runtime, onboard-read, and
  non-default onboard-write re-test.
- Release notes clearly state that Edge onboard profile sync is guarded over
  USB and Bluetooth and that default Fn + Triangle cannot be overwritten.
- Any failed checklist item has either a fix, a known limitation, or a linked
  issue before publishing.

Standard DualSense should remain listed as supported, but not as equally
hardware-verified, until its USB and Bluetooth cells complete this same matrix.
