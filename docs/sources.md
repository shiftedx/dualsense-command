# Source ledger

Read [Provenance Policy](provenance-policy.md) before protocol or asset work.
This is the sanitized entry point for fresh clones. Private local experiments
remain in ignored `PROVENANCE.md`; summarize evidence here before code relies
on it. Historical reviews below do not establish current hardware support.

| Area | Reference / evidence | Scope |
| --- | --- | --- |
| Forza Data Out | [Official documentation](https://support.forzamotorsport.net/hc/en-us/articles/21742934024211-Forza-Motorsport-Data-Out-Documentation), [variant history](https://forums.forza.net/t/forza-motorsport-7-data-out-feature-details/74013) | Public telemetry reference; original DSCC parsing. Reviewed May 2026. |
| Assetto shared memory | [Public field reference](https://www.assettocorsamods.net/threads/doc-shared-memory-reference.58/) | Read-only shared-memory field reference. Reviewed May 2026. |
| Windows HID | [HidD_SetFeature](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/hidsdi/nf-hidsdi-hidd_setfeature) | Platform feature-report requirements; not controller-specific protocol permission. |
| DualSense behavior | [Community data structures](https://controllers.fandom.com/wiki/Sony_DualSense/Data_Structures), [descriptor notes](https://github.com/nondebug/dualsense) | Reference-only public report observations; no implementation copying. Reviewed May 2026. |
| Edge settings | [Sony PC guide](https://www.playstation.com/en-au/support/hardware/set-up-edge-pc/) | User-visible settings and slot concepts; no Sony assets or binaries copied. |
| Steam Input | [Overview](https://partner.steamgames.com/doc/features/steam_controller), [sources](https://partner.steamgames.com/doc/features/steam_controller/input_source?language=english) | Public concepts; original guarded filesystem companion. |
| HIDMaestro broker | [Provider repository](https://github.com/hifihedgehog/HIDMaestro), approved external release `v1.3.13` | MIT provider called through an isolated broker; no source, descriptors, layouts, or profile data copied. Release workflow pins archive digest. |
| Controller artwork | [SVG Repo vector](https://www.svgrepo.com/svg/324525/dualsense), CC0 per May 2026 review | Reference for bundled tray icon from a user-supplied SVG. Record a source for each new asset. |
| Controller diagram / key glyphs | `web/public/controller-diagram/` | Original DSCC geometric SVGs and text labels created September 2026; repository Apache-2.0 license. Unverified PNGs removed from shipped files. |
| Fonts / generic icons | Installed Fontsource and Lucide license files | See root third-party notices and redistribution ledger. |
| Optional Forza glyph pack | Explicit local `DSCC_FORZA_GLYPH_ARCHIVE` | No pack is bundled. Install and restore require a user-supplied ZIP, capped at 16 MiB. Users must establish permission for their own archive. |
| Hardware experiments | [Hardware Matrix](hardware-matrix.md) | Sanitized evidence and transport-specific gaps; mock tests do not replace physical verification. |

For new entries, record source, review date, allowed use, license status, and
the sanitized test result. Reference-only material does not authorize copying
implementation or assets. Keep raw paths, identifiers, and report bytes private.
