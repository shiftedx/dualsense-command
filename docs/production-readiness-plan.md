# Production Readiness

Last updated: 2026-05-26

DSCC `0.3.3` is a public unsigned Windows beta. Testers can use it now. Do not
call it production-ready until the hardware matrix is complete.

## Current Status

- Windows x86_64 MSI is the main release path.
- DSCC Standard is the recommended Windows installer for most users.
- Bridge installers are opt-in for non-Steam DSCC Input Bridge testing.
- The MSI and binaries are unsigned. README and troubleshooting docs explain the
  SmartScreen warning.
- Linux artifacts are beta archives with bundled web UI assets.
- Updates are check-and-link only. DSCC does not auto-install updates.
- Profiles/settings are stored in the user config folder and are backed up
  before install/upgrade.

## Public Beta Messaging

Keep these points visible in README, troubleshooting, issue templates, and
support replies:

- The Windows installer is unsigned and may trigger SmartScreen.
- Standard is the default download. Bridge builds are larger compatibility
  options for non-Steam game testing.
- DSCC does not auto-install updates.
- Hardware claims should point to the Windows hardware matrix and say whether a
  controller/transport cell is verified or pending.
- DualSense Edge onboard sync only reports synced after HID acknowledgement and
  typed readback verification.
- Support requests should include a sanitized support bundle when possible.
- Setup questions, tuning ideas, and "is this expected?" reports belong in
  GitHub Discussions before becoming tracked bugs.

## Already In Place

- GitHub release workflow builds Standard, Bridge, and Bridge
  framework-dependent Windows MSIs, Windows raw binaries, Linux beta archives
  with `web/dist`, and SHA256 checksum files.
- Release packaging reuses the checked `web/dist` artifact instead of rebuilding
  the web UI separately for Windows and Linux.
- Release workflow runs Rust and web checks before packaging.
- Windows MSI install/upgrade/uninstall smoke guidance is tracked in
  [Windows Installer Smoke](windows-installer-smoke.md).
- GitHub Releases update checks exist and are link-only.
- Tray routes open dashboard, haptics, and button mapping.
- Tray dashboard opening is debounced.
- Trigger input polling only runs on the visible haptics view.
- Game trigger/rumble output requires supported-game detection, an active
  profile, and fresh telemetry.
- Supported-game detection may set the lightbar before telemetry.
- Manual tests are time-limited.
- Profile Save As, import, export, rename, delete, and activation exist.
- Point-based trigger curves and Forza brake/throttle tuning are implemented.
- Forza Data Out and Assetto Corsa Rally telemetry paths are live.
- DualSense Edge onboard slots can be read and written over guarded USB or
  Bluetooth HID feature-report paths, with default-slot protection and readback
  verification.
- Public Windows hardware matrix and release-candidate validation checklist are
  tracked in [Windows Hardware Matrix](hardware-matrix.md).
- A sanitized support bundle is available from the app Support panel and
  `dscc-cli support-bundle`.
- First-run onboarding is available in the app and can be reopened from the
  header Guide button after dismissal.

## Still Needed Before Calling It Production-Ready

- Complete the public Windows hardware matrix physical runs:
  - DualSense Edge USB current-release pass.
  - DualSense Edge Bluetooth current-release runtime/read/stage pass.
  - DualSense USB current-release pass.
  - DualSense Bluetooth current-release pass.
- Keep Linux marked beta until HID permissions and hardware smoke tests are
  documented across common Ubuntu setups.
- Keep the public [Linux Beta Guide](linux-beta.md) and Linux artifact README in
  sync whenever the archive layout, udev guidance, or launch command changes.
- Run clean-user install, upgrade, uninstall, and orphan-process smoke tests for
  each release.
- Improve beginner support docs and issue templates as feedback arrives.
- Keep public hardware claims backed by tracked docs or release notes.

## Release Checklist

Run this before public releases unless the change is docs-only:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace --all-features
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run test:source-audit
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
npm.cmd --prefix web run test:visual-smoke
```

Also confirm:

- GitHub CI is green.
- Web release-size budget is still passing and any large new assets are
  intentional.
- Version metadata matches the tag, crates, web package, root package, MSI, and
  changelog.
- README and troubleshooting docs mention that the MSI is unsigned.
- Standard, Bridge, Bridge framework-dependent, archives, and checksum files
  are uploaded.
- The final MSI installs, launches, upgrades, and uninstalls cleanly.
  Use `packaging\windows-installer-smoke.ps1` for the repeatable Windows smoke.
- Hardware-matrix entries used in release notes are marked Verified or listed
  as pending physical validation.

## Security Checklist

- API defaults to `127.0.0.1:43473`.
- Forza Data Out defaults to `127.0.0.1:5300`.
- LAN Access is off unless the user enables it.
- Direct `dscc-agent` non-loopback binding requires `DSCC_ENABLE_LAN_API=1`.
- Forza UDP non-loopback binding requires `DSCC_ENABLE_LAN_FORZA=1`.
- Cross-origin mutating HTTP requests and WebSocket upgrades are rejected.
- No raw HID-byte HTTP route exists.
- Controller output flows through typed frame/profile paths.
- Steam Input writes stay under guarded `controller_*.vdf` paths with backups.
- Forza glyph writes stay under trusted install roots with backups.

## No-Ship Conditions

Do not publish a beta if:

- A controller can remain tensioned after telemetry stops, the game exits, the
  route changes, or a manual test ends.
- Unsupported games or Global Profile can produce game trigger/rumble output.
- The packaged Games page is sluggish or unresponsive.
- The tray opens duplicate dashboard tabs or blocks while opening the menu.
- The MSI cannot install, upgrade, or uninstall cleanly.
- CI or local release gates fail.
- README or troubleshooting docs omit the unsigned-installer warning.
