<script lang="ts">
  import { Minus, Plus, RotateCcw } from '@lucide/svelte';
  import Tooltip from '../../../components/Tooltip.svelte';
  import { TRIGGER_CURVE_POINT_MAX, TRIGGER_CURVE_POINT_MIN } from './hapticsModel';

  type TriggerSide = 'l2' | 'r2';
  type TuningScope = 'none' | 'global' | 'game';
  type CurvePointView = {
    index: number;
    x: number | string;
    y: number | string;
    locked: boolean;
  };
  type CurveShape = {
    rangeStart: number | string;
    rangeWidth: number | string;
    rangeEnd: number | string;
    path: string;
    curvePoints: CurvePointView[];
  };
  type CurveLive = {
    liveX: number | string;
    liveY: number | string;
  };
  type CurveHover = {
    side: TriggerSide;
    left: number;
    top: number;
    x: number;
    y: number;
  } | null;
  type CurveDragPoint = {
    side: TriggerSide;
    index: number;
  } | null;
  type PatternOption = {
    label: string;
    badge?: string;
  };

  const emptyCurveShape: CurveShape = {
    rangeStart: 0,
    rangeWidth: 100,
    rangeEnd: 100,
    path: 'M 0 100 L 100 0',
    curvePoints: []
  };
  const emptyCurveLive: CurveLive = { liveX: 0, liveY: 100 };
  const noop = () => undefined;

  // Semantic-column rendering (Task 6): 'L2'/'R2' renders a single trigger's
  // curve editor so each tuning column owns its own instrument; 'both' keeps
  // the legacy two-curve block. showCurves/showControls let the canvas park
  // the shared head + base-feel strip separately without losing them.
  export let trigger: 'L2' | 'R2' | 'both' = 'both';
  export let showCurves = true;
  export let showControls = true;

  export let selectedTuningScope: TuningScope = 'none';
  export let snapshot: unknown = null;
  export let baseFeelTestActive = false;
  export let baseFeelTestBusy = false;
  export let resetTriggerCurvesToProfileDefaults: () => void = noop;
  export let toggleBaseFeelTest: () => Promise<void> | void = noop;

  export let l2CurveShape: CurveShape = emptyCurveShape;
  export let r2CurveShape: CurveShape = emptyCurveShape;
  export let l2CurveLive: CurveLive = emptyCurveLive;
  export let r2CurveLive: CurveLive = emptyCurveLive;
  export let curveHover: CurveHover = null;
  export let curveDragPoint: CurveDragPoint = null;

  export let l2LivePress = 0;
  export let r2LivePress = 0;
  export let l2From = 0;
  export let l2To = 100;
  export let r2From = 0;
  export let r2To = 100;
  export let l2Curve = 1;
  export let r2Curve = 1;
  export let l2CurvePoints: unknown[] = [];
  export let r2CurvePoints: unknown[] = [];

  export let triggerEffect = 'Adaptive resistance';
  export let triggerIntensity = 'Strong (Standard)';
  export let vibrationIntensity = 'Medium';
  export let vibrationMode = 'Balanced';
  export let triggerEffectOptions: PatternOption[] = [];
  export let vibrationModeOptions: PatternOption[] = [];
  export let triggerEffectHelp: Record<string, string> = {};
  export let triggerStrengthHelp: Record<string, string> = {};
  export let vibrationHelp: Record<string, string> = {};
  export let vibrationModeHelp: Record<string, string> = {};

  export let triggerPressLabel: (value: number) => string = (value) => `${Math.round(value * 100)}%`;
  export let triggerRangeTooltip: (side: 'L2' | 'R2', edge: 'from' | 'to', value: number, startValue?: number) => string = () => '';
  export let triggerCurveTooltip: (side: 'L2' | 'R2', value: number) => string = () => '';
  export let showTriggerPress: (side: TriggerSide, value: number) => boolean = () => false;
  export let handleCurvePointer: (event: PointerEvent, side: TriggerSide) => void = noop as (event: PointerEvent, side: TriggerSide) => void;
  export let updateCurveHover: (event: PointerEvent, side: TriggerSide) => void = noop as (event: PointerEvent, side: TriggerSide) => void;
  export let clearCurveHover: (side: TriggerSide) => void = noop as (side: TriggerSide) => void;
  export let handleCurvePointPointer: (event: PointerEvent, side: TriggerSide, index: number) => void = noop as (event: PointerEvent, side: TriggerSide, index: number) => void;
  export let setTriggerRangeValue: (side: TriggerSide, edge: 'from' | 'to', value: number) => void = noop as (side: TriggerSide, edge: 'from' | 'to', value: number) => void;
  export let setTriggerCurveValue: (side: TriggerSide, value: number) => void = noop as (side: TriggerSide, value: number) => void;
  export let removeCurvePoint: (side: TriggerSide) => void = noop as (side: TriggerSide) => void;
  export let addCurvePoint: (side: TriggerSide) => void = noop as (side: TriggerSide) => void;
  export let setTriggerEffect: (value: string) => void = noop as (value: string) => void;
  export let setTriggerIntensity: (value: string) => void = noop as (value: string) => void;
  export let setVibrationIntensity: (value: string) => void = noop as (value: string) => void;
  export let setVibrationMode: (value: string) => void = noop as (value: string) => void;
</script>

<section class="dm-physics" aria-label="Actuation curve tuning">
  {#if showControls}
  <div class="dm-section-head">
    <div>
      <span>Actuation Engine</span>
      <h2>Trigger Curves</h2>
    </div>
    <div class="dm-section-actions">
      <Tooltip text="Restores L2/R2 range, curve, base force, and body feel to the active profile defaults. Custom profiles reset to the Base curve." side="top" align="end">
        <button
          class="dm-test-button"
          type="button"
          disabled={!snapshot}
          onclick={resetTriggerCurvesToProfileDefaults}
        >
          <RotateCcw size={14} /> Reset
        </button>
      </Tooltip>
      {#if selectedTuningScope === 'game'}
        <Tooltip text="Holds the current L2 and R2 base resistance on the controller without needing a game." side="top" align="end">
          <button
            class:active={baseFeelTestActive}
            class="dm-test-button"
            type="button"
            aria-pressed={baseFeelTestActive}
            disabled={baseFeelTestBusy || !snapshot}
            onclick={() => void toggleBaseFeelTest()}
          >
            {baseFeelTestActive ? 'Testing Actuation' : 'Test Actuation'}
          </button>
        </Tooltip>
      {/if}
    </div>
  </div>
  {/if}

  {#if showCurves}
  <div class="dm-curve-stack">
    {#if trigger !== 'R2'}
    <article class="dm-curve-module" aria-label="L2 brake actuation curve">
      <div class="dm-module-title">
        <div>
          <span>L2</span>
          <strong>Brake Pressure</strong>
        </div>
        <code>{triggerPressLabel(l2LivePress)}</code>
      </div>
      <div
        class="dm-curve-frame"
        role="img"
        aria-label="L2 actuation response curve with live input crosshair"
        onpointerdown={(event) => handleCurvePointer(event, 'l2')}
        onpointermove={(event) => updateCurveHover(event, 'l2')}
        onpointerleave={() => clearCurveHover('l2')}
      >
        <svg class="dm-trigger-curve" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <defs>
            <filter id="dm-blue-glow" x="-20%" y="-20%" width="140%" height="140%">
              <feGaussianBlur stdDeviation="1.1" result="blur" />
              <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
            </filter>
          </defs>
          <path class="curve-grid" d="M 0 75 H 100 M 0 50 H 100 M 0 25 H 100 M 25 0 V 100 M 50 0 V 100 M 75 0 V 100" />
          <path class="curve-linear" d="M 0 100 L 100 0" />
          <rect class="curve-range-fill" x={l2CurveShape.rangeStart} y="96" width={l2CurveShape.rangeWidth} height="2.5" rx="1.25" />
          <line class="curve-range-edge" x1={l2CurveShape.rangeStart} y1="0" x2={l2CurveShape.rangeStart} y2="100" />
          <line class="curve-range-edge" x1={l2CurveShape.rangeEnd} y1="0" x2={l2CurveShape.rangeEnd} y2="100" />
          <path class="curve-force" d={l2CurveShape.path} />
          {#if curveHover?.side === 'l2'}
            <line class="curve-crosshair" x1={curveHover.left.toFixed(2)} y1="0" x2={curveHover.left.toFixed(2)} y2="100" />
          {/if}
          {#if showTriggerPress('l2', l2LivePress)}
            <line class="curve-live" x1={l2CurveLive.liveX} y1="0" x2={l2CurveLive.liveX} y2="100" />
            <circle class="curve-live-dot" cx={l2CurveLive.liveX} cy={l2CurveLive.liveY} r="1.75" />
          {/if}
        </svg>
        {#each l2CurveShape.curvePoints as point}
          <button
            class:active={curveDragPoint?.side === 'l2' && curveDragPoint.index === point.index}
            class:locked={point.locked}
            class="dm-curve-control-handle"
            style="--point-x:{point.x}%;--point-y:{point.y}%;"
            type="button"
            aria-label="L2 curve control point"
            aria-disabled={point.locked}
            onpointerdown={(event) => point.locked ? (event.preventDefault(), event.stopPropagation()) : handleCurvePointPointer(event, 'l2', point.index)}
          ></button>
        {/each}
        {#if curveHover?.side === 'l2'}
          <div class="dm-curve-tooltip" style="left:{curveHover.left}%;top:{curveHover.top}%;">
            <code>IN {Math.round(curveHover.x * 100).toString().padStart(3, '0')}</code>
            <code>OUT {Math.round(curveHover.y * 100).toString().padStart(3, '0')}</code>
          </div>
        {/if}
      </div>
      <div class="dm-slider-bank">
        <Tooltip block text={triggerRangeTooltip('L2', 'from', l2From)} side="top" align="start">
          <label class="dm-slider-row">
            <span>Start</span>
            <input class="dm-range" style="--value:{l2From}%" value={l2From} max={l2To} min="0" type="range" oninput={(event) => setTriggerRangeValue('l2', 'from', event.currentTarget.valueAsNumber)} />
            <code>{l2From.toString().padStart(3, '0')}</code>
          </label>
        </Tooltip>
        <Tooltip block text={triggerRangeTooltip('L2', 'to', l2To, l2From)} side="top" align="start">
          <label class="dm-slider-row">
            <span>End</span>
            <input class="dm-range" style="--value:{l2To}%" value={l2To} max="100" min={l2From} type="range" oninput={(event) => setTriggerRangeValue('l2', 'to', event.currentTarget.valueAsNumber)} />
            <code>{l2To.toString().padStart(3, '0')}</code>
          </label>
        </Tooltip>
        <Tooltip block text={triggerCurveTooltip('L2', l2Curve)} side="top" align="start">
          <label class="dm-slider-row">
            <span>Curve</span>
            <input class="dm-range" style="--value:{((l2Curve - 0.5) / 3) * 100}%" value={l2Curve} max="3.5" min="0.5" step="0.05" type="range" oninput={(event) => setTriggerCurveValue('l2', event.currentTarget.valueAsNumber)} />
            <code>{l2Curve.toFixed(2)}</code>
          </label>
        </Tooltip>
        <div class="dm-curve-point-row">
          <span>Points</span>
          <div class="dm-curve-point-actions">
            <Tooltip text="Remove the least dramatic editable control point." side="top" align="center">
              <button class="dm-icon-button" type="button" aria-label="Remove L2 curve point" disabled={l2CurvePoints.length <= TRIGGER_CURVE_POINT_MIN} onclick={() => removeCurvePoint('l2')}>
                <Minus size={14} />
              </button>
            </Tooltip>
            <code>{l2CurvePoints.length}</code>
            <Tooltip text="Add an editable control point to the widest curve segment." side="top" align="center">
              <button class="dm-icon-button" type="button" aria-label="Add L2 curve point" disabled={l2CurvePoints.length >= TRIGGER_CURVE_POINT_MAX} onclick={() => addCurvePoint('l2')}>
                <Plus size={14} />
              </button>
            </Tooltip>
          </div>
        </div>
      </div>
    </article>
    {/if}

    {#if trigger !== 'L2'}
    <article class="dm-curve-module" aria-label="R2 throttle actuation curve">
      <div class="dm-module-title">
        <div>
          <span>R2</span>
          <strong>Throttle Load</strong>
        </div>
        <code>{triggerPressLabel(r2LivePress)}</code>
      </div>
      <div
        class="dm-curve-frame"
        role="img"
        aria-label="R2 actuation response curve with live input crosshair"
        onpointerdown={(event) => handleCurvePointer(event, 'r2')}
        onpointermove={(event) => updateCurveHover(event, 'r2')}
        onpointerleave={() => clearCurveHover('r2')}
      >
        <svg class="dm-trigger-curve" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <path class="curve-grid" d="M 0 75 H 100 M 0 50 H 100 M 0 25 H 100 M 25 0 V 100 M 50 0 V 100 M 75 0 V 100" />
          <path class="curve-linear" d="M 0 100 L 100 0" />
          <rect class="curve-range-fill" x={r2CurveShape.rangeStart} y="96" width={r2CurveShape.rangeWidth} height="2.5" rx="1.25" />
          <line class="curve-range-edge" x1={r2CurveShape.rangeStart} y1="0" x2={r2CurveShape.rangeStart} y2="100" />
          <line class="curve-range-edge" x1={r2CurveShape.rangeEnd} y1="0" x2={r2CurveShape.rangeEnd} y2="100" />
          <path class="curve-force" d={r2CurveShape.path} />
          {#if curveHover?.side === 'r2'}
            <line class="curve-crosshair" x1={curveHover.left.toFixed(2)} y1="0" x2={curveHover.left.toFixed(2)} y2="100" />
          {/if}
          {#if showTriggerPress('r2', r2LivePress)}
            <line class="curve-live" x1={r2CurveLive.liveX} y1="0" x2={r2CurveLive.liveX} y2="100" />
            <circle class="curve-live-dot" cx={r2CurveLive.liveX} cy={r2CurveLive.liveY} r="1.75" />
          {/if}
        </svg>
        {#each r2CurveShape.curvePoints as point}
          <button
            class:active={curveDragPoint?.side === 'r2' && curveDragPoint.index === point.index}
            class:locked={point.locked}
            class="dm-curve-control-handle"
            style="--point-x:{point.x}%;--point-y:{point.y}%;"
            type="button"
            aria-label="R2 curve control point"
            aria-disabled={point.locked}
            onpointerdown={(event) => point.locked ? (event.preventDefault(), event.stopPropagation()) : handleCurvePointPointer(event, 'r2', point.index)}
          ></button>
        {/each}
        {#if curveHover?.side === 'r2'}
          <div class="dm-curve-tooltip" style="left:{curveHover.left}%;top:{curveHover.top}%;">
            <code>IN {Math.round(curveHover.x * 100).toString().padStart(3, '0')}</code>
            <code>OUT {Math.round(curveHover.y * 100).toString().padStart(3, '0')}</code>
          </div>
        {/if}
      </div>
      <div class="dm-slider-bank">
        <Tooltip block text={triggerRangeTooltip('R2', 'from', r2From)} side="top" align="start">
          <label class="dm-slider-row">
            <span>Start</span>
            <input class="dm-range" style="--value:{r2From}%" value={r2From} max={r2To} min="0" type="range" oninput={(event) => setTriggerRangeValue('r2', 'from', event.currentTarget.valueAsNumber)} />
            <code>{r2From.toString().padStart(3, '0')}</code>
          </label>
        </Tooltip>
        <Tooltip block text={triggerRangeTooltip('R2', 'to', r2To, r2From)} side="top" align="start">
          <label class="dm-slider-row">
            <span>End</span>
            <input class="dm-range" style="--value:{r2To}%" value={r2To} max="100" min={r2From} type="range" oninput={(event) => setTriggerRangeValue('r2', 'to', event.currentTarget.valueAsNumber)} />
            <code>{r2To.toString().padStart(3, '0')}</code>
          </label>
        </Tooltip>
        <Tooltip block text={triggerCurveTooltip('R2', r2Curve)} side="top" align="start">
          <label class="dm-slider-row">
            <span>Curve</span>
            <input class="dm-range" style="--value:{((r2Curve - 0.5) / 3) * 100}%" value={r2Curve} max="3.5" min="0.5" step="0.05" type="range" oninput={(event) => setTriggerCurveValue('r2', event.currentTarget.valueAsNumber)} />
            <code>{r2Curve.toFixed(2)}</code>
          </label>
        </Tooltip>
        <div class="dm-curve-point-row">
          <span>Points</span>
          <div class="dm-curve-point-actions">
            <Tooltip text="Remove the least dramatic editable control point." side="top" align="center">
              <button class="dm-icon-button" type="button" aria-label="Remove R2 curve point" disabled={r2CurvePoints.length <= TRIGGER_CURVE_POINT_MIN} onclick={() => removeCurvePoint('r2')}>
                <Minus size={14} />
              </button>
            </Tooltip>
            <code>{r2CurvePoints.length}</code>
            <Tooltip text="Add an editable control point to the widest curve segment." side="top" align="center">
              <button class="dm-icon-button" type="button" aria-label="Add R2 curve point" disabled={r2CurvePoints.length >= TRIGGER_CURVE_POINT_MAX} onclick={() => addCurvePoint('r2')}>
                <Plus size={14} />
              </button>
            </Tooltip>
          </div>
        </div>
      </div>
    </article>
    {/if}
  </div>
  {/if}

  {#if showControls}
  <div class="dm-parameter-strip" aria-label="Base force and light routing">
    <Tooltip block text={triggerEffectHelp[triggerEffect] ?? 'Selects the base adaptive trigger behavior.'} side="top" align="start">
      <label>
        <span>Mode</span>
        <select value={triggerEffect} onchange={(event) => setTriggerEffect(event.currentTarget.value)}>
          {#each triggerEffectOptions as option}
            <option>{option.label}</option>
          {/each}
        </select>
      </label>
    </Tooltip>
    <Tooltip block text={triggerStrengthHelp[triggerIntensity] ?? 'Controls the base trigger force multiplier.'} side="top" align="start">
      <label>
        <span>Force</span>
        <select value={triggerIntensity} onchange={(event) => setTriggerIntensity(event.currentTarget.value)}>
          <option>Off</option><option>Weak</option><option>Medium</option><option>Strong (Standard)</option>
        </select>
      </label>
    </Tooltip>
    <Tooltip block text={vibrationHelp[vibrationIntensity] ?? 'Controls the body rumble multiplier.'} side="top" align="start">
      <label>
        <span>Body</span>
        <select value={vibrationIntensity} onchange={(event) => setVibrationIntensity(event.currentTarget.value)}>
          <option>Off</option><option>Low</option><option>Medium</option><option>High</option>
        </select>
      </label>
    </Tooltip>
    <Tooltip block text={vibrationModeHelp[vibrationMode] ?? 'Controls the body haptic motor blend.'} side="top" align="start">
      <label>
        <span>Feel</span>
        <select value={vibrationMode} onchange={(event) => setVibrationMode(event.currentTarget.value)}>
          {#each vibrationModeOptions as option}
            <option>{option.label}</option>
          {/each}
        </select>
      </label>
    </Tooltip>
  </div>
  {/if}
</section>
