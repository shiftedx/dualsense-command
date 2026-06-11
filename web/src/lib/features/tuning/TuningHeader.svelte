<script lang="ts">
  import { tick } from 'svelte';
  import InitialBadge from '../../../components/InitialBadge.svelte';
  import ProfileMenu from './ProfileMenu.svelte';
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
    setupVerified = true,
    setupGuideOpen = false,
    controller = undefined,
    profiles = [],
    activeProfileId = '',
    selectedOverrideProfileId = '',
    selectedActionProfile = null,
    canRenameSelectedProfile = false,
    canDeleteSelectedProfile = false,
    profileConfigDirty = false,
    unsavedChangeCount = 0,
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
    onToggleSetupGuide = () => {},
    onOpenSetupGuide = () => {},
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
    setupVerified?: boolean;
    setupGuideOpen?: boolean;
    controller?: ControllerStatus | undefined;
    profiles?: ProfileSummary[];
    activeProfileId?: string;
    selectedOverrideProfileId?: string;
    selectedActionProfile?: ProfileSummary | null;
    canRenameSelectedProfile?: boolean;
    canDeleteSelectedProfile?: boolean;
    profileConfigDirty?: boolean;
    unsavedChangeCount?: number;
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
    onToggleSetupGuide?: () => void;
    onOpenSetupGuide?: () => void;
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
      setupGuideGame: scope === 'game' ? selectedGame : null,
      setupGuideEnabled: true
    })
  );
  const chipState = $derived(
    telemetryChipState({ scope, selectedGame, adapterRunning, packetRateHz, setupVerified })
  );
  const chipPresentation = $derived.by(() => {
    switch (chipState) {
      case 'fresh':
        return {
          label: 'Telemetry Fresh',
          suffix: '· setup ↗',
          title: 'Game data is arriving — the driving feel is live. Open the setup guide to check the port or re-copy values.'
        };
      case 'quiet':
        return {
          label: 'Telemetry Quiet',
          suffix: '· fix ↗',
          title: 'The game is running but its data feed is silent. Open the guide to fix it.'
        };
      case 'setup':
        return {
          label: 'One-Time Setup Needed',
          suffix: '',
          title: "About 2 minutes, once. Then it's automatic forever."
        };
      case 'none':
        return {
          label: 'No Setup Needed',
          suffix: '· setup ↗',
          title: 'This game needs no telemetry feed. Open the guide for details.'
        };
      default:
        return null;
    }
  });

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
  const unsavedNote = $derived(
    unsavedChangeCount > 0
      ? `· ${unsavedChangeCount} unsaved change${unsavedChangeCount === 1 ? '' : 's'}`
      : profileConfigDirty
        ? '· unsaved changes'
        : ''
  );

  // --- game menu state (the profile menu lives in ProfileMenu.svelte) -------
  let gameMenuOpen = $state(false);
  let profileMenuOpen = $state(false);
  let menuLeft = $state(0);
  let menuTop = $state(0);
  let gameTrigger: HTMLButtonElement | undefined = $state();
  let profileTrigger: HTMLButtonElement | undefined = $state();
  let menuEl: HTMLDivElement | undefined = $state();

  const closeGameMenu = (returnFocus = true) => {
    if (!gameMenuOpen) return;
    gameMenuOpen = false;
    if (returnFocus) gameTrigger?.focus();
  };

  const toggleGameMenu = async () => {
    profileMenuOpen = false;
    if (gameMenuOpen) {
      closeGameMenu();
      return;
    }
    if (!gameTrigger) return;
    const rect = gameTrigger.getBoundingClientRect();
    menuLeft = Math.max(8, Math.min(rect.left, window.innerWidth - 268));
    menuTop = Math.min(rect.bottom + 6, window.innerHeight - 60);
    gameMenuOpen = true;
    await tick();
    focusMenuItem(0);
  };

  const toggleProfileMenu = () => {
    closeGameMenu(false);
    profileMenuOpen = !profileMenuOpen;
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
      closeGameMenu();
      return;
    }
    if (event.key === 'Tab') {
      // Close so aria-expanded stays honest, but let Tab move focus naturally.
      closeGameMenu(false);
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const items = menuItems();
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    focusMenuItem(current + (event.key === 'ArrowDown' ? 1 : -1));
  };

  const handleWindowPointerDown = (event: PointerEvent) => {
    if (!gameMenuOpen) return;
    const target = event.target as Node;
    if (menuEl?.contains(target)) return;
    if (gameTrigger?.contains(target)) return;
    closeGameMenu(false);
  };

  // The menu is position: fixed and placed once at open; any scroll or
  // resize would leave it floating away from the trigger, so just close.
  $effect(() => {
    if (!gameMenuOpen) return;
    const closeOnViewportChange = (event: Event) => {
      // Ignore scrolls that originate inside the menu itself.
      if (event.target instanceof Node && menuEl?.contains(event.target)) return;
      closeGameMenu(false);
    };
    window.addEventListener('scroll', closeOnViewportChange, true);
    window.addEventListener('resize', closeOnViewportChange);
    return () => {
      window.removeEventListener('scroll', closeOnViewportChange, true);
      window.removeEventListener('resize', closeOnViewportChange);
    };
  });

  const pickGameEntry = (entry: GameSelectEntry) => {
    closeGameMenu();
    if (entry.kind === 'everyday') void onSelectGlobal();
    else if (entry.kind === 'game') void onSelectGame(entry.game);
    else if (entry.kind === 'setup-guide') onOpenSetupGuide();
    else if (entry.kind === 'add-game') void onOpenAddGame();
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
            aria-expanded={gameMenuOpen}
            onclick={() => void toggleGameMenu()}
          >
            <strong>{gameModel.title}</strong>
            <span class="tuning-caret" aria-hidden="true">▾</span>
          </button>
          {#if chipState && chipPresentation}
            <button
              class="tuning-telemetry-chip clickable"
              class:fresh={chipState === 'fresh'}
              class:quiet={chipState === 'quiet'}
              class:setup={chipState === 'setup'}
              class:none={chipState === 'none'}
              type="button"
              disabled={!setupVerified}
              title={!setupVerified ? 'The setup guide is open below — it closes by itself once setup verifies.' : chipPresentation.title}
              aria-pressed={setupGuideOpen}
              onclick={onToggleSetupGuide}
            >
              ● {chipPresentation.label}{#if chipPresentation.suffix}
                <span class="tuning-chip-suffix">{chipPresentation.suffix}</span>
              {/if}
            </button>
          {/if}
        </div>
        <div class="tuning-header-profile-row">
          <button
            class="tuning-header-profile"
            type="button"
            bind:this={profileTrigger}
            aria-haspopup="menu"
            aria-expanded={profileMenuOpen}
            disabled={!profiles.length}
            title="Choose the saved profile to edit in this tuning scope."
            onclick={toggleProfileMenu}
          >
            <span class="tuning-profile-label">Saved profile:</span>
            <strong>{profileName}</strong>
            <span class="tuning-caret" aria-hidden="true">▾</span>
          </button>
          {#if unsavedNote}
            <span class="tuning-unsaved-note">{unsavedNote}</span>
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

<ProfileMenu
  bind:open={profileMenuOpen}
  anchor={profileTrigger}
  {profiles}
  {selectedProfileId}
  {activeProfileId}
  {selectedActionProfile}
  {canRenameSelectedProfile}
  {canDeleteSelectedProfile}
  {profileConfigDirty}
  {profileSaveBusy}
  {profileFileBusy}
  {profileSaveAsBusy}
  {profileRenameBusy}
  {saveAsOpen}
  bind:saveAsName
  {renameProfileId}
  bind:renameName
  {onSelectProfile}
  {onSaveProfile}
  {onBeginSaveAs}
  {onCancelSaveAs}
  {onSubmitSaveAs}
  {onSaveAsKeydown}
  {onBeginRename}
  {onCancelRename}
  {onSubmitRename}
  {onRenameKeydown}
  {onDeleteProfile}
  {onRestoreDefaults}
  {onImportFile}
  {onExportProfile}
/>

{#if gameMenuOpen}
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
            onclick={() => pickGameEntry(entry)}
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
