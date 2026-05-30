<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import ControllerCard from './ControllerCard.svelte';
  import Tooltip from '../../../components/Tooltip.svelte';
  import { getControllerInput } from '../../api';
  import {
    controllerBatteryDetail,
    controllerConnectionText,
    controllerDiagnosticDetail,
    controllerModelText,
    controllerPermissionDetail,
    controllerTransportDetail
  } from '../../controllerDisplay';
  import type {
    ControllerConfiguration,
    ControllerInputButtonState,
    ControllerInputMode,
    ControllerInputState,
    ControllerInputStickState,
    ControllerStatus,
    EdgeProfileSlot,
    EdgeProfilesResponse,
    InputBridgeStatus
  } from '../../types';

  type ObservedStickRange = {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
    maxMagnitude: number;
  };

  type ObservedInputRange = {
    leftStick: ObservedStickRange;
    rightStick: ObservedStickRange;
    l2Min: number;
    l2Max: number;
    r2Min: number;
    r2Max: number;
  };

  const LIVE_INPUT_POLL_INTERVAL_MS = 40;
  const EMPTY_STICK: ControllerInputStickState = { x: 0, y: 0, magnitude: 0 };

  export let active = false;
  export let controllers: ControllerStatus[] = [];
  export let controller: ControllerStatus | undefined = undefined;
  export let selectedControllerId: string | null = null;
  export let renameActiveId = '';
  export let renameName = '';
  export let renameBusy = false;
  export let currentControllerConfig: ControllerConfiguration | null = null;
  export let leftStickDeadzone = 0;
  export let rightStickDeadzone = 0;
  export let edgeProfiles: EdgeProfilesResponse | null = null;
  export let edgeProfilesLoading = false;
  export let edgeProfilesBusySlot = '';
  export let edgeProfilesError = '';
  export let edgeSlotsReadTooltip = '';
  export let edgeSlotWriteLabel = 'Write';
  export let inputBridge: InputBridgeStatus | null = null;
  export let activeGameName: string | null = null;
  export let activeInputProvider = 'native_dualsense';
  export let inputBridgeBusy: 'mode' | 'start' | 'stop' | '' = '';
  export let onSelect: (controllerId: string) => void = () => {};
  export let onBeginRename: (item: ControllerStatus) => void = () => {};
  export let onSubmitRename: () => void | Promise<void> = () => {};
  export let onCancelRename: () => void = () => {};
  export let onRenameKeydown: (event: KeyboardEvent) => void = () => {};
  export let onSetInputMode: (mode: ControllerInputMode) => void | Promise<void> = () => {};
  export let onSetStickDeadzone: (side: 'left' | 'right', value: number) => void | Promise<void> = () => {};
  export let onStartInputBridge: () => void | Promise<void> = () => {};
  export let onStopInputBridge: () => void | Promise<void> = () => {};
  export let onRefreshEdgeProfiles: () => void | Promise<void> = () => {};
  export let onWriteEdgeSlot: (slot: EdgeProfileSlot) => void | Promise<void> = () => {};
  export let edgeSlotName: (slot: EdgeProfileSlot) => string = (slot) => slot.name ?? slot.shortcut;
  export let edgeSlotStatus: (slot: EdgeProfileSlot) => string = (slot) => slot.state;
  export let edgeSlotInfoTooltip: (slot: EdgeProfileSlot) => string = (slot) => edgeSlotStatus(slot);
  export let edgeSlotWriteTooltip: (slot: EdgeProfileSlot) => string = (slot) => `Write ${edgeSlotName(slot)}`;

  let inputState: ControllerInputState | null = null;
  let inputFresh = false;
  let inputBusy = false;
  let inputPollTimer: number | undefined;
  let inputFrame: number | undefined;
  let pendingInputState: ControllerInputState | null = null;
  let observedForController = '';
  let observed = emptyObservedInputRange();

  $: selectedInput = inputFresh ? inputState : null;
  $: leftStick = selectedInput?.axes.leftStick ?? EMPTY_STICK;
  $: rightStick = selectedInput?.axes.rightStick ?? EMPTY_STICK;
  $: l2Value = selectedInput?.triggers.l2 ?? 0;
  $: r2Value = selectedInput?.triggers.r2 ?? 0;
  $: visibleButtons = visibleInputButtons(selectedInput?.buttons ?? [], controller);
  $: bridgeSession = inputBridge?.sessions.find((item) => item.controllerId === controller?.id) ?? null;
  $: appRequiresBridge = activeInputProvider === 'dscc_input_bridge';
  $: controllerBridgeConfigured =
    currentControllerConfig?.inputMode === 'dscc_input_bridge' && Boolean(currentControllerConfig?.inputBridge?.enabled);
  $: bridgeSessionActive = bridgeSession?.state === 'active';
  $: powerDiagnostics = controller?.powerDiagnostics ?? null;
  $: powerSuggestions = batteryFriendlySuggestions(controller, currentControllerConfig);
  $: if ((controller?.id ?? '') !== observedForController) {
    observedForController = controller?.id ?? '';
    observed = emptyObservedInputRange();
    resetInputState();
  }
  $: syncInputPolling();

  function emptyObservedStickRange(): ObservedStickRange {
    return {
      minX: 0,
      maxX: 0,
      minY: 0,
      maxY: 0,
      maxMagnitude: 0
    };
  }

  function emptyObservedInputRange(): ObservedInputRange {
    return {
      leftStick: emptyObservedStickRange(),
      rightStick: emptyObservedStickRange(),
      l2Min: 0,
      l2Max: 0,
      r2Min: 0,
      r2Max: 0
    };
  }

  function shouldPollInput() {
    return Boolean(
      active &&
        controller?.id &&
        typeof window !== 'undefined' &&
        typeof document !== 'undefined' &&
        !document.hidden
    );
  }

  function startInputPolling() {
    if (!shouldPollInput()) return;
    if (inputPollTimer !== undefined) return;
    void pollInput();
    inputPollTimer = window.setInterval(() => void pollInput(), LIVE_INPUT_POLL_INTERVAL_MS);
  }

  function stopInputPolling() {
    if (inputPollTimer !== undefined) {
      window.clearInterval(inputPollTimer);
      inputPollTimer = undefined;
    }
    clearInputFrame();
    pendingInputState = null;
    inputBusy = false;
  }

  function syncInputPolling() {
    if (shouldPollInput()) startInputPolling();
    else stopInputPolling();
  }

  async function pollInput() {
    if (inputBusy || !shouldPollInput()) return;
    const requestedControllerId = controller?.id;
    if (!requestedControllerId) return;
    inputBusy = true;
    try {
      const next = await getControllerInput(requestedControllerId);
      if (!shouldPollInput() || next.controllerId !== requestedControllerId || controller?.id !== requestedControllerId) {
        return;
      }
      queueInputState(next);
    } catch {
      if (!shouldPollInput()) return;
      inputFresh = false;
    } finally {
      inputBusy = false;
    }
  }

  function clearInputFrame() {
    if (inputFrame !== undefined && typeof window !== 'undefined') {
      window.cancelAnimationFrame(inputFrame);
    }
    inputFrame = undefined;
  }

  function resetInputState() {
    pendingInputState = null;
    clearInputFrame();
    inputState = null;
    inputFresh = false;
  }

  function queueInputState(next: ControllerInputState) {
    pendingInputState = next;
    if (typeof window === 'undefined' || typeof window.requestAnimationFrame !== 'function') {
      flushInputState();
      return;
    }
    if (inputFrame === undefined) {
      inputFrame = window.requestAnimationFrame(flushInputState);
    }
  }

  function flushInputState() {
    inputFrame = undefined;
    const next = pendingInputState;
    pendingInputState = null;
    if (!next || !shouldPollInput() || next.controllerId !== controller?.id) return;
    inputState = next;
    inputFresh = next.available;
    if (next.available) recordObservedInput(next);
  }

  function recordObservedInput(input: ControllerInputState) {
    observed = {
      leftStick: observeStick(observed.leftStick, input.axes.leftStick),
      rightStick: observeStick(observed.rightStick, input.axes.rightStick),
      l2Min: Math.min(observed.l2Min, input.triggers.l2),
      l2Max: Math.max(observed.l2Max, input.triggers.l2),
      r2Min: Math.min(observed.r2Min, input.triggers.r2),
      r2Max: Math.max(observed.r2Max, input.triggers.r2)
    };
  }

  function observeStick(range: ObservedStickRange, stick: ControllerInputStickState): ObservedStickRange {
    return {
      minX: Math.min(range.minX, stick.x),
      maxX: Math.max(range.maxX, stick.x),
      minY: Math.min(range.minY, stick.y),
      maxY: Math.max(range.maxY, stick.y),
      maxMagnitude: Math.max(range.maxMagnitude, stick.magnitude)
    };
  }

  function visibleInputButtons(buttons: ControllerInputButtonState[], item: ControllerStatus | undefined) {
    if (item?.family === 'DualSense Edge') return buttons;
    return buttons.filter((button) => !button.id.startsWith('edge_'));
  }

  function percent(value: number) {
    return `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
  }

  function signedPercent(value: number) {
    return `${Math.round(Math.max(-1, Math.min(1, value)) * 100)}`;
  }

  function plotPosition(value: number) {
    return 50 + Math.max(-1, Math.min(1, value)) * 45;
  }

  function stickStyle(stick: ControllerInputStickState) {
    return `--stick-x:${plotPosition(stick.x)}%;--stick-y:${plotPosition(stick.y)}%;--stick-mag:${Math.max(8, stick.magnitude * 100)}%;`;
  }

  function triggerStyle(value: number) {
    return `--trigger-fill:${Math.max(0, Math.min(1, value)) * 100}%;`;
  }

  function rangePair(min: number, max: number) {
    return `${percent(min)} / ${percent(max)}`;
  }

  function stickRange(range: ObservedStickRange) {
    return `X ${signedPercent(range.minX)}..${signedPercent(range.maxX)} / Y ${signedPercent(range.minY)}..${signedPercent(range.maxY)}`;
  }

  function suggestedDeadzone(stick: ControllerInputStickState) {
    return `${Math.min(40, Math.max(3, Math.ceil(stick.magnitude * 100 + 2)))}%`;
  }

  function inputFreshness(input: ControllerInputState | null) {
    if (!input?.available) return input?.message ?? 'Input unavailable';
    if (typeof input.ageMs === 'number') return `${input.source} / ${input.ageMs}ms`;
    if (typeof input.sampledAtMs === 'number') return `${input.source} / ${Math.max(0, Date.now() - input.sampledAtMs)}ms`;
    return input.source;
  }

  function statusTone(item: ControllerStatus | undefined) {
    if (!item) return 'No controller';
    if (!item.connected) return controllerDiagnosticDetail(item);
    return controllerConnectionText(item);
  }

  function inputPathTitle() {
    if (bridgeSessionActive) {
      return 'DSCC Input Bridge';
    }
    if (controllerBridgeConfigured) return 'DSCC Input Bridge Ready';
    if (appRequiresBridge) return 'Bridge Available';
    if (currentControllerConfig?.inputMode === 'steam_input_companion' || activeInputProvider === 'steam_input') {
      return 'Steam Input Companion';
    }
    return 'Native DualSense';
  }

  function inputPathDetail() {
    if (bridgeSessionActive || controllerBridgeConfigured || appRequiresBridge) {
      return inputBridge?.available
        ? `${inputBridge.provider} / ${inputBridge.state}`
        : inputBridge?.message ?? 'Bridge backend unavailable';
    }
    if (inputPathTitle() === 'Steam Input Companion') {
      return 'Steam handles game-facing mapping; DSCC keeps typed haptics and diagnostics.';
    }
    return 'Physical controller input is passed through normally.';
  }

  function duplicateInputDetail() {
    if (!controller) return 'No physical controller selected';
    if (bridgeSessionActive) return 'Bridge active; hide the physical controller only if duplicate game input appears.';
    if (controllerBridgeConfigured || appRequiresBridge) {
      return inputBridge?.available
        ? 'Physical controller remains visible until a bridge session starts.'
        : 'Bridge unavailable; physical controller remains visible.';
    }
    if (inputPathTitle() === 'Steam Input Companion') {
      return 'Steam may expose a virtual layout while DSCC keeps diagnostics on the physical controller.';
    }
    return 'Physical controller visible';
  }

  function bridgeSessionState() {
    return bridgeSession ? `${bridgeSession.state} / ${bridgeSession.message}` : 'No bridge session active';
  }

  function formatHz(value: number | null | undefined) {
    return typeof value === 'number' ? `${value.toFixed(value >= 10 ? 0 : 1)} Hz` : 'Unavailable';
  }

  function formatMs(value: number | null | undefined) {
    return typeof value === 'number' ? `${Math.round(value)}ms` : 'Unavailable';
  }

  function formatCount(value: number | null | undefined) {
    return typeof value === 'number' ? Math.round(value).toLocaleString() : 'Unavailable';
  }

  function formatFlag(value: boolean | null | undefined, yes: string, no: string) {
    if (typeof value !== 'boolean') return 'Unavailable';
    return value ? yes : no;
  }

  function hasPowerMetrics(item: ControllerStatus | undefined) {
    const diagnostics = item?.powerDiagnostics;
    if (!diagnostics) return false;
    return Object.values(diagnostics).some((value) => value !== null && value !== undefined);
  }

  function batteryFriendlySuggestions(
    item: ControllerStatus | undefined,
    config: ControllerConfiguration | null
  ) {
    const suggestions: string[] = [];
    const diagnostics = item?.powerDiagnostics;
    const nativeRumble =
      diagnostics?.nativeRumblePassthrough ?? config?.forza.bodyRumbleMode === 'native_passthrough';
    const adaptiveTriggers =
      diagnostics?.adaptiveTriggersRetained ??
      Boolean(config?.trigger.effect && config.trigger.effect.toLowerCase() !== 'off');
    const lightbarBrightness = config?.lightbar.enabled ? config.lightbar.brightness : 0;

    suggestions.push(
      nativeRumble
        ? 'Native rumble passthrough is preferred for body motors; keep it when the game already drives strong rumble.'
        : 'Prefer native rumble passthrough for body motors when a profile does not need DSCC-only rumble shaping.'
    );
    suggestions.push(
      adaptiveTriggers
        ? 'Adaptive triggers are retained; reduce redundant writes before weakening trigger effects.'
        : 'Retain adaptive triggers for drive cues, then save power through write cadence and lighting.'
    );
    suggestions.push(
      lightbarBrightness > 45
        ? 'Dim lightbar and player LEDs when visual telemetry is not needed; haptics stay intact.'
        : 'Keep lightbar and player LEDs modest; use haptics for primary feedback.'
    );

    return suggestions;
  }

  onMount(() => {
    document.addEventListener('visibilitychange', syncInputPolling);
    return () => {
      document.removeEventListener('visibilitychange', syncInputPolling);
      stopInputPolling();
    };
  });

  onDestroy(stopInputPolling);
</script>

<section class="dm-controllers-page" aria-label="Controllers">
  <div class="dm-games-column">
    <div class="dm-games-head">
      <span>Hardware</span>
      <h2>Controllers</h2>
    </div>
    <div class="dm-controller-choice-list">
      {#if controllers.length}
        {#each controllers as item, index (item.id)}
          <ControllerCard
            {item}
            {index}
            selected={item.id === selectedControllerId}
            renameActive={renameActiveId === item.id}
            bind:renameName
            renameBusy={renameBusy}
            onSelect={onSelect}
            onBeginRename={onBeginRename}
            onSubmitRename={onSubmitRename}
            onCancelRename={onCancelRename}
            onRenameKeydown={onRenameKeydown}
          />
        {/each}
      {:else}
        <div class="dm-empty-choice">
          <strong>No controller detected</strong>
          <span>Controller unavailable</span>
        </div>
      {/if}
    </div>

    {#if controller?.family === 'DualSense Edge'}
      <section class="dm-edge-slots" aria-label="DualSense Edge onboard profiles">
        <div class="dm-edge-slots-head">
          <div>
            <span>Onboard Memory</span>
            <strong>Edge Slots</strong>
          </div>
          <Tooltip text={edgeSlotsReadTooltip} side="bottom" align="end">
            <button
              type="button"
              class="dm-mini-button"
              disabled={edgeProfilesLoading}
              aria-label="Refresh DualSense Edge onboard slots"
              onclick={() => void onRefreshEdgeProfiles()}
            >
              {edgeProfilesLoading ? 'Reading' : 'Read'}
            </button>
          </Tooltip>
        </div>

        {#if edgeProfilesError}
          <p class="dm-edge-slots-note error">{edgeProfilesError}</p>
        {:else if edgeProfiles?.warning}
          <p class="dm-edge-slots-note">{edgeProfiles.warning}</p>
        {/if}

        <div class="dm-edge-slot-list">
          {#if edgeProfiles?.slots.length}
            {#each edgeProfiles.slots as slot (slot.slotId)}
              <div class="dm-edge-slot-row" class:disabled={!slot.editable}>
                <Tooltip block text={edgeSlotInfoTooltip(slot)} side="right" align="start">
                  <div class="dm-edge-slot-copy">
                    <span>{slot.shortcut}</span>
                    <strong>{edgeSlotName(slot)}</strong>
                    <small>{edgeSlotStatus(slot)}</small>
                  </div>
                </Tooltip>
                {#if slot.editable}
                  <Tooltip text={edgeSlotWriteTooltip(slot)} side="left" align="center">
                    <button
                      type="button"
                      class="dm-mini-button primary"
                      disabled={!currentControllerConfig || edgeProfilesBusySlot === slot.slotId}
                      onclick={() => void onWriteEdgeSlot(slot)}
                    >
                      {edgeProfilesBusySlot === slot.slotId ? 'Writing' : edgeSlotWriteLabel}
                    </button>
                  </Tooltip>
                {/if}
              </div>
            {/each}
          {:else}
            <div class="dm-edge-slot-row disabled">
              <div>
                <span>Fn Slots</span>
                <strong>{edgeProfilesLoading ? 'Reading slots' : 'No slot data'}</strong>
                <small>{edgeProfilesLoading ? 'controller scan' : 'unavailable'}</small>
              </div>
            </div>
          {/if}
        </div>
      </section>
    {/if}
  </div>

  <div class="dm-controllers-workbench">
    <section class="dm-controller-overview" aria-label="Selected controller details">
      <div class="dm-controller-overview-copy">
        <span>Selected</span>
        <strong>{controllerModelText(controller)}</strong>
        <small>{statusTone(controller)}</small>
      </div>
      {#if controller}
        <dl class="dm-controller-metric-grid">
          <div>
            <dt>Connection</dt>
            <dd>{controllerTransportDetail(controller)}</dd>
          </div>
          <div>
            <dt>Battery</dt>
            <dd>{controllerBatteryDetail(controller)}</dd>
          </div>
          <div>
            <dt>Permission</dt>
            <dd>{controllerPermissionDetail(controller)}</dd>
          </div>
          <div>
            <dt>Diagnostics</dt>
            <dd>{controllerDiagnosticDetail(controller)}</dd>
          </div>
          <div class="wide">
            <dt>Sanitized ID</dt>
            <dd class="mono">{controller.id}</dd>
          </div>
        </dl>
      {/if}
    </section>

    {#if !controller}
      <section class="dm-controller-empty-state" aria-label="No controller selected">
        <strong>No controller selected</strong>
        <span>Connect a DualSense controller to view input routing, live stick plots, trigger travel, and calibration readings.</span>
      </section>
    {:else}
    <section class="dm-input-path-panel" aria-label="Controller input path">
      <div class="dm-live-panel-head">
        <div>
          <span>Input Path</span>
          <strong>{inputPathTitle()}</strong>
        </div>
        <code>{activeGameName ?? 'No active app'}</code>
      </div>
      <dl class="dm-controller-metric-grid compact">
        <div>
          <dt>Provider</dt>
          <dd>{inputPathDetail()}</dd>
        </div>
        <div>
          <dt>Duplicate Input</dt>
          <dd>{duplicateInputDetail()}</dd>
        </div>
        <div class="wide">
          <dt>Bridge Session</dt>
          <dd>{bridgeSessionState()}</dd>
        </div>
      </dl>
      <div class="dm-input-path-actions" aria-label="Input path controls">
        <button
          type="button"
          class:active={currentControllerConfig?.inputMode === 'native_dualsense'}
          disabled={!controller || !currentControllerConfig || Boolean(inputBridgeBusy)}
          aria-pressed={currentControllerConfig?.inputMode === 'native_dualsense'}
          onclick={() => void onSetInputMode('native_dualsense')}
        >Native</button>
        <button
          type="button"
          class:active={currentControllerConfig?.inputMode === 'steam_input_companion'}
          disabled={!controller || !currentControllerConfig || Boolean(inputBridgeBusy)}
          aria-pressed={currentControllerConfig?.inputMode === 'steam_input_companion'}
          onclick={() => void onSetInputMode('steam_input_companion')}
        >Steam</button>
        <button
          type="button"
          class:active={controllerBridgeConfigured}
          disabled={!controller || !currentControllerConfig || Boolean(inputBridgeBusy)}
          aria-pressed={controllerBridgeConfigured}
          onclick={() => void onSetInputMode('dscc_input_bridge')}
        >Bridge</button>
        <span></span>
        <button
          type="button"
          class="primary"
          disabled={!controller || !controllerBridgeConfigured || !appRequiresBridge || bridgeSessionActive || !inputBridge?.available || Boolean(inputBridgeBusy)}
          onclick={() => void onStartInputBridge()}
        >{inputBridgeBusy === 'start' ? 'Starting' : 'Start'}</button>
        <button
          type="button"
          disabled={!controller || !bridgeSession || !bridgeSessionActive || Boolean(inputBridgeBusy)}
          onclick={() => void onStopInputBridge()}
        >{inputBridgeBusy === 'stop' ? 'Stopping' : 'Stop'}</button>
      </div>
      {#if controllerBridgeConfigured && !appRequiresBridge}
        <p class="dm-edge-slots-note">Bridge sessions start only while the selected local app is active.</p>
      {/if}
      {#if inputBridge?.warnings.length && inputPathTitle() === 'DSCC Input Bridge'}
        <p class="dm-edge-slots-note">{inputBridge.warnings[0]}</p>
      {/if}
    </section>

    <section class="dm-power-diagnostics-panel" aria-label="Power diagnostics">
      <div class="dm-live-panel-head">
        <div>
          <span>Power Diagnostics</span>
          <strong>{hasPowerMetrics(controller) ? 'Output Cadence' : 'Awaiting Agent Metrics'}</strong>
        </div>
        <code>{controller.transport}</code>
      </div>
      <dl class="dm-controller-metric-grid compact">
        <div>
          <dt>Write Rate</dt>
          <dd>{formatHz(powerDiagnostics?.outputWriteRateHz)}</dd>
        </div>
        <div>
          <dt>Cadence</dt>
          <dd>{formatMs(powerDiagnostics?.outputCadenceMs)}</dd>
        </div>
        <div>
          <dt>Suppressed</dt>
          <dd>{formatCount(powerDiagnostics?.suppressedRedundantReports)}</dd>
        </div>
        <div>
          <dt>Keepalive</dt>
          <dd>{formatMs(powerDiagnostics?.keepaliveIntervalMs)}</dd>
        </div>
        <div>
          <dt>Last Write</dt>
          <dd>{formatMs(powerDiagnostics?.lastWriteAgeMs)}</dd>
        </div>
        <div>
          <dt>Rumble Path</dt>
          <dd>{formatFlag(powerDiagnostics?.nativeRumblePassthrough, 'native passthrough', 'DSCC shaped')}</dd>
        </div>
        <div class="wide">
          <dt>Trigger Policy</dt>
          <dd>{formatFlag(powerDiagnostics?.adaptiveTriggersRetained, 'adaptive triggers retained', 'adaptive triggers not retained')}</dd>
        </div>
      </dl>
      <div class="dm-power-suggestion-list" aria-label="Battery-friendly haptic guidance">
        {#each powerSuggestions as suggestion}
          <p>{suggestion}</p>
        {/each}
      </div>
    </section>

    <section class="dm-live-input-panel" aria-label="Live controller input">
      <div class="dm-live-panel-head">
        <div>
          <span>Live Input</span>
          <strong>{inputFresh ? 'Streaming' : 'Unavailable'}</strong>
        </div>
        <code>{inputFreshness(inputState)}</code>
      </div>

      <div class="dm-live-stick-grid">
        <article class="dm-stick-module">
          <div class="dm-stick-head">
            <span>Left Stick</span>
            <code>{signedPercent(leftStick.x)} / {signedPercent(leftStick.y)}</code>
          </div>
          <div class="dm-stick-plot" style={stickStyle(leftStick)} aria-hidden="true">
            <span class="dm-stick-ring"></span>
            <span class="dm-stick-dot"></span>
          </div>
          <dl>
            <div>
              <dt>Drift</dt>
              <dd>{percent(leftStick.magnitude)}</dd>
            </div>
            <div>
              <dt>Suggested DZ</dt>
              <dd>{suggestedDeadzone(leftStick)}</dd>
            </div>
          </dl>
          <div class="dm-stick-tuning-row">
            <span>Deadzone</span>
            <input
              class="dm-mini-range"
              style="--value:{leftStickDeadzone * 2.5}%"
              type="range"
              min="0"
              max="40"
              value={leftStickDeadzone}
              disabled={!controller || !currentControllerConfig}
              aria-label="Left stick deadzone"
              oninput={(event) => void onSetStickDeadzone('left', event.currentTarget.valueAsNumber)}
            />
            <code>{leftStickDeadzone}%</code>
          </div>
        </article>

        <article class="dm-stick-module">
          <div class="dm-stick-head">
            <span>Right Stick</span>
            <code>{signedPercent(rightStick.x)} / {signedPercent(rightStick.y)}</code>
          </div>
          <div class="dm-stick-plot" style={stickStyle(rightStick)} aria-hidden="true">
            <span class="dm-stick-ring"></span>
            <span class="dm-stick-dot"></span>
          </div>
          <dl>
            <div>
              <dt>Drift</dt>
              <dd>{percent(rightStick.magnitude)}</dd>
            </div>
            <div>
              <dt>Suggested DZ</dt>
              <dd>{suggestedDeadzone(rightStick)}</dd>
            </div>
          </dl>
          <div class="dm-stick-tuning-row">
            <span>Deadzone</span>
            <input
              class="dm-mini-range"
              style="--value:{rightStickDeadzone * 2.5}%"
              type="range"
              min="0"
              max="40"
              value={rightStickDeadzone}
              disabled={!controller || !currentControllerConfig}
              aria-label="Right stick deadzone"
              oninput={(event) => void onSetStickDeadzone('right', event.currentTarget.valueAsNumber)}
            />
            <code>{rightStickDeadzone}%</code>
          </div>
        </article>
      </div>

      <div class="dm-trigger-meter-grid">
        <article class="dm-trigger-meter" style={triggerStyle(l2Value)}>
          <div>
            <span>L2</span>
            <strong>{percent(l2Value)}</strong>
          </div>
          <div class="dm-trigger-bar" aria-hidden="true"><span></span></div>
        </article>
        <article class="dm-trigger-meter" style={triggerStyle(r2Value)}>
          <div>
            <span>R2</span>
            <strong>{percent(r2Value)}</strong>
          </div>
          <div class="dm-trigger-bar" aria-hidden="true"><span></span></div>
        </article>
      </div>

      <div class="dm-button-grid" aria-label="Live button states">
        {#each visibleButtons as button (button.id)}
          <div class:pressed={button.pressed} class="dm-button-state">
            <span>{button.label}</span>
            <code>{button.id === 'l2' || button.id === 'r2' ? percent(button.value) : button.pressed ? 'ON' : 'OFF'}</code>
          </div>
        {/each}
      </div>
    </section>

    <section class="dm-calibration-readout" aria-label="Calibration measurements">
      <div class="dm-live-panel-head">
        <div>
          <span>Calibration</span>
          <strong>Session Readings</strong>
        </div>
      </div>
      <dl class="dm-calibration-grid">
        <div>
          <dt>Left Range</dt>
          <dd>{stickRange(observed.leftStick)}</dd>
        </div>
        <div>
          <dt>Right Range</dt>
          <dd>{stickRange(observed.rightStick)}</dd>
        </div>
        <div>
          <dt>L2 Range</dt>
          <dd>{rangePair(observed.l2Min, observed.l2Max)}</dd>
        </div>
        <div>
          <dt>R2 Range</dt>
          <dd>{rangePair(observed.r2Min, observed.r2Max)}</dd>
        </div>
      </dl>
    </section>
    {/if}
  </div>
</section>
