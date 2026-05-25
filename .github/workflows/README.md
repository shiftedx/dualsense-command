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

Linux runners install `libudev-dev` so `hidapi` can compile.

## Release

`release.yml` runs when a `v*` tag is pushed.

It builds:

- Unsigned Windows MSI.
- Windows raw binary zip.
- Experimental Linux raw binary archive.
- SHA256 checksum files.

The release workflow publishes the GitHub Release and uploads artifacts. Windows
artifacts are unsigned unless signing is added later.

## Local Checks

```powershell
npm.cmd run check
```

On this Windows host, use the GNU Rust toolchain if running Rust commands by
hand:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
```
