# GitHub Actions

## CI

`ci.yml` runs on pushes to `main` / `develop` and on pull requests.

It checks:

- Rust format.
- Rust clippy.
- Rust workspace tests on Ubuntu and Windows.
- Web typecheck.
- Button-mapping p95 guard.
- Vite production build.
- Web release-size budget.

Linux runners install `libudev-dev` so `hidapi` can compile.

## Release

`release.yml` runs when a `v*` tag is pushed.

It builds:

- Unsigned Windows MSI.
- Windows raw binary zip.
- Linux beta archive with bundled web UI.
- SHA256 checksum files.

The release workflow publishes the GitHub Release and uploads artifacts. Windows
artifacts are unsigned unless signing is added later.

The release workflow builds `web/dist` once in `web-checks`, checks its size
budget, uploads it as `release-web-dist`, then reuses that artifact for Windows
and Linux packaging. Do not re-add package-job web builds unless packaging needs
platform-specific assets.

The Linux archive also carries the optional
`70-dualsense-command-center.rules` udev file and an artifact README. Keep those
in sync with [docs/linux-beta.md](../../docs/linux-beta.md).

After building or downloading a Windows MSI, run
`packaging\windows-installer-smoke.ps1` from a clean Windows test account or VM
for the install, upgrade, uninstall, and orphan-process smoke path.

## Local Checks

```powershell
npm.cmd --prefix web run check
```

On this Windows host, use the GNU Rust toolchain if running Rust commands by
hand:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
```
