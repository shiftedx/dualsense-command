# Linux Beta Guide

Linux support is a beta path for testers who are comfortable running a local
CLI agent. The release archive includes the production web UI, so Vite and the
web development server are not needed.

## What To Expect

- The Linux archive targets x86_64 glibc desktop distributions.
- There is no Linux tray app yet. Start DSCC from a terminal with `dscc-cli`.
- The API and web UI still bind to `127.0.0.1:43473` by default.
- Hardware output is enabled by default in release builds. Use
  `DSCC_DISABLE_HARDWARE_OUTPUT=1` for dry-run diagnostics.
- WSL is useful for CLI or UI smoke checks, but it is not the main hardware
  support target. Native Linux is recommended for controller testing.

## Run The Release Archive

Extract into a fresh folder so the bundled `web/dist` stays beside the binaries:

```bash
mkdir dscc-linux-beta
tar -xzf DualSenseCommandCenter-v0.3.4-linux-x86_64-experimental.tar.gz -C dscc-linux-beta
cd dscc-linux-beta
./dscc-cli serve --addr 127.0.0.1:43473
```

Then open `http://127.0.0.1:43473/`.

If you move the web files, set `DSCC_WEB_DIST` to the absolute path of the
`web/dist` folder before starting the agent.

## Runtime Packages

Most desktop installs already have the runtime libraries DSCC needs. If the
agent fails to start because a system library is missing, install the matching
runtime package for your distribution:

```bash
# Debian/Ubuntu
sudo apt update
sudo apt install libudev1

# Fedora
sudo dnf install systemd-libs

# Arch
sudo pacman -S systemd
```

When building DSCC from git on Linux, install the development package too:

```bash
# Debian/Ubuntu
sudo apt install build-essential pkg-config libudev-dev

# Fedora
sudo dnf install gcc pkgconf-pkg-config systemd-devel

# Arch
sudo pacman -S base-devel pkgconf
```

## HID And Udev Permissions

DSCC uses HID access for controller discovery, diagnostics, adaptive triggers,
rumble, lightbar updates, and DualSense Edge onboard profiles. On many Linux
desktops the controller is visible, but a normal user cannot open the hidraw
device until udev grants access.

First test without `sudo`:

```bash
./dscc-cli devices list-hid --experimental --probe-open
./dscc-cli devices diagnose
```

If the controller appears with permission denied, install the udev rule included
in the release archive:

```bash
sudo install -m 0644 70-dualsense-command-center.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Unplug and reconnect the controller. If your desktop still does not grant the
ACL immediately, log out and back in.

The packaged rule is desktop-scoped for a beta: it grants the active local
desktop session access to Sony Interactive Entertainment hidraw devices through
`TAG+="uaccess"`. Do not run DSCC with `sudo` for normal use; fix the hidraw
permission instead so config files stay owned by your user.

If your setup does not use systemd-logind ACLs, such as a headless or SSH-only
session, use one local group rule instead of the packaged `uaccess` rule:

```bash
sudo groupadd -f plugdev
sudo usermod -aG plugdev "$USER"
sudo tee /etc/udev/rules.d/70-dualsense-command-center.rules >/dev/null <<'EOF'
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="054c", MODE="0660", GROUP="plugdev"
EOF
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Log out and back in after changing group membership.

To remove the rule:

```bash
sudo rm -f /etc/udev/rules.d/70-dualsense-command-center.rules
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## WSL Notes

WSL does not provide the same HID and Bluetooth device model as a native Linux
desktop. The Linux archive can still serve the UI in WSL, but hardware testing
should happen on native Linux unless you have deliberately attached a USB device
to WSL and are prepared to debug that environment.

## Quick Validation

From the extracted archive, these low-risk checks help confirm the package is
usable before a hardware test:

```bash
test -x ./dscc-cli
test -x ./dscc-agent
test -d ./web/dist
test -f ./70-dualsense-command-center.rules
./dscc-cli paths
./dscc-cli devices diagnose
```

For controller issues, include the sanitized output of these commands after the
agent is running:

```bash
./dscc-cli devices list-hid --experimental --probe-open --json
./dscc-cli support-bundle
```
