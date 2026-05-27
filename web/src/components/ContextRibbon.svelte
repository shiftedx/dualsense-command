<script lang="ts">
  import InitialBadge from './InitialBadge.svelte';
  import Tooltip from './Tooltip.svelte';
  import { controllerConnectionText, controllerModelText } from '../lib/controllerDisplay';
  import type { ControllerStatus, ProfileSummary, SupportedGame } from '../lib/types';

  type TuningScope = 'none' | 'global' | 'game';
  type MenuPosition = {
    left: number;
    top: number;
    minWidth: number;
  };

  const SCOPE_ACCENT_GLOBAL = '#C18BEF';

  export let controller: ControllerStatus | null | undefined = null;
  export let connectedControllers: ControllerStatus[] = [];
  export let connectedControllerIds: string[] = [];
  export let profileTargetsAllConnected = false;
  export let profileTargetControllerIds: string[] = [];
  export let selectedTuningScope: TuningScope = 'global';
  export let selectedTuningGameId = '';
  export let steamContextGame: SupportedGame | null = null;
  export let steamContextArt = '';
  export let steamContextBackdropArt = '';
  export let steamContextMeta = '';
  export let discoveredGames: SupportedGame[] = [];
  export let profileContextProfiles: ProfileSummary[] = [];
  export let selectedOverrideProfileId = '';
  export let activeProfileId = '';
  export let activeProfileHeaderName = 'None';
  export let activeProfileHeaderMeta = '';
  export let listenOnAllInterfaces = false;
  export let appSettingsBusy = false;
  export let lanRestartRequired = false;
  export let desiredBindAddress: string | null | undefined = null;
  export let currentBindAddress: string | null | undefined = null;
  export let glyphOverrideEnabled = false;
  export let glyphStatus = '';
  export let gameAccentColor: (game: SupportedGame | null | undefined) => string = () => SCOPE_ACCENT_GLOBAL;
  export let onPickGlobal: () => void | Promise<void> = () => {};
  export let onPickGame: (game: SupportedGame) => void | Promise<void> = () => {};
  export let onPickProfile: (profileId: string) => void | Promise<void> = () => {};
  export let onPickAllControllers: () => void = () => {};
  export let onPickController: (controllerId: string) => void = () => {};
  export let onUpdateLanAccess: (enabled: boolean) => void | Promise<void> = () => {};
  export let onUpdateGlyphOverride: () => void | Promise<void> = () => {};

  let scopePickerOpen = false;
  let profilePickerOpen = false;
  let controllerPickerOpen = false;
  let scopeTriggerEl: HTMLButtonElement | null = null;
  let profileTriggerEl: HTMLButtonElement | null = null;
  let controllerTriggerEl: HTMLButtonElement | null = null;
  let scopeMenuPos: MenuPosition = { left: 0, top: 0, minWidth: 240 };
  let profileMenuPos: MenuPosition = { left: 0, top: 0, minWidth: 240 };
  let controllerMenuPos: MenuPosition = { left: 0, top: 0, minWidth: 240 };

  const menuPositionFor = (element: HTMLButtonElement | null, minWidth: number): MenuPosition => {
    if (!element) return { left: 0, top: 0, minWidth };
    const rect = element.getBoundingClientRect();
    return {
      left: rect.left,
      top: rect.bottom + 6,
      minWidth: Math.max(minWidth, rect.width)
    };
  };

  const updateMenuPositions = () => {
    scopeMenuPos = menuPositionFor(scopeTriggerEl, 240);
    profileMenuPos = menuPositionFor(profileTriggerEl, 260);
    controllerMenuPos = menuPositionFor(controllerTriggerEl, 260);
  };

  const closePickers = () => {
    scopePickerOpen = false;
    profilePickerOpen = false;
    controllerPickerOpen = false;
  };

  const toggleScopePicker = () => {
    if (!scopePickerOpen) updateMenuPositions();
    scopePickerOpen = !scopePickerOpen;
    if (scopePickerOpen) {
      profilePickerOpen = false;
      controllerPickerOpen = false;
    }
  };

  const toggleProfilePicker = () => {
    if (!profilePickerOpen) updateMenuPositions();
    profilePickerOpen = !profilePickerOpen;
    if (profilePickerOpen) {
      scopePickerOpen = false;
      controllerPickerOpen = false;
    }
  };

  const toggleControllerPicker = () => {
    if (!controllerPickerOpen) updateMenuPositions();
    controllerPickerOpen = !controllerPickerOpen;
    if (controllerPickerOpen) {
      scopePickerOpen = false;
      profilePickerOpen = false;
    }
  };

  const handleWindowChange = () => {
    if (scopePickerOpen || profilePickerOpen || controllerPickerOpen) updateMenuPositions();
  };

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key !== 'Escape' || (!scopePickerOpen && !profilePickerOpen && !controllerPickerOpen)) return;
    event.preventDefault();
    closePickers();
  };

  const handleDocumentClick = (event: MouseEvent) => {
    if (!scopePickerOpen && !profilePickerOpen && !controllerPickerOpen) return;
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (target.closest('.dm-ribbon-picker-host')) return;
    closePickers();
  };

  const profileTargetSummary = () => {
    if (profileTargetsAllConnected) return 'All Connected';
    const selectedId = profileTargetControllerIds[0] ?? controller?.id ?? '';
    const selected = connectedControllers.find((item) => item.id === selectedId) ?? controller;
    return selected ? selected.name || controllerModelText(selected) : 'No Controller';
  };

  const profileAccentColor = (scope: ProfileSummary['scope']): string => {
    if (scope === 'Game') return '#0070CC';
    if (scope === 'Built-in') return '#7EE0FF';
    return SCOPE_ACCENT_GLOBAL;
  };

  const pickGlobal = async () => {
    closePickers();
    if (selectedTuningScope !== 'global') await onPickGlobal();
  };

  const pickGame = async (game: SupportedGame) => {
    closePickers();
    if (selectedTuningScope === 'game' && selectedTuningGameId === game.gameId) return;
    await onPickGame(game);
  };

  const pickProfile = async (profileId: string) => {
    closePickers();
    if (!profileId || profileId === selectedOverrideProfileId) return;
    await onPickProfile(profileId);
  };

  const pickAllControllers = () => {
    onPickAllControllers();
    closePickers();
  };

  const pickController = (controllerId: string) => {
    onPickController(controllerId);
    closePickers();
  };
</script>

<svelte:window
  onkeydown={handleKeydown}
  onclick={handleDocumentClick}
  onresize={handleWindowChange}
  onscroll={handleWindowChange}
/>

<section class="dm-tuning-ribbon" aria-label="Selected game context and production controls">
  <div class="dm-steam-identity">
    <div class="dm-ribbon-picker-host">
      <button
        bind:this={controllerTriggerEl}
        type="button"
        class="dm-steam-identity-cell dm-ribbon-picker-trigger"
        class:open={controllerPickerOpen}
        aria-haspopup="listbox"
        aria-expanded={controllerPickerOpen}
        disabled={!connectedControllerIds.length}
        onclick={toggleControllerPicker}
      >
        <span>Target Controller</span>
        <strong>{profileTargetsAllConnected ? 'All Connected' : profileTargetSummary()}</strong>
        <p>{profileTargetsAllConnected ? `${connectedControllerIds.length} controller${connectedControllerIds.length === 1 ? '' : 's'}` : controllerConnectionText(controller ?? undefined)}</p>
        <span class="dm-ribbon-picker-caret" aria-hidden="true">▾</span>
      </button>
      {#if controllerPickerOpen}
        <div
          class="dm-ribbon-picker-menu"
          role="listbox"
          aria-label="Select target controller"
          style:left="{controllerMenuPos.left}px"
          style:top="{controllerMenuPos.top}px"
          style:min-width="{controllerMenuPos.minWidth}px"
        >
          <button
            type="button"
            class="dm-ribbon-picker-item"
            class:active={profileTargetsAllConnected}
            disabled={connectedControllerIds.length <= 1}
            role="option"
            aria-selected={profileTargetsAllConnected}
            onclick={pickAllControllers}
          >
            <span class="dm-ribbon-picker-thumb art" aria-hidden="true">
              <InitialBadge label="A" accent={SCOPE_ACCENT_GLOBAL} />
            </span>
            <span class="dm-ribbon-picker-copy">
              <strong>All Connected</strong>
              <small>{connectedControllerIds.length} controller{connectedControllerIds.length === 1 ? '' : 's'}</small>
            </span>
          </button>
          <div class="dm-ribbon-picker-divider" role="separator"></div>
          {#each connectedControllers as item (item.id)}
            <button
              type="button"
              class="dm-ribbon-picker-item"
              class:active={!profileTargetsAllConnected && profileTargetControllerIds.includes(item.id)}
              role="option"
              aria-selected={!profileTargetsAllConnected && profileTargetControllerIds.includes(item.id)}
              onclick={() => pickController(item.id)}
            >
              <span class="dm-ribbon-picker-thumb art" aria-hidden="true">
                <span class="dm-controller-glyph small" aria-hidden="true"></span>
              </span>
              <span class="dm-ribbon-picker-copy">
                <strong>{item.name || controllerModelText(item)}</strong>
                <small>{controllerConnectionText(item)}</small>
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="dm-ribbon-picker-host">
      <button
        bind:this={scopeTriggerEl}
        type="button"
        class="dm-steam-identity-cell dm-game-identity-cell dm-ribbon-picker-trigger"
        class:open={scopePickerOpen}
        aria-haspopup="listbox"
        aria-expanded={scopePickerOpen}
        onclick={toggleScopePicker}
      >
        {#if selectedTuningScope === 'game' && steamContextBackdropArt}
          <span class="dm-ribbon-game-backdrop" aria-hidden="true">
            <img src={steamContextBackdropArt} alt="" loading="lazy" />
          </span>
        {/if}
        <span class="dm-ribbon-game-media" aria-hidden="true">
          {#if selectedTuningScope === 'game' && steamContextArt}
            <img src={steamContextArt} alt="" loading="lazy" />
          {:else}
            <InitialBadge label="G" accent={SCOPE_ACCENT_GLOBAL} />
          {/if}
        </span>
        <span class="dm-ribbon-game-copy">
          <span>{selectedTuningScope === 'global' ? 'Selected Scope' : 'Selected Game'}</span>
          <strong>{selectedTuningScope === 'global' ? 'Global Profile' : steamContextGame?.name ?? 'No supported game selected'}</strong>
          <p>{steamContextMeta}</p>
        </span>
        <span class="dm-ribbon-picker-caret" aria-hidden="true">▾</span>
      </button>
      {#if scopePickerOpen}
        <div
          class="dm-ribbon-picker-menu"
          role="listbox"
          aria-label="Select tuning scope"
          style:left="{scopeMenuPos.left}px"
          style:top="{scopeMenuPos.top}px"
          style:min-width="{scopeMenuPos.minWidth}px"
        >
          <button
            type="button"
            class="dm-ribbon-picker-item"
            class:active={selectedTuningScope === 'global'}
            role="option"
            aria-selected={selectedTuningScope === 'global'}
            onclick={() => void pickGlobal()}
          >
            <span class="dm-ribbon-picker-thumb art" aria-hidden="true">
              <InitialBadge label="G" accent={SCOPE_ACCENT_GLOBAL} />
            </span>
            <span class="dm-ribbon-picker-copy">
              <strong>Global Profile</strong>
              <small>Controller-only tuning</small>
            </span>
          </button>
          {#if discoveredGames.length}
            <div class="dm-ribbon-picker-divider" role="separator"></div>
            {#each discoveredGames as game (game.gameId)}
              {@const gameArt = game.artwork?.capsuleUrl ?? game.artwork?.bannerUrl ?? game.artwork?.iconUrl}
              <button
                type="button"
                class="dm-ribbon-picker-item"
                class:active={selectedTuningScope === 'game' && game.gameId === selectedTuningGameId}
                role="option"
                aria-selected={selectedTuningScope === 'game' && game.gameId === selectedTuningGameId}
                onclick={() => void pickGame(game)}
              >
                <span class="dm-ribbon-picker-thumb art" aria-hidden="true">
                  {#if gameArt}
                    <img src={gameArt} alt="" loading="lazy" />
                  {:else}
                    <InitialBadge label={game.name} accent={gameAccentColor(game)} />
                  {/if}
                </span>
                <span class="dm-ribbon-picker-copy">
                  <strong>{game.name}</strong>
                  <small>{game.supportLevel === 'custom' ? 'custom game' : game.running ? 'running' : game.installed ? 'installed' : 'discovered'}</small>
                </span>
              </button>
            {/each}
          {/if}
        </div>
      {/if}
    </div>

    <div class="dm-ribbon-picker-host">
      <button
        bind:this={profileTriggerEl}
        type="button"
        class="dm-steam-identity-cell dm-active-profile-cell dm-ribbon-picker-trigger"
        class:open={profilePickerOpen}
        aria-haspopup="listbox"
        aria-expanded={profilePickerOpen}
        aria-live="polite"
        disabled={profileContextProfiles.length === 0}
        onclick={toggleProfilePicker}
      >
        <span>Live Profile</span>
        <strong>{activeProfileHeaderName}</strong>
        <p>{activeProfileHeaderMeta}</p>
        <span class="dm-ribbon-picker-caret" aria-hidden="true">▾</span>
      </button>
      {#if profilePickerOpen && profileContextProfiles.length}
        <div
          class="dm-ribbon-picker-menu profile"
          role="listbox"
          aria-label="Select active profile"
          style:left="{profileMenuPos.left}px"
          style:top="{profileMenuPos.top}px"
          style:min-width="{profileMenuPos.minWidth}px"
        >
          {#each profileContextProfiles as profile (profile.id)}
            <button
              type="button"
              class="dm-ribbon-picker-item"
              class:active={profile.id === (selectedOverrideProfileId || activeProfileId)}
              role="option"
              aria-selected={profile.id === (selectedOverrideProfileId || activeProfileId)}
              onclick={() => void pickProfile(profile.id)}
            >
              <span class="dm-ribbon-picker-thumb art" aria-hidden="true">
                <InitialBadge label={profile.name} accent={profileAccentColor(profile.scope)} />
              </span>
              <span class="dm-ribbon-picker-copy">
                <strong>{profile.name}</strong>
                <small>{profile.builtIn ? (profile.scope === 'Global' ? 'Stock / Global' : 'Built-in template') : profile.scope === 'Game' ? `Custom / ${steamContextGame?.name ?? 'game'}` : 'Custom / Global'}{profile.id === activeProfileId ? ' / live' : ''}</small>
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="dm-system-toggles" aria-label="Production system controls">
    <Tooltip block text="Local keeps the web UI bound to this PC. LAN exposes it on your network so you can tune from another device; a restart may be required after changing the bind address." side="bottom" align="end">
      <div class="dm-location-line">
        <label>
          <span>Web UI Location</span>
          <select
            value={listenOnAllInterfaces ? 'lan' : 'local'}
            disabled={appSettingsBusy}
            aria-label="Web UI location"
            onchange={(event) => void onUpdateLanAccess(event.currentTarget.value === 'lan')}
          >
            <option value="local">Local Only</option>
            <option value="lan">LAN Access</option>
          </select>
          <small>{lanRestartRequired ? `restart -> ${desiredBindAddress}` : currentBindAddress}</small>
        </label>
      </div>
    </Tooltip>
    <Tooltip block text="Installs or restores PlayStation-style button glyphs for supported games. DSCC keeps backups so the game can be returned to its default glyph files." side="bottom" align="end">
      <div class="dm-switch-line dm-glyph-switch">
        <div>
          <span>Controller Glyphs</span>
          <strong>{glyphOverrideEnabled ? 'PlayStation Icons' : 'Game Default'}</strong>
          <small>{glyphStatus}</small>
        </div>
        <button
          class:active={glyphOverrideEnabled}
          class="dm-toggle"
          type="button"
          disabled={appSettingsBusy}
          aria-label="Toggle PlayStation controller button glyphs"
          aria-pressed={glyphOverrideEnabled}
          onclick={() => void onUpdateGlyphOverride()}
        ><span></span></button>
      </div>
    </Tooltip>
  </div>
</section>
