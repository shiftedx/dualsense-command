# DualSense Command Center

DualSense Command Center, or DSCC, is an easy Windows app for PlayStation
DualSense and DualSense Edge controllers. It gives you adaptive triggers,
haptics, profiles, lightbar controls, and racing telemetry without setting up
Python or running scripts.

<img width="2095" height="1422" alt="DualSense Command Center haptics screen" src="https://github.com/user-attachments/assets/76650865-d946-45e5-a723-88e978988822" />

## Download

Get the latest Windows installer from [GitHub Releases](https://github.com/shiftedx/dualsense-command/releases/latest).

- Current release: `0.2.9`
- Recommended download: Windows x86_64 MSI
- Linux builds: beta archive with bundled web UI

The MSI is unsigned right now, so Windows SmartScreen may warn you. Profiles and
settings are stored in your user folder and are preserved during upgrades.
Only continue past SmartScreen if you downloaded the MSI from the official
release page and, when needed, checked it against the published checksum.

## Quick Start

1. Download and run the Windows MSI.
2. Launch **DualSense Command Center** from the Start menu.
3. Connect a DualSense or DualSense Edge controller over USB or Bluetooth.
4. Open DSCC from the tray icon, or visit `http://127.0.0.1:43473/`.
5. Use **Global Profile** for normal controller tuning.
6. Start a supported game to use telemetry-powered haptics.

For Forza games, enable the in-game **Data Out** / **UDP Race Telemetry** option:

- Target IP: `127.0.0.1`
- Target port: `5300`

That is the main setup. DSCC runs locally from the tray and opens a local web UI
at `http://127.0.0.1:43473/`.

## Supported Controllers

DSCC supports DualSense and DualSense Edge controllers on Windows over USB and
Bluetooth. DualSense Edge is fully supported for the normal DSCC runtime
experience: profiles, adaptive triggers, telemetry haptics, lightbar controls,
diagnostics, and safe game-gated output.

DualSense Edge onboard Fn-slot profile sync uses guarded HID feature reports on
USB and Bluetooth. DSCC only marks a slot synced after the controller
acknowledges the write and a fresh readback matches.
See the [Windows Hardware Matrix](docs/hardware-matrix.md) for the current
validation checklist.

### Linux Beta

Download the Linux archive, extract it into a fresh folder, then run:

```bash
./dscc-cli serve --addr 127.0.0.1:43473
```

Open `http://127.0.0.1:43473/`. Release archives include the production web UI,
so you do not need to run Vite. Linux controller access may still require local
USB/HID permissions; see the [Linux Beta Guide](docs/linux-beta.md) for udev
setup and validation commands.

## Main Features

- Tunes L2/R2 adaptive trigger feel.
- Lets you create, save, import, export, and switch profiles.
- Provides 4-8 point custom trigger curves for detailed brake/throttle feel.
- Uses live game telemetry for racing haptics when available.
- Adds body-rumble cues such as paddle-shift and landing thumps.
- Controls lightbar color, brightness, RPM colors, and player LEDs.
- Shows controller health, battery, connection, and basic diagnostics.
- Helps with Steam Input button mappings for supported game layouts, including
  a DualSense Edge paddle shift preset for keyboard-backed shifting.
- Reads and writes supported DualSense Edge onboard Fn-slot settings over USB
  or Bluetooth, with default-slot protection and readback verification.
- Checks GitHub Releases for updates and links you there. It does not install
  updates automatically.

## Supported Games

### Forza

Supported through the game's built-in Data Out / UDP telemetry:

- Forza Horizon 5
- Forza Horizon 6
- Forza Motorsport

DSCC can use Forza telemetry for brake pressure, ABS/front slip, throttle load,
shift thump, rev limiter buzz, road texture, rumble strips, tire slip, puddle
drag, suspension/impact thumps, RPM lighting, and player LEDs.

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

## Known Beta Limits

- The Windows installer is unsigned, so SmartScreen may show a publisher
  warning.
- DSCC checks GitHub Releases for updates, but it does not install updates for
  you.
- Some controller/connection combinations are still pending public hardware
  matrix validation.

## Need Help?

- Start with [Troubleshooting](docs/troubleshooting.md).
- For bug reports, copy or export a sanitized support bundle from the DSCC
  Support panel. It is designed to leave out raw hardware IDs and private paths.
- Ask setup questions, "is this expected?" beta-limit questions, or tuning ideas
  in [GitHub Discussions](https://github.com/shiftedx/dualsense-command/discussions).
- Report reproducible bugs in [GitHub Issues](https://github.com/shiftedx/dualsense-command/issues).

## DualSense Edge Notes

DualSense Edge is supported for DSCC runtime tuning on Windows over USB and
Bluetooth. On-controller Fn-slot profile sync uses the same typed profile model
over guarded USB or Bluetooth HID feature reports, and only covers supported
static settings such as trigger deadzones, stick presets, vibration intensity,
trigger intensity, and button mappings.

Live telemetry haptics are not stored on the controller. They require DSCC to be
running.

## For Developers

Most users do not need this section.

Install web dependencies:

```powershell
npm.cmd --prefix web ci
```

Run the app locally:

```powershell
npm.cmd --prefix web run dev
```

Run the usual validation set:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
```

Contributor docs:

- [Docs index](docs/README.md)
- [Linux Beta Guide](docs/linux-beta.md)
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
