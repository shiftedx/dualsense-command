# DualSense Command Center

DualSense Command Center, or DSCC, is a Windows tray app for PlayStation
DualSense and DualSense Edge controllers. It gives you adaptive trigger tuning,
haptics, lightbar controls, profiles, and racing telemetry without setting up
Python, scripts, or command-line tools.

<img width="2095" height="1422" alt="image" src="https://github.com/user-attachments/assets/76650865-d946-45e5-a723-88e978988822" />


## Download

- Latest release: `0.2.8`
- Main platform: Windows x86_64
- Installer: unsigned Windows MSI from GitHub Releases
- Linux: experimental raw binaries only

Download DSCC from the [latest GitHub Release](https://github.com/shiftedx/dualsense-command/releases/latest).

The MSI is currently unsigned, so Windows SmartScreen may warn during install.
Release assets include checksum files, and your existing DSCC profiles/settings
are stored in your user config folder so they are preserved during upgrades.

## Quick Start

1. Download and run the Windows MSI.
2. Launch **DualSense Command Center** from the Start menu.
3. Connect a DualSense or DualSense Edge controller over USB or Bluetooth.
4. Open DSCC from the tray icon, or visit `http://127.0.0.1:43473/`.
5. Use **Global Profile** for normal controller tuning.
6. Start a supported game and select its detected profile for telemetry haptics.

For Forza games, enable the in-game **Data Out** / **UDP Race Telemetry** option:

- Target IP: `127.0.0.1`
- Target port: `5300`

That is the main setup. DSCC runs locally and handles the agent, tray, web UI,
profiles, and controller output for you.

## What DSCC Does

- Tunes adaptive trigger feel for L2 and R2.
- Lets you create, save, import, export, and switch profiles.
- Provides 4-8 point custom trigger curves for detailed brake/throttle feel.
- Drives racing haptics from supported games when live telemetry is available.
- Adds body-rumble event cues such as paddle-shift and landing thumps.
- Controls lightbar color, brightness, RPM colors, and player LEDs.
- Shows controller health, battery, connection, and diagnostics.
- Mirrors Steam Input button mappings for supported game layouts.
- Offers experimental DualSense Edge onboard slot staging.
- Checks GitHub Releases for app updates.

## Supported Games

### Forza

Supported through the game's built-in Data Out / UDP telemetry:

- Forza Horizon 5
- Forza Horizon 6
- Forza Motorsport

DSCC can use Forza telemetry for brake pressure, ABS/front slip, handbrake wall,
throttle load, shift thump, rev limiter buzz, road texture, rumble strips, tire
slip, puddle drag, suspension/impact thumps, RPM lighting, and player LEDs.

### Assetto Corsa Rally

Assetto Corsa Rally support uses the game's public Assetto-style shared memory
telemetry on Windows. Launch the game, enter a driving session, and select the
detected profile in DSCC. No UDP port setup is required.

## Safety And Privacy

DSCC is local-first by default:

- The web UI and API run on `127.0.0.1:43473`.
- The Forza telemetry listener runs on `127.0.0.1:5300`.
- LAN access is off unless you explicitly enable it in the app.
- Game haptics require a supported detected game, an active profile, and fresh
  telemetry.
- Manual test effects only run during the requested test.
- Hardware output uses validated controller frame models, not raw HID-byte APIs.
- Steam Input writes are guarded and backed up before changes are applied.

When no supported game is active, DSCC defaults to the Global Profile instead of
taking over game-specific haptics.

## DualSense Edge Notes

DualSense Edge onboard profile support is experimental. DSCC can stage static
settings locally for Fn profile slots and can attempt guarded USB writes for
supported static data when the controller and platform allow it.

Live telemetry haptics are not stored on the controller. They require DSCC to be
running.

## LAN Access

LAN Access is intended for users who want to open the DSCC web UI from another
device on the same network.

Use **Web UI Location -> LAN Access** in the app, then restart DSCC. Direct
non-loopback launches still require `DSCC_ENABLE_LAN_API=1`, and non-loopback
Forza telemetry binding requires `DSCC_ENABLE_LAN_FORZA=1`.

## For Developers

Most users do not need this section. These commands are for contributors working
from source.

Install web dependencies:

```powershell
npm.cmd --prefix web ci
```

Run the app locally:

```powershell
npm.cmd run dev
```

Run the usual validation set:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

Useful contributor docs:

- [Architecture](docs/architecture.md)
- [Contributing](docs/contributing.md)
- [Game module guide](docs/game-module-contribution-guide.md)
- [Module manifest draft](docs/module-manifest-format.md)

DSCC is a clean-room project. Do not copy code, packet layouts, constants, or
schemas from incompatible implementations.

## License

DualSense Command Center source code is licensed under the Apache License,
Version 2.0. Third-party dependencies and bundled visual assets retain their own
terms where applicable.
