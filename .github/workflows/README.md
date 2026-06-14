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

- Unsigned Windows Standard MSI.
- Unsigned Windows Bridge MSI.
- Unsigned Windows Bridge framework-dependent MSI.
- Windows raw binary zip.
- Linux beta archive with bundled web UI.
- SHA256 checksum files.

Windows installer intent is deliberately explicit:

| Artifact | Default audience |
| --- | --- |
| `standard` | Most users. Controller tuning, profiles, haptics, telemetry, diagnostics, and Steam Input support without the non-Steam bridge broker payload. |
| `bridge` | Users testing DSCC Input Bridge for non-Steam games. Bundles the self-contained HIDMaestro broker. |
| `bridge-framework-dependent` | Advanced bridge users with the matching x64 .NET runtime already installed. |

Bridge installers include HIDMaestro under the MIT License and install the
notice beside the broker at `hidmaestro\THIRD_PARTY_NOTICES.txt`.

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
