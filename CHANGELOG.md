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
