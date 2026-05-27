<script lang="ts">
  import { X } from '@lucide/svelte';
  import type {
    AddLocalAppRequest,
    SteamLibraryEntry,
    ValidateLocalAppRequest,
    ValidateLocalAppResponse
  } from '../../types';
  import LocalAppForm from './LocalAppForm.svelte';
  import SteamLibraryPicker from './SteamLibraryPicker.svelte';
  import './addGameDialog.css';

  export let open = false;
  export let entries: SteamLibraryEntry[] = [];
  export let loading = false;
  export let busyAppId = '';
  export let errorMessage = '';
  export let onClose: () => void = () => {};
  export let onAdd: (entry: SteamLibraryEntry, processNames?: string[]) => void | Promise<void> = () => {};
  export let onValidateLocal: (request: ValidateLocalAppRequest) => Promise<ValidateLocalAppResponse> =
    async () => {
      throw new Error('Local app validation is unavailable.');
    };
  export let onAddLocal: (request: AddLocalAppRequest) => void | Promise<void> = () => {};

  let mode: 'steam' | 'local' = 'steam';
  let steamQuery = '';
  let localName = '';
  let localExecutablePath = '';
  let localProcessDraft = '';
  let localProcesses: string[] = [];
  let localValidation: ValidateLocalAppResponse | null = null;
  let localBusy = false;
  let localError = '';

  $: if (!open) {
    mode = 'steam';
    localError = '';
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  }

  function handleBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) onClose();
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
          <h2 id="dm-add-game-title">
            {mode === 'steam' ? 'Pick from your Steam library' : 'Add a local app'}
          </h2>
          <p>
            {mode === 'steam'
              ? 'DSCC saves a profile per Steam game and can mirror Steam Input bindings where available.'
              : 'Local apps use DSCC Input Bridge mappings and process-name detection. No injection, hooks, or game files are modified.'}
          </p>
        </div>
        <button type="button" class="dm-add-game-close" aria-label="Close" onclick={onClose}>
          <X size={16} />
        </button>
      </header>

      <div class="dm-add-game-tabs" role="tablist" aria-label="Add game source">
        <button
          type="button"
          role="tab"
          class:active={mode === 'steam'}
          aria-selected={mode === 'steam'}
          onclick={() => { mode = 'steam'; }}
        >Steam Library</button>
        <button
          type="button"
          role="tab"
          class:active={mode === 'local'}
          aria-selected={mode === 'local'}
          onclick={() => { mode = 'local'; }}
        >Local App</button>
      </div>

      {#if mode === 'steam'}
        <SteamLibraryPicker
          {entries}
          {loading}
          {busyAppId}
          {errorMessage}
          {onClose}
          {onAdd}
          bind:query={steamQuery}
        />
      {:else}
        <LocalAppForm
          {onClose}
          {onValidateLocal}
          {onAddLocal}
          bind:localName
          bind:localExecutablePath
          bind:localProcessDraft
          bind:localProcesses
          bind:localValidation
          bind:localBusy
          bind:localError
        />
      {/if}
    </div>
  </div>
{/if}
