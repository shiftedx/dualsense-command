# Release Trust

DSCC publishes unsigned Windows beta installers. The project does not currently
sign MSIs because there is no paid code-signing certificate.

## What We Do Instead

- Build releases in GitHub Actions from a tag.
- Publish SHA256 checksum files next to every artifact.
- Keep Standard, Bridge, and Bridge Framework-Dependent installer flavors
  separate.
- Assert that Standard builds do not bundle the HIDMaestro broker.
- Pin the external HIDMaestro release URL and SHA256 used for Bridge builds.
- Keep release artifacts, signing keys, private captures, and installer
  intermediates out of Git.

## What Users Should Download

- Use `standard` unless you need DSCC Input Bridge for local non-Steam apps.
- Use `bridge` only for non-Steam bridge testing and expect a larger download.
- Use `bridge-framework-dependent` only when the matching x64 .NET runtime is
  already installed.

## Verify A Download

```powershell
Get-FileHash .\DualSenseCommandCenter-v0.3.2-windows-x86_64-standard-unsigned.msi -Algorithm SHA256
Get-Content .\SHA256SUMS-windows.txt
```

The hash from `Get-FileHash` must match the entry in the checksum file.

## Not Yet Provided

- Code-signed MSI files.
- Hardware-backed release attestations.
- A signed updater.

Do not add auto-update install behavior until release signing and rollback
rules are in place.

