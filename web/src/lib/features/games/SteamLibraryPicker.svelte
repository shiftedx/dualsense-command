<script lang="ts">
  import { Search } from '@lucide/svelte';
  import { browseSteamLibrary } from '../../api';
  import InitialBadge from '../../../components/InitialBadge.svelte';
  import type {
    SteamLibraryBrowseEntry,
    SteamLibraryBrowseResponse,
    SteamLibraryEntry
  } from '../../types';
  import {
    buildSteamBrowseBreadcrumbs,
    countAvailableSteamEntries,
    filterSteamLibraryEntries,
    formatPlaytime,
    initialExecutableSelection,
    joinBrowsePath,
    steamEntryArt,
    type SteamExecutableSelection
  } from './addGameModel';
  import SteamExecutablePicker from './SteamExecutablePicker.svelte';

  export let entries: SteamLibraryEntry[] = [];
  export let loading = false;
  export let busyAppId = '';
  export let errorMessage = '';
  export let onClose: () => void = () => {};
  export let onAdd: (entry: SteamLibraryEntry, processNames?: string[]) => void | Promise<void> = () => {};
  export let query = '';

  let searchInputEl: HTMLInputElement | null = null;
  let pickEntry: SteamLibraryEntry | null = null;
  let pickBrowsePath = '';
  let pickBrowseEntries: SteamLibraryBrowseEntry[] = [];
  let pickBrowseTruncated = false;
  let pickBrowseLoading = false;
  let pickBrowseError = '';
  let pickSelectedExes: SteamExecutableSelection[] = [];

  $: filtered = filterSteamLibraryEntries(entries, query);
  $: availableCount = countAvailableSteamEntries(entries);
  $: pickBreadcrumbs = buildSteamBrowseBreadcrumbs(pickEntry, pickBrowsePath);

  $: if (searchInputEl) {
    queueMicrotask(() => searchInputEl?.focus());
  }

  async function openPickFor(entry: SteamLibraryEntry) {
    pickEntry = entry;
    pickBrowsePath = '';
    pickBrowseEntries = [];
    pickBrowseTruncated = false;
    pickBrowseError = '';
    pickSelectedExes = initialExecutableSelection(entry);
    await loadBrowsePath('');
  }

  function cancelPick() {
    pickEntry = null;
    pickBrowsePath = '';
    pickBrowseEntries = [];
    pickBrowseTruncated = false;
    pickBrowseLoading = false;
    pickBrowseError = '';
    pickSelectedExes = [];
  }

  async function loadBrowsePath(nextPath: string) {
    if (!pickEntry) return;
    pickBrowseLoading = true;
    pickBrowseError = '';
    try {
      const response: SteamLibraryBrowseResponse = await browseSteamLibrary(
        pickEntry.appId,
        nextPath
      );
      pickBrowsePath = response.relativePath;
      pickBrowseEntries = response.entries;
      pickBrowseTruncated = response.truncated;
    } catch (caught) {
      pickBrowseError = caught instanceof Error ? caught.message : 'Could not list directory.';
      pickBrowseEntries = [];
      pickBrowseTruncated = false;
    } finally {
      pickBrowseLoading = false;
    }
  }

  function browseInto(entry: SteamLibraryBrowseEntry) {
    if (entry.kind !== 'dir') return;
    void loadBrowsePath(joinBrowsePath(pickBrowsePath, entry.name));
  }

  function toggleExeSelection(entry: SteamLibraryBrowseEntry) {
    if (entry.kind !== 'exe') return;
    const relativePath = joinBrowsePath(pickBrowsePath, entry.name);
    const existsIndex = pickSelectedExes.findIndex(
      (item) => item.name.toLowerCase() === entry.name.toLowerCase()
    );
    if (existsIndex >= 0) {
      pickSelectedExes = pickSelectedExes.filter((_, index) => index !== existsIndex);
    } else {
      pickSelectedExes = [...pickSelectedExes, { name: entry.name, relativePath }];
    }
  }

  function removeSelectedExe(name: string) {
    pickSelectedExes = pickSelectedExes.filter(
      (item) => item.name.toLowerCase() !== name.toLowerCase()
    );
  }

  function submitPick() {
    if (!pickEntry || pickSelectedExes.length === 0) return;
    const entry = pickEntry;
    const names = pickSelectedExes.map((item) => item.name);
    cancelPick();
    void onAdd(entry, names);
  }
</script>

<div class="dm-add-game-search">
  <Search size={14} aria-hidden="true" />
  <input
    bind:this={searchInputEl}
    bind:value={query}
    type="search"
    placeholder="Search by name, app id, or install folder"
    autocomplete="off"
    spellcheck="false"
  />
</div>

{#if errorMessage}
  <div class="dm-add-game-error" role="alert">{errorMessage}</div>
{/if}

<div class="dm-add-game-list" role="listbox" aria-label="Steam library games">
  {#if loading}
    <div class="dm-add-game-empty">Scanning Steam library&hellip;</div>
  {:else if entries.length === 0}
    <div class="dm-add-game-empty">
      <strong>No Steam library found</strong>
      <span>
        DSCC could not enumerate Steam app manifests. Make sure Steam is installed, then try again.
      </span>
    </div>
  {:else if filtered.length === 0}
    <div class="dm-add-game-empty">
      <strong>No matches</strong>
      <span>Nothing in your Steam library matches &ldquo;{query}&rdquo;.</span>
    </div>
  {:else}
    {#each filtered as entry (entry.appId)}
      {@const art = steamEntryArt(entry)}
      {@const busy = busyAppId === entry.appId}
      <article class="dm-add-game-row" class:already={entry.alreadyInCatalog}>
        <span class="dm-add-game-art" aria-hidden="true">
          <span class="dm-add-game-art-fallback">
            <InitialBadge label={entry.name} accent="#3BA0FF" />
          </span>
          {#if art}
            <img
              src={art}
              alt=""
              loading="lazy"
              onerror={(event) => {
                const img = event.currentTarget;
                if (img instanceof HTMLImageElement) img.style.display = 'none';
              }}
            />
          {/if}
        </span>
        <div class="dm-add-game-copy">
          <strong>{entry.name}</strong>
          <span class="dm-add-game-meta">
            <em>Steam {entry.appId}</em>
            {#if formatPlaytime(entry.stats?.playtimeMinutes)}
              <em>{formatPlaytime(entry.stats?.playtimeMinutes)}</em>
            {/if}
            <em>{entry.installDir}</em>
          </span>
          {#if entry.processCandidates.length}
            <small>Detects: {entry.processCandidates.slice(0, 3).join(', ')}{entry.processCandidates.length > 3 ? ', ...' : ''}</small>
          {:else}
            <small class="dm-add-game-warn">No .exe candidates found; auto-load may not work.</small>
          {/if}
        </div>
        <div class="dm-add-game-actions">
          {#if entry.alreadyInCatalog}
            <span class="dm-add-game-pill" aria-label="Already in catalog">Already added</span>
          {:else}
            <button
              type="button"
              class="dm-add-game-button"
              disabled={busy || entry.processCandidates.length === 0}
              title={entry.processCandidates.length === 0 ? 'No .exe candidates auto-detected - use Select... to specify' : 'Add with auto-detected process names'}
              onclick={() => void onAdd(entry)}
            >{busy ? 'Adding...' : 'Add'}</button>
            <button
              type="button"
              class="dm-add-game-secondary-button"
              disabled={busy}
              title="Pick which .exe DSCC should watch for to auto-load this profile"
              onclick={() => void openPickFor(entry)}
            >Select...</button>
          {/if}
        </div>
      </article>
    {/each}
  {/if}
</div>

<footer class="dm-add-game-foot">
  <span>{availableCount} {availableCount === 1 ? 'game' : 'games'} ready to add</span>
  <button type="button" class="dm-add-game-secondary" onclick={onClose}>Close</button>
</footer>

{#if pickEntry}
  <SteamExecutablePicker
    entry={pickEntry}
    browsePath={pickBrowsePath}
    browseEntries={pickBrowseEntries}
    browseTruncated={pickBrowseTruncated}
    browseLoading={pickBrowseLoading}
    browseError={pickBrowseError}
    selectedExes={pickSelectedExes}
    {busyAppId}
    breadcrumbs={pickBreadcrumbs}
    onCancel={cancelPick}
    onLoadPath={loadBrowsePath}
    onBrowseInto={browseInto}
    onToggleExe={toggleExeSelection}
    onRemoveSelected={removeSelectedExe}
    onSubmit={submitPick}
  />
{/if}
