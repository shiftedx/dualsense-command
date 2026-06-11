<!--
  Saved rail (Task 7; layout truth: tuning-canvas-v9 mockup).

  The rail is furniture: fixed --saved-rail-w wide, sticky, docked right of
  .canvas-grid inside TuningCanvas's .work-and-rail flex wrapper. It never
  participates in column wrapping. Below 900px the rail hides and this
  component instead renders a bottom-docked bar (only while dirty) that
  expands to the full diff list on tap.

  Safety framing (spec §12): Preview feel is always captioned exactly
  "3s · nothing saved"; Save/Discard say in plain words what they touch.
-->
<script lang="ts">
  import type { SavedDiffRow } from './savedDiff';

  let {
    profileName = 'profile',
    rows = [],
    dirtyCount = 0,
    previewActive = false,
    previewBusy = false,
    previewDisabled = false,
    saveBusy = false,
    canSave = false,
    onPreviewFeel = () => {},
    onSave = () => {},
    onDiscard = () => {}
  }: {
    profileName?: string;
    rows?: SavedDiffRow[];
    dirtyCount?: number;
    previewActive?: boolean;
    previewBusy?: boolean;
    previewDisabled?: boolean;
    saveBusy?: boolean;
    canSave?: boolean;
    onPreviewFeel?: () => void | Promise<void>;
    onSave?: () => void | Promise<void>;
    onDiscard?: () => void;
  } = $props();

  let barExpanded = $state(false);

  // Collapse the expanded phone list once everything is saved or discarded.
  $effect(() => {
    if (dirtyCount === 0) barExpanded = false;
  });

  const changeSummary = $derived(`${dirtyCount} unsaved change${dirtyCount === 1 ? '' : 's'}`);
</script>

{#snippet diffRows()}
  {#each rows as item (item.id)}
    <div class="saved-row" class:dirty={item.dirty}>
      <span class="saved-row-label">{item.label}</span>
      <span class="saved-row-value">
        {#if item.dirty}
          <s class="saved-row-was">{item.savedValue}</s>
          <span class="saved-row-now">→ {item.currentValue}</span>
        {:else}
          <span class="saved-row-saved">{item.savedValue}</span>
        {/if}
      </span>
    </div>
  {/each}
{/snippet}

{#snippet actionButtons()}
  <button
    class="saved-save-button"
    type="button"
    disabled={!canSave || saveBusy}
    title={`Writes the current values into ${profileName}. Until then the controller follows your tweaks but the profile keeps its saved values.`}
    onclick={() => void onSave()}
  >{saveBusy ? 'Saving…' : 'Save changes'}</button>
  <button
    class="saved-discard-button"
    type="button"
    disabled={dirtyCount === 0}
    title={`Throws away the tweaks and puts back what ${profileName} has saved.`}
    onclick={onDiscard}
  >Discard</button>
{/snippet}

<aside class="saved-rail" aria-label="Saved profile values">
  <div class="saved-rail-title">Saved in {profileName}</div>
  <div class="saved-rail-rows">
    {@render diffRows()}
  </div>
  <div class="saved-rail-foot">
    <div class="saved-preview-row">
      <button
        class="saved-preview-button"
        class:active={previewActive}
        type="button"
        aria-pressed={previewActive}
        disabled={previewBusy || previewDisabled}
        onclick={() => void onPreviewFeel()}
      >{previewActive ? 'Previewing…' : 'Preview feel'}</button>
      <span class="saved-preview-note">3s · nothing saved</span>
    </div>
    <div class="saved-rail-actions">
      {@render actionButtons()}
    </div>
  </div>
</aside>

{#if dirtyCount > 0}
  <div class="saved-mobile-bar" role="region" aria-label="Unsaved changes">
    {#if barExpanded}
      <div class="saved-mobile-rows">
        {@render diffRows()}
        <div class="saved-preview-row">
          <button
            class="saved-preview-button"
            class:active={previewActive}
            type="button"
            aria-pressed={previewActive}
            disabled={previewBusy || previewDisabled}
            onclick={() => void onPreviewFeel()}
          >{previewActive ? 'Previewing…' : 'Preview feel'}</button>
          <span class="saved-preview-note">3s · nothing saved</span>
        </div>
      </div>
    {/if}
    <div class="saved-mobile-summary">
      <button
        class="saved-mobile-toggle"
        type="button"
        aria-expanded={barExpanded}
        onclick={() => {
          barExpanded = !barExpanded;
        }}
      >
        <strong>{changeSummary}</strong>
        <span>· tap to compare</span>
      </button>
      <div class="saved-mobile-actions">
        {@render actionButtons()}
      </div>
    </div>
  </div>
{/if}
