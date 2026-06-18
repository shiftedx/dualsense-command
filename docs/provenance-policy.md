# Provenance Policy

DSCC is clean-room. Public docs, public settings, original experiments, and
sanitized hardware validation are allowed. Incompatible code, private captures,
and copied packet layouts are not allowed.

## Record Provenance When Adding

- HID report offsets or controller protocol constants.
- Telemetry packet layouts, shared-memory fields, or SDK constants.
- Steam Input path behavior or VDF assumptions.
- Game process names, store IDs, install-folder names, and artwork sources.
- Provider or broker protocol fields.
- Controller asset sources.

Use private local notes for raw research. Move only sanitized source summaries
into tracked docs or PR notes.

## Allowed Sources

- Public platform documentation.
- Public game settings screens and manuals.
- Public standards and API docs.
- Original DSCC experiments summarized without raw private values.
- Hardware validation results with paths, serials, and account data removed.
- Permissively licensed projects for concepts only, when license-compatible and
  cited.

## Blocked Sources

- Incompatible implementation code.
- Decompiled binaries.
- Private SDK headers without redistribution permission.
- Raw HID captures that include paths, serials, addresses, or payloads.
- Steam account paths, user IDs, and private library paths.
- Code, constants, comments, defaults, or structures copied from projects with
  incompatible licenses.

## PR Requirement

Every game, adapter, HID, broker, or protocol PR must include a short
provenance section:

```text
Provenance:
- Process name from public store install folder.
- UDP fields from official game telemetry docs.
- Controller behavior verified locally; no raw HID path or report payload
  included.
```

If the provenance cannot be explained in three or four clear bullets, split the
PR and document the research first.
