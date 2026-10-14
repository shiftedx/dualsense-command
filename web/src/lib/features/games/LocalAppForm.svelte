<script lang="ts">
  import { X } from '@lucide/svelte';
  import type {
    AddLocalAppRequest,
    ValidateLocalAppRequest,
    ValidateLocalAppResponse
  } from '../../types';

  export let onClose: () => void = () => {};
  export let onValidateLocal: (request: ValidateLocalAppRequest) => Promise<ValidateLocalAppResponse> =
    async () => {
      throw new Error('Local app validation is unavailable.');
    };
  export let onAddLocal: (request: AddLocalAppRequest) => void | Promise<void> = () => {};

  export let localName = '';
  export let localExecutablePath = '';
  export let localProcessDraft = '';
  export let localProcesses: string[] = [];
  export let localValidation: ValidateLocalAppResponse | null = null;
  export let localBusy = false;
  export let localError = '';

  $: localProcessList = localProcesses.length
    ? localProcesses
    : localValidation?.processNames ?? [];
  $: localCanValidate = Boolean(localExecutablePath.trim()) && !localBusy;
  $: localCanAdd = Boolean(localExecutablePath.trim() && localName.trim()) && !localBusy;

  function localRequest(): ValidateLocalAppRequest {
    return {
      name: localName.trim() || null,
      executablePath: localExecutablePath.trim(),
      processNames: localProcessList
    };
  }

  function addLocalProcess() {
    const value = localProcessDraft.trim();
    if (!value) return;
    if (!localProcesses.some((item) => item.toLowerCase() === value.toLowerCase())) {
      localProcesses = [...localProcesses, value];
    }
    localProcessDraft = '';
    localValidation = null;
  }

  function removeLocalProcess(name: string) {
    localProcesses = localProcessList.filter((item) => item.toLowerCase() !== name.toLowerCase());
    localValidation = null;
  }

  function handleLocalProcessKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter' && event.key !== ',') return;
    event.preventDefault();
    addLocalProcess();
  }

  async function validateLocal() {
    if (!localCanValidate) return;
    localBusy = true;
    localError = '';
    try {
      const response = await onValidateLocal(localRequest());
      localValidation = response;
      if (!localName.trim()) localName = response.name;
      if (!localProcesses.length) localProcesses = response.processNames;
    } catch (caught) {
      localValidation = null;
      localError = caught instanceof Error ? caught.message : 'Unable to validate local app.';
    } finally {
      localBusy = false;
    }
  }

  async function submitLocalApp() {
    if (!localCanAdd) return;
    localBusy = true;
    localError = '';
    try {
      await onAddLocal({
        name: localName.trim(),
        executablePath: localExecutablePath.trim(),
        processNames: localProcessList
      });
      localName = '';
      localExecutablePath = '';
      localProcessDraft = '';
      localProcesses = [];
      localValidation = null;
      onClose();
    } catch (caught) {
      localError = caught instanceof Error ? caught.message : 'Unable to add local app.';
    } finally {
      localBusy = false;
    }
  }
</script>

<div class="dm-local-app-form">
  {#if localError}
    <div class="dm-add-game-error" role="alert">{localError}</div>
  {:else if localValidation?.warnings.length}
    <div class="dm-add-game-error warn" role="status">{localValidation.warnings.join(' ')}</div>
  {/if}

  <label class="dm-local-app-field">
    <span>App name</span>
    <input
      bind:value={localName}
      type="text"
      maxlength="80"
      spellcheck="false"
      placeholder="My non-Steam game"
      disabled={localBusy}
    />
  </label>

  <label class="dm-local-app-field">
    <span>.exe path</span>
    <input
      bind:value={localExecutablePath}
      type="text"
      spellcheck="false"
      placeholder="C:\Games\Example\Example.exe"
      disabled={localBusy}
    />
  </label>

  <div class="dm-local-app-field">
    <span>Watched processes</span>
    <div class="dm-local-process-row">
      <input
        bind:value={localProcessDraft}
        type="text"
        spellcheck="false"
        placeholder="Example.exe"
        disabled={localBusy}
        onkeydown={handleLocalProcessKeydown}
      />
      <button
        type="button"
        class="dm-add-game-secondary-button"
        disabled={localBusy || !localProcessDraft.trim()}
        onclick={addLocalProcess}
      >Add</button>
    </div>
    <div class="dm-local-process-chips" aria-label="Watched process names">
      {#if localProcessList.length}
        {#each localProcessList as name (name)}
          <span class="dm-add-game-selected-chip">
            {name}
            <button
              type="button"
              class="dm-add-game-selected-chip-x"
              aria-label={`Remove ${name}`}
              onclick={() => removeLocalProcess(name)}
            ><X size={11} /></button>
          </span>
        {/each}
      {:else}
        <span class="dm-add-game-pick-selected-empty">Validate to infer the executable name, or add one manually.</span>
      {/if}
    </div>
  </div>

  {#if localValidation}
    <div class="dm-local-validation" role="status">
      <strong>{localValidation.name}</strong>
      <span>{localValidation.executableName} / {localValidation.processNames.join(', ')}</span>
    </div>
  {/if}
</div>

<footer class="dm-add-game-foot">
  <span>{localValidation?.valid ? 'Local app validated' : 'DSCC Bridge local app profile'}</span>
  <div class="dm-add-game-pick-actions">
    <button
      type="button"
      class="dm-add-game-secondary"
      disabled={!localCanValidate}
      onclick={() => void validateLocal()}
    >
      {localBusy ? 'Working' : 'Validate'}
    </button>
    <button
      type="button"
      class="dm-add-game-button"
      disabled={!localCanAdd}
      onclick={() => void submitLocalApp()}
    >
      {localBusy ? 'Adding...' : 'Add Local App'}
    </button>
  </div>
</footer>
