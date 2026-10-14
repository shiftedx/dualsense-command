<script lang="ts">
  import { ChevronRight, FileCode, Folder, X } from '@lucide/svelte';
  import type { SteamLibraryBrowseEntry, SteamLibraryEntry } from '../../types';
  import {
    formatBrowseSize,
    isExecutableSelected,
    type SteamBrowseCrumb,
    type SteamExecutableSelection
  } from './addGameModel';

  export let entry: SteamLibraryEntry;
  export let browsePath = '';
  export let browseEntries: SteamLibraryBrowseEntry[] = [];
  export let browseTruncated = false;
  export let browseLoading = false;
  export let browseError = '';
  export let selectedExes: SteamExecutableSelection[] = [];
  export let busyAppId = '';
  export let breadcrumbs: SteamBrowseCrumb[] = [];
  export let onCancel: () => void = () => {};
  export let onLoadPath: (path: string) => void | Promise<void> = () => {};
  export let onBrowseInto: (entry: SteamLibraryBrowseEntry) => void = () => {};
  export let onToggleExe: (entry: SteamLibraryBrowseEntry) => void = () => {};
  export let onRemoveSelected: (name: string) => void = () => {};
  export let onSubmit: () => void = () => {};
</script>

<div
  class="dm-add-game-backdrop dm-add-game-pick-backdrop"
  role="presentation"
  onclick={(event) => { if (event.target === event.currentTarget) onCancel(); }}
  onkeydown={(event) => { if (event.key === 'Escape') { event.preventDefault(); onCancel(); } }}
>
  <div
    class="dm-add-game-dialog dm-add-game-pick-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="dm-add-game-pick-title"
    tabindex="-1"
  >
    <header class="dm-add-game-head">
      <div>
        <span>Select executables</span>
        <h2 id="dm-add-game-pick-title">{entry.name}</h2>
        <p>
          Browse the install folder and tick the .exe(s) DSCC should watch for. When any of
          those processes start, the profile you tune for this game auto-loads on the
          controller.
        </p>
      </div>
      <button type="button" class="dm-add-game-close" aria-label="Close" onclick={onCancel}>
        <X size={16} />
      </button>
    </header>

    <nav class="dm-add-game-breadcrumbs" aria-label="Folder breadcrumb">
      {#each breadcrumbs as crumb, index (crumb.path)}
        {#if index > 0}
          <ChevronRight size={12} aria-hidden="true" />
        {/if}
        <button
          type="button"
          class="dm-add-game-breadcrumb"
          class:current={crumb.path === browsePath}
          disabled={crumb.path === browsePath || browseLoading}
          onclick={() => void onLoadPath(crumb.path)}
        >{crumb.label}</button>
      {/each}
    </nav>

    <div class="dm-add-game-browse-body">
      {#if browseError}
        <div class="dm-add-game-error" role="alert">{browseError}</div>
      {/if}

      <div class="dm-add-game-browse-list" role="listbox" aria-label="Folder contents">
        {#if browseLoading}
          <div class="dm-add-game-browse-empty">Reading folder&hellip;</div>
        {:else if browseEntries.length === 0}
          <div class="dm-add-game-browse-empty">
            {browsePath ? 'No folders or .exe files here.' : 'Install folder is empty or unreadable.'}
          </div>
        {:else}
          {#each browseEntries as browseEntry (browseEntry.name + browseEntry.kind)}
            {@const selected = isExecutableSelected(selectedExes, browseEntry)}
            <button
              type="button"
              class="dm-add-game-browse-row"
              class:dir={browseEntry.kind === 'dir'}
              class:exe={browseEntry.kind === 'exe'}
              class:selected
              onclick={() => {
                if (browseEntry.kind === 'dir') {
                  onBrowseInto(browseEntry);
                } else {
                  onToggleExe(browseEntry);
                }
              }}
            >
              <span class="dm-add-game-browse-icon" aria-hidden="true">
                {#if browseEntry.kind === 'dir'}
                  <Folder size={14} />
                {:else}
                  <FileCode size={14} />
                {/if}
              </span>
              <span class="dm-add-game-browse-name">{browseEntry.name}</span>
              {#if browseEntry.kind === 'exe' && browseEntry.sizeBytes}
                <span class="dm-add-game-browse-meta">{formatBrowseSize(browseEntry.sizeBytes)}</span>
              {/if}
              {#if browseEntry.kind === 'dir'}
                <ChevronRight size={12} class="dm-add-game-browse-affordance" aria-hidden="true" />
              {:else if selected}
                <span class="dm-add-game-browse-affordance selected-pill">Selected</span>
              {:else}
                <span class="dm-add-game-browse-affordance hint-pill">Select</span>
              {/if}
            </button>
          {/each}
          {#if browseTruncated}
            <div class="dm-add-game-browse-empty">Folder truncated &mdash; only the first batch is shown.</div>
          {/if}
        {/if}
      </div>

      <div class="dm-add-game-pick-selected" aria-label="Selected executables">
        <span class="dm-add-game-pick-selected-label">Watching for</span>
        {#if selectedExes.length === 0}
          <span class="dm-add-game-pick-selected-empty">Pick at least one .exe</span>
        {:else}
          {#each selectedExes as item (item.name)}
            <span class="dm-add-game-selected-chip" title={item.relativePath}>
              {item.name}
              <button
                type="button"
                class="dm-add-game-selected-chip-x"
                aria-label={`Remove ${item.name}`}
                onclick={() => onRemoveSelected(item.name)}
              ><X size={11} /></button>
            </span>
          {/each}
        {/if}
      </div>
    </div>

    <footer class="dm-add-game-foot">
      <span>{selectedExes.length} {selectedExes.length === 1 ? 'process' : 'processes'} selected</span>
      <div class="dm-add-game-pick-actions">
        <button type="button" class="dm-add-game-secondary" onclick={onCancel}>Cancel</button>
        <button
          type="button"
          class="dm-add-game-button"
          disabled={busyAppId === entry.appId || selectedExes.length === 0}
          onclick={onSubmit}
        >{busyAppId === entry.appId ? 'Adding...' : 'Add with selection'}</button>
      </div>
    </footer>
  </div>
</div>
