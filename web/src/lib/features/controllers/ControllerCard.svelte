<script lang="ts">
  import type { ControllerStatus } from '../../types';
  import {
    controllerBatteryDetail,
    controllerBatteryFillWidth,
    controllerBatteryReadable,
    controllerBatteryText,
    controllerConnectionText,
    controllerDiagnosticDetail,
    controllerModelText,
    controllerPermissionDetail,
    controllerTransportDetail,
    shortControllerId
  } from '../../controllerDisplay';

  export let item: ControllerStatus;
  export let index = 0;
  export let selected = false;
  export let renameActive = false;
  export let renameName = '';
  export let renameBusy = false;
  export let onSelect: (controllerId: string) => void = () => {};
  export let onBeginRename: (item: ControllerStatus) => void = () => {};
  export let onSubmitRename: () => void | Promise<void> = () => {};
  export let onCancelRename: () => void = () => {};
  export let onRenameKeydown: (event: KeyboardEvent) => void = () => {};

  let expanded = false;

  const toggleDetails = () => {
    expanded = !expanded;
  };
</script>

<article
  class="dm-controller-card"
  class:active={selected}
  class:disconnected={!item.connected}
>
  <button
    class="dm-controller-select-zone"
    type="button"
    aria-pressed={selected}
    onclick={() => onSelect(item.id)}
  >
    <span class="dm-controller-card-top">
      <code>{index + 1}</code>
      {#if controllerBatteryReadable(item)}
        <span class="dm-battery-pill compact">
          <svg class="dm-battery" viewBox="0 0 32 16" aria-hidden="true">
            <rect x="1" y="3" width="26" height="10" rx="2" />
            <path d="M28 6h2.5v4H28z" />
            <rect class="dm-battery-fill" x="4" y="5.5" width={controllerBatteryFillWidth(item)} height="5" rx="1" />
          </svg>
          <span>{controllerBatteryText(item)}</span>
        </span>
      {/if}
    </span>
    <span class="dm-controller-glyph controller-card" aria-hidden="true"></span>
    <span class="dm-controller-copy">
      <strong>{controllerModelText(item)}</strong>
      <small>{controllerConnectionText(item)}</small>
      <small class="dm-controller-id" title={item.id}>{shortControllerId(item.id)}</small>
      <span class="dm-controller-capabilities" aria-hidden="true">
        {#each item.capabilities.slice(0, 3) as capability}
          <em>{capability}</em>
        {/each}
      </span>
    </span>
  </button>
  {#if renameActive}
    <span class="dm-controller-rename-wrap">
      <input
        bind:value={renameName}
        class="dm-controller-rename-input"
        disabled={renameBusy}
        maxlength="64"
        spellcheck="false"
        aria-label="Controller name"
        onclick={(event) => event.stopPropagation()}
        onkeydown={onRenameKeydown}
      />
      <span class="dm-controller-rename-actions">
        <button type="button" disabled={renameBusy || !renameName.trim()} onclick={(event) => { event.stopPropagation(); void onSubmitRename(); }}>Save</button>
        <button type="button" disabled={renameBusy} onclick={(event) => { event.stopPropagation(); onCancelRename(); }}>Cancel</button>
      </span>
    </span>
  {:else}
    <span class="dm-controller-card-actions">
      <button class="dm-controller-rename-button" type="button" onclick={() => onBeginRename(item)}>Rename</button>
      <button
        class="dm-controller-expand-button"
        type="button"
        aria-expanded={expanded}
        aria-controls={`dm-controller-details-${item.id}`}
        onclick={toggleDetails}
      >{expanded ? 'Hide details' : 'Show details'}</button>
    </span>
  {/if}
  {#if expanded}
    <div class="dm-controller-details" id={`dm-controller-details-${item.id}`}>
      <dl class="dm-controller-details-grid">
        <div>
          <dt>Family</dt>
          <dd>{item.family}</dd>
        </div>
        <div>
          <dt>Connection</dt>
          <dd>{controllerTransportDetail(item)}</dd>
        </div>
        <div>
          <dt>Battery</dt>
          <dd>{controllerBatteryDetail(item)}</dd>
        </div>
        <div>
          <dt>Permission</dt>
          <dd>{controllerPermissionDetail(item)}</dd>
        </div>
        <div>
          <dt>Diagnostics</dt>
          <dd>{controllerDiagnosticDetail(item)}</dd>
        </div>
        <div class="wide">
          <dt>Sanitized ID</dt>
          <dd class="mono">{item.id}</dd>
        </div>
        {#if item.capabilities.length}
          <div class="wide">
            <dt>Capabilities ({item.capabilities.length})</dt>
            <dd>
              <span class="dm-controller-capabilities expanded">
                {#each item.capabilities as capability}
                  <em>{capability}</em>
                {/each}
              </span>
            </dd>
          </div>
        {/if}
      </dl>
    </div>
  {/if}
</article>
