# Hardware Validation Template

Use this file as the public, sanitized format for controller validation.
Do not paste raw reports, HID paths, serials, Bluetooth addresses, Steam account
paths, or private usernames.

## Test Run

- DSCC version:
- Installer/archive:
- Date:
- Tester:
- OS build:
- Controller:
- Firmware:
- Transport: USB / Bluetooth
- Install type: clean / upgrade

## Environment

- DSCC API bind: loopback / LAN opt-in
- Hardware output: enabled / disabled
- Telemetry source:
- Supported game:
- Steam Input mode:
- DSCC Input Bridge mode:

## Checklist

| Step | Result | Notes |
| --- | --- | --- |
| Clean install starts tray and agent | Not run | |
| Controller appears with model and transport | Not run | |
| UI hides raw HID path and serial data | Not run | |
| Live sticks/triggers/buttons update | Not run | |
| L2/R2 preview starts and neutralizes | Not run | |
| Lightbar preview works | Not run | |
| Global Profile does not apply game telemetry | Not run | |
| Supported game detection selects expected scope | Not run | |
| Telemetry becomes fresh while game runs | Not run | |
| Stale telemetry neutralizes triggers and rumble | Not run | |
| Disconnect/reconnect recovers | Not run | |
| Support bundle is sanitized | Not run | |

## DualSense Edge Extra Checks

| Step | Result | Notes |
| --- | --- | --- |
| Read Edge onboard slots | Not run | |
| Default Fn + Triangle slot is protected | Not run | |
| Write assignable slot with safe no-op profile | Not run | |
| Readback matches typed settings | Not run | |
| Bluetooth onboard path reports accurate status | Not run | |

## Performance Notes

- UI responsiveness:
- Bridge loop status:
- Hardware output timing:
- CPU/memory observations:
- Any frame drops or stale-input events:

## Result

- Status: Verified / Supported, pending pass / Failed
- Follow-up issue:
- Release note wording:

