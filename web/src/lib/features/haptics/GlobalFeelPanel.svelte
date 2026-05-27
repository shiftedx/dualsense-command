<script lang="ts">
  import Tooltip from '../../../components/Tooltip.svelte';

  type PatternOption = {
    label: string;
    badge?: string;
  };

  const noop = () => undefined;

  export let snapshot: unknown = null;
  export let baseFeelTestActive = false;
  export let baseFeelTestBusy = false;
  export let triggerEffect = 'Adaptive resistance';
  export let triggerIntensity = 'Strong (Standard)';
  export let vibrationIntensity = 'Medium';
  export let vibrationMode = 'Balanced';
  export let triggerEffectOptions: PatternOption[] = [];
  export let vibrationModeOptions: PatternOption[] = [];
  export let triggerEffectHelp: Record<string, string> = {};
  export let vibrationModeHelp: Record<string, string> = {};
  export let setTriggerEffect: (value: string) => void = noop as (value: string) => void;
  export let setVibrationIntensity: (value: string) => void = noop as (value: string) => void;
  export let setVibrationMode: (value: string) => void = noop as (value: string) => void;
  export let toggleBaseFeelTest: () => Promise<void> | void = noop;
  export let previewBodyHaptics: () => Promise<void> | void = noop;
</script>

<div class="dm-section-head compact">
  <div>
    <span>Controller Feel</span>
    <h2>Base Haptics</h2>
  </div>
</div>
<div class="dm-global-feel-panel">
  <article>
    <div class="dm-global-feel-heading">
      <strong>Trigger pattern</strong>
      <code>{triggerIntensity}</code>
    </div>
    <span>L2 and R2 use the selected hardware pattern with the curves configured on the left.</span>
    <div class="dm-pattern-grid" aria-label="Trigger haptic pattern">
      {#each triggerEffectOptions as option}
        <Tooltip block text={triggerEffectHelp[option.label] ?? 'Selects the base adaptive trigger behavior.'} side="bottom" align="start">
          <button
            class:active={triggerEffect === option.label}
            class="dm-pattern-option"
            type="button"
            aria-pressed={triggerEffect === option.label}
            onclick={() => setTriggerEffect(option.label)}
          >
            <strong>{option.label}</strong>
            <span>{option.badge}</span>
          </button>
        </Tooltip>
      {/each}
    </div>
    <button class:active={baseFeelTestActive} class="dm-test-button" type="button" disabled={baseFeelTestBusy || !snapshot} onclick={() => void toggleBaseFeelTest()}>
      {baseFeelTestActive ? 'Stop Preview' : 'Preview Triggers'}
    </button>
  </article>
  <article>
    <div class="dm-global-feel-heading">
      <strong>Body haptics</strong>
      <code>{vibrationMode}</code>
    </div>
    <span>Global profiles keep game telemetry off while storing controller-level body strength and motor blend.</span>
    <div class="dm-global-feel-controls">
      <label>
        <span>Strength</span>
        <select value={vibrationIntensity} onchange={(event) => setVibrationIntensity(event.currentTarget.value)}>
          <option>Off</option><option>Low</option><option>Medium</option><option>High</option>
        </select>
      </label>
      <label>
        <span>Motor blend</span>
        <select value={vibrationMode} onchange={(event) => setVibrationMode(event.currentTarget.value)}>
          {#each vibrationModeOptions as option}
            <option>{option.label}</option>
          {/each}
        </select>
      </label>
    </div>
    <div class="dm-vibration-mode-grid" aria-label="Body haptic character">
      {#each vibrationModeOptions as option}
        <Tooltip block text={vibrationModeHelp[option.label] ?? 'Controls the body haptic motor blend.'} side="bottom" align="start">
          <button
            class:active={vibrationMode === option.label}
            class="dm-pattern-option"
            type="button"
            aria-pressed={vibrationMode === option.label}
            onclick={() => setVibrationMode(option.label)}
          >
            <strong>{option.label}</strong>
            <span>{option.badge}</span>
          </button>
        </Tooltip>
      {/each}
    </div>
    <button class="dm-test-button" type="button" disabled={!snapshot || vibrationIntensity === 'Off'} onclick={() => void previewBodyHaptics()}>
      Preview Body
    </button>
  </article>
</div>
