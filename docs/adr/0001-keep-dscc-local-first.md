# Keep DSCC Local First

DSCC runs as a local command center with a user-session tray, local web UI, and
loopback-first API because controller output, Steam files, game telemetry, and
diagnostics are sensitive user-machine surfaces. LAN access remains explicit
opt-in so local use is ergonomic without accidentally exposing mutation routes
or hardware controls to the network.
