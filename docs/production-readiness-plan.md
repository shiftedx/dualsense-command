# Production Readiness

Last updated: 2026-05-24

DSCC `0.2.8` is a public unsigned Windows beta. It is ready for testers, but it
should not be called broadly production-ready yet.

## Current Status

- Windows x86_64 MSI is the main release path.
- The MSI and binaries are unsigned.
- Linux artifacts are experimental raw binaries.
- Updates are check-and-link only. DSCC does not auto-install updates.
- Profiles/settings are stored in the user config folder and are backed up
  before install/upgrade.

## Already In Place

- GitHub release workflow builds Windows MSI, Windows raw binaries, Linux raw
  binaries, and SHA256 checksum files.
- Release workflow runs Rust and web checks before packaging.
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
- DualSense Edge onboard slots can be read/written over guarded USB paths when
  supported; otherwise changes are staged locally.

## Still Needed Before Calling It Production-Ready

- Decide whether to keep shipping unsigned or pursue signing.
- Complete and publish the Windows hardware matrix:
  - DualSense USB.
  - DualSense Bluetooth.
  - DualSense Edge USB.
  - DualSense Edge Bluetooth.
- Keep Linux marked experimental until Linux HID permissions and hardware smoke
  tests are documented.
- Run clean-user install, upgrade, uninstall, and orphan-process smoke tests for
  each release.
- Improve beginner support docs and issue templates as feedback arrives.
- Add a support bundle or clearly document current log/diagnostic limits.
- Keep public hardware claims backed by tracked docs or release notes.

## Release Checklist

Run this before public releases unless the change is docs-only:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

Also confirm:

- GitHub CI is green.
- Version metadata matches the tag, crates, web package, root package, MSI, and
  changelog.
- Release notes mention that the MSI is unsigned.
- MSI, archives, and checksum files are uploaded.
- The final MSI installs, launches, upgrades, and uninstalls cleanly.

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
- Release notes do not disclose the unsigned installer.
