# Release readiness

Version: **0.5.1**. Audit: September 30, 2026.
Engineering checks pass; full production validation remains open.

```mermaid
flowchart LR
    Source[Reviewed source] --> Checks[Tests + notices + asset hashes]
    Checks --> Install[User installer release]
    Install --> Physical[Complete hardware + installer matrix]
    Physical --> Signed[Verified signed artifacts]
```

| Gate | Evidence | Next action |
| --- | --- | --- |
| Source | Atomic persistence; bounded broker/discovery; guarded shutdown; loopback security; concise task/agent guides | Run [checks](contributing.md#validation) on the merged version |
| Redistribution | Original SVG replacements; verified asset hashes; exact notices for 219 Rust/browser versions | Regenerate notices after lock changes; run the [distribution gate](release-trust.md) |
| Hardware | User confirmed Edge Bluetooth L2/R2, rumble, blue lightbar and neutralization on the 0.5.0 audit working tree; graceful exit took 10ms | Complete the exact-version [matrix](hardware-matrix.md): USB, reconnect, telemetry, session end and onboard settings |
| Standard MSI | Existing-account install, reinstall launch, shortcuts, uninstall, config retention and process cleanup pass **with startup disabled** | Verify startup, clean-account install and distinct-version upgrade using [Installer Smoke](windows-installer-smoke.md) |
| Startup | MSI reports a registry write that independent native reads cannot see; speculative helpers were removed | Resolve the discrepancy in a clean Windows account; leave startup unchecked meanwhile |
| Bridge | Dependency/runtime notices required; no local .NET SDK validation | Build both flavors in release CI; validate actual broker payloads and lifecycle |
| Signing | Signing policy implemented; no certificate configured | Configure [credentials](release-trust.md) and verify an actual signed artifact |
| Linux | Source/packaging implemented; native hardware evidence pending | Keep [beta](linux-beta.md) claims until native HID checks pass |

## Audit checks

- Rust: format, all-feature Clippy and 361 workspace tests pass; two manual tests ignored.
- Web: typecheck, source/snapshot/haptics checks, build, 12 visual cases and drag budget pass.
- Mapping p95: lookup 0.047ms / chips 0.039ms / parse 0.004ms. Drag: 16.7ms p95,
  13.7 mutations/move. Synthetic results do not measure native HID latency.
- Bundle: 784.2 KiB raw / 438.0 KiB gzip. npm and Cargo advisory scans report zero
  known vulnerabilities. Repeat scans before release.

Treat historical audit results separately from current CI. Publish validation
limits in release notes; compilation and mocks cannot close physical gates.
