<script lang="ts">
  import { Cable, CircleHelp, ExternalLink, LifeBuoy, RefreshCw } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import AppSidebar from './components/AppSidebar.svelte';
  import AddGameDialog from './lib/features/games/AddGameDialog.svelte';
  import StatusView from './lib/features/status/StatusView.svelte';
  import TuningHeader from './lib/features/tuning/TuningHeader.svelte';
  import OnboardingTutorial from './components/OnboardingTutorial.svelte';
  import SupportPanel from './components/SupportPanel.svelte';
  import ToastStack from './components/ToastStack.svelte';
  import { guardView, hashForView, isViewHash, viewFromHash } from './app/navigation';
  import { createAppRuntime } from './app/runtime';
  import {
    createButtonMappingSession,
    createButtonMappingSessionState,
    type ButtonMappingSessionStateStore
  } from './app/buttonMappingSession';
  import { EMPTY_BUTTON_MAPPING_VIEW_SESSION } from './lib/features/buttonMapping/buttonMappingState';
  import { ButtonMappingView } from './lib/features/buttonMapping';
  import {
    EDGE_ONBOARD_SLOTS_READ_TOOLTIP,
    edgeProfileWriteRequest,
    edgeSlotInfoTooltip as edgeOnboardSlotInfoTooltip,
    edgeSlotName,
    edgeSlotStatus,
    edgeSlotWriteLabel as edgeOnboardSlotWriteLabel,
    edgeSlotWriteTooltip as edgeOnboardSlotWriteTooltip,
    emptyEdgeOnboardProfileState,
    isEdgeTargetController,
    shouldReadEdgeOnboardProfiles,
    shouldResetEdgeOnboardProfiles
  } from './app/edgeOnboardProfiles';
  import { markOnboardingDismissed, shouldOpenOnboarding } from './app/onboardingState';
  import {
    baseForzaTriggerDefaults,
    buildBuiltInProfileConfig,
    buildControllerConfigDraft,
    buildDefaultControllerConfig as createDefaultControllerConfig,
    editableConfigFromController as createEditableConfigFromController,
    editableConfigFromProfileExport as createEditableConfigFromProfileExport,
    normalizeInputBridgeConfig,
    profileConfigSignature as createProfileConfigSignature,
    type EditableControllerConfig,
    type ProfileDraftValues
  } from './app/profileDraft';
  import {
    createDebouncedAsyncTask,
    createOneShotTimer,
    createQueuedThrottleTask,
    createTriggerInputPoller
  } from './app/runtimePolling';
  import { uniquePartialErrorAreas } from './app/partialErrors';
  import { buildUiSupportBundle, downloadSupportBundleText } from './app/supportBundle';
  import { toastDurationMs, toastToneForMessage, type ToastMessage, type ToastTone } from './app/toastState';
  import {
    UPDATE_RELEASE_PAGE_URL,
    normalizeVersion,
    readDismissedUpdateVersion,
    type UpdateCheckState,
    updateCheckErrorState,
    updateCheckStateFromResult,
    writeDismissedUpdateVersion
  } from './app/updateState';
  import {
    allProfileTargetsForWorkspace,
    clearProfileOverrideForWorkspaceTargets,
    deriveProfileWorkspace,
    deriveTargetControllerWorkspace,
    gameTuningProfileSelection,
    globalTuningProfileSelection,
    inputBridgeBindingProfileIdForWorkspace,
    profileTargetSummaryForWorkspace,
    reconcileSelectedOverrideProfileId,
    reconcileTargetControllerWorkspaceSelection,
    reconcileTuningSelection,
    saveControllerConfigForWorkspaceTargets,
    setProfileOverrideForWorkspaceTargets,
    singleProfileTargetSelection,
    targetControllerSelection,
    type TuningScope
  } from './app/profileWorkspace';
  import ControllersView from './lib/features/controllers/ControllersView.svelte';
  import EdgeSlotsView from './lib/features/controllers/EdgeSlotsView.svelte';
  import GlobalFeelPanel from './lib/features/haptics/GlobalFeelPanel.svelte';
  import LightbarControls from './lib/features/haptics/LightbarControls.svelte';
  import TelemetryRoutingPanel from './lib/features/haptics/TelemetryRoutingPanel.svelte';
  import TriggerCurvesPanel from './lib/features/haptics/TriggerCurvesPanel.svelte';
  import TuningCanvas from './lib/features/tuning/TuningCanvas.svelte';
  import SavedRail from './lib/features/tuning/SavedRail.svelte';
  import SetupGuide from './lib/features/tuning/SetupGuide.svelte';
  import { telemetryPortFromAdapter } from './lib/features/tuning/setupRequirements';
  import {
    loadVerifiedSetupGameIds,
    markSetupVerified,
    type VerifiedSetupGameIds
  } from './app/setupVerification';
  import { savedDiffRows, unsavedChangeCount } from './lib/features/tuning/savedDiff';
  import {
    clampUnit,
    DEFAULT_BODY_FEEL,
    DEFAULT_LIGHTBAR_BRIGHTNESS,
    DEFAULT_LIGHTBAR_COLOR,
    DEFAULT_REDLINE_COLOR,
    defaultTriggerCurve,
    defaultTriggerCurvePoints,
    type ForzaEffectMeta,
    type LightbarColorTarget,
    normalizeStickDeadzone,
    normalizeTriggerCurve,
    normalizeTriggerCurvePoints,
    normalizeTriggerPercent,
    triggerCurvePointOutput,
    triggerCurvePointsFromCurve
  } from './lib/features/haptics/hapticsModel';
  import {
    intensityTooltip,
    routeTooltip,
    triggerCurveTooltip,
    triggerPressLabel,
    triggerRangeTooltip,
    vibrationIntensityPercent,
    vibrationModeRequest,
    type TriggerCurveDisplayMode,
    type TriggerSide
  } from './lib/features/haptics/hapticsCurvePresentation';
  import {
    beginCurveDrag,
    curveGraphPointFromPointer,
    curveHoverFor,
    curveLiveViewFor,
    curvePointFromGraphPoint,
    curveShapeViewFor,
    triggerCurveEditorContext,
    triggerRangeWithEdgeSet,
    withCurvePointAdded,
    withCurvePointAddedOrSelected,
    withCurvePointRemoved,
    withCurvePointSet,
    type CurveDragPoint,
    type CurveHoverState,
    type CurvePointEdit,
    type TriggerCurveEditorContext,
    type TriggerRangeEdge
  } from './app/triggerCurveEditor';
  import {
    bodyRumbleModeOptions,
    forzaEffectMetas,
    forzaRoutes,
    tuningColumnForEffect,
    triggerEffectHelp,
    triggerEffectOptions,
    triggerStrengthHelp,
    vibrationHelp,
    vibrationModeHelp,
    vibrationModeOptions
  } from './lib/features/haptics/hapticsOptions';
  import {
    defaultForzaAbsTuning,
    defaultForzaBrakeTuning,
    defaultForzaEffects,
    defaultForzaRevLimiterTuning,
    defaultForzaShiftTuning,
    defaultForzaThrottleTuning,
    forzaIntensityFromPercent,
    forzaIntensityPercent,
    forzaPresetEffects,
    normalizeEffectId,
    normalizeForzaAbsTuning,
    normalizeForzaBrakeTuning,
    normalizeForzaEffects,
    normalizeForzaRevLimiterTuning,
    normalizeForzaShiftTuning,
    normalizeForzaThrottleTuning
  } from './app/hapticsState';
  import {
    createForzaEffectState,
    defaultForzaTuningValues,
    forzaEffectById,
    forzaTuningFromConfig,
    type ForzaTuningValues
  } from './app/forzaEffectState';
  import {
    defaultProfileIdForGame,
    usesForzaRuntimeProfile
  } from './lib/features/profiles/profileSelection';
  import {
    createProfileManagement,
    type ProfileManagementStateStore
  } from './app/profileManagement';
  import type { AppView } from './app/navigation';
  import {
    addCustomGame,
    addLocalApp,
    connectAppSnapshotSocket,
    exportProfile,
    getAppSnapshot,
    getSupportBundle,
    getAppUpdateCheck,
    getControllerInput,
    getControllerConfig,
    getEdgeProfiles,
    getSteamLibrary,
    removeCustomGame,
    runEffectTest,
    saveAppSettings,
    saveControllerConfig,
    startInputBridgeSession,
    stopInputBridgeSession,
    writeEdgeProfile,
    updateControllerName,
    validateLocalApp
  } from './lib/api';
  import {
    controllerBatteryReadable,
    controllerConnectionText,
    controllerModelText
  } from './lib/controllerDisplay';
  import type {
    AppSnapshot,
    AddLocalAppRequest,
    ControllerConfiguration,
    ControllerInputMode,
    ControllerStatus,
    CurrentEffectState,
    EdgeProfileSlot,
    EdgeProfilesResponse,
    EffectTestRequest,
    ExportedProfile,
    ForzaEffectConfiguration,
    ProfileSummary,
    SteamLibraryEntry,
    SupportBundle,
    SupportedGame,
    TriggerCurvePoint,
    ValidateLocalAppRequest
  } from './lib/types';

  type SupportBundleBusy = 'copy' | 'download' | '';
  const FALLBACK_POLL_INTERVAL_MS = 5000;
  const TRIGGER_INPUT_POLL_INTERVAL_MS = 40;
  const BASE_FEEL_TEST_DURATION_MS = 30000;
  const BASE_FEEL_TEST_REFRESH_INTERVAL_MS = 35;
  const SNAPSHOT_INVALIDATION_DEBOUNCE_MS = 500;
  const LIVE_CONFIG_SYNC_DEBOUNCE_MS = 120;
  let snapshot: AppSnapshot | null = null;
  let loading = true;
  let error = '';
  let selectedControllerId = '';
  let profileTargetControllerIds: string[] = [];
  let controllerRenameId = '';
  let controllerRenameName = '';
  let controllerRenameBusy = false;
  let addGameOpen = false;
  let addGameLoading = false;
  let addGameEntries: SteamLibraryEntry[] = [];
  let addGameError = '';
  let addGameBusyAppId = '';
  let applyMessage = '';
  let appSettingsMessage = '';
  let appSettingsBusy = false;
  let supportPanelOpen = false;
  let supportBundleBusy: SupportBundleBusy = '';
  let supportBundleMessage = '';
  let supportBundleTone: ToastTone = 'info';
  let profileOverrideMessage = '';
  let toastMessages: ToastMessage[] = [];
  let nextToastId = 1;
  let selectedOverrideProfileId = '';
  let selectedTuningScope: TuningScope = 'global';
  let selectedTuningGameId = '';
  let configLoadedFor = '';
  let configLoadError = '';
  let currentControllerConfig: ControllerConfiguration | null = null;
  let edgeProfilesLoadedFor = '';
  let edgeProfiles: EdgeProfilesResponse | null = null;
  let edgeProfilesLoading = false;
  let edgeProfilesBusySlot = '';
  let edgeProfilesError = '';
  let inputBridgeBusy: 'mode' | 'start' | 'stop' | '' = '';
  let profileSaveBaselineSignature = '';
  // The saved baseline retained as a config object so the saved rail can diff
  // the working draft against what the profile actually has on disk.
  let profileSaveBaselineConfig: EditableControllerConfig | null = null;
  let profileConfigDirty = false;
  let effectActivityUntil: Record<string, number> = {};
  let partialErrorsDismissed = false;
  let lastPartialErrorSignature = '';
  let updateCheck: UpdateCheckState = { state: 'idle' };
  let checkedUpdateVersion = '';
  let updateDismissedVersion = '';
  let updateDismissalLoaded = false;
  let onboardingOpen = false;
  let onboardingLoaded = false;
  // Per-game setup state (Task 8). `verifiedSetupGameIds` mirrors the
  // persisted "packets seen at least once" flags; `setupGuideManual` tracks a
  // deliberate re-entry (chip, dropdown, Status deep-link) and resets when the
  // selected game changes. Unverified games pin the guide open on their own.
  let verifiedSetupGameIds: VerifiedSetupGameIds = loadVerifiedSetupGameIds();
  let setupGuideManual = false;
  let setupGuideGameTracker = '';
  let renameProfileId = '';
  let renameProfileName = '';
  let profileRenameBusy = false;
  let profileSaveBusy = false;
  let saveAsProfileOpen = false;
  let saveAsProfileName = '';
  let profileSaveAsBusy = false;
  let profileFileBusy = false;
  let appRuntime: ReturnType<typeof createAppRuntime> | undefined;
  let baseFeelTestActive = false;
  let baseFeelTestBusy = false;
  let l2ControllerPress = 0;
  let r2ControllerPress = 0;
  let controllerInputFresh = false;
  let buttonMappingSessionState = createButtonMappingSessionState();
  let buttonMappingSession = EMPTY_BUTTON_MAPPING_VIEW_SESSION;
  const buttonMappingSessionStore: ButtonMappingSessionStateStore = {
    get: () => buttonMappingSessionState,
    set: (next) => {
      buttonMappingSessionState = next;
    },
    update: (updater) => {
      buttonMappingSessionState = updater(buttonMappingSessionState);
    }
  };

  let l2From = 6;
  let l2To = 100;
  let r2From = 0;
  let r2To = 100;
  let l2Curve = defaultTriggerCurve('l2');
  let r2Curve = defaultTriggerCurve('r2');
  let l2CurvePoints: TriggerCurvePoint[] = defaultTriggerCurvePoints('l2');
  let r2CurvePoints: TriggerCurvePoint[] = defaultTriggerCurvePoints('r2');
  let curveHover: CurveHoverState | null = null;
  let curveDragSide: TriggerSide | null = null;
  let curveDragPoint: CurveDragPoint | null = null;
  let triggerCurveDisplayMode: TriggerCurveDisplayMode = 'base';
  let activeView: AppView = 'status';
  let triggerEffect = 'Adaptive resistance';
  let triggerIntensity = 'Strong (Standard)';
  let vibrationIntensity = 'Medium';
  let vibrationMode: string = DEFAULT_BODY_FEEL;
  let forzaTuning: ForzaTuningValues = defaultForzaTuningValues();
  $: forzaBodyRumbleMode = forzaTuning.bodyRumbleMode;
  $: forzaEffects = forzaTuning.effects;
  $: forzaAbsTuning = forzaTuning.abs;
  $: forzaBrakeTuning = forzaTuning.brake;
  $: forzaThrottleTuning = forzaTuning.throttle;
  $: forzaShiftTuning = forzaTuning.shift;
  $: forzaRevLimiterTuning = forzaTuning.revLimiter;
  let lightbarEnabled = true;
  let lightbarColor: string = DEFAULT_LIGHTBAR_COLOR;
  let rpmColor: string = DEFAULT_REDLINE_COLOR;
  let lightbarBrightness: number = DEFAULT_LIGHTBAR_BRIGHTNESS;
  let leftStickDeadzone = 0;
  let rightStickDeadzone = 0;

  $: enabledForzaEffectCount = forzaEffects.filter((effect) => effect.enabled).length;
  $: allForzaEffectsEnabled = enabledForzaEffectCount === forzaEffectMetas.length;
  // Reactive lookup map so {@const tuning = ...} inside {#each} re-evaluates
  // when forzaEffects is reassigned (Svelte can't statically trace the
  // dependency through a plain function call to forzaEffect()).
  $: forzaEffectsById = new Map(forzaEffects.map((effect) => [effect.id, effect]));
  // Semantic tuning columns: effects grouped by what is being tuned (never by
  // control type); anything unmapped lands in Road feel.
  const brakeEffectMetas = forzaEffectMetas.filter((meta) => tuningColumnForEffect(meta) === 'brake');
  const throttleEffectMetas = forzaEffectMetas.filter((meta) => tuningColumnForEffect(meta) === 'throttle');
  const roadEffectMetas = forzaEffectMetas.filter((meta) => tuningColumnForEffect(meta) === 'road');
  const lightEffectMetas = forzaEffectMetas.filter((meta) => tuningColumnForEffect(meta) === 'lights');

  $: {
    const nextSelection = reconcileTargetControllerWorkspaceSelection({
      controllers: snapshot?.controllers,
      selectedControllerId,
      profileTargetControllerIds
    });
    if (nextSelection.selectedControllerId !== selectedControllerId) {
      selectedControllerId = nextSelection.selectedControllerId;
    }
    const nextTargets = nextSelection.profileTargetControllerIds;
    if (
      nextTargets.length !== profileTargetControllerIds.length ||
      nextTargets.some((id, index) => id !== profileTargetControllerIds[index])
    ) {
      profileTargetControllerIds = nextTargets;
    }
  }
  $: targetControllerWorkspace = deriveTargetControllerWorkspace({
    controllers: snapshot?.controllers,
    selectedControllerId,
    profileTargetControllerIds
  });
  $: controllers = targetControllerWorkspace.controllers;
  $: connectedControllers = targetControllerWorkspace.connectedControllers;
  $: connectedControllerIds = targetControllerWorkspace.connectedControllerIds;
  $: controller = targetControllerWorkspace.controller;
  $: profileTargetsAllConnected = targetControllerWorkspace.profileTargetsAllConnected;

  const triggerInputPoller = createTriggerInputPoller({
    intervalMs: TRIGGER_INPUT_POLL_INTERVAL_MS,
    getControllerId: () => controller?.id,
    shouldPoll: () => shouldPollTriggerInput(),
    getControllerInput,
    onState: (state) => {
      l2ControllerPress = state.l2;
      r2ControllerPress = state.r2;
      controllerInputFresh = state.fresh;
    }
  });
  const baseFeelTestDurationTimer = createOneShotTimer(BASE_FEEL_TEST_DURATION_MS, () => {
    markBaseFeelTestInactive();
  });
  const baseFeelTestRefreshTask = createQueuedThrottleTask({
    minIntervalMs: BASE_FEEL_TEST_REFRESH_INTERVAL_MS,
    shouldRun: () => baseFeelTestActive,
    run: () => startBaseFeelTest(true)
  });
  const liveConfigSync = createDebouncedAsyncTask({
    delayMs: LIVE_CONFIG_SYNC_DEBOUNCE_MS,
    run: () => syncLiveControllerConfig()
  });
  $: status = snapshot?.status;
  $: profiles = snapshot?.profiles ?? [];
  $: activeProfileId = profiles.find((profile) => profile.active)?.id ?? snapshot?.profileResolution.selectedProfileId ?? '';
  $: supportedGamesForSelection = snapshot?.gameDetection.supportedGames ?? [];
  $: {
    const nextSelection = reconcileTuningSelection({
      selectedTuningScope,
      selectedTuningGameId,
      supportedGames: supportedGamesForSelection
    });
    if (nextSelection.selectedTuningScope !== selectedTuningScope) {
      selectedTuningScope = nextSelection.selectedTuningScope;
    }
    if (nextSelection.selectedTuningGameId !== selectedTuningGameId) {
      selectedTuningGameId = nextSelection.selectedTuningGameId;
    }
  }
  $: selectedTuningGameForReconcile = selectedTuningGameId
    ? supportedGamesForSelection.find((game) => game.gameId === selectedTuningGameId) ?? null
    : null;
  $: profileContextDefaultProfileIdForReconcile = defaultProfileIdForGame(
    selectedTuningScope === 'game' ? selectedTuningGameForReconcile : null,
    profiles,
    activeProfileId,
    currentControllerConfig
  );
  $: profileTargetSummaryText = profileTargetSummaryForWorkspace(targetControllerWorkspace);
  $: profileWorkspace = deriveProfileWorkspace({
    snapshot,
    selectedTuningScope,
    selectedTuningGameId,
    selectedOverrideProfileId,
    currentControllerConfig,
    profileConfigDirty,
    controllerSelected: Boolean(controller),
    profileTargetSummary: profileTargetSummaryText
  });
  $: globalProfilePreview = profileWorkspace.globalProfilePreview;
  $: logs = snapshot?.logs ?? [];
  $: diagnostics = snapshot?.diagnostics ?? [];
  $: telemetry = snapshot?.telemetry ?? [];
  $: telemetryByName = new Map(telemetry.map((item) => [item.name, item]));
  $: effectState = snapshot?.effectState;
  $: l2LivePress = controllerInputFresh ? l2ControllerPress : selectedTuningScope === 'global' ? 0 : telemetryUnitValue('input.brake');
  $: r2LivePress = controllerInputFresh ? r2ControllerPress : selectedTuningScope === 'global' ? 0 : telemetryUnitValue('input.throttle');
  $: triggerCurveDisplayMode = selectedTuningScope === 'game' && usesForzaRuntimeProfile(selectedTuningGame) ? 'forza' : 'base';
  $: appSettings = snapshot?.appSettings;
  $: forzaGlyphs = appSettings?.settings.forzaPlaystationGlyphs;
  $: listenOnAllInterfaces = appSettings?.settings.listenOnAllInterfaces ?? false;
  $: lanRestartRequired = appSettings?.restartRequired ?? false;
  $: glyphOverrideEnabled = forzaGlyphs?.enabled ?? false;
  $: glyphInstallPath =
    forzaGlyphs?.installPath ?? 'C:\\Program Files (x86)\\Steam\\steamapps\\common\\ForzaHorizon6';
  $: adapter =
    snapshot?.adapters.find((item) => item.id === snapshot?.profileResolution.activeAdapterId || item.name === status?.activeAdapter) ??
    snapshot?.adapters[0];
  $: displayedParityEffects = (effectState?.parityEffects ?? []).map((effect) => {
    const id = normalizeEffectId(effect.id);
    return effect.state !== 'disabled' && (effect.state === 'active' || (effectActivityUntil[id] ?? 0) > Date.now())
      ? { ...effect, state: 'active' }
      : effect;
  });
  $: effectStatusById = new Map(displayedParityEffects.map((effect) => [normalizeEffectId(effect.id), effect]));
  $: activeProfileName = profileWorkspace.activeProfileName;
  $: activeProfile = profileWorkspace.activeProfile;
  $: selectedOverrideProfile = profileWorkspace.selectedOverrideProfile;
  $: selectedActionProfile = profileWorkspace.selectedActionProfile;
  $: canDeleteSelectedProfile = profileWorkspace.canDeleteSelectedProfile;
  $: canRenameSelectedProfile = profileWorkspace.canRenameSelectedProfile;
  $: controllerHeaderName = controllerModelText(controller);
  $: controllerHeaderMeta = controllerConnectionText(controller);
  $: controllerHeaderBatteryReadable = controllerBatteryReadable(controller);
  $: overrideActive = profileWorkspace.overrideActive;
  $: detectedGameLabel = profileWorkspace.detectedGameLabel;
  $: supportedGames = profileWorkspace.supportedGames;
  $: selectedGame = profileWorkspace.selectedGame;
  $: discoveredGames = profileWorkspace.discoveredGames;
  $: selectedTuningGame = profileWorkspace.selectedTuningGame;
  $: tuningReady = profileWorkspace.tuningReady;
  $: buttonMappingReady = profileWorkspace.buttonMappingReady;
  $: edgeSlotsReady = isEdgeTargetController(controller);
  $: {
    const guardedView = guardView(activeView, { tuningReady, buttonMappingReady, edgeSlotsReady });
    if (guardedView !== activeView) {
      activeView = guardedView;
      setViewHash(guardedView);
    }
  }
  $: profileContextGame = profileWorkspace.profileContextGame;
  $: profileContextGameId = profileWorkspace.profileContextGameId;
  $: profileContextLabel = profileWorkspace.profileContextLabel;
  $: profileContextDefaultProfileId = profileWorkspace.profileContextDefaultProfileId;
  $: profileContextDefaultProfile = profileWorkspace.profileContextDefaultProfile;
  $: profileContextProfiles = profileWorkspace.profileContextProfiles;
  $: activeProfileContextLabel = profileWorkspace.activeProfileContextLabel;
  $: profileContextDetail = profileWorkspace.profileContextDetail;
  $: detectionSignalText = profileWorkspace.detectionSignalText;
  $: steamContextGame = profileWorkspace.steamContextGame;
  $: steamContextArt = profileWorkspace.steamContextArt;
  $: steamContextBackdropArt = profileWorkspace.steamContextBackdropArt;
  $: steamContextMeta = profileWorkspace.steamContextMeta;
  $: activeProfileHeader = profileWorkspace.activeProfileHeader;
  $: activeProfileHeaderName = profileWorkspace.activeProfileHeaderName;
  $: activeProfileHeaderMeta = profileWorkspace.activeProfileHeaderMeta;
  $: buttonMappingActive = activeView === 'advancedButtonMapping';
  // View composition: each route renders its own view; views the guard rejects
  // land on 'status' (Edge onboard slots land on Controller details instead).
  $: showAdvancedControllerView = activeView === 'advancedController';
  $: showEdgeSlotsView = activeView === 'advancedEdgeSlots';
  $: showStatusView = activeView === 'status';
  $: showTuningView = activeView === 'tuning';
  $: showWorkspaceViews =
    showAdvancedControllerView ||
    showEdgeSlotsView ||
    (tuningReady && (activeView === 'tuning' || activeView === 'advancedButtonMapping'));
  $: steamInputStatus = snapshot?.steamInput;
  $: inputBridgeStatus = snapshot?.inputBridge;
  $: telemetryPacketRate = adapter?.packetRateHz ?? 0;
  // --- per-game setup guide state (Task 8) --------------------------------
  $: telemetryPort = telemetryPortFromAdapter(adapter);
  $: selectedGameSetupVerified =
    selectedTuningScope !== 'game' || !selectedTuningGame
      ? true
      : Boolean(verifiedSetupGameIds[selectedTuningGame.gameId]);
  // Fresh Telemetry attributed to the selected game: it must be the game the
  // agent actually detected, with its Telemetry Adapter running and packets
  // arriving. Telemetry loss never yanks the canvas — it only flips the chip
  // to Quiet and surfaces a Status finding.
  $: selectedGameTelemetryFresh = Boolean(
    selectedTuningScope === 'game' &&
      selectedTuningGame?.running &&
      selectedTuningGame.supportLevel === 'telemetry' &&
      snapshot?.gameDetection.activeGameId === selectedTuningGame.gameId &&
      adapter?.state === 'running' &&
      telemetryPacketRate > 0
  );
  $: if (selectedTuningGameId !== setupGuideGameTracker) {
    setupGuideGameTracker = selectedTuningGameId;
    setupGuideManual = false;
  }
  $: showSetupGuide =
    showTuningView &&
    tuningReady &&
    selectedTuningScope === 'game' &&
    Boolean(selectedTuningGame) &&
    (setupGuideManual || !selectedGameSetupVerified);

  const markSelectedGameSetupVerified = () => {
    const gameId = selectedTuningScope === 'game' ? (selectedTuningGame?.gameId ?? '') : '';
    if (!gameId) return;
    verifiedSetupGameIds = markSetupVerified(verifiedSetupGameIds, gameId);
  };

  // Passive completion: SetupGuide calls this when the first packets arrive
  // (or via "Start tuning" on the zero-setup variant). The canvas swaps in
  // because the unverified pin disappears — no click required.
  const completeSetupGuide = () => {
    markSelectedGameSetupVerified();
    setupGuideManual = false;
  };

  const toggleSetupGuide = () => {
    if (!selectedGameSetupVerified) return; // pinned open until verified
    setupGuideManual = !setupGuideManual;
  };

  const openSetupGuide = () => {
    if (selectedTuningScope !== 'game') return;
    setupGuideManual = true;
  };

  // Status → Needs attention deep-link: land on #/tuning with the guide open
  // for the detected game.
  const openSetupGuideFromStatus = async () => {
    const game = selectedGame ?? selectedTuningGame ?? null;
    if (!game) {
      navigateToView('tuning');
      return;
    }
    await selectTuningGame(game);
    setupGuideGameTracker = game.gameId;
    setupGuideManual = true;
  };
  $: telemetryRateText = `${telemetryPacketRate >= 100 ? telemetryPacketRate.toFixed(0) : telemetryPacketRate.toFixed(1)} Hz`;
  $: telemetryRateDetail = telemetryRateStatusText(adapter);
  $: systemReadoutTitle = selectedTuningScope === 'global' ? 'Profile Scope' : 'Telemetry Rate';
  $: systemReadoutValue = selectedTuningScope === 'global' ? 'Global' : telemetryRateText;
  $: systemReadoutDetail =
    selectedTuningScope === 'global'
      ? 'Controller-only tuning'
      : telemetryRateDetail;
  $: overrideScope = profileWorkspace.overrideScope;
  // Sync the override dropdown when the ACTIVE profile changes (server-side
  // activation, override flip, snapshot refresh) — but never fight the user
  // who is manually picking from the dropdown. The tracker remembers the last
  // active profile we mirrored, so the reactive block only fires on a real
  // change.
  let lastSyncedActiveProfileId = '';
  $: if (!profileSaveBusy && selectedTuningScope === 'none' && activeProfileId && activeProfileId !== lastSyncedActiveProfileId) {
    selectedOverrideProfileId = activeProfileId;
    lastSyncedActiveProfileId = activeProfileId;
  }
  $: {
    const nextOverrideProfileId = reconcileSelectedOverrideProfileId({
      profiles,
      selectedOverrideProfileId,
      profileContextDefaultProfileId: profileContextDefaultProfileIdForReconcile,
      activeProfileId,
      profileResolution: snapshot?.profileResolution
    });
    if (nextOverrideProfileId !== selectedOverrideProfileId) {
      selectedOverrideProfileId = nextOverrideProfileId;
    }
  }

  const trackEffectActivity = (effect: CurrentEffectState) => {
    const now = Date.now();
    const nextActivity = { ...effectActivityUntil };
    for (const item of effect.parityEffects) {
      const id = normalizeEffectId(item.id);
      if (item.state === 'disabled') {
        delete nextActivity[id];
      } else if (item.state === 'active') {
        nextActivity[id] = now + 550;
      } else if ((nextActivity[id] ?? 0) <= now) {
        delete nextActivity[id];
      }
    }
    effectActivityUntil = nextActivity;
  };

  const applySnapshot = (next: AppSnapshot) => {
    trackEffectActivity(next.effectState);
    const signature = (next.partialErrors ?? []).map((entry) => entry.endpoint).sort().join('|');
    if (signature !== lastPartialErrorSignature) {
      partialErrorsDismissed = false;
      lastPartialErrorSignature = signature;
    }
    snapshot = next;
    error = '';
    loading = false;
  };

  const refresh = async () => {
    try {
      applySnapshot(await getAppSnapshot());
      error = '';
    } catch (caught) {
      error = caught instanceof Error ? caught.message : 'Unable to load live command center state.';
    } finally {
      loading = false;
    }
  };

  $: partialErrors = snapshot?.partialErrors ?? [];
  $: showPartialErrorBanner = partialErrors.length > 0 && !partialErrorsDismissed;
  $: partialErrorAreas = uniquePartialErrorAreas(partialErrors.map((entry) => entry.endpoint));
  $: showUpdateBanner =
    updateCheck.state === 'available' &&
    Boolean(updateCheck.latestVersion) &&
    updateCheck.latestVersion !== updateDismissedVersion;
  $: if (status?.version) {
    void checkForAppUpdate(status.version);
  }

  const dismissPartialErrors = () => {
    partialErrorsDismissed = true;
  };

  const loadDismissedUpdateVersion = () => {
    if (typeof window === 'undefined' || updateDismissalLoaded) return;
    updateDismissalLoaded = true;
    updateDismissedVersion = readDismissedUpdateVersion();
  };

  const dismissUpdateBanner = () => {
    const version = updateCheck.latestVersion ?? '';
    updateDismissedVersion = version;
    writeDismissedUpdateVersion(version);
  };

  const loadOnboardingPreference = () => {
    if (typeof window === 'undefined' || onboardingLoaded) return;
    onboardingLoaded = true;
    onboardingOpen = shouldOpenOnboarding();
  };

  const openOnboarding = () => {
    onboardingOpen = true;
  };

  const dismissOnboarding = () => {
    onboardingOpen = false;
    markOnboardingDismissed();
  };

  const checkForAppUpdate = async (currentVersionRaw: string) => {
    if (typeof window === 'undefined' || typeof fetch !== 'function') return;
    const currentVersion = normalizeVersion(currentVersionRaw);
    if (!currentVersion || currentVersion.toLowerCase() === 'unknown' || checkedUpdateVersion === currentVersion) return;

    checkedUpdateVersion = currentVersion;
    updateCheck = { state: 'checking', currentVersion };
    try {
      const result = await getAppUpdateCheck(currentVersion);
      updateCheck = updateCheckStateFromResult(result);
    } catch (caught) {
      updateCheck = updateCheckErrorState(currentVersion, caught);
      console.warn('DSCC update check failed', caught);
    }
  };

  const appViewFromHash = (): AppView => {
    if (typeof window === 'undefined') return 'status';
    return viewFromHash(window.location.hash, { tuningReady, buttonMappingReady, edgeSlotsReady });
  };

  const setViewHash = (view: AppView) => {
    if (typeof window === 'undefined') return;
    const nextHash = hashForView(view);
    if (window.location.hash !== nextHash) window.location.hash = nextHash;
  };

  const syncViewFromHash = () => {
    const view = appViewFromHash();
    activeView = view;
    setViewHash(view);
  };

  const navigateToView = (view: AppView) => {
    view = guardView(view, { tuningReady, buttonMappingReady, edgeSlotsReady });
    activeView = view;
    setViewHash(view);
  };

  const dismissToast = (id: number) => {
    toastMessages = toastMessages.filter((toast) => toast.id !== id);
  };

  const showToast = (message: string, tone: ToastTone = toastToneForMessage(message)) => {
    const text = message.trim();
    if (!text) return;
    const id = nextToastId++;
    toastMessages = [
      ...toastMessages.filter((toast) => toast.message !== text),
      { id, tone, message: text }
    ].slice(-4);
    window.setTimeout(() => dismissToast(id), toastDurationMs(tone));
  };

  const inputBridgeBindingProfileId = () => inputBridgeBindingProfileIdForWorkspace(profileWorkspace);

  $: buttonMappingSession = createButtonMappingSession({
    state: buttonMappingSessionState,
    store: buttonMappingSessionStore,
    active: buttonMappingActive,
    controller,
    controllerHeaderName,
    selectedTuningScope,
    steamContextGame,
    steamInputStatus,
    inputBridgeStatus,
    activeProfileName,
    profileContextGameName: profileContextGame?.name ?? null,
    bridgeProfileId: inputBridgeBindingProfileId(),
    refresh,
    notify: showToast
  });

  const setTriggerRangeValue = (side: TriggerSide, edge: TriggerRangeEdge, rawValue: number | string) => {
    if (side === 'l2') {
      ({ from: l2From, to: l2To } = triggerRangeWithEdgeSet({ from: l2From, to: l2To }, edge, rawValue));
    } else {
      ({ from: r2From, to: r2To } = triggerRangeWithEdgeSet({ from: r2From, to: r2To }, edge, rawValue));
    }
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  const setTriggerCurveValue = (side: TriggerSide, rawValue: number | string) => {
    const value = normalizeTriggerCurve(rawValue, defaultTriggerCurve(side));
    if (side === 'l2') {
      l2Curve = value;
      l2CurvePoints = triggerCurvePointsFromCurve(value);
    } else {
      r2Curve = value;
      r2CurvePoints = triggerCurvePointsFromCurve(value);
    }
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };
  const profileTargetSummary = () => profileTargetSummaryText;

  const setAllProfileTargets = () => {
    if (!connectedControllerIds.length) return;
    profileTargetControllerIds = allProfileTargetsForWorkspace(targetControllerWorkspace);
  };

  const setSingleProfileTargetController = (controllerId: string) => {
    const selection = singleProfileTargetSelection(targetControllerWorkspace, controllerId);
    if (!selection) return;
    profileTargetControllerIds = selection.profileTargetControllerIds;
    selectedControllerId = selection.selectedControllerId;
    configLoadedFor = '';
    stopTriggerInputPolling();
  };

  const setProfileOverrideForTargets = (profileId: string, gameId: string | null) =>
    setProfileOverrideForWorkspaceTargets({
      workspace: targetControllerWorkspace,
      profileId,
      gameId
    });

  const clearProfileOverrideForTargets = (gameId: string | null) =>
    clearProfileOverrideForWorkspaceTargets({
      workspace: targetControllerWorkspace,
      gameId
    });

  const saveControllerConfigForProfileTargets = async (config: EditableControllerConfig) => {
    const selectedUpdate = await saveControllerConfigForWorkspaceTargets({
      workspace: targetControllerWorkspace,
      config
    });
    if (selectedUpdate) currentControllerConfig = selectedUpdate;
  };

  const selectTargetController = (controllerId: string) => {
    const selection = targetControllerSelection(targetControllerWorkspace, controllerId);
    if (!selection) return;
    selectedControllerId = selection.selectedControllerId;
    profileTargetControllerIds = selection.profileTargetControllerIds;
    configLoadedFor = '';
    stopTriggerInputPolling();
  };

  const openAddGameDialog = async () => {
    addGameOpen = true;
    addGameError = '';
    addGameLoading = true;
    try {
      const response = await getSteamLibrary();
      addGameEntries = response.games;
    } catch (caught) {
      addGameError = caught instanceof Error ? caught.message : 'Unable to load Steam library.';
      addGameEntries = [];
    } finally {
      addGameLoading = false;
    }
  };

  const closeAddGameDialog = () => {
    if (addGameBusyAppId) return;
    addGameOpen = false;
    addGameError = '';
  };

  const addGameFromLibrary = async (entry: SteamLibraryEntry, processNames?: string[]) => {
    if (addGameBusyAppId) return;
    addGameBusyAppId = entry.appId;
    addGameError = '';
    try {
      const response = await addCustomGame(entry.appId, processNames ?? []);
      await refresh();
      addGameEntries = addGameEntries.map((item) =>
        item.appId === entry.appId ? { ...item, alreadyInCatalog: true } : item
      );
      setApplyMessage(`Added ${response.game.name}. Tune a profile, and DSCC will auto-load it when the game launches.`);
    } catch (caught) {
      addGameError = caught instanceof Error ? caught.message : 'Unable to add game.';
    } finally {
      addGameBusyAppId = '';
    }
  };

  const validateLocalGameFromDialog = async (request: ValidateLocalAppRequest) =>
    validateLocalApp(request);

  const addLocalGameFromDialog = async (request: AddLocalAppRequest) => {
    const response = await addLocalApp(request);
    await refresh();
    setApplyMessage(`Added ${response.game.name}. DSCC Bridge mapping is available for this local app.`);
  };

  const pickAllControllers = () => {
    setAllProfileTargets();
  };

  const pickControllerTarget = (controllerId: string) => {
    setSingleProfileTargetController(controllerId);
  };

  const beginControllerRename = (item: ControllerStatus) => {
    controllerRenameId = item.id;
    controllerRenameName = item.name || controllerModelText(item);
  };

  const cancelControllerRename = () => {
    controllerRenameId = '';
    controllerRenameName = '';
  };

  const submitControllerRename = async () => {
    const id = controllerRenameId;
    const name = controllerRenameName.trim();
    if (!id || !name || controllerRenameBusy) return;
    controllerRenameBusy = true;
    try {
      const updated = await updateControllerName(id, name);
      if (snapshot) {
        snapshot = {
          ...snapshot,
          controllers: snapshot.controllers.map((item) => (item.id === updated.id ? { ...item, name: updated.name } : item))
        };
      }
      cancelControllerRename();
      await refresh();
      showToast(`Renamed controller to ${updated.name}`, 'success');
    } catch (caught) {
      showToast(caught instanceof Error ? caught.message : 'Unable to rename controller.', 'error');
    } finally {
      controllerRenameBusy = false;
    }
  };

  const handleControllerRenameKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submitControllerRename();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      cancelControllerRename();
    }
  };

  const saveControllerInputMode = async (mode: ControllerInputMode) => {
    if (!controller || !currentControllerConfig || currentControllerConfig.controllerId !== controller.id || inputBridgeBusy) return;
    inputBridgeBusy = 'mode';
    try {
      const config = editableConfigFromController(currentControllerConfig);
      config.inputMode = mode;
      config.inputBridge = {
        ...normalizeInputBridgeConfig(config.inputBridge),
        enabled: mode === 'dscc_input_bridge',
        outputKind: 'xbox360'
      };
      currentControllerConfig = await saveControllerConfig(controller.id, config);
      await refresh();
      showToast(
        mode === 'dscc_input_bridge'
          ? 'DSCC Input Bridge enabled for this controller'
          : 'Controller input path updated',
        'success'
      );
    } catch (caught) {
      showToast(caught instanceof Error ? caught.message : 'Unable to update controller input path.', 'error');
    } finally {
      inputBridgeBusy = '';
    }
  };

  const startControllerInputBridge = async () => {
    if (!controller || inputBridgeBusy) return;
    inputBridgeBusy = 'start';
    try {
      await startInputBridgeSession(controller.id);
      await refresh();
      showToast('DSCC Input Bridge session started', 'success');
    } catch (caught) {
      showToast(caught instanceof Error ? caught.message : 'Unable to start DSCC Input Bridge.', 'error');
    } finally {
      inputBridgeBusy = '';
    }
  };

  const stopControllerInputBridge = async () => {
    if (!controller || inputBridgeBusy) return;
    inputBridgeBusy = 'stop';
    try {
      await stopInputBridgeSession(controller.id);
      await refresh();
      showToast('DSCC Input Bridge session stopped', 'success');
    } catch (caught) {
      showToast(caught instanceof Error ? caught.message : 'Unable to stop DSCC Input Bridge.', 'error');
    } finally {
      inputBridgeBusy = '';
    }
  };

  const selectGlobalTuning = async () => {
    selectedTuningScope = 'global';
    selectedTuningGameId = '';
    const profileId = globalTuningProfileSelection(profiles, activeProfileId);
    selectedOverrideProfileId = profileId;
    activeView = 'tuning';
    setViewHash('tuning');
    if (profileId) await selectProfileForScope(profileId, null, 'Global Profile');
  };

  const selectTuningGame = async (game: SupportedGame) => {
    selectedTuningScope = 'game';
    selectedTuningGameId = game.gameId;
    const preferredProfileId = gameTuningProfileSelection({
      game,
      profiles,
      activeProfileId,
      currentControllerConfig
    });
    if (preferredProfileId) selectedOverrideProfileId = preferredProfileId;
    activeView = 'tuning';
    setViewHash('tuning');
    if (preferredProfileId) await selectProfileForScope(preferredProfileId, game.gameId, game.name);
  };

  const selectProfileForScope = async (
    profileId: string,
    gameId: string | null = profileContextGameId,
    scopeLabel: string = profileContextLabel
  ) => {
    const profile = profiles.find((item) => item.id === profileId);
    if (!snapshot || !profile) return;
    selectedOverrideProfileId = profileId;
    try {
      const resolution = await setProfileOverrideForTargets(profileId, gameId);
      if (resolution) snapshot = { ...snapshot, profileResolution: resolution };
      await loadProfileConfigForEditor(profile);
      await refresh();
      setProfileOverrideMessage(`${profile.name} selected for ${scopeLabel} on ${profileTargetSummary()}`, 'success');
    } catch (caught) {
      setProfileOverrideMessage(caught instanceof Error ? caught.message : 'Unable to select profile.', 'error');
      await refresh();
    }
  };

  const isEdgeController = () => isEdgeTargetController(controller);

  const editableConfigFromController = (config: ControllerConfiguration): EditableControllerConfig =>
    createEditableConfigFromController(config, isEdgeController());

  const buildDefaultControllerConfig = (): EditableControllerConfig =>
    createDefaultControllerConfig({
      isEdge: isEdgeController(),
      defaultForzaEffects: defaultForzaEffects(),
      defaultForzaBrakeTuning: defaultForzaBrakeTuning(),
      defaultForzaAbsTuning: defaultForzaAbsTuning(),
      defaultForzaThrottleTuning: defaultForzaThrottleTuning(),
      defaultForzaShiftTuning: defaultForzaShiftTuning(),
      defaultForzaRevLimiterTuning: defaultForzaRevLimiterTuning()
    });

  const profileConfigSignature = (config: EditableControllerConfig | ControllerConfiguration): string =>
    createProfileConfigSignature(config, {
      isEdge: isEdgeController(),
      normalizeForzaBrakeTuning,
      normalizeForzaEffects,
      normalizeForzaAbsTuning,
      normalizeForzaThrottleTuning,
      normalizeForzaShiftTuning,
      normalizeForzaRevLimiterTuning,
      forzaIntensityPercent
    });

  $: profileConfigDirty =
    Boolean(currentControllerConfig && profileSaveBaselineSignature) &&
    profileConfigSignature(buildControllerConfig()) !== profileSaveBaselineSignature;


  // Saved rail diff (Task 7). The object literal mirrors
  // currentProfileDraftValues() but names every draft variable directly so
  // Svelte re-derives the snapshot the moment any tunable value moves (a
  // plain function call would hide the dependencies from the compiler).
  $: profileDraftSnapshot = {
    l2From,
    l2To,
    r2From,
    r2To,
    l2Curve,
    r2Curve,
    l2CurvePoints,
    r2CurvePoints,
    triggerEffect,
    triggerIntensity,
    vibrationIntensity,
    vibrationMode,
    lightbarEnabled,
    lightbarColor,
    rpmColor,
    lightbarBrightness,
    forzaBodyRumbleMode: forzaTuning.bodyRumbleMode,
    forzaEffects: forzaTuning.effects,
    forzaBrakeTuning: forzaTuning.brake,
    forzaAbsTuning: forzaTuning.abs,
    forzaThrottleTuning: forzaTuning.throttle,
    forzaShiftTuning: forzaTuning.shift,
    forzaRevLimiterTuning: forzaTuning.revLimiter,
    leftStickDeadzone,
    rightStickDeadzone
  };
  // The rail diff is display-only (the dirty flag has its own signature path),
  // so it recomputes on a 100ms trailing debounce instead of per input event.
  let savedRailRows: ReturnType<typeof savedDiffRows> = [];
  let savedRailDiffTimer = 0;
  const refreshSavedRailRows = () => {
    savedRailRows = savedDiffRows(profileSaveBaselineConfig, profileDraftSnapshot, {
      includeForza: selectedTuningScope === 'game',
      intensityPercent: forzaIntensityPercent
    });
  };
  $: {
    // Touched (not used) so the legacy compiler re-runs this block when they change.
    void profileDraftSnapshot;
    void profileSaveBaselineConfig;
    void selectedTuningScope;
    if (typeof window === 'undefined') {
      refreshSavedRailRows();
    } else {
      window.clearTimeout(savedRailDiffTimer);
      savedRailDiffTimer = window.setTimeout(refreshSavedRailRows, 100);
    }
  }
  $: unsavedCount = unsavedChangeCount(savedRailRows);
  $: savedRailProfileName =
    profiles.find((profile) => profile.id === (selectedOverrideProfileId || activeProfileId))?.name ??
    'this profile';

  // Discard: put the draft back to the saved baseline. The live controller
  // sync then pushes the restored values to hardware; the baseline itself is
  // untouched, so the dirty flag and rail diff clear together.
  const discardDraftChanges = () => {
    if (!profileSaveBaselineConfig) return;
    applyEditableConfig(profileSaveBaselineConfig);
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
    setApplyMessage('Unsaved changes discarded', 'info');
  };

  // Saved-curve ghosts: dashed echo of the saved curve in each editor while
  // the draft's shape differs from the saved one.
  const savedCurveShapePath = (
    side: TriggerSide,
    saved: EditableControllerConfig | null,
    displayMode: TriggerCurveDisplayMode
  ): string | null => {
    if (!saved) return null;
    return curveShapeViewFor(
      triggerCurveEditorContext({
        side,
        from: side === 'l2' ? saved.trigger.l2From : saved.trigger.r2From,
        to: side === 'l2' ? saved.trigger.l2To : saved.trigger.r2To,
        curve: side === 'l2' ? saved.trigger.l2Curve : saved.trigger.r2Curve,
        points: side === 'l2' ? saved.trigger.l2CurvePoints : saved.trigger.r2CurvePoints,
        triggerEffect: saved.trigger.effect,
        triggerIntensity: saved.trigger.intensity,
        displayMode,
        forzaEffects: saved.forza?.effects ?? [],
        forzaBrakeTuning: normalizeForzaBrakeTuning(saved.forza?.brake),
        forzaThrottleTuning: normalizeForzaThrottleTuning(saved.forza?.throttle)
      })
    ).path;
  };

  $: l2SavedShapePath = savedCurveShapePath('l2', profileSaveBaselineConfig, triggerCurveDisplayMode);
  $: r2SavedShapePath = savedCurveShapePath('r2', profileSaveBaselineConfig, triggerCurveDisplayMode);
  $: l2SavedCurvePath = l2SavedShapePath && l2SavedShapePath !== l2CurveShape.path ? l2SavedShapePath : null;
  $: r2SavedCurvePath = r2SavedShapePath && r2SavedShapePath !== r2CurveShape.path ? r2SavedShapePath : null;

  const forzaEffectState = createForzaEffectState({
    store: {
      get: () => forzaTuning,
      set: (next) => {
        forzaTuning = next;
      }
    },
    onChanged: () => scheduleLiveControllerConfigSync()
  });

  const forzaEffect = (id: string): ForzaEffectConfiguration => forzaEffectById(forzaEffects, id);
  const updateForzaEffect = forzaEffectState.updateEffect;
  const setForzaBodyRumbleMode = forzaEffectState.setBodyRumbleMode;
  const updateForzaAbsTuning = forzaEffectState.updateAbsTuning;
  const updateForzaBrakeTuning = forzaEffectState.updateBrakeTuning;
  const updateForzaThrottleTuning = forzaEffectState.updateThrottleTuning;
  const updateForzaShiftTuning = forzaEffectState.updateShiftTuning;
  const updateForzaRevLimiterTuning = forzaEffectState.updateRevLimiterTuning;

  const toggleAllForzaEffects = () => {
    forzaEffectState.setAllEffectsEnabled(!allForzaEffectsEnabled);
  };

  const telemetryUnitValue = (signal: string) => {
    const value = telemetryByName.get(signal)?.value;
    return typeof value === 'number' && Number.isFinite(value) ? clampUnit(value) : 0;
  };

  $: l2CurveEditorContext = triggerCurveEditorContext({
    side: 'l2',
    from: l2From,
    to: l2To,
    curve: l2Curve,
    points: l2CurvePoints,
    triggerEffect,
    triggerIntensity,
    displayMode: triggerCurveDisplayMode,
    forzaEffects,
    forzaBrakeTuning,
    forzaThrottleTuning
  });
  $: r2CurveEditorContext = triggerCurveEditorContext({
    side: 'r2',
    from: r2From,
    to: r2To,
    curve: r2Curve,
    points: r2CurvePoints,
    triggerEffect,
    triggerIntensity,
    displayMode: triggerCurveDisplayMode,
    forzaEffects,
    forzaBrakeTuning,
    forzaThrottleTuning
  });
  // Handler-side context builder: reads the raw state directly so point edits
  // made earlier in the same event see their own writes (the reactive
  // l2/r2CurveEditorContext objects only refresh on Svelte's next flush).
  const curveEditorContext = (side: TriggerSide): TriggerCurveEditorContext =>
    triggerCurveEditorContext({
      side,
      from: side === 'l2' ? l2From : r2From,
      to: side === 'l2' ? l2To : r2To,
      curve: side === 'l2' ? l2Curve : r2Curve,
      points: side === 'l2' ? l2CurvePoints : r2CurvePoints,
      triggerEffect,
      triggerIntensity,
      displayMode: triggerCurveDisplayMode,
      forzaEffects: forzaTuning.effects,
      forzaBrakeTuning: forzaTuning.brake,
      forzaThrottleTuning: forzaTuning.throttle
    });

  $: l2CurveShape = curveShapeViewFor(l2CurveEditorContext);
  $: r2CurveShape = curveShapeViewFor(r2CurveEditorContext);
  $: l2CurveLive = curveLiveViewFor(l2CurveEditorContext, l2LivePress);
  $: r2CurveLive = curveLiveViewFor(r2CurveEditorContext, r2LivePress);
  $: triggerRangeTooltipForCurrentTuning = (
    side: 'L2' | 'R2',
    edge: 'from' | 'to',
    value: number,
    startValue = 0
  ) => triggerRangeTooltip(side, edge, value, startValue, forzaBrakeTuning, forzaThrottleTuning);

  const showTriggerPress = (_side: 'l2' | 'r2', value: number) =>
    baseFeelTestActive || clampUnit(value) > 0.01;

  const setPointsForSide = (side: TriggerSide, points: TriggerCurvePoint[], alreadyNormalized = false) => {
    const normalized = alreadyNormalized ? points : normalizeTriggerCurvePoints(points, side === 'l2' ? l2Curve : r2Curve);
    if (side === 'l2') {
      l2CurvePoints = normalized;
    } else {
      r2CurvePoints = normalized;
    }
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  // Point edits come from withCurvePointSet/withCurvePointAddedOrSelected,
  // which already start from normalizeTriggerCurvePoints() output — skip the
  // second normalization pass per pointermove.
  const applyCurvePointEdit = (side: TriggerSide, edit: CurvePointEdit) => {
    if (edit.points) setPointsForSide(side, edit.points, true);
    return edit.index;
  };

  const setCurvePoint = (side: TriggerSide, index: number, point: TriggerCurvePoint) =>
    applyCurvePointEdit(side, withCurvePointSet(curveEditorContext(side), index, point));

  const addOrSelectCurvePoint = (side: TriggerSide, point: TriggerCurvePoint) =>
    applyCurvePointEdit(side, withCurvePointAddedOrSelected(curveEditorContext(side), point));

  const addCurvePoint = (side: TriggerSide) => {
    const points = withCurvePointAdded(curveEditorContext(side));
    if (points) setPointsForSide(side, points);
  };

  const removeCurvePoint = (side: TriggerSide) => {
    const points = withCurvePointRemoved(curveEditorContext(side));
    if (points) setPointsForSide(side, points);
  };

  const updateCurveHover = (event: PointerEvent, side: TriggerSide) => {
    const target = event.currentTarget as HTMLElement;
    const { x } = curveGraphPointFromPointer(event, target);
    curveHover = curveHoverFor(curveEditorContext(side), x);
  };

  const handleCurvePointer = (event: PointerEvent, side: TriggerSide) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    event.preventDefault();

    const target = event.currentTarget as HTMLElement;
    curveDragSide = side;
    let pointIndex = -1;

    beginCurveDrag(event, target, {
      applyInitialEvent: true,
      onPoint: ({ x, output }) => {
        const point = curvePointFromGraphPoint(curveEditorContext(side), x, output);
        pointIndex = pointIndex < 0 ? addOrSelectCurvePoint(side, point) : setCurvePoint(side, pointIndex, point);
        curveDragPoint = { side, index: pointIndex };
        curveHover = curveHoverFor(curveEditorContext(side), x);
      },
      onEnd: () => {
        curveDragSide = null;
        curveDragPoint = null;
      }
    });
  };

  const handleCurvePointPointer = (event: PointerEvent, side: TriggerSide, index: number) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const frame = (event.currentTarget as SVGElement).closest('.dm-curve-frame') as HTMLElement | null;
    if (!frame) return;
    curveDragSide = side;
    curveDragPoint = { side, index };

    beginCurveDrag(event, frame, {
      onPoint: ({ x, output }) => {
        setCurvePoint(side, index, curvePointFromGraphPoint(curveEditorContext(side), x, output));
        curveHover = curveHoverFor(curveEditorContext(side), x);
      },
      onEnd: () => {
        curveDragSide = null;
        curveDragPoint = null;
      }
    });
  };

  const clearCurveHover = (side: TriggerSide) => {
    if (curveDragSide === side) return;
    if (curveHover?.side === side) curveHover = null;
  };

  const applyEditableConfig = (config: Omit<ControllerConfiguration, 'controllerId' | 'model'>) => {
    l2From = normalizeTriggerPercent(config.trigger.l2From);
    l2To = Math.max(l2From, normalizeTriggerPercent(config.trigger.l2To));
    r2From = normalizeTriggerPercent(config.trigger.r2From);
    r2To = Math.max(r2From, normalizeTriggerPercent(config.trigger.r2To));
    l2Curve = normalizeTriggerCurve(config.trigger.l2Curve, defaultTriggerCurve('l2'));
    r2Curve = normalizeTriggerCurve(config.trigger.r2Curve, defaultTriggerCurve('r2'));
    l2CurvePoints = normalizeTriggerCurvePoints(config.trigger.l2CurvePoints, l2Curve);
    r2CurvePoints = normalizeTriggerCurvePoints(config.trigger.r2CurvePoints, r2Curve);
    triggerEffect = config.trigger.effect;
    triggerIntensity = config.trigger.intensity;
    vibrationIntensity = config.trigger.vibration;
    vibrationMode = config.trigger.vibrationMode ?? DEFAULT_BODY_FEEL;
    lightbarEnabled = config.lightbar?.enabled ?? true;
    lightbarColor = config.lightbar?.color ?? DEFAULT_LIGHTBAR_COLOR;
    rpmColor = config.lightbar?.rpmColor ?? DEFAULT_REDLINE_COLOR;
    lightbarBrightness = config.lightbar?.brightness ?? DEFAULT_LIGHTBAR_BRIGHTNESS;
    leftStickDeadzone = normalizeStickDeadzone(config.sticks?.leftDeadzone ?? 0);
    rightStickDeadzone = normalizeStickDeadzone(config.sticks?.rightDeadzone ?? 0);
    forzaTuning = forzaTuningFromConfig(config.forza);
  };
  // Capture the saved baseline as both a signature (cheap dirty check) and a
  // config object (saved rail diff + discard target).
  const captureProfileSaveBaseline = () => {
    const config = buildControllerConfig();
    profileSaveBaselineSignature = profileConfigSignature(config);
    profileSaveBaselineConfig = config;
  };

  const applyControllerConfig = (config: ControllerConfiguration, updateProfileBaseline = true) => {
    currentControllerConfig = config;
    applyEditableConfig(config);
    if (updateProfileBaseline) captureProfileSaveBaseline();
  };

  const loadControllerConfig = async (controllerId: string) => {
    configLoadedFor = controllerId;
    configLoadError = '';
    currentControllerConfig = null;
    profileSaveBaselineSignature = '';
    profileSaveBaselineConfig = null;
    try {
      const config = await getControllerConfig(controllerId);
      if (config.controllerId !== controllerId || selectedControllerId !== controllerId) return;
      applyControllerConfig(config);
    } catch (caught) {
      if (selectedControllerId !== controllerId) return;
      configLoadError = caught instanceof Error ? caught.message : 'Unable to load controller configuration.';
      showToast(configLoadError, 'error');
    }
  };

  const loadEdgeProfiles = async (controllerId: string, force = false) => {
    if (
      !shouldReadEdgeOnboardProfiles({
        controller,
        loadedFor: edgeProfilesLoadedFor,
        profiles: edgeProfiles,
        loading: edgeProfilesLoading,
        force
      })
    ) {
      return;
    }
    edgeProfilesLoadedFor = controllerId;
    edgeProfilesLoading = true;
    edgeProfilesError = '';
    try {
      edgeProfiles = await getEdgeProfiles(controllerId);
    } catch (caught) {
      edgeProfiles = null;
      edgeProfilesError = caught instanceof Error ? caught.message : 'Unable to read Edge onboard slots.';
    } finally {
      edgeProfilesLoading = false;
    }
  };

  const resetEdgeProfiles = () => {
    const empty = emptyEdgeOnboardProfileState();
    edgeProfilesLoadedFor = empty.loadedFor;
    edgeProfiles = empty.profiles;
    edgeProfilesLoading = empty.loading;
    edgeProfilesBusySlot = empty.busySlot;
    edgeProfilesError = empty.error;
  };

  const edgeSlotsReadTooltip = EDGE_ONBOARD_SLOTS_READ_TOOLTIP;
  const edgeSlotInfoTooltip = edgeOnboardSlotInfoTooltip;
  const edgeSlotWriteTooltip = (slot: EdgeProfileSlot) =>
    edgeOnboardSlotWriteTooltip(slot, edgeProfiles);
  const edgeSlotWriteLabel = () => edgeOnboardSlotWriteLabel(edgeProfiles);

  const writeCurrentConfigToEdgeSlot = async (slot: EdgeProfileSlot) => {
    if (!controller || !slot.editable || edgeProfilesBusySlot) return;
    edgeProfilesBusySlot = slot.slotId;
    edgeProfilesError = '';
    try {
      const config = buildControllerConfig();
      const response = await writeEdgeProfile(
        controller.id,
        slot.slotId,
        edgeProfileWriteRequest({
          slot,
          profileName: activeProfileHeaderName || activeProfile?.name || 'DSCC Profile',
          config
        })
      );
      showToast(response.message, response.accepted ? 'success' : 'error');
      await loadEdgeProfiles(controller.id, true);
    } catch (caught) {
      edgeProfilesError = caught instanceof Error ? caught.message : 'Unable to write Edge onboard slot.';
      showToast(edgeProfilesError, 'error');
    } finally {
      edgeProfilesBusySlot = '';
    }
  };

  const builtInProfileConfig = (profileId: string): EditableControllerConfig =>
    buildBuiltInProfileConfig({
      profileId,
      isEdge: isEdgeController(),
      defaultForzaEffects: defaultForzaEffects(),
      defaultForzaBrakeTuning: defaultForzaBrakeTuning(),
      defaultForzaAbsTuning: defaultForzaAbsTuning(),
      defaultForzaThrottleTuning: defaultForzaThrottleTuning(),
      defaultForzaShiftTuning: defaultForzaShiftTuning(),
      defaultForzaRevLimiterTuning: defaultForzaRevLimiterTuning(),
      builtInForzaEffects: forzaPresetEffects(profileId === 'forza-horizon-immersive' ? 'immersive' : 'base'),
      profileAssignments: currentControllerConfig?.profileAssignments ?? []
    });

  const editableConfigFromProfileExport = (config: NonNullable<ExportedProfile['config']>): EditableControllerConfig =>
    createEditableConfigFromProfileExport(config, {
      isEdge: isEdgeController(),
      defaultForzaEffects: defaultForzaEffects(),
      defaultForzaBrakeTuning: defaultForzaBrakeTuning(),
      defaultForzaAbsTuning: defaultForzaAbsTuning(),
      defaultForzaThrottleTuning: defaultForzaThrottleTuning(),
      defaultForzaShiftTuning: defaultForzaShiftTuning(),
      defaultForzaRevLimiterTuning: defaultForzaRevLimiterTuning(),
      profileAssignments: currentControllerConfig?.profileAssignments ?? []
    });

  const loadProfileConfigForEditor = async (profile: ProfileSummary) => {
    let config: EditableControllerConfig | null = null;
    if (profile.builtIn) {
      config = builtInProfileConfig(profile.id);
    } else {
      const exported = await exportProfile(profile.id);
      config = exported.config ? editableConfigFromProfileExport(exported.config) : buildControllerConfig();
    }

    applyEditableConfig(config);
    captureProfileSaveBaseline();
  };

  const applyTriggerConfig = (trigger: EditableControllerConfig['trigger']) => {
    l2From = normalizeTriggerPercent(trigger.l2From);
    l2To = Math.max(l2From, normalizeTriggerPercent(trigger.l2To));
    r2From = normalizeTriggerPercent(trigger.r2From);
    r2To = Math.max(r2From, normalizeTriggerPercent(trigger.r2To));
    l2Curve = normalizeTriggerCurve(trigger.l2Curve, defaultTriggerCurve('l2'));
    r2Curve = normalizeTriggerCurve(trigger.r2Curve, defaultTriggerCurve('r2'));
    l2CurvePoints = normalizeTriggerCurvePoints(trigger.l2CurvePoints, l2Curve);
    r2CurvePoints = normalizeTriggerCurvePoints(trigger.r2CurvePoints, r2Curve);
    triggerEffect = trigger.effect;
    triggerIntensity = trigger.intensity;
    vibrationIntensity = trigger.vibration;
    vibrationMode = trigger.vibrationMode ?? DEFAULT_BODY_FEEL;
  };

  const resetTriggerCurvesToProfileDefaults = () => {
    applyTriggerConfig(baseForzaTriggerDefaults());
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
    const profileLabel = activeProfile?.builtIn ? activeProfile.name : 'Base';
    setApplyMessage(`Reset trigger curves to ${profileLabel} defaults`);
  };

  const currentProfileDraftValues = (): ProfileDraftValues => ({
    l2From,
    l2To,
    r2From,
    r2To,
    l2Curve,
    r2Curve,
    l2CurvePoints,
    r2CurvePoints,
    triggerEffect,
    triggerIntensity,
    vibrationIntensity,
    vibrationMode,
    lightbarEnabled,
    lightbarColor,
    rpmColor,
    lightbarBrightness,
    // Read forzaTuning directly, not the `$:` aliases (forzaEffects et al.):
    // captureProfileSaveBaseline() runs synchronously right after
    // applyEditableConfig() reassigns forzaTuning, before Svelte re-runs the
    // reactive aliases — reading the aliases here baselines stale values and
    // marks a freshly loaded profile dirty.
    forzaBodyRumbleMode: forzaTuning.bodyRumbleMode,
    forzaEffects: forzaTuning.effects,
    forzaBrakeTuning: forzaTuning.brake,
    forzaAbsTuning: forzaTuning.abs,
    forzaThrottleTuning: forzaTuning.throttle,
    forzaShiftTuning: forzaTuning.shift,
    forzaRevLimiterTuning: forzaTuning.revLimiter,
    leftStickDeadzone,
    rightStickDeadzone
  });

  const buildControllerConfig = (): EditableControllerConfig => {
    const base = currentControllerConfig
      ? editableConfigFromController(currentControllerConfig)
      : buildDefaultControllerConfig();

    return buildControllerConfigDraft(base, currentProfileDraftValues(), {
      isEdge: isEdgeController(),
      normalizeForzaBrakeTuning,
      normalizeForzaEffects,
      normalizeForzaAbsTuning,
      normalizeForzaThrottleTuning,
      normalizeForzaShiftTuning,
      normalizeForzaRevLimiterTuning
    });
  };

  const saveCurrentConfig = async () => {
    if (!controller || (currentControllerConfig && currentControllerConfig.controllerId !== controller.id)) return false;
    const controllerId = controller.id;
    try {
      const updated = await saveControllerConfig(controllerId, buildControllerConfig());
      if (selectedControllerId === controllerId) currentControllerConfig = updated;
      return true;
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to save config');
      return false;
    }
  };

  const syncLiveControllerConfig = async () => {
    if (!controller || !currentControllerConfig || currentControllerConfig.controllerId !== controller.id) return;
    const controllerId = controller.id;
    try {
      const updated = await saveControllerConfig(controllerId, buildControllerConfig());
      if (selectedControllerId === controllerId) currentControllerConfig = updated;
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to update live controller config');
    }
  };

  function scheduleLiveControllerConfigSync() {
    if (!controller || !currentControllerConfig) return;
    liveConfigSync.schedule();
  }

  const setTriggerEffect = (value: string) => {
    triggerEffect = value;
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  const setTriggerIntensity = (value: string) => {
    triggerIntensity = value;
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  const setVibrationIntensity = (value: string) => {
    vibrationIntensity = value;
    scheduleLiveControllerConfigSync();
  };

  const setVibrationMode = (value: string) => {
    vibrationMode = value;
    scheduleLiveControllerConfigSync();
  };

  const setLightbarEnabled = (enabled: boolean) => {
    lightbarEnabled = enabled;
    scheduleLiveControllerConfigSync();
  };

  const setLightbarBrightness = (value: number | string) => {
    lightbarBrightness = normalizeTriggerPercent(value);
    scheduleLiveControllerConfigSync();
  };

  const handleLightbarColorChange = (_target: LightbarColorTarget, _color: string) => {
    scheduleLiveControllerConfigSync();
  };

  const setStickDeadzone = (side: 'left' | 'right', value: number | string) => {
    if (side === 'left') leftStickDeadzone = normalizeStickDeadzone(value);
    else rightStickDeadzone = normalizeStickDeadzone(value);
    scheduleLiveControllerConfigSync();
  };
  const restoreDefaults = async () => {
    const selectedProfile = profiles.find((profile) => profile.id === (selectedOverrideProfileId || activeProfileId));
    const profileId = selectedProfile && !selectedProfile.builtIn
      ? defaultProfileIdForGame(profileContextGame, profiles, activeProfileId, currentControllerConfig) || 'global'
      : selectedProfile?.id ?? defaultProfileIdForGame(profileContextGame, profiles, activeProfileId, currentControllerConfig);
    if (!profileId) {
      setApplyMessage('No active profile selected');
      return;
    }
    const profileName = profiles.find((profile) => profile.id === profileId)?.name ?? activeProfileName;

    try {
      await selectProfileForScope(profileId);
      setApplyMessage(`Restored ${profileName}`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to restore active profile');
    }
  };

  const setApplyMessage = (message: string, tone: ToastTone = toastToneForMessage(message)) => {
    applyMessage = message;
    showToast(message, tone);
    window.setTimeout(() => {
      if (applyMessage === message) applyMessage = '';
    }, 2600);
  };

  const setAppSettingsMessage = (message: string, tone: ToastTone = toastToneForMessage(message)) => {
    appSettingsMessage = message;
    showToast(message, tone);
    window.setTimeout(() => {
      if (appSettingsMessage === message) appSettingsMessage = '';
    }, 4200);
  };

  const setProfileOverrideMessage = (message: string, tone: ToastTone = toastToneForMessage(message)) => {
    profileOverrideMessage = message;
    showToast(message, tone);
  };

  const setSupportBundleMessage = (message: string, tone: ToastTone = toastToneForMessage(message, 'info')) => {
    supportBundleMessage = message;
    supportBundleTone = tone;
    showToast(message, tone);
  };

  const loadSupportBundle = async (): Promise<{ bundle: SupportBundle; fallback: boolean }> => {
    try {
      return { bundle: await getSupportBundle(), fallback: false };
    } catch (caught) {
      if (!snapshot) throw caught;
      const message = caught instanceof Error ? caught.message : 'Support bundle endpoint unavailable.';
      return {
        bundle: buildUiSupportBundle({
          snapshot,
          status,
          listenOnAllInterfaces,
          selectedTuningScope,
          selectedTuningGame,
          activeProfile,
          controllers,
          diagnostics,
          supportedGames,
          effectState,
          logs,
          agentBundleError: message
        }),
        fallback: true
      };
    }
  };

  const copySupportBundle = async () => {
    if (supportBundleBusy) return;
    supportBundleBusy = 'copy';
    try {
      const { bundle, fallback } = await loadSupportBundle();
      const body = JSON.stringify(bundle, null, 2);
      if (!navigator.clipboard?.writeText) {
        downloadSupportBundleText(body);
        setSupportBundleMessage('Clipboard unavailable. Exported a sanitized support bundle instead.', 'info');
        return;
      }
      await navigator.clipboard.writeText(body);
      setSupportBundleMessage(
        fallback ? 'Copied a sanitized UI support bundle. The agent bundle endpoint was unavailable.' : 'Copied sanitized support bundle.',
        fallback ? 'info' : 'success'
      );
    } catch (caught) {
      setSupportBundleMessage(caught instanceof Error ? caught.message : 'Unable to copy support bundle.', 'error');
    } finally {
      supportBundleBusy = '';
    }
  };

  const exportSupportBundle = async () => {
    if (supportBundleBusy) return;
    supportBundleBusy = 'download';
    try {
      const { bundle, fallback } = await loadSupportBundle();
      downloadSupportBundleText(JSON.stringify(bundle, null, 2));
      setSupportBundleMessage(
        fallback ? 'Exported a sanitized UI support bundle. The agent bundle endpoint was unavailable.' : 'Exported sanitized support bundle.',
        fallback ? 'info' : 'success'
      );
    } catch (caught) {
      setSupportBundleMessage(caught instanceof Error ? caught.message : 'Unable to export support bundle.', 'error');
    } finally {
      supportBundleBusy = '';
    }
  };

  const updateLanAccess = async (nextListenOnAllInterfaces = !listenOnAllInterfaces) => {
    if (!snapshot || appSettingsBusy) return;
    if (nextListenOnAllInterfaces === listenOnAllInterfaces) return;
    appSettingsBusy = true;
    try {
      const updated = await saveAppSettings({ listenOnAllInterfaces: nextListenOnAllInterfaces });
      snapshot = {
        ...snapshot,
        appSettings: updated,
        status: { ...snapshot.status, bindAddress: updated.effectiveBindAddress }
      };
      setAppSettingsMessage(
        updated.restartRequired
          ? `Saved. Restart DSCC to use ${updated.desiredBindAddress}.`
          : `Web UI is listening on ${updated.effectiveBindAddress}.`,
        updated.restartRequired ? 'info' : 'success'
      );
      await refresh();
    } catch (caught) {
      setAppSettingsMessage(caught instanceof Error ? caught.message : 'Unable to update LAN access.', 'error');
    } finally {
      appSettingsBusy = false;
    }
  };

  const updateForzaGlyphOverride = async () => {
    if (!snapshot || appSettingsBusy) return;
    appSettingsBusy = true;
    try {
      const updated = await saveAppSettings({
        forzaPlaystationGlyphs: {
          enabled: !glyphOverrideEnabled,
          installPath: forzaGlyphs?.installPath ?? null
        }
      });
      snapshot = { ...snapshot, appSettings: updated };
      setAppSettingsMessage(updated.settings.forzaPlaystationGlyphs.lastMessage, 'success');
      await refresh();
    } catch (caught) {
      setAppSettingsMessage(caught instanceof Error ? caught.message : 'Unable to update controller button glyphs.', 'error');
    } finally {
      appSettingsBusy = false;
    }
  };

  const applyProfileOverride = async () => {
    if (!snapshot || !selectedOverrideProfileId) return;
    try {
      const resolution = await setProfileOverrideForTargets(selectedOverrideProfileId, profileContextGameId);
      if (resolution) snapshot = { ...snapshot, profileResolution: resolution };
      setProfileOverrideMessage(`${selectedOverrideProfile?.name ?? selectedOverrideProfileId} is now used for ${overrideScope}`, 'success');
      await refresh();
    } catch (caught) {
      setProfileOverrideMessage(caught instanceof Error ? caught.message : 'Unable to set profile override.', 'error');
    }
  };

  const returnToAutomaticProfile = async () => {
    if (!snapshot) return;
    const previousScope = overrideScope;
    try {
      const resolution = await clearProfileOverrideForTargets(profileContextGameId);
      if (resolution) snapshot = { ...snapshot, profileResolution: resolution };
      setProfileOverrideMessage(`Automatic profile selection restored for ${previousScope}`, 'success');
      await refresh();
    } catch (caught) {
      setProfileOverrideMessage(caught instanceof Error ? caught.message : 'Unable to clear profile override.', 'error');
    }
  };

  const profileManagementStore: ProfileManagementStateStore = {
    get: () => ({
      renameProfileId,
      renameProfileName,
      renameBusy: profileRenameBusy,
      saveBusy: profileSaveBusy,
      saveAsOpen: saveAsProfileOpen,
      saveAsName: saveAsProfileName,
      saveAsBusy: profileSaveAsBusy,
      fileBusy: profileFileBusy
    }),
    set: (next) => {
      renameProfileId = next.renameProfileId;
      renameProfileName = next.renameProfileName;
      profileRenameBusy = next.renameBusy;
      profileSaveBusy = next.saveBusy;
      saveAsProfileOpen = next.saveAsOpen;
      saveAsProfileName = next.saveAsName;
      profileSaveAsBusy = next.saveAsBusy;
      profileFileBusy = next.fileBusy;
    }
  };

  const profileManagement = createProfileManagement({
    store: profileManagementStore,
    getSnapshot: () => snapshot,
    setSnapshot: (next) => {
      snapshot = next;
    },
    getProfiles: () => profiles,
    getActiveProfileId: () => activeProfileId,
    getSelectedActionProfile: () => selectedActionProfile ?? null,
    getProfileContextGame: () => profileContextGame ?? null,
    getProfileContextGameId: () => profileContextGameId,
    getSelectedTuningScope: () => selectedTuningScope,
    getSelectedOverrideProfileId: () => selectedOverrideProfileId,
    setSelectedOverrideProfileId: (id) => {
      selectedOverrideProfileId = id;
    },
    markActiveProfileSynced: (id) => {
      lastSyncedActiveProfileId = id;
    },
    getControllerId: () => controller?.id,
    resetConfigLoaded: () => {
      configLoadedFor = '';
    },
    loadControllerConfig,
    buildControllerConfig: () => buildControllerConfig(),
    profileConfigSignature: (config) => profileConfigSignature(config),
    setProfileSaveBaseline: (signature) => {
      profileSaveBaselineSignature = signature;
      // Saving makes the current draft the new saved truth for the rail diff.
      profileSaveBaselineConfig = buildControllerConfig();
    },
    saveControllerConfigForProfileTargets,
    setProfileOverrideForTargets: (profileId, gameId) => setProfileOverrideForTargets(profileId, gameId),
    refresh,
    notify: setApplyMessage
  });

  const activateProfileById = profileManagement.activateProfileById;
  const beginRenameSelectedProfile = profileManagement.beginRenameSelectedProfile;
  const cancelRenameProfile = profileManagement.cancelRenameProfile;
  const submitRenameProfile = profileManagement.submitRenameProfile;
  const handleRenameProfileKeydown = profileManagement.handleRenameProfileKeydown;
  const beginSaveAsProfile = profileManagement.beginSaveAsProfile;
  const cancelSaveAsProfile = profileManagement.cancelSaveAsProfile;
  const submitSaveAsProfile = profileManagement.submitSaveAsProfile;
  const handleSaveAsProfileKeydown = profileManagement.handleSaveAsProfileKeydown;
  const saveActiveProfile = profileManagement.saveActiveProfile;
  const deleteProfileById = profileManagement.deleteProfileById;
  const exportSelectedProfile = profileManagement.exportSelectedProfile;
  const handleProfileImport = profileManagement.handleProfileImport;

  const telemetryRateStatusText = (item: AppSnapshot['adapters'][number] | undefined) => {
    if (!item) return 'no active stream';
    if (item.state === 'running') return `${item.name} / live packets`;
    if (item.state === 'needs_setup') return `${item.name} / waiting for UDP`;
    if (item.state === 'ready') return `${item.name} / listening`;
    if (item.state === 'faulted') return `${item.name} / blocked`;
    return item.name;
  };

  function stopTriggerInputPolling() {
    triggerInputPoller.stop();
  }

  function clearBaseFeelTestTimers() {
    baseFeelTestDurationTimer.clear();
    baseFeelTestRefreshTask.clear();
  }

  function markBaseFeelTestInactive() {
    baseFeelTestActive = false;
    baseFeelTestBusy = false;
    clearBaseFeelTestTimers();
    stopTriggerInputPolling();
  }

  function shouldPollTriggerInput() {
    return Boolean(
      controller?.id &&
        activeView === 'tuning' &&
        typeof window !== 'undefined' &&
        typeof document !== 'undefined' &&
        !document.hidden
    );
  }

  function syncTriggerInputPolling() {
    triggerInputPoller.sync();
  }

  $: if (typeof window !== 'undefined' && isViewHash(window.location.hash)) {
    const routeView = appViewFromHash();
    if (routeView !== activeView) {
      activeView = routeView;
      syncTriggerInputPolling();
    }
  }

  async function pollTriggerInput() {
    await triggerInputPoller.poll();
  }

  function startTriggerInputPolling() {
    triggerInputPoller.start();
  }

  function armBaseFeelTestTimer() {
    baseFeelTestDurationTimer.arm();
  }

  function scheduleBaseFeelTestRefresh() {
    baseFeelTestRefreshTask.schedule();
  }

  const baseFeelTestRequest = (): EffectTestRequest => ({
    target: 'base_feel',
    mode: 'hold',
    intensity: 100,
    durationMs: BASE_FEEL_TEST_DURATION_MS,
    l2Position: controllerInputFresh ? l2ControllerPress : undefined,
    r2Position: controllerInputFresh ? r2ControllerPress : undefined,
    trigger: buildControllerConfig().trigger
  });

  const startBaseFeelTest = async (refreshOnly = false) => {
    if (!snapshot) return;
    if (!refreshOnly) baseFeelTestBusy = true;
    try {
      if (!refreshOnly) await pollTriggerInput();
      // The test response's output frame has no UI consumers; reassigning the
      // whole snapshot here invalidated every snapshot-derived statement per
      // 35ms refresh tick. The 1Hz snapshot stream keeps effectState current.
      await runEffectTest(baseFeelTestRequest(), controller?.id);
      baseFeelTestActive = true;
      startTriggerInputPolling();
      armBaseFeelTestTimer();
      if (!refreshOnly) {
        setApplyMessage('Base feel test is live. Squeeze L2/R2 while adjusting curves; hardware output now follows the same curve shown in the graph.');
      }
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Base feel test failed');
      markBaseFeelTestInactive();
    } finally {
      if (!refreshOnly) baseFeelTestBusy = false;
    }
  };

  const stopBaseFeelTest = async () => {
    if (!snapshot) {
      markBaseFeelTestInactive();
      return;
    }
    baseFeelTestBusy = true;
    baseFeelTestRefreshTask.clear();
    try {
      // Output frame has no UI consumers; the 1Hz snapshot stream keeps state current.
      await runEffectTest(
        {
          target: 'base_feel',
          mode: 'off',
          intensity: 0,
          durationMs: 100
        },
        controller?.id
      );
      setApplyMessage('Base feel test stopped');
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to stop Base feel test');
    } finally {
      markBaseFeelTestInactive();
    }
  };

  const toggleBaseFeelTest = async () => {
    if (baseFeelTestBusy) return;
    if (baseFeelTestActive) {
      await stopBaseFeelTest();
    } else {
      await startBaseFeelTest();
    }
  };

  const previewBodyHaptics = async () => {
    if (!snapshot) return;
    const intensity = vibrationIntensityPercent(vibrationIntensity);
    if (intensity <= 0) {
      setApplyMessage('Body haptics are off; raise Body strength to preview.');
      return;
    }

    try {
      // Output frame has no UI consumers; the 1Hz snapshot stream keeps state current.
      await runEffectTest(
        {
          target: 'rumble',
          mode: vibrationModeRequest(vibrationMode),
          intensity,
          durationMs: 900
        },
        controller?.id
      );
      setApplyMessage(`${vibrationMode} body haptics previewed`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Body haptics preview failed');
    }
  };

  const previewLightbarColor = async (color: string, label: string) => {
    // /test-effect takes parameters in the request body, so preview first
    // and only persist the config if the preview is accepted by the agent.
    if (!snapshot) return;

    const intensity = lightbarEnabled ? lightbarBrightness : 0;
    try {
      // Output frame has no UI consumers; the 1Hz snapshot stream keeps state current.
      await runEffectTest(
        {
          target: 'lightbar',
          mode: color,
          intensity,
          durationMs: 650
        },
        controller?.id
      );
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : `${label} preview failed`);
      return;
    }

    const saved = await saveCurrentConfig();
    if (!saved) return;
    await refresh();
    setApplyMessage(`${label} ${color} previewed`);
  };

  const previewLightbar = async () => previewLightbarColor(lightbarColor, 'Lightbar');
  const previewRpmColor = async () => previewLightbarColor(rpmColor, 'Redline Ramp');

  const startAppRuntime = () => {
    if (typeof window === 'undefined' || appRuntime?.isStarted()) return;
    appRuntime = createAppRuntime({
      fallbackPollIntervalMs: FALLBACK_POLL_INTERVAL_MS,
      snapshotInvalidationDebounceMs: SNAPSHOT_INVALIDATION_DEBOUNCE_MS,
      refresh,
      applySnapshot,
      connectSnapshotSocket: connectAppSnapshotSocket,
      onStart: () => {
        loadDismissedUpdateVersion();
        loadOnboardingPreference();
        syncViewFromHash();
        syncTriggerInputPolling();
      },
      onVisible: syncTriggerInputPolling,
      onHidden: syncTriggerInputPolling,
      onHashChange: () => {
        syncViewFromHash();
        syncTriggerInputPolling();
      },
      onStop: () => {
        liveConfigSync.clear();
        clearBaseFeelTestTimers();
        stopTriggerInputPolling();
        window.clearTimeout(savedRailDiffTimer);
      }
    });
    appRuntime.start();
  };

  const stopAppRuntime = () => {
    appRuntime?.stop();
    appRuntime = undefined;
  };

  onMount(() => {
    startAppRuntime();
    return stopAppRuntime;
  });

  // Live trigger polling feeds the haptics curve cursor and the base-feel test.
  // It is intentionally limited to the visible Haptics view so inactive routes
  // do not spend the 25Hz input budget or trigger extra DOM work.
  $: if (controller?.id && activeView === 'tuning') {
    startTriggerInputPolling();
  } else {
    stopTriggerInputPolling();
  }

  $: if (controller?.id && controller.id !== configLoadedFor) {
    void loadControllerConfig(controller.id);
  }

  $: if (controller?.id && isEdgeTargetController(controller)) {
    void loadEdgeProfiles(controller.id);
  } else if (
    shouldResetEdgeOnboardProfiles({
      controller,
      loadedFor: edgeProfilesLoadedFor,
      profiles: edgeProfiles
    })
  ) {
    resetEdgeProfiles();
  }
</script>

<div class="app-shell">
  <AppSidebar
    view={activeView}
    readiness={{ tuningReady, buttonMappingReady, edgeSlotsReady }}
    onNavigate={navigateToView}
  >
    {#snippet footer()}
      <div class="sidebar-footer">
        <button
          class="sidebar-item"
          type="button"
          title="Open the quick start guide again. It explains Profiles, trigger tests, telemetry safety, and support bundles."
          aria-label="Open quick start guide"
          onclick={openOnboarding}
        >
          <CircleHelp size={14} /> Guide
        </button>
        <button
          class:active={supportPanelOpen}
          class="sidebar-item"
          type="button"
          title="Copy or export a sanitized support bundle for GitHub issues or Discord help. Raw hardware ids are excluded."
          aria-expanded={supportPanelOpen}
          aria-controls="support-bundle-panel"
          onclick={() => {
            supportPanelOpen = !supportPanelOpen;
          }}
        >
          <LifeBuoy size={14} /> Support
        </button>
      </div>
    {/snippet}
  </AppSidebar>

  <main class="app-main">
  {#if loading}
    <section class="ops-state">
      <RefreshCw class="spin" size={24} />
      <strong>Initializing command surface</strong>
      <span>Synchronizing controller, profile, and telemetry state</span>
    </section>
  {:else if error}
    <section class="ops-state">
      <Cable size={26} />
      <strong>Agent unavailable</strong>
      <span>{error}</span>
      <button class="solid-action compact" type="button" onclick={refresh}>Retry</button>
    </section>
  {:else if snapshot}
    <!-- Utility row: cross-view context that has no single page home — the
         target controller for writes, the web UI bind address, the Forza glyph
         override, and a compact system readout. -->
    <section class="app-toolbar" aria-label="Controller and display options">
      <label class="app-toolbar-field">
        <span>Target Controller</span>
        <select
          aria-label="Target controller"
          disabled={!connectedControllerIds.length}
          value={profileTargetsAllConnected ? '__all__' : profileTargetControllerIds[0] ?? controller?.id ?? ''}
          onchange={(event) => {
            const picked = event.currentTarget.value;
            if (picked === '__all__') pickAllControllers();
            else if (picked) pickControllerTarget(picked);
          }}
        >
          {#if connectedControllerIds.length > 1}
            <option value="__all__">All Connected</option>
          {/if}
          {#each connectedControllers as item (item.id)}
            <option value={item.id}>{item.name || controllerModelText(item)}</option>
          {/each}
        </select>
      </label>
      <label class="app-toolbar-field">
        <span>Web UI Location</span>
        <select
          aria-label="Web UI location"
          disabled={appSettingsBusy}
          title={lanRestartRequired ? `restart -> ${appSettings?.desiredBindAddress}` : status?.bindAddress}
          value={listenOnAllInterfaces ? 'lan' : 'local'}
          onchange={(event) => void updateLanAccess(event.currentTarget.value === 'lan')}
        >
          <option value="local">Local Only</option>
          <option value="lan">LAN Access</option>
        </select>
        <small>{lanRestartRequired ? `restart -> ${appSettings?.desiredBindAddress}` : status?.bindAddress}</small>
      </label>
      <button
        class="app-toolbar-toggle"
        class:active={glyphOverrideEnabled}
        type="button"
        disabled={appSettingsBusy}
        aria-pressed={glyphOverrideEnabled}
        title={forzaGlyphs?.lastStatus ?? glyphInstallPath}
        onclick={() => void updateForzaGlyphOverride()}
      >
        Controller Glyphs: {glyphOverrideEnabled ? 'PlayStation Icons' : 'Game Default'}
      </button>
      <div class="app-toolbar-spacer"></div>
      <div
        class="app-toolbar-readout"
        title={selectedTuningScope === 'global' ? systemReadoutDetail : adapter?.setupHint ?? telemetryRateDetail}
      >
        <span>{systemReadoutTitle}</span>
        <p><strong>{systemReadoutValue}</strong><small>{systemReadoutDetail}</small></p>
      </div>
    </section>

    {#if showPartialErrorBanner}
      <aside class="ops-warning dm-warning" role="status" aria-live="polite">
        <span>Some areas are temporarily unavailable: {partialErrorAreas.join(', ')}.</span>
        <button type="button" aria-label="Dismiss partial agent data notice" onclick={dismissPartialErrors}>dismiss</button>
      </aside>
    {/if}

    {#if showUpdateBanner}
      <aside class="ops-warning dm-warning update" role="status" aria-live="polite">
        <span>Update available: {updateCheck.latestVersion}. Current build {updateCheck.currentVersion}.</span>
        <div class="dm-warning-actions">
          <a href={updateCheck.releaseUrl ?? UPDATE_RELEASE_PAGE_URL} target="_blank" rel="noreferrer">
            <ExternalLink size={13} /> Download
          </a>
          <button type="button" aria-label="Dismiss update notice" onclick={dismissUpdateBanner}>dismiss</button>
        </div>
      </aside>
    {/if}

    {#if supportPanelOpen}
      <SupportPanel
        busy={supportBundleBusy}
        message={supportBundleMessage}
        tone={supportBundleTone}
        onCopy={copySupportBundle}
        onExport={exportSupportBundle}
      />
    {/if}

    <OnboardingTutorial
      open={onboardingOpen}
      onClose={dismissOnboarding}
      onNavigate={navigateToView}
    />

    {#if showStatusView}
      <StatusView
        {controllers}
        {controller}
        detectedGame={selectedGame}
        detectedGameName={snapshot.gameDetection.activeGameName ?? null}
        {activeProfile}
        {activeProfileName}
        {overrideActive}
        {adapter}
        adapters={snapshot.adapters ?? []}
        renameActiveId={controllerRenameId}
        bind:renameName={controllerRenameName}
        renameBusy={controllerRenameBusy}
        onBeginRename={beginControllerRename}
        onSubmitRename={submitControllerRename}
        onCancelRename={cancelControllerRename}
        onRenameKeydown={handleControllerRenameKeydown}
        onOpenSetupGuide={openSetupGuideFromStatus}
      />
    {/if}
    {#if showTuningView}
      <TuningHeader
        scope={selectedTuningScope}
        selectedGame={selectedTuningGame}
        {discoveredGames}
        adapterRunning={adapter?.state === 'running'}
        packetRateHz={telemetryPacketRate}
        setupVerified={selectedGameSetupVerified}
        setupGuideOpen={showSetupGuide}
        onToggleSetupGuide={toggleSetupGuide}
        onOpenSetupGuide={openSetupGuide}
        {controller}
        profiles={profileContextProfiles}
        {activeProfileId}
        {selectedOverrideProfileId}
        {selectedActionProfile}
        {canRenameSelectedProfile}
        {canDeleteSelectedProfile}
        {profileConfigDirty}
        unsavedChangeCount={unsavedCount}
        {profileSaveBusy}
        {profileFileBusy}
        {profileSaveAsBusy}
        {profileRenameBusy}
        saveAsOpen={saveAsProfileOpen}
        bind:saveAsName={saveAsProfileName}
        {renameProfileId}
        bind:renameName={renameProfileName}
        onSelectGlobal={selectGlobalTuning}
        onSelectGame={selectTuningGame}
        onOpenAddGame={openAddGameDialog}
        onSelectProfile={selectProfileForScope}
        onSaveProfile={saveActiveProfile}
        onBeginSaveAs={beginSaveAsProfile}
        onCancelSaveAs={cancelSaveAsProfile}
        onSubmitSaveAs={submitSaveAsProfile}
        onSaveAsKeydown={handleSaveAsProfileKeydown}
        onBeginRename={beginRenameSelectedProfile}
        onCancelRename={cancelRenameProfile}
        onSubmitRename={submitRenameProfile}
        onRenameKeydown={handleRenameProfileKeydown}
        onDeleteProfile={(profile) => void deleteProfileById(profile.id, profile.name)}
        onRestoreDefaults={restoreDefaults}
        onImportFile={handleProfileImport}
        onExportProfile={exportSelectedProfile}
      />
    {/if}
    {#if showWorkspaceViews}
      {#if showAdvancedControllerView}
        <ControllersView
          active={showAdvancedControllerView}
          {controllers}
          {controller}
          {selectedControllerId}
          renameActiveId={controllerRenameId}
          bind:renameName={controllerRenameName}
          renameBusy={controllerRenameBusy}
          {currentControllerConfig}
          {leftStickDeadzone}
          {rightStickDeadzone}
          inputBridge={snapshot?.inputBridge ?? null}
          activeGameName={selectedGame?.name ?? null}
          activeInputProvider={selectedGame?.inputProvider ?? currentControllerConfig?.inputMode ?? 'native_dualsense'}
          onSelect={selectTargetController}
          onBeginRename={beginControllerRename}
          onSubmitRename={submitControllerRename}
          onCancelRename={cancelControllerRename}
          onRenameKeydown={handleControllerRenameKeydown}
          {inputBridgeBusy}
          onSetInputMode={saveControllerInputMode}
          onSetStickDeadzone={setStickDeadzone}
          onStartInputBridge={startControllerInputBridge}
          onStopInputBridge={stopControllerInputBridge}
          {supportBundleBusy}
          onDownloadSupportBundle={exportSupportBundle}
        />
      {/if}

      {#if showEdgeSlotsView}
        <EdgeSlotsView
          {controller}
          {currentControllerConfig}
          {edgeProfiles}
          {edgeProfilesLoading}
          {edgeProfilesBusySlot}
          {edgeProfilesError}
          {edgeSlotsReadTooltip}
          edgeSlotWriteLabel={edgeSlotWriteLabel()}
          onRefreshEdgeProfiles={() => controller && void loadEdgeProfiles(controller.id, true)}
          onWriteEdgeSlot={writeCurrentConfigToEdgeSlot}
          {edgeSlotName}
          {edgeSlotStatus}
          {edgeSlotInfoTooltip}
          {edgeSlotWriteTooltip}
        />
      {/if}

      {#if activeView === 'tuning' && tuningReady}
      <!-- Each trigger column owns its own curve editor instrument. -->
      {#snippet triggerCurveEditor(trigger: 'L2' | 'R2')}
        <TriggerCurvesPanel
          {trigger}
          showControls={false}
          {selectedTuningScope}
          {snapshot}
          {baseFeelTestActive}
          {baseFeelTestBusy}
          {resetTriggerCurvesToProfileDefaults}
          {toggleBaseFeelTest}
          {l2CurveShape}
          {r2CurveShape}
          {l2CurveLive}
          {r2CurveLive}
          {curveHover}
          {curveDragPoint}
          {l2SavedCurvePath}
          {r2SavedCurvePath}
          {l2LivePress}
          {r2LivePress}
          {l2From}
          {l2To}
          {r2From}
          {r2To}
          {l2Curve}
          {r2Curve}
          {l2CurvePoints}
          {r2CurvePoints}
          {triggerPressLabel}
          triggerRangeTooltip={triggerRangeTooltipForCurrentTuning}
          {triggerCurveTooltip}
          {showTriggerPress}
          {handleCurvePointer}
          {updateCurveHover}
          {clearCurveHover}
          {handleCurvePointPointer}
          {setTriggerRangeValue}
          {setTriggerCurveValue}
          {removeCurvePoint}
          {addCurvePoint}
        />
      {/snippet}
      <!-- One embedded effect list per semantic column (game scope only). -->
      {#snippet forzaEffectGroup(metas: ForzaEffectMeta[])}
        <TelemetryRoutingPanel
          showChrome={false}
          forzaEffectMetas={metas}
          {forzaEffectsById}
          {effectStatusById}
          {forzaBrakeTuning}
          {forzaAbsTuning}
          {forzaThrottleTuning}
          {forzaShiftTuning}
          {forzaRevLimiterTuning}
          {forzaRoutes}
          {forzaEffect}
          {updateForzaBrakeTuning}
          {updateForzaAbsTuning}
          {updateForzaThrottleTuning}
          {updateForzaShiftTuning}
          {updateForzaRevLimiterTuning}
          {updateForzaEffect}
          {intensityTooltip}
          {routeTooltip}
          {forzaIntensityPercent}
          {forzaIntensityFromPercent}
        />
      {/snippet}
      {#if showSetupGuide && selectedTuningGame}
        <!-- Setup is a canvas state: the walkthrough replaces the tuning grid
             (rail included) until the game's requirements verify or the
             manually re-opened guide is toggled away. -->
        <SetupGuide
          game={selectedTuningGame}
          telemetryFresh={selectedGameTelemetryFresh}
          verified={selectedGameSetupVerified}
          port={telemetryPort}
          packetRateHz={telemetryPacketRate}
          adapterName={adapter?.name ?? ''}
          adapterHint={adapter?.setupHint ?? ''}
          onVerified={completeSetupGuide}
          onStartTuning={completeSetupGuide}
        />
      {:else}
      <TuningCanvas>
        <svelte:fragment slot="brake">
          {@render triggerCurveEditor('L2')}
          {#if selectedTuningScope === 'game'}
            {@render forzaEffectGroup(brakeEffectMetas)}
          {/if}
        </svelte:fragment>
        <svelte:fragment slot="throttle">
          {@render triggerCurveEditor('R2')}
          {#if selectedTuningScope === 'game'}
            {@render forzaEffectGroup(throttleEffectMetas)}
          {/if}
        </svelte:fragment>
        <svelte:fragment slot="road">
          {#if selectedTuningScope === 'game'}
            {@render forzaEffectGroup(roadEffectMetas)}
          {/if}
        </svelte:fragment>
        <svelte:fragment slot="lights">
          <LightbarControls
            {selectedTuningScope}
            {lightbarEnabled}
            bind:lightbarColor
            bind:rpmColor
            {lightbarBrightness}
            onColorChange={handleLightbarColorChange}
            {setLightbarBrightness}
            {setLightbarEnabled}
            {previewLightbar}
            {previewRpmColor}
          />
          {#if selectedTuningScope === 'game'}
            {@render forzaEffectGroup(lightEffectMetas)}
          {/if}
        </svelte:fragment>
        <svelte:fragment slot="rail">
          <SavedRail
            profileName={savedRailProfileName}
            rows={savedRailRows}
            dirty={profileConfigDirty}
            previewActive={baseFeelTestActive}
            previewBusy={baseFeelTestBusy}
            previewDisabled={!snapshot}
            saveBusy={profileSaveBusy}
            canSave={Boolean(selectedActionProfile) && profileConfigDirty}
            onPreviewFeel={toggleBaseFeelTest}
            onSave={saveActiveProfile}
            onDiscard={discardDraftChanges}
          />
        </svelte:fragment>
        <svelte:fragment slot="below">
          <!-- Parked until Tasks 8-10 re-home it; nothing previously rendered
               may be lost. Curve reset/test head + base feel strip: -->
          <TriggerCurvesPanel
            showCurves={false}
            {selectedTuningScope}
            {snapshot}
            {baseFeelTestActive}
            {baseFeelTestBusy}
            {resetTriggerCurvesToProfileDefaults}
            {toggleBaseFeelTest}
            {triggerEffect}
            {triggerIntensity}
            {vibrationIntensity}
            {vibrationMode}
            {triggerEffectOptions}
            {vibrationModeOptions}
            {triggerEffectHelp}
            {triggerStrengthHelp}
            {vibrationHelp}
            {vibrationModeHelp}
            {setTriggerEffect}
            {setTriggerIntensity}
            {setVibrationIntensity}
            {setVibrationMode}
          />
          {#if selectedTuningScope === 'game'}
            <!-- Telemetry stream head + body rumble source routing chrome. -->
            <div class="canvas-parked">
              <TelemetryRoutingPanel
                showEffects={false}
                {enabledForzaEffectCount}
                {allForzaEffectsEnabled}
                {forzaEffectMetas}
                {forzaBodyRumbleMode}
                {bodyRumbleModeOptions}
                {toggleAllForzaEffects}
                {setForzaBodyRumbleMode}
              />
            </div>
          {:else}
            <!-- Global scope: base haptics panel (trigger pattern + body). -->
            <div class="canvas-parked">
              <GlobalFeelPanel
                {snapshot}
                {baseFeelTestActive}
                {baseFeelTestBusy}
                {triggerEffect}
                {triggerIntensity}
                {vibrationIntensity}
                {vibrationMode}
                {triggerEffectOptions}
                {vibrationModeOptions}
                {triggerEffectHelp}
                {vibrationModeHelp}
                {setTriggerEffect}
                {setVibrationIntensity}
                {setVibrationMode}
                {toggleBaseFeelTest}
                {previewBodyHaptics}
              />
            </div>
          {/if}
        </svelte:fragment>
      </TuningCanvas>
      {/if}
      {/if}
    <ButtonMappingView session={buttonMappingSession} />
    {/if}
  {/if}
  <ToastStack messages={toastMessages} onDismiss={dismissToast} />

  <AddGameDialog
    open={addGameOpen}
    entries={addGameEntries}
    loading={addGameLoading}
    busyAppId={addGameBusyAppId}
    errorMessage={addGameError}
    onClose={closeAddGameDialog}
    onAdd={(entry, processNames) => void addGameFromLibrary(entry, processNames)}
    onValidateLocal={validateLocalGameFromDialog}
    onAddLocal={addLocalGameFromDialog}
  />
  </main>
</div>
