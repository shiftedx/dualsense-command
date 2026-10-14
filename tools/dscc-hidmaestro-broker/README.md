# DSCC HIDMaestro Broker

This broker is the production virtual-output provider boundary for DSCC Input
Bridge. The Rust agent talks to it over versioned JSON lines on stdio; the
broker loads `HIDMaestro.Core.dll` next to itself at runtime and creates an
Xbox 360 virtual controller for local non-Steam app profiles.

## Build

DSCC Standard does not bundle this broker. That is the recommended installer
for most users because Steam Input, controller tuning, profiles, haptics,
telemetry, and diagnostics do not need virtual output.

Bridge-capable installers are explicit:

- `Bridge`: bundles the self-contained broker for non-Steam DSCC Input Bridge
  testing. Larger, most convenient bridge package.
- `BridgeFrameworkDependent`: bundles the framework-dependent broker for users
  who already have the matching x64 .NET 10 Runtime. Smaller, advanced-user
  package.

When switching between publish flavors, remove the existing `publish` directory
first so stale framework-dependent marker files cannot be packaged into the
wrong MSI flavor.

### Default Self-Contained Publish

Use .NET 10 and provide the HIDMaestro core assembly from the approved
HIDMaestro release pinned in `.github/workflows/release.yml` and recorded in
the root `THIRD_PARTY_NOTICES.md`:

```powershell
dotnet publish tools/dscc-hidmaestro-broker `
  -c Release `
  -r win-x64 `
  --self-contained true `
  -p:PublishSingleFile=true `
  -p:EnableCompressionInSingleFile=true `
  -p:DebugType=None `
  -p:DebugSymbols=false `
  -p:HidMaestroCoreDll="C:\path\to\HIDMaestro.Core.dll"
```

The resulting publish directory must contain:

- `dscc-hidmaestro-broker.exe`
- `HIDMaestro.Core.dll`
- `THIRD_PARTY_NOTICES.txt`

Package the self-contained Bridge MSI with:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\package-msi.ps1 `
  -Version 0.3.2 `
  -TargetTriple x86_64-pc-windows-gnu `
  -SkipWebBuild `
  -InstallerFlavor Bridge
```

### Optional Framework-Dependent Publish

Use this flavor only when the installer audience understands the prerequisite:
the target machine must already have the x64 .NET 10 Runtime installed.

```powershell
dotnet publish tools/dscc-hidmaestro-broker `
  -c Release `
  -r win-x64 `
  --self-contained false `
  -p:PublishSingleFile=false `
  -p:DebugType=None `
  -p:DebugSymbols=false `
  -p:HidMaestroCoreDll="C:\path\to\HIDMaestro.Core.dll"
```

The resulting publish directory must contain:

- `dscc-hidmaestro-broker.exe`
- `dscc-hidmaestro-broker.dll`
- `dscc-hidmaestro-broker.deps.json`
- `dscc-hidmaestro-broker.runtimeconfig.json`
- `HIDMaestro.Core.dll`
- `THIRD_PARTY_NOTICES.txt`

Package the framework-dependent MSI with:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\package-msi.ps1 `
  -Version 0.3.2 `
  -TargetTriple x86_64-pc-windows-gnu `
  -SkipWebBuild `
  -InstallerFlavor BridgeFrameworkDependent
```

This writes `target\installer\DualSenseCommandCenter-0.3.2-bridge-framework-dependent.msi`
so it is not confused with the default self-contained artifact.

### Standard MSI

Package the slim installer without the broker:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\package-msi.ps1 `
  -Version 0.3.2 `
  -TargetTriple x86_64-pc-windows-gnu `
  -SkipWebBuild `
  -InstallerFlavor Standard
```

This writes `target\installer\DualSenseCommandCenter-0.3.2-standard.msi`.
Bridge status will report the provider as not installed until the user installs
a Bridge flavor or configures an external broker path.

### Validation

Before shipping either flavor:

```powershell
powershell -NoProfile -Command "$tokens=$null;$errors=$null;[System.Management.Automation.Language.Parser]::ParseFile('packaging\package-msi.ps1',[ref]$tokens,[ref]$errors) > $null; if ($errors.Count) { $errors | ForEach-Object { $_.Message }; exit 1 }"
```

For the framework-dependent flavor, install the matching x64 .NET 10 Runtime on
the target machine and smoke-test the broker protocol:

```powershell
@(
  '{"protocol":"dev.dscc.hidmaestro-broker.v1","id":1,"command":"hello"}',
  '{"protocol":"dev.dscc.hidmaestro-broker.v1","id":2,"command":"provider_status"}',
  '{"protocol":"dev.dscc.hidmaestro-broker.v1","id":3,"command":"shutdown"}'
) | tools\dscc-hidmaestro-broker\bin\Release\net10.0\win-x64\publish\dscc-hidmaestro-broker.exe
```

The `hello` and `shutdown` responses should succeed. A non-admin
`provider_status` response may report unavailable because HIDMaestro controller
creation requires administrator privileges.

## Runtime

DSCC discovers the broker next to the agent at `hidmaestro\dscc-hidmaestro-broker.exe`.
For local development, set:

```powershell
$env:DSCC_HIDMAESTRO_BROKER="C:\path\to\dscc-hidmaestro-broker.exe"
```

HIDMaestro controller creation requires administrator privileges. In a
non-elevated process the broker reports provider status as unavailable and
refuses session creation without attempting any driver or device mutation.

Update frames are compact and typed: `lx`, `ly`, `rx`, `ry`, `lt`, `rt`, and a
validated button bitmask. Nested gamepad-state payloads are intentionally not
accepted, which keeps the 8 ms bridge path small and avoids parallel protocol
shapes.
