# DualSense Command Center

[![Latest release](https://img.shields.io/github/v/release/shiftedx/dualsense-command?style=for-the-badge&label=release)](https://github.com/shiftedx/dualsense-command/releases/latest)
[![Release downloads](https://img.shields.io/github/downloads/shiftedx/dualsense-command/total?style=for-the-badge&label=downloads)](https://github.com/shiftedx/dualsense-command/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/shiftedx/dualsense-command/ci.yml?branch=main&style=for-the-badge&label=ci)](https://github.com/shiftedx/dualsense-command/actions/workflows/ci.yml)
[![Stars](https://img.shields.io/github/stars/shiftedx/dualsense-command?style=for-the-badge)](https://github.com/shiftedx/dualsense-command/stargazers)
[![License](https://img.shields.io/github/license/shiftedx/dualsense-command?style=for-the-badge)](LICENSE)

DualSense Command Center (DSCC) is a free Windows app for the PlayStation DualSense and DualSense Edge controller.

It changes how your controller feels on PC: how the triggers push back, how it rumbles, and how it reacts to your racing games. No Python, no scripts, no command line.

<img width="2095" height="1422" alt="DualSense Command Center haptics screen" src="https://github.com/user-attachments/assets/76650865-d946-45e5-a723-88e978988822" />

A few words you will see a lot:

- **Adaptive triggers**: the L2 and R2 triggers can stiffen, click, or push back, so a brake pedal feels like a brake pedal.
- **Haptics**: full-body rumble through the controller, richer than the default buzz.
- **Telemetry**: live data your racing game sends out, such as brake pressure and engine RPM. DSCC turns it into feel.

## What you can do

- Tune how the L2 and R2 triggers feel, or draw your own 4-to-8 point curves for brake and throttle.
- Feel your racing games. DSCC reads live telemetry and drives trigger resistance and rumble for braking, ABS, gear shifts, the rev limiter, road texture, and more.
- Save setups as profiles. Create, name, import, export, and switch between them, and DSCC keeps them across updates.
- Set the lights: lightbar color and brightness, RPM colors, and the player LEDs.
- Check controller health at a glance: battery, connection, and basic diagnostics.
- Get help with Steam button mapping for supported games, including a DualSense Edge paddle-shift preset.
- Learn as you go with a built-in guide and tooltips on the main controls.

## Download and install

Most people need one file.

1. Open the [latest release](https://github.com/shiftedx/dualsense-command/releases/latest) and download **DSCC Standard**, the Windows x86_64 `.msi`. The current release is `0.4.1`.
2. Run the installer. Windows may show a blue SmartScreen warning, because we have not signed the installer yet. If you got the file from the official release page above, click **More info**, then **Run anyway**.
3. The installer asks whether to start DSCC with Windows, add a desktop shortcut, and open it after setup. The defaults work for most people.

Your profiles and settings live in your user folder and stay in place when you update. To verify a download, the release page lists SHA256 checksums for each file.

## First-time setup

1. Open **DualSense Command Center** from the Start menu or the tray icon. It also opens in your browser at `http://127.0.0.1:43473/`.
2. Connect your DualSense or DualSense Edge over USB, or pair it over Bluetooth.
3. A short guide appears the first time. Read it or skip it, and reopen it later from the **Guide** button.
4. For everyday tuning, use the **Global Profile**.
5. To feel a racing game, start a supported game. DSCC switches to that game's profile on its own.

### Turn on Forza telemetry

Forza games need one setting from inside the game. Open the game's settings, turn on **Data Out** (some games call it **UDP Race Telemetry**), and enter:

- IP address: `127.0.0.1`
- Port: `5300`

Assetto Corsa Rally needs no setup. See [Games that work](#games-that-work).

## Games that work

### Forza

Forza Horizon 5, Forza Horizon 6, and Forza Motorsport, through the in-game Data Out telemetry above.

DSCC turns that data into trigger and rumble feel for braking and ABS, throttle, gear shifts, the rev limiter, road texture, tire slip, RPM lighting, and more.

### Assetto Corsa Rally

Works through the game's shared-memory telemetry on Windows, with no port setup. Launch the game, enter a driving session, and pick the detected profile in DSCC.

## Will my controller work?

DSCC supports the DualSense and DualSense Edge on Windows, over USB or Bluetooth.

The Edge gets the full experience: profiles, adaptive triggers, telemetry haptics, lightbar controls, and diagnostics. The [Windows hardware matrix](docs/hardware-matrix.md) lists what the team has tested.

## Is it safe?

DSCC runs on your PC and stays there.

- The app and its web UI run on your machine at `127.0.0.1:43473`. The Forza listener uses `127.0.0.1:5300`.
- Nothing opens to your network unless you turn on LAN access in the app.
- Game haptics run only when a supported game is active, a profile is on, and fresh telemetry is flowing. The rest of the time you get the Global Profile.
- Test effects run only while you run the test.
- DSCC backs up your Steam Input files before it changes them, and sends controller output through validated frames instead of raw HID byte writes.

## Get help

- Stuck on setup? Start with [Troubleshooting](docs/troubleshooting.md).
- Hit a bug? Open the **Support** panel in DSCC. It exports a support bundle that leaves out your hardware IDs and private paths, ready to attach to a report.
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

DualSense Command Center uses the Apache License, Version 2.0. Bundled visual assets and third-party dependencies keep their own terms where they apply.
