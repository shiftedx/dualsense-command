# GitHub Actions

| Workflow | Trigger | Required checks |
| --- | --- | --- |
| [CI](ci.yml) | Pull requests; pushes to `main` / `develop` | Documentation links, release metadata, Rust format/Clippy/tests on Ubuntu and Windows, complete web checks. |
| [Release](release.yml) | `vMAJOR.MINOR.PATCH[-prerelease]` tags | Version/tag/changelog agreement, redistribution permission, Rust and web checks before packaging. |

Linux runners install `libudev-dev`; Windows release builds use GNU Rust and
MinGW. Both use Node.js 24 for web checks. Playwright Chromium is installed in CI.

Release packaging reuses the checked `web/dist` artifact across Windows and
Linux. Every installer/archive includes license notices; archives include
checksums. Stable Windows artifacts require signing secrets; beta tags publish unsigned prereleases and do not become Latest.

| Artifact | Audience |
| --- | --- |
| Standard MSI | Default Windows installer. |
| Bridge MSI | Optional non-Steam Input Bridge; self-contained HIDMaestro broker. |
| Bridge Framework-Dependent MSI | Bridge testing with the matching x64 .NET runtime installed. |
| Windows binary archive | Diagnostics. |
| Linux archive | Beta; bundled UI and optional udev rule. |

See [Release Trust](../../docs/release-trust.md),
[Linux Beta](../../docs/linux-beta.md), and
[the current production audit](../../docs/production-readiness-plan.md).
A green build does not establish hardware, installer lifecycle, or asset rights.

## Local checks

```powershell
npm.cmd run check
npm.cmd run check:release
npm.cmd run inventory:licenses
node tools/check-release.mjs --tag v0.5.1 --distribution
```

`check` runs documentation, metadata, web, and Rust suites. Distribution checks
remain separate from development checks; every shipped asset must have a verified ledger entry.
The license inventory uses offline Cargo metadata plus the npm lockfile; fetch
locked dependencies first. It records declared licenses, not legal clearance.

Use the [contribution guide](../../docs/contributing.md) for setup and targeted
checks. Run `packaging\windows-installer-smoke.ps1` from a clean Windows account
or VM before releasing an MSI.
