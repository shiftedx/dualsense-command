# DualSense Command Center 0.2.4

Release date: 2026-05-22

A narrow UI hotfix for the 0.2.3 tuning surface.

## Fixes

- **Fixed Body Source layout overlap at 1440p.** The telemetry routing panel now reserves a dedicated row for the Forza body-rumble source control, so the Native / DSCC toggle no longer compresses into the telemetry effect rows.
- **Fixed trigger curve visuals for telemetry profiles.** Forza and Assetto telemetry scopes now draw the same force model used by the backend runtime profile instead of showing a generic full-height exponent curve.
- **R2 throttle graph now shows the tuned end-stop behavior.** The curve stays light through normal throttle travel, then shows the overtravel ramp and hard stop near the backend's 95% guard.
- **Global trigger tuning still uses the base actuation preview.** The backend-runtime graph is only used for telemetry game scopes, keeping controller-only Global tuning simple and editable.

## Validation gate

This hotfix was cut after a clean run of:

```powershell
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

The 1440p browser check also confirmed the Body Source control and telemetry list no longer overlap.

## Install

Download `DualSenseCommandCenter-0.2.4.msi` from the Releases page and run it. The MSI is unsigned, so Windows SmartScreen may show a publisher warning.

# DualSense Command Center 0.2.3

Release date: 2026-05-22

A focused hardware-readiness release for DualSense Edge owners, Forza haptics,
and production build hygiene. The headline is onboard Edge profile sync: DSCC
can now read the controller's Fn slots over USB, stage profile data safely when
hardware sync is unavailable, and write supported static profile settings back
to the Edge without exposing raw HID-byte write APIs.

## Highlights

- **DualSense Edge onboard memory is now a real DSCC surface.** The Games /
  Controller page now exposes the Edge Fn profiles, reads controller slots over
  USB, and shows whether each slot is synced from hardware, locally staged, or
  unavailable until a USB refresh.
- **Static Edge profiles can travel with the controller.** DSCC can write
  supported profile data to `Fn + Circle`, `Fn + Cross`, and `Fn + Square`,
  including trigger range/resistance settings, lightbar color and brightness,
  stick presets, and supported button remaps. Live telemetry effects still
  require DSCC to be running.
- **Forza body rumble now preserves the game's native feel by default.** The
  new body-rumble mode defaults to native passthrough, so Forza keeps its
  built-in engine and road feel while DSCC only adds short event cues such as
  shift and landing thumps. A DSCC full-control mode remains available for
  heavier custom tuning.
- **Production builds no longer activate mock data.** The browser mock harness
  is now dev-only: production ignores `?mock=1`, localStorage mock flags, and
  mock environment switches, and the production bundle does not include the
  fixture payload.
- **The release train is back on a single version.** Rust crates, the web app,
  the MSI packaging default, README install command, package lock metadata, and
  tray version assertions now agree on `0.2.3`.

## DualSense Edge Onboard Memory

- Added a typed Edge onboard profile model in `dscc-device` rather than passing
  loose JSON or raw bytes through the app.
- Added clean read/write helpers for Edge onboard profiles behind the existing
  validated device-output boundary.
- Added feature-report read/write support to the device transport trait,
  `hidapi` backend, and mock transport so the behavior can be tested without
  touching real controller memory.
- Added API endpoints:
  - `GET /api/controllers/:id/edge-profiles`
  - `PUT /api/controllers/:id/edge-profiles/:slot`
- Added persistent agent state for Edge slot data, including normalized staged
  slots and last-read hardware snapshots.
- Added a safe fallback when hardware sync is not available: users can still
  stage slot settings locally and see that the controller has not been written
  yet.
- Kept `Fn + Triangle` as the default/read-only slot. Assignable writes are
  limited to the user profile slots.
- Edge profile writes use the current DSCC profile as the source of truth for
  supported static controller settings; telemetry-only effects are deliberately
  not written to onboard memory.

## UX

- Added an **Edge Onboard Memory** panel to the Games / Controller page for
  DualSense Edge controllers.
- Added a USB refresh action for reading the controller's current onboard slot
  state.
- Added per-slot write actions with disabled/default-state behavior when a slot
  should not be written.
- Added status copy for synced hardware slots, locally staged slots, USB-only
  hardware reads/writes, and hardware-output-disabled staging.
- Added focused tooltips for the new Edge panel:
  - what the read action does and why USB matters
  - what each slot state means
  - what the write action includes and what still requires DSCC at runtime
- Wired the Forza body-rumble mode into the UI with clear Native / DSCC choices
  and explanatory help text.

## Safety

- Edge onboard hardware reads and writes require a DualSense Edge connected over
  USB. Bluetooth and Windows fallback controller entries only expose staged
  local state.
- Hardware profile sync still honors DSCC hardware-output mode. If hardware
  output is disabled, writes are staged locally instead of silently pretending
  controller memory changed.
- No raw HID-byte write route was added. Hardware writes continue to flow
  through validated `ControllerOutputFrame` and typed Edge profile paths.
- Manual/live telemetry haptics remain separate from onboard memory writes, so
  a saved Edge Fn profile will not imply DSCC telemetry effects work without the
  agent running.
- Release packaging still backs up persisted user state before install or
  upgrade, preserving existing profiles and controller settings outside the
  install folder.

## Reliability

- Mock API loading now happens through a dev-only dynamic import path instead
  of a production-reachable static bundle.
- `docs/contributing.md` now documents the mock harness as a Vite development
  tool only, with production builds explicitly ignoring mock toggles.
- The Edge onboard flow has route-level tests for visibility, staging, conflict
  paths, and hardware fallback behavior.
- Device-layer tests cover Edge profile round-tripping, rejecting writes to the
  default slot, feature-report transport plumbing, and output-manager hardware
  integration.
- The browser build was scanned for mock fixture strings after production
  build; none were present.

## API And Runtime

- Snapshot/controller DTOs now include Edge profile slot state for the selected
  controller.
- Profile update paths now preserve and normalize Forza `bodyRumbleMode` so
  native passthrough remains the default.
- The backend keeps Forza body rumble in native-passthrough mode unless the
  profile explicitly opts into DSCC full-control body rumble.
- Edge hardware reads/writes run through blocking-safe device-manager calls so
  the API runtime does not stall while the HID backend talks to the controller.
- The UI API client rejects Edge onboard profile read/write calls when the
  browser is running against the development mock API, avoiding a fake
  production dead end.

## Validation gate

This release was cut after a clean run of:

```powershell
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run typecheck
npm.cmd --prefix web run build
npm.cmd --prefix web run test:button-map
```

Additional release-readiness details:

- 212 Rust tests passed across the workspace.
- `svelte-check` reported 0 errors and 0 warnings.
- Button mapping performance guard stayed well under budget:
  `lookup=0.049ms`, `chips=0.031ms`, `parse=0.003ms` at p95 over 300 samples.
- Local unsigned Windows MSI was built successfully:
  `DualSenseCommandCenter-0.2.3.msi`.
- Local MSI SHA256:
  `AF5B76BD3E47B41B9D8E4638590F7FFB0C676FCD492916945B50214C365C5E31`.
- GitHub CI for `main` completed successfully after the release commit.
- GitHub release workflow for tag `v0.2.3` completed successfully and uploaded
  Windows unsigned beta artifacts plus experimental Linux raw binaries to the
  draft release flow.

## Install

Download `DualSenseCommandCenter-0.2.3.msi` from the Releases page and run it.
Per-user install; tray + agent start automatically.

The MSI is unsigned, so Windows SmartScreen may show a publisher warning.
Existing DSCC profiles and controller settings are stored in the user's config
directory and are not overwritten by the install folder. During install or
upgrade, DSCC backs up existing persisted state to
`state.preinstall-0.2.3.json` when `state.json` exists.

# DualSense Command Center 0.2.0

A focused release built around a redesigned Games surface, a custom-game flow that pulls from the user's Steam library, and an overhauled tuning ribbon that exposes the active profile in one click.

## Highlights

- **Add any Steam game to DSCC.** New **+ Add a Game** flow scans your installed Steam library and lets you register games that don't have a built-in module. Each custom game gets its own per-game profile that auto-loads when the game's `.exe` launches.
- **Pick the launch `.exe` from a real directory browser.** When DSCC can't auto-detect a launcher (e.g. games that ship a deep `Binaries/Win64/Game-Shipping.exe`), the **Select…** dialog now lets you navigate the install folder and tick the right executables. Backed by a sandboxed `GET /api/games/steam-library/browse` endpoint that's locked to the game's own install root.
- **Live profile + scope switching from any view.** The tuning ribbon's Selected Scope and Active Profile chips are now dropdown buttons — switch between Global, any installed game, or any saved profile without leaving the page.
- **Forza Horizon now defaults to Immersive.** New installs and freshly detected Forza Horizon 5 / 6 sessions start on the richer Immersive preset instead of Base.

## UX

- New Profile Scope chip pattern (eyebrow label / accent value / descriptor) standardised across the tuning ribbon and the Global Profile card.
- App header replaced controller-info duplication with brand + tagline ("Adaptive triggers, haptics, and live telemetry — tuned locally.").
- Controller card on the Games tab is now expandable — **Show details** reveals family, full HID id, transport, battery, permission state, diagnostic status, and the full capability list.
- Controller name no longer compresses/truncates; long custom names wrap on word boundaries.
- Games tab redesigned: smaller controller column, 2-up game grid, taller capsule artwork (220 × 150), portrait capsules show without cropping.
- Custom `<InitialBadge>` SVG component replaced the plain letter placeholders — gradient-filled, console-bracket cornered, scope-tinted.
- Add Game modal: real Steam capsule art (loaded from the local Steam library cache via `/api/games/steam-art/:app_id/:kind`), search by name/appId/install folder, in-place fallback when an asset 404s.
- Global Profile chip's auto-load hint moved into a tooltip so the page stays uncluttered.
- Tuning Ribbon → top tab labelled "Profiles" (was "Games"); section heading simplified to "Games" under the Tuning Scope eyebrow.
- "Active Profile" cell in the tuning ribbon — clarifies what's currently driving the controller.
- Button Mapping: removed the misaligned focus-PNG overlay; the controller render is now a clean reference image with the row hover acting as the active cue.
- Button Mapping is now available in Global scope (degraded mode with a clear explainer when no Steam Input layout applies).

## Bug fixes

- **Save was lighting up when nothing had been edited.** Dirty-state baseline is now derived from the live editable config after profile load, so Save only enables on real divergence from the loaded preset.
- **Telemetry rows on the Haptics page were dimming for effects the user had enabled.** The disabled visual now follows the user's toggle, not the agent's adapter-bound activity state.
- **Ribbon dropdowns wouldn't open.** Menus were being clipped by ancestor `overflow: hidden`; switched to `position: fixed` with dynamic anchoring.
- Add Game modal artwork: stopped relying solely on Steam CDN URLs (many apps have no public capsule). Now reads local Steam librarycache first, CDN only as fallback, with an `onerror` letter fallback if both fail.

## Performance

- **Stopped a 25 Hz render storm.** Live trigger-input polling now only runs on the Adaptive Triggers & Haptics view (the only view that consumes those values). On other tabs it's fully stopped, removing ~25 wasted full-component re-renders per second.
- **InitialBadge SVG def churn fixed.** Gradient/highlight IDs are now generated once per component instance instead of on every render. Previously, ~22 badges on the Games tab were forcing SVG def re-links on every state change, making clicks elsewhere feel sticky.
- Button-mapping p95 budgets remain green (`lookup ≤ 8 ms`, `chips ≤ 8 ms`, `parse ≤ 2 ms` against 300 samples).

## API changes

- `GET /api/games/steam-library` — full list of installed Steam games with art, stats, and discovered `.exe` candidates.
- `POST /api/games/custom` — register a user-added game (optional `processNames` override).
- `DELETE /api/games/custom/:gameId` — remove a user-added game.
- `GET /api/games/steam-art/:app_id/:kind` — serves art straight from the local Steam library cache. Sandboxed: numeric app ids only.
- `GET /api/games/steam-library/browse?appId&path` — sandboxed directory listing for a Steam game's install folder. Canonicalises the resolved path and rejects anything that escapes the install root.
- Snapshot `supportedGames[]` entries now carry `supportLevel: 'telemetry' | 'custom'`; user games are merged in and surface with a `CUSTOM` badge.

## Agent

- New `UserGameConfig` persisted alongside the rest of agent state — survives restarts.
- Process detection extended to match user-game `.exe` names so the auto-load promise holds for custom games too.
- Forza Horizon 5 / 6 default profile flipped from Base → Immersive (Forza Motorsport stays on Base).

## Validation gate

The release was cut after a clean run of:

```
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
npm.cmd --prefix web run check   # typecheck + button-map p95 + production build
```

122 tests in `dscc-agent`, 0 svelte-check warnings across 3760 files.

## Install

Download `DualSenseCommandCenter-0.2.0.msi` from the Releases page and run it. Per-user install; tray + agent start automatically.

The MSI is unsigned — Windows SmartScreen may show a publisher warning.
