<script lang="ts">
  import Tooltip from '../../../components/Tooltip.svelte';
  import type {
    ForzaBodyRumbleMode,
    ForzaEffectConfiguration,
    ForzaEffectRoute
  } from '../../types';
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
  const defaultEffect = (id: string): ForzaEffectConfiguration => ({
    id,
    enabled: false,
    intensity: 0,
    route: 'body_both'
  });

  export let enabledForzaEffectCount = 0;
  export let allForzaEffectsEnabled = false;
  export let forzaEffectMetas: ForzaEffectMeta[] = [];
  export let forzaEffectsById: ReadonlyMap<string, ForzaEffectConfiguration> = new Map();
  export let effectStatusById: ReadonlyMap<string, { state?: string }> = new Map();
  export let forzaBodyRumbleMode: ForzaBodyRumbleMode = 'native_passthrough';
  export let bodyRumbleModeOptions: BodyRumbleModeOption[] = [];
  export let forzaRoutes: RouteOption[] = [];
  export let forzaEffect: (id: string) => ForzaEffectConfiguration = defaultEffect;
  export let toggleAllForzaEffects: () => void = noop;
  export let setForzaBodyRumbleMode: (value: ForzaBodyRumbleMode) => void = noop as (value: ForzaBodyRumbleMode) => void;
  export let updateForzaEffect: (id: string, patch: Partial<ForzaEffectConfiguration>) => void = noop as (
    id: string,
    patch: Partial<ForzaEffectConfiguration>
  ) => void;
  export let intensityTooltip: (meta: ForzaEffectMeta, intensity: number) => string = () => '';
  export let routeTooltip: (route: ForzaEffectRoute) => string = () => '';
  export let forzaIntensityPercent: (intensity: number) => number = () => 0;
  export let forzaIntensityFromPercent: (value: number | string) => number = () => 0;
</script>

<div class="dm-section-head compact">
  <div>
    <span>Haptic Routing</span>
    <h2>Telemetry Stream</h2>
  </div>
  <div class="dm-effects-count">
    <code>{enabledForzaEffectCount}/{forzaEffectMetas.length}</code>
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

<div class="dm-channel-list">
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
    </article>
  {/each}
</div>
