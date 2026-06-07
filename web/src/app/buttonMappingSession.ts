import {
  assembleSteamBindingRaw,
  buildSteamBindingBySlotKey,
  createSteamMirrorGroups,
  parseSteamBindingTriple,
  resolveFocusedSlotKey,
  steamBindingKey,
  steamBindingSlots,
  steamBindingTargetPart,
  type SteamBindingSlot
} from '../lib/features/buttonMapping';
import {
  bindingTargetGroupsForProvider,
  type ButtonMappingProviderKind,
  type ButtonMappingViewSession
} from '../lib/features/buttonMapping/buttonMappingState';
import {
  EMPTY_STEAM_BINDING_MAP,
  EMPTY_STEAM_INPUT_BINDINGS,
  EMPTY_STEAM_MIRROR_GROUPS,
  defaultSteamBindingsForFamily,
  preparedSteamBindingTargetGroups
} from '../lib/features/buttonMapping/steamBindingTargets';
import { writeInputBridgeBinding, writeSteamInputBinding, writeSteamInputPaddlePreset } from '../lib/api';
import type {
  ControllerStatus,
  InputBridgeStatus,
  SteamInputBinding,
  SteamInputLayout,
  SteamInputStatus,
  SupportedGame
} from '../lib/types';
import { toastToneForMessage, type ToastTone } from './toastState';

type ButtonMappingTuningScope = 'none' | 'global' | 'game';

export type ButtonMappingSessionState = {
  selectedBindingKey: string;
  bindingDraft: string;
  bindingLabelDraft: string;
  lastBindingDraftKey: string;
  optimisticBindings: SteamInputBinding[] | null;
  activeContextKey: string;
  bindingBusy: boolean;
  bindingMessage: string;
  paddlePresetLeftKey: string;
  paddlePresetRightKey: string;
  hoveredSlotKey: string;
  activeSlotKey: string;
};

export type ButtonMappingSessionStateStore = {
  get: () => ButtonMappingSessionState;
  set: (next: ButtonMappingSessionState) => void;
  update: (updater: (state: ButtonMappingSessionState) => ButtonMappingSessionState) => void;
};

export type ButtonMappingSessionContext = {
  active: boolean;
  controller: ControllerStatus | null | undefined;
  controllerHeaderName: string;
  selectedTuningScope: ButtonMappingTuningScope;
  steamContextGame: SupportedGame | null | undefined;
  steamInputStatus: SteamInputStatus | undefined;
  inputBridgeStatus: InputBridgeStatus | undefined;
  activeProfileName: string | null | undefined;
  profileContextGameName: string | null | undefined;
  bridgeProfileId: string | null;
  refresh: () => Promise<unknown>;
  notify: (message: string, tone?: ToastTone) => void;
};

export type CreateButtonMappingSessionInput = ButtonMappingSessionContext & {
  state: ButtonMappingSessionState;
  store: ButtonMappingSessionStateStore;
};

export function createButtonMappingSessionState(): ButtonMappingSessionState {
  return {
    selectedBindingKey: '',
    bindingDraft: '',
    bindingLabelDraft: '',
    lastBindingDraftKey: '',
    optimisticBindings: null,
    activeContextKey: '',
    bindingBusy: false,
    bindingMessage: '',
    paddlePresetLeftKey: 'Q',
    paddlePresetRightKey: 'E',
    hoveredSlotKey: '',
    activeSlotKey: ''
  };
}

const normalizedSteamControllerType = (controllerLike: string | null | undefined) => {
  const value = (controllerLike ?? '').toLowerCase();
  if (value.includes('edge')) return 'controller_ps5_edge';
  if (value.includes('dualsense') || value.includes('ps5')) return 'controller_ps5';
  if (value.includes('dualshock') || value.includes('ps4')) return 'controller_ps4';
  return '';
};

const selectSteamInputLayout = (
  layouts: SteamInputLayout[],
  game: SupportedGame | null | undefined,
  controllerFamily: ControllerStatus['family'] | string | null | undefined
) => {
  if (!layouts.length) return null;
  const appId = game?.appId ?? null;
  const controllerType = normalizedSteamControllerType(controllerFamily);
  const sameApp = appId ? layouts.filter((layout) => layout.appId === appId) : [];
  if (!appId || !sameApp.length) return null;
  const candidates = sameApp;
  return (
    candidates.find((layout) => layout.controllerType === controllerType) ??
    candidates.find((layout) => layout.controllerType === 'controller_ps5_edge') ??
    candidates.find((layout) => layout.controllerType === 'controller_ps5') ??
    candidates[0] ??
    null
  );
};

const normalizePaddlePresetKey = (value: string) =>
  value
    .trim()
    .replaceAll(' ', '_')
    .replaceAll('-', '_')
    .toUpperCase()
    .replace(/[^A-Z0-9_]/g, '')
    .slice(0, 32);

const updateSessionState = (
  store: ButtonMappingSessionStateStore,
  patch: Partial<ButtonMappingSessionState>
) => {
  store.update((current) => ({ ...current, ...patch }));
};

const setBindingMessage = (
  store: ButtonMappingSessionStateStore,
  context: ButtonMappingSessionContext,
  message: string,
  tone: ToastTone = toastToneForMessage(message, 'info')
) => {
  updateSessionState(store, { bindingMessage: message });
  context.notify(message, tone);
};

const applyOptimisticBinding = (
  store: ButtonMappingSessionStateStore,
  rawBindings: SteamInputBinding[],
  updatedBinding: SteamInputBinding
) => {
  const updatedKey = steamBindingKey(updatedBinding);
  const baseBindings = store.get().optimisticBindings ?? rawBindings;
  let replaced = false;
  const optimisticBindings = baseBindings.map((binding) => {
    if (steamBindingKey(binding) !== updatedKey) return binding;
    replaced = true;
    return updatedBinding;
  });
  updateSessionState(store, {
    optimisticBindings: replaced ? optimisticBindings : [...optimisticBindings, updatedBinding]
  });
};

export function createButtonMappingSession(input: CreateButtonMappingSessionInput): ButtonMappingViewSession {
  const {
    state,
    store,
    active,
    controller,
    selectedTuningScope,
    steamContextGame,
    steamInputStatus,
    inputBridgeStatus
  } = input;
  const bridgeContextActive =
    active &&
    selectedTuningScope === 'game' &&
    steamContextGame?.inputProvider === 'dscc_input_bridge';
  const providerKind: ButtonMappingProviderKind = bridgeContextActive ? 'bridge' : 'steam';
  const providerLabel = bridgeContextActive ? 'DSCC Input Bridge' : 'Steam Input';
  const providerOnline = bridgeContextActive
    ? Boolean(inputBridgeStatus?.available)
    : Boolean(steamInputStatus?.running);
  const mappingAvailabilityMessage = bridgeContextActive
    ? inputBridgeStatus?.available
      ? 'Bridge edits are staged through DSCC typed mapping and sent through the configured virtual-output provider.'
      : inputBridgeStatus?.message ?? 'DSCC Input Bridge backend is unavailable.'
    : steamInputStatus?.available
      ? ''
      : steamInputStatus?.warnings?.[0] ?? 'Steam Input layout data is unavailable.';
  const steamInputLayout = active
    ? bridgeContextActive
      ? null
      : selectSteamInputLayout(steamInputStatus?.layouts ?? [], steamContextGame, controller?.family)
    : null;
  const rawSteamInputBindings = active
    ? steamInputLayout?.bindings ?? EMPTY_STEAM_INPUT_BINDINGS
    : EMPTY_STEAM_INPUT_BINDINGS;
  const contextKey = [
    active ? steamInputLayout?.source ?? '' : '',
    active ? steamContextGame?.gameId ?? '' : '',
    active ? controller?.id ?? '' : '',
    active ? controller?.family ?? '' : ''
  ].join('|');

  let nextState = state;
  const patchNextState = (patch: Partial<ButtonMappingSessionState>) => {
    nextState = { ...nextState, ...patch };
  };

  if (active && contextKey !== nextState.activeContextKey) {
    patchNextState({
      activeContextKey: contextKey,
      optimisticBindings: null,
      activeSlotKey: '',
      hoveredSlotKey: ''
    });
  }

  const realSteamInputBindings = active
    ? nextState.optimisticBindings ?? rawSteamInputBindings
    : EMPTY_STEAM_INPUT_BINDINGS;
  const realSteamBindingBySlotKey = active
    ? buildSteamBindingBySlotKey(realSteamInputBindings, steamBindingSlots)
    : EMPTY_STEAM_BINDING_MAP;
  const defaultSteamBindingBySlotKey = active
    ? defaultSteamBindingsForFamily(controller?.family)
    : EMPTY_STEAM_BINDING_MAP;
  const steamBindingBySlotKey = active
    ? new Map([...defaultSteamBindingBySlotKey, ...realSteamBindingBySlotKey])
    : EMPTY_STEAM_BINDING_MAP;
  const steamInputBindings = active
    ? [
        ...realSteamInputBindings,
        ...[...defaultSteamBindingBySlotKey.entries()]
          .filter(([slotKey]) => !realSteamBindingBySlotKey.has(slotKey))
          .map(([, binding]) => binding)
      ]
    : EMPTY_STEAM_INPUT_BINDINGS;

  if (
    active &&
    steamInputBindings.length &&
    !nextState.activeSlotKey &&
    !steamInputBindings.some((binding) => steamBindingKey(binding) === nextState.selectedBindingKey)
  ) {
    patchNextState({ selectedBindingKey: steamBindingKey(steamInputBindings[0]) });
  }
  if (active && !steamInputBindings.length && nextState.selectedBindingKey) {
    patchNextState({ selectedBindingKey: '' });
  }

  const selectedSteamBinding =
    active && nextState.selectedBindingKey
      ? steamInputBindings.find((binding) => steamBindingKey(binding) === nextState.selectedBindingKey) ?? null
      : null;
  if (
    active &&
    selectedSteamBinding &&
    steamBindingKey(selectedSteamBinding) !== nextState.lastBindingDraftKey
  ) {
    patchNextState({
      lastBindingDraftKey: steamBindingKey(selectedSteamBinding),
      bindingDraft: selectedSteamBinding.rawBinding,
      bindingLabelDraft: parseSteamBindingTriple(selectedSteamBinding.rawBinding).label,
      bindingMessage: ''
    });
  }

  if (nextState !== state) {
    store.set(nextState);
  }

  const steamPaddlePresetVisible =
    active &&
    !bridgeContextActive &&
    controller?.family === 'DualSense Edge' &&
    selectedTuningScope === 'game';
  const steamPaddlePresetLeftBinding = steamPaddlePresetVisible
    ? steamBindingBySlotKey.get('edgeBackLeft') ?? null
    : null;
  const steamPaddlePresetRightBinding = steamPaddlePresetVisible
    ? steamBindingBySlotKey.get('edgeBackRight') ?? null
    : null;
  const steamPaddlePresetAvailable = Boolean(
    steamPaddlePresetVisible &&
    steamInputLayout &&
    steamPaddlePresetLeftBinding &&
    steamPaddlePresetRightBinding &&
    !steamPaddlePresetLeftBinding.synthetic &&
    !steamPaddlePresetRightBinding.synthetic
  );
  const steamPaddlePresetStatus = !steamPaddlePresetVisible
    ? 'DualSense Edge controller required.'
    : !steamInputLayout
      ? 'Select a Steam game with a loaded Steam Input layout.'
      : !steamPaddlePresetAvailable
        ? 'Steam Input must expose both Edge back paddles before DSCC can write them.'
        : 'Writes Back Left and Back Right as Steam Input keyboard bindings for this game. This is PC-local and does not change onboard Edge memory.';
  const focusedSlotKey = active
    ? resolveFocusedSlotKey({
        hoveredKey: nextState.hoveredSlotKey,
        activeKey: nextState.activeSlotKey,
        bindingBySlotKey: steamBindingBySlotKey,
        selectedBindingKey: nextState.selectedBindingKey
      })
    : '';
  const focusedSlotMeta = focusedSlotKey
    ? steamBindingSlots.find((slot) => slot.key === focusedSlotKey) ?? null
    : null;
  const focusedSlotBinding = focusedSlotMeta ? steamBindingBySlotKey.get(focusedSlotMeta.key) ?? null : null;
  const focusedSlotSelectedBinding =
    focusedSlotBinding && steamBindingKey(focusedSlotBinding) === nextState.selectedBindingKey
      ? focusedSlotBinding
      : null;
  const steamMirrorGroups = active
    ? createSteamMirrorGroups({
        bindingBySlotKey: steamBindingBySlotKey,
        controllerFamily: controller?.family,
        selectedBindingKey: nextState.selectedBindingKey,
        activeSlotKey: nextState.activeSlotKey
      })
    : EMPTY_STEAM_MIRROR_GROUPS;
  const mappedVisibleChipCount = steamMirrorGroups.reduce(
    (count, group) => count + group.rows.filter((row) => row.binding).length,
    0
  );
  const steamLayoutTitle = active
    ? bridgeContextActive
      ? 'DSCC Bridge Xbox 360 Layout'
      : steamInputLayout?.title ?? 'Steam Input Layout'
    : 'Steam Input Layout';

  const setPaddlePresetLeftKey = (value: string) => {
    updateSessionState(store, { paddlePresetLeftKey: normalizePaddlePresetKey(value) });
  };

  const setPaddlePresetRightKey = (value: string) => {
    updateSessionState(store, { paddlePresetRightKey: normalizePaddlePresetKey(value) });
  };

  const applyBindingTargetChange = (nextTargetRaw: string) => {
    const next = parseSteamBindingTriple(nextTargetRaw);
    const current = parseSteamBindingTriple(store.get().bindingDraft);
    updateSessionState(store, {
      bindingDraft: assembleSteamBindingRaw({
        command: next.command,
        param: next.param,
        icon: current.icon,
        label: current.label
      })
    });
  };

  const applyBindingLabelChange = (nextLabel: string) => {
    const current = parseSteamBindingTriple(store.get().bindingDraft);
    updateSessionState(store, {
      bindingLabelDraft: nextLabel,
      bindingDraft: assembleSteamBindingRaw({
        ...current,
        label: nextLabel
      })
    });
  };

  const applyBindingRawChange = (nextRaw: string) => {
    updateSessionState(store, {
      bindingDraft: nextRaw,
      bindingLabelDraft: parseSteamBindingTriple(nextRaw).label
    });
  };

  const resetBindingDraft = () => {
    if (!selectedSteamBinding) return;
    updateSessionState(store, {
      bindingDraft: selectedSteamBinding.rawBinding,
      bindingLabelDraft: parseSteamBindingTriple(selectedSteamBinding.rawBinding).label,
      lastBindingDraftKey: steamBindingKey(selectedSteamBinding),
      bindingMessage: ''
    });
  };

  const selectBinding = (binding: SteamInputBinding | null | undefined) => {
    if (!binding) {
      setBindingMessage(store, input, 'That Steam input is not present in the loaded layout yet.', 'info');
      return;
    }
    const bindingKey = steamBindingKey(binding);
    updateSessionState(store, {
      selectedBindingKey: bindingKey,
      lastBindingDraftKey: bindingKey,
      bindingDraft: binding.rawBinding,
      bindingLabelDraft: parseSteamBindingTriple(binding.rawBinding).label,
      bindingMessage: ''
    });
  };

  const selectSlot = (slot: SteamBindingSlot) => {
    const binding = steamBindingBySlotKey.get(slot.key) ?? null;
    updateSessionState(store, { activeSlotKey: slot.key });
    if (binding) {
      selectBinding(binding);
    } else {
      updateSessionState(store, {
        selectedBindingKey: '',
        lastBindingDraftKey: '',
        bindingDraft: '',
        bindingLabelDraft: ''
      });
      setBindingMessage(store, input, `${slot.label} has no Steam Input binding in this layout yet.`, 'info');
    }
  };

  const hoverSlot = (slot: SteamBindingSlot | null) => {
    updateSessionState(store, { hoveredSlotKey: slot?.key ?? '' });
  };

  const saveBinding = async (dryRun = false) => {
    const bindingToSave = bridgeContextActive
      ? focusedSlotBinding ?? selectedSteamBinding
      : focusedSlotSelectedBinding ?? selectedSteamBinding;
    const currentState = store.get();
    if (bridgeContextActive) {
      if (!bindingToSave) {
        setBindingMessage(store, input, 'Select a bridge input before saving.', 'error');
        return;
      }
      if (!controller?.id) {
        setBindingMessage(store, input, 'Select a controller before saving bridge bindings.', 'error');
        return;
      }
      if (!inputBridgeStatus?.available) {
        setBindingMessage(
          store,
          input,
          inputBridgeStatus?.message ?? 'DSCC Input Bridge backend is unavailable.',
          'error'
        );
        return;
      }
      const rawBinding = currentState.bindingDraft.trim();
      if (!rawBinding) {
        setBindingMessage(store, input, 'Choose a bridge target before saving.', 'error');
        return;
      }
      updateSessionState(store, { bindingBusy: true });
      setBindingMessage(
        store,
        input,
        dryRun ? 'Validating DSCC Bridge binding...' : 'Saving DSCC Bridge binding...',
        'info'
      );
      try {
        const response = await writeInputBridgeBinding({
          controllerId: controller.id,
          profileId: input.bridgeProfileId,
          inputId: bindingToSave.inputId,
          target: rawBinding,
          dryRun
        });
        const warningText = response.warnings.length ? ` ${response.warnings.join(' ')}` : '';
        setBindingMessage(store, input, `${response.message}${warningText}`, response.accepted ? 'success' : 'error');
        if (response.accepted && !dryRun) {
          const parsed = parseSteamBindingTriple(rawBinding);
          const updatedBinding: SteamInputBinding = {
            ...bindingToSave,
            rawBinding,
            binding: parsed.label || steamBindingTargetPart(rawBinding) || rawBinding,
            synthetic: false
          };
          const updatedKey = steamBindingKey(updatedBinding);
          updateSessionState(store, {
            selectedBindingKey: updatedKey,
            lastBindingDraftKey: updatedKey
          });
          applyOptimisticBinding(store, rawSteamInputBindings, updatedBinding);
        }
      } catch (caught) {
        setBindingMessage(
          store,
          input,
          caught instanceof Error ? caught.message : 'Unable to save DSCC Bridge binding.',
          'error'
        );
      } finally {
        updateSessionState(store, { bindingBusy: false });
      }
      return;
    }

    if (!steamInputLayout || !bindingToSave) {
      setBindingMessage(store, input, 'Load a Steam Input layout and select a binding first.', 'error');
      return;
    }
    if (bindingToSave.synthetic) {
      setBindingMessage(
        store,
        input,
        'This input is using DSCC default mapping. Open or create a Steam Input layout for this game before saving a custom binding.',
        'error'
      );
      return;
    }
    const rawBinding = currentState.bindingDraft.trim();
    if (!rawBinding) {
      setBindingMessage(store, input, 'Choose a target binding before saving.', 'error');
      return;
    }
    updateSessionState(store, { bindingBusy: true });
    setBindingMessage(
      store,
      input,
      dryRun ? 'Validating Steam Input write...' : 'Saving Steam Input binding...',
      'info'
    );
    try {
      const response = await writeSteamInputBinding({
        layoutSource: steamInputLayout.source,
        appId: steamInputLayout.appId ?? steamContextGame?.appId ?? null,
        inputId: bindingToSave.inputId,
        groupId: bindingToSave.groupId ?? null,
        activator: bindingToSave.activator ?? null,
        rawBinding,
        profileName: input.activeProfileName || input.profileContextGameName || steamContextGame?.name || null,
        dryRun
      });
      setBindingMessage(
        store,
        input,
        response.backupPath ? `${response.message} Backup: ${response.backupPath}` : response.message,
        'success'
      );
      const selectedKey = steamBindingKey(response.binding);
      updateSessionState(store, {
        selectedBindingKey: selectedKey,
        lastBindingDraftKey: selectedKey,
        bindingDraft: response.binding.rawBinding,
        bindingLabelDraft: parseSteamBindingTriple(response.binding.rawBinding).label
      });
      if (!dryRun) {
        applyOptimisticBinding(store, rawSteamInputBindings, response.binding);
        void input.refresh().finally(() => {
          updateSessionState(store, { optimisticBindings: null });
        });
      }
    } catch (caught) {
      setBindingMessage(
        store,
        input,
        caught instanceof Error ? caught.message : 'Unable to write Steam Input binding.',
        'error'
      );
    } finally {
      updateSessionState(store, { bindingBusy: false });
    }
  };

  const applyPaddlePreset = async (dryRun = false) => {
    const currentState = store.get();
    if (!steamInputLayout) {
      setBindingMessage(store, input, 'Load a Steam Input layout before applying the paddle preset.', 'error');
      return;
    }
    if (controller?.family !== 'DualSense Edge') {
      setBindingMessage(store, input, 'Steam Input paddle presets require a DualSense Edge layout.', 'error');
      return;
    }
    if (!steamPaddlePresetAvailable) {
      setBindingMessage(store, input, steamPaddlePresetStatus, 'error');
      return;
    }
    updateSessionState(store, { bindingBusy: true });
    setBindingMessage(
      store,
      input,
      dryRun ? 'Validating Edge paddle preset...' : 'Saving Edge paddle preset...',
      'info'
    );
    try {
      const response = await writeSteamInputPaddlePreset({
        layoutSource: steamInputLayout.source,
        appId: steamInputLayout.appId ?? steamContextGame?.appId ?? null,
        leftKey: currentState.paddlePresetLeftKey || 'Q',
        rightKey: currentState.paddlePresetRightKey || 'E',
        profileName: input.activeProfileName || input.profileContextGameName || steamContextGame?.name || null,
        dryRun
      });
      const warningText = response.warnings.length ? ` ${response.warnings.join(' ')}` : '';
      setBindingMessage(
        store,
        input,
        response.backupPath
          ? `${response.message} Backup: ${response.backupPath}${warningText}`
          : `${response.message}${warningText}`,
        'success'
      );
      if (!dryRun) {
        for (const paddle of response.paddles) {
          applyOptimisticBinding(store, rawSteamInputBindings, paddle.binding);
        }
        const selectedPaddle = response.paddles[0]?.binding;
        if (selectedPaddle) {
          const selectedKey = steamBindingKey(selectedPaddle);
          updateSessionState(store, {
            selectedBindingKey: selectedKey,
            lastBindingDraftKey: selectedKey,
            bindingDraft: selectedPaddle.rawBinding,
            bindingLabelDraft: parseSteamBindingTriple(selectedPaddle.rawBinding).label
          });
        }
        void input.refresh().finally(() => {
          updateSessionState(store, { optimisticBindings: null });
        });
      }
    } catch (caught) {
      setBindingMessage(
        store,
        input,
        caught instanceof Error ? caught.message : 'Unable to save Edge paddle preset.',
        'error'
      );
    } finally {
      updateSessionState(store, { bindingBusy: false });
    }
  };

  return {
    active,
    steamInputRunning: Boolean(steamInputStatus?.running),
    providerLabel,
    providerKind,
    providerOnline,
    mappingAvailabilityMessage,
    mappingReadOnly: bridgeContextActive ? !inputBridgeStatus?.available : !steamInputLayout,
    defaultMirrorOnly: active && !bridgeContextActive && !steamInputLayout,
    controllerHeaderName: input.controllerHeaderName,
    controllerTransport: controller?.transport,
    gameName: selectedTuningScope === 'global' ? 'Global Profile' : steamContextGame?.name ?? 'No supported game selected',
    steamLayoutTitle,
    mappedVisibleChipCount,
    steamMirrorGroups,
    focusedSlotMeta,
    focusedSlotBinding,
    focusedSlotSelectedBinding,
    steamBindingBusy: nextState.bindingBusy,
    steamInputLayoutAvailable: bridgeContextActive ? Boolean(inputBridgeStatus?.available) : Boolean(steamInputLayout),
    paddlePresetVisible: steamPaddlePresetVisible,
    paddlePresetAvailable: steamPaddlePresetAvailable,
    paddlePresetStatus: steamPaddlePresetStatus,
    paddlePresetLeftKey: nextState.paddlePresetLeftKey,
    paddlePresetRightKey: nextState.paddlePresetRightKey,
    steamBindingDraft: nextState.bindingDraft,
    steamBindingLabelDraft: nextState.bindingLabelDraft,
    bindingLabelFieldLabel: bridgeContextActive ? 'Label' : 'Label (Steam UI)',
    rawFieldLabel: bridgeContextActive ? 'Bridge mapping' : 'Raw VDF',
    rawFieldPlaceholder: bridgeContextActive ? 'xinput_button a, , A' : 'xinput_button ... / key_press ...',
    targetGroups: bindingTargetGroupsForProvider(preparedSteamBindingTargetGroups, providerKind),
    onSelectSlot: selectSlot,
    onHoverSlot: hoverSlot,
    onPaddlePresetLeftKeyChange: setPaddlePresetLeftKey,
    onPaddlePresetRightKeyChange: setPaddlePresetRightKey,
    onApplyPaddlePreset: () => applyPaddlePreset(false),
    onTargetChange: applyBindingTargetChange,
    onLabelChange: applyBindingLabelChange,
    onRawDraftChange: applyBindingRawChange,
    onResetDraft: resetBindingDraft,
    onSaveBinding: () => saveBinding(false)
  };
}
