# Use Typed Controller Output Boundary

DSCC writes controller output through typed profile and `ControllerOutputFrame`
paths rather than exposing raw HID-byte APIs. The typed boundary makes clamps,
validation, dry-run behavior, redundant-write suppression, and protocol
provenance visible before real hardware can be changed.
