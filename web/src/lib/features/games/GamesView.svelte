<script lang="ts">
  import Tooltip from '../../../components/Tooltip.svelte';
  import InitialBadge from '../../../components/InitialBadge.svelte';
  import { controllerConnectionText, controllerModelText } from '../../controllerDisplay';
  import type { ControllerStatus, ProfileSummary, SupportedGame } from '../../types';

  export let controller: ControllerStatus | undefined = undefined;
  export let connectedControllers: ControllerStatus[] = [];
  export let selectedTuningScope: 'none' | 'global' | 'game' = 'global';
  export let selectedTuningGameId = '';
  export let globalProfilePreview: ProfileSummary | null | undefined = null;
  export let profileTargetsAllConnected = false;
  export let profileTargetControllerIds: string[] = [];
  export let discoveredGames: SupportedGame[] = [];
  export let detectionSignalText = '';
  export let gameArtwork: (game: SupportedGame, kind: 'hero' | 'banner' | 'capsule' | 'icon') => string | null = () => null;
  export let gameMediaDetails: (game: SupportedGame) => string[] = () => [];
  export let profileScopeCount: (game: SupportedGame) => number = () => 0;
  export let gameAccentColor: (game: SupportedGame) => string = () => '#3BA0FF';
  export let gameTileStatus: (game: SupportedGame) => string = () => 'installed';
  export let onSelectGlobal: () => void | Promise<void> = () => {};
  export let onSelectGame: (game: SupportedGame) => void | Promise<void> = () => {};
  export let onOpenAddGame: () => void | Promise<void> = () => {};
  export let onPickAllControllers: () => void = () => {};
  export let onPickControllerTarget: (controllerId: string) => void = () => {};

  $: connectedCount = connectedControllers.length;
  $: targetSummary = profileTargetsAllConnected
    ? 'All Connected'
    : connectedControllers.find((item) => profileTargetControllerIds.includes(item.id))?.name ||
      controllerModelText(controller);
  $: targetDetail = profileTargetsAllConnected
    ? `${connectedCount} controller${connectedCount === 1 ? '' : 's'}`
    : controllerConnectionText(controller);
</script>

<section class="dm-games-page" aria-label="Profiles and supported games">
  <div class="dm-games-column wide">
    <div class="dm-games-head">
      <span>Profiles</span>
      <h2>Games</h2>
    </div>

    <div class="dm-profile-target-strip" aria-label="Profile target controller">
      <div>
        <span>Target Controller</span>
        <strong>{targetSummary}</strong>
        <small>{targetDetail}</small>
      </div>
      {#if connectedControllers.length > 1}
        <button
          type="button"
          class:active={profileTargetsAllConnected}
          aria-pressed={profileTargetsAllConnected}
          onclick={onPickAllControllers}
        >All</button>
        {#each connectedControllers as item (item.id)}
          <button
            type="button"
            class:active={!profileTargetsAllConnected && profileTargetControllerIds.includes(item.id)}
            aria-pressed={!profileTargetsAllConnected && profileTargetControllerIds.includes(item.id)}
            onclick={() => onPickControllerTarget(item.id)}
          >{item.name || controllerModelText(item)}</button>
        {/each}
      {/if}
    </div>

    <div class="dm-scope-strip">
      <Tooltip
        text="Per-game profiles auto-load when the game launches via Steam. Global tunes the controller when nothing is detected or an unsupported game is running."
        side="bottom"
        align="start"
      >
        <button
          type="button"
          class:active={selectedTuningScope === 'global'}
          disabled={!controller}
          onclick={() => void onSelectGlobal()}
        >
          <span class="dm-controller-glyph small" aria-hidden="true"></span>
          <span class="dm-scope-chip">
            <span class="dm-scope-chip-label">Profile Scope</span>
            <strong class="dm-scope-chip-value">Global</strong>
            <small class="dm-scope-chip-detail">{globalProfilePreview?.name ?? 'Base'} · Controller-only tuning</small>
          </span>
        </button>
      </Tooltip>
    </div>

    {#if discoveredGames.length}
      <div class="dm-game-grid">
        {#each discoveredGames as game (game.gameId)}
          {@const heroArt = gameArtwork(game, 'hero') ?? gameArtwork(game, 'banner')}
          {@const tileArt = gameArtwork(game, 'banner') ?? gameArtwork(game, 'capsule') ?? gameArtwork(game, 'icon')}
          {@const details = gameMediaDetails(game)}
          {@const scopedProfiles = profileScopeCount(game)}
          <button
            type="button"
            class="dm-game-card"
            class:active={selectedTuningScope === 'game' && game.gameId === selectedTuningGameId}
            class:running={game.running}
            class:custom={game.supportLevel === 'custom'}
            disabled={!controller}
            aria-pressed={selectedTuningScope === 'game' && game.gameId === selectedTuningGameId}
            style={heroArt ? `--game-hero: url("${heroArt}")` : ''}
            onclick={() => void onSelectGame(game)}
          >
            <span class="dm-game-card-media">
              {#if tileArt}
                <img
                  src={tileArt}
                  alt=""
                  loading="lazy"
                  aria-hidden="true"
                  onerror={(event) => {
                    const img = event.currentTarget;
                    if (img instanceof HTMLImageElement) img.style.display = 'none';
                  }}
                />
              {/if}
              <span class="dm-game-art-fallback" aria-hidden="true">
                <InitialBadge label={game.name} accent={gameAccentColor(game)} />
              </span>
              <code>{game.running ? 'LIVE' : game.supportLevel === 'custom' ? 'CUSTOM' : game.installed ? 'READY' : 'SUPPORTED'}</code>
            </span>
            <span class="dm-game-copy">
              <strong>{game.name}</strong>
              <span class="dm-game-meta">
                {#each details as detail}
                  <em>{detail}</em>
                {/each}
              </span>
              <small>{scopedProfiles ? `${scopedProfiles} game profile${scopedProfiles === 1 ? '' : 's'}` : game.supportLevel === 'custom' ? 'custom / no telemetry adapter' : `${gameTileStatus(game)} / telemetry`}</small>
            </span>
          </button>
        {/each}
        <button
          type="button"
          class="dm-game-card dm-game-card-add"
          disabled={!controller}
          aria-label="Add a custom game from your Steam library"
          onclick={() => void onOpenAddGame()}
        >
          <span class="dm-add-game-icon" aria-hidden="true">+</span>
          <span class="dm-game-copy">
            <strong>Add a Game</strong>
            <small>Pick from your installed Steam library or local apps. DSCC will save a profile per game and auto-load it on launch.</small>
          </span>
        </button>
      </div>
    {:else}
      <div class="dm-empty-choice wide">
        <strong>No supported games discovered</strong>
        <span>{detectionSignalText || 'Steam library data unavailable'}</span>
        <button
          type="button"
          class="dm-mini-button"
          style="margin-top: 8px;"
          disabled={!controller}
          onclick={() => void onOpenAddGame()}
        >Add a game manually</button>
      </div>
    {/if}
  </div>
</section>
