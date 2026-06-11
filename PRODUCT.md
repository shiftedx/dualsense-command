# Product

## Register

product

## Users

Non-technical Windows gamers ("No Python, no scripts, no command line") who own a
PlayStation DualSense or DualSense Edge and play on PC — mostly racing games.
They open DSCC in two modes: a one-time **setup** moment (connect controller,
check it works) and a recurring **play** moment (pick a game, tune how it feels).
Between those moments the app should answer one question at a glance:
"is everything working?"

They are not sim-rig tinkerers. Density, jargon, and diagnostics-first layouts
read as risk, not power.

## Product Purpose

DSCC changes how a DualSense controller feels on PC: adaptive trigger
resistance, rich haptics, lights, and live racing-game telemetry turned into
feel. Success looks like: a user connects a controller, starts a supported
game, and feels the difference — without ever wondering whether the app is
about to do something scary to their hardware.

## Brand Personality

Calm, trustworthy, capable. Quiet confidence: a well-made tool that writes to
real hardware and never makes that feel dangerous. Plain language over jargon
(domain terms come from CONTEXT.md and are law). Microcopy reassures rather
than performs.

## Anchor References

- **Linear** — soft dark surfaces, quiet hierarchy, restrained accent use,
  speed as a feeling.
- **Raycast** — disciplined density, muted accents, exemplary settings panels.

## Anti-references

- The "gamer cockpit" HUD: grid textures, glows, glassmorphism, scanline
  energy, all-caps telemetry labels everywhere. (This is what the current UI
  does and what the rework moves away from.)
- RGB-gamer aesthetic (Razer/ROG software): loud, busy, salesy.
- Diagnostics-first layouts that lead with raw numbers instead of the task.

## Design Principles

1. **Status before controls.** Every screen answers "is everything OK?" before
   it offers anything to change.
2. **The task is the navigation.** Lead with what the user is doing (set up my
   controller, make my game feel great), not with tool categories.
3. **Advanced is opt-in.** Button mapping, calibration readouts, Edge slots,
   and raw telemetry live behind a clear "Advanced" door — reachable, never
   ambient.
4. **Safety is visible, not scary.** Always distinguish dry-run/test effects
   from live Hardware Output, and Global Profile from an active Game Profile,
   in plain words.
5. **Quiet surfaces, one voice.** One accent color doing real work (state,
   selection, primary action), consistent component vocabulary on every screen.

## Accessibility & Inclusion

WCAG AA contrast (≥4.5:1 body text), full `prefers-reduced-motion` support,
keyboard navigable. The 390px mobile viewport is an enforced layout target via
the visual smoke test.
