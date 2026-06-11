<script lang="ts">
  import { tick } from 'svelte';
  import InitialBadge from '../../../components/InitialBadge.svelte';
  import { gameAccentColor, gameArtwork } from '../games/gamePresentation';
  import { controllerModelText } from '../../controllerDisplay';
  import {
    buildGameSelectModel,
    telemetryChipState,
    type GameSelectEntry,
    type TuningScopeKind
  } from './gameSelect';
  import type { ControllerStatus, ProfileSummary, SupportedGame } from '../../types';

  let {
    scope = 'global',
    selectedGame = null,
    discoveredGames = [],
    adapterRunning = false,
    packetRateHz = 0,
    controller = undefined,
    profiles = [],
    activeProfileId = '',
    selectedOverrideProfileId = '',
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
    onSelectGlobal = () => {},
    onSelectGame = () => {},
    onOpenAddGame = () => {},
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
    scope?: TuningScopeKind;
    selectedGame?: SupportedGame | null;
    discoveredGames?: SupportedGame[];
    adapterRunning?: boolean;
    packetRateHz?: number;
    controller?: ControllerStatus | undefined;
    profiles?: ProfileSummary[];
    activeProfileId?: string;
    selectedOverrideProfileId?: string;
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
    onSelectGlobal?: () => void | Promise<void>;
    onSelectGame?: (game: SupportedGame) => void | Promise<void>;
    onOpenAddGame?: () => void | Promise<void>;
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

  const gameModel = $derived(
    buildGameSelectModel({
      games: discoveredGames,
      scope,
      selectedGameId: selectedGame?.gameId ?? '',
      setupGuideGame:
        scope === 'game' && selectedGame?.supportLevel === 'telemetry' ? selectedGame : null,
      setupGuideEnabled: false
    })
  );
  const chipState = $derived(
    telemetryChipState({ scope, selectedGame, adapterRunning, packetRateHz })
  );

  const heroArt = $derived(scope === 'game' ? gameArtwork(selectedGame, 'hero') : null);
  const thumbArt = $derived(scope === 'game' ? gameArtwork(selectedGame, 'banner') : null);
  const selectedProfileId = $derived(selectedOverrideProfileId || activeProfileId);
  const profileName = $derived(
    profiles.find((profile) => profile.id === selectedProfileId)?.name ?? 'None'
  );
  const controllerAlias = $derived(controllerModelText(controller));
  const batteryText = $derived(
    controller?.connected && typeof controller.battery === 'number' && controller.batteryState !== 'unknown'
      ? `${controller.battery}%`
      : ''
  );

  // --- menu state -----------------------------------------------------------
  type MenuKind = 'game' | 'profile';
  let openMenu = $state<MenuKind | null>(null);
  let menuLeft = $state(0);
  let menuTop = $state(0);
  let gameTrigger: HTMLButtonElement | undefined = $state();
  let profileTrigger: HTMLButtonElement | undefined = $state();
  let menuEl: HTMLDivElement | undefined = $state();
  let importInput: HTMLInputElement | undefined = $state();

  const triggerFor = (kind: MenuKind) => (kind === 'game' ? gameTrigger : profileTrigger);

  const closeMenu = (returnFocus = true) => {
    if (!openMenu) return;
    const trigger = triggerFor(openMenu);
    openMenu = null;
    if (returnFocus) trigger?.focus();
  };

  const openMenuFor = async (kind: MenuKind) => {
    if (openMenu === kind) {
      closeMenu();
      return;
    }
    const trigger = triggerFor(kind);
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    menuLeft = Math.max(8, Math.min(rect.left, window.innerWidth - 268));
    menuTop = Math.min(rect.bottom + 6, window.innerHeight - 60);
    openMenu = kind;
    await tick();
    focusMenuItem(0);
  };

  const menuItems = (): HTMLButtonElement[] =>
    menuEl ? Array.from(menuEl.querySelectorAll<HTMLButtonElement>('button.tuning-menu-item:not(:disabled)')) : [];

  const focusMenuItem = (index: number) => {
    const items = menuItems();
    if (!items.length) return;
    const next = ((index % items.length) + items.length) % items.length;
    items[next]?.focus();
  };

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
    if (!openMenu) return;
    const target = event.target as Node;
    if (menuEl?.contains(target)) return;
    if (triggerFor(openMenu)?.contains(target)) return;
    closeMenu(false);
  };

  // The menus are position: fixed and placed once at open; any scroll or
  // resize would leave them floating away from the trigger, so just close.
  $effect(() => {
    if (!openMenu) return;
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

  const pickGameEntry = (entry: GameSelectEntry) => {
    if (entry.kind === 'setup-guide') return;
    closeMenu();
    if (entry.kind === 'everyday') void onSelectGlobal();
    else if (entry.kind === 'game') void onSelectGame(entry.game);
    else if (entry.kind === 'add-game') void onOpenAddGame();
  };

  const pickProfile = (profileId: string) => {
    closeMenu();
    if (profileId !== selectedProfileId) void onSelectProfile(profileId);
  };

  const runProfileAction = (action: () => void | Promise<void>) => {
    closeMenu();
    void action();
  };

  const requestImport = () => {
    closeMenu();
    if (!profileFileBusy) importInput?.click();
  };

  const deleteSelectedProfile = () => {
    const profile = selectedActionProfile;
    if (profile) runProfileAction(() => onDeleteProfile(profile));
  };
</script>

<svelte:window onpointerdown={handleWindowPointerDown} />

<header
  class="tuning-header"
  class:has-art={Boolean(heroArt)}
  style={heroArt ? `background-image: url("${heroArt}")` : ''}
>
  {#if heroArt}
    <div class="tuning-header-scrim" aria-hidden="true"></div>
  {/if}
  <div class="tuning-header-content">
    <div class="tuning-header-identity">
      {#if scope === 'game' && selectedGame}
        <span class="tuning-header-thumb" aria-hidden="true">
          {#if thumbArt}
            <img src={thumbArt} alt="" loading="lazy" />
          {:else}
            <InitialBadge label={selectedGame.name} accent={gameAccentColor(selectedGame)} size={48} />
          {/if}
        </span>
      {/if}
      <div class="tuning-header-titles">
        <div class="tuning-header-title-row">
          <button
            class="tuning-header-game"
            type="button"
            bind:this={gameTrigger}
            aria-haspopup="menu"
            aria-expanded={openMenu === 'game'}
            onclick={() => void openMenuFor('game')}
          >
            <strong>{gameModel.title}</strong>
            <span class="tuning-caret" aria-hidden="true">▾</span>
          </button>
          {#if chipState}
            <span
              class="tuning-telemetry-chip"
              class:fresh={chipState === 'fresh'}
              class:quiet={chipState === 'quiet'}
              title={chipState === 'fresh'
                ? 'Game data is arriving — the driving feel is live.'
                : 'The game is running but its data feed is silent. The setup guide can help.'}
            >
              ● {chipState === 'fresh' ? 'Telemetry Fresh' : 'Telemetry Quiet'}
            </span>
          {/if}
        </div>
        <div class="tuning-header-profile-row">
          <button
            class="tuning-header-profile"
            type="button"
            bind:this={profileTrigger}
            aria-haspopup="menu"
            aria-expanded={openMenu === 'profile'}
            disabled={!profiles.length}
            onclick={() => void openMenuFor('profile')}
          >
            <span class="tuning-profile-label">Profile:</span>
            <strong>{profileName}</strong>
            <span class="tuning-caret" aria-hidden="true">▾</span>
          </button>
          {#if profileConfigDirty}
            <span class="tuning-unsaved-note">· unsaved changes</span>
          {/if}
        </div>
      </div>
    </div>
    <div class="tuning-header-controller">
      <span class="tuning-controller-alias">{controllerAlias}</span>
      {#if controller?.connected}
        <span class="tuning-controller-dot" aria-hidden="true">●</span>
      {/if}
      {#if batteryText}
        <span class="tuning-controller-battery">{batteryText}</span>
      {/if}
    </div>
  </div>
</header>

{#if saveAsOpen || renameProfileId}
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
    {:else}
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
    {/if}
  </div>
{/if}

{#if openMenu === 'game'}
  <div
    class="tuning-menu"
    role="menu"
    tabindex="-1"
    aria-label="Pick a game to tune"
    bind:this={menuEl}
    style={`left:${menuLeft}px;top:${menuTop}px;`}
    onkeydown={handleMenuKeydown}
  >
    {#each gameModel.groups as group (group.id)}
      {#if group.id === 'actions'}
        <div class="tuning-menu-divider" role="separator"></div>
      {:else if group.label}
        <div class="tuning-menu-label">{group.label}</div>
      {/if}
      {#each group.entries as entry (entry.id)}
        {#if entry.kind === 'game'}
          <button
            class="tuning-menu-item"
            class:current={entry.current}
            type="button"
            role="menuitem"
            onclick={() => pickGameEntry(entry)}
          >
            <span class="tuning-menu-item-text">{#if entry.running}<span class="tuning-running-dot" aria-hidden="true">●</span>{/if}{entry.label}</span>
            {#if entry.running}<span class="tuning-menu-item-meta">detected</span>
            {:else if entry.current}<span class="tuning-menu-item-meta">current</span>{/if}
          </button>
        {:else if entry.kind === 'everyday'}
          <button
            class="tuning-menu-item"
            class:current={entry.current}
            type="button"
            role="menuitem"
            onclick={() => pickGameEntry(entry)}
          >
            <span class="tuning-menu-item-text">{entry.label}</span>
            <span class="tuning-menu-item-meta">{entry.detail}</span>
          </button>
        {:else if entry.kind === 'setup-guide'}
          <button
            class="tuning-menu-item accent"
            type="button"
            role="menuitem"
            disabled={!entry.enabled}
            title="The setup walkthrough opens here in an upcoming update."
          >
            <span class="tuning-menu-item-text">{entry.label}</span>
          </button>
        {:else}
          <button class="tuning-menu-item accent" type="button" role="menuitem" onclick={() => pickGameEntry(entry)}>
            <span class="tuning-menu-item-text">{entry.label}</span>
          </button>
        {/if}
      {/each}
    {/each}
  </div>
{/if}

{#if openMenu === 'profile'}
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
