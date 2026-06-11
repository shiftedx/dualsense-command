<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import ControllerCard from './ControllerCard.svelte';
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
  export let glyphOverrideEnabled = false;
  export let glyphOverrideBusy = false;
  export let glyphOverrideTitle = '';
  export let onToggleGlyphOverride: () => void | Promise<void> = () => {};
  export let supportBundleBusy: 'copy' | 'download' | '' = '';
  export let onDownloadSupportBundle: () => void | Promise<void> = () => {};

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
  $: alias = controller?.name || controller?.family || 'No controller';
  $: stickDriftLine = driftSummary(selectedInput, leftStick, rightStick);
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

  /** Plain-words stick summary; raw per-stick values stay in the group below. */
  function driftSummary(
    input: ControllerInputState | null,
    left: ControllerInputStickState,
    right: ControllerInputStickState
  ) {
    if (!input) return 'waiting for input';
    const max = Math.max(left.magnitude, right.magnitude);
    if (max < 0.05) return 'centered · no drift detected';
    return `reading ${percent(max)} of travel — fine while you move them; if untouched, raise the deadzone`;
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
        : inputBridge?.message ?? 'Bridge unavailable';
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

<section class="ctl-view" aria-label="Controller details">
  <div class="ctl-head">
    <h1 class="ctl-title">Controller details</h1>
    <span class="ctl-sub">{alias} &middot; live readouts for checking, not for everyday tuning</span>
  </div>

  <div class="ctl-groups">
    {#if controllers.length > 1}
      <div class="ctl-group ctl-group-controllers">
        <div class="lbl">Controllers</div>
        <div class="ctl-controller-list">
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
        </div>
      </div>
    {/if}

    {#if !controller}
      <div class="ctl-group">
        <div class="lbl">Live input</div>
        <div class="ctl-surf ctl-empty">
          <strong>No controller selected</strong>
          <span>Connect a DualSense controller to view live input, trigger travel, and calibration readings.</span>
        </div>
      </div>
    {:else}
      <div class="ctl-group">
        <div class="lbl">Live input</div>
        <div class="ctl-surf" aria-label="Live controller input">
          <div class="ctl-meter-row">
            <span>L2</span>
            <span class="ctl-mut">{percent(l2Value)}</span>
          </div>
          <div class="ctl-meter" aria-hidden="true"><span style={triggerStyle(l2Value)}></span></div>
          <div class="ctl-meter-row">
            <span>R2</span>
            <span class="ctl-mut">{percent(r2Value)}</span>
          </div>
          <div class="ctl-meter" aria-hidden="true"><span style={triggerStyle(r2Value)}></span></div>
          <div class="ctl-meter-row">
            <span>Sticks</span>
            <span class="ctl-mut">{stickDriftLine}</span>
          </div>
          <div class="ctl-meter-row">
            <span>Feed</span>
            <span class="ctl-mut ctl-mono">{inputFreshness(inputState)}</span>
          </div>
        </div>

        <div class="ctl-stick-grid">
          <article class="ctl-stick">
            <div class="ctl-meter-row">
              <span>Left stick</span>
              <span class="ctl-mut ctl-mono">{signedPercent(leftStick.x)} / {signedPercent(leftStick.y)}</span>
            </div>
            <div class="ctl-stick-plot" style={stickStyle(leftStick)} aria-hidden="true">
              <span class="ctl-stick-ring"></span>
              <span class="ctl-stick-dot"></span>
            </div>
            <div class="ctl-rows">
              <div class="ctl-row"><span>Off-center now</span><span class="ctl-mut">{percent(leftStick.magnitude)}</span></div>
              <div class="ctl-row"><span>Suggested deadzone</span><span class="ctl-mut">{suggestedDeadzone(leftStick)}</span></div>
            </div>
            <div class="ctl-deadzone-row">
              <span>Deadzone</span>
              <input
                type="range"
                min="0"
                max="40"
                value={leftStickDeadzone}
                disabled={!controller || !currentControllerConfig}
                aria-label="Left stick deadzone"
                oninput={(event) => void onSetStickDeadzone('left', event.currentTarget.valueAsNumber)}
              />
              <span class="ctl-mono">{leftStickDeadzone}%</span>
            </div>
          </article>
          <article class="ctl-stick">
            <div class="ctl-meter-row">
              <span>Right stick</span>
              <span class="ctl-mut ctl-mono">{signedPercent(rightStick.x)} / {signedPercent(rightStick.y)}</span>
            </div>
            <div class="ctl-stick-plot" style={stickStyle(rightStick)} aria-hidden="true">
              <span class="ctl-stick-ring"></span>
              <span class="ctl-stick-dot"></span>
            </div>
            <div class="ctl-rows">
              <div class="ctl-row"><span>Off-center now</span><span class="ctl-mut">{percent(rightStick.magnitude)}</span></div>
              <div class="ctl-row"><span>Suggested deadzone</span><span class="ctl-mut">{suggestedDeadzone(rightStick)}</span></div>
            </div>
            <div class="ctl-deadzone-row">
              <span>Deadzone</span>
              <input
                type="range"
                min="0"
                max="40"
                value={rightStickDeadzone}
                disabled={!controller || !currentControllerConfig}
                aria-label="Right stick deadzone"
                oninput={(event) => void onSetStickDeadzone('right', event.currentTarget.valueAsNumber)}
              />
              <span class="ctl-mono">{rightStickDeadzone}%</span>
            </div>
          </article>
        </div>

        <div class="ctl-button-grid" aria-label="Live button states">
          {#each visibleButtons as button (button.id)}
            <div class:pressed={button.pressed} class="ctl-button-state">
              <span>{button.label}</span>
              <span class="ctl-mono">{button.id === 'l2' || button.id === 'r2' ? percent(button.value) : button.pressed ? 'ON' : 'OFF'}</span>
            </div>
          {/each}
        </div>
      </div>

      <div class="ctl-group">
        <div class="lbl">Connection</div>
        <div class="ctl-rows">
          <div class="ctl-row"><span>Controller</span><span class="ctl-mut">{controllerModelText(controller)}</span></div>
          <div class="ctl-row"><span>State</span><span class="ctl-mut">{statusTone(controller)}</span></div>
          <div class="ctl-row"><span>Transport</span><span class="ctl-mut">{controllerTransportDetail(controller)}</span></div>
          <div class="ctl-row"><span>Battery</span><span class="ctl-mut">{controllerBatteryDetail(controller)}</span></div>
          <div class="ctl-row"><span>Permission</span><span class="ctl-mut">{controllerPermissionDetail(controller)}</span></div>
          <div class="ctl-row"><span>Diagnostics</span><span class="ctl-mut">{controllerDiagnosticDetail(controller)}</span></div>
          <div class="ctl-row"><span>Sanitized ID</span><span class="ctl-mut ctl-mono">{controller.id}</span></div>
        </div>

        <div class="lbl ctl-sublabel">Input path</div>
        <div class="ctl-rows">
          <div class="ctl-row"><span>Path</span><span class="ctl-mut">{inputPathTitle()}</span></div>
          <div class="ctl-row"><span>Active app</span><span class="ctl-mut">{activeGameName ?? 'No active app'}</span></div>
          <div class="ctl-row"><span>Provider</span><span class="ctl-mut">{inputPathDetail()}</span></div>
          <div class="ctl-row"><span>Duplicate input</span><span class="ctl-mut">{duplicateInputDetail()}</span></div>
          <div class="ctl-row"><span>Bridge session</span><span class="ctl-mut">{bridgeSessionState()}</span></div>
        </div>
        <div class="ctl-actions" aria-label="Input path controls">
          <button
            type="button"
            class="ctl-button"
            class:active={currentControllerConfig?.inputMode === 'native_dualsense'}
            disabled={!controller || !currentControllerConfig || Boolean(inputBridgeBusy)}
            aria-pressed={currentControllerConfig?.inputMode === 'native_dualsense'}
            onclick={() => void onSetInputMode('native_dualsense')}
          >Native</button>
          <button
            type="button"
            class="ctl-button"
            class:active={currentControllerConfig?.inputMode === 'steam_input_companion'}
            disabled={!controller || !currentControllerConfig || Boolean(inputBridgeBusy)}
            aria-pressed={currentControllerConfig?.inputMode === 'steam_input_companion'}
            onclick={() => void onSetInputMode('steam_input_companion')}
          >Steam</button>
          <button
            type="button"
            class="ctl-button"
            class:active={controllerBridgeConfigured}
            disabled={!controller || !currentControllerConfig || Boolean(inputBridgeBusy)}
            aria-pressed={controllerBridgeConfigured}
            onclick={() => void onSetInputMode('dscc_input_bridge')}
          >Bridge</button>
          <button
            type="button"
            class="ctl-button primary"
            disabled={!controller || !controllerBridgeConfigured || !appRequiresBridge || bridgeSessionActive || !inputBridge?.available || Boolean(inputBridgeBusy)}
            onclick={() => void onStartInputBridge()}
          >{inputBridgeBusy === 'start' ? 'Starting' : 'Start'}</button>
          <button
            type="button"
            class="ctl-button"
            disabled={!controller || !bridgeSession || !bridgeSessionActive || Boolean(inputBridgeBusy)}
            onclick={() => void onStopInputBridge()}
          >{inputBridgeBusy === 'stop' ? 'Stopping' : 'Stop'}</button>
        </div>
        {#if controllerBridgeConfigured && !appRequiresBridge}
          <p class="ctl-note">Bridge sessions start only while the selected local app is active.</p>
        {/if}
        {#if inputBridge?.warnings.length && inputPathTitle() === 'DSCC Input Bridge'}
          <p class="ctl-note">{inputBridge.warnings[0]}</p>
        {/if}
      </div>

      <div class="ctl-group">
        <div class="lbl">Power</div>
        <div class="ctl-rows">
          <div class="ctl-row"><span>Write rate</span><span class="ctl-mut">{formatHz(powerDiagnostics?.outputWriteRateHz)}</span></div>
          <div class="ctl-row"><span>Cadence</span><span class="ctl-mut">{formatMs(powerDiagnostics?.outputCadenceMs)}</span></div>
          <div class="ctl-row"><span>Suppressed writes</span><span class="ctl-mut">{formatCount(powerDiagnostics?.suppressedRedundantReports)}</span></div>
          <div class="ctl-row"><span>Keepalive</span><span class="ctl-mut">{formatMs(powerDiagnostics?.keepaliveIntervalMs)}</span></div>
          <div class="ctl-row"><span>Last write</span><span class="ctl-mut">{formatMs(powerDiagnostics?.lastWriteAgeMs)}</span></div>
          <div class="ctl-row"><span>Rumble path</span><span class="ctl-mut">{formatFlag(powerDiagnostics?.nativeRumblePassthrough, 'native passthrough', 'DSCC shaped')}</span></div>
          <div class="ctl-row"><span>Trigger policy</span><span class="ctl-mut">{formatFlag(powerDiagnostics?.adaptiveTriggersRetained, 'adaptive triggers retained', 'adaptive triggers not retained')}</span></div>
        </div>
        <div class="ctl-suggestions" aria-label="Battery-friendly haptic guidance">
          {#each powerSuggestions as suggestion}
            <p class="ctl-note">{suggestion}</p>
          {/each}
        </div>
      </div>

      <div class="ctl-group">
        <div class="lbl">Session readings</div>
        <div class="ctl-rows">
          <div class="ctl-row"><span>Left range</span><span class="ctl-mut ctl-mono">{stickRange(observed.leftStick)}</span></div>
          <div class="ctl-row"><span>Right range</span><span class="ctl-mut ctl-mono">{stickRange(observed.rightStick)}</span></div>
          <div class="ctl-row"><span>L2 range</span><span class="ctl-mut ctl-mono">{rangePair(observed.l2Min, observed.l2Max)}</span></div>
          <div class="ctl-row"><span>R2 range</span><span class="ctl-mut ctl-mono">{rangePair(observed.r2Min, observed.r2Max)}</span></div>
        </div>
        <p class="ctl-note">Ranges are observed while this page is open; move the sticks and pull the triggers to fill them in.</p>
      </div>
    {/if}

    <div class="ctl-group narrow">
      <div class="lbl">Button icons</div>
      <div class="ctl-setting-row">
        <div>
          <strong>Forza button icons</strong>
          <span>Show PlayStation icons where DSCC manages the supported Forza icon files.</span>
        </div>
        <button
          type="button"
          class="ctl-button"
          class:active={glyphOverrideEnabled}
          disabled={glyphOverrideBusy}
          aria-pressed={glyphOverrideEnabled}
          title={glyphOverrideTitle}
          onclick={() => void onToggleGlyphOverride()}
        >{glyphOverrideEnabled ? 'PlayStation icons' : 'Game default'}</button>
      </div>
    </div>

    <div class="ctl-group narrow">
      <div class="lbl">Support</div>
      <div class="ctl-support">
        <span>Having trouble?</span>
        <p class="ctl-note">Download a Support Bundle &mdash; sanitized diagnostics, no private data.</p>
        <button
          type="button"
          class="ctl-button"
          disabled={Boolean(supportBundleBusy)}
          onclick={() => void onDownloadSupportBundle()}
        >{supportBundleBusy === 'download' ? 'Preparing bundle' : 'Download bundle'}</button>
      </div>
    </div>
  </div>
</section>
