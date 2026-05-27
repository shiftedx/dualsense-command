# Security Policy

## Supported Versions

Use the latest GitHub Release unless a maintainer asks you to test a specific
build. Pre-1.0 releases may change internal APIs between versions.

## Reporting A Vulnerability

Open a private security advisory on GitHub when possible. If that is not
available, open a minimal issue that says you have a security report and avoid
posting exploit details or private device data.

Do not include:

- Raw HID paths, reports, serials, or Bluetooth addresses.
- Steam account IDs, userdata paths, or private library paths.
- Driver payloads or broker-private paths.
- Full executable paths for local games.
- Support bundles that you have not reviewed.

## Local App Boundary

DSCC is local-first. The default API binds to `127.0.0.1`. LAN API exposure must
be explicitly enabled. Mutating routes reject cross-origin requests.

The app does not add game injection, hooks, memory scanning, anti-cheat bypasses,
or raw HID-byte routes.

## Release Trust

Windows MSIs are unsigned until the project has a code-signing certificate.
Verify downloads with the SHA256 checksum files attached to each release. See
[Release Trust](docs/release-trust.md).

