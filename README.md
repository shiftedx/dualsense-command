# DualSense Command Center

[![Release](https://img.shields.io/github/v/release/shiftedx/dualsense-command?label=release)](https://github.com/shiftedx/dualsense-command/releases/latest)
[![CI](https://img.shields.io/github/actions/workflow/status/shiftedx/dualsense-command/ci.yml?branch=main)](https://github.com/shiftedx/dualsense-command/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/shiftedx/dualsense-command)](LICENSE)

DSCC is a free local Windows app for tuning DualSense and DualSense Edge
adaptive triggers, haptics, lights, and profiles. Everyday setup needs no scripts
or command line. Linux remains [beta](docs/linux-beta.md).

## Install and play

[**Install 0.5.1 — Windows Standard MSI**](https://github.com/shiftedx/dualsense-command/releases/download/v0.5.1/DualSenseCommandCenter-v0.5.1-windows-x86_64-standard-unsigned.msi)
Unsigned: Windows may show trust prompts. [Release notes and checksums](https://github.com/shiftedx/dualsense-command/releases/latest).

1. Verify the MSI's SHA256 checksum and signing status using [Release Trust](docs/release-trust.md).
2. Quit any running DSCC from its tray menu, then run the MSI to install or update. Profiles/settings stay in your user folder. Leave **Start with Windows** unchecked until its [validation caveat](docs/release-trust.md) is resolved.
3. Open DSCC from Start or the tray; connect the controller by USB or Bluetooth.
4. Follow **Guide** and tune the **Global Profile**. Start a supported game for automatic Game Profile selection and telemetry effects.

Controller/transport coverage, installer lifecycle, Bridge validation and signing
have [remaining validation gaps](docs/production-readiness-plan.md). Check the
[hardware matrix](docs/hardware-matrix.md) for actual evidence.

| Workflow | Controls / setup |
| --- | --- |
| Controller tuning | L2/R2 resistance, clicks, feedback curves, rumble, lightbar, battery and connection status. |
| Profiles | Save, import, export, rename and switch tuning. |
| Forza Horizon 5 / Horizon 6 / Motorsport | Enable **Data Out / UDP Race Telemetry** at `127.0.0.1:5300`; brake, ABS, throttle, shifts, limiter, texture, slip and RPM effects. |
| Assetto Corsa Rally | Windows shared memory; enter a driving session and select the detected profile. No port setup. |
| Steam Input | Mirror supported mappings and apply the Edge paddle-shift preset; edits create backups. |
| Edge slots / optional Input Bridge | Follow the [hardware matrix](docs/hardware-matrix.md) / [installer guide](docs/release-trust.md). Live telemetry effects require DSCC running. |

## Output and privacy

The API defaults to `127.0.0.1:43473`; Forza UDP defaults to
`127.0.0.1:5300`. LAN exposure requires opt-in. Validated output frames guard
controller writes; no raw HID-write routes exist. Game trigger/rumble effects
require an active Game Profile and fresh telemetry; manual tests expire.

## Help and development

- Setup or connection trouble: [Troubleshooting](docs/troubleshooting.md).
- Bugs: export a sanitized bundle from **Support**, then file [an issue](https://github.com/shiftedx/dualsense-command/issues). Tuning questions: [Discussions](https://github.com/shiftedx/dualsense-command/discussions).
- Development: [Contributing](docs/contributing.md) covers safe runs and `npm.cmd run check`; [docs index](docs/README.md) maps source owners and workflows.

DSCC follows a [clean-room provenance policy](docs/provenance-policy.md).
Code uses [Apache-2.0](LICENSE); bundled assets/dependencies retain their
[third-party terms](THIRD_PARTY_NOTICES.md). Bridge distributions include the
HIDMaestro MIT notice in `hidmaestro\THIRD_PARTY_NOTICES.txt`.
