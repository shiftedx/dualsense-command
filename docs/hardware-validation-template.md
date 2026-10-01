# Hardware Validation Template

Copy this template into sanitized release evidence. Use the numbered
[hardware checklist](hardware-matrix.md#validation-checklist) as the canonical
procedure. Exclude raw reports, HID paths, serials, Bluetooth addresses, Steam
account paths and private usernames.

## Test Run

| Field | Value |
| --- | --- |
| Date / tester label | |
| DSCC version / artifact SHA256 | |
| OS build / install type | Clean / upgrade |
| Controller / firmware / transport | USB / Bluetooth |
| API binding / output | Loopback / LAN opt-in; enabled / disabled |
| Game / telemetry source | |
| Steam Input / Input Bridge mode | |

## Checklist Results

Record each matrix step and each applicable Edge extra separately. Use **Pass**,
**Fail** or **Not run**; explain failures and skipped checks. A disabled-write
run cannot verify physical effects or onboard sync.

| Matrix step / Edge extra | Result | Observation / follow-up issue |
| --- | --- | --- |
| | Not run | |

For shutdown, distinguish tray quit, restart, tray termination and session end.
Record effect neutralization, elapsed exit time and any cleanup timeout. Forced
agent/OS termination cannot guarantee a final hardware write.

## Performance and Result

- UI responsiveness / CPU / memory:
- Output timing / bridge status / stale-input events:
- Status: Verified / Partial / Failed
- Remaining checks / linked issues:
- Public release wording:
