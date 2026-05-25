# Troubleshooting

This page covers the issues new DSCC users are most likely to hit.

## Windows Warns About The Installer

The current MSI is unsigned. Windows SmartScreen may show a publisher warning.
Download DSCC only from the [official Releases page](https://github.com/shiftedx/dualsense-command/releases/latest)
and compare the file with the published SHA256 checksum if you want extra
confidence.

## The App Does Not Open

1. Start **DualSense Command Center** from the Start menu.
2. Check the system tray for the DSCC icon.
3. Open `http://127.0.0.1:43473/` in a browser.
4. If it still fails, quit DSCC from the tray and start it again.

## Controller Is Not Detected

- Try USB first. It is the most reliable connection for testing.
- Close other apps that may own the controller.
- Reconnect the controller, then restart DSCC.
- For DualSense Edge onboard profile features, use USB. Bluetooth may only show
  staged local state.

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

## Steam Input Button Mapping Looks Empty

Open or create a real Steam Input layout for the selected game, then refresh
DSCC. DSCC can show safe defaults, but it will not write generated placeholder
mappings back to Steam.

## Reporting A Problem

Open a [GitHub Issue](https://github.com/shiftedx/dualsense-command/issues) and
include:

- DSCC version.
- Controller model.
- USB or Bluetooth.
- Windows version.
- Game and telemetry status.
- What you expected.
- What actually happened.
