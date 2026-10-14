# DualSense Command Center

DualSense Command Center is the product context for configuring PlayStation
DualSense and DualSense Edge behavior from a local app. Its language centers on
controllers, profiles, game telemetry, safe hardware output, and clean-room game
support.

## Language

### Controllers

**Controller**:
A PlayStation DualSense, DualSense Edge, or compatible Sony gamepad known to
DSCC.
_Avoid_: Device, HID device, gamepad when referring to the user-facing object

**Controller Family**:
The supported controller class, such as DualSense or DualSense Edge.
_Avoid_: Model string, product name

**Controller Transport**:
The connection path between the controller and host, usually USB or Bluetooth.
_Avoid_: Bus, backend

**Target Controller**:
The controller currently selected for profile resolution, haptics, mapping, and
configuration.
_Avoid_: Active device, selected device

**Controller Alias**:
A user-editable display name for a controller.
_Avoid_: Controller id, identity

### Profiles And Effects

**DSCC Software Profile**:
A DSCC-owned profile that describes controller tuning, game tuning, and runtime
effects.
_Avoid_: Onboard profile, controller memory profile

**Global Profile**:
The controller-only tuning scope used when no supported game profile is
selected.
_Avoid_: Default game profile, fallback game

**Game Profile**:
A DSCC software profile selected for a supported game.
_Avoid_: Integration profile, telemetry profile

**Manual Override**:
A troubleshooting choice that forces profile resolution to use a specific
profile.
_Avoid_: Active profile when the forced nature matters

**Profile Resolution**:
The process of choosing the effective profile from controller, game, telemetry,
and manual override context.
_Avoid_: Profile selection when explaining why a profile is active

**Edge Onboard Profile**:
A DualSense Edge profile stored in the controller's own profile slots.
_Avoid_: DSCC software profile, runtime profile

**Edge Onboard Slot**:
One of the DualSense Edge shortcut slots associated with Fn plus an action
button.
_Avoid_: Profile index, memory bank

**Runtime Live Effect**:
A temporary haptic, light, LED, or rumble output produced while DSCC is running.
_Avoid_: Onboard setting, saved profile

**Hardware Output**:
Actual controller writes that can change adaptive triggers, lights, player LEDs,
or rumble.
_Avoid_: HID write when discussing product behavior

**Dry-Run Output**:
Validated controller output that is exercised without writing to real hardware.
_Avoid_: Mock profile, disabled output

### Games And Telemetry

**Supported Game**:
A game DSCC knows how to detect, present, and pair with profiles or telemetry.
_Avoid_: Integration when the game identity is the point

**Game Module**:
The metadata that names a supported game, describes how to detect it, and links
it to profiles and adapters.
_Avoid_: Adapter, parser

**Telemetry Adapter**:
The source-specific reader that turns game telemetry into DSCC signals.
_Avoid_: Game module, integration when parser behavior is the point

**Telemetry Source**:
A live origin of telemetry data, such as UDP packets or shared memory.
_Avoid_: Game when the data transport is the point

**Normalized Signal**:
A named game or controller value expressed in DSCC's shared signal vocabulary.
_Avoid_: Packet field, raw value

**Fresh Telemetry**:
Telemetry recent enough for DSCC to drive trigger or rumble effects.
_Avoid_: Connected when packet freshness is the point

**Stale Telemetry**:
Telemetry that is too old to keep driving trigger or rumble effects.
_Avoid_: Disconnected when the source is present but no longer fresh

**Forza Data Out**:
The public Forza telemetry feed used by DSCC's Forza adapter.
_Avoid_: Forza module, Horizon packet

### Input And Mapping

**Steam Input Companion**:
DSCC's guarded mirror of Steam Input layout data and writes for the selected
game.
_Avoid_: Steam controller mode, native mapping

**Button Mapping**:
The DSCC view and workflow for inspecting or changing controller input bindings
for a selected game layout.
_Avoid_: Button assignment when referring to the Steam Input mirror workflow

**DSCC Input Bridge**:
DSCC's virtual input workflow for mapping physical controller input to supported
host outputs.
_Avoid_: Steam Input, Edge onboard mapping

### Modules And Safety

**Community Module**:
A user-provided DSCC extension package that is currently limited to data-only
metadata, assets, and profile templates.
_Avoid_: Plugin, executable module

**Profile Pack**:
A data-only community module that contributes DSCC profiles for supported games.
_Avoid_: Adapter pack, parser pack

**Clean Room**:
The project rule that DSCC behavior must come from public references,
permissive sources, or original experiments rather than incompatible
implementations.
_Avoid_: Reverse engineering when the source boundary is the point

**Provenance**:
The recorded source or experiment that justifies a protocol, telemetry, asset,
or controller behavior claim.
_Avoid_: Citation when the clean-room permission is the point

**Support Bundle**:
Sanitized diagnostic information prepared for troubleshooting DSCC without
exposing private paths, serials, account ids, or raw report payloads.
_Avoid_: Log dump, debug archive
