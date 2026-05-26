# Windows Installer Smoke

Use this on a clean Windows test account or VM before publishing a Windows
release. The smoke path is intentionally per-user and uses the MSI path you pass
in; it does not hardcode developer machine paths.

## Script

The helper lives at `packaging/windows-installer-smoke.ps1`.

By default it only checks that the MSI exists, records its SHA256, and prints the
checklist:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File packaging\windows-installer-smoke.ps1 -MsiPath .\target\installer\DualSenseCommandCenter-<version>-standard.msi
```

Run the live smoke only on a disposable or clean test account:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File packaging\windows-installer-smoke.ps1 -BaselineMsiPath <previous-msi> -MsiPath .\target\installer\DualSenseCommandCenter-<version>-standard.msi -Execute
```

If there is no previous MSI handy, omit `-BaselineMsiPath`; the script installs
the current MSI and runs it again to exercise the WiX same-version upgrade path.

## What It Checks

- The baseline/current MSI files exist, are non-empty, and have SHA256 hashes.
- No existing DSCC install markers or `dscc-agent` / `dscc-tray` processes are
  present unless an explicit dirty-box override is passed.
- Install creates the expected per-user payload under
  `$env:LOCALAPPDATA\Programs\DualSense Command Center`.
- Start menu shortcuts are created under the current user's Start menu folder.
- The HKCU run-at-login value points at `dscc-tray.exe`.
- Fresh install and upgrade launch at most one `dscc-tray` and one `dscc-agent`
  process, and those processes run from the current install folder.
- Uninstall removes installer-owned payload, shortcuts, the run key, and leaves
  no DSCC processes behind.

MSI logs are written under a temporary `dscc-msi-smoke-*` directory unless
`-LogDirectory` is supplied.

## Release Checklist

1. Build or download the release MSI and verify its published SHA256. Use
   `standard` for the main smoke unless you are explicitly validating Bridge.
2. Run the non-mutating preflight command and confirm it points at the intended
   artifact.
3. Use a clean Windows test account or VM with no existing DSCC install.
4. Run the live smoke with `-Execute`.
5. Keep the generated install, upgrade, and uninstall logs with the release
   validation notes if anything fails.

## Installer Flavors

| Flavor | Smoke expectation |
| --- | --- |
| `standard` | No `hidmaestro` broker folder is installed. This is the default user path. |
| `bridge` | Installs the self-contained broker under `hidmaestro`. Larger payload. |
| `bridge-framework-dependent` | Installs only the framework-dependent broker files and requires the matching x64 .NET runtime. |
