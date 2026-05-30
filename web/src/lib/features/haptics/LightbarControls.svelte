<script lang="ts">
  import {
    clampUnit,
    LIGHTBAR_COLOR_PRESETS,
    normalizeTriggerPercent,
    type LightbarColorTarget
  } from './hapticsModel';

  type TuningScope = 'none' | 'global' | 'game';

  const noop = () => undefined;

  export let selectedTuningScope: TuningScope = 'none';
  export let lightbarEnabled = true;
  export let lightbarColor = '#4cc9f0';
  export let rpmColor = '#ff3a2e';
  export let lightbarBrightness = 72;
  export let onColorChange: (target: LightbarColorTarget, color: string) => void = noop as (
    target: LightbarColorTarget,
    color: string
  ) => void;
  export let setLightbarBrightness: (value: number | string) => void = noop as (value: number | string) => void;
  export let setLightbarEnabled: (enabled: boolean) => void = noop as (enabled: boolean) => void;
  export let previewLightbar: () => Promise<void> | void = noop;
  export let previewRpmColor: () => Promise<void> | void = noop;

  const colorPresets = LIGHTBAR_COLOR_PRESETS;

  let pickerOpen = false;
  let pickerTarget: LightbarColorTarget = 'lightbar';
  let pickerHue = 195;
  let pickerSat = 0.7;
  let pickerVal = 0.94;
  let pickerHex = lightbarColor;
  let pickerColor = lightbarColor;
  let pickerEl: HTMLDivElement | undefined;
  let lightbarPillEl: HTMLButtonElement | undefined;
  let rpmPillEl: HTMLButtonElement | undefined;

  $: pickerColor = pickerTarget === 'rpm' ? rpmColor : lightbarColor;
  $: if (!pickerOpen) pickerHex = pickerColor;

  function hsvToHex(h: number, s: number, v: number): string {
    const hh = (((h % 360) + 360) % 360) / 60;
    const c = v * s;
    const x = c * (1 - Math.abs((hh % 2) - 1));
    const m = v - c;
    let r = 0;
    let g = 0;
    let b = 0;
    if (hh < 1) {
      r = c;
      g = x;
    } else if (hh < 2) {
      r = x;
      g = c;
    } else if (hh < 3) {
      g = c;
      b = x;
    } else if (hh < 4) {
      g = x;
      b = c;
    } else if (hh < 5) {
      r = x;
      b = c;
    } else {
      r = c;
      b = x;
    }
    const toHex = (n: number) => Math.round((n + m) * 255).toString(16).padStart(2, '0');
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
  }

  function hexToHsv(hex: string): { h: number; s: number; v: number } | null {
    const match = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
    if (!match) return null;
    const r = parseInt(match[1].slice(0, 2), 16) / 255;
    const g = parseInt(match[1].slice(2, 4), 16) / 255;
    const b = parseInt(match[1].slice(4, 6), 16) / 255;
    const max = Math.max(r, g, b);
    const delta = max - Math.min(r, g, b);
    let h = 0;
    if (delta !== 0) {
      if (max === r) h = ((g - b) / delta) % 6;
      else if (max === g) h = (b - r) / delta + 2;
      else h = (r - g) / delta + 4;
      h *= 60;
      if (h < 0) h += 360;
    }
    return { h, s: max === 0 ? 0 : delta / max, v: max };
  }

  function pickerFallback(target: LightbarColorTarget) {
    return target === 'rpm' ? { h: 4, s: 0.82, v: 1 } : { h: 195, s: 0.7, v: 0.94 };
  }

  function setPickerColor(hex: string) {
    if (pickerTarget === 'rpm') {
      rpmColor = hex;
    } else {
      lightbarColor = hex;
    }
    pickerHex = hex;
    onColorChange(pickerTarget, hex);
  }

  function openPicker(target: LightbarColorTarget = 'lightbar') {
    if (!lightbarEnabled) return;
    pickerTarget = target;
    const color = target === 'rpm' ? rpmColor : lightbarColor;
    const hsv = hexToHsv(color) ?? pickerFallback(target);
    pickerHue = hsv.h;
    pickerSat = hsv.s;
    pickerVal = hsv.v;
    pickerHex = color;
    pickerOpen = true;
  }

  function closePicker() {
    pickerOpen = false;
  }

  function togglePicker(target: LightbarColorTarget = 'lightbar') {
    pickerOpen && pickerTarget === target ? closePicker() : openPicker(target);
  }

  function commitHsv() {
    const hex = hsvToHex(pickerHue, pickerSat, pickerVal);
    setPickerColor(hex);
  }

  function commitPreset(hex: string) {
    setPickerColor(hex);
    const hsv = hexToHsv(hex) ?? { h: 0, s: 0, v: 0 };
    pickerHue = hsv.h;
    pickerSat = hsv.s;
    pickerVal = hsv.v;
  }

  function commitHex() {
    const match = /^#?([0-9a-f]{6})$/i.exec(pickerHex.trim());
    if (!match) {
      pickerHex = pickerColor;
      return;
    }
    const hex = `#${match[1].toLowerCase()}`;
    setPickerColor(hex);
    const hsv = hexToHsv(hex) ?? { h: 0, s: 0, v: 0 };
    pickerHue = hsv.h;
    pickerSat = hsv.s;
    pickerVal = hsv.v;
  }

  function handleHueInput(event: Event) {
    pickerHue = +(event.currentTarget as HTMLInputElement).value;
    commitHsv();
  }

  function handleSvPointer(event: PointerEvent) {
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    const apply = (nextEvent: PointerEvent) => {
      const rect = target.getBoundingClientRect();
      pickerSat = clampUnit((nextEvent.clientX - rect.left) / rect.width);
      pickerVal = 1 - clampUnit((nextEvent.clientY - rect.top) / rect.height);
      commitHsv();
    };
    apply(event);
    const move = (nextEvent: PointerEvent) => apply(nextEvent);
    const up = (nextEvent: PointerEvent) => {
      try {
        target.releasePointerCapture(nextEvent.pointerId);
      } catch {}
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerup', up);
      target.removeEventListener('pointercancel', up);
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerup', up);
    target.addEventListener('pointercancel', up);
  }

  function handleSvKeydown(event: KeyboardEvent) {
    const step = event.shiftKey ? 0.1 : 0.01;
    if (event.key === 'ArrowLeft') pickerSat = clampUnit(pickerSat - step);
    else if (event.key === 'ArrowRight') pickerSat = clampUnit(pickerSat + step);
    else if (event.key === 'ArrowDown') pickerVal = clampUnit(pickerVal - step);
    else if (event.key === 'ArrowUp') pickerVal = clampUnit(pickerVal + step);
    else return;

    event.preventDefault();
    commitHsv();
  }

  function handleHexKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    commitHex();
    closePicker();
  }

  function handleColorDocClick(event: MouseEvent) {
    if (!pickerOpen) return;
    const target = event.target as Node;
    if (pickerEl?.contains(target) || lightbarPillEl?.contains(target) || rpmPillEl?.contains(target)) return;
    closePicker();
  }

  function handleColorKey(event: KeyboardEvent) {
    if (event.key === 'Escape' && pickerOpen) closePicker();
  }
</script>

<svelte:document
  onmousedown={pickerOpen ? handleColorDocClick : undefined}
  onkeydown={pickerOpen ? handleColorKey : undefined}
/>

<div class="dm-rgb-console" aria-label="RGB output controls">
  <div class="dm-console-title">
    <span>RGB Controls</span>
    <strong>{selectedTuningScope === 'global' ? 'Lightbar' : 'Lightbar & Redline'}</strong>
  </div>
  <div class="dm-led-controls">
    <div class="dm-led-row">
      <span>LED</span>
      <div class="ops-lightbar-popover-wrap">
        <button
          bind:this={lightbarPillEl}
          type="button"
          class="dm-color-pill ops-lightbar-preview"
          class:on={lightbarEnabled}
          class:disabled={!lightbarEnabled}
          class:open={pickerOpen && pickerTarget === 'lightbar'}
          aria-label="Lightbar color"
          aria-expanded={pickerOpen && pickerTarget === 'lightbar'}
          aria-haspopup="dialog"
          style="--lb-color: {lightbarColor}; --lb-alpha: {lightbarEnabled ? lightbarBrightness / 100 : 0};"
          onclick={() => togglePicker('lightbar')}
        ><span class="ops-lightbar-glow" aria-hidden="true"></span></button>
        {#if pickerOpen && pickerTarget === 'lightbar'}
          <div bind:this={pickerEl} class="ops-color-popover" role="dialog" aria-label="Lightbar color picker">
            <div
              class="ops-color-sv"
              style="background-color: hsl({pickerHue}, 100%, 50%);"
              role="slider"
              tabindex="0"
              aria-label="Saturation and brightness"
              aria-valuemin="0"
              aria-valuemax="100"
              aria-valuenow={Math.round(pickerVal * 100)}
              aria-valuetext="Saturation {Math.round(pickerSat * 100)}%, brightness {Math.round(pickerVal * 100)}%"
              onpointerdown={handleSvPointer}
              onkeydown={handleSvKeydown}
            >
              <div class="ops-color-sv-overlay"></div>
              <div class="ops-color-sv-cursor" style="left: {pickerSat * 100}%; top: {(1 - pickerVal) * 100}%; background: {pickerHex};"></div>
            </div>
            <input type="range" min="0" max="360" value={pickerHue} oninput={handleHueInput} class="ops-color-hue" aria-label="Hue" />
            <div class="ops-color-row">
              <span class="ops-color-row-swatch" style="background: {pickerHex};"></span>
              <input
                type="text"
                bind:value={pickerHex}
                onchange={commitHex}
                onkeydown={handleHexKeydown}
                maxlength="7"
                class="ops-color-hex"
                aria-label="Hex color"
                spellcheck="false"
              />
            </div>
            <div class="ops-color-presets" role="group" aria-label="Color presets">
              {#each colorPresets as preset (preset)}
                <button
                  type="button"
                  class="ops-color-preset"
                  class:selected={pickerHex.toLowerCase() === preset.toLowerCase()}
                  style="background: {preset};"
                  title={preset}
                  aria-label="Preset {preset}"
                  onclick={() => commitPreset(preset)}
                ></button>
              {/each}
            </div>
          </div>
        {/if}
      </div>
      <input
        class="dm-mini-range"
        style="--value:{lightbarBrightness}%"
        value={lightbarBrightness}
        disabled={!lightbarEnabled}
        max="100"
        min="0"
        type="range"
        aria-label="Lightbar brightness"
        oninput={(event) => setLightbarBrightness(event.currentTarget.valueAsNumber)}
      />
      <code>{normalizeTriggerPercent(lightbarBrightness).toString().padStart(3, '0')}</code>
      <button
        class:active={lightbarEnabled}
        class="dm-toggle"
        type="button"
        aria-label="Toggle lightbar"
        aria-pressed={lightbarEnabled}
        onclick={() => setLightbarEnabled(!lightbarEnabled)}
      ><span></span></button>
      <button class="dm-mini-button" type="button" onclick={previewLightbar}>Preview</button>
    </div>
    {#if selectedTuningScope === 'game'}
      <div class="dm-led-row">
        <span>Redline blink</span>
        <div class="ops-lightbar-popover-wrap">
          <button
            bind:this={rpmPillEl}
            type="button"
            class="dm-color-pill ops-lightbar-preview"
            class:on={lightbarEnabled}
            class:disabled={!lightbarEnabled}
            class:open={pickerOpen && pickerTarget === 'rpm'}
            disabled={!lightbarEnabled}
            aria-label="Redline blink color"
            aria-expanded={pickerOpen && pickerTarget === 'rpm'}
            aria-haspopup="dialog"
            style="--lb-color: {rpmColor}; --lb-alpha: {lightbarEnabled ? lightbarBrightness / 100 : 0};"
            onclick={() => togglePicker('rpm')}
          ><span class="ops-lightbar-glow" aria-hidden="true"></span></button>
          {#if pickerOpen && pickerTarget === 'rpm'}
            <div bind:this={pickerEl} class="ops-color-popover" role="dialog" aria-label="Redline blink color picker">
              <div
                class="ops-color-sv"
                style="background-color: hsl({pickerHue}, 100%, 50%);"
                role="slider"
                tabindex="0"
                aria-label="Saturation and brightness"
                aria-valuemin="0"
                aria-valuemax="100"
                aria-valuenow={Math.round(pickerVal * 100)}
                aria-valuetext="Saturation {Math.round(pickerSat * 100)}%, brightness {Math.round(pickerVal * 100)}%"
                onpointerdown={handleSvPointer}
                onkeydown={handleSvKeydown}
              >
                <div class="ops-color-sv-overlay"></div>
                <div class="ops-color-sv-cursor" style="left: {pickerSat * 100}%; top: {(1 - pickerVal) * 100}%; background: {pickerHex};"></div>
              </div>
              <input type="range" min="0" max="360" value={pickerHue} oninput={handleHueInput} class="ops-color-hue" aria-label="Hue" />
              <div class="ops-color-row">
                <span class="ops-color-row-swatch" style="background: {pickerHex};"></span>
                <input
                  type="text"
                  bind:value={pickerHex}
                  onchange={commitHex}
                  onkeydown={handleHexKeydown}
                  maxlength="7"
                  class="ops-color-hex"
                  aria-label="Hex color"
                  spellcheck="false"
                />
              </div>
              <div class="ops-color-presets" role="group" aria-label="Color presets">
                {#each colorPresets as preset (preset)}
                  <button
                    type="button"
                    class="ops-color-preset"
                    class:selected={pickerHex.toLowerCase() === preset.toLowerCase()}
                    style="background: {preset};"
                    title={preset}
                    aria-label="Preset {preset}"
                    onclick={() => commitPreset(preset)}
                  ></button>
                {/each}
              </div>
            </div>
          {/if}
        </div>
        <input
          class="dm-mini-range"
          style="--value:{lightbarBrightness}%"
          value={lightbarBrightness}
          disabled={!lightbarEnabled}
          max="100"
          min="0"
          type="range"
          aria-label="Redline blink brightness"
          oninput={(event) => setLightbarBrightness(event.currentTarget.valueAsNumber)}
        />
        <code>{normalizeTriggerPercent(lightbarBrightness).toString().padStart(3, '0')}</code>
        <button
          class:active={lightbarEnabled}
          class="dm-toggle"
          type="button"
          aria-label="Toggle redline blink"
          aria-pressed={lightbarEnabled}
          onclick={() => setLightbarEnabled(!lightbarEnabled)}
        ><span></span></button>
        <button class="dm-mini-button" type="button" onclick={previewRpmColor}>Preview</button>
      </div>
    {/if}
  </div>
</div>
