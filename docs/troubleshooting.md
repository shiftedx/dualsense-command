# Troubleshooting

This page covers the issues new DSCC users are most likely to hit.

## Windows Warns About The Installer

The current MSI is unsigned. Windows SmartScreen may show a publisher warning.
Download DSCC only from the [official Releases page](https://github.com/shiftedx/dualsense-command/releases/latest)
and compare the file with the published SHA256 checksum if you want extra
confidence.

If you choose to continue, use SmartScreen's **More info** option only after
confirming the download came from the official release page. Delete installers
from any other source.

## The App Does Not Open

1. Start **DualSense Command Center** from the Start menu.
2. Check the system tray for the DSCC icon.
3. Open `http://127.0.0.1:43473/` in a browser.
4. If it still fails, quit DSCC from the tray and start it again.

## Controller Is Not Detected

- Try USB first. It is the most reliable connection for testing.
- Close other apps that may own the controller.
- Reconnect the controller, then restart DSCC.
- For DualSense Edge onboard profile features, USB is the most reliable first
  test. Bluetooth can sync onboard slots when Windows exposes HID
  feature-report access; otherwise DSCC stages changes locally and shows the
  reason.
- Check the [Windows Hardware Matrix](hardware-matrix.md) to see which
  controller/transport combinations have completed public validation.

## Forza Telemetry Is Not Working

In the game settings, enable **Data Out** or **UDP Race Telemetry**:

- Target IP: `127.0.0.1`
- Target port: `5300`

Only one app can usually listen on the same UDP port. Close other telemetry
tools if DSCC shows no packets.

## Triggers Feel Neutral In Game

DSCC keeps triggers and rumble neutral until it sees:

1. A supported game.
2. An active profile for that game.
3. Fresh telemetry from the game.

This is intentional. It prevents DSCC from taking over the controller while
you are not actually driving.

## LAN Access Does Not Work

LAN Access is off by default. In DSCC, use **Web UI Location -> LAN Access**,
save the setting, and restart the app.

Only enable LAN Access on a network you trust.

## Linux Page Does Not Open

Use the Linux release archive rather than only copying the raw binary. From the
extracted folder, run:

```bash
./dscc-cli serve --addr 127.0.0.1:43473
```

Then open `http://127.0.0.1:43473/`. The archive includes `web/dist`, so Vite is
not needed. If you built from git yourself, run `npm --prefix web ci` and
`npm --prefix web run build` first, or set `DSCC_WEB_DIST` to your built
`web/dist` folder.

See the [Linux Beta Guide](linux-beta.md) for full setup and artifact sanity
checks.

## Linux Controller Opens Only With Sudo

Do not run DSCC with `sudo` for normal use. Install the udev rule from the
Linux release archive, reconnect the controller, then test again:

```bash
sudo install -m 0644 70-dualsense-command-center.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
./dscc-cli devices list-hid --experimental --probe-open
```

If the controller still shows permission denied, log out and back in so the
desktop session receives the new device ACL. More detail is in the
[Linux Beta Guide](linux-beta.md).

## Linux Trigger Tests Feel Laggy

Use USB first and make sure your user has permission to open the controller HID
device without `sudo`. Release builds now run the trigger preview loop inside
the agent so curve testing does not depend on browser-to-agent round trips for
every trigger movement.

## Steam Input Button Mapping Looks Empty

Open or create a real Steam Input layout for the selected game, then refresh
DSCC. DSCC can show safe defaults, but it will not write generated placeholder
mappings back to Steam.

## Create A Support Bundle

The fastest bug report is one with a sanitized support bundle.

1. Open DSCC.
2. Open the **Support** panel.
3. Choose **Copy JSON** or **Export JSON** for the sanitized support bundle.
4. Attach it to your GitHub issue, or paste it if it is short.

If the web UI will not open but the local agent is running, you can also run:

```powershell
dscc-cli support-bundle
```

The bundle is meant to exclude raw HID paths, serials, Bluetooth addresses, and
private Steam account paths. Please still avoid adding those manually in issue
comments or screenshots.

## Reporting A Problem

For setup questions, tuning ideas, or "is this expected?" beta behavior, start
with [GitHub Discussions](https://github.com/shiftedx/dualsense-command/discussions).

Open a [GitHub Issue](https://github.com/shiftedx/dualsense-command/issues) and
include:

- A sanitized support bundle when possible.
- DSCC version.
- Controller model.
- USB or Bluetooth.
- Operating system and distribution, such as Windows 11 or Ubuntu 24.04.
- Which [hardware matrix](hardware-matrix.md) checklist step failed, if this is
  a controller support issue.
- Game and telemetry status.
- What you expected.
- What actually happened.
