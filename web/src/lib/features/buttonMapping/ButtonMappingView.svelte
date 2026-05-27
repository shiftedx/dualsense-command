<script lang="ts">
  import { ChevronDown, Keyboard, Search, Wand2 } from '@lucide/svelte';
  import Tooltip from '../../../components/Tooltip.svelte';
  import {
    chipDisplayLabel,
    parseSteamBindingTriple,
    steamBindingTargetPart,
    steamSlotIconUrl
  } from './buttonMapping';
  import type { SteamBindingSlot, SteamMirrorGroup, SteamMirrorRow } from './buttonMapping';
  import type { ControllerStatus, SteamInputBinding } from '../../types';

  type PreparedSteamBindingTargetGroup = {
    label: string;
    options: Array<{ label: string; raw: string; targetKey: string; searchText: string }>;
  };

  export let active = false;
  export let steamInputRunning = false;
  export let providerLabel = 'Steam Input';
  export let providerKind: 'steam' | 'bridge' = 'steam';
  export let providerOnline = false;
  export let mappingAvailabilityMessage = '';
  export let mappingReadOnly = false;
  export let defaultMirrorOnly = false;
  export let controllerHeaderName = '';
  export let controllerTransport: ControllerStatus['transport'] | undefined = undefined;
  export let gameName = 'No supported game selected';
  export let steamLayoutTitle = 'Steam Input Layout';
  export let mappedVisibleChipCount = 0;
  export let steamMirrorGroups: SteamMirrorGroup[] = [];
  export let focusedSlotMeta: SteamBindingSlot | null = null;
  export let focusedSlotBinding: SteamInputBinding | null = null;
  export let focusedSlotSelectedBinding: SteamInputBinding | null = null;
  export let steamBindingBusy = false;
  export let steamInputLayoutAvailable = false;
  export let paddlePresetVisible = false;
  export let paddlePresetAvailable = false;
  export let paddlePresetStatus = '';
  export let paddlePresetLeftKey = 'Q';
  export let paddlePresetRightKey = 'E';
  export let steamBindingDraft = '';
  export let steamBindingLabelDraft = '';
  export let bindingLabelFieldLabel = 'Label (Steam UI)';
  export let rawFieldLabel = 'Raw VDF';
  export let rawFieldPlaceholder = 'xinput_button ... / key_press ...';
  export let targetGroups: PreparedSteamBindingTargetGroup[] = [];
  export let onSelectSlot: (slot: SteamBindingSlot) => void = () => {};
  export let onHoverSlot: (slot: SteamBindingSlot | null) => void = () => {};
  export let onPaddlePresetLeftKeyChange: (nextKey: string) => void = () => {};
  export let onPaddlePresetRightKeyChange: (nextKey: string) => void = () => {};
  export let onApplyPaddlePreset: () => void | Promise<void> = () => {};
  export let onTargetChange: (rawOption: string) => void = () => {};
  export let onLabelChange: (nextLabel: string) => void = () => {};
  export let onRawDraftChange: (nextRaw: string) => void = () => {};
  export let onResetDraft: () => void = () => {};
  export let onSaveBinding: () => void | Promise<void> = () => {};

  let targetPickerOpen = false;
  let targetSearchQuery = '';
  let targetSearchInputEl: HTMLInputElement | null = null;

  $: leftMirrorGroups = steamMirrorGroups.filter((group) => group.placement === 'left');
  $: centerMirrorGroups = steamMirrorGroups.filter((group) => group.placement === 'center');
  $: rightMirrorGroups = steamMirrorGroups.filter((group) => group.placement === 'right');
  $: bottomMirrorGroups = steamMirrorGroups.filter((group) => group.placement === 'bottom');
  $: mirroredInputCount = steamMirrorGroups.reduce((count, group) => count + group.rows.length, 0);
  $: noMirrorAvailable = mirroredInputCount === 0;
  $: currentSteamBindingTargetKey = steamBindingDraft ? steamBindingTargetPart(steamBindingDraft) : '';
  $: filteredTargetGroups = (() => {
    const q = targetSearchQuery.trim().toLowerCase();
    if (!q) return targetGroups;
    return targetGroups
      .map((group) => {
        const groupMatches = group.label.toLowerCase().includes(q);
        const options = groupMatches
          ? group.options
          : group.options.filter((option) => option.searchText.includes(q));
        return { ...group, options };
      })
      .filter((group) => group.options.length > 0);
  })();

  const currentTargetLabel = (): string => {
    if (!steamBindingDraft) return 'Select target…';
    for (const group of targetGroups) {
      for (const option of group.options) {
        if (option.targetKey === currentSteamBindingTargetKey) return option.label;
      }
    }
    const { command, param } = parseSteamBindingTriple(steamBindingDraft);
    if (!command) return 'Select target…';
    return param ? `Custom: ${command} ${param}` : `Custom: ${command}`;
  };

  const openTargetPicker = () => {
    targetPickerOpen = true;
    targetSearchQuery = '';
    queueMicrotask(() => targetSearchInputEl?.focus());
  };

  const closeTargetPicker = () => {
    targetPickerOpen = false;
    targetSearchQuery = '';
  };

  const toggleTargetPicker = () => {
    if (targetPickerOpen) closeTargetPicker();
    else openTargetPicker();
  };

  const pickTargetOption = (rawOption: string) => {
    onTargetChange(rawOption);
    closeTargetPicker();
  };

  const selectMirrorRow = (row: SteamMirrorRow) => {
    onSelectSlot(row.slot);
  };

  const handleTargetPickerKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      closeTargetPicker();
    }
  };

  function clickOutside(node: HTMLElement, callback: () => void) {
    const onMouseDown = (event: MouseEvent) => {
      if (!node.contains(event.target as Node)) callback();
    };
    document.addEventListener('mousedown', onMouseDown);
    return {
      destroy() {
        document.removeEventListener('mousedown', onMouseDown);
      }
    };
  }
</script>

<section
  class:dm-view-hidden={!active}
  class="dm-button-map-page"
  aria-label="Button mapping workspace"
  aria-hidden={!active}
>
  <header class="dm-mapping-header">
    <div class="dm-mapping-titleblock">
      <span class="dm-mapping-eyebrow">
        {providerOnline || steamInputRunning ? `${providerLabel} · Online` : `${providerLabel} · Offline`}
        <em>·</em>
        {controllerHeaderName.toUpperCase()}
        {#if controllerTransport && controllerTransport !== 'Unknown'}
          <em>·</em>
          {controllerTransport}
        {/if}
      </span>
      <h2>Customize Button Assignments</h2>
    </div>
    <p class="dm-mapping-context">
      <strong>{gameName}</strong>
      <em>· {steamLayoutTitle}</em>
      {#if !defaultMirrorOnly && mirroredInputCount > 0}
        <em class="dm-mapping-context-count">· {mappedVisibleChipCount}/{mirroredInputCount} inputs mapped</em>
      {/if}
    </p>
    {#if defaultMirrorOnly}
      <p class="dm-mapping-provider-note">Default mirror only. No writable {providerLabel} layout is loaded for this game/app yet.</p>
    {/if}
    {#if mappingAvailabilityMessage}
      <p class="dm-mapping-provider-note">{mappingAvailabilityMessage}</p>
    {/if}
  </header>

  {#if paddlePresetVisible}
    <section class="dm-paddle-preset" aria-label="Steam Input paddle shift preset">
      <div class="dm-paddle-preset-title">
        <Keyboard size={15} aria-hidden="true" />
        <div>
          <span>Steam Input / PC only</span>
          <strong>Paddle Shift</strong>
          <em>Onboard Fn profiles are unchanged.</em>
        </div>
      </div>
      <label class="dm-paddle-key-field">
        <span>Back Left</span>
        <input
          value={paddlePresetLeftKey}
          maxlength="32"
          spellcheck="false"
          autocomplete="off"
          disabled={steamBindingBusy}
          aria-label="Back Left paddle keyboard key"
          oninput={(event) => onPaddlePresetLeftKeyChange((event.currentTarget as HTMLInputElement).value)}
        />
      </label>
      <label class="dm-paddle-key-field">
        <span>Back Right</span>
        <input
          value={paddlePresetRightKey}
          maxlength="32"
          spellcheck="false"
          autocomplete="off"
          disabled={steamBindingBusy}
          aria-label="Back Right paddle keyboard key"
          oninput={(event) => onPaddlePresetRightKeyChange((event.currentTarget as HTMLInputElement).value)}
        />
      </label>
      <Tooltip text={paddlePresetStatus} side="bottom" align="end">
        <button
          class="dm-paddle-preset-action"
          type="button"
          disabled={steamBindingBusy || !paddlePresetAvailable || !paddlePresetLeftKey.trim() || !paddlePresetRightKey.trim()}
          onclick={() => void onApplyPaddlePreset()}
        >
          <Wand2 size={14} aria-hidden="true" />
          <span>{steamBindingBusy ? 'Saving' : 'Apply'}</span>
        </button>
      </Tooltip>
    </section>
  {/if}

  <div class="dm-steam-mirror" aria-label="Steam Input controller layout mirror">
    <div class="dm-steam-rail left">
      {#each leftMirrorGroups as group (group.key)}
        <section class="dm-steam-group" aria-label={group.label}>
          {#if group.label !== 'Left Controls'}
            <span class="dm-steam-group-title">{group.label}</span>
          {/if}
          {#each group.staticRows ?? [] as label (label)}
            <span class="dm-steam-static-row">{label}</span>
          {/each}
          {#each group.rows as row (row.key)}
            <button
              type="button"
              class="dm-steam-row"
              class:active={row.selected}
              onclick={() => selectMirrorRow(row)}
              onmouseenter={() => onHoverSlot(row.slot)}
              onmouseleave={() => onHoverSlot(null)}
              onfocus={() => onHoverSlot(row.slot)}
              onblur={() => onHoverSlot(null)}
              aria-label="{row.slot.label}: {row.displayLabel}"
            >
              <strong>{row.displayLabel}</strong>
              <span class="dm-steam-input-icon">
                {#if row.iconUrl}
                  <img src={row.iconUrl} alt="" aria-hidden="true" />
                {:else}
                  {row.slot.label.slice(0, 2).toUpperCase()}
                {/if}
              </span>
            </button>
          {/each}
        </section>
      {/each}
    </div>

    <div class="dm-steam-center">
      <strong class="dm-steam-layout-title">{noMirrorAvailable ? 'Controller Base Layout' : steamLayoutTitle}</strong>
      {#if noMirrorAvailable}
        <p class="dm-steam-empty-note">
          No {providerLabel} layout in this scope. Select a game from Profiles to remap inputs against the active provider.
        </p>
      {:else if defaultMirrorOnly}
        <p class="dm-steam-empty-note">
          Showing DSCC's default controller mirror for orientation. Apply stays disabled until a writable provider layout is available.
        </p>
      {/if}
      <div class="dm-steam-controller-art">
        <img class="dm-controller-base" src="/dualsense/controller_front.png" alt="DualSense controller front view" />
      </div>
      {#each centerMirrorGroups as group (group.key)}
        <section class="dm-steam-group center" aria-label={group.label}>
          <span class="dm-steam-group-title">{group.label}</span>
          {#each group.staticRows ?? [] as label (label)}
            <span class="dm-steam-static-row">{label}</span>
          {/each}
          {#each group.rows as row (row.key)}
            <button
              type="button"
              class="dm-steam-row center"
              class:active={row.selected}
              onclick={() => selectMirrorRow(row)}
              onmouseenter={() => onHoverSlot(row.slot)}
              onmouseleave={() => onHoverSlot(null)}
              onfocus={() => onHoverSlot(row.slot)}
              onblur={() => onHoverSlot(null)}
              aria-label="{row.slot.label}: {row.displayLabel}"
            >
              <span class="dm-steam-input-icon">
                {#if row.iconUrl}
                  <img src={row.iconUrl} alt="" aria-hidden="true" />
                {:else}
                  {row.slot.label.slice(0, 2).toUpperCase()}
                {/if}
              </span>
              <strong>{row.displayLabel}</strong>
            </button>
          {/each}
        </section>
      {/each}
    </div>

    <div class="dm-steam-rail right">
      {#each rightMirrorGroups as group (group.key)}
        <section class="dm-steam-group" aria-label={group.label}>
          {#if group.label !== 'Right Controls'}
            <span class="dm-steam-group-title">{group.label}</span>
          {/if}
          {#each group.staticRows ?? [] as label (label)}
            <span class="dm-steam-static-row">{label}</span>
          {/each}
          {#each group.rows as row (row.key)}
            <button
              type="button"
              class="dm-steam-row"
              class:active={row.selected}
              onclick={() => selectMirrorRow(row)}
              onmouseenter={() => onHoverSlot(row.slot)}
              onmouseleave={() => onHoverSlot(null)}
              onfocus={() => onHoverSlot(row.slot)}
              onblur={() => onHoverSlot(null)}
              aria-label="{row.slot.label}: {row.displayLabel}"
            >
              <span class="dm-steam-input-icon">
                {#if row.iconUrl}
                  <img src={row.iconUrl} alt="" aria-hidden="true" />
                {:else}
                  {row.slot.label.slice(0, 2).toUpperCase()}
                {/if}
              </span>
              <strong>{row.displayLabel}</strong>
            </button>
          {/each}
        </section>
      {/each}
    </div>

    <div class="dm-steam-bottom-grid">
      {#each bottomMirrorGroups as group (group.key)}
        <section class="dm-steam-group bottom" aria-label={group.label}>
          <span class="dm-steam-group-title">{group.label}</span>
          {#each group.staticRows ?? [] as label (label)}
            <span class="dm-steam-static-row">{label}</span>
          {/each}
          {#each group.rows as row (row.key)}
            <button
              type="button"
              class="dm-steam-row compact"
              class:active={row.selected}
              onclick={() => selectMirrorRow(row)}
              onmouseenter={() => onHoverSlot(row.slot)}
              onmouseleave={() => onHoverSlot(null)}
              onfocus={() => onHoverSlot(row.slot)}
              onblur={() => onHoverSlot(null)}
              aria-label="{row.slot.label}: {row.displayLabel}"
            >
              <span class="dm-steam-input-icon">
                {#if row.iconUrl}
                  <img src={row.iconUrl} alt="" aria-hidden="true" />
                {:else}
                  {row.slot.label.slice(0, 2).toUpperCase()}
                {/if}
              </span>
              <strong>{row.displayLabel}</strong>
            </button>
          {/each}
        </section>
      {/each}
    </div>
  </div>

  <div class="dm-mapping-tray" class:populated={Boolean(focusedSlotMeta)}>
    <div class="dm-mapping-tray-info">
      {#if focusedSlotMeta}
        {@const focusedIconUrl = steamSlotIconUrl(focusedSlotMeta.key)}
        {#if focusedIconUrl}
          <img class="dm-key-icon lg" src={focusedIconUrl} alt="" aria-hidden="true" />
        {:else}
          <span class="dm-key-icon lg placeholder" aria-hidden="true">{focusedSlotMeta.label.slice(0, 2).toUpperCase()}</span>
        {/if}
        <div class="dm-mapping-tray-labels">
          <span>{focusedSlotMeta.group}</span>
          <strong>{focusedSlotMeta.label}</strong>
          <em>{chipDisplayLabel(focusedSlotBinding)}</em>
        </div>
      {:else}
        <div class="dm-mapping-tray-labels">
          <span>Select an input</span>
          <strong>Hover or click any chip to edit its {providerLabel} binding</strong>
        </div>
      {/if}
    </div>

    {#if focusedSlotMeta && focusedSlotSelectedBinding}
      <div class="dm-mapping-tray-controls">
        <div class="dm-mapping-tray-field">
          <span>Target</span>
          <div class="dm-target-combo" use:clickOutside={closeTargetPicker}>
            <button
              type="button"
              class="dm-target-combo-trigger"
              class:open={targetPickerOpen}
              disabled={steamBindingBusy || mappingReadOnly}
              onclick={toggleTargetPicker}
              aria-haspopup="listbox"
              aria-expanded={targetPickerOpen}
            >
              <span class="dm-target-combo-value">{currentTargetLabel()}</span>
              <ChevronDown size={14} aria-hidden="true" />
            </button>
            {#if targetPickerOpen}
              <div class="dm-target-combo-panel" onkeydown={handleTargetPickerKeydown} role="listbox" tabindex="-1">
                <div class="dm-target-combo-searchbar">
                  <Search size={13} aria-hidden="true" />
                  <input
                    bind:this={targetSearchInputEl}
                    bind:value={targetSearchQuery}
                    type="search"
                    spellcheck="false"
                    placeholder="Search bindings…"
                    aria-label="Search Steam Input bindings"
                  />
                </div>
                <div class="dm-target-combo-list">
                  {#each filteredTargetGroups as group (group.label)}
                    <div class="dm-target-combo-group">{group.label}</div>
                    {#each group.options as option (option.raw)}
                      <button
                        type="button"
                        class="dm-target-combo-option"
                        class:active={option.targetKey === currentSteamBindingTargetKey}
                        onclick={() => pickTargetOption(option.raw)}
                        role="option"
                        aria-selected={option.targetKey === currentSteamBindingTargetKey}
                      >
                        {option.label}
                      </button>
                    {/each}
                  {:else}
                    <div class="dm-target-combo-empty">
                      No matches for <strong>{targetSearchQuery}</strong>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        </div>
        <label class="dm-mapping-tray-field">
          <span>{bindingLabelFieldLabel}</span>
          <input
            value={steamBindingLabelDraft}
            oninput={(event) => onLabelChange((event.currentTarget as HTMLInputElement).value)}
            disabled={steamBindingBusy || mappingReadOnly}
            spellcheck="false"
            placeholder="e.g. Next radio station"
          />
        </label>
        <label class="dm-mapping-tray-field grow">
          <span>{rawFieldLabel}</span>
          <input
            value={steamBindingDraft}
            oninput={(event) => onRawDraftChange((event.currentTarget as HTMLInputElement).value)}
            disabled={steamBindingBusy || mappingReadOnly}
            spellcheck="false"
            placeholder={rawFieldPlaceholder}
          />
        </label>
      </div>
    {/if}

    <div class="dm-mapping-tray-actions">
      <button
        class="dm-mapping-action ghost"
        type="button"
        disabled={!focusedSlotSelectedBinding || steamBindingBusy || mappingReadOnly}
        onclick={onResetDraft}
      >Reset</button>
      <button
        class="dm-mapping-action primary"
        type="button"
        disabled={steamBindingBusy || mappingReadOnly || !steamInputLayoutAvailable || !focusedSlotSelectedBinding || (providerKind !== 'bridge' && focusedSlotSelectedBinding.synthetic)}
        onclick={() => void onSaveBinding()}
      >Apply</button>
    </div>
  </div>
</section>
