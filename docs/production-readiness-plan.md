# Production Readiness Plan

Last updated: 2026-05-21

DualSense Command Center is close to a public beta, but it should not be called
broad production-ready until release, hardware, installer, and support gates are
repeatable. This plan tracks the remaining work needed to ship an unsigned
Windows beta safely and then move toward a production 1.0 release.

## Release Stance

- Current target: Windows x86_64 public beta.
- Windows distribution: unsigned MSI from GitHub Releases.
- Linux status: build-health and experimental runtime only until Linux HID
  permissions, runtime docs, and hardware smoke tests are complete.
- Update policy: manual update checks only. Do not silently download or replace
  unsigned binaries.
- Production claim: hold until the Windows hardware matrix, packaged installer
  smoke, update behavior, and support docs have passed.

## Signing Decision

The release can proceed unsigned, but the release notes and installer docs must
make that tradeoff explicit.

- The MSI and bundled binaries are unsigned unless `packaging/package-msi.ps1`
  is run with `-CertificatePath`.
- Unsigned public installers can trigger Windows SmartScreen or publisher
  warnings and may be blocked by managed enterprise environments.
- Publish SHA256 checksums beside every MSI and release archive.
- Publish artifacts only from GitHub Releases, with a tag, changelog, known
  issues, and matching version metadata.
- Avoid auto-update installation while unsigned. The app may check GitHub
  Releases and link users to the release page, but the user must manually choose
  whether to install.

Code signing options to revisit after the beta:

- Microsoft Store MSIX distribution can provide free Store signing, but it is a
  different distribution path than the current MSI.
- Azure Artifact Signing is Microsoft's managed non-Store signing path, but it
  is paid and still builds SmartScreen reputation over time.
- Traditional OV certificates are paid and also build SmartScreen reputation
  over time.
- SignPath's open-source sponsorship may be a free option if the project is
  accepted and the release flow is wired to their process.

References:

- Microsoft code signing options:
  https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- Microsoft SignTool:
  https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool
- SignPath open-source program:
  https://signpath.io/solutions/open-source-community
- GitHub release management:
  https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository

## Tester Feedback Coverage

| Feedback | Readiness handling |
| --- | --- |
| Controller details expansion is sluggish | Gate packaged UI performance. Trigger input polling must remain haptics-only, hidden tabs must stop polling, and `#/games` must stay responsive. |
| Tray task icon opens two tabs | Gate tray startup, existing-instance activation, and dashboard launch from Start menu, tray click, and tray menu. |
| Auto check app for updates | Keep GitHub Releases check on app start. Release docs must state this is a check/link, not an auto-installer. |
| Tray button mapping does not open mapping page | Gate tray quick links to dashboard, triggers/haptics, and button mapping. |
| Show active profile in tray | Gate tray snapshot cache, active controller/profile display, and stale/offline labels. |
| Mouse cursor spins over tray menu | Gate custom tray menu hover and cursor behavior on Windows. |
| App must not take controller until supported game or manual test | Gate hardware output policy and tests: no supported game plus no manual test means no hardware writes. |
| Triggers must not tension before telemetry | Gate stale/no-telemetry output and watchdog neutralization. |
| Paddle shift thump too weak | Game-feel tuning work before 1.0. Add a calibration pass for shift thump range and physical feel. |
| Throttle load feels too strong at low percent | Game-feel tuning work before 1.0. Revisit throttle response curve, minimum output, and default preset. |
| ABS start point and brake end-stop requests | Game-feel backlog. Treat as production polish unless repeated safety or stuck-output reports appear. |
| Save As for profiles | Product polish before 1.0. Add profile duplicate/save-as flow and tests. |

## Beta Ship Gates

### Release Branch Hygiene

- [ ] Release branch contains only intentional release changes.
- [ ] Dirty local artifacts are excluded: `target/`, `web/dist/`,
  `web/node_modules/`, release archives, private captures, and ignored handoff
  notes.
- [ ] Version metadata is consistent across Rust crates, web package metadata,
  installer version, changelog, and GitHub release tag.
- [x] Release automation produces checksums from the final uploaded artifacts, not
  intermediate staging files.

### Safety

- [ ] No raw HID-byte HTTP route exists.
- [ ] All controller output flows through `ControllerOutputFrame`.
- [ ] Hardware output is blocked unless a supported game profile has live
  telemetry or a manual effect test is active.
- [ ] Manual effect tests release or neutralize output when the test ends,
  the controller changes, the haptics page is left, or the app is hidden.
- [ ] Stale telemetry sends neutral output and does not keep last live tension.
- [ ] Unsupported game, no game, global profile, or missing active profile does
  not produce live trigger tension.
- [ ] `DSCC_DISABLE_HARDWARE_OUTPUT=1` and `DSCC_ENABLE_HARDWARE_OUTPUT=0`
  produce diagnostics-only dry runs in packaged builds.

### Build

- [ ] Rust format:
  `cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check`
- [ ] Rust workspace tests:
  `cargo +stable-x86_64-pc-windows-gnu test --workspace --target x86_64-pc-windows-gnu`
- [ ] Rust clippy:
  `cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings`
- [ ] Web typecheck:
  `npm.cmd --prefix web run typecheck`
- [ ] Web production build:
  `npm.cmd --prefix web run build`
- [ ] Button mapping p95 guard:
  `npm.cmd --prefix web run test:button-map`
- [ ] GitHub CI is green on Windows and Ubuntu.
- [x] Tag-triggered release workflow runs Rust and web checks before packaging.

### Security

- [ ] API defaults to `127.0.0.1:43473`.
- [ ] Forza Data Out defaults to `127.0.0.1:5300`.
- [ ] LAN API exposure still requires `DSCC_ENABLE_LAN_API=1`.
- [ ] LAN Forza UDP exposure still requires `DSCC_ENABLE_LAN_FORZA=1`.
- [ ] Cross-origin mutating HTTP requests are rejected.
- [ ] Cross-origin WebSocket upgrades are rejected.
- [ ] Steam Input writes stay under guarded `controller_*.vdf` paths, preserve
  backups, and reject unsafe roots.
- [ ] Forza glyph writes stay under trusted install roots, preserve backups, and
  refuse replacement when the original or backup is missing.

Cross-origin mutation smoke:

```powershell
curl.exe -i -X PUT http://127.0.0.1:43473/api/app-settings `
  -H "Host: 127.0.0.1:43473" `
  -H "Origin: http://evil.example" `
  -H "Content-Type: application/json" `
  --data "{\"listenOnAllInterfaces\":false}"
```

Expected: `403`.

### Packaging

- [ ] Build release binaries:
  `cargo +stable-x86_64-pc-windows-gnu build -p dscc-agent -p dscc-tray --release --target x86_64-pc-windows-gnu`
- [ ] Build unsigned MSI:
  `powershell -NoProfile -ExecutionPolicy Bypass -File packaging\package-msi.ps1 -Version 0.2.0 -TargetTriple x86_64-pc-windows-gnu`
- [x] Tag-triggered release workflow builds unsigned Windows beta artifacts
  without requiring signing secrets.
- [x] Linux release artifact is labeled experimental when published as raw
  binaries.
- [ ] Fresh install on a clean Windows user profile.
- [ ] Upgrade install over the previous beta build.
- [ ] Uninstall removes Start menu shortcuts, startup entry, and installed files.
- [ ] No orphaned agent or tray process remains after uninstall or quit.
- [ ] Installed tray serves the bundled `web/dist` UI from the local agent.
- [ ] MSI, agent, and tray hashes are recorded in release notes.

Installer smoke commands:

```powershell
$msi = Resolve-Path target\installer\DualSenseCommandCenter-0.2.0.msi
Get-FileHash $msi -Algorithm SHA256
Get-AuthenticodeSignature $msi
msiexec.exe /i $msi /qn /L*v "$env:TEMP\dscc-install.log"
curl.exe -fsS http://127.0.0.1:43473/api/status
curl.exe -fsS http://127.0.0.1:43473/api/diagnostics
Test-Path "$env:LOCALAPPDATA\Programs\DualSense Command Center\web\dist\index.html"
msiexec.exe /x $msi /qn /L*v "$env:TEMP\dscc-uninstall.log"
```

For unsigned beta builds, `Get-AuthenticodeSignature` returning `NotSigned` is
expected. Do not use verified-publisher install behavior as a beta acceptance
criterion.

### Tray

- [ ] Tray opens the dashboard once from Start menu launch.
- [ ] Tray opens the dashboard once from tray click.
- [ ] Tray menu links open the dashboard, haptics, and button mapping routes.
- [ ] Tray menu shows controller, agent, diagnostics, and active profile state.
- [ ] Tray health cache updates without blocking menu open.
- [ ] Cursor remains normal over every tray menu row.
- [ ] Stop/restart/quit only manage the DSCC-owned agent process.
- [ ] External agent mode hides or disables unsafe process controls.

### Packaged UI

- [ ] `#/games` controller details expansion is responsive.
- [ ] `#/games` does not start trigger `/input` polling.
- [ ] `#/button-mapping` does not start trigger `/input` polling.
- [ ] Haptics view starts trigger `/input` polling only while visible and active.
- [ ] Hidden tabs stop trigger polling.
- [ ] Base Feel, L2, R2, rumble, and lightbar tests stop cleanly after route
  change, controller change, or hidden tab.
- [ ] Update banner shows only when GitHub Releases has a newer version.
- [ ] Update check fails quietly when offline or rate limited.

### Hardware Matrix

Minimum Windows beta matrix:

- [ ] Windows 11 + DualSense USB.
- [ ] Windows 11 + DualSense Bluetooth.
- [ ] Windows 11 + DualSense Edge USB.
- [ ] Windows 11 + DualSense Edge Bluetooth.

For each combination:

- [ ] Enumerate and open through `hidapi` as the current user.
- [ ] Verify no raw HID path, serial, Bluetooth address, or full report payload
  appears in logs, docs, or API output.
- [ ] Connect, disconnect, reconnect, then verify neutral output.
- [ ] Verify behavior while Steam is running.
- [ ] Verify behavior with no supported game running.
- [ ] Verify supported game running without telemetry.
- [ ] Verify supported game with live Forza Data Out telemetry.
- [ ] Verify stale telemetry neutralization within the expected watchdog window.
- [ ] Verify manual effect tests apply only for their test duration.

Linux remains experimental until this additional matrix is captured:

- [ ] Linux + DualSense USB.
- [ ] Linux + DualSense Bluetooth.
- [ ] Linux + DualSense Edge USB.
- [ ] Linux + DualSense Edge Bluetooth.
- [ ] Document required udev rules or permissions.

### Documentation And Support

- [ ] README states platform scope, unsigned MSI behavior, and dry-run env vars.
- [ ] Release notes include install steps, checksum, known issues, and rollback.
- [ ] Troubleshooting covers SmartScreen, blocked HID access, Steam ownership,
  Forza Data Out setup, update-check failure, and app logs.
- [ ] Bug report template asks for app version, controller model, transport,
  Windows version, game, telemetry status, Steam status, and sanitized logs.
- [ ] Hardware validation notes are updated after every new hardware pass.
- [ ] Public hardware claims are backed by tracked release notes or tracked
  validation docs, not only ignored local notes.
- [ ] Asset and glyph redistribution notes are reviewed before publishing
  bundled assets in a public release.
- [ ] Provenance is updated before any new HID, telemetry, or protocol behavior
  depends on new external sources or experiments.

Support diagnostics smoke:

```powershell
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- paths --json
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- devices diagnose --json
$env:DSCC_DISABLE_HARDWARE_OUTPUT='1'
cargo +stable-x86_64-pc-windows-gnu run -p dscc-cli -- devices list-hid --experimental --probe-open --json
```

## Production 1.0 Gates

These are not required to cut the unsigned beta, but they should be closed
before calling the app production-ready.

- [ ] Decide whether real hardware output remains default-on or moves behind an
  explicit first-run enable toggle.
- [ ] Complete the Windows controller matrix and record results.
- [ ] Complete Linux runtime validation or remove Linux production claims.
- [ ] Add profile Save As / duplicate flow.
- [ ] Add a game-feel calibration pass for low-throttle tension, brake end-stop,
  ABS start, and shift thump range.
- [ ] Add a support bundle export with sanitized diagnostics.
- [ ] Add persistent rotating logs or make the in-memory log limitation explicit
  in support docs.
- [ ] Add release automation that builds artifacts from a clean tag.
- [ ] Add a checksum generation step for all release artifacts.
- [ ] Add installer upgrade/uninstall smoke to the release checklist.
- [ ] Decide on signing path: continue unsigned, apply for SignPath, use Azure
  Artifact Signing, buy an OV certificate, or switch to Store/MSIX.
- [ ] Move update checks into the agent, cache results, support beta/stable
  channels, and verify release metadata before offering an update.
- [ ] Continue Svelte extraction by moving dashboard/games/profile workflows
  into feature modules after haptics and button mapping.

## Release Procedure

1. Freeze feature work except safety, packaging, docs, and release blockers.
2. Update version metadata and changelog.
3. Run the beta build gates locally.
4. Confirm GitHub CI is green.
5. Build release binaries and unsigned MSI.
6. Generate SHA256 checksums.
7. Install, upgrade, uninstall, and tray-smoke the MSI on a clean Windows user
   profile.
8. Run the minimum Windows hardware matrix.
9. Publish a GitHub pre-release with MSI, checksums, changelog, known issues,
   and unsigned-install notes.
10. Send the release to private testers first.
11. Promote to public beta only after no safety, stuck-output, or install
    blocker reports remain open.

## No-Ship Conditions

Do not publish a new beta if any of these are true:

- A controller can remain tensioned after telemetry stops, the game exits, the
  route changes, or a manual test ends.
- Unsupported games or global profiles can produce real hardware output.
- Packaged `#/games` remains sluggish or unresponsive during normal use.
- The tray opens duplicate dashboard tabs or blocks while opening the menu.
- The MSI cannot install, upgrade, or uninstall cleanly on a clean Windows user
  profile.
- CI or local release gates fail.
- Release notes do not disclose that the MSI is unsigned.
