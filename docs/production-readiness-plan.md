# Production Readiness Plan

Last updated: 2026-05-24

DualSense Command Center `0.2.8` is an unsigned Windows x86_64 beta published
through GitHub Releases. It is usable by testers, but it should not be described
as broadly production-ready until the hardware matrix, installer smoke tests,
support docs, and signing/update decisions are repeatable.

## Release Stance

- Current public release: `0.2.8` Windows x86_64 beta.
- Windows distribution: unsigned per-user MSI from GitHub Releases.
- Linux status: experimental raw binaries only.
- Update policy: DSCC may check GitHub Releases and link the user to the latest
  release, but it must not silently download or install unsigned binaries.
- Production claim: hold until Windows hardware coverage, installer
  install/upgrade/uninstall smoke, support workflows, and public validation docs
  are stronger.

## Signing Decision

Current `0.2.8` artifacts are unsigned. Release notes and installer docs must
continue to make that tradeoff clear.

- The MSI and bundled binaries are unsigned unless `packaging/package-msi.ps1`
  is run with `-CertificatePath`.
- Unsigned public installers can trigger Windows SmartScreen or publisher
  warnings and may be blocked by managed enterprise environments.
- Publish SHA256 checksums beside every MSI and release archive.
- Publish artifacts only from GitHub Releases, with a tag, changelog, known
  issues, and matching version metadata.
- Avoid auto-update installation while unsigned.

Signing options to revisit:

- Microsoft Store MSIX distribution.
- Azure Artifact Signing.
- Traditional OV certificate.
- SignPath open-source sponsorship, if accepted.

## Completed Since The First Beta Plan

- Tag-triggered release workflow runs Rust and web checks before packaging.
- Release workflow builds unsigned Windows MSI artifacts, Windows raw binaries,
  experimental Linux raw binaries, and SHA256 checksum files.
- Release workflow now publishes the GitHub Release instead of leaving artifacts
  in a draft-only state.
- Installer backs up persisted `state.json` before install or upgrade.
- GitHub Releases update checks exist and remain link-only.
- Tray dashboard opening is debounced, tray quick links route to the dashboard,
  haptics, and button-mapping pages, and the menu shows agent/controller/profile
  health without blocking normal use.
- Installed tray launches the agent with LAN API capability, while the user-facing
  LAN Access toggle remains the actual exposure control.
- Controller details expansion and trigger-input polling were tightened so
  `#/games` and `#/button-mapping` do not poll trigger input.
- Trigger input polling now runs only on the visible haptics view.
- Hardware output is gated: game trigger/rumble output requires a supported game,
  an active profile, and fresh telemetry; manual tests are time-limited.
- Supported-game detection may emit lightbar-only output before telemetry, but
  triggers and rumble remain neutral until live data arrives.
- Profile Save As, import, export, rename, delete, and activation flows exist.
- Point-based trigger curves, backend-aligned previews, default button mappings,
  and the recent Forza brake/throttle feel tuning are implemented.
- Forza Data Out and Assetto Corsa Rally telemetry paths are integrated.
- DualSense Edge onboard slots can be read over USB and can stage or write
  supported static profile data through guarded typed paths.

## Remaining Production Work

- Decide whether unsigned releases remain acceptable or pursue a signing path.
- Complete and publish the Windows hardware matrix:
  - Windows 11 + DualSense USB.
  - Windows 11 + DualSense Bluetooth.
  - Windows 11 + DualSense Edge USB.
  - Windows 11 + DualSense Edge Bluetooth.
- Keep Linux labeled experimental until USB/Bluetooth permissions, docs, and
  hardware smoke tests are captured.
- Run clean-user install, upgrade, uninstall, and orphan-process MSI smoke for
  each public release.
- Add beginner-facing troubleshooting for SmartScreen, blocked HID access, Steam
  ownership, Forza Data Out setup, LAN Access, app logs, and rollback.
- Add a bug report template that asks for app version, controller model,
  transport, Windows version, game, telemetry state, Steam state, and sanitized
  logs.
- Add a support bundle export or clearly document the current diagnostic/log
  limitations.
- Keep public hardware claims backed by tracked release notes or tracked
  validation docs, not only ignored local lab notes.
- Continue Svelte extraction after the haptics and button-mapping work so the
  app shell stays easy to reason about.

## Release Gates

Run these before every public release unless the release is explicitly scoped to
docs only.

### Version And Branch Hygiene

- Release branch contains only intentional release changes.
- Dirty local artifacts are excluded: `target/`, `web/dist/`,
  `web/node_modules/`, release archives, private captures, and ignored handoff
  notes.
- Version metadata is consistent across Rust crates, web package metadata, root
  package metadata, installer version, changelog, and GitHub release tag.
- Local agent instruction files, private planning docs, raw captures, and local provenance drafts
  are not committed unless intentionally moved into a public tracked doc.

### Build

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

Also confirm GitHub CI is green on Windows and Ubuntu.

### Security

- API defaults to `127.0.0.1:43473`.
- Forza Data Out defaults to `127.0.0.1:5300`.
- LAN API exposure is disabled unless the user enables LAN Access. Installed tray
  builds may pass `DSCC_ENABLE_LAN_API=1` to grant the setting permission, but
  persisted `listenOnAllInterfaces` still controls whether the agent binds to
  `0.0.0.0`.
- Direct `dscc-agent` non-loopback binding still requires `DSCC_ENABLE_LAN_API=1`.
- Forza UDP non-loopback binding requires `DSCC_ENABLE_LAN_FORZA=1`.
- Cross-origin mutating HTTP requests and WebSocket upgrades are rejected.
- No raw HID-byte HTTP route exists.
- All controller output flows through typed frame/profile paths.
- Steam Input writes stay under guarded `controller_*.vdf` paths, preserve
  backups, and reject unsafe roots.
- Forza glyph writes stay under trusted install roots, preserve backups, and
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

```powershell
cargo +stable-x86_64-pc-windows-gnu build -p dscc-agent -p dscc-tray --release --target x86_64-pc-windows-gnu
powershell -NoProfile -ExecutionPolicy Bypass -File packaging\package-msi.ps1 -Version <version> -TargetTriple x86_64-pc-windows-gnu
```

Installer smoke:

```powershell
$msi = Resolve-Path target\installer\DualSenseCommandCenter-<version>.msi
Get-FileHash $msi -Algorithm SHA256
Get-AuthenticodeSignature $msi
msiexec.exe /i $msi /qn /L*v "$env:TEMP\dscc-install.log"
curl.exe -fsS http://127.0.0.1:43473/api/status
curl.exe -fsS http://127.0.0.1:43473/api/diagnostics
Test-Path "$env:LOCALAPPDATA\Programs\DualSense Command Center\web\dist\index.html"
msiexec.exe /x $msi /qn /L*v "$env:TEMP\dscc-uninstall.log"
```

For unsigned beta builds, `Get-AuthenticodeSignature` returning `NotSigned` is
expected.

### Packaged UI And Hardware Smoke

- Tray opens the dashboard once from Start menu launch, tray click, and tray
  menu actions.
- Tray menu links open dashboard, haptics, and button mapping routes.
- Tray stop/restart/quit controls only manage the DSCC-owned agent process.
- `#/games` controller details expansion is responsive.
- `#/games` and `#/button-mapping` do not start trigger `/input` polling.
- Haptics view starts trigger `/input` polling only while visible and active.
- Base Feel, L2, R2, rumble, and lightbar tests stop after route change,
  controller change, hidden tab, or test completion.
- Supported game without telemetry keeps triggers and rumble neutral.
- Supported game with telemetry drives the expected profile.
- Stale telemetry neutralizes within the watchdog window.
- No raw HID path, serial, Bluetooth address, or full report payload appears in
  logs, docs, or API output.

## Release Procedure

1. Freeze feature work except safety, packaging, docs, and release blockers.
2. Update version metadata, changelog, and README release status.
3. Run local build/security gates.
4. Confirm GitHub CI is green.
5. Build and install the MSI locally for smoke testing.
6. Tag the release and let the release workflow build final artifacts.
7. Verify the GitHub Release contains the MSI, raw binaries, checksum files,
   changelog, known issues, and unsigned-install notes.
8. Install, upgrade, uninstall, and tray-smoke the final MSI.
9. Run the available hardware matrix and record results.
10. Keep the release public only if no safety, stuck-output, install, or update
    discovery blocker remains open.

## No-Ship Conditions

Do not publish a new beta if any of these are true:

- A controller can remain tensioned after telemetry stops, the game exits, the
  route changes, or a manual test ends.
- Unsupported games or global profiles can produce game trigger/rumble output.
- Packaged `#/games` is sluggish or unresponsive during normal use.
- The tray opens duplicate dashboard tabs or blocks while opening the menu.
- The MSI cannot install, upgrade, or uninstall cleanly on a clean Windows user
  profile.
- CI or local release gates fail.
- Release notes do not disclose that the MSI is unsigned.
