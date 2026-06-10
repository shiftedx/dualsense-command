# DSCC Web UI Rework — Design Brief

**Date:** 2026-06-10 · **Branch:** `ui-improvements` · **Status:** awaiting approval, no implementation yet
**Inputs:** brainstorm + 12 browser-validated mockups (`.superpowers/brainstorm/9273-1781129906/content/`), PRODUCT.md, CONTEXT.md, impeccable skill (product register)

## 1. Feature Summary

Reimagine the DSCC web UI as a calm, task-oriented app for non-technical Windows
gamers. The four parallel tool routes become a three-destination shell — Status,
Tuning, Advanced — where the primary surface is a game-led tuning canvas and
power tools are demoted behind an Advanced door. Visual system moves from the
"gamer cockpit" HUD to a Calm Console aesthetic (Linear/Raycast lineage) with a
PlayStation-blue accent.

## 2. Primary User Action

Pick a game (or Everyday), tune how the controller feels, and always know two
things at a glance: *is everything working* and *what's saved vs. what I've
tweaked*.

## 3. Design Direction

- **Color strategy:** Restrained. Neutral dark surfaces; one accent doing real
  work (actions, selection, live/edited state).
- **Scene sentence:** An evening PC gamer at a desk or couch in a dim room,
  controller in hand, game about to launch — glancing at DSCC to confirm
  everything works, then nudging feel. → Dark theme, low-glare, quiet.
- **Anchors:** Linear (soft dark surfaces, quiet hierarchy), Raycast
  (disciplined density, settings craft).
- **Anti-references:** current HUD (grid textures, glows, glass), RGB-gamer
  software, diagnostics-first layouts.

## 4. Scope

- **Fidelity:** production-ready (the mockups are direction; implementation is
  real Svelte 5 + CSS, no component library).
- **Breadth:** whole web UI — shell, all routes, tokens.
- **Interactivity:** shipped-quality; behavior preserved except where this
  brief intentionally changes UX.
- **Time intent:** iterate route-by-route on `ui-improvements`; nothing merges
  until the gates pass and the user signs off in the running app.

## 5. Information Architecture

```
Sidebar (slim, persistent; icon rail <760px)
├─ Status        ← default route on launch
├─ Tuning        ← game-led canvas (the heart of the app)
├─ ADVANCED ▸    ← collapsed group, expands in place
│   ├─ Controller details   (live input, calibration, connection)
│   ├─ Button mapping       (existing workflow, restyled only)
│   └─ Edge onboard slots
└─ ⚙ Settings
```

- **No Games page.** Game selection is a dropdown in the Tuning header:
  Running now (detected) → Everyday (Global Profile) → Supported games →
  "Setup guide for <game>…" → "+ Add a game manually…".
- **Profiles live in the canvas**: profile selector under the game title;
  create/duplicate/import/export inside that menu.
- Old routes `#/games`, `#/controllers`, `#/adaptive-triggers-haptics`,
  `#/button-mapping` map to `#/status`, `#/tuning`,
  `#/advanced/controller`, `#/advanced/button-mapping` (old hashes redirect).

## 6. The Tuning Canvas (locked pattern, mockup v9)

**Header band (per game, bespoke):** Steam `library_hero` behind a slim ~80px
band, double-scrimmed into the page bg; `library_600x900` cover thumbnail
anchors the game dropdown; profile selector + unsaved-changes count beneath the
title; controller name/battery right-aligned; clickable telemetry status chip.
Everyday (Global Profile) uses a neutral band, no game art.

**Working surface — semantic columns, not control-type rows:**
- **Brake · L2** — trigger curve editor + brake effects (ABS pulse, lockup rumble)
- **Throttle · R2** — trigger curve editor + throttle effects (gear-shift kick,
  rev-limiter buzz)
- **Road feel** — road texture rumble, surface detail; game-provided effects
  appear here
- **Lights** — lightbar mode/RPM colors, brightness, player LEDs

**Saved rail (furniture):** fixed ~260px panel docked right, sticky on scroll,
excluded from wrapping. Shows the active profile's saved values; edited rows
render `saved → current` with strikethrough; curve editors echo with a dashed
saved-curve ghost. Contains Preview feel ("3s · nothing saved") above
Save changes / Discard. Below ~900px it becomes a docked bottom bar
("2 unsaved changes · Save · Discard", expands to the list on tap).

**Layout contract (the no-wasted-space law):**
- Controls have intrinsic sizes; extra width adds columns or grows
  *instruments only* — never stretches sliders/rows.
- Curve editors: flex 280→460px (instruments earn size). Sliders: ~220px cap.
- Columns flex-wrap: 5-across on wide, Road feel/Lights wrap under
  Brake/Throttle near 720p widths, single column <760px.
- Saved rail is outside the wrap container at all times (until the <900px bar).
- Smoke test enforces no horizontal overflow at 390px.

## 7. Status (default route)

1. **One sentence of truth:** green/yellow/red dot + "Everything is working." +
   one plain-language clause naming game and controller.
2. **Controller** block: alias, family, connection/transport, battery; Rename
   inline; hint line for adding another controller.
3. **What's active, and why:** Profile Resolution as plain-words rows — game
   detected, profile in use, telemetry freshness, "when the game closes →
   back to Global Profile".
4. **Needs attention:** the only home for warnings; states "Nothing else needs
   you" when empty. Links into game setup if telemetry goes quiet.

## 8. Per-Game Setup (canvas state, not a pop-up)

- Each Game Module declares its requirements. Selecting an unverified game
  renders the walkthrough *in the canvas*; once verified, the canvas shows
  tuning controls permanently.
- Forza-style flow: numbered steps (a real sequence): ① game found
  (auto-verified) ② enable Data Out with exact menu path + copy buttons for
  IP/port ③ drive. A "Listening on port 5300…" box flips green passively when
  packets arrive — completion requires zero clicks.
- Zero-setup games (Assetto Corsa Rally): "No setup needed" reassurance +
  offer to pre-tune with base feel.
- **Re-entry:** the header telemetry chip ("● TELEMETRY FRESH · setup ↗") opens
  the guide anytime; turns yellow and deep-links to the fix when telemetry is
  quiet. Fallback entry in the game dropdown. Telemetry loss never yanks the
  canvas — it flags Status → Needs attention.

## 9. Advanced

Sidebar group expands in place. Controller details = live input meters,
connection facts (transport, input path, polling, firmware), stick-drift
plain-words readout, Support Bundle download. Framed as "for checking, not for
everyday tuning." Button mapping keeps its current workflow and copy
(read-only mirror messaging preserved for the smoke test), restyled to the new
tokens. Edge onboard slots move here.

## 10. Visual System (tokens.css rewrite)

Neutrals (Calm Console): bg `#141417` · sidebar/header `#18181c` · surface
`#1d1d22` · raised `#26262c` · hairline `#232329` / `#2e2e36` · ink `#d6d6dc`
· muted `#8b8b96` (large/secondary text only; verify 4.5:1 where body-sized).

Accent (PlayStation blue): `--accent #0070CC` (primary buttons) ·
`--accent-bright #1f8fff` (slider fills, live lines, meters) ·
`--accent-text #5db2ff` (accent text on dark, ≥4.5:1) · `--accent-tint #12273d`
(edited/unsaved bg) · `--accent-outline #1d4f80` (edited outlines).
Semantic: green = working, yellow = attention, red = errors only.
Final values to be expressed in OKLCH with contrast verified at implementation.

Type: Inter only (drop Space Grotesk display pairing); fixed rem scale, ratio
~1.2; JetBrains Mono retained solely for literal values (ports, IPs).
Surfaces: flat fills + hairlines; no glassmorphism, grid textures, glows, or
side-stripe borders. Radii ~6–10px. Motion: 150–250ms ease-out state
transitions only; full `prefers-reduced-motion` support.

## 11. Key States

- Tuning: setup-needed (walkthrough) · waiting-for-detection · active-clean ·
  active-with-unsaved (count in header, diff in rail, ghost curves) ·
  Everyday/Global (neutral band) · no-controller (point to Status).
- Status: all-good · attention (yellow) · no-controller-yet (first-run:
  connect instructions) · telemetry-quiet.
- Saved rail: clean (values only) · dirty (diffs + actions) · mobile bar.
- Empty states teach ("Plug in or pair a controller and it appears here").

## 12. Safety Framing (constraint #4)

Preview feel is always labeled "Nothing is saved"; unsaved drift is always
visible (header count + rail diff); plain-words distinction between what's
saved, what the controller is using right now, and what happens on
Save/Discard. Global vs Game Profile state is always named in the header and
on Status.

## 13. Constraints & Gates

- Domain language from CONTEXT.md is law; no _Avoid_ terms in copy.
  "Everyday" is a presentation label only and always appears with its domain
  term ("Everyday · Global Profile"); the term of record in copy, code, and
  docs remains Global Profile.
- Mock/real API contracts unchanged (`web/src/lib/mock/`, `web/src/lib/api/`).
  Steam art assets come via existing game artwork fields; if hero/cover grades
  are missing for a title, fall back to a neutral band + InitialBadge.
- Behavior preserved except UX changes specified here.
- Gates before any "done": `npm run typecheck`, `npm run build`,
  `npm run test:visual-smoke` (route-text assertions will need updating to the
  new IA in the same change), `npm run test:source-audit`, plus eyes-on
  `dev:mock` at desktop and 390px.
- Coordinate with the architecture-decomposition effort: this rework
  effectively decomposes App.svelte's route shells; note overlaps in PRs.

## 14. Implementation Shape (route-by-route, each step green)

1. Tokens + shell: new tokens.css, sidebar, hash redirects (old routes still
   render existing views inside the new shell).
2. Status page (new).
3. Tuning canvas: header band + game dropdown + profile menu; semantic
   columns wrapping existing trigger/haptics controls; saved rail.
4. Per-game setup state + telemetry chip re-entry.
5. Advanced: move controllers detail + button mapping + Edge slots; restyle.
6. Delete dead styles (HUD, ribbon, games grid); update smoke-test
   expectations; full gate run.

## 15. Open Questions (deliberately few)

- Sidebar at 390px: icon rail vs. bottom tab bar — decide in implementation
  against the smoke test; mockups assume collapse, either satisfies the
  contract.
- Onboarding tutorial and LAN settings panel: keep current behavior, restyle
  only; revisit copy in a later pass.
