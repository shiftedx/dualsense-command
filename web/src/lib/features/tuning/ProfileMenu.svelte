<!--
  Profile cluster extracted from TuningHeader (Task 7 housekeeping, scheduled
  in the Task 5 review): the profile dropdown menu, the inline save-as/rename
  name editors, and the hidden import file input. Pure extraction — behavior
  matches the in-header version: Escape closes with focus returned to the
  anchor, Tab closes without stealing focus, any outside pointerdown / window
  scroll / resize closes without focus return, ArrowUp/ArrowDown cycle items.

  The trigger button stays in TuningHeader (it is header chrome); it is passed
  in as `anchor` for positioning and focus return, and `open` is bindable so
  the header toggles it.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { ProfileSummary } from '../../types';

  let {
    open = $bindable(false),
    anchor = undefined,
    profiles = [],
    selectedProfileId = '',
    activeProfileId = '',
    selectedActionProfile = null,
    canRenameSelectedProfile = false,
    canDeleteSelectedProfile = false,
    profileConfigDirty = false,
    profileSaveBusy = false,
    profileFileBusy = false,
    profileSaveAsBusy = false,
    profileRenameBusy = false,
    saveAsOpen = false,
    saveAsName = $bindable(''),
    renameProfileId = '',
    renameName = $bindable(''),
    onSelectProfile = () => {},
    onSaveProfile = () => {},
    onBeginSaveAs = () => {},
    onCancelSaveAs = () => {},
    onSubmitSaveAs = () => {},
    onSaveAsKeydown = () => {},
    onBeginRename = () => {},
    onCancelRename = () => {},
    onSubmitRename = () => {},
    onRenameKeydown = () => {},
    onDeleteProfile = () => {},
    onRestoreDefaults = () => {},
    onImportFile = () => {},
    onExportProfile = () => {}
  }: {
    open?: boolean;
    anchor?: HTMLButtonElement | undefined;
    profiles?: ProfileSummary[];
    selectedProfileId?: string;
    activeProfileId?: string;
    selectedActionProfile?: ProfileSummary | null;
    canRenameSelectedProfile?: boolean;
    canDeleteSelectedProfile?: boolean;
    profileConfigDirty?: boolean;
    profileSaveBusy?: boolean;
    profileFileBusy?: boolean;
    profileSaveAsBusy?: boolean;
    profileRenameBusy?: boolean;
    saveAsOpen?: boolean;
    saveAsName?: string;
    renameProfileId?: string;
    renameName?: string;
    onSelectProfile?: (profileId: string) => void | Promise<void>;
    onSaveProfile?: () => void | Promise<void>;
    onBeginSaveAs?: () => void;
    onCancelSaveAs?: () => void;
    onSubmitSaveAs?: () => void | Promise<void>;
    onSaveAsKeydown?: (event: KeyboardEvent) => void;
    onBeginRename?: () => void;
    onCancelRename?: () => void;
    onSubmitRename?: () => void | Promise<void>;
    onRenameKeydown?: (event: KeyboardEvent) => void;
    onDeleteProfile?: (profile: ProfileSummary) => void | Promise<void>;
    onRestoreDefaults?: () => void | Promise<void>;
    onImportFile?: (event: Event) => void | Promise<void>;
    onExportProfile?: () => void | Promise<void>;
  } = $props();

  let menuLeft = $state(0);
  let menuTop = $state(0);
  // Deleting is irreversible, so the Delete item swaps to an inline confirm
  // (same editor strip as rename/save-as) instead of deleting on first click.
  let deleteConfirmProfile: ProfileSummary | null = $state(null);
  let menuEl: HTMLDivElement | undefined = $state();
  let importInput: HTMLInputElement | undefined = $state();
  let placedFor: HTMLButtonElement | undefined;

  const closeMenu = (returnFocus = true) => {
    if (!open) return;
    open = false;
    if (returnFocus) anchor?.focus();
  };

  const menuItems = (): HTMLButtonElement[] =>
    menuEl ? Array.from(menuEl.querySelectorAll<HTMLButtonElement>('button.tuning-menu-item:not(:disabled)')) : [];

  const focusMenuItem = (index: number) => {
    const items = menuItems();
    if (!items.length) return;
    const next = ((index % items.length) + items.length) % items.length;
    items[next]?.focus();
  };

  // Place the menu against the anchor and focus the first item when opened.
  $effect(() => {
    if (!open || !anchor) {
      placedFor = undefined;
      return;
    }
    if (placedFor !== anchor) {
      placedFor = anchor;
      const rect = anchor.getBoundingClientRect();
      menuLeft = Math.max(8, Math.min(rect.left, window.innerWidth - 268));
      menuTop = Math.min(rect.bottom + 6, window.innerHeight - 60);
      void tick().then(() => focusMenuItem(0));
    }
  });

  const handleMenuKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      closeMenu();
      return;
    }
    if (event.key === 'Tab') {
      // Close so aria-expanded stays honest, but let Tab move focus naturally.
      closeMenu(false);
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const items = menuItems();
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    focusMenuItem(current + (event.key === 'ArrowDown' ? 1 : -1));
  };

  const handleWindowPointerDown = (event: PointerEvent) => {
    if (!open) return;
    const target = event.target as Node;
    if (menuEl?.contains(target)) return;
    if (anchor?.contains(target)) return;
    closeMenu(false);
  };

  // The menu is position: fixed and placed once at open; any scroll or
  // resize would leave it floating away from the trigger, so just close.
  $effect(() => {
    if (!open) return;
    const closeOnViewportChange = (event: Event) => {
      // Ignore scrolls that originate inside the menu itself.
      if (event.target instanceof Node && menuEl?.contains(event.target)) return;
      closeMenu(false);
    };
    window.addEventListener('scroll', closeOnViewportChange, true);
    window.addEventListener('resize', closeOnViewportChange);
    return () => {
      window.removeEventListener('scroll', closeOnViewportChange, true);
      window.removeEventListener('resize', closeOnViewportChange);
    };
  });

  const pickProfile = (profileId: string) => {
    closeMenu();
    deleteConfirmProfile = null;
    if (profileId !== selectedProfileId) void onSelectProfile(profileId);
  };

  const runProfileAction = (action: () => void | Promise<void>) => {
    closeMenu();
    deleteConfirmProfile = null;
    void action();
  };

  const requestImport = () => {
    closeMenu();
    if (!profileFileBusy) importInput?.click();
  };

  const deleteSelectedProfile = () => {
    const profile = selectedActionProfile;
    if (!profile) return;
    closeMenu();
    // Only one inline editor at a time: close rename/save-as before confirming.
    onCancelSaveAs();
    onCancelRename();
    deleteConfirmProfile = profile;
  };

  const cancelDeleteConfirm = () => {
    deleteConfirmProfile = null;
  };

  const confirmDeleteProfile = () => {
    const profile = deleteConfirmProfile;
    deleteConfirmProfile = null;
    if (profile) void onDeleteProfile(profile);
  };
</script>

<svelte:window onpointerdown={handleWindowPointerDown} />

{#if saveAsOpen || renameProfileId || deleteConfirmProfile}
  <div class="tuning-profile-editor">
    {#if saveAsOpen}
      <label>
        <span>New profile name</span>
        <input
          bind:value={saveAsName}
          disabled={profileSaveAsBusy}
          maxlength="80"
          spellcheck="false"
          aria-label="New profile name"
          onkeydown={onSaveAsKeydown}
        />
      </label>
      <button class="dm-mini-button" type="button" disabled={profileSaveAsBusy} onclick={onCancelSaveAs}>Cancel</button>
      <button
        class="dm-mini-button primary"
        type="button"
        disabled={profileSaveAsBusy || !saveAsName.trim()}
        onclick={() => void onSubmitSaveAs()}
      >{profileSaveAsBusy ? 'Saving' : 'Create'}</button>
    {:else if renameProfileId}
      <label>
        <span>Profile name</span>
        <input
          bind:value={renameName}
          disabled={profileRenameBusy}
          maxlength="80"
          spellcheck="false"
          aria-label="Profile name"
          onkeydown={onRenameKeydown}
        />
      </label>
      <button class="dm-mini-button" type="button" disabled={profileRenameBusy} onclick={onCancelRename}>Cancel</button>
      <button
        class="dm-mini-button primary"
        type="button"
        disabled={profileRenameBusy || !renameName.trim()}
        onclick={() => void onSubmitRename()}
      >{profileRenameBusy ? 'Saving' : 'Apply'}</button>
    {:else if deleteConfirmProfile}
      <span class="tuning-profile-editor-text">Delete &ldquo;{deleteConfirmProfile.name}&rdquo;?</span>
      <button class="dm-mini-button" type="button" disabled={profileFileBusy} onclick={cancelDeleteConfirm}>Cancel</button>
      <button
        class="dm-mini-button primary"
        type="button"
        disabled={profileFileBusy}
        onclick={confirmDeleteProfile}
      >{profileFileBusy ? 'Deleting' : 'Delete'}</button>
    {/if}
  </div>
{/if}

{#if open}
  <div
    class="tuning-menu"
    role="menu"
    tabindex="-1"
    aria-label="Profile actions"
    bind:this={menuEl}
    style={`left:${menuLeft}px;top:${menuTop}px;`}
    onkeydown={handleMenuKeydown}
  >
    <div class="tuning-menu-label">Profiles</div>
    {#each profiles as profile (profile.id)}
      <button
        class="tuning-menu-item"
        class:current={profile.id === selectedProfileId}
        type="button"
        role="menuitemradio"
        aria-checked={profile.id === selectedProfileId}
        onclick={() => pickProfile(profile.id)}
      >
        <span class="tuning-menu-item-text">{profile.name}</span>
        {#if profile.id === activeProfileId}<span class="tuning-menu-item-meta">live</span>{/if}
      </button>
    {/each}
    <div class="tuning-menu-divider" role="separator"></div>
    <button
      class="tuning-menu-item"
      type="button"
      role="menuitem"
      disabled={!selectedActionProfile || profileSaveBusy || !profileConfigDirty}
      onclick={() => runProfileAction(onSaveProfile)}
    >
      <span class="tuning-menu-item-text">{profileSaveBusy ? 'Saving…' : 'Save changes'}</span>
    </button>
    <button
      class="tuning-menu-item"
      type="button"
      role="menuitem"
      disabled={!selectedActionProfile || profileSaveAsBusy}
      onclick={() => runProfileAction(onBeginSaveAs)}
    >
      <span class="tuning-menu-item-text">Duplicate as new profile…</span>
    </button>
    <button
      class="tuning-menu-item"
      type="button"
      role="menuitem"
      disabled={!canRenameSelectedProfile || profileRenameBusy}
      onclick={() => runProfileAction(onBeginRename)}
    >
      <span class="tuning-menu-item-text">Rename…</span>
    </button>
    <button
      class="tuning-menu-item"
      type="button"
      role="menuitem"
      disabled={!canDeleteSelectedProfile || profileFileBusy}
      onclick={deleteSelectedProfile}
    >
      <span class="tuning-menu-item-text">Delete</span>
    </button>
    <button class="tuning-menu-item" type="button" role="menuitem" onclick={() => runProfileAction(onRestoreDefaults)}>
      <span class="tuning-menu-item-text">Reset to profile defaults</span>
    </button>
    <div class="tuning-menu-divider" role="separator"></div>
    <button class="tuning-menu-item" type="button" role="menuitem" disabled={profileFileBusy} onclick={requestImport}>
      <span class="tuning-menu-item-text">Import profile file…</span>
    </button>
    <button
      class="tuning-menu-item"
      type="button"
      role="menuitem"
      disabled={!selectedProfileId || profileFileBusy}
      onclick={() => runProfileAction(onExportProfile)}
    >
      <span class="tuning-menu-item-text">Export profile file</span>
    </button>
  </div>
{/if}

<input
  bind:this={importInput}
  class="ops-hidden-file"
  type="file"
  accept="application/json,.json,.dscc-profile"
  onchange={(event) => void onImportFile(event)}
/>
