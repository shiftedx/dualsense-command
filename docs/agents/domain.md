# Agent Domain Docs

This repo uses a single-context domain documentation layout. The whole product
shares one root glossary plus one ADR directory.

## Canonical Files

- `CONTEXT.md`: current product language, preferred terms, and terms to avoid.
- `docs/adr/`: accepted architecture decisions that shape future changes.

Read `CONTEXT.md` before renaming domain concepts, adding feature copy, shaping
issue text, or writing new agent guidance. Read relevant ADRs before changing
module boundaries, clean-room policy, profile behavior, controller output, game
module behavior, or community module behavior.

## Current ADRs

- `docs/adr/0001-keep-dscc-local-first.md`
- ADR 0002: preserve clean-room provenance
- `docs/adr/0003-separate-profile-kinds.md`
- `docs/adr/0004-use-typed-controller-output-boundary.md`
- `docs/adr/0005-separate-game-modules-and-telemetry-adapters.md`
- `docs/adr/0006-keep-community-modules-data-only.md`

## How To Use The Domain Docs

1. Start with `CONTEXT.md` to match repo language.
2. Check `docs/adr/` when the change touches an architectural boundary.
3. Preserve existing terms unless the issue explicitly asks to rename them.
4. Add or update an ADR only for durable architecture decisions, not ordinary
   implementation notes.
5. Keep one shared product context. Do not create per-crate domain glossaries
   unless the repo intentionally changes its documentation model.

## Domain Boundaries To Preserve

- Use `Controller`, `Target Controller`, and `Controller Alias` for user-facing
  controller concepts.
- Keep `DSCC Software Profile`, `Global Profile`, `Game Profile`, and
  `Edge Onboard Profile` distinct.
- Use `Game Module` for supported-game metadata and `Telemetry Adapter` for
  source-specific telemetry readers.
- Use `moduleId` for game module identity and `adapterId` for telemetry adapter
  identity.
- Keep hardware output on typed `ControllerOutputFrame` paths. Do not introduce
  raw HID-byte APIs.
- Keep community modules data-only until an ADR or explicit issue changes that
  policy.

If a change needs new product language, update `CONTEXT.md` in the same change
and mention the terminology decision in the validation summary.
