# DualSense Command Center

[![Latest release](https://img.shields.io/github/v/release/shiftedx/dualsense-command?style=for-the-badge&label=release)](https://github.com/shiftedx/dualsense-command/releases/latest)
[![CI](https://img.shields.io/github/actions/workflow/status/shiftedx/dualsense-command/ci.yml?branch=main&style=for-the-badge&label=ci)](https://github.com/shiftedx/dualsense-command/actions/workflows/ci.yml)
[![Stars](https://img.shields.io/github/stars/shiftedx/dualsense-command?style=for-the-badge)](https://github.com/shiftedx/dualsense-command/stargazers)
[![License](https://img.shields.io/github/license/shiftedx/dualsense-command?style=for-the-badge)](LICENSE)

DualSense Command Center (DSCC) is a free Windows app for PlayStation DualSense
and DualSense Edge controllers on PC.

Tune adaptive triggers, haptics, lights, profiles, and racing-game telemetry
from one local app. No Python, no scripts, no command line.

Quick terms:

- **Adaptive triggers**: L2/R2 resistance, clicks, and feedback.
- **Haptics**: controller rumble and texture.
- **Telemetry**: game data like brake pressure, RPM, speed, and tire slip.

## Features

- Tune L2/R2 feel, including custom brake and throttle curves.
- Turn racing telemetry into trigger feedback, rumble, lightbar color, and RPM LEDs.
- Save, import, export, rename, and switch profiles.
- Check battery, connection, diagnostics, and controller status.
- Mirror Steam Input mappings for supported games, including an Edge paddle-shift preset.
- Use the built-in guide and tooltips when setting up.

## Download and install

1. Open the [latest release](https://github.com/shiftedx/dualsense-command/releases/latest).
2. Download **DSCC Standard** for Windows x86_64. The current release is `0.5.0`.
3. Run the `.msi` installer. Windows may show a SmartScreen warning because the installer is not signed yet; if you downloaded it from the official release page, choose **More info**, then **Run anyway**.

Your profiles and settings live in your user folder and stay in place when you update. To verify a download, the release page lists SHA256 checksums for each file.

## First-time setup

1. Open **DualSense Command Center** from the Start menu or tray icon.
2. Connect your DualSense or DualSense Edge over USB, or pair it over Bluetooth.
3. Follow or skip the first-run guide. You can reopen it later with **Guide**.
4. For everyday tuning, use the **Global Profile**.
5. To use racing telemetry, start a supported game. DSCC switches profiles automatically.

### Turn on Forza telemetry

Forza games need one in-game setting. Turn on **Data Out** or **UDP Race Telemetry**, then enter:

- IP address: `127.0.0.1`
- Port: `5300`

Assetto Corsa Rally needs no setup. See [Games that work](#games-that-work).

## Games that work

### Forza

Forza Horizon 5, Forza Horizon 6, and Forza Motorsport work through Data Out
telemetry. DSCC uses it for braking, ABS, throttle, gear shifts, rev limiter,
road texture, tire slip, RPM lighting, and more.

### Assetto Corsa Rally

Works through shared-memory telemetry on Windows, with no port setup. Launch the
game, enter a driving session, and pick the detected profile in DSCC.

## Will my controller work?

DSCC supports DualSense and DualSense Edge on Windows over USB or Bluetooth. The
Edge gets the full experience: profiles, adaptive triggers, telemetry haptics,
lightbar controls, and diagnostics.

The [Windows hardware matrix](docs/hardware-matrix.md) lists tested controller
and connection combinations.

## Is it safe?

DSCC runs locally on your PC.

- The app uses `127.0.0.1:43473`; Forza telemetry uses `127.0.0.1:5300`.
- LAN access stays off unless you enable it.
- Game haptics require a supported game, an active profile, and fresh telemetry.
- Test effects stop when the test ends.
- Steam Input edits are backed up before changes are written.
- Controller output goes through validated frames, not raw HID write routes.

## Get help

- Stuck on setup? Start with [Troubleshooting](docs/troubleshooting.md).
- Hit a bug? Use the **Support** panel to export a sanitized support bundle.
- Questions or tuning ideas? Post in [GitHub Discussions](https://github.com/shiftedx/dualsense-command/discussions).
- Reproducible bugs go in [GitHub Issues](https://github.com/shiftedx/dualsense-command/issues).

Thanks to the early testers who report rough edges and ask for the features that matter.

## Advanced and other options

<details>
<summary><strong>Other Windows installers</strong> (most people can ignore these)</summary>

Pick one installer. Most people want **DSCC Standard**.

| Installer | Use this when | Bundles the non-Steam Input Bridge broker? | Notes |
| --- | --- | --- | --- |
| **DSCC Standard** | You want controller tuning, profiles, haptics, telemetry, diagnostics, and Steam Input support, in the smallest normal install. | No | Recommended for most users. Non-Steam bridge screens stay visible but show the provider as not installed. |
| **DSCC Bridge** | You want to test the DSCC Input Bridge with local non-Steam games, without installing a separate .NET runtime. | Yes, self-contained | Larger installer. Best plug-and-play bridge option. |
| **DSCC Bridge Framework-Dependent** | You want bridge support and already have the matching x64 .NET runtime. | Yes, framework-dependent | Smaller than Bridge, but it needs that runtime. For advanced users. |

Steam games and normal controller tuning do not need a Bridge installer. Use Bridge to reach games beyond Steam.

</details>

<details>
<summary><strong>DualSense Edge onboard slots</strong></summary>

DSCC reads and writes the Edge onboard Fn-slot settings over USB or Bluetooth, through guarded HID feature reports. It marks a slot as synced only after the controller confirms the write and a fresh read matches.

Onboard sync covers static settings: trigger deadzones, stick presets, vibration intensity, trigger intensity, and button mappings. `Fn + Circle`, `Fn + Cross`, and `Fn + Square` are editable; `Fn + Triangle` stays the read-only default.

DSCC does not store live telemetry haptics on the controller. Those need DSCC running.

</details>

<details>
<summary><strong>Linux beta</strong></summary>

Download the Linux archive, extract it into a fresh folder, then run:

```bash
./dscc-cli serve --addr 127.0.0.1:43473
```

Open `http://127.0.0.1:43473/`. The archive includes the production web UI, so you do not need Vite.

Linux controller access may need local USB/HID permissions. The [Linux Beta Guide](docs/linux-beta.md) covers udev setup and validation commands.

</details>

<details>
<summary><strong>Known beta limits</strong></summary>

- We have not signed the Windows installer yet, so SmartScreen may show a publisher warning.
- DSCC checks GitHub Releases for updates and links you there. It does not install updates for you.
- Some controller and connection combinations still need hardware-matrix validation.

</details>

## For developers

Most users can skip this section.

Install web dependencies:

```powershell
npm.cmd --prefix web ci
```

Run the app locally:

```powershell
npm.cmd --prefix web run dev
```

Run the validation set:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace --all-features
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run test:source-audit
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
npm.cmd --prefix web run test:release-size
npm.cmd --prefix web run test:visual-smoke
```

Contributor docs:

- [Docs index](docs/README.md)
- [Linux Beta Guide](docs/linux-beta.md)
- [Architecture](docs/architecture.md)
- [Contributing](docs/contributing.md)
- [Game module guide](docs/game-module-contribution-guide.md)
- [Game module PR template](docs/game-module-template.md)
- [Provenance policy](docs/provenance-policy.md)
- [Release trust](docs/release-trust.md)
- [Hardware validation template](docs/hardware-validation-template.md)
- [Module manifest draft](docs/module-manifest-format.md)

DSCC is a clean-room project. Do not copy code, packet layouts, constants, or schemas from incompatible implementations.

## License

DualSense Command Center uses the Apache License, Version 2.0. Bundled visual assets and third-party dependencies keep their own terms where they apply. Bridge installers that bundle `HIDMaestro.Core.dll` include the HIDMaestro MIT license notice in `hidmaestro\THIRD_PARTY_NOTICES.txt`; the tracked source notice is [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
