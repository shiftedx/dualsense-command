# Linux Beta Guide

The experimental archive targets x86_64 glibc desktops and bundles the web UI.
There is no Linux tray; start the CLI agent. WSL can smoke-test CLI/UI, but
native Linux is the hardware test target. No Windows matrix result proves Linux
controller support.

## Run The Release Archive

Extract the downloaded archive into a fresh folder, preserving `web/dist`:

```bash
mkdir dscc-linux-beta
tar -xzf <downloaded-linux-archive.tar.gz> -C dscc-linux-beta
cd dscc-linux-beta
./dscc-cli serve --addr 127.0.0.1:43473
```

Open `http://127.0.0.1:43473/`; Vite is unnecessary. If moving the UI, set
`DSCC_WEB_DIST` to its absolute `web/dist` path before launch. Real output defaults
on. **Before diagnostics or smoke tests**, disable it:

```bash
export DSCC_DISABLE_HARDWARE_OUTPUT=1
```

Clear that flag only for deliberate physical output validation.

## Runtime Packages

Install missing runtime libraries; source builds also need development tools:

| Distribution | Runtime | Additional source-build packages |
| --- | --- | --- |
| Debian/Ubuntu | `sudo apt install libudev1` | `sudo apt install build-essential pkg-config libudev-dev` |
| Fedora | `sudo dnf install systemd-libs` | `sudo dnf install gcc pkgconf-pkg-config systemd-devel` |
| Arch | `sudo pacman -S systemd` | `sudo pacman -S base-devel pkgconf` |

For source builds, follow [Contributing](contributing.md) and build the web UI.

## HID And Udev Permissions

Test as your user with output disabled:

```bash
./dscc-cli devices list-hid --experimental --probe-open
./dscc-cli devices diagnose
```

If HID open is denied, install the archive's active-desktop-session rule:

```bash
sudo install -m 0644 70-dualsense-command-center.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Reconnect; log out/in if ACLs remain unavailable. The rule grants Sony hidraw
access through `TAG+="uaccess"`. Fix permissions rather than running DSCC with
`sudo`, which can leave root-owned config.

For headless/SSH sessions without systemd-logind ACLs, replace the packaged rule
with a local group rule:

```bash
sudo groupadd -f plugdev
sudo usermod -aG plugdev "$USER"
sudo tee /etc/udev/rules.d/70-dualsense-command-center.rules >/dev/null <<'EOF'
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="054c", MODE="0660", GROUP="plugdev"
EOF
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Log out/in after group changes. Remove either rule with:

```bash
sudo rm -f /etc/udev/rules.d/70-dualsense-command-center.rules
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## WSL Notes

WSL has different HID/Bluetooth access. Use native Linux for physical tests;
USB attachment to WSL requires separate setup and does not validate native Linux.

## Quick Validation

With output disabled, check the extracted artifact:

```bash
test -x ./dscc-cli
test -x ./dscc-agent
test -d ./web/dist
test -f ./70-dualsense-command-center.rules
./dscc-cli paths
./dscc-cli devices diagnose
```

For issues, collect after agent startup:

```bash
./dscc-cli devices list-hid --experimental --probe-open --json
./dscc-cli support-bundle
```

Review output for private identifiers before sharing. Follow
[Troubleshooting](troubleshooting.md#reporting-a-problem) for reports; record
physical tests separately from dry-run/package checks.
