<script lang="ts">
  import { CopyPlus, RotateCcw, Save } from '@lucide/svelte';
  import Tooltip from '../../../components/Tooltip.svelte';
  import type { ProfileSummary } from '../../types';

  export let profileContextProfiles: ProfileSummary[] = [];
  export let activeProfileId = '';
  export let selectedOverrideProfileId = '';
  export let selectedActionProfile: ProfileSummary | null | undefined = null;
  export let selectedGameName = 'game';
  export let canRenameSelectedProfile = false;
  export let canDeleteSelectedProfile = false;
  export let profileConfigDirty = false;
  export let profileSaveBusy = false;
  export let profileFileBusy = false;
  export let profileSaveAsBusy = false;
  export let profileRenameBusy = false;
  export let saveAsProfileOpen = false;
  export let saveAsProfileName = '';
  export let renameProfileId = '';
  export let renameProfileName = '';
  export let onSelectProfile: (profileId: string) => void | Promise<void> = () => {};
  export let onImportFile: (event: Event) => void | Promise<void> = () => {};
  export let onExportProfile: () => void | Promise<void> = () => {};
  export let onBeginSaveAs: () => void = () => {};
  export let onCancelSaveAs: () => void = () => {};
  export let onSubmitSaveAs: () => void | Promise<void> = () => {};
  export let onSaveAsKeydown: (event: KeyboardEvent) => void = () => {};
  export let onBeginRename: () => void = () => {};
  export let onCancelRename: () => void = () => {};
  export let onSubmitRename: () => void | Promise<void> = () => {};
  export let onRenameKeydown: (event: KeyboardEvent) => void = () => {};
  export let onDeleteProfile: (profile: ProfileSummary) => void | Promise<void> = () => {};
  export let onRestoreDefaults: () => void | Promise<void> = () => {};
  export let onSaveProfile: () => void | Promise<void> = () => {};

  let importInput: HTMLInputElement | undefined;

  const profileOptionLabel = (profile: ProfileSummary) => {
    if (profile.scope === 'Game') return `${profile.name} / ${selectedGameName}`;
    if (profile.id === activeProfileId) return `${profile.name} / live`;
    return profile.name;
  };

  $: profileOptions = profileContextProfiles.map((profile) => ({
    profile,
    label: profileOptionLabel(profile)
  }));

  const requestImport = () => {
    if (!profileFileBusy) importInput?.click();
  };
</script>

<div class="dm-profile-console">
  <div class="dm-profile-line">
    <label>
      <span>Editing Profile</span>
      <select
        value={selectedOverrideProfileId || activeProfileId}
        disabled={!profileContextProfiles.length}
        onchange={(event) => void onSelectProfile(event.currentTarget.value)}
      >
        {#each profileOptions as option}
          <option value={option.profile.id}>{option.label}</option>
        {/each}
      </select>
    </label>
    <div class="dm-action-row">
      <Tooltip text="Import a DSCC profile JSON file into the selected scope." side="top" align="center">
        <button class="dm-mini-button" type="button" onclick={requestImport}>Import</button>
      </Tooltip>
      <input bind:this={importInput} class="ops-hidden-file" type="file" accept="application/json,.json,.dscc-profile" onchange={(event) => void onImportFile(event)} />
      <Tooltip text="Export the selected profile as a DSCC profile JSON file." side="top" align="center">
        <button class="dm-mini-button" type="button" disabled={!activeProfileId || profileFileBusy} onclick={() => void onExportProfile()}>Export</button>
      </Tooltip>
      <Tooltip text="Save the current tuning into a new custom profile without changing the built-in template." side="top" align="center">
        <button
          class="dm-mini-button wide"
          type="button"
          disabled={!selectedActionProfile || profileSaveAsBusy}
          onclick={onBeginSaveAs}
        ><CopyPlus size={14} /> Save As</button>
      </Tooltip>
      <Tooltip text={canRenameSelectedProfile ? 'Rename the selected custom profile.' : 'Built-in profiles cannot be renamed. Use Save As first.'} side="top" align="center">
        <button
          class="dm-mini-button"
          type="button"
          disabled={!canRenameSelectedProfile || profileRenameBusy || !selectedActionProfile}
          onclick={onBeginRename}
        >Rename</button>
      </Tooltip>
      <Tooltip text={canDeleteSelectedProfile ? 'Delete the selected custom profile from DSCC.' : 'Built-in profiles cannot be deleted.'} side="top" align="center">
        <button
          class="dm-mini-button"
          type="button"
          disabled={!canDeleteSelectedProfile || profileFileBusy || !selectedActionProfile}
          onclick={() => selectedActionProfile && void onDeleteProfile(selectedActionProfile)}
        >Delete</button>
      </Tooltip>
      <Tooltip text="Restore the selected profile's trigger, haptic, and lightbar values to defaults." side="top" align="center">
        <button class="dm-mini-button" type="button" onclick={() => void onRestoreDefaults()}><RotateCcw size={14} /> Reset</button>
      </Tooltip>
      <Tooltip text={profileConfigDirty ? 'Save the current tuning to the selected profile.' : 'No unsaved tuning changes.'} side="top" align="end">
        <button
          class:dirty={profileConfigDirty}
          class="dm-apply-button"
          type="button"
          disabled={!selectedActionProfile || profileSaveBusy || !profileConfigDirty}
          onclick={() => void onSaveProfile()}
        ><Save size={14} /> {profileSaveBusy ? 'Saving' : 'Save'}</button>
      </Tooltip>
    </div>
  </div>

  {#if saveAsProfileOpen}
    <div class="dm-profile-rename">
      <label>
        <span>Save As</span>
        <input
          bind:value={saveAsProfileName}
          disabled={profileSaveAsBusy}
          maxlength="80"
          spellcheck="false"
          onkeydown={onSaveAsKeydown}
          aria-label="New profile name"
        />
      </label>
      <div class="dm-action-row">
        <button class="dm-mini-button" type="button" disabled={profileSaveAsBusy} onclick={onCancelSaveAs}>Cancel</button>
        <button class="dm-mini-button primary" type="button" disabled={profileSaveAsBusy || !saveAsProfileName.trim()} onclick={() => void onSubmitSaveAs()}>
          {profileSaveAsBusy ? 'Saving' : 'Create'}
        </button>
      </div>
    </div>
  {/if}

  {#if renameProfileId}
    <div class="dm-profile-rename">
      <label>
        <span>Name</span>
        <input
          bind:value={renameProfileName}
          disabled={profileRenameBusy}
          maxlength="80"
          spellcheck="false"
          onkeydown={onRenameKeydown}
          aria-label="Profile name"
        />
      </label>
      <div class="dm-action-row">
        <button class="dm-mini-button" type="button" disabled={profileRenameBusy} onclick={onCancelRename}>Cancel</button>
        <button class="dm-mini-button primary" type="button" disabled={profileRenameBusy || !renameProfileName.trim()} onclick={() => void onSubmitRename()}>
          {profileRenameBusy ? 'Saving' : 'Apply'}
        </button>
      </div>
    </div>
  {/if}
</div>
