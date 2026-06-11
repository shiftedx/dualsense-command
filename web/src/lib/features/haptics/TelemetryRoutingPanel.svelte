<script lang="ts">
  import Tooltip from '../../../components/Tooltip.svelte';
  import type {
    ForzaAbsMode,
    ForzaAbsSlipSource,
    ForzaAbsTuningConfiguration,
    ForzaBrakeTuningConfiguration,
    ForzaBodyRumbleMode,
    ForzaEffectConfiguration,
    ForzaEffectRoute,
    ForzaRevLimiterTuningConfiguration,
    ForzaShiftClutchMode,
    ForzaShiftTuningConfiguration,
    ForzaThrottleTuningConfiguration
  } from '../../types';
  import { forzaAbsModeOptions, forzaAbsSlipSourceOptions } from './hapticsOptions';
  import type { ForzaEffectMeta } from './hapticsModel';

  type BodyRumbleModeOption = {
    value: ForzaBodyRumbleMode;
    label: string;
    badge: string;
    help: string;
  };

  type RouteOption = {
    value: ForzaEffectRoute;
    label: string;
  };

  const noop = () => undefined;
  let advancedOpen = false;
  const defaultEffect = (id: string): ForzaEffectConfiguration => ({
    id,
    enabled: false,
    intensity: 0,
    route: 'body_both'
  });

  // Semantic-column rendering (Task 6): the tuning canvas renders one
  // embedded instance per column with a filtered `forzaEffectMetas` subset
  // (showChrome=false), and parks the stream head + body source chrome below
  // the grid in a chrome-only instance (showEffects=false). Defaults keep the
  // legacy all-in-one panel behavior.
  export let showChrome = true;
  export let showEffects = true;

  export let enabledForzaEffectCount = 0;
  export let allForzaEffectsEnabled = false;
  export let forzaEffectMetas: ForzaEffectMeta[] = [];
  export let forzaEffectsById: ReadonlyMap<string, ForzaEffectConfiguration> = new Map();
  export let effectStatusById: ReadonlyMap<string, { state?: string }> = new Map();
  export let forzaBodyRumbleMode: ForzaBodyRumbleMode = 'native_passthrough';
  export let forzaBrakeTuning: ForzaBrakeTuningConfiguration = {
    baselineForce: 76 / 255,
    normalForce: 1,
    endstopForce: 1,
    endstopBoost: 1.25,
    wallPosition: 0.48,
    guardMinEnd: 0.48,
    fullForceAt: 0.8,
    rampCurve: 0.8
  };
  export let forzaAbsTuning: ForzaAbsTuningConfiguration = {
    mode: 'strong_pulse',
    slipSource: 'auto_front_first',
    slipThreshold: 0.68,
    brakeThresholdRatio: 0.38,
    minSpeedKmh: 12,
    minStrength: 48 / 63,
    maxStrength: 1,
    frequencyHz: 34,
    curve: 1
  };
  export let forzaThrottleTuning: ForzaThrottleTuningConfiguration = {
    baselineForce: 3 / 255,
    normalForce: 28 / 255,
    endstopForce: 106 / 255,
    endstopBoost: 3,
    wallPosition: 0.8,
    guardMinEnd: 0.8,
    rampWidth: 0.2,
    rampCurve: 2.4
  };
  export let forzaShiftTuning: ForzaShiftTuningConfiguration = {
    wallFormAt: 0.15,
    frequencyHz: 34,
    wallZones: 4,
    bodyLowWeight: 0.92,
    bodyHighWeight: 0.84,
    clutchMode: 'auto',
    clutchThreshold: 0.4,
    withClutchStrength: 0.58,
    withoutClutchStrength: 1,
    withClutchDurationMs: 130,
    withoutClutchDurationMs: 240,
    clutchBodyCut: 0.78
  };
  export let forzaRevLimiterTuning: ForzaRevLimiterTuningConfiguration = {
    thresholdRatio: 0.93,
    minStrength: 18 / 63,
    maxStrength: 18 / 63,
    frequencyHz: 42,
    wallFormThrottleAt: 0.6,
    wallZones: 4,
    curve: 1,
    bodyLowWeight: 0.2,
    bodyHighWeight: 0.8
  };
  export let bodyRumbleModeOptions: BodyRumbleModeOption[] = [];
  export let forzaRoutes: RouteOption[] = [];
  export let forzaEffect: (id: string) => ForzaEffectConfiguration = defaultEffect;
  export let toggleAllForzaEffects: () => void = noop;
  export let setForzaBodyRumbleMode: (value: ForzaBodyRumbleMode) => void = noop as (value: ForzaBodyRumbleMode) => void;
  export let updateForzaBrakeTuning: (patch: Partial<ForzaBrakeTuningConfiguration>) => void = noop as (
    patch: Partial<ForzaBrakeTuningConfiguration>
  ) => void;
  export let updateForzaAbsTuning: (patch: Partial<ForzaAbsTuningConfiguration>) => void = noop as (
    patch: Partial<ForzaAbsTuningConfiguration>
  ) => void;
  export let updateForzaThrottleTuning: (patch: Partial<ForzaThrottleTuningConfiguration>) => void = noop as (
    patch: Partial<ForzaThrottleTuningConfiguration>
  ) => void;
  export let updateForzaShiftTuning: (patch: Partial<ForzaShiftTuningConfiguration>) => void = noop as (
    patch: Partial<ForzaShiftTuningConfiguration>
  ) => void;
  export let updateForzaRevLimiterTuning: (patch: Partial<ForzaRevLimiterTuningConfiguration>) => void = noop as (
    patch: Partial<ForzaRevLimiterTuningConfiguration>
  ) => void;
  export let updateForzaEffect: (id: string, patch: Partial<ForzaEffectConfiguration>) => void = noop as (
    id: string,
    patch: Partial<ForzaEffectConfiguration>
  ) => void;
  export let intensityTooltip: (meta: ForzaEffectMeta, intensity: number) => string = () => '';
  export let routeTooltip: (route: ForzaEffectRoute) => string = () => '';
  export let forzaIntensityPercent: (intensity: number) => number = () => 0;
  export let forzaIntensityFromPercent: (value: number | string) => number = () => 0;

  const inputNumber = (value: number | string, fallback = 0) => {
    const numeric = Number(value);
    return Number.isFinite(numeric) ? numeric : fallback;
  };

  const percentValue = (value: number) => Math.round(value * 100);
  const updateAbsPercent = (
    key: 'brakeThresholdRatio' | 'minStrength' | 'maxStrength',
    value: number | string
  ) => {
    const patch: Partial<ForzaAbsTuningConfiguration> = {};
    patch[key] = inputNumber(value, percentValue(forzaAbsTuning[key])) / 100;
    updateForzaAbsTuning(patch);
  };

  const updateBrakePercent = (
    key: 'baselineForce' | 'normalForce' | 'endstopForce' | 'wallPosition' | 'guardMinEnd' | 'fullForceAt',
    value: number | string
  ) => {
    const patch: Partial<ForzaBrakeTuningConfiguration> = {};
    patch[key] = inputNumber(value, percentValue(forzaBrakeTuning[key])) / 100;
    updateForzaBrakeTuning(patch);
  };

  const updateBrakeNumber = (key: 'endstopBoost' | 'rampCurve', value: number | string) => {
    const patch: Partial<ForzaBrakeTuningConfiguration> = {};
    patch[key] = inputNumber(value, forzaBrakeTuning[key]);
    updateForzaBrakeTuning(patch);
  };

  const updateThrottlePercent = (
    key: 'baselineForce' | 'normalForce' | 'endstopForce' | 'wallPosition' | 'guardMinEnd' | 'rampWidth',
    value: number | string
  ) => {
    const patch: Partial<ForzaThrottleTuningConfiguration> = {};
    patch[key] = inputNumber(value, percentValue(forzaThrottleTuning[key])) / 100;
    updateForzaThrottleTuning(patch);
  };

  const updateThrottleNumber = (key: 'endstopBoost' | 'rampCurve', value: number | string) => {
    const patch: Partial<ForzaThrottleTuningConfiguration> = {};
    patch[key] = inputNumber(value, forzaThrottleTuning[key]);
    updateForzaThrottleTuning(patch);
  };

  const updateShiftPercent = (
    key:
      | 'wallFormAt'
      | 'bodyLowWeight'
      | 'bodyHighWeight'
      | 'clutchThreshold'
      | 'withClutchStrength'
      | 'withoutClutchStrength'
      | 'clutchBodyCut',
    value: number | string
  ) => {
    const patch: Partial<ForzaShiftTuningConfiguration> = {};
    patch[key] = inputNumber(value, percentValue(forzaShiftTuning[key])) / 100;
    updateForzaShiftTuning(patch);
  };

  const updateShiftNumber = (
    key: 'frequencyHz' | 'wallZones' | 'withClutchDurationMs' | 'withoutClutchDurationMs',
    value: number | string
  ) => {
    const patch: Partial<ForzaShiftTuningConfiguration> = {};
    patch[key] = inputNumber(value, forzaShiftTuning[key]);
    updateForzaShiftTuning(patch);
  };

  const updateShiftClutchMode = (mode: ForzaShiftClutchMode) => {
    updateForzaShiftTuning({ clutchMode: mode });
  };

  const selectNumberValue = (event: FocusEvent) => {
    const target = event.target;
    if (!(target instanceof HTMLInputElement) || target.type !== 'number') {
      return;
    }
    requestAnimationFrame(() => target.select());
  };

  const updateRevPercent = (
    key: 'thresholdRatio' | 'minStrength' | 'maxStrength' | 'wallFormThrottleAt' | 'bodyLowWeight' | 'bodyHighWeight',
    value: number | string
  ) => {
    const patch: Partial<ForzaRevLimiterTuningConfiguration> = {};
    patch[key] = inputNumber(value, percentValue(forzaRevLimiterTuning[key])) / 100;
    updateForzaRevLimiterTuning(patch);
  };

  const updateRevNumber = (key: 'frequencyHz' | 'wallZones' | 'curve', value: number | string) => {
    const patch: Partial<ForzaRevLimiterTuningConfiguration> = {};
    patch[key] = inputNumber(value, forzaRevLimiterTuning[key]);
    updateForzaRevLimiterTuning(patch);
  };
</script>

{#if showChrome}
<div class="dm-section-head compact">
  <div>
    <span>Haptic Routing</span>
    <h2>Telemetry Stream</h2>
  </div>
  <div class="dm-effects-count">
    <code>{enabledForzaEffectCount}/{forzaEffectMetas.length}</code>
    {#if showEffects}
      <button
        class:active={advancedOpen}
        class="dm-mini-button dm-advanced-toggle"
        type="button"
        aria-expanded={advancedOpen}
        onclick={() => (advancedOpen = !advancedOpen)}
      >Advanced</button>
    {/if}
    <button
      class:active={allForzaEffectsEnabled}
      class="dm-toggle"
      type="button"
      aria-label="Toggle all effects"
      aria-pressed={allForzaEffectsEnabled}
      onclick={toggleAllForzaEffects}
    ><span></span></button>
  </div>
</div>
{/if}

{#if showChrome}
<div class="dm-body-mode-panel" aria-label="Body rumble source">
  <div class="dm-body-mode-title">
    <span>Body Source</span>
    <code>{forzaBodyRumbleMode === 'native_passthrough' ? 'Native' : 'DSCC'}</code>
  </div>
  <div class="dm-body-mode-toggle" role="radiogroup" aria-label="Forza body rumble mode">
    {#each bodyRumbleModeOptions as option}
      <Tooltip block text={option.help} side="bottom" align="start">
        <button
          class:active={forzaBodyRumbleMode === option.value}
          class="dm-body-mode-option"
          type="button"
          role="radio"
          aria-checked={forzaBodyRumbleMode === option.value}
          onclick={() => setForzaBodyRumbleMode(option.value)}
        >
          <strong>{option.label}</strong>
          <span>{option.badge}</span>
        </button>
      </Tooltip>
    {/each}
  </div>
</div>
{/if}

{#if showEffects}
{#if !showChrome}
  <div class="dm-effects-embedded-head">
    <button
      class:active={advancedOpen}
      class="dm-mini-button dm-advanced-toggle"
      type="button"
      aria-expanded={advancedOpen}
      onclick={() => (advancedOpen = !advancedOpen)}
    >Advanced</button>
  </div>
{/if}
<div
  class:advanced={advancedOpen}
  class="dm-channel-list"
  onfocusin={selectNumberValue}
>
  {#each forzaEffectMetas as meta (meta.id)}
    {@const tuning = forzaEffectsById.get(meta.id) ?? forzaEffect(meta.id)}
    {@const status = effectStatusById.get(meta.id)}
    <article
      class:active={tuning.enabled && status?.state === 'active'}
      class:disabled={!tuning.enabled}
      class="dm-channel-strip"
    >
      <Tooltip text={(tuning.enabled ? 'Disable ' : 'Enable ') + meta.label + '.'} side="right" align="start">
        <button
          class:active={tuning.enabled}
          class="dm-toggle"
          type="button"
          aria-label={meta.label + ' enabled'}
          aria-pressed={tuning.enabled}
          onclick={() => updateForzaEffect(meta.id, { enabled: !tuning.enabled })}
        ><span></span></button>
      </Tooltip>
      <Tooltip block text={meta.help} side="bottom" align="start">
        <div class="dm-channel-name">
          <strong>{meta.label}</strong>
        </div>
      </Tooltip>
      <Tooltip block text={intensityTooltip(meta, tuning.intensity)} side="bottom" align="center">
        <label class="dm-fader">
          <input
            class="dm-range"
            style="--value:{forzaIntensityPercent(tuning.intensity)}%"
            aria-label={meta.label + ' intensity slider'}
            max="100"
            min="0"
            type="range"
            value={forzaIntensityPercent(tuning.intensity)}
            oninput={(event) => updateForzaEffect(meta.id, { intensity: forzaIntensityFromPercent(event.currentTarget.valueAsNumber) })}
          />
          <input
            class="dm-fader-value"
            aria-label={meta.label + ' intensity value'}
            max="100"
            min="0"
            step="1"
            type="number"
            value={forzaIntensityPercent(tuning.intensity)}
            oninput={(event) => updateForzaEffect(meta.id, { intensity: forzaIntensityFromPercent(event.currentTarget.value) })}
          />
        </label>
      </Tooltip>
      <Tooltip block text={routeTooltip(tuning.route)} side="bottom" align="end">
        <label class="dm-route-select-wrap">
          <span>Route</span>
          <select
            class="dm-route-select"
            aria-label={meta.label + ' route'}
            value={tuning.route}
            onchange={(event) => updateForzaEffect(meta.id, { route: event.currentTarget.value as ForzaEffectRoute })}
          >
            {#each forzaRoutes as route}
              <option value={route.value}>{route.label}</option>
            {/each}
          </select>
        </label>
      </Tooltip>
      {#if advancedOpen && meta.id === 'brake_resistance'}
        <div class="dm-channel-advanced dm-effect-advanced-grid" aria-label="Advanced brake pressure tuning">
          <label>
            <span>Initial force %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaBrakeTuning.baselineForce)}
              oninput={(event) => updateBrakePercent('baselineForce', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Pedal force %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaBrakeTuning.normalForce)}
              oninput={(event) => updateBrakePercent('normalForce', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Max input %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaBrakeTuning.endstopForce)}
              oninput={(event) => updateBrakePercent('endstopForce', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Max boost %</span>
            <input
              class="dm-fader-value"
              max="500"
              min="0"
              step="5"
              type="number"
              value={Math.round(forzaBrakeTuning.endstopBoost * 100)}
              oninput={(event) => updateBrakeNumber('endstopBoost', inputNumber(event.currentTarget.value, forzaBrakeTuning.endstopBoost * 100) / 100)}
            />
          </label>
          <label>
            <span>Guard from %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaBrakeTuning.guardMinEnd)}
              oninput={(event) => updateBrakePercent('guardMinEnd', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Wall at %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaBrakeTuning.wallPosition)}
              oninput={(event) => updateBrakePercent('wallPosition', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Full force %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaBrakeTuning.fullForceAt)}
              oninput={(event) => updateBrakePercent('fullForceAt', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Ramp curve</span>
            <input
              class="dm-fader-value"
              max="4"
              min="0.4"
              step="0.05"
              type="number"
              value={forzaBrakeTuning.rampCurve}
              oninput={(event) => updateBrakeNumber('rampCurve', event.currentTarget.value)}
            />
          </label>
        </div>
      {:else if advancedOpen && meta.id === 'abs_slip_pulse'}
        <div class="dm-channel-advanced dm-effect-advanced-grid" aria-label="Advanced ABS tuning">
          <label>
            <span>Mode</span>
            <select
              class="dm-route-select"
              value={forzaAbsTuning.mode}
              onchange={(event) => updateForzaAbsTuning({ mode: event.currentTarget.value as ForzaAbsMode })}
            >
              {#each forzaAbsModeOptions as option}
                <option value={option.value}>{option.label}</option>
              {/each}
            </select>
          </label>
          <label>
            <span>Slip source</span>
            <select
              class="dm-route-select"
              value={forzaAbsTuning.slipSource}
              onchange={(event) => updateForzaAbsTuning({ slipSource: event.currentTarget.value as ForzaAbsSlipSource })}
            >
              {#each forzaAbsSlipSourceOptions as option}
                <option value={option.value}>{option.label}</option>
              {/each}
            </select>
          </label>
          <label>
            <span>Brake at %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaAbsTuning.brakeThresholdRatio)}
              oninput={(event) => updateAbsPercent('brakeThresholdRatio', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Slip trigger</span>
            <input
              class="dm-fader-value"
              max="2"
              min="0.05"
              step="0.01"
              type="number"
              value={forzaAbsTuning.slipThreshold}
              oninput={(event) => updateForzaAbsTuning({ slipThreshold: inputNumber(event.currentTarget.value, forzaAbsTuning.slipThreshold) })}
            />
          </label>
          <label>
            <span>Min speed</span>
            <input
              class="dm-fader-value"
              max="250"
              min="0"
              step="1"
              type="number"
              value={Math.round(forzaAbsTuning.minSpeedKmh)}
              oninput={(event) => updateForzaAbsTuning({ minSpeedKmh: inputNumber(event.currentTarget.value, forzaAbsTuning.minSpeedKmh) })}
            />
          </label>
          <label>
            <span>Min force %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaAbsTuning.minStrength)}
              oninput={(event) => updateAbsPercent('minStrength', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Max force %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaAbsTuning.maxStrength)}
              oninput={(event) => updateAbsPercent('maxStrength', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Frequency</span>
            <input
              class="dm-fader-value"
              max="80"
              min="1"
              step="1"
              type="number"
              value={Math.round(forzaAbsTuning.frequencyHz)}
              oninput={(event) => updateForzaAbsTuning({ frequencyHz: inputNumber(event.currentTarget.value, forzaAbsTuning.frequencyHz) })}
            />
          </label>
          <label>
            <span>Curve</span>
            <input
              class="dm-fader-value"
              max="3"
              min="0.4"
              step="0.05"
              type="number"
              value={forzaAbsTuning.curve}
              oninput={(event) => updateForzaAbsTuning({ curve: inputNumber(event.currentTarget.value, forzaAbsTuning.curve) })}
            />
          </label>
        </div>
      {:else if advancedOpen && meta.id === 'throttle_resistance'}
        <div class="dm-channel-advanced dm-effect-advanced-grid" aria-label="Advanced throttle tuning">
          <label>
            <span>Initial force %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaThrottleTuning.baselineForce)}
              oninput={(event) => updateThrottlePercent('baselineForce', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Pedal force %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaThrottleTuning.normalForce)}
              oninput={(event) => updateThrottlePercent('normalForce', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Max input %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaThrottleTuning.endstopForce)}
              oninput={(event) => updateThrottlePercent('endstopForce', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Max boost %</span>
            <input
              class="dm-fader-value"
              max="500"
              min="0"
              step="5"
              type="number"
              value={Math.round(forzaThrottleTuning.endstopBoost * 100)}
              oninput={(event) => updateThrottleNumber('endstopBoost', inputNumber(event.currentTarget.value, forzaThrottleTuning.endstopBoost * 100) / 100)}
            />
          </label>
          <label>
            <span>Guard from %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaThrottleTuning.guardMinEnd)}
              oninput={(event) => updateThrottlePercent('guardMinEnd', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Wall at %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaThrottleTuning.wallPosition)}
              oninput={(event) => updateThrottlePercent('wallPosition', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Ramp width %</span>
            <input
              class="dm-fader-value"
              max="80"
              min="1"
              step="1"
              type="number"
              value={percentValue(forzaThrottleTuning.rampWidth)}
              oninput={(event) => updateThrottlePercent('rampWidth', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Ramp curve</span>
            <input
              class="dm-fader-value"
              max="4"
              min="0.4"
              step="0.05"
              type="number"
              value={forzaThrottleTuning.rampCurve}
              oninput={(event) => updateThrottleNumber('rampCurve', event.currentTarget.value)}
            />
          </label>
        </div>
      {:else if advancedOpen && meta.id === 'gear_shift_thump'}
        <div class="dm-channel-advanced dm-effect-advanced-grid" aria-label="Advanced shift tuning">
          <label>
            <span>Wall trigger %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaShiftTuning.wallFormAt)}
              oninput={(event) => updateShiftPercent('wallFormAt', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Kick Hz</span>
            <input
              class="dm-fader-value"
              max="80"
              min="1"
              step="1"
              type="number"
              value={Math.round(forzaShiftTuning.frequencyHz)}
              oninput={(event) => updateShiftNumber('frequencyHz', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Wall zones</span>
            <input
              class="dm-fader-value"
              max="8"
              min="1"
              step="1"
              type="number"
              value={Math.round(forzaShiftTuning.wallZones)}
              oninput={(event) => updateShiftNumber('wallZones', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Clutch mode</span>
            <select
              class="dm-fader-value"
              value={forzaShiftTuning.clutchMode}
              onchange={(event) => updateShiftClutchMode(event.currentTarget.value as ForzaShiftClutchMode)}
            >
              <option value="auto">Auto</option>
              <option value="manual_clutch">Manual clutch</option>
              <option value="off">Off</option>
            </select>
          </label>
          <label>
            <span>Clutch bite %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaShiftTuning.clutchThreshold)}
              oninput={(event) => updateShiftPercent('clutchThreshold', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Clutch body damp %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaShiftTuning.clutchBodyCut)}
              oninput={(event) => updateShiftPercent('clutchBodyCut', event.currentTarget.value)}
            />
          </label>
          <p class="dm-advanced-note">
            75-90% gives the clearest clutch uncoupling feel. Lower values are subtle; 95%+ nearly mutes continuous DSCC body rumble.
          </p>
          <label>
            <span>Clean shift %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaShiftTuning.withClutchStrength)}
              oninput={(event) => updateShiftPercent('withClutchStrength', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Clean shift ms</span>
            <input
              class="dm-fader-value"
              max="400"
              min="40"
              step="5"
              type="number"
              value={Math.round(forzaShiftTuning.withClutchDurationMs)}
              oninput={(event) => updateShiftNumber('withClutchDurationMs', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Missed clutch %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaShiftTuning.withoutClutchStrength)}
              oninput={(event) => updateShiftPercent('withoutClutchStrength', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Missed clutch ms</span>
            <input
              class="dm-fader-value"
              max="500"
              min="40"
              step="5"
              type="number"
              value={Math.round(forzaShiftTuning.withoutClutchDurationMs)}
              oninput={(event) => updateShiftNumber('withoutClutchDurationMs', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Body low %</span>
            <input
              class="dm-fader-value"
              max="150"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaShiftTuning.bodyLowWeight)}
              oninput={(event) => updateShiftPercent('bodyLowWeight', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Body high %</span>
            <input
              class="dm-fader-value"
              max="150"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaShiftTuning.bodyHighWeight)}
              oninput={(event) => updateShiftPercent('bodyHighWeight', event.currentTarget.value)}
            />
          </label>
        </div>
      {:else if advancedOpen && meta.id === 'rev_limiter_buzz'}
        <div class="dm-channel-advanced dm-effect-advanced-grid" aria-label="Advanced rev limiter tuning">
          <label>
            <span>RPM threshold %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="50"
              step="1"
              type="number"
              value={percentValue(forzaRevLimiterTuning.thresholdRatio)}
              oninput={(event) => updateRevPercent('thresholdRatio', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Min buzz %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaRevLimiterTuning.minStrength)}
              oninput={(event) => updateRevPercent('minStrength', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Max buzz %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaRevLimiterTuning.maxStrength)}
              oninput={(event) => updateRevPercent('maxStrength', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Buzz Hz</span>
            <input
              class="dm-fader-value"
              max="80"
              min="1"
              step="1"
              type="number"
              value={Math.round(forzaRevLimiterTuning.frequencyHz)}
              oninput={(event) => updateRevNumber('frequencyHz', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Wall trigger %</span>
            <input
              class="dm-fader-value"
              max="100"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaRevLimiterTuning.wallFormThrottleAt)}
              oninput={(event) => updateRevPercent('wallFormThrottleAt', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Wall zones</span>
            <input
              class="dm-fader-value"
              max="8"
              min="1"
              step="1"
              type="number"
              value={Math.round(forzaRevLimiterTuning.wallZones)}
              oninput={(event) => updateRevNumber('wallZones', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Ramp curve</span>
            <input
              class="dm-fader-value"
              max="4"
              min="0.4"
              step="0.05"
              type="number"
              value={forzaRevLimiterTuning.curve}
              oninput={(event) => updateRevNumber('curve', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Body low %</span>
            <input
              class="dm-fader-value"
              max="150"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaRevLimiterTuning.bodyLowWeight)}
              oninput={(event) => updateRevPercent('bodyLowWeight', event.currentTarget.value)}
            />
          </label>
          <label>
            <span>Body high %</span>
            <input
              class="dm-fader-value"
              max="150"
              min="0"
              step="1"
              type="number"
              value={percentValue(forzaRevLimiterTuning.bodyHighWeight)}
              oninput={(event) => updateRevPercent('bodyHighWeight', event.currentTarget.value)}
            />
          </label>
        </div>
      {:else if advancedOpen}
        <div class="dm-channel-advanced dm-channel-signal-strip" aria-label={meta.label + ' advanced status'}>
          <span><strong>Group</strong><code>{meta.group}</code></span>
          <span><strong>Signal</strong><code>{meta.signal}</code></span>
          <span><strong>Status</strong><code>{status?.state ?? 'ready'}</code></span>
          <span><strong>Default route</strong><code>{meta.defaultRoute}</code></span>
        </div>
      {/if}
    </article>
  {/each}
</div>
{/if}
