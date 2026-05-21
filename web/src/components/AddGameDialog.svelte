<script lang="ts">
  import { ChevronRight, FileCode, Folder, Search, X } from '@lucide/svelte';
  import { browseSteamLibrary } from '../lib/api';
  import InitialBadge from './InitialBadge.svelte';
  import type {
    SteamLibraryBrowseEntry,
    SteamLibraryBrowseResponse,
    SteamLibraryEntry
  } from '../lib/types';

  export let open = false;
  export let entries: SteamLibraryEntry[] = [];
  export let loading = false;
  export let busyAppId = '';
  export let errorMessage = '';
  export let onClose: () => void = () => {};
  export let onAdd: (entry: SteamLibraryEntry, processNames?: string[]) => void | Promise<void> = () => {};

  let query = '';
  let searchInputEl: HTMLInputElement | null = null;
  let pickEntry: SteamLibraryEntry | null = null;
  let pickBrowsePath = '';
  let pickBrowseEntries: SteamLibraryBrowseEntry[] = [];
  let pickBrowseInstallPath = '';
  let pickBrowseTruncated = false;
  let pickBrowseLoading = false;
  let pickBrowseError = '';
  let pickSelectedExes: Array<{ name: string; relativePath: string }> = [];

  $: filtered = (() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter((entry) => {
      return (
        entry.name.toLowerCase().includes(q) ||
        entry.appId.includes(q) ||
        entry.installDir.toLowerCase().includes(q)
      );
    });
  })();

  $: availableCount = entries.filter((entry) => !entry.alreadyInCatalog).length;

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  }

  function handleBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) onClose();
  }

  function formatPlaytime(minutes: number | null | undefined): string {
    if (!minutes || minutes <= 0) return '';
    if (minutes < 60) return `${minutes}m played`;
    const hours = minutes / 60;
    return `${hours.toFixed(hours < 10 ? 1 : 0)}h played`;
  }

  function entryArt(entry: SteamLibraryEntry): string | null {
    return (
      entry.artwork?.capsuleUrl ??
      entry.artwork?.bannerUrl ??
      entry.artwork?.heroUrl ??
      entry.artwork?.iconUrl ??
      null
    );
  }

  $: if (open && searchInputEl) {
    queueMicrotask(() => searchInputEl?.focus());
  }

  async function openPickFor(entry: SteamLibraryEntry) {
    pickEntry = entry;
    pickBrowsePath = '';
    pickBrowseEntries = [];
    pickBrowseInstallPath = entry.installPath;
    pickBrowseTruncated = false;
    pickBrowseError = '';
    pickSelectedExes = entry.processCandidates.map((name) => ({ name, relativePath: name }));
    await loadBrowsePath('');
  }

  function cancelPick() {
    pickEntry = null;
    pickBrowsePath = '';
    pickBrowseEntries = [];
    pickBrowseInstallPath = '';
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
      pickBrowseInstallPath = response.installPath;
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

  function joinBrowsePath(parent: string, child: string): string {
    if (!parent) return child;
    if (!child) return parent;
    return `${parent}/${child}`;
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

  function isExeSelected(entry: SteamLibraryBrowseEntry): boolean {
    if (entry.kind !== 'exe') return false;
    return pickSelectedExes.some(
      (item) => item.name.toLowerCase() === entry.name.toLowerCase()
    );
  }

  function removeSelectedExe(name: string) {
    pickSelectedExes = pickSelectedExes.filter(
      (item) => item.name.toLowerCase() !== name.toLowerCase()
    );
  }

  $: pickBreadcrumbs = (() => {
    if (!pickEntry) return [] as Array<{ label: string; path: string }>;
    const crumbs: Array<{ label: string; path: string }> = [
      { label: pickEntry.installDir || '⟂ root', path: '' }
    ];
    if (pickBrowsePath) {
      const parts = pickBrowsePath.split('/').filter((p) => p.length > 0);
      let acc = '';
      for (const part of parts) {
        acc = acc ? `${acc}/${part}` : part;
        crumbs.push({ label: part, path: acc });
      }
    }
    return crumbs;
  })();

  function formatSize(bytes: number | null | undefined): string {
    if (!bytes || bytes <= 0) return '';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  function submitPick() {
    if (!pickEntry || pickSelectedExes.length === 0) return;
    const entry = pickEntry;
    const names = pickSelectedExes.map((item) => item.name);
    cancelPick();
    void onAdd(entry, names);
  }
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open}
  <div
    class="dm-add-game-backdrop"
    role="presentation"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
  >
    <div
      class="dm-add-game-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="dm-add-game-title"
      tabindex="-1"
    >
      <header class="dm-add-game-head">
        <div>
          <span>Add Game</span>
          <h2 id="dm-add-game-title">Pick from your Steam library</h2>
          <p>
            DSCC saves a profile per game. When the selected game launches via Steam, that profile
            auto-loads on the controller. Games already covered by a built-in module are marked.
          </p>
        </div>
        <button type="button" class="dm-add-game-close" aria-label="Close" onclick={onClose}>
          <X size={16} />
        </button>
      </header>

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
          <div class="dm-add-game-empty">Scanning Steam library…</div>
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
            <span>Nothing in your Steam library matches “{query}”.</span>
          </div>
        {:else}
          {#each filtered as entry (entry.appId)}
            {@const art = entryArt(entry)}
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
                  <small>Detects: {entry.processCandidates.slice(0, 3).join(', ')}{entry.processCandidates.length > 3 ? ', …' : ''}</small>
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
                    title={entry.processCandidates.length === 0 ? 'No .exe candidates auto-detected — use Select… to specify' : 'Add with auto-detected process names'}
                    onclick={() => void onAdd(entry)}
                  >{busy ? 'Adding…' : 'Add'}</button>
                  <button
                    type="button"
                    class="dm-add-game-secondary-button"
                    disabled={busy}
                    title="Pick which .exe DSCC should watch for to auto-load this profile"
                    onclick={() => openPickFor(entry)}
                  >Select…</button>
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
    </div>
  </div>

  {#if pickEntry}
    <div
      class="dm-add-game-backdrop dm-add-game-pick-backdrop"
      role="presentation"
      onclick={(event) => { if (event.target === event.currentTarget) cancelPick(); }}
      onkeydown={(event) => { if (event.key === 'Escape') { event.preventDefault(); cancelPick(); } }}
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
            <h2 id="dm-add-game-pick-title">{pickEntry.name}</h2>
            <p>
              Browse the install folder and tick the .exe(s) DSCC should watch for. When any of
              those processes start, the profile you tune for this game auto-loads on the
              controller.
            </p>
          </div>
          <button type="button" class="dm-add-game-close" aria-label="Close" onclick={cancelPick}>
            <X size={16} />
          </button>
        </header>

        <nav class="dm-add-game-breadcrumbs" aria-label="Folder breadcrumb">
          {#each pickBreadcrumbs as crumb, index (crumb.path)}
            {#if index > 0}
              <ChevronRight size={12} aria-hidden="true" />
            {/if}
            <button
              type="button"
              class="dm-add-game-breadcrumb"
              class:current={crumb.path === pickBrowsePath}
              disabled={crumb.path === pickBrowsePath || pickBrowseLoading}
              onclick={() => void loadBrowsePath(crumb.path)}
            >{crumb.label}</button>
          {/each}
        </nav>

        <div class="dm-add-game-browse-body">
          {#if pickBrowseError}
            <div class="dm-add-game-error" role="alert">{pickBrowseError}</div>
          {/if}

          <div class="dm-add-game-browse-list" role="listbox" aria-label="Folder contents">
            {#if pickBrowseLoading}
              <div class="dm-add-game-browse-empty">Reading folder…</div>
            {:else if pickBrowseEntries.length === 0}
              <div class="dm-add-game-browse-empty">
                {pickBrowsePath ? 'No folders or .exe files here.' : 'Install folder is empty or unreadable.'}
              </div>
            {:else}
              {#each pickBrowseEntries as entry (entry.name + entry.kind)}
                {@const selected = isExeSelected(entry)}
                <button
                  type="button"
                  class="dm-add-game-browse-row"
                  class:dir={entry.kind === 'dir'}
                  class:exe={entry.kind === 'exe'}
                  class:selected
                  onclick={() => entry.kind === 'dir' ? browseInto(entry) : toggleExeSelection(entry)}
                >
                  <span class="dm-add-game-browse-icon" aria-hidden="true">
                    {#if entry.kind === 'dir'}
                      <Folder size={14} />
                    {:else}
                      <FileCode size={14} />
                    {/if}
                  </span>
                  <span class="dm-add-game-browse-name">{entry.name}</span>
                  {#if entry.kind === 'exe' && entry.sizeBytes}
                    <span class="dm-add-game-browse-meta">{formatSize(entry.sizeBytes)}</span>
                  {/if}
                  {#if entry.kind === 'dir'}
                    <ChevronRight size={12} class="dm-add-game-browse-affordance" aria-hidden="true" />
                  {:else if selected}
                    <span class="dm-add-game-browse-affordance selected-pill">Selected</span>
                  {:else}
                    <span class="dm-add-game-browse-affordance hint-pill">Select</span>
                  {/if}
                </button>
              {/each}
              {#if pickBrowseTruncated}
                <div class="dm-add-game-browse-empty">Folder truncated &mdash; only the first batch is shown.</div>
              {/if}
            {/if}
          </div>

          <div class="dm-add-game-pick-selected" aria-label="Selected executables">
            <span class="dm-add-game-pick-selected-label">Watching for</span>
            {#if pickSelectedExes.length === 0}
              <span class="dm-add-game-pick-selected-empty">Pick at least one .exe</span>
            {:else}
              {#each pickSelectedExes as item (item.name)}
                <span class="dm-add-game-selected-chip" title={item.relativePath}>
                  {item.name}
                  <button
                    type="button"
                    class="dm-add-game-selected-chip-x"
                    aria-label={`Remove ${item.name}`}
                    onclick={() => removeSelectedExe(item.name)}
                  ><X size={11} /></button>
                </span>
              {/each}
            {/if}
          </div>
        </div>

        <footer class="dm-add-game-foot">
          <span>{pickSelectedExes.length} {pickSelectedExes.length === 1 ? 'process' : 'processes'} selected</span>
          <div class="dm-add-game-pick-actions">
            <button type="button" class="dm-add-game-secondary" onclick={cancelPick}>Cancel</button>
            <button
              type="button"
              class="dm-add-game-button"
              disabled={busyAppId === pickEntry.appId || pickSelectedExes.length === 0}
              onclick={submitPick}
            >{busyAppId === pickEntry.appId ? 'Adding…' : 'Add with selection'}</button>
          </div>
        </footer>
      </div>
    </div>
  {/if}
{/if}

<style>
  .dm-add-game-backdrop {
    position: fixed;
    inset: 0;
    z-index: 8000;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(0, 0, 0, 0.62);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
  }

  .dm-add-game-dialog {
    display: flex;
    flex-direction: column;
    width: min(760px, 100%);
    max-height: min(720px, calc(100vh - 48px));
    border: 1px solid rgba(0, 112, 204, 0.32);
    border-radius: 10px;
    background:
      linear-gradient(180deg, rgba(26, 28, 34, 0.96), rgba(14, 14, 18, 0.98));
    box-shadow:
      0 32px 80px rgba(0, 0, 0, 0.6),
      inset 0 1px 0 rgba(226, 232, 240, 0.06);
    overflow: hidden;
  }

  .dm-add-game-head {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
    gap: 12px;
    padding: 18px 20px 12px;
    border-bottom: 1px solid rgba(113, 113, 122, 0.18);
  }

  .dm-add-game-head span {
    display: block;
    color: var(--tungsten);
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .dm-add-game-head h2 {
    margin: 4px 0 6px;
    color: #FFFFFF;
    font-family: "Space Grotesk", "Inter Tight", Inter, sans-serif;
    font-size: 20px;
    font-weight: 700;
    line-height: 1.15;
  }

  .dm-add-game-head p {
    margin: 0;
    max-width: 60ch;
    color: var(--tungsten);
    font-size: 12px;
    line-height: 1.4;
  }

  .dm-add-game-close {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 1px solid rgba(113, 113, 122, 0.32);
    border-radius: 6px;
    color: var(--haptic);
    background: rgba(18, 18, 20, 0.6);
    cursor: pointer;
  }

  .dm-add-game-close:hover {
    border-color: rgba(0, 112, 204, 0.6);
    background: rgba(0, 112, 204, 0.16);
  }

  .dm-add-game-search {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 20px 0;
    padding: 0 10px;
    border: 1px solid rgba(113, 113, 122, 0.28);
    border-radius: 6px;
    background: rgba(10, 10, 12, 0.62);
    color: var(--tungsten);
  }

  .dm-add-game-search:focus-within {
    border-color: rgba(0, 112, 204, 0.62);
    box-shadow: 0 0 0 2px rgba(0, 112, 204, 0.16);
  }

  .dm-add-game-search input {
    flex: 1 1 auto;
    min-width: 0;
    padding: 8px 0;
    border: 0;
    color: #FFFFFF;
    background: transparent;
    font-size: 13px;
    font-weight: 600;
    outline: none;
  }

  .dm-add-game-search input::placeholder {
    color: var(--tungsten);
  }

  .dm-add-game-error {
    margin: 12px 20px 0;
    padding: 8px 10px;
    border: 1px solid rgba(229, 62, 62, 0.45);
    border-radius: 5px;
    color: #FECACA;
    background: rgba(229, 62, 62, 0.14);
    font-size: 12px;
    line-height: 1.35;
  }

  .dm-add-game-list {
    flex: 1 1 auto;
    overflow-y: auto;
    padding: 14px 20px 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .dm-add-game-empty {
    display: grid;
    gap: 4px;
    padding: 24px 8px;
    text-align: center;
    color: var(--tungsten);
    font-size: 13px;
  }

  .dm-add-game-empty strong {
    color: #FFFFFF;
    font-size: 14px;
    font-weight: 700;
  }

  .dm-add-game-row {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    padding: 10px 12px;
    border: 1px solid rgba(113, 113, 122, 0.18);
    border-radius: 7px;
    background: rgba(18, 18, 20, 0.46);
    transition: border-color 140ms ease-out, background 140ms ease-out;
  }

  .dm-add-game-row:hover {
    border-color: rgba(0, 112, 204, 0.5);
    background: rgba(0, 112, 204, 0.08);
  }

  .dm-add-game-row.already {
    opacity: 0.62;
  }

  .dm-add-game-art {
    position: relative;
    display: grid;
    place-items: center;
    width: 130px;
    height: 110px;
    border-radius: 6px;
    overflow: hidden;
    background: rgba(0, 112, 204, 0.12);
  }

  .dm-add-game-art img {
    grid-column: 1;
    grid-row: 1;
    position: relative;
    z-index: 1;
    width: 100%;
    height: 100%;
    object-fit: contain;
    opacity: 0.96;
  }

  .dm-add-game-art-fallback {
    grid-column: 1;
    grid-row: 1;
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    padding: 6px;
    color: #FFFFFF;
  }

  .dm-add-game-art-fallback :global(svg) {
    max-width: 100%;
    max-height: 100%;
    width: auto;
    height: 100%;
  }

  .dm-add-game-copy {
    display: grid;
    gap: 4px;
    min-width: 0;
  }

  .dm-add-game-copy strong {
    overflow: hidden;
    color: #FFFFFF;
    font-size: 14px;
    font-weight: 700;
    line-height: 1.18;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dm-add-game-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 6px;
    color: var(--tungsten);
    font-size: 11px;
  }

  .dm-add-game-meta em {
    padding: 2px 5px;
    border-radius: 3px;
    background: rgba(226, 232, 240, 0.075);
    font-style: normal;
    font-weight: 700;
  }

  .dm-add-game-copy small {
    color: var(--tungsten);
    font-size: 11px;
    line-height: 1.3;
  }

  .dm-add-game-warn {
    color: #FBBF24;
  }

  .dm-add-game-actions {
    display: flex;
    gap: 6px;
  }

  .dm-add-game-button {
    padding: 6px 14px;
    border: 1px solid rgba(0, 112, 204, 0.65);
    border-radius: 5px;
    color: #FFFFFF;
    background: rgba(0, 112, 204, 0.32);
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .dm-add-game-button:hover:not(:disabled) {
    background: rgba(0, 112, 204, 0.46);
  }

  .dm-add-game-button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .dm-add-game-secondary-button {
    padding: 6px 12px;
    border: 1px solid rgba(113, 113, 122, 0.42);
    border-radius: 5px;
    color: var(--haptic);
    background: rgba(10, 10, 12, 0.42);
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  .dm-add-game-secondary-button:hover:not(:disabled) {
    border-color: rgba(0, 112, 204, 0.6);
    background: rgba(0, 112, 204, 0.16);
  }

  .dm-add-game-secondary-button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .dm-add-game-pick-backdrop {
    z-index: 8500;
  }

  .dm-add-game-pick-dialog {
    width: min(640px, 100%);
    max-height: min(680px, calc(100vh - 48px));
  }

  .dm-add-game-breadcrumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    padding: 10px 20px 0;
    color: var(--tungsten);
    font-size: 11px;
    font-family: "JetBrains Mono", "Space Mono", ui-monospace, monospace;
  }

  .dm-add-game-breadcrumb {
    appearance: none;
    border: 0;
    background: transparent;
    color: var(--haptic);
    font: inherit;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
  }

  .dm-add-game-breadcrumb:hover:not(:disabled) {
    background: rgba(0, 112, 204, 0.14);
    color: #FFFFFF;
  }

  .dm-add-game-breadcrumb.current {
    color: #FFFFFF;
    font-weight: 700;
    cursor: default;
  }

  .dm-add-game-browse-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px 20px 4px;
    flex: 1 1 auto;
    min-height: 0;
  }

  .dm-add-game-browse-list {
    flex: 1 1 auto;
    min-height: 200px;
    overflow-y: auto;
    border: 1px solid rgba(113, 113, 122, 0.22);
    border-radius: 6px;
    background: rgba(10, 10, 12, 0.42);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .dm-add-game-browse-empty {
    padding: 16px 8px;
    text-align: center;
    color: var(--tungsten);
    font-size: 12px;
  }

  .dm-add-game-browse-row {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border: 0;
    border-radius: 5px;
    color: var(--haptic);
    background: transparent;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 120ms ease-out, color 120ms ease-out;
  }

  .dm-add-game-browse-row:hover,
  .dm-add-game-browse-row:focus-visible {
    outline: none;
    color: #FFFFFF;
    background: rgba(0, 112, 204, 0.14);
  }

  .dm-add-game-browse-row.dir .dm-add-game-browse-icon {
    color: var(--actuation);
  }

  .dm-add-game-browse-row.exe .dm-add-game-browse-icon {
    color: var(--haptic);
  }

  .dm-add-game-browse-row.selected {
    color: #FFFFFF;
    background: rgba(0, 112, 204, 0.26);
    box-shadow: inset 0 0 0 1px rgba(0, 112, 204, 0.55);
  }

  .dm-add-game-browse-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .dm-add-game-browse-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: "JetBrains Mono", "Space Mono", ui-monospace, monospace;
    font-size: 12px;
    font-weight: 600;
  }

  .dm-add-game-browse-meta {
    color: var(--tungsten);
    font-size: 10px;
    font-family: "JetBrains Mono", monospace;
  }

  .dm-add-game-browse-affordance {
    color: var(--tungsten);
  }

  .dm-add-game-browse-affordance.selected-pill {
    color: var(--actuation);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .dm-add-game-browse-affordance.hint-pill {
    color: var(--tungsten);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    opacity: 0.65;
  }

  .dm-add-game-browse-row:hover .dm-add-game-browse-affordance.hint-pill {
    opacity: 1;
    color: var(--haptic);
  }

  .dm-add-game-pick-selected {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 8px 10px;
    border: 1px dashed rgba(113, 113, 122, 0.28);
    border-radius: 6px;
    min-height: 38px;
  }

  .dm-add-game-pick-selected-label {
    color: var(--tungsten);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    margin-right: 4px;
  }

  .dm-add-game-pick-selected-empty {
    color: var(--tungsten);
    font-size: 11px;
    font-style: italic;
  }

  .dm-add-game-selected-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 4px 3px 8px;
    border: 1px solid rgba(0, 112, 204, 0.4);
    border-radius: 999px;
    background: rgba(0, 112, 204, 0.16);
    color: #FFFFFF;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 600;
  }

  .dm-add-game-selected-chip-x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border: 0;
    border-radius: 50%;
    color: var(--haptic);
    background: transparent;
    cursor: pointer;
  }

  .dm-add-game-selected-chip-x:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #FFFFFF;
  }

  .dm-add-game-pick-actions {
    display: inline-flex;
    gap: 8px;
  }

  .dm-add-game-pill {
    padding: 4px 8px;
    border: 1px solid rgba(113, 113, 122, 0.32);
    border-radius: 999px;
    color: var(--tungsten);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .dm-add-game-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 20px 16px;
    border-top: 1px solid rgba(113, 113, 122, 0.18);
    color: var(--tungsten);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .dm-add-game-secondary {
    padding: 7px 14px;
    border: 1px solid rgba(113, 113, 122, 0.34);
    border-radius: 5px;
    color: var(--haptic);
    background: rgba(10, 10, 12, 0.42);
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .dm-add-game-secondary:hover {
    border-color: rgba(0, 112, 204, 0.6);
    background: rgba(0, 112, 204, 0.14);
  }
</style>
