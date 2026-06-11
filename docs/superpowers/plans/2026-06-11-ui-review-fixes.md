# UI Review Fixes (P1–P2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the P1–P2 findings from the 2026-06-11 critique (`.impeccable/critique/2026-06-11T05-34-28Z__web-src.md`): curve-drag performance, route-intent guard, parked-panel containment, copy-law fixes, idle reactive churn, toolbar containment, Cmd/Ctrl+S, self-hosted Inter-only fonts, guard-bounce toast, and saved-rail diff debounce.

**Architecture:** All work is in `web/` (Svelte 5; App.svelte and the tuning panels compile in legacy `$:` mode — keep that mode, do not migrate to runes here). Perf fixes follow one principle: stop letting high-frequency events (35ms base-feel refresh, raw pointermove, 1Hz snapshot ticks) invalidate the whole snapshot-derived `$:` graph. A new measurement harness (`web/scripts/curve-drag-budget.mjs`) provides before/after evidence; `npm run check` is the regression rail after every task.

**Tech Stack:** Svelte 5 (legacy mode in touched files), Vite, Playwright (devDep), @fontsource-variable packages (new devDeps).

**Hard rules (from the project, violations failed prior reviews):**
- Never touch `web/src/lib/api/*`, `web/src/lib/mock/*`, `web/src/lib/types.ts`.
- Copy law: no Device/HID/gamepad/plugin/backend/bus in user copy; "Everyday" pairs with "Global Profile"; no "legacy" in production source.
- No local Rust toolchain — `web/` only.
- Never commit to `main`. This plan's branch: `ui-review-fixes` off `ui-improvements`.
- Gates: `cd web && npm run check` must stay green at every commit.
- The user reviews visually: tasks marked **CHECKPOINT** stop for an eyes-on `dev:mock` look before committing.

---

### Task 0: Branch + baseline

**Files:** none (git + measurements only)

- [ ] **Step 0.1: Create the branch**

```bash
cd /Users/kmcdowell/Documents/repos/dualsense-command
git checkout ui-improvements && git pull
git checkout -b ui-review-fixes
```

- [ ] **Step 0.2: Confirm the gate baseline**

Run: `cd web && npm run check`
Expected: all gates green (typecheck, source-audit, button-map, snapshot-map, haptics-graph, build, release-size, visual-smoke). If anything is red, STOP — the branch moved; report to the user.

---

### Task 1: Curve-drag measurement harness

**Files:**
- Create: `web/scripts/curve-drag-budget.mjs`

A measurement tool (not yet a CI gate): drives a 240-step drag on the L2 curve editor in mock mode and reports DOM mutations per move and frame-time p50/p95/max. Run before and after Tasks 2–4 to prove the win.

- [ ] **Step 1.1: Write the harness**

```js
// web/scripts/curve-drag-budget.mjs
// Measures DOM mutation volume and frame times during a scripted curve drag
// in mock mode. Usage: node scripts/curve-drag-budget.mjs [--url http://...]
// Without --url it spawns `npm run dev:mock` on a free port and stops it after.
import { spawn } from 'node:child_process';
import net from 'node:net';
import process from 'node:process';
import { chromium } from 'playwright';

const urlArgIndex = process.argv.indexOf('--url');
const externalUrl = urlArgIndex >= 0 ? process.argv[urlArgIndex + 1] : null;
const host = '127.0.0.1';

function findOpenPort(startPort) {
  return new Promise((resolve, reject) => {
    const tryPort = (candidate) => {
      const server = net.createServer();
      server.unref();
      server.once('error', (error) => {
        if (error.code === 'EADDRINUSE' || error.code === 'EACCES') tryPort(candidate + 1);
        else reject(error);
      });
      server.listen(candidate, host, () => {
        server.close(() => resolve(candidate));
      });
    };
    tryPort(startPort);
  });
}

async function waitForServer(url, timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      /* not up yet */
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`dev server did not come up at ${url}`);
}

let server = null;
let baseUrl = externalUrl;
if (!baseUrl) {
  const port = await findOpenPort(5180);
  baseUrl = `http://${host}:${port}`;
  const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  server = spawn(npmCommand, ['run', 'dev:mock', '--', '--port', String(port), '--strictPort'], {
    cwd: new URL('..', import.meta.url).pathname,
    stdio: 'ignore'
  });
  await waitForServer(baseUrl);
}

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  await page.goto(baseUrl);
  await page.waitForSelector('.app-toolbar', { timeout: 15000 });
  await page.evaluate(() => {
    window.location.hash = '#/tuning';
  });
  const frame = page.locator('.dm-curve-frame').first();
  await frame.waitFor({ timeout: 15000 });

  await page.evaluate(() => {
    window.__dragMetrics = { mutations: 0, frames: [] };
    const observer = new MutationObserver((records) => {
      window.__dragMetrics.mutations += records.length;
    });
    observer.observe(document.body, { childList: true, subtree: true, attributes: true, characterData: true });
    let last = performance.now();
    const tick = (now) => {
      window.__dragMetrics.frames.push(now - last);
      last = now;
      window.__dragMetrics.raf = requestAnimationFrame(tick);
    };
    window.__dragMetrics.raf = requestAnimationFrame(tick);
  });

  const box = await frame.boundingBox();
  const startX = box.x + box.width * 0.3;
  const endX = box.x + box.width * 0.7;
  const y = box.y + box.height * 0.5;
  const MOVES = 240;
  await page.mouse.move(startX, y);
  await page.mouse.down();
  for (let i = 1; i <= MOVES; i += 1) {
    const x = startX + ((endX - startX) * i) / MOVES;
    const wobble = Math.sin(i / 8) * box.height * 0.2;
    await page.mouse.move(x, y + wobble);
    await new Promise((resolve) => setTimeout(resolve, 16));
  }
  await page.mouse.up();

  const metrics = await page.evaluate(() => {
    cancelAnimationFrame(window.__dragMetrics.raf);
    return { mutations: window.__dragMetrics.mutations, frames: window.__dragMetrics.frames };
  });
  const frames = metrics.frames.filter((ms) => ms > 0).sort((a, b) => a - b);
  const pick = (q) => frames[Math.min(frames.length - 1, Math.floor(frames.length * q))] ?? 0;
  console.log(
    JSON.stringify(
      {
        moves: MOVES,
        mutations: metrics.mutations,
        mutationsPerMove: Number((metrics.mutations / MOVES).toFixed(1)),
        frameP50Ms: Number(pick(0.5).toFixed(1)),
        frameP95Ms: Number(pick(0.95).toFixed(1)),
        frameMaxMs: Number(frames[frames.length - 1]?.toFixed(1) ?? 0)
      },
      null,
      2
    )
  );
} finally {
  await browser.close();
  if (server) server.kill();
}
```

- [ ] **Step 1.2: Run it and record the BEFORE numbers**

Run: `cd web && node scripts/curve-drag-budget.mjs`
Expected: JSON with `mutationsPerMove` ≈ 8–14 and `frameP95Ms` near or above 16.7. Save the output — it goes in the PR description.

- [ ] **Step 1.3: Commit**

```bash
git add web/scripts/curve-drag-budget.mjs
git commit -m "ui-review: add curve-drag mutation/frame measurement harness"
```

---

### Task 2: Stop reassigning `snapshot` from effect-test responses (P1, drag jank ½)

`effectState.output` has **no consumers**: no component reads it, and the support bundle (`web/src/app/supportBundle.ts:143-147`) serializes only `reason`, `dryRun`, `hardwareOutputEnabled`, `warnings`, `parityEffects`. The four `snapshot = { ...snapshot, effectState: { ... output } }` reassignment blocks exist solely to refresh a dead field — at up to ~28Hz during a base-feel test, each one re-running ~60 `$:` statements.

**Files:**
- Modify: `web/src/App.svelte` (four sites: `startBaseFeelTest` ~:1972, `stopBaseFeelTest` ~:2000, `previewBodyHaptics` ~:2041, `previewLightbarColor` ~:2072)

- [ ] **Step 2.1: In `startBaseFeelTest`, drop the reassignment**

Replace:
```js
      const result = await runEffectTest(baseFeelTestRequest(), controller?.id);

      snapshot = {
        ...snapshot,
        effectState: {
          ...snapshot.effectState,
          output: result.output
        }
      };
      baseFeelTestActive = true;
```
with:
```js
      // The test response's output frame has no UI consumers; reassigning the
      // whole snapshot here invalidated every snapshot-derived statement per
      // 35ms refresh tick. The 1Hz snapshot stream keeps effectState current.
      await runEffectTest(baseFeelTestRequest(), controller?.id);
      baseFeelTestActive = true;
```

- [ ] **Step 2.2: Apply the same change at the other three sites**

In `stopBaseFeelTest`, `previewBodyHaptics`, and `previewLightbarColor`: change `const result = await runEffectTest(...)` to `await runEffectTest(...)` and delete the `snapshot = { ...snapshot, effectState: { ...snapshot.effectState, output: result.output } };` block that follows. Nothing else in those functions changes (e.g. `previewLightbarColor` still proceeds to `saveCurrentConfig()`/`refresh()`).

- [ ] **Step 2.3: Verify**

Run: `cd web && npm run typecheck && npm run test:haptics-graph`
Expected: PASS (typecheck will catch any now-unused `result`).
Then eyes-on: `npm run dev:mock`, Tuning → press "Preview feel", drag a curve point — base-feel still activates, toast still appears, no errors in console.

- [ ] **Step 2.4: Commit**

```bash
git add web/src/App.svelte
git commit -m "ui-review: stop effect tests from reassigning the whole snapshot"
```

---

### Task 3: rAF-coalesced curve drag, cached rect, no double normalization (P1, drag jank ²⁄₂)

**Files:**
- Modify: `web/src/app/triggerCurveEditor.ts:273-292` (`beginCurveDrag`)
- Modify: `web/src/App.svelte:1258-1272` (`setPointsForSide`, `applyCurvePointEdit`)

- [ ] **Step 3.1: Rewrite `beginCurveDrag`**

Replace the whole function with:

```ts
export const beginCurveDrag = (event: PointerEvent, target: HTMLElement, options: CurveDragOptions) => {
  target.setPointerCapture(event.pointerId);

  // Pointer capture pins the target for the whole drag, so one rect read at
  // drag start replaces a forced layout per pointermove event.
  const rect = target.getBoundingClientRect();
  const pointFrom = (pointerEvent: PointerEvent) => ({
    x: clampUnit((pointerEvent.clientX - rect.left) / Math.max(1, rect.width)),
    output: clampUnit(1 - (pointerEvent.clientY - rect.top) / Math.max(1, rect.height))
  });

  // High-rate mice deliver up to 1000 pointermove events/s; coalesce to one
  // application per animation frame.
  let pending: PointerEvent | null = null;
  let frame = 0;
  const flush = () => {
    frame = 0;
    if (pending) {
      const next = pending;
      pending = null;
      options.onPoint(pointFrom(next));
    }
  };
  const applyPoint = (pointerEvent: PointerEvent) => {
    pending = pointerEvent;
    if (!frame) frame = requestAnimationFrame(flush);
  };

  const stopDrag = () => {
    if (frame) cancelAnimationFrame(frame);
    flush();
    options.onEnd();
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    target.removeEventListener('pointermove', applyPoint);
    target.removeEventListener('pointerup', stopDrag);
    target.removeEventListener('pointercancel', stopDrag);
  };

  if (options.applyInitialEvent) options.onPoint(pointFrom(event));
  target.addEventListener('pointermove', applyPoint);
  target.addEventListener('pointerup', stopDrag);
  target.addEventListener('pointercancel', stopDrag);
};
```

(`clampUnit` is already imported at the top of this file; `curveGraphPointFromPointer` stays exported — `updateCurveHover` in App.svelte still uses it for non-drag hover moves.)

- [ ] **Step 3.2: Skip the second normalization on point edits**

In `web/src/App.svelte`, replace:
```js
  const setPointsForSide = (side: TriggerSide, points: TriggerCurvePoint[]) => {
    const normalized = normalizeTriggerCurvePoints(points, side === 'l2' ? l2Curve : r2Curve);
    if (side === 'l2') {
      l2CurvePoints = normalized;
    } else {
      r2CurvePoints = normalized;
    }
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  const applyCurvePointEdit = (side: TriggerSide, edit: CurvePointEdit) => {
    if (edit.points) setPointsForSide(side, edit.points);
    return edit.index;
  };
```
with:
```js
  const setPointsForSide = (side: TriggerSide, points: TriggerCurvePoint[], alreadyNormalized = false) => {
    const normalized = alreadyNormalized ? points : normalizeTriggerCurvePoints(points, side === 'l2' ? l2Curve : r2Curve);
    if (side === 'l2') {
      l2CurvePoints = normalized;
    } else {
      r2CurvePoints = normalized;
    }
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  // Point edits come from withCurvePointSet/withCurvePointAddedOrSelected,
  // which already start from normalizeTriggerCurvePoints() output — skip the
  // second normalization pass per pointermove.
  const applyCurvePointEdit = (side: TriggerSide, edit: CurvePointEdit) => {
    if (edit.points) setPointsForSide(side, edit.points, true);
    return edit.index;
  };
```
(`addCurvePoint` / `removeCurvePoint` keep calling `setPointsForSide(side, points)` with the default `false` — their arrays append/filter after normalization, so they still need the pass.)

- [ ] **Step 3.3: Verify gates and behavior**

Run: `cd web && npm run typecheck && npm run test:haptics-graph`
Expected: PASS. Then in `dev:mock`: drag curve points on both L2 and R2 — point follows the cursor smoothly, releases cleanly, hover crosshair still tracks, add/remove point buttons still work.

- [ ] **Step 3.4: Re-measure**

Run: `cd web && node scripts/curve-drag-budget.mjs`
Expected: `mutationsPerMove` and `frameP95Ms` both well below the Task 1 baseline (target: p95 ≤ 12ms at the harness's 60Hz event rate). Record the numbers.

- [ ] **Step 3.5: Commit**

```bash
git add web/src/app/triggerCurveEditor.ts web/src/App.svelte
git commit -m "ui-review: rAF-coalesce curve drags, cache drag rect, skip double normalization"
```

---

### Task 4: Debounce the saved-rail diff (P2)

`savedDiffRows()` rebuilds the full formatted diff (~40 rows, both curves point-by-point, 15+ forza rows) on every input event of any tunable. The rail is display-only — `profileConfigDirty` has its own path — so a 100ms trailing debounce is safe.

**Files:**
- Modify: `web/src/App.svelte:1122-1126`

- [ ] **Step 4.1: Replace the direct `$:` derivation**

Replace:
```js
  $: savedRailRows = savedDiffRows(profileSaveBaselineConfig, profileDraftSnapshot, {
    includeForza: selectedTuningScope === 'game',
    intensityPercent: forzaIntensityPercent
  });
  $: unsavedCount = unsavedChangeCount(savedRailRows);
```
with:
```js
  // The rail diff is display-only (the dirty flag has its own signature path),
  // so it recomputes on a 100ms trailing debounce instead of per input event.
  let savedRailRows: ReturnType<typeof savedDiffRows> = [];
  let savedRailDiffTimer = 0;
  const refreshSavedRailRows = () => {
    savedRailRows = savedDiffRows(profileSaveBaselineConfig, profileDraftSnapshot, {
      includeForza: selectedTuningScope === 'game',
      intensityPercent: forzaIntensityPercent
    });
  };
  $: {
    void profileDraftSnapshot;
    void profileSaveBaselineConfig;
    void selectedTuningScope;
    if (typeof window === 'undefined') {
      refreshSavedRailRows();
    } else {
      window.clearTimeout(savedRailDiffTimer);
      savedRailDiffTimer = window.setTimeout(refreshSavedRailRows, 100);
    }
  }
  $: unsavedCount = unsavedChangeCount(savedRailRows);
```

- [ ] **Step 4.2: Verify**

Run: `cd web && npm run typecheck`
Expected: PASS. In `dev:mock`: tweak a slider — the rail row and the header's unsaved-count chip appear ~100ms after the tweak; Discard clears them; Save still works; the <900px bottom bar count still updates.

- [ ] **Step 4.3: Commit**

```bash
git add web/src/App.svelte
git commit -m "ui-review: debounce saved-rail diff recomputation"
```

---

### Task 5: Route intent — honor deep links after the first snapshot, explain permanent bounces (P1 + P2)

Today `syncViewFromHash()` guards the hash before the first snapshot arrives and **rewrites** it, so F5 on `#/tuning` lands on `#/status` forever. Fix: parse the *intent* separately, keep the typed hash while loading, promote the intent when readiness flips true, and toast when the bounce is genuinely permanent. This task also renames `legacyRedirects` (copy-law: banned word in source).

**Files:**
- Modify: `web/src/app/navigation.ts`
- Modify: `web/src/App.svelte` (`syncViewFromHash` ~:746, new `$:` near the readiness guard ~:504)

- [ ] **Step 5.1: navigation.ts — rename + add unguarded intent parser**

Rename `legacyRedirects` to `oldRouteRedirects` (3 occurrences: declaration :34, `knownViewHashes` :44, `viewFromHash` :65 — the doc comments already say "Old routes"). Then add below `viewFromHash`:

```ts
/** The view a hash is asking for, before readiness guards — null for unknown hashes. */
export function viewIntentFromHash(rawHash: string): AppView | null {
  const hash = oldRouteRedirects[rawHash] ?? rawHash;
  return appViews.find((item) => item.hash === hash)?.id ?? null;
}
```

- [ ] **Step 5.2: App.svelte — track and promote the intent**

Add `viewIntentFromHash` to the existing `./app/navigation` import. Then replace:
```js
  const syncViewFromHash = () => {
    const view = appViewFromHash();
    activeView = view;
    setViewHash(view);
  };
```
with:
```js
  // A deep link / reload may ask for a view whose readiness is still unknown
  // (no snapshot yet). Park the intent instead of rewriting the hash, promote
  // it when readiness flips true, and explain the bounce when it's permanent.
  let requestedView: AppView | null = null;
  const guardBounceMessages: Partial<Record<AppView, string>> = {
    tuning: 'Tuning opens once a controller is connected.',
    advancedButtonMapping: 'Button mapping needs a game selected in Tuning first.'
  };

  const syncViewFromHash = () => {
    const intent = typeof window === 'undefined' ? null : viewIntentFromHash(window.location.hash);
    const view = appViewFromHash();
    requestedView = intent && intent !== view ? intent : null;
    activeView = view;
    // Only rewrite the hash once readiness is known (or the hash was junk);
    // a pending intent keeps the user's original hash in the address bar.
    if (snapshot || !requestedView) setViewHash(view);
  };
```
And add this reactive block directly after the existing readiness-guard `$:` block (the one wrapping `guardView(activeView, ...)` at ~:504):
```js
  $: if (requestedView && snapshot && !loading) {
    const readiness = { tuningReady, buttonMappingReady, edgeSlotsReady };
    const promoted = guardView(requestedView, readiness);
    if (promoted === requestedView) {
      activeView = requestedView;
      setViewHash(requestedView);
    } else {
      const message = guardBounceMessages[requestedView];
      if (message) showToast(message, 'info');
      setViewHash(activeView);
    }
    requestedView = null;
  }
```

- [ ] **Step 5.3: Verify all routing behaviors**

Run: `cd web && npm run typecheck && npm run test:visual-smoke`
Expected: PASS — visual-smoke asserts the old-route redirect `#/games` → `#/tuning` still lands.
Then in `dev:mock` (clear `dscc-setup-verified-v1` is not needed):
1. Open `http://127.0.0.1:<port>/#/tuning` cold → briefly Status, then Tuning once the snapshot lands; hash ends `#/tuning`. F5 on Tuning → returns to Tuning.
2. Open `#/advanced/button-mapping` cold with no game selected → lands Status **with the toast** "Button mapping needs a game selected in Tuning first."
3. Old route `#/games` → ends on `#/tuning`.

- [ ] **Step 5.4: Commit**

```bash
git add web/src/app/navigation.ts web/src/App.svelte
git commit -m "ui-review: honor deep-link intent after first snapshot; toast permanent guard bounces"
```

---

### Task 6: Cut idle reactive churn at the 1Hz snapshot tick (P2)

Three identity-churn sources invalidate panels every second with nothing changing: `trackEffectActivity` always reassigns `effectActivityUntil`; `effectStatusById` is a fresh Map per tick (invalidating all 5 TelemetryRoutingPanel instances); `createButtonMappingSession` re-runs even when the mapping view is inactive.

**Files:**
- Modify: `web/src/App.svelte` (`trackEffectActivity` :638-652, `effectStatusById` :485, `buttonMappingSession` :775-790)

- [ ] **Step 6.1: Short-circuit `trackEffectActivity`**

Replace the function with:
```js
  const trackEffectActivity = (effect: CurrentEffectState) => {
    const now = Date.now();
    const nextActivity = { ...effectActivityUntil };
    let changed = false;
    for (const item of effect.parityEffects) {
      const id = normalizeEffectId(item.id);
      if (item.state === 'disabled') {
        if (id in nextActivity) {
          delete nextActivity[id];
          changed = true;
        }
      } else if (item.state === 'active') {
        nextActivity[id] = now + 550;
        changed = true;
      } else if ((nextActivity[id] ?? 0) <= now && id in nextActivity) {
        delete nextActivity[id];
        changed = true;
      }
    }
    if (changed) effectActivityUntil = nextActivity;
  };
```

- [ ] **Step 6.2: Memoize `effectStatusById` on a state signature**

Replace `$: effectStatusById = new Map(displayedParityEffects.map((effect) => [normalizeEffectId(effect.id), effect]));` with:
```js
  // Rebuild the Map (a prop of all TelemetryRoutingPanel instances) only when
  // an effect's state actually changes, not on every snapshot tick.
  let effectStatusById = new Map<string, (typeof displayedParityEffects)[number]>();
  let effectStatusSignature = '__unset__';
  $: {
    const signature = displayedParityEffects
      .map((effect) => `${normalizeEffectId(effect.id)}:${effect.state}`)
      .join('|');
    if (signature !== effectStatusSignature) {
      effectStatusSignature = signature;
      effectStatusById = new Map(displayedParityEffects.map((effect) => [normalizeEffectId(effect.id), effect]));
    }
  }
```

- [ ] **Step 6.3: Skip session creation while button mapping is inactive**

`ButtonMappingView` already ships `EMPTY_BUTTON_MAPPING_VIEW_SESSION` as its default prop. Import it (extend the existing `./lib/features/buttonMapping` import if it re-exports it, otherwise import from `./lib/features/buttonMapping/buttonMappingState`) and replace the `$: buttonMappingSession = createButtonMappingSession({ ... })` statement with:
```js
  $: buttonMappingSession = buttonMappingActive
    ? createButtonMappingSession({
        state: buttonMappingSessionState,
        store: buttonMappingSessionStore,
        active: buttonMappingActive,
        controller,
        controllerHeaderName,
        selectedTuningScope,
        steamContextGame,
        steamInputStatus,
        inputBridgeStatus,
        activeProfileName,
        profileContextGameName: profileContextGame?.name ?? null,
        bridgeProfileId: inputBridgeBindingProfileId(),
        refresh,
        notify: showToast
      })
    : EMPTY_BUTTON_MAPPING_VIEW_SESSION;
```

- [ ] **Step 6.4: Verify**

Run: `cd web && npm run typecheck && npm run test:button-map && npm run test:visual-smoke`
Expected: PASS (visual-smoke exercises button mapping end-to-end, which proves the empty-session swap doesn't break the inactive view). In `dev:mock`: effects on the tuning canvas still light up when active (mock fires them); Advanced → Button mapping still renders and edits.

- [ ] **Step 6.5: Commit**

```bash
git add web/src/App.svelte
git commit -m "ui-review: stop 1Hz snapshot ticks from rebuilding identical maps and sessions"
```

---

### Task 7: Copy-law fixes — "HID" out of user copy, "legacy" identifiers renamed (P2)

**Files:**
- Modify: `web/src/components/SupportPanel.svelte:20`
- Modify: `web/src/components/OnboardingTutorial.svelte:56`
- Modify: `web/src/lib/features/tuning/SetupGuide.svelte:51,76,79`
- Modify: `web/scripts/source-audit.mjs:76`
- (navigation.ts rename already done in Task 5.)

- [ ] **Step 7.1: SupportPanel.svelte** — replace
`<p>No raw HID paths, raw hardware IDs, serial numbers, or Bluetooth addresses are included.</p>`
with
`<p>No controller hardware identifiers, serial numbers, or Bluetooth addresses are included.</p>`

- [ ] **Step 7.2: OnboardingTutorial.svelte** — replace
`body: 'The Support panel copies a diagnostic bundle that leaves out raw HID paths, serials, Bluetooth addresses, and private Steam account paths.',`
with
`body: 'The Support panel copies a diagnostic bundle that leaves out controller hardware identifiers, serial numbers, Bluetooth addresses, and private Steam account paths.',`

- [ ] **Step 7.3: SetupGuide.svelte** — rename `legacyCopy` to `fallbackCopy` (declaration :51 and both call sites :76, :79).

- [ ] **Step 7.4: Close the audit gap** — in `web/scripts/source-audit.mjs`, the `legacy production surface` rule uses `/\blegacy\b/i`, which camelCase identifiers (`legacyRedirects`) evade. Change the pattern to `/legacy/i`. Then run `cd web && npm run test:source-audit` — expect zero findings (Tasks 5 + 7.3 removed the identifiers). If it now flags files outside `web/src` or comments you can't rename, report them rather than weakening the pattern back.

- [ ] **Step 7.5: Verify + commit**

Run: `cd web && npm run typecheck && npm run test:source-audit`
Expected: PASS.
```bash
git add web/src/components/SupportPanel.svelte web/src/components/OnboardingTutorial.svelte web/src/lib/features/tuning/SetupGuide.svelte web/scripts/source-audit.mjs
git commit -m "ui-review: copy law — HID out of user copy, legacy identifiers renamed, audit catches camelCase"
```

---

### Task 8: Self-host fonts; finish the Inter-only type system (P2) — **CHECKPOINT**

`app.css:1` is a render-blocking Google Fonts import (offline/LAN hang risk for a local agent) that still downloads Space Grotesk + Inter Tight, and three stylesheets still set Space Grotesk heading stacks against the rework's Inter-only system.

**Files:**
- Modify: `web/package.json` (new devDeps)
- Modify: `web/src/main.ts`
- Modify: `web/src/styles/app.css:1`
- Modify: `web/src/styles/tokens.css:47,51`
- Modify: `web/src/lib/features/games/addGameDialog.css:49`, `web/src/styles/button-mapping/base.css:51`, `web/src/styles/haptics/routing.css:163`, `web/src/components/Tooltip.svelte:101`

- [ ] **Step 8.1: Install self-hosted variable fonts**

```bash
cd web && npm install -D @fontsource-variable/inter @fontsource-variable/jetbrains-mono
```

- [ ] **Step 8.2: Import them in `web/src/main.ts`** (before the app styles import):

```ts
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
```

- [ ] **Step 8.3: Delete the Google Fonts `@import`** — remove line 1 of `web/src/styles/app.css` entirely.

- [ ] **Step 8.4: Point the tokens at the variable families** — in `tokens.css`:

```css
  --font-mono: "JetBrains Mono Variable", "JetBrains Mono", ui-monospace, monospace; /* literal values only: ports, IPs, raw readouts */
```
and
```css
  font-family: "Inter Variable", Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
```

- [ ] **Step 8.5: Remove the stray family declarations** — delete the `font-family: "Space Grotesk", "Inter Tight", Inter, sans-serif;` line from each of `addGameDialog.css:49`, `button-mapping/base.css:51`, `haptics/routing.css:163` (headings inherit Inter from the root), and delete the redundant `font-family: Inter, ui-sans-serif, system-ui, sans-serif;` line from `Tooltip.svelte:101` (it restates the root default).

- [ ] **Step 8.6: Verify offline + budget**

Run: `cd web && npm run build && npm run test:release-size && grep -r "fonts.googleapis" dist/ && echo "FAIL: external font ref" || echo "no external fonts"`
Expected: build green, release-size green (woff2 adds ~400–500kB raw; budget is 2.5MB with ~776kB used), "no external fonts".

- [ ] **Step 8.7: CHECKPOINT — user eyes-on**

Headings in the Add Game dialog, Button mapping title block, and section heads visibly change from Space Grotesk to Inter. Start `dev:mock`, show the user those three surfaces, and get a yes before committing. If the user wants a heading voice back, the decision is theirs (a deliberate single display face is voice; the current three-file copy-paste is drift).

- [ ] **Step 8.8: Commit**

```bash
git add web/package.json web/package-lock.json web/src/main.ts web/src/styles/app.css web/src/styles/tokens.css web/src/lib/features/games/addGameDialog.css web/src/styles/button-mapping/base.css web/src/styles/haptics/routing.css web/src/components/Tooltip.svelte
git commit -m "ui-review: self-host Inter/JetBrains Mono, drop Space Grotesk remnants and Google Fonts import"
```

---

### Task 9: Contain the utility toolbar (P2) — **CHECKPOINT**

At 390px the toolbar eats the first ~340px of every view before the status sentence, and the ambient `restart -> address` readout is settings-jargon. Fix: drop the ambient bind-address caption (it stays in the select's `title`), and collapse the toolbar behind a one-row disclosure below 760px.

**Files:**
- Modify: `web/src/App.svelte:2227-2281` (toolbar markup)
- Modify: `web/src/styles/shell-v2.css` (toolbar rules ~:129)

- [ ] **Step 9.1: Markup** — in App.svelte, delete the caption line
`<small>{lanRestartRequired ? `restart -> ${appSettings?.desiredBindAddress}` : status?.bindAddress}</small>`
then wrap the toolbar contents in a collapsible body with a narrow-only disclosure button. The section becomes:

```svelte
    <section class="app-toolbar" class:open={toolbarOpen} aria-label="Controller and display options">
      <button
        class="app-toolbar-disclosure"
        type="button"
        aria-expanded={toolbarOpen}
        onclick={() => {
          toolbarOpen = !toolbarOpen;
        }}
      >
        Controller &amp; display options
      </button>
      <div class="app-toolbar-items">
        <!-- existing children move here unchanged: the two .app-toolbar-field
             labels, the glyph .app-toolbar-toggle, .app-toolbar-spacer, and
             .app-toolbar-readout -->
      </div>
    </section>
```
and add `let toolbarOpen = false;` with the other component state near the top of the script (around :260).

- [ ] **Step 9.2: CSS** — in `shell-v2.css`, after the `.app-toolbar` rule block add:

```css
/* The toolbar is ambient context, not a destination: at narrow widths it
   collapses to one quiet row so Status leads the screen. */
.app-toolbar-disclosure {
  display: none;
  width: 100%;
  padding: 4px 2px;
  text-align: left;
  font-size: 0.72rem;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  color: var(--ink-muted);
  background: none;
  border: none;
  cursor: pointer;
}

.app-toolbar-items {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 10px 16px;
  min-width: 0;
  flex: 1;
}

@media (max-width: 759px) {
  .app-toolbar { padding: 6px 12px; }
  .app-toolbar-disclosure { display: block; }
  .app-toolbar:not(.open) .app-toolbar-items { display: none; }
  .app-toolbar.open .app-toolbar-items { padding-top: 6px; }
}
```
(The flex properties move from `.app-toolbar` to `.app-toolbar-items`; keep `.app-toolbar`'s background/border/margin/padding as-is, and keep `display: flex` on it so the disclosure and items stack — change `.app-toolbar` to `flex-direction: column; align-items: stretch;`.)

- [ ] **Step 9.3: Verify**

Run: `cd web && npm run typecheck && npm run test:visual-smoke`
Expected: PASS — visual-smoke includes the 390px viewport and selects a game via page controls; if it targeted the removed caption or toolbar layout, fix the script's selector, not the design. In `dev:mock` at 390px: the toolbar is one slim row; tapping it reveals the fields; at ≥760px nothing changed except the missing bind-address caption (still in the select's hover `title`).

- [ ] **Step 9.4: CHECKPOINT — user eyes-on at 390px and 1440px**, then commit:

```bash
git add web/src/App.svelte web/src/styles/shell-v2.css
git commit -m "ui-review: collapse utility toolbar at narrow widths, drop ambient bind-address readout"
```

---

### Task 10: Cmd/Ctrl+S saves the profile (P2)

**Files:**
- Modify: `web/src/App.svelte` (handler near the other handlers; `<svelte:window>` next to the app-shell markup)

- [ ] **Step 10.1: Add the handler** (near `navigateToView` ~:752):

```js
  // Cmd/Ctrl+S writes the draft into the profile when there is something to
  // save — the same action as the rail's "Save changes". Always prevent the
  // browser save dialog while the app has focus.
  const handleGlobalKeydown = (event: KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === 's') {
      event.preventDefault();
      if (activeView === 'tuning' && profileConfigDirty && selectedActionProfile && !profileSaveBusy) {
        void saveActiveProfile();
      }
    }
  };
```

- [ ] **Step 10.2: Wire it** — directly above `<div class="app-shell">` add:

```svelte
<svelte:window onkeydown={handleGlobalKeydown} />
```

- [ ] **Step 10.3: Verify + commit**

Run: `cd web && npm run typecheck`
Expected: PASS. In `dev:mock`: tweak a slider on Tuning, press Cmd+S → SAVED toast, rail clears; press Cmd+S on Status → nothing happens, no browser save dialog.

```bash
git add web/src/App.svelte
git commit -m "ui-review: Cmd/Ctrl+S saves profile changes from the tuning view"
```

---

### Task 11: Park the parked panels behind one quiet disclosure (P1) — **CHECKPOINT**

The below-canvas strip (Trigger curve controls, Base Haptics / Telemetry Stream chrome, Body Source) duplicates canvas controls and breaks the register. Re-homing them is a design project the user owns; the shippable containment is a single calm disclosure so the canvas ends where the canvas ends. Nothing previously rendered may be lost (project rule) — collapsed-but-reachable satisfies that.

**Files:**
- Modify: `web/src/App.svelte:2572-2635` (the `below` slot)
- Modify: `web/src/styles/tuning.css` (new `.canvas-more` rules)

- [ ] **Step 11.1: Wrap the slot content**

```svelte
        <svelte:fragment slot="below">
          <!-- Parked until these controls get real homes; collapsed so the
               canvas keeps one voice. Nothing previously rendered is lost. -->
          <details class="canvas-more">
            <summary>More tuning controls</summary>
            <!-- existing content unchanged: the TriggerCurvesPanel
                 showCurves={false} instance, then the scope-conditional
                 .canvas-parked blocks -->
          </details>
        </svelte:fragment>
```

- [ ] **Step 11.2: Style it calmly** — add to `tuning.css`:

```css
/* Parked controls live behind one quiet door below the canvas. */
.canvas-more {
  margin-top: 18px;
  border-top: 1px solid var(--hairline);
}

.canvas-more > summary {
  padding: 10px 2px;
  font-size: 0.78rem;
  color: var(--ink-muted);
  cursor: pointer;
  list-style: none;
  transition: color var(--speed) var(--ease);
}

.canvas-more > summary::-webkit-details-marker { display: none; }

.canvas-more > summary::before {
  content: '▸';
  display: inline-block;
  margin-right: 6px;
  transition: transform var(--speed) var(--ease);
}

.canvas-more[open] > summary::before { transform: rotate(90deg); }

.canvas-more > summary:hover,
.canvas-more > summary:focus-visible { color: var(--ink); }

.canvas-more > summary:focus-visible {
  outline: 2px solid var(--accent-bright);
  outline-offset: 2px;
  border-radius: var(--radius-s);
}
```

- [ ] **Step 11.3: Verify**

Run: `cd web && npm run check` (full — this task can shift layout at all three viewports).
Expected: green. In `dev:mock`: Everyday and a game scope both show the closed "More tuning controls" row below the canvas; opening it reveals the previous panels intact; keyboard (Tab + Enter) toggles it; reduced motion honored (`--speed` is 0).

- [ ] **Step 11.4: CHECKPOINT — user eyes-on.** This is the accepted-debt area; the user may instead want re-homing or per-scope labels. Show closed + open states at 1440px and 390px. Then:

```bash
git add web/src/App.svelte web/src/styles/tuning.css
git commit -m "ui-review: collapse parked tuning panels behind a quiet disclosure"
```

---

### Task 12: Final verification + PR

- [ ] **Step 12.1: Full gates**

Run: `cd web && npm run check`
Expected: all green.

- [ ] **Step 12.2: Final perf numbers**

Run: `cd web && node scripts/curve-drag-budget.mjs`
Record alongside the Task 1 baseline.

- [ ] **Step 12.3: Push and open the PR (stacked on ui-improvements)**

```bash
git push -u origin ui-review-fixes
gh pr create --base ui-improvements --title "UI review fixes: curve-drag perf, route intent, register cleanup" --body "<summary: the 10 P1–P2 findings, before/after harness numbers, checkpoints the user approved; link .impeccable/critique/2026-06-11T05-34-28Z__web-src.md>

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```
If PR #28 has merged by then, use `--base main` instead (and rebase onto main first). Note in the PR body that the P3 backlog (orphaned focus/ PNGs, rail typography, Edge slots copy, Status desktop layout, emoji glyph, bottom-bar padding) is deliberately deferred.

---

## Self-review notes

- **Spec coverage:** critique items 1–10 → Tasks 2+3 (item 1), 5 (items 2+9), 11 (item 3), 7 (item 4), 6 (item 5), 9 (item 6), 10 (item 7), 8 (item 8), 4 (item 10). P3 items deliberately excluded per user scope.
- **Ordering:** perf first (user's pick), measurement harness before any perf change.
- **Risk areas flagged in-task:** visual-smoke's old-route redirect assertion (Task 5), visual-smoke's game-selection flow (Task 9), haptics-graph parity for normalization skip (Task 3), release-size for fonts (Task 8).
- **Forbidden files:** no task touches `web/src/lib/api/*`, `web/src/lib/mock/*`, or `web/src/lib/types.ts`.
