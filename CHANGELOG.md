# DualSense Command Center 0.3.8

Release date: 2026-06-01

0.3.8 is a clutch-tuning fix release. It keeps the 0.3.7 shift behavior, but
makes the advanced controls easier to understand and easier to edit.

## Clutch Control Fixes

- `Clutch at %` is now `Clutch bite %`, which means the clutch press threshold.
- `With clutch` is now `Clean shift`.
- `No clutch` is now `Missed clutch`.
- New `Clutch unload %` control adjusts how much DSCC-generated drivetrain body
  rumble drops while the clutch is pressed.
- Number fields select their current value on focus, so users can type a new
  value without fighting the existing digit.

## Runtime Safety

- The clutch unload value is stored in the typed Forza shift tuning config.
- Saved 0.3.7 profiles load safely with the new default value.
- The backend clamps the new setting before use.
- A regression test now proves the clutch unload setting changes drivetrain body
  rumble.

## Thanks

- Thanks to **ゾアン・ファンイ** for testing the clutch build quickly and pointing out
  where the labels and number fields were getting in the way.

## Validation Gate

This release passed:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test -p dscc-agent
cargo +stable-x86_64-pc-windows-gnu clippy -p dscc-agent --all-targets --target x86_64-pc-windows-gnu -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:haptics-graph
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
npm.cmd --prefix web run test:source-audit
npm.cmd --prefix web run test:visual-smoke
```

# DualSense Command Center 0.3.7

Release date: 2026-06-01

0.3.7 adds clutch-aware shift feedback for racing telemetry profiles and keeps
the 0.3.6 USB output hotfix.

## Clutch-Aware Shift Feel

- DSCC now reads clutch telemetry when the game provides it.
- Paddle shift thump can tell smooth clutch-assisted shifts from harsher
  no-clutch shifts.
- Clutch-assisted shifts use a shorter, softer kick.
- No-clutch shifts use a stronger, longer kick so missed clutch timing feels
  more mechanical.
- DSCC still uses the plain shift thump when clutch telemetry is missing, so
  older telemetry paths do not feel broken.
- Thanks to **ゾアン・ファンイ** for the clutch idea and the clear driver-feel notes
  behind it.

## Advanced Tuning

- The Haptics advanced panel now exposes clutch mode, clutch threshold,
  with-clutch strength, with-clutch duration, no-clutch strength, and no-clutch
  duration.
- The settings are saved with the same typed profile config as the other Forza
  haptics controls.
- The mock telemetry path now includes clutch and numeric shift-pulse values for
  UI testing only.

## Runtime Behavior

- Pressing the clutch reduces DSCC-generated drivetrain body load, so the car
  feels more uncoupled during shifts.
- Shift pulse strength now flows through the same effect rules that drive R2 and
  body rumble.
- The 0.3.6 USB output report fix remains in this release.

## Validation Gate

This release passed:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:haptics-graph
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
```

# DualSense Command Center 0.3.6

Release date: 2026-05-31

0.3.6 is a focused hotfix for USB DualSense output writes and clearer setup
errors.

## USB Output Hotfix

- DSCC now sends the DualSense USB output report as 48 bytes, matching the
  Windows HID write accepted by affected controllers.
- DSCC keeps the Bluetooth output path unchanged.
- DSCC adds regression coverage so a valid 48-byte USB write is not treated as a
  short hardware write.

## Support Messages

- Failed hardware actions now show the useful agent message without dumping the
  full JSON response into the UI.
- Troubleshooting now covers stale browser/API tabs and the common "game is
  detected, but no UDP packets are arriving" setup state.

## Validation Gate

This release passed:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test -p dscc-device
cargo +stable-x86_64-pc-windows-gnu clippy -p dscc-device --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
```

# DualSense Command Center 0.3.5

Release date: 2026-05-30

0.3.5 tunes L2 braking, adds Jlu's redline blink request, and keeps live
controller visuals active only when the UI needs them.

## Brake, ABS, And Throttle Feel

- DSCC increases L2 brake resistance across the full pull, with a stronger
  end-range ramp for every profile.
- DSCC makes ABS feedback stronger in the Standard immersive tune.
- DSCC adds advanced throttle controls for baseline force, ramp force, end-stop
  force, overtravel guard, and response curve.
- DSCC adds advanced ABS controls for threshold, flutter strength, frequency, wall
  shape, brake-speed scaling, and body blend.
- DSCC adds advanced shift-thump controls for wall trigger point, kick frequency,
  wall zones, and body low/high motor blend.
- DSCC adds advanced rev-limiter controls for RPM threshold, buzz strength range,
  buzz frequency, wall trigger point, wall zones, ramp curve, and body blend.

## Redline Lighting

- DSCC replaces the continuous RPM lightbar with a blinking redline alert.
- DSCC blinks player LEDs at redline for a clearer shift cue.
- Thanks to **Jlu** for recommending the blinking RPM cue.

## UI Responsiveness

- Trigger curve live markers update on browser animation frames and stop outside
  the visible Haptics view or active base-feel test.
- Controllers live input coalesces stick, trigger, and button updates to the
  browser frame loop and ignores stale route/controller responses.
- The haptics graph parity guard keeps trigger visuals aligned with backend
  runtime math.

## Support And Release Trust

- The in-app **Support** panel now includes a direct GitHub repository link
  alongside the sanitized support-bundle copy/export actions.
- README and troubleshooting docs now point users to the same repo, issues,
  discussions, release, and support-bundle flow.
- The Standard MSI remains the recommended installer for most users. Bridge
  installers remain opt-in for non-Steam DSCC Input Bridge testing.
- The MSI remains unsigned. Verify downloads from GitHub Releases with the
  published SHA256 files.

## Validation Gate

This release passed:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:haptics-graph
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
npm.cmd --prefix web run test:source-audit
npm.cmd --prefix web run test:visual-smoke
```

# DualSense Command Center 0.3.4

Release date: 2026-05-27

0.3.4 fixes a Windows restart/shutdown blocker in the tray host.

## Windows Restart And Shutdown

- The tray now explicitly accepts Windows session-end requests so restart and
  shutdown can proceed without DSCC appearing in the blocking-app screen.
- On confirmed session end, DSCC removes its tray icon and stops the owned local
  agent quickly instead of waiting on UI, browser, or network cleanup.
- The tray popup handles the same session-end path, so an open right-click menu
  cannot hold the session open.

## Validation Gate

This release was cut after a clean run of:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
npm.cmd --prefix web run test:source-audit
npm.cmd --prefix web run test:visual-smoke
```

Windows users should choose the `standard` MSI unless they specifically need
DSCC Input Bridge testing for local non-Steam app profiles. The MSI remains
unsigned.

# DualSense Command Center 0.3.3

Release date: 2026-05-27

0.3.3 focuses on DualSense battery efficiency without reducing the current
haptics, adaptive trigger, or telemetry feature set.

## Battery And Output Efficiency

- Hardware output now suppresses redundant encoded reports and keeps the
  controller alive with the existing timed keepalive instead of rewriting the
  same state every frame.
- The suppression check compares the encoded report shape, so equivalent output
  is skipped even when upstream floating-point frame construction varies.
- Haptics and adaptive triggers remain retained: DSCC still writes immediately
  when the desired controller output changes.

## Controller Diagnostics

- The Controllers tab now shows power diagnostics for output cadence, write
  rate, suppressed reports, keepalive interval, and passthrough status.
- Support bundles include the same sanitized counters so testers can report
  battery and output behavior without exposing HID paths, serials, addresses, or
  raw report bytes.

## Validation Gate

This release was cut after a clean run of:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
npm.cmd --prefix web run test:source-audit
npm.cmd --prefix web run test:visual-smoke
```

Windows users should choose the `standard` MSI unless they specifically need
DSCC Input Bridge testing for local non-Steam app profiles. The MSI remains
unsigned.

# DualSense Command Center 0.3.2

Release date: 2026-05-27

0.3.2 tightens the repo, bridge path, and release checks. It keeps the public
API stable.

## Security And Privacy

- Source audit now checks tracked and local source files, including the
  HIDMaestro broker, for banned raw-HID, driver-payload, private-path, and
  leftover development surfaces.
- Sensitive mutation routes have broader same-origin regression coverage.
- Ignored local research, handoff, validation, build, broker, and web artifacts
  remain outside Git.

## Performance And Structure

- The HIDMaestro broker accepts only the compact typed update frame for bridge
  sessions. The nested state fallback was removed from the 8 ms bridge path.
- `dscc-agent` API handlers and effect runtime helpers are split into focused
  modules.
- Button Mapping and Haptics CSS now live in feature-owned files with no
  production `legacy` style bucket.
- App helpers for support bundles, game presentation, profile selection, and
  haptics presentation moved out of `App.svelte`.

## Contributor Workflow

- The game module guide now maps common tasks to the files contributors should
  edit when adding supported games, local-app profiles, Steam discovery, UDP
  parsers, shared-memory sources, or haptic defaults.
- Architecture docs now match the route, effect, game-detection, and web app
  module boundaries.
- CI and release checks include source audit, button-map p95, release-size
  budget, and Playwright visual smoke across app routes.

## Validation Gate

This release was cut after a clean run of:

```powershell
npm.cmd run check
cargo +stable-x86_64-pc-windows-gnu build -p dscc-agent -p dscc-tray -p dscc-cli --release --target x86_64-pc-windows-gnu
```

Windows users should choose the `standard` MSI unless they need DSCC Input
Bridge for local non-Steam app profiles. Verify downloads with the matching
SHA256 file.

# DualSense Command Center 0.3.1

Latest release notes are listed first. For install steps, start with the
[README](README.md).

Release date: 2026-05-26

0.3.1 splits Windows installers into clear, named choices so the normal user
path stays slim while non-Steam bridge testing remains available.

## Windows Installers

Pick one installer. Do not install Bridge unless you need non-Steam DSCC Input
Bridge testing.

| Installer | Intended user | Why it exists |
| --- | --- | --- |
| **DSCC Standard** | Most users | Smallest normal install. Includes profiles, haptics, telemetry, diagnostics, controller tuning, and Steam Input support. Does not bundle the HIDMaestro broker. |
| **DSCC Bridge** | Non-Steam bridge testers | Bundles the self-contained HIDMaestro broker so DSCC Input Bridge can create a virtual Xbox 360 controller for local non-Steam app profiles. |
| **DSCC Bridge Framework-Dependent** | Advanced bridge testers | Bundles the smaller framework-dependent broker and requires the matching x64 .NET runtime to already be installed. |

## Packaging

- Added explicit `Standard`, `Bridge`, and `BridgeFrameworkDependent` MSI
  flavors to `packaging/package-msi.ps1`.
- Standard packaging no longer requires or stages any `hidmaestro` broker
  payload.
- Bridge packaging still validates that the broker output shape matches the
  selected flavor, so a framework-dependent build cannot accidentally package a
  self-contained runtime shape.
- The tag release workflow now builds and uploads all three Windows MSI
  variants with flavor names in the artifact filenames.
- Release notes now tell users to choose Standard first and use Bridge only for
  compatibility beyond Steam.

## UI

- Moved the Controllers page tuning bar to the top of the page so Controllers,
  Adaptive Triggers & Haptics, and Button Mapping use the same command layout.
- Kept Profiles focused on game/profile scope selection, with controller
  targeting in the expanded tuning ribbon.

## Validation Gate

This release was cut after a clean run of:

```powershell
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
cargo +stable-x86_64-pc-windows-gnu build -p dscc-agent -p dscc-tray -p dscc-cli --release --target x86_64-pc-windows-gnu
```

## Install

Download the `standard` Windows x86_64 MSI unless you specifically need
non-Steam DSCC Input Bridge testing. Verify downloads with the included SHA256
checksum file.

# DualSense Command Center 0.3.0

Latest release notes are listed first. For install steps, start with the
[README](README.md).

Release date: 2026-05-25

0.3.0 adds a skippable quick-start guide, updates help copy across the app, and
ships the Linux telemetry/input fixes from the 0.2.9 issue report.

## App UI

- Added a compact first-run guide covering Profiles, telemetry safety, trigger
  testing, and support bundles.
- Added a **Guide** button in the header so users can reopen the quick start
  after dismissing it.
- Added tooltips for Profiles, Adaptive Triggers & Haptics, and Button Mapping.
- Added tooltips for Import, Export, Save As, Rename, Delete, Reset, and Save.
- Updated Edge onboard slot copy to describe USB and Bluetooth sync support.
- Changed empty Edge slot refresh text from "usb scan" to "controller scan."

## Linux Beta Reliability

- Linux process detection now reads full `ps` arguments and extracts Proton
  Windows `.exe` names from command lines.
- DSCC can select a supported game from live telemetry when process detection
  misses the Proton game process.
- Global profile overrides no longer block detected supported-game profiles.
- Trigger input reads now drain queued HID reports and use the newest trigger
  state, which reduces lag in Linux trigger tests.
- Support bundles now mark telemetry live from adapter state when packets are
  flowing.

## Release Artifacts

- Bumped Rust crate, web package, installer, README, docs, and issue-template
  metadata to `0.3.0`.
- The release workflow limits uploaded assets to the Windows MSI, Windows
  binaries zip, Linux archive, and checksum files.
- The update checker links to GitHub Releases. DSCC still does not install
  updates automatically.

## Validation Gate

This release was cut after a clean run of:

```powershell
npm.cmd --prefix web run check
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-gnu build -p dscc-agent -p dscc-tray -p dscc-cli --release --target x86_64-pc-windows-gnu
```

WSL Ubuntu also passed:

```bash
cargo test -p dscc-device -p dscc-agent --lib
cargo build -p dscc-agent -p dscc-cli --release
```

## Install

Download `DualSenseCommandCenter-v0.3.0-windows-x86_64-unsigned.msi` from the
Releases page and run it. Profiles and settings live in the user config folder
and are preserved during upgrades. Verify downloads with the included SHA256
checksum file.

# DualSense Command Center 0.2.9

Latest release notes are listed first. For install steps, start with the
[README](README.md).

Release date: 2026-05-25

0.2.9 adds DualSense Edge onboard Fn-slot profile sync over Bluetooth with
controller acknowledgement and readback verification. It also adds a Steam Input
paddle-shift helper for Forza-style keyboard-backed layouts and documents what
DSCC can store on the controller itself.

## DualSense Edge Onboard Profiles

- **Bluetooth onboard profile writes are now supported on Windows.** DSCC can
  read and write supported DualSense Edge Fn-slot settings over USB or
  Bluetooth when Windows exposes HID feature-report access.
- **Writes are verified before DSCC calls them synced.** Every onboard write now
  requires the controller control/ack report and a fresh typed slot readback
  that matches the requested profile.
- **The default Fn + Triangle profile remains protected.** DSCC only writes the
  assignable Fn + Square, Fn + Cross, and Fn + Circle slots.
- **Bluetooth uses the correct Edge control reports.** Clean-room hardware
  probing found that Bluetooth rejects the USB write IDs but accepts the
  selectorless `0x63..0x65` control reports for assignable slots.
- **Profile names are normalized to controller storage limits.** Edge onboard
  names are capped to the controller's 40 UTF-16 character storage shape so
  readback verification stays exact.
- **Runtime haptics are still runtime haptics.** Telemetry effects, custom
  Forza/Assetto trigger curves, lightbar/RPM behavior, and body thumps still
  require DSCC to be running; they are not stored in Edge onboard memory.

## Edge Paddle Shift Helper

- **Added a real Steam Input paddle preset API.** DSCC can write an Edge paddle
  shift preset into a detected Steam Input layout for supported game profiles.
- **Defaults target keyboard-backed shifting.** Back Left defaults to `Q` and
  Back Right defaults to `E`, with user-editable keys for layouts that need a
  different pair.
- **The preset only writes real Edge paddle bindings.** DSCC refuses synthetic
  placeholders and requires a real DualSense Edge Steam Input layout containing
  Back Left and Back Right entries.
- **Steam files stay guarded.** Writes remain limited to canonical
  `controller_*.vdf` files under Steam's trusted userdata tree, with backups and
  size/path checks intact.

## Docs And Support Clarity

- **README and troubleshooting now reflect the current app.** Setup and feature
  descriptions were tightened for less technical users, including clearer notes
  about unsigned Windows installs, local-only defaults, LAN opt-in, and update
  checks.
- **The Windows hardware matrix now calls out Edge onboard Bluetooth writes.**
  It distinguishes implemented support from full release-candidate matrix
  coverage so support claims stay honest.
- **Edge onboard limits are clearer.** DSCC can store controller button remaps,
  trigger deadzones, stick response presets, vibration intensity, trigger
  intensity, and profile names onboard; keyboard keys and telemetry haptics stay
  PC/runtime features.

## Validation Gate

This release was cut after a clean run of:

```powershell
npm.cmd --prefix web run check
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-gnu build -p dscc-agent -p dscc-tray -p dscc-cli --release --target x86_64-pc-windows-gnu
```

A bounded hardware validation was also run against a Bluetooth DualSense Edge:
DSCC read the user-approved Fn + Square slot, wrote the same typed profile back
over Bluetooth, received the control report, and re-read an exact typed match.

## Install

Download `DualSenseCommandCenter-v0.2.9-windows-x86_64-unsigned.msi` from the
Releases page and run it. The MSI is unsigned, so Windows SmartScreen may show a
publisher warning. Verify downloads with the included SHA256 checksum file.

# DualSense Command Center 0.2.8

Latest release notes are listed first. For install steps, start with the
[README](README.md).

Release date: 2026-05-23

This is a narrow LAN-access hotfix for 0.2.7. The previous release kept the agent's all-interface bind behind the `DSCC_ENABLE_LAN_API` safety gate, but the installed tray launcher did not pass that capability through when it started `dscc-agent.exe`. As a result, selecting **LAN Access** in the app failed with a 403 before the user could save the setting.

## LAN Access

- **Restored the in-app LAN Access workflow for installed builds.** The Windows tray now starts the agent with `DSCC_ENABLE_LAN_API=1`, which allows the app's own Web UI Location control to save the user's LAN opt-in.
- **LAN remains off by default.** This hotfix does not bind the API to the network automatically. The agent still starts on `127.0.0.1:43473` unless the persisted app setting says LAN Access is enabled.
- **The user-facing toggle remains the actual exposure control.** Selecting LAN Access persists `listenOnAllInterfaces=true`; after restart, the tray uses the saved setting to start the agent on `0.0.0.0:43473`.
- **Direct agent launches keep the explicit env gate.** Running `dscc-agent.exe` or `dscc-cli serve` outside the tray still requires `DSCC_ENABLE_LAN_API=1` before non-loopback binding is accepted.
- **Added tray coverage for the launch contract.** A Windows tray unit test now asserts the spawned agent process receives the LAN API capability env var.

## Validation gate

This hotfix was cut after a clean run of:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu test -p dscc-tray tray_
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

The local MSI was rebuilt and installed, then the running installed agent was checked on `127.0.0.1:43473`.

## Install

Download `DualSenseCommandCenter-v0.2.8-windows-x86_64-unsigned.msi` from the Releases page and run it. The MSI is unsigned, so Windows SmartScreen may show a publisher warning.

# DualSense Command Center 0.2.7

Release date: 2026-05-23

This release is a focused feel-and-polish update for the 0.2.6 trigger editor. It keeps the new granular curve system, fixes reports of missing default mappings, and retunes the Forza brake/throttle profile around a wider sustained adaptive-resistance zone instead of a short end-stop bump.

## Adaptive Trigger Tuning

- **Forza R2 starts lighter and stays smoother through normal throttle travel.** The baseline and normal throttle forces were reduced so small throttle inputs do not feel chunky while cruising, correcting the over-heavy low-end feel testers reported.
- **R2 now ramps hard from about 60-80% travel, then holds max adaptive resistance.** The throttle curve keeps the first half of the pull easy, builds rapidly through the upper-mid range, and then uses a sustained max-strength adaptive-resistance zone through the rest of travel so the end stop feels like a real wall instead of a momentary bump.
- **Forza L2 brake now gets a wider max-strength lock-warning range.** The default brake warning wall now begins around 72% trigger travel instead of around 82%, and a custom 90% brake endpoint begins the hard wall around 70%. That gives ABS/front-slip effects more physical headroom and makes the lock-warning range harder to accidentally push through.
- **Brake force now ramps into the hard wall instead of jumping straight to it.** L2 uses a short progressive overtravel ramp before the max-strength zone, then holds the high-force adaptive resistance through the rest of the pull unless higher-priority brake effects take over.
- **End stops now use sustained adaptive resistance instead of DualSense wall output.** The full-force brake and throttle regions are encoded as `AdaptiveResistance`, which better matches the heavy trigger feel seen when the controller is engaged without telemetry and avoids the old "small bump then soft travel" behavior.
- **The trigger preview matches the backend force model.** The frontend graph constants and tooltips were updated with the same L2 wall, L2 ramp, R2 wall, R2 ramp width, and R2 ramp curve used by the hardware output runtime.
- **Regression tests cover the new brake and throttle shape.** Backend tests assert the light throttle start, progressive R2 ramp, sustained R2 max zone, wider L2 max zone, and custom trigger endpoint behavior.

## Button Mapping Defaults

- **Missing button assignments now normalize to DSCC defaults.** Saved controller, profile, and Edge slot payloads that omit `buttons` now load with the expected Cross/Circle/Square/Triangle, shoulders, sticks, system buttons, and Edge back-button defaults instead of appearing unmapped.
- **The Button Mapping view now shows default Steam-style bindings when no layout entry exists.** This prevents first-load screens from looking empty while still making it clear when a user is looking at DSCC defaults rather than an actual Steam layout write target.
- **Synthetic defaults are guarded from writes.** The app will ask users to open or create a real Steam Input layout before saving a custom binding, instead of trying to write a generated placeholder binding back to Steam.

## Trigger Editor Polish

- **Curve control points are sharper and easier to grab.** The graph handles now render as CSS-positioned controls instead of stretched SVG circles, keeping the dots crisp on high-resolution displays and wide layouts.
- **Range sliders received a visual pass.** Trigger sliders now use thicker rounded tracks, polished thumbs, and clearer hover/focus states across the tuning surface.
- **Button mapping performance coverage includes default overlays.** The p95 guard now exercises the default-binding merge path so the fallback mappings stay cheap enough for the UI.

## Release Publishing

- **0.2.7 is published as the latest normal release.** The tag workflow now creates a published GitHub Release instead of leaving the generated artifacts as a draft prerelease, so the update checker and Releases page can see the build as the current latest version.
- **Windows artifacts are unsigned.** The release workflow still produces the MSI and raw Windows binaries with SHA256 checksum files.

## Validation gate

This release was cut after a clean run of:

```powershell
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
```

## Install

Download `DualSenseCommandCenter-v0.2.7-windows-x86_64-unsigned.msi` from the Releases page and run it. The MSI is unsigned, so Windows SmartScreen may show a publisher warning.

# DualSense Command Center 0.2.6

Release date: 2026-05-22

This release adds point-based adaptive trigger curves so users can build the same detailed L2/R2 response shapes that previously had to be hard-coded into the telemetry profiles.

## Adaptive Trigger Curve Editor

- **Added draggable curve control points for L2 and R2.** The Trigger Curves graph now exposes editable dots directly on the response line so users can shape where resistance comes in, how sharply it rises, and how wide the stiff end-stop range should feel.
- **Profiles now support 4-8 saved points per trigger.** Every trigger profile keeps locked 0% and 100% endpoints plus editable interior points; users can add points to the widest curve segment or remove the least dramatic interior point from the UI.
- **The curve slider still works as a fast reset tool.** Moving the existing Curve slider regenerates a smooth exponent-based curve, giving users a quick way back to a clean brake or throttle ramp before fine-tuning individual dots.
- **The graph remains aligned to telemetry runtime behavior.** Forza and Assetto telemetry scopes map the editable dots onto the same normal-response region used by the backend, then continue to show the fixed DSCC end-wall and throttle overtravel ramp after that region.
- **Custom points are saved, exported, imported, and applied live.** The frontend sends `l2CurvePoints` and `r2CurvePoints` with profile/config updates, and the backend persists them as part of the trigger configuration.

## Runtime And Compatibility

- **Hardware output now evaluates point curves natively.** `dscc-core` gained a `SignalPoints` value source, so the effect engine interpolates the saved point curve directly instead of approximating it with one exponent.
- **Base Feel and Test Actuation respect custom dots.** Manual trigger tests use the same saved point arrays, so tuning feedback from the test buttons matches the profile the controller will use during telemetry.
- **Older saved profiles keep their previous feel.** Profiles created before 0.2.6 that only contain `l2Curve` and `r2Curve` now generate matching point arrays on load instead of falling back to the built-in default dots.
- **Input is clamped and normalized on both sides.** The UI and backend keep endpoints locked, sort/dedupe points, enforce the 4-8 point range, and clamp inputs/outputs to 0-100 so malformed imports cannot produce invalid trigger output.

## Validation gate

This release was cut after a clean run of:

```powershell
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
```

## Install

Download `DualSenseCommandCenter-0.2.6.msi` from the Releases page and run it. The MSI is unsigned, so Windows SmartScreen may show a publisher warning.

# DualSense Command Center 0.2.5

Release date: 2026-05-22

An adaptive-trigger feel hotfix focused on the brake and throttle end-stop behavior in telemetry profiles.

## Adaptive Trigger Changes

- **End walls now persist through the remaining trigger travel.** DSCC's encoded DualSense wall output now resists from the configured wall position all the way to full trigger travel instead of creating a very short wall band that could feel like a small bump and then disappear.
- **L2 brake lock warning is harder to push through.** Forza and Assetto telemetry profiles now arm the high-force brake wall earlier when the L2 end point is high, giving ABS and front-slip cues more physical headroom before the user fully overtravels the trigger.
- **R2 throttle stays lighter through normal travel.** Normal throttle resistance has been reduced so small and mid-throttle inputs feel smoother and less chunky.
- **R2 throttle end-stop is much stronger.** The final throttle guard now uses a wider, steeper ramp into a stronger wall so the last part of travel is significantly harder to press and shift thumps retain punch even near full throttle.
- **Trigger curve preview remains honest.** The frontend graph constants and tooltips were updated to match the backend force model, including the earlier brake warning point and wider throttle end-stop ramp.

## Validation gate

This hotfix was cut after a clean run of:

```powershell
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
```

## Install

Download `DualSenseCommandCenter-0.2.5.msi` from the Releases page and run it. The MSI is unsigned, so Windows SmartScreen may show a publisher warning.

# DualSense Command Center 0.2.4

Release date: 2026-05-22

A narrow UI hotfix for the 0.2.3 tuning surface.

## Fixes

- **Fixed Body Source layout overlap at 1440p.** The telemetry routing panel now reserves a dedicated row for the Forza body-rumble source control, so the Native / DSCC toggle no longer compresses into the telemetry effect rows.
- **Fixed trigger curve visuals for telemetry profiles.** Forza and Assetto telemetry scopes now draw the same force model used by the backend runtime profile instead of showing a generic full-height exponent curve.
- **R2 throttle graph now shows the tuned end-stop behavior.** The curve stays light through normal throttle travel, then shows the overtravel ramp and hard stop near the backend's 95% guard.
- **Global trigger tuning still uses the base actuation preview.** The backend-runtime graph is only used for telemetry game scopes, keeping controller-only Global tuning simple and editable.

## Validation gate

This hotfix was cut after a clean run of:

```powershell
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

The 1440p browser check also confirmed the Body Source control and telemetry list no longer overlap.

## Install

Download `DualSenseCommandCenter-0.2.4.msi` from the Releases page and run it. The MSI is unsigned, so Windows SmartScreen may show a publisher warning.

# DualSense Command Center 0.2.3

Release date: 2026-05-22

A focused hardware-readiness release for DualSense Edge owners, Forza haptics,
and production build hygiene. DSCC can now read the controller's Fn slots over
USB, stage profile data when hardware sync is unavailable, and write supported
static profile settings back to the Edge without exposing raw HID-byte write
APIs.

## Highlights

- **DualSense Edge onboard memory is now a real DSCC surface.** The Games /
  Controller page now exposes the Edge Fn profiles, reads controller slots over
  USB, and shows whether each slot is synced from hardware, locally staged, or
  unavailable until a USB refresh.
- **Static Edge profiles can travel with the controller.** DSCC can write
  supported profile data to `Fn + Circle`, `Fn + Cross`, and `Fn + Square`,
  including trigger range/resistance settings, lightbar color and brightness,
  stick presets, and supported button remaps. Live telemetry effects still
  require DSCC to be running.
- **Forza body rumble now preserves the game's native feel by default.** The
  new body-rumble mode defaults to native passthrough, so Forza keeps its
  built-in engine and road feel while DSCC only adds short event cues such as
  shift and landing thumps. A DSCC full-control mode remains available for
  heavier custom tuning.
- **Production builds no longer activate mock data.** The browser mock harness
  is now dev-only: production ignores `?mock=1`, localStorage mock flags, and
  mock environment switches, and the production bundle does not include the
  fixture payload.
- **The release train is back on a single version.** Rust crates, the web app,
  the MSI packaging default, README install command, package lock metadata, and
  tray version assertions now agree on `0.2.3`.

## DualSense Edge Onboard Memory

- Added a typed Edge onboard profile model in `dscc-device` rather than passing
  loose JSON or raw bytes through the app.
- Added clean read/write helpers for Edge onboard profiles behind the existing
  validated device-output boundary.
- Added feature-report read/write support to the device transport trait,
  `hidapi` backend, and mock transport so the behavior can be tested without
  touching real controller memory.
- Added API endpoints:
  - `GET /api/controllers/:id/edge-profiles`
  - `PUT /api/controllers/:id/edge-profiles/:slot`
- Added persistent agent state for Edge slot data, including normalized staged
  slots and last-read hardware snapshots.
- Added a safe fallback when hardware sync is not available: users can still
  stage slot settings locally and see that the controller has not been written
  yet.
- Kept `Fn + Triangle` as the default/read-only slot. Assignable writes are
  limited to the user profile slots.
- Edge profile writes use the current DSCC profile as the source of truth for
  supported static controller settings; telemetry-only effects are deliberately
  not written to onboard memory.

## UX

- Added an **Edge Onboard Memory** panel to the Games / Controller page for
  DualSense Edge controllers.
- Added a USB refresh action for reading the controller's current onboard slot
  state.
- Added per-slot write actions with disabled/default-state behavior when a slot
  should not be written.
- Added status copy for synced hardware slots, locally staged slots, USB-only
  hardware reads/writes, and hardware-output-disabled staging.
- Added focused tooltips for the new Edge panel:
  - what the read action does and why USB matters
  - what each slot state means
  - what the write action includes and what still requires DSCC at runtime
- Wired the Forza body-rumble mode into the UI with clear Native / DSCC choices
  and explanatory help text.

## Safety

- Edge onboard hardware reads and writes require a DualSense Edge connected over
  USB. Bluetooth and Windows fallback controller entries only expose staged
  local state.
- Hardware profile sync still honors DSCC hardware-output mode. If hardware
  output is disabled, writes are staged locally instead of silently pretending
  controller memory changed.
- No raw HID-byte write route was added. Hardware writes continue to flow
  through validated `ControllerOutputFrame` and typed Edge profile paths.
- Manual/live telemetry haptics remain separate from onboard memory writes, so
  a saved Edge Fn profile will not imply DSCC telemetry effects work without the
  agent running.
- Release packaging still backs up persisted user state before install or
  upgrade, preserving existing profiles and controller settings outside the
  install folder.

## Reliability

- Mock API loading now happens through a dev-only dynamic import path instead
  of a production-reachable static bundle.
- `docs/contributing.md` now documents the mock harness as a Vite development
  tool only, with production builds explicitly ignoring mock toggles.
- The Edge onboard flow has route-level tests for visibility, staging, conflict
  paths, and hardware fallback behavior.
- Device-layer tests cover Edge profile round-tripping, rejecting writes to the
  default slot, feature-report transport plumbing, and output-manager hardware
  integration.
- The browser build was scanned for mock fixture strings after production
  build; none were present.

## API And Runtime

- Snapshot/controller DTOs now include Edge profile slot state for the selected
  controller.
- Profile update paths now preserve and normalize Forza `bodyRumbleMode` so
  native passthrough remains the default.
- The backend keeps Forza body rumble in native-passthrough mode unless the
  profile explicitly opts into DSCC full-control body rumble.
- Edge hardware reads/writes run through blocking-safe device-manager calls so
  the API runtime does not stall while the HID backend talks to the controller.
- The UI API client rejects Edge onboard profile read/write calls when the
  browser is running against the development mock API, avoiding a fake
  production dead end.

## Validation gate

This release was cut after a clean run of:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

Additional release-readiness details:

- 212 Rust tests passed across the workspace.
- `svelte-check` reported 0 errors and 0 warnings.
- Button mapping performance guard stayed well under budget:
  `lookup=0.049ms`, `chips=0.031ms`, `parse=0.003ms` at p95 over 300 samples.
- Local unsigned Windows MSI was built successfully:
  `DualSenseCommandCenter-0.2.3.msi`.
- Local MSI SHA256:
  `AF5B76BD3E47B41B9D8E4638590F7FFB0C676FCD492916945B50214C365C5E31`.
- GitHub CI for `main` completed successfully after the release commit.
- GitHub release workflow for tag `v0.2.3` completed successfully and uploaded
  Windows unsigned beta artifacts plus experimental Linux raw binaries to the
  draft release flow.

## Install

Download `DualSenseCommandCenter-0.2.3.msi` from the Releases page and run it.
Per-user install; tray + agent start automatically.

The MSI is unsigned, so Windows SmartScreen may show a publisher warning.
Existing DSCC profiles and controller settings are stored in the user's config
directory and are not overwritten by the install folder. During install or
upgrade, DSCC backs up existing persisted state to
`state.preinstall-0.2.3.json` when `state.json` exists.

# DualSense Command Center 0.2.0

A focused release built around a redesigned Games surface, a custom-game flow that pulls from the user's Steam library, and an overhauled tuning ribbon that exposes the active profile in one click.

## Highlights

- **Add any Steam game to DSCC.** New **+ Add a Game** flow scans your installed Steam library and lets you register games that don't have a built-in module. Each custom game gets its own per-game profile that auto-loads when the game's `.exe` launches.
- **Pick the launch `.exe` from a real directory browser.** When DSCC can't auto-detect a launcher (e.g. games that ship a deep `Binaries/Win64/Game-Shipping.exe`), the **Select…** dialog now lets you navigate the install folder and tick the right executables. Backed by a sandboxed `GET /api/games/steam-library/browse` endpoint that's locked to the game's own install root.
- **Live profile + scope switching from any view.** The tuning ribbon's Selected Scope and Active Profile chips are now dropdown buttons — switch between Global, any installed game, or any saved profile without leaving the page.
- **Forza Horizon now defaults to Immersive.** New installs and freshly detected Forza Horizon 5 / 6 sessions start on the richer Immersive preset instead of Base.

## UX

- New Profile Scope chip pattern (eyebrow label / accent value / descriptor) standardised across the tuning ribbon and the Global Profile card.
- App header replaced controller-info duplication with brand + tagline ("Adaptive triggers, haptics, and live telemetry — tuned locally.").
- Controller card on the Games tab is now expandable — **Show details** reveals family, full HID id, transport, battery, permission state, diagnostic status, and the full capability list.
- Controller name no longer compresses/truncates; long custom names wrap on word boundaries.
- Games tab redesigned: smaller controller column, 2-up game grid, taller capsule artwork (220 × 150), portrait capsules show without cropping.
- Custom `<InitialBadge>` SVG component replaced the plain letter placeholders — gradient-filled, console-bracket cornered, scope-tinted.
- Add Game modal: real Steam capsule art (loaded from the local Steam library cache via `/api/games/steam-art/:app_id/:kind`), search by name/appId/install folder, in-place fallback when an asset 404s.
- Global Profile chip's auto-load hint moved into a tooltip so the page stays uncluttered.
- Tuning Ribbon → top tab labelled "Profiles" (was "Games"); section heading simplified to "Games" under the Tuning Scope eyebrow.
- "Active Profile" cell in the tuning ribbon — clarifies what's currently driving the controller.
- Button Mapping: removed the misaligned focus-PNG overlay; the controller render is now a clean reference image with the row hover acting as the active cue.
- Button Mapping is now available in Global scope (degraded mode with a clear explainer when no Steam Input layout applies).

## Bug fixes

- **Save was lighting up when nothing had been edited.** Dirty-state baseline is now derived from the live editable config after profile load, so Save only enables on real divergence from the loaded preset.
- **Telemetry rows on the Haptics page were dimming for effects the user had enabled.** The disabled visual now follows the user's toggle, not the agent's adapter-bound activity state.
- **Ribbon dropdowns wouldn't open.** Menus were being clipped by ancestor `overflow: hidden`; switched to `position: fixed` with dynamic anchoring.
- Add Game modal artwork: stopped relying solely on Steam CDN URLs (many apps have no public capsule). Now reads local Steam librarycache first, CDN only as fallback, with an `onerror` letter fallback if both fail.

## Performance

- **Stopped a 25 Hz render storm.** Live trigger-input polling now only runs on the Adaptive Triggers & Haptics view (the only view that consumes those values). On other tabs it's fully stopped, removing ~25 wasted full-component re-renders per second.
- **InitialBadge SVG def churn fixed.** Gradient/highlight IDs are now generated once per component instance instead of on every render. Previously, ~22 badges on the Games tab were forcing SVG def re-links on every state change, making clicks elsewhere feel sticky.
- Button-mapping p95 budgets remain green (`lookup ≤ 8 ms`, `chips ≤ 8 ms`, `parse ≤ 2 ms` against 300 samples).

## API changes

- `GET /api/games/steam-library` — full list of installed Steam games with art, stats, and discovered `.exe` candidates.
- `POST /api/games/custom` — register a user-added game (optional `processNames` override).
- `DELETE /api/games/custom/:gameId` — remove a user-added game.
- `GET /api/games/steam-art/:app_id/:kind` — serves art straight from the local Steam library cache. Sandboxed: numeric app ids only.
- `GET /api/games/steam-library/browse?appId&path` — sandboxed directory listing for a Steam game's install folder. Canonicalises the resolved path and rejects anything that escapes the install root.
- Snapshot `supportedGames[]` entries now carry `supportLevel: 'telemetry' | 'custom'`; user games are merged in and surface with a `CUSTOM` badge.

## Agent

- New `UserGameConfig` persisted alongside the rest of agent state — survives restarts.
- Process detection extended to match user-game `.exe` names so the auto-load promise holds for custom games too.
- Forza Horizon 5 / 6 default profile flipped from Base → Immersive (Forza Motorsport stays on Base).

## Validation gate

The release was cut after a clean run of:

```
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run check   # typecheck + button-map p95 + production build
```

122 tests in `dscc-agent`, 0 svelte-check warnings across 3760 files.

## Install

Download `DualSenseCommandCenter-0.2.0.msi` from the Releases page and run it. Per-user install; tray + agent start automatically.

The MSI is unsigned — Windows SmartScreen may show a publisher warning.
