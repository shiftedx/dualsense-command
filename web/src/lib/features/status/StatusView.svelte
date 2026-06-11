<script lang="ts">
  import type {
    AdapterStatus,
    ControllerStatus,
    ProfileSummary,
    SupportedGame
  } from '../../types';

  let {
    controllers = [],
    controller = undefined,
    detectedGame = null,
    detectedGameName = null,
    activeProfile = undefined,
    activeProfileName = 'None',
    overrideActive = false,
    adapter = undefined,
    adapters = [],
    renameActiveId = '',
    renameName = $bindable(''),
    renameBusy = false,
    onBeginRename = () => {},
    onSubmitRename = () => {},
    onCancelRename = () => {},
    onRenameKeydown = () => {}
  }: {
    controllers?: ControllerStatus[];
    controller?: ControllerStatus | undefined;
    detectedGame?: SupportedGame | null;
    detectedGameName?: string | null;
    activeProfile?: ProfileSummary | undefined;
    activeProfileName?: string;
    overrideActive?: boolean;
    adapter?: AdapterStatus | undefined;
    adapters?: AdapterStatus[];
    renameActiveId?: string;
    renameName?: string;
    renameBusy?: boolean;
    onBeginRename?: (item: ControllerStatus) => void;
    onSubmitRename?: () => void | Promise<void>;
    onCancelRename?: () => void;
    onRenameKeydown?: (event: KeyboardEvent) => void;
  } = $props();

  type Finding = { id: string; text: string; detail?: string };

  const connectedControllers = $derived(controllers.filter((item) => item.connected));
  const hasController = $derived(connectedControllers.length > 0);
  const alias = $derived(controller?.name || controller?.family || 'your controller');
  const gameName = $derived(detectedGameName ?? detectedGame?.name ?? null);
  const gameRunning = $derived(Boolean(detectedGame?.running && gameName));
  const telemetryExpected = $derived(gameRunning && detectedGame?.supportLevel === 'telemetry');
  const telemetryFresh = $derived(
    Boolean(adapter && adapter.state === 'running' && adapter.packetRateHz > 0)
  );

  const findings = $derived.by(() => {
    const list: Finding[] = [];
    for (const item of controllers) {
      if (!item.connected) {
        list.push({
          id: `controller-disconnected-${item.id}`,
          text: `${item.name || item.family} is disconnected.`,
          detail: 'Reconnect it and its tuned feel comes right back.'
        });
      }
    }
    if (telemetryExpected && !telemetryFresh) {
      list.push({
        id: 'telemetry-quiet',
        text: `${gameName} is running, but its telemetry has gone quiet.`,
        detail: adapter?.setupHint || 'Check the game\'s Data Out setting if the feel stops.'
      });
    }
    for (const item of adapters) {
      if (item.state === 'faulted') {
        list.push({
          id: `adapter-faulted-${item.id}`,
          text: `${item.name} is blocked — another app may be using its port.`,
          detail: item.setupHint || undefined
        });
      }
    }
    return list;
  });

  const dotState = $derived(!hasController ? 'danger' : findings.length ? 'warn' : 'ok');
  const headline = $derived(
    !hasController
      ? 'No controller connected yet.'
      : findings.length
        ? 'Something needs your attention.'
        : 'Everything is working.'
  );
  const clause = $derived.by(() => {
    if (!hasController) return 'Plug in or pair a controller and DSCC takes it from there.';
    if (gameRunning && telemetryExpected && !telemetryFresh) {
      return `${gameName} detected, but its telemetry is quiet on ${alias}.`;
    }
    if (gameRunning) return `${gameName} detected — tuned feel is live on ${alias}.`;
    return `${alias} is connected and ready. Tuned feel starts when a supported game does.`;
  });

  const profileScopeNote = $derived.by(() => {
    if (!activeProfile) return '';
    if (activeProfile.scope === 'Game') return 'its Game Profile';
    if (activeProfile.scope === 'Global') return 'Everyday · Global Profile';
    return 'built-in';
  });

  const connectionLine = (item: ControllerStatus): string => {
    const parts: string[] = [];
    if (item.connected && item.transport !== 'Unknown') parts.push(item.transport);
    if (typeof item.battery === 'number' && item.batteryState !== 'unknown') {
      if (item.batteryState === 'charging') parts.push(`battery ${item.battery}%, charging`);
      else if (item.batteryState === 'full') parts.push(`battery ${item.battery}%, full`);
      else parts.push(`battery ${item.battery}%`);
    }
    return parts.join(' · ');
  };
</script>

<section class="status-view" aria-label="Status">
  <h1 class="visually-hidden">Status</h1>

  <div class="status-sentence">
    <span class="status-dot {dotState}" aria-hidden="true"></span>
    <span class="status-headline">{headline}</span>
    <span class="status-clause">{clause}</span>
  </div>

  <div class="status-groups">
    <div class="status-group">
      <div class="lbl">Controller</div>
      {#if controllers.length}
        <div class="status-surf">
          {#each controllers as item (item.id)}
            <div class="status-controller-card">
              <div class="status-controller-icon" aria-hidden="true">🎮</div>
              <div class="status-controller-main">
                {#if renameActiveId === item.id}
                  <div class="status-rename">
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      type="text"
                      aria-label="Controller name"
                      bind:value={renameName}
                      onkeydown={onRenameKeydown}
                      disabled={renameBusy}
                      autofocus
                    />
                    <button
                      class="status-link"
                      type="button"
                      disabled={renameBusy}
                      onclick={() => void onSubmitRename()}
                    >Save</button>
                    <button
                      class="status-link mut"
                      type="button"
                      disabled={renameBusy}
                      onclick={onCancelRename}
                    >Cancel</button>
                  </div>
                {:else}
                  <div class="status-controller-name">
                    {item.name || item.family}
                    <span class="status-mut">&middot; {item.family}</span>
                  </div>
                {/if}
                <div class="status-controller-line">
                  {#if item.connected}
                    <span class="status-ok"><span aria-hidden="true">&#9679;</span> Connected</span>
                  {:else}
                    <span class="status-warn"><span aria-hidden="true">&#9679;</span> Disconnected</span>
                  {/if}
                  {#if connectionLine(item)}
                    &middot; {connectionLine(item)}
                  {/if}
                </div>
              </div>
              {#if renameActiveId !== item.id}
                <button
                  class="status-link"
                  type="button"
                  disabled={renameBusy}
                  onclick={() => onBeginRename(item)}
                >Rename</button>
              {/if}
            </div>
          {/each}
        </div>
        <div class="status-hint">Another controller? Plug it in or pair it and it appears here.</div>
      {:else}
        <div class="status-surf status-empty">Plug in or pair a controller and it appears here.</div>
      {/if}
    </div>

    <div class="status-group">
      <div class="lbl">What's active, and why</div>
      <div class="status-rows">
        <div class="status-row">
          <span>Game detected</span>
          {#if gameRunning}
            <span>{gameName} <span class="status-ok" aria-hidden="true">&#9679;</span></span>
          {:else if gameName}
            <span>{gameName} <span class="status-mut">(installed, not running)</span></span>
          {:else}
            <span class="status-mut">None yet</span>
          {/if}
        </div>
        <div class="status-row">
          <span>Profile in use</span>
          <span class="status-strong">
            {activeProfileName}
            {#if profileScopeNote}
              <span class="status-mut">({profileScopeNote})</span>
            {/if}
            {#if overrideActive}
              <span class="status-mut">&middot; chosen by you</span>
            {/if}
          </span>
        </div>
        <div class="status-row">
          <span>Telemetry</span>
          {#if telemetryExpected && telemetryFresh}
            <span class="status-ok">Fresh &middot; driving feel is live</span>
          {:else if telemetryExpected}
            <span class="status-warn">Quiet &middot; waiting for game data</span>
          {:else}
            <span class="status-mut">Idle until a supported game runs</span>
          {/if}
        </div>
        <div class="status-row">
          <span>When the game closes</span>
          <span class="status-mut">Back to Global Profile</span>
        </div>
      </div>
    </div>

    <div class="status-group narrow">
      <div class="lbl">Needs attention</div>
      <div class="status-surf">
        {#if findings.length}
          {#each findings as finding (finding.id)}
            <div class="status-finding">
              <span class="status-warn" aria-hidden="true">&#9679;</span> {finding.text}
              {#if finding.detail}
                <div class="status-finding-detail">{finding.detail}</div>
              {/if}
            </div>
          {/each}
        {:else}
          <div class="status-finding">Nothing else needs you.</div>
          <div class="status-finding-detail">This box stays empty when all is well.</div>
        {/if}
      </div>
    </div>
  </div>
</section>
