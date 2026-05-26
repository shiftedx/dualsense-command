# DSCC HIDMaestro Broker

This broker is the production virtual-output provider boundary for DSCC Input
Bridge. The Rust agent talks to it over versioned JSON lines on stdio; the
broker loads `HIDMaestro.Core.dll` next to itself at runtime and creates an
Xbox 360 virtual controller for local non-Steam app profiles.

## Build

Use .NET 10 and provide the HIDMaestro core assembly from the approved
HIDMaestro release recorded in `PROVENANCE.md`:

```powershell
dotnet publish tools/dscc-hidmaestro-broker `
  -c Release `
  -r win-x64 `
  --self-contained true `
  -p:HidMaestroCoreDll="C:\path\to\HIDMaestro.Core.dll"
```

The resulting publish directory must contain:

- `dscc-hidmaestro-broker.exe`
- `HIDMaestro.Core.dll`

## Runtime

DSCC discovers the broker next to the agent at `hidmaestro\dscc-hidmaestro-broker.exe`.
For local development, set:

```powershell
$env:DSCC_HIDMAESTRO_BROKER="C:\path\to\dscc-hidmaestro-broker.exe"
```

HIDMaestro controller creation requires administrator privileges. In a
non-elevated process the broker reports provider status as unavailable and
refuses session creation without attempting any driver or device mutation.
