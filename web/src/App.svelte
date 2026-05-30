<script lang="ts">
  import { Cable, CircleHelp, ExternalLink, LifeBuoy, RefreshCw } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import Tooltip from './components/Tooltip.svelte';
  import ContextRibbon from './components/ContextRibbon.svelte';
  import AddGameDialog from './lib/features/games/AddGameDialog.svelte';
  import GamesView from './lib/features/games/GamesView.svelte';
  import OnboardingTutorial from './components/OnboardingTutorial.svelte';
  import ViewNav from './components/ViewNav.svelte';
  import SupportPanel from './components/SupportPanel.svelte';
  import ToastStack from './components/ToastStack.svelte';
  import { appViews, guardView, hashForView, viewFromHash, viewTooltips } from './app/navigation';
  import { createAppRuntime } from './app/runtime';
  import {
    ButtonMappingView,
    assembleSteamBindingRaw,
    buildSteamBindingBySlotKey,
    createSteamMirrorGroups,
    parseSteamBindingTriple,
    steamBindingKey,
    steamBindingSlots,
    steamBindingTargetPart,
    steamSlotGlyphs
  } from './lib/features/buttonMapping';
  import { bindingTargetGroupsForProvider } from './lib/features/buttonMapping/buttonMappingState';
  import {
    EMPTY_STEAM_BINDING_MAP,
    EMPTY_STEAM_INPUT_BINDINGS,
    EMPTY_STEAM_MIRROR_GROUPS,
    defaultSteamBindingsForFamily,
    preparedSteamBindingTargetGroups
  } from './lib/features/buttonMapping/steamBindingTargets';
  import {
    profileTargetsCoverAllConnectedControllers,
    reconcileProfileTargetControllerIds,
    resolveSelectedControllerId,
    resolveSelectedProfileTargetIds,
    selectCurrentController,
    summarizeProfileTargets
  } from './app/controllerSelection';
  import { markOnboardingDismissed, shouldOpenOnboarding } from './app/onboardingState';
  import {
    baseForzaTriggerDefaults,
    buildBuiltInProfileConfig,
    buildControllerConfigDraft,
    buildDefaultControllerConfig as createDefaultControllerConfig,
    editableConfigFromController as createEditableConfigFromController,
    editableConfigFromProfileExport as createEditableConfigFromProfileExport,
    normalizeForzaBodyRumbleMode,
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
  import ControllersView from './lib/features/controllers/ControllersView.svelte';
  import HapticsAside from './lib/features/haptics/HapticsAside.svelte';
  import HapticsView from './lib/features/haptics/HapticsView.svelte';
  import TriggerCurvesPanel from './lib/features/haptics/TriggerCurvesPanel.svelte';
  import {
    TRIGGER_CURVE_POINT_MAX,
    TRIGGER_CURVE_POINT_MIN,
    clampUnit,
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
    forzaTriggerForceModelFor,
    intensityTooltip,
    routeTooltip,
    triggerCurveLiveView,
    triggerCurveShapeView,
    triggerCurveTooltip,
    triggerCurveValueFor,
    triggerPressLabel,
    triggerRangeTooltip,
    triggerRangeValuesFor,
    triggerStrengthScalarFor,
    vibrationIntensityPercent,
    vibrationModeRequest,
    type TriggerCurveDisplayMode,
    type TriggerSide
  } from './lib/features/haptics/hapticsCurvePresentation';
  import {
    bodyRumbleModeOptions,
    forzaEffectMetas,
    forzaRoutes,
    shiftThumpPresetHelp,
    shiftThumpPresets,
    triggerEffectHelp,
    triggerEffectOptions,
    triggerStrengthHelp,
    vibrationHelp,
    vibrationModeHelp,
    vibrationModeOptions
  } from './lib/features/haptics/hapticsOptions';
  import {
    clamp,
    clampForzaIntensity,
    defaultForzaAbsTuning,
    defaultForzaEffects,
    defaultForzaRevLimiterTuning,
    defaultForzaShiftTuning,
    defaultForzaThrottleTuning,
    forzaIntensityFromPercent,
    forzaIntensityPercent,
    forzaPresetEffects,
    normalizeEffectId,
    normalizeForzaAbsTuning,
    normalizeForzaEffects,
    normalizeForzaRevLimiterTuning,
    normalizeForzaShiftTuning,
    normalizeForzaThrottleTuning
  } from './app/hapticsState';
  import {
    achievementText,
    formatLastPlayed,
    formatPlaytime,
    gameAccentColor,
    gameArtwork,
    gameDetectionStatusText,
    gameLauncherLabel,
    gameMediaDetails,
    gameProviderMeta,
    gameTileStatus,
    profileScopeCount as countProfilesForGame
  } from './lib/features/games/gamePresentation';
  import {
    assignmentForGame,
    defaultProfileIdForGame,
    profileContextTag,
    profileImportPayload,
    profilesForGame,
    sanitizeFileName,
    uniqueProfileName,
    usesForzaRuntimeProfile
  } from './lib/features/profiles/profileSelection';
  import type { AppView } from './app/navigation';
  import type { SteamBindingSlot } from './lib/features/buttonMapping';
  import {
    activateProfile,
    addCustomGame,
    addLocalApp,
    clearProfileOverride,
    connectAppSnapshotSocket,
    createProfile,
    deleteProfile,
    exportProfile,
    getAppSnapshot,
    getSupportBundle,
    getAppUpdateCheck,
    getControllerInput,
    getControllerConfig,
    getEdgeProfiles,
    getSteamLibrary,
    importProfile,
    removeCustomGame,
    renameProfile,
    runEffectTest,
    saveAppSettings,
    saveControllerConfig,
    startInputBridgeSession,
    stopInputBridgeSession,
    writeEdgeProfile,
    saveProfileConfig,
    setProfileOverride,
    updateControllerName,
    validateLocalApp,
    writeInputBridgeBinding,
    writeSteamInputBinding,
    writeSteamInputPaddlePreset
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
    ForzaAbsTuningConfiguration,
    ForzaBodyRumbleMode,
    ForzaEffectConfiguration,
    ForzaRevLimiterTuningConfiguration,
    ForzaShiftTuningConfiguration,
    ForzaThrottleTuningConfiguration,
    ProfileSummary,
    SteamInputBinding,
    SteamInputLayout,
    SteamLibraryEntry,
    SupportBundle,
    SupportedGame,
    TriggerCurvePoint,
    ValidateLocalAppRequest
  } from './lib/types';

  type TuningScope = 'none' | 'global' | 'game';
  type SupportBundleBusy = 'copy' | 'download' | '';
  $: activeBindingTargetGroups = bindingTargetGroupsForProvider(
    preparedSteamBindingTargetGroups,
    bridgeContextActive ? 'bridge' : 'steam'
  );

  const resetSteamBindingDraft = () => {
    if (selectedSteamBinding) {
      steamBindingDraft = selectedSteamBinding.rawBinding;
      steamBindingLabelDraft = parseSteamBindingTriple(selectedSteamBinding.rawBinding).label;
      lastSteamBindingDraftKey = steamBindingKey(selectedSteamBinding);
      clearSteamBindingMessage();
    }
  };
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
  let newProfileName = '';
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
  let selectedSteamBindingKey = '';
  let selectedSteamBinding: SteamInputBinding | null = null;
  let steamBindingDraft = '';
  let steamBindingLabelDraft = '';
  let lastSteamBindingDraftKey = '';
  let optimisticSteamInputBindings: SteamInputBinding[] | null = null;
  let activeSteamMappingContextKey = '';
  let steamBindingBusy = false;
  let steamBindingMessage = '';
  let paddlePresetLeftKey = 'Q';
  let paddlePresetRightKey = 'E';
  let hoveredSteamSlotKey = '';
  let activeSteamSlotKey = '';

  let l2From = 6;
  let l2To = 100;
  let r2From = 0;
  let r2To = 100;
  let l2Curve = defaultTriggerCurve('l2');
  let r2Curve = defaultTriggerCurve('r2');
  let l2CurvePoints: TriggerCurvePoint[] = defaultTriggerCurvePoints('l2');
  let r2CurvePoints: TriggerCurvePoint[] = defaultTriggerCurvePoints('r2');
  let curveHover: { side: TriggerSide; x: number; y: number; left: number; top: number } | null = null;
  let curveDragSide: TriggerSide | null = null;
  let curveDragPoint: { side: TriggerSide; index: number } | null = null;
  let triggerCurveDisplayMode: TriggerCurveDisplayMode = 'base';
  let activeView: AppView = 'games';
  let triggerEffect = 'Adaptive resistance';
  let triggerIntensity = 'Strong (Standard)';
  let vibrationIntensity = 'Medium';
  let vibrationMode = 'Balanced';
  let forzaBodyRumbleMode: ForzaBodyRumbleMode = 'native_passthrough';
  let forzaAbsTuning: ForzaAbsTuningConfiguration = defaultForzaAbsTuning();
  let forzaThrottleTuning: ForzaThrottleTuningConfiguration = defaultForzaThrottleTuning();
  let forzaShiftTuning: ForzaShiftTuningConfiguration = defaultForzaShiftTuning();
  let forzaRevLimiterTuning: ForzaRevLimiterTuningConfiguration = defaultForzaRevLimiterTuning();
  let lightbarEnabled = true;
  let lightbarColor = '#4cc9f0';
  let rpmColor = '#ff3a2e';
  let lightbarBrightness = 72;
  let leftStickDeadzone = 0;
  let rightStickDeadzone = 0;

  let forzaEffects: ForzaEffectConfiguration[] = defaultForzaEffects();
  $: enabledForzaEffectCount = forzaEffects.filter((effect) => effect.enabled).length;
  $: allForzaEffectsEnabled = enabledForzaEffectCount === forzaEffectMetas.length;
  // Reactive lookup map so {@const tuning = ...} inside {#each} re-evaluates
  // when forzaEffects is reassigned (Svelte can't statically trace the
  // dependency through a plain function call to forzaEffect()).
  $: forzaEffectsById = new Map(forzaEffects.map((effect) => [effect.id, effect]));

  $: controllers = snapshot?.controllers ?? [];
  $: connectedControllers = controllers.filter((item) => item.connected);
  $: connectedControllerIds = connectedControllers.map((item) => item.id);
  $: {
    const nextSelectedControllerId = resolveSelectedControllerId(controllers, selectedControllerId);
    if (nextSelectedControllerId !== selectedControllerId) selectedControllerId = nextSelectedControllerId;
  }
  $: controller = selectCurrentController(controllers, selectedControllerId);
  $: {
    const nextTargets = reconcileProfileTargetControllerIds(
      profileTargetControllerIds,
      connectedControllerIds,
      selectedControllerId
    );
    if (
      nextTargets.length !== profileTargetControllerIds.length ||
      nextTargets.some((id, index) => id !== profileTargetControllerIds[index])
    ) {
      profileTargetControllerIds = nextTargets;
    }
  }
  $: profileTargetsAllConnected = profileTargetsCoverAllConnectedControllers(
    profileTargetControllerIds,
    connectedControllerIds
  );

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
  $: globalProfilePreview =
    profiles.find((profile) => profile.scope === 'Global') ??
    profiles.find((profile) => profile.scope === 'Built-in' && profile.id === 'forza-horizon-immersive') ??
    profiles.find((profile) => profile.scope === 'Built-in');
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
  $: activeProfileName = effectState?.selectedProfileName ?? status?.activeProfile ?? 'None';
  $: activeProfile = profiles.find((profile) => profile.id === activeProfileId);
  $: selectedOverrideProfile = profiles.find((profile) => profile.id === selectedOverrideProfileId);
  $: selectedActionProfile =
    profiles.find((profile) => profile.id === (selectedOverrideProfileId || activeProfileId)) ??
    activeProfile ??
    null;
  $: canDeleteSelectedProfile = Boolean(selectedActionProfile && !selectedActionProfile.builtIn);
  $: canRenameSelectedProfile = Boolean(selectedActionProfile && !selectedActionProfile.builtIn);
  $: controllerHeaderName = controllerModelText(controller);
  $: controllerHeaderMeta = controllerConnectionText(controller);
  $: controllerHeaderBatteryReadable = controllerBatteryReadable(controller);
  $: overrideActive = Boolean(snapshot?.profileResolution.overrideProfileId);
  $: detectedGameLabel = snapshot?.gameDetection.activeGameName ?? snapshot?.profileResolution.detectedGameId ?? 'current game';
  $: supportedGames = snapshot?.gameDetection.supportedGames ?? [];
  $: if (selectedTuningGameId && supportedGames.length && !supportedGames.some((game) => game.gameId === selectedTuningGameId)) {
    selectedTuningGameId = '';
    if (selectedTuningScope === 'game') selectedTuningScope = 'global';
  }
  $: selectedGame =
    snapshot?.gameDetection.selectedGame ??
    supportedGames.find((game) => game.gameId === snapshot?.gameDetection.activeGameId) ??
    null;
  $: discoveredGames = supportedGames
    .filter((game) => game.running || game.installed || game.gameId === selectedGame?.gameId)
    .sort((left, right) =>
      Number(right.running) - Number(left.running) ||
      Number(right.installed) - Number(left.installed) ||
      left.name.localeCompare(right.name)
    );
  $: selectedTuningGame = selectedTuningGameId
    ? supportedGames.find((game) => game.gameId === selectedTuningGameId) ?? null
    : null;
  $: if (selectedTuningScope !== 'game' && selectedTuningGameId) {
    selectedTuningGameId = '';
  }
  $: tuningReady = Boolean(controller && (selectedTuningScope === 'global' || selectedTuningGame));
  $: buttonMappingReady = Boolean(controller && selectedTuningScope === 'game' && selectedTuningGame);
  $: {
    const guardedView = guardView(activeView, { tuningReady, buttonMappingReady });
    if (guardedView !== activeView) {
      activeView = guardedView;
      setViewHash(guardedView);
    }
  }
  $: profileContextGame = selectedTuningScope === 'game' ? selectedTuningGame : null;
  $: profileContextGameId = profileContextGame?.gameId ?? null;
  $: profileContextLabel = selectedTuningScope === 'global' ? 'Global Profile' : profileContextGame?.name ?? detectedGameLabel;
  $: profileContextAssignment = assignmentForGame(profileContextGame, currentControllerConfig);
  $: profileContextDefaultProfileId =
    profileContextAssignment?.profileId ?? defaultProfileIdForGame(profileContextGame, profiles, activeProfileId, currentControllerConfig);
  $: profileContextDefaultProfile = profiles.find((profile) => profile.id === profileContextDefaultProfileId);
  $: profileContextProfiles = profilesForGame(
    profiles,
    profileContextGame,
    profileContextDefaultProfileId,
    selectedOverrideProfileId,
    activeProfileId
  );
  $: profileContextBadgeProfile = selectedOverrideProfile ?? profileContextProfiles[0] ?? activeProfile;
  $: activeProfileContextLabel =
    selectedTuningScope === 'global'
      ? 'global scope'
      : profileContextGame && profileContextBadgeProfile
        ? profileContextTag(profileContextBadgeProfile, profileContextGame, profileContextDefaultProfileId, activeProfileId)
        : 'game scope';
  $: profileContextDetail =
    selectedTuningScope === 'global'
      ? 'Controller-wide tuning'
      : profileContextGame
        ? [
        gameTileStatus(profileContextGame),
        formatPlaytime(profileContextGame.stats?.playtimeMinutes),
        achievementText(profileContextGame),
        profileContextDefaultProfile ? `${profileContextDefaultProfile.name} profile` : ''
      ]
        .filter(Boolean)
        .join(' / ')
        : overrideScope;
  $: detectionSignalText = gameDetectionStatusText(snapshot?.gameDetection);
  $: steamContextGame = profileContextGame;
  $: steamContextArt =
    gameArtwork(steamContextGame, 'capsule') ??
    gameArtwork(steamContextGame, 'banner') ??
    gameArtwork(steamContextGame, 'icon') ??
    '';
  $: steamContextBackdropArt =
    gameArtwork(steamContextGame, 'banner') ??
    gameArtwork(steamContextGame, 'hero') ??
    gameArtwork(steamContextGame, 'capsule') ??
    '';
  $: steamContextMeta = steamContextGame
    ? [
        gameProviderMeta(steamContextGame),
        formatPlaytime(steamContextGame.stats?.playtimeMinutes),
        achievementText(steamContextGame),
        formatLastPlayed(steamContextGame.stats?.lastPlayedUnix),
        gameTileStatus(steamContextGame)
      ]
        .filter(Boolean)
        .join(' / ')
    : selectedTuningScope === 'global'
      ? 'Controller-wide haptics'
      : detectionSignalText || 'Steam library data unavailable';
  $: activeProfileHeader = selectedActionProfile ?? profiles.find((profile) => profile.id === activeProfileId) ?? null;
  $: activeProfileHeaderName = activeProfileHeader?.name ?? activeProfileName ?? 'None';
  $: activeProfileHeaderMeta = (() => {
    if (!activeProfileHeader) return profiles.length ? 'No profile resolved' : 'Profiles loading';
    const scope = activeProfileHeader.builtIn && activeProfileHeader.scope === 'Built-in'
      ? 'Built-in template'
      : activeProfileHeader.builtIn
        ? 'Stock / Global'
      : activeProfileHeader.scope === 'Game'
        ? `Custom / ${steamContextGame?.name ?? 'game'}`
        : 'Custom / Global';
    const mode = activeProfileHeader.id === activeProfileId ? 'Live profile' : 'Editing profile';
    return profileConfigDirty ? `${scope} / ${mode} / unsaved changes` : `${scope} / ${mode}`;
  })();
  $: buttonMappingActive = activeView === 'buttonMapping';
  $: steamInputStatus = snapshot?.steamInput;
  $: inputBridgeStatus = snapshot?.inputBridge;
  $: bridgeContextActive =
    buttonMappingActive &&
    selectedTuningScope === 'game' &&
    steamContextGame?.inputProvider === 'dscc_input_bridge';
  $: mappingProviderLabel = bridgeContextActive ? 'DSCC Input Bridge' : 'Steam Input';
  $: mappingProviderOnline = bridgeContextActive
    ? Boolean(inputBridgeStatus?.available)
    : Boolean(steamInputStatus?.running);
  $: mappingAvailabilityMessage = bridgeContextActive
    ? inputBridgeStatus?.available
      ? 'Bridge edits are staged through DSCC typed mapping and sent through the configured virtual-output provider.'
      : inputBridgeStatus?.message ?? 'DSCC Input Bridge backend is unavailable.'
    : steamInputStatus?.available
      ? ''
      : steamInputStatus?.warnings?.[0] ?? 'Steam Input layout data is unavailable.';
  $: steamInputLayout = buttonMappingActive
    ? bridgeContextActive
      ? null
      : selectSteamInputLayout(steamInputStatus?.layouts ?? [], steamContextGame, controller?.family)
    : null;
  $: steamPaddlePresetVisible =
    buttonMappingActive && !bridgeContextActive && controller?.family === 'DualSense Edge' && selectedTuningScope === 'game';
  $: rawSteamInputBindings = buttonMappingActive
    ? steamInputLayout?.bindings ?? EMPTY_STEAM_INPUT_BINDINGS
    : EMPTY_STEAM_INPUT_BINDINGS;
  $: steamMappingContextKey = [
    buttonMappingActive ? steamInputLayout?.source ?? '' : '',
    buttonMappingActive ? steamContextGame?.gameId ?? '' : '',
    buttonMappingActive ? controller?.id ?? '' : '',
    buttonMappingActive ? controller?.family ?? '' : ''
  ].join('|');
  $: if (buttonMappingActive && steamMappingContextKey !== activeSteamMappingContextKey) {
    activeSteamMappingContextKey = steamMappingContextKey;
    optimisticSteamInputBindings = null;
    activeSteamSlotKey = '';
    hoveredSteamSlotKey = '';
  }
  $: realSteamInputBindings = buttonMappingActive
    ? optimisticSteamInputBindings ?? rawSteamInputBindings
    : EMPTY_STEAM_INPUT_BINDINGS;
  $: realSteamBindingBySlotKey = buttonMappingActive
    ? buildSteamBindingBySlotKey(realSteamInputBindings, steamBindingSlots)
    : EMPTY_STEAM_BINDING_MAP;
  $: steamPaddlePresetLeftBinding = steamPaddlePresetVisible
    ? realSteamBindingBySlotKey.get('edgeBackLeft') ?? null
    : null;
  $: steamPaddlePresetRightBinding = steamPaddlePresetVisible
    ? realSteamBindingBySlotKey.get('edgeBackRight') ?? null
    : null;
  $: steamPaddlePresetAvailable = Boolean(
    steamPaddlePresetVisible &&
    steamInputLayout &&
    steamPaddlePresetLeftBinding &&
    steamPaddlePresetRightBinding &&
    !steamPaddlePresetLeftBinding.synthetic &&
    !steamPaddlePresetRightBinding.synthetic
  );
  $: steamPaddlePresetStatus = !steamPaddlePresetVisible
    ? 'DualSense Edge controller required.'
    : !steamInputLayout
      ? 'Select a Steam game with a loaded Steam Input layout.'
      : !steamPaddlePresetAvailable
        ? 'Steam Input must expose both Edge back paddles before DSCC can write them.'
        : 'Writes Back Left and Back Right as Steam Input keyboard bindings for this game. This is PC-local and does not change onboard Edge memory.';
  $: defaultSteamBindingBySlotKey = buttonMappingActive
    ? defaultSteamBindingsForFamily(controller?.family)
    : EMPTY_STEAM_BINDING_MAP;
  $: steamBindingBySlotKey = buttonMappingActive
    ? new Map([...defaultSteamBindingBySlotKey, ...realSteamBindingBySlotKey])
    : EMPTY_STEAM_BINDING_MAP;
  $: steamInputBindings = buttonMappingActive
    ? [
        ...realSteamInputBindings,
        ...[...defaultSteamBindingBySlotKey.entries()]
          .filter(([slotKey]) => !realSteamBindingBySlotKey.has(slotKey))
          .map(([, binding]) => binding)
      ]
    : EMPTY_STEAM_INPUT_BINDINGS;
  $: if (
    buttonMappingActive &&
    steamInputBindings.length &&
    !activeSteamSlotKey &&
    !steamInputBindings.some((binding) => steamBindingKey(binding) === selectedSteamBindingKey)
  ) {
    selectedSteamBindingKey = steamBindingKey(steamInputBindings[0]);
  }
  $: if (buttonMappingActive && !steamInputBindings.length && selectedSteamBindingKey) {
    selectedSteamBindingKey = '';
  }
  $: selectedSteamBinding =
    buttonMappingActive && selectedSteamBindingKey
      ? steamInputBindings.find((binding) => steamBindingKey(binding) === selectedSteamBindingKey) ?? null
      : null;
  $: if (buttonMappingActive && selectedSteamBinding && steamBindingKey(selectedSteamBinding) !== lastSteamBindingDraftKey) {
    lastSteamBindingDraftKey = steamBindingKey(selectedSteamBinding);
    steamBindingDraft = selectedSteamBinding.rawBinding;
    steamBindingLabelDraft = parseSteamBindingTriple(selectedSteamBinding.rawBinding).label;
    clearSteamBindingMessage();
  }
  $: steamLayoutTitle = buttonMappingActive
    ? bridgeContextActive
      ? 'DSCC Bridge Xbox 360 Layout'
      : steamInputLayout?.title ?? 'Steam Input Layout'
    : 'Steam Input Layout';
  // Focused slot drives the controller-stage focus highlight. Hover wins, then
  // explicitly-clicked slot, then the slot owning the currently selected binding.
  $: focusedSlotKey = buttonMappingActive ? (() => {
    if (hoveredSteamSlotKey) return hoveredSteamSlotKey;
    if (activeSteamSlotKey) return activeSteamSlotKey;
    const fromBinding = steamBindingSlots.find((slot) => {
      const binding = steamBindingBySlotKey.get(slot.key);
      return Boolean(binding && steamBindingKey(binding) === selectedSteamBindingKey);
    });
    return fromBinding?.key ?? '';
  })() : '';
  $: focusedSlotMeta = focusedSlotKey
    ? steamBindingSlots.find((slot) => slot.key === focusedSlotKey) ?? null
    : null;
  // Materialised chip list joined with current slot/binding state. Edge chips
  // are hidden when the controller is not an Edge and nothing is mapped to them
  // yet — keeps the stage uncluttered for stock DualSense users.
  $: focusedSlotBinding = focusedSlotMeta ? steamBindingBySlotKey.get(focusedSlotMeta.key) ?? null : null;
  $: focusedSlotSelectedBinding =
    focusedSlotBinding && steamBindingKey(focusedSlotBinding) === selectedSteamBindingKey ? focusedSlotBinding : null;
  $: steamMirrorGroups = buttonMappingActive
    ? createSteamMirrorGroups({
        bindingBySlotKey: steamBindingBySlotKey,
        controllerFamily: controller?.family,
        selectedBindingKey: selectedSteamBindingKey,
        activeSlotKey: activeSteamSlotKey
      })
    : EMPTY_STEAM_MIRROR_GROUPS;
  $: mappedVisibleChipCount = steamMirrorGroups.reduce(
    (count, group) => count + group.rows.filter((row) => row.binding).length,
    0
  );
  $: telemetryPacketRate = adapter?.packetRateHz ?? 0;
  $: telemetryRateText = `${telemetryPacketRate >= 100 ? telemetryPacketRate.toFixed(0) : telemetryPacketRate.toFixed(1)} Hz`;
  $: telemetryRateDetail = telemetryRateStatusText(adapter);
  $: systemReadoutTitle = selectedTuningScope === 'global' ? 'Profile Scope' : 'Telemetry Rate';
  $: systemReadoutValue = selectedTuningScope === 'global' ? 'Global' : telemetryRateText;
  $: systemReadoutDetail =
    selectedTuningScope === 'global'
      ? 'Controller-only tuning'
      : telemetryRateDetail;
  $: overrideScope =
    controller && snapshot
      ? `${profileTargetSummary()} / ${profileContextLabel}`
      : profileContextLabel;
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
  $: if (profiles.length > 0 && !profiles.some((profile) => profile.id === selectedOverrideProfileId)) {
    selectedOverrideProfileId =
      profileContextDefaultProfileId ||
      activeProfileId ||
      snapshot?.profileResolution.overrideProfileId ||
      snapshot?.profileResolution.selectedProfileId ||
      profiles[0].id;
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

  type TriggerRangeEdge = 'from' | 'to';
  const appViewFromHash = (): AppView => {
    if (typeof window === 'undefined') return 'games';
    return viewFromHash(window.location.hash, { tuningReady, buttonMappingReady });
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
    view = guardView(view, { tuningReady, buttonMappingReady });
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

  const inputBridgeBindingProfileId = () => {
    if (!profileContextGameId) return null;
    const selectedProfileId = selectedOverrideProfileId || profileContextDefaultProfileId;
    const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId);
    if (
      selectedProfile &&
      !selectedProfile.builtIn &&
      selectedProfile.scope === 'Game' &&
      selectedProfile.gameId === profileContextGameId
    ) {
      return selectedProfile.id;
    }
    return profiles.find((profile) =>
      !profile.builtIn &&
      profile.scope === 'Game' &&
      profile.gameId === profileContextGameId
    )?.id ?? null;
  };

  // Update steamBindingDraft when one of the structured fields (target / label)
  // is edited, preserving the rest. Touching the raw VDF input still wins.
  const applySteamBindingTargetChange = (nextTargetRaw: string) => {
    const next = parseSteamBindingTriple(nextTargetRaw);
    const current = parseSteamBindingTriple(steamBindingDraft);
    steamBindingDraft = assembleSteamBindingRaw({
      command: next.command,
      param: next.param,
      icon: current.icon,
      label: current.label
    });
  };
  const applySteamBindingLabelChange = (nextLabel: string) => {
    steamBindingLabelDraft = nextLabel;
    const current = parseSteamBindingTriple(steamBindingDraft);
    steamBindingDraft = assembleSteamBindingRaw({
      ...current,
      label: nextLabel
    });
  };
  const syncSteamBindingLabelFromRaw = () => {
    steamBindingLabelDraft = parseSteamBindingTriple(steamBindingDraft).label;
  };

  const applySteamBindingRawChange = (nextRaw: string) => {
    steamBindingDraft = nextRaw;
    syncSteamBindingLabelFromRaw();
  };

  const clearSteamBindingMessage = () => {
    steamBindingMessage = '';
  };

  const setSteamBindingMessage = (message: string, tone: ToastTone = toastToneForMessage(message, 'info')) => {
    steamBindingMessage = message;
    showToast(message, tone);
  };

  const selectSteamBinding = (binding: SteamInputBinding | null | undefined) => {
    if (!binding) {
      setSteamBindingMessage('That Steam input is not present in the loaded layout yet.', 'info');
      return;
    }
    selectedSteamBindingKey = steamBindingKey(binding);
    lastSteamBindingDraftKey = selectedSteamBindingKey;
    steamBindingDraft = binding.rawBinding;
    steamBindingLabelDraft = parseSteamBindingTriple(binding.rawBinding).label;
    clearSteamBindingMessage();
  };

  const selectSteamSlot = (slot: SteamBindingSlot) => {
    activeSteamSlotKey = slot.key;
    const binding = steamBindingBySlotKey.get(slot.key) ?? null;
    if (binding) {
      selectSteamBinding(binding);
    } else {
      selectedSteamBindingKey = '';
      lastSteamBindingDraftKey = '';
      steamBindingDraft = '';
      steamBindingLabelDraft = '';
      setSteamBindingMessage(`${slot.label} has no Steam Input binding in this layout yet.`, 'info');
    }
  };

  const hoverSteamSlot = (slot: SteamBindingSlot | null) => {
    hoveredSteamSlotKey = slot?.key ?? '';
  };

  const applyOptimisticSteamBinding = (updatedBinding: SteamInputBinding) => {
    const updatedKey = steamBindingKey(updatedBinding);
    const baseBindings = optimisticSteamInputBindings ?? rawSteamInputBindings;
    let replaced = false;
    optimisticSteamInputBindings = baseBindings.map((binding) => {
      if (steamBindingKey(binding) !== updatedKey) return binding;
      replaced = true;
      return updatedBinding;
    });
    if (!replaced) optimisticSteamInputBindings = [...optimisticSteamInputBindings, updatedBinding];
  };

  const saveSteamBinding = async (dryRun = false) => {
    const bindingToSave = bridgeContextActive
      ? focusedSlotBinding ?? selectedSteamBinding
      : focusedSlotSelectedBinding ?? selectedSteamBinding;
    if (bridgeContextActive) {
      if (!bindingToSave) {
        setSteamBindingMessage('Select a bridge input before saving.', 'error');
        return;
      }
      if (!controller?.id) {
        setSteamBindingMessage('Select a controller before saving bridge bindings.', 'error');
        return;
      }
      if (!inputBridgeStatus?.available) {
        setSteamBindingMessage(inputBridgeStatus?.message ?? 'DSCC Input Bridge backend is unavailable.', 'error');
        return;
      }
      const rawBinding = steamBindingDraft.trim();
      if (!rawBinding) {
        setSteamBindingMessage('Choose a bridge target before saving.', 'error');
        return;
      }
      steamBindingBusy = true;
      setSteamBindingMessage(dryRun ? 'Validating DSCC Bridge binding...' : 'Saving DSCC Bridge binding...', 'info');
      try {
        const response = await writeInputBridgeBinding({
          controllerId: controller.id,
          profileId: inputBridgeBindingProfileId(),
          inputId: bindingToSave.inputId,
          target: rawBinding,
          dryRun
        });
        const warningText = response.warnings.length ? ` ${response.warnings.join(' ')}` : '';
        setSteamBindingMessage(`${response.message}${warningText}`, response.accepted ? 'success' : 'error');
        if (response.accepted && !dryRun) {
          const parsed = parseSteamBindingTriple(rawBinding);
          const updatedBinding: SteamInputBinding = {
            ...bindingToSave,
            rawBinding,
            binding: parsed.label || steamBindingTargetPart(rawBinding) || rawBinding,
            synthetic: false
          };
          selectedSteamBindingKey = steamBindingKey(updatedBinding);
          lastSteamBindingDraftKey = selectedSteamBindingKey;
          applyOptimisticSteamBinding(updatedBinding);
        }
      } catch (caught) {
        setSteamBindingMessage(caught instanceof Error ? caught.message : 'Unable to save DSCC Bridge binding.', 'error');
      } finally {
        steamBindingBusy = false;
      }
      return;
    }
    if (!steamInputLayout || !bindingToSave) {
      setSteamBindingMessage('Load a Steam Input layout and select a binding first.', 'error');
      return;
    }
    if (bindingToSave.synthetic) {
      setSteamBindingMessage('This input is using DSCC default mapping. Open or create a Steam Input layout for this game before saving a custom binding.', 'error');
      return;
    }
    const rawBinding = steamBindingDraft.trim();
    if (!rawBinding) {
      setSteamBindingMessage('Choose a target binding before saving.', 'error');
      return;
    }
    steamBindingBusy = true;
    setSteamBindingMessage(dryRun ? 'Validating Steam Input write...' : 'Saving Steam Input binding...', 'info');
    try {
      const response = await writeSteamInputBinding({
        layoutSource: steamInputLayout.source,
        appId: steamInputLayout.appId ?? steamContextGame?.appId ?? null,
        inputId: bindingToSave.inputId,
        groupId: bindingToSave.groupId ?? null,
        activator: bindingToSave.activator ?? null,
        rawBinding,
        profileName: activeProfileName || profileContextGame?.name || steamContextGame?.name || null,
        dryRun
      });
      setSteamBindingMessage(
        response.backupPath ? `${response.message} Backup: ${response.backupPath}` : response.message,
        'success'
      );
      selectedSteamBindingKey = steamBindingKey(response.binding);
      lastSteamBindingDraftKey = selectedSteamBindingKey;
      steamBindingDraft = response.binding.rawBinding;
      steamBindingLabelDraft = parseSteamBindingTriple(response.binding.rawBinding).label;
      if (!dryRun) {
        applyOptimisticSteamBinding(response.binding);
        void refresh().finally(() => {
          optimisticSteamInputBindings = null;
        });
      }
    } catch (caught) {
      setSteamBindingMessage(caught instanceof Error ? caught.message : 'Unable to write Steam Input binding.', 'error');
    } finally {
      steamBindingBusy = false;
    }
  };

  const normalizePaddlePresetKey = (value: string) =>
    value
      .trim()
      .replaceAll(' ', '_')
      .replaceAll('-', '_')
      .toUpperCase()
      .replace(/[^A-Z0-9_]/g, '')
      .slice(0, 32);

  const setPaddlePresetLeftKey = (value: string) => {
    paddlePresetLeftKey = normalizePaddlePresetKey(value);
  };

  const setPaddlePresetRightKey = (value: string) => {
    paddlePresetRightKey = normalizePaddlePresetKey(value);
  };

  const applySteamPaddlePreset = async (dryRun = false) => {
    if (!steamInputLayout) {
      setSteamBindingMessage('Load a Steam Input layout before applying the paddle preset.', 'error');
      return;
    }
    if (controller?.family !== 'DualSense Edge') {
      setSteamBindingMessage('Steam Input paddle presets require a DualSense Edge layout.', 'error');
      return;
    }
    if (!steamPaddlePresetAvailable) {
      setSteamBindingMessage(steamPaddlePresetStatus, 'error');
      return;
    }
    steamBindingBusy = true;
    setSteamBindingMessage(dryRun ? 'Validating Edge paddle preset...' : 'Saving Edge paddle preset...', 'info');
    try {
      const response = await writeSteamInputPaddlePreset({
        layoutSource: steamInputLayout.source,
        appId: steamInputLayout.appId ?? steamContextGame?.appId ?? null,
        leftKey: paddlePresetLeftKey || 'Q',
        rightKey: paddlePresetRightKey || 'E',
        profileName: activeProfileName || profileContextGame?.name || steamContextGame?.name || null,
        dryRun
      });
      const warningText = response.warnings.length ? ` ${response.warnings.join(' ')}` : '';
      setSteamBindingMessage(
        response.backupPath ? `${response.message} Backup: ${response.backupPath}${warningText}` : `${response.message}${warningText}`,
        'success'
      );
      if (!dryRun) {
        for (const paddle of response.paddles) {
          applyOptimisticSteamBinding(paddle.binding);
        }
        const selectedPaddle = response.paddles[0]?.binding;
        if (selectedPaddle) {
          selectedSteamBindingKey = steamBindingKey(selectedPaddle);
          lastSteamBindingDraftKey = selectedSteamBindingKey;
          steamBindingDraft = selectedPaddle.rawBinding;
          steamBindingLabelDraft = parseSteamBindingTriple(selectedPaddle.rawBinding).label;
        }
        void refresh().finally(() => {
          optimisticSteamInputBindings = null;
        });
      }
    } catch (caught) {
      setSteamBindingMessage(caught instanceof Error ? caught.message : 'Unable to save Edge paddle preset.', 'error');
    } finally {
      steamBindingBusy = false;
    }
  };

  const setTriggerRangeValue = (side: TriggerSide, edge: TriggerRangeEdge, rawValue: number | string) => {
    const value = normalizeTriggerPercent(rawValue);
    if (side === 'l2') {
      if (edge === 'from') {
        l2From = Math.min(value, l2To);
      } else {
        l2To = Math.max(value, l2From);
      }
    } else {
      if (edge === 'from') {
        r2From = Math.min(value, r2To);
      } else {
        r2To = Math.max(value, r2From);
      }
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
  const selectedProfileTargetIds = () =>
    resolveSelectedProfileTargetIds({
      profileTargetControllerIds,
      connectedControllerIds,
      controllerId: controller?.id
    });

  const profileTargetSummary = () => {
    const ids = selectedProfileTargetIds();
    return summarizeProfileTargets({
      targetIds: ids,
      controllers,
      connectedControllerIds
    });
  };

  const setAllProfileTargets = () => {
    if (!connectedControllerIds.length) return;
    profileTargetControllerIds = [...connectedControllerIds];
  };

  const setSingleProfileTargetController = (controllerId: string) => {
    if (!connectedControllerIds.includes(controllerId)) return;
    profileTargetControllerIds = [controllerId];
    selectedControllerId = controllerId;
    configLoadedFor = '';
    stopTriggerInputPolling();
  };

  const setProfileOverrideForTargets = async (profileId: string, gameId: string | null) => {
    const targetIds = selectedProfileTargetIds();
    let resolution = null;
    for (const targetId of targetIds) {
      resolution = await setProfileOverride({ controllerId: targetId, gameId, profileId });
    }
    return resolution ?? setProfileOverride({ controllerId: null, gameId, profileId });
  };

  const clearProfileOverrideForTargets = async (gameId: string | null) => {
    const targetIds = selectedProfileTargetIds();
    let resolution = null;
    for (const targetId of targetIds) {
      resolution = await clearProfileOverride({ controllerId: targetId, gameId });
    }
    return resolution ?? (gameId ? clearProfileOverride({ gameId }) : null);
  };

  const saveControllerConfigForProfileTargets = async (config: EditableControllerConfig) => {
    let selectedUpdate: ControllerConfiguration | null = null;
    for (const targetId of selectedProfileTargetIds()) {
      const updated = await saveControllerConfig(targetId, config);
      if (targetId === selectedControllerId) selectedUpdate = updated;
    }
    if (selectedUpdate) currentControllerConfig = selectedUpdate;
  };

  const selectTargetController = (controllerId: string) => {
    if (!controllerId || controllerId === selectedControllerId) return;
    selectedControllerId = controllerId;
    if (profileTargetControllerIds.length <= 1) profileTargetControllerIds = [controllerId];
    configLoadedFor = '';
    stopTriggerInputPolling();
  };

  const profileScopeCount = (game: SupportedGame) => countProfilesForGame(game, profiles);

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
    const profileId =
      profiles.find((profile) => profile.id === 'global')?.id ??
      profiles.find((profile) => profile.scope === 'Global')?.id ??
      profiles.find((profile) => profile.scope !== 'Game' && profile.id === activeProfileId)?.id ??
      profiles[0]?.id ??
      '';
    selectedOverrideProfileId = profileId;
    activeView = 'haptics';
    setViewHash('haptics');
    if (profileId) await selectProfileForScope(profileId, null, 'Global Profile');
  };

  const selectTuningGame = async (game: SupportedGame) => {
    selectedTuningScope = 'game';
    selectedTuningGameId = game.gameId;
    const preferredProfileId = defaultProfileIdForGame(game, profiles, activeProfileId, currentControllerConfig);
    if (preferredProfileId) selectedOverrideProfileId = preferredProfileId;
    activeView = 'haptics';
    setViewHash('haptics');
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

  const isEdgeController = () => controller?.family === 'DualSense Edge';

  const editableConfigFromController = (config: ControllerConfiguration): EditableControllerConfig =>
    createEditableConfigFromController(config, isEdgeController());

  const buildDefaultControllerConfig = (): EditableControllerConfig =>
    createDefaultControllerConfig({
      isEdge: isEdgeController(),
      defaultForzaEffects: defaultForzaEffects(),
      defaultForzaAbsTuning: defaultForzaAbsTuning(),
      defaultForzaThrottleTuning: defaultForzaThrottleTuning(),
      defaultForzaShiftTuning: defaultForzaShiftTuning(),
      defaultForzaRevLimiterTuning: defaultForzaRevLimiterTuning()
    });

  const profileConfigSignature = (config: EditableControllerConfig | ControllerConfiguration): string =>
    createProfileConfigSignature(config, {
      isEdge: isEdgeController(),
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

  const forzaEffect = (id: string): ForzaEffectConfiguration =>
    forzaEffects.find((effect) => effect.id === id) ??
    defaultForzaEffects().find((effect) => effect.id === id) ??
    defaultForzaEffects()[0];

  const updateForzaEffect = (id: string, patch: Partial<ForzaEffectConfiguration>) => {
    forzaEffects = normalizeForzaEffects(
      forzaEffects.map((effect) =>
        effect.id === id
          ? {
              ...effect,
              ...patch,
              intensity:
                patch.intensity === undefined ? effect.intensity : clampForzaIntensity(patch.intensity)
            }
          : effect
      )
    );
    scheduleLiveControllerConfigSync();
  };

  const applyShiftThumpPreset = (intensity: number) => {
    updateForzaEffect('gear_shift_thump', {
      enabled: intensity > 0,
      intensity,
      route: 'r2_and_body'
    });
  };

  const setAllForzaEffects = (enabled: boolean) => {
    forzaEffects = normalizeForzaEffects(forzaEffects.map((effect) => ({ ...effect, enabled })));
    scheduleLiveControllerConfigSync();
  };

  const setForzaBodyRumbleMode = (mode: ForzaBodyRumbleMode) => {
    forzaBodyRumbleMode = normalizeForzaBodyRumbleMode(mode);
    scheduleLiveControllerConfigSync();
  };

  const updateForzaAbsTuning = (patch: Partial<ForzaAbsTuningConfiguration>) => {
    forzaAbsTuning = normalizeForzaAbsTuning({
      ...forzaAbsTuning,
      ...patch
    });
    scheduleLiveControllerConfigSync();
  };

  const updateForzaThrottleTuning = (patch: Partial<ForzaThrottleTuningConfiguration>) => {
    forzaThrottleTuning = normalizeForzaThrottleTuning({
      ...forzaThrottleTuning,
      ...patch
    });
    scheduleLiveControllerConfigSync();
  };

  const updateForzaShiftTuning = (patch: Partial<ForzaShiftTuningConfiguration>) => {
    forzaShiftTuning = normalizeForzaShiftTuning({
      ...forzaShiftTuning,
      ...patch
    });
    scheduleLiveControllerConfigSync();
  };

  const updateForzaRevLimiterTuning = (patch: Partial<ForzaRevLimiterTuningConfiguration>) => {
    forzaRevLimiterTuning = normalizeForzaRevLimiterTuning({
      ...forzaRevLimiterTuning,
      ...patch
    });
    scheduleLiveControllerConfigSync();
  };

  const toggleAllForzaEffects = () => {
    setAllForzaEffects(!allForzaEffectsEnabled);
  };

  const telemetryUnitValue = (signal: string) => {
    const value = telemetryByName.get(signal)?.value;
    return typeof value === 'number' && Number.isFinite(value) ? clampUnit(value) : 0;
  };

  const triggerStrengthScalar = () => triggerStrengthScalarFor(triggerEffect, triggerIntensity);

  const triggerRangeValues = (side: TriggerSide) =>
    side === 'l2' ? triggerRangeValuesFor(l2From, l2To) : triggerRangeValuesFor(r2From, r2To);

  const triggerCurveValue = (side: TriggerSide, position: number) =>
    side === 'l2'
      ? triggerCurveValueFor(side, position, l2From, l2To, l2Curve, l2CurvePoints, defaultTriggerCurve('l2'), triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects, forzaThrottleTuning)
      : triggerCurveValueFor(side, position, r2From, r2To, r2Curve, r2CurvePoints, defaultTriggerCurve('r2'), triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects, forzaThrottleTuning);

  $: l2CurveShape = triggerCurveShapeView('l2', l2From, l2To, l2Curve, l2CurvePoints, defaultTriggerCurve('l2'), triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects, forzaThrottleTuning);
  $: r2CurveShape = triggerCurveShapeView('r2', r2From, r2To, r2Curve, r2CurvePoints, defaultTriggerCurve('r2'), triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects, forzaThrottleTuning);
  $: l2CurveLive = triggerCurveLiveView('l2', l2From, l2To, l2Curve, l2CurvePoints, defaultTriggerCurve('l2'), l2LivePress, triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects, forzaThrottleTuning);
  $: r2CurveLive = triggerCurveLiveView('r2', r2From, r2To, r2Curve, r2CurvePoints, defaultTriggerCurve('r2'), r2LivePress, triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects, forzaThrottleTuning);
  $: triggerRangeTooltipForCurrentTuning = (
    side: 'L2' | 'R2',
    edge: 'from' | 'to',
    value: number,
    startValue = 0
  ) => triggerRangeTooltip(side, edge, value, startValue, forzaThrottleTuning);

  const showTriggerPress = (_side: 'l2' | 'r2', value: number) =>
    baseFeelTestActive || clampUnit(value) > 0.01;
  const curveGraphPointFromPointer = (event: PointerEvent, target: HTMLElement) => {
    const rect = target.getBoundingClientRect();
    const x = clampUnit((event.clientX - rect.left) / Math.max(1, rect.width));
    const output = clampUnit(1 - (event.clientY - rect.top) / Math.max(1, rect.height));
    return { x, output };
  };

  const setCurveHover = (side: TriggerSide, x: number) => {
    const y = triggerCurveValue(side, x);
    curveHover = {
      side,
      x,
      y,
      left: x * 100,
      top: (1 - y) * 100
    };
  };

  const curvePointFromGraphPoint = (side: TriggerSide, input: number, output: number) => {
    const range = triggerRangeValues(side);
    const start = range.from / 100;
    const end = Math.max(start + 0.01, range.to / 100);
    let activeTravel = clamp((input - start) / (end - start), 0.01, 0.99);
    let normalizedOutput = output;

    if (triggerCurveDisplayMode === 'forza') {
      const model =
        side === 'l2'
          ? forzaTriggerForceModelFor(side, l2From, l2To, l2Curve, l2CurvePoints, defaultTriggerCurve(side), triggerEffect, triggerIntensity, forzaEffects, forzaThrottleTuning)
          : forzaTriggerForceModelFor(side, r2From, r2To, r2Curve, r2CurvePoints, defaultTriggerCurve(side), triggerEffect, triggerIntensity, forzaEffects, forzaThrottleTuning);
      if (model && model.normalForce > model.baselineForce) {
        const editableEnd = model.rampStart ?? model.wall;
        const editableInput = clamp(input, model.start + 0.0001, Math.max(model.start + 0.0001, editableEnd - 0.0001));
        activeTravel = clamp((editableInput - model.start) / (editableEnd - model.start), 0.01, 0.99);
        normalizedOutput = clamp((Math.min(output, model.normalForce) - model.baselineForce) / (model.normalForce - model.baselineForce), 0.01, 0.99);
      }
    } else {
      const strength = triggerStrengthScalar();
      normalizedOutput = clamp(strength > 0 ? output / strength : output, 0.01, 0.99);
    }

    return {
      input: normalizeTriggerPercent(activeTravel * 100),
      output: normalizeTriggerPercent(normalizedOutput * 100)
    };
  };

  const pointsForSide = (side: TriggerSide) => (side === 'l2' ? l2CurvePoints : r2CurvePoints);
  const setPointsForSide = (side: TriggerSide, points: TriggerCurvePoint[]) => {
    const normalized = normalizeTriggerCurvePoints(points, side === 'l2' ? l2Curve : r2Curve);
    if (side === 'l2') {
      l2CurvePoints = normalized;
    } else {
      r2CurvePoints = normalized;
    }
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  const setCurvePoint = (side: TriggerSide, index: number, point: TriggerCurvePoint) => {
    const current = normalizeTriggerCurvePoints(pointsForSide(side), side === 'l2' ? l2Curve : r2Curve);
    if (index <= 0 || index >= current.length - 1) return index;
    const previous = current[index - 1];
    const next = current[index + 1];
    current[index] = {
      input: normalizeTriggerPercent(clamp(point.input, previous.input + 1, next.input - 1)),
      output: normalizeTriggerPercent(point.output)
    };
    setPointsForSide(side, current);
    return index;
  };

  const addOrSelectCurvePoint = (side: TriggerSide, point: TriggerCurvePoint) => {
    const current = normalizeTriggerCurvePoints(pointsForSide(side), side === 'l2' ? l2Curve : r2Curve);
    if (current.length >= TRIGGER_CURVE_POINT_MAX) {
      let nearest = 1;
      let distance = Number.POSITIVE_INFINITY;
      for (let index = 1; index < current.length - 1; index += 1) {
        const nextDistance = Math.abs(current[index].input - point.input);
        if (nextDistance < distance) {
          distance = nextDistance;
          nearest = index;
        }
      }
      return setCurvePoint(side, nearest, point);
    }

    const nextPoints = [...current, point].sort((a, b) => a.input - b.input);
    const index = Math.max(1, Math.min(nextPoints.length - 2, nextPoints.findIndex((candidate) => candidate === point)));
    setPointsForSide(side, nextPoints);
    return index;
  };

  const addCurvePoint = (side: TriggerSide) => {
    const current = normalizeTriggerCurvePoints(pointsForSide(side), side === 'l2' ? l2Curve : r2Curve);
    if (current.length >= TRIGGER_CURVE_POINT_MAX) return;

    let bestIndex = 0;
    let bestGap = 0;
    for (let index = 0; index < current.length - 1; index += 1) {
      const gap = current[index + 1].input - current[index].input;
      if (gap > bestGap) {
        bestGap = gap;
        bestIndex = index;
      }
    }
    const left = current[bestIndex];
    const right = current[bestIndex + 1];
    const input = normalizeTriggerPercent((left.input + right.input) / 2);
    const output = normalizeTriggerPercent((left.output + right.output) / 2);
    setPointsForSide(side, [...current, { input, output }]);
  };

  const removeCurvePoint = (side: TriggerSide) => {
    const current = normalizeTriggerCurvePoints(pointsForSide(side), side === 'l2' ? l2Curve : r2Curve);
    if (current.length <= TRIGGER_CURVE_POINT_MIN) return;

    let removeIndex = current.length - 2;
    let smallestBend = Number.POSITIVE_INFINITY;
    for (let index = 1; index < current.length - 1; index += 1) {
      const left = current[index - 1];
      const point = current[index];
      const right = current[index + 1];
      const expected = left.output + ((right.output - left.output) * (point.input - left.input)) / Math.max(1, right.input - left.input);
      const bend = Math.abs(point.output - expected);
      if (bend < smallestBend) {
        smallestBend = bend;
        removeIndex = index;
      }
    }
    setPointsForSide(side, current.filter((_, index) => index !== removeIndex));
  };

  const updateCurveHover = (event: PointerEvent, side: TriggerSide) => {
    const target = event.currentTarget as HTMLElement;
    const { x } = curveGraphPointFromPointer(event, target);
    setCurveHover(side, x);
  };

  const handleCurvePointer = (event: PointerEvent, side: TriggerSide) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    event.preventDefault();

    const target = event.currentTarget as HTMLElement;
    curveDragSide = side;
    target.setPointerCapture(event.pointerId);
    let pointIndex = -1;

    const applyPoint = (pointerEvent: PointerEvent) => {
      const { x, output } = curveGraphPointFromPointer(pointerEvent, target);
      const point = curvePointFromGraphPoint(side, x, output);
      pointIndex = pointIndex < 0 ? addOrSelectCurvePoint(side, point) : setCurvePoint(side, pointIndex, point);
      curveDragPoint = { side, index: pointIndex };
      setCurveHover(side, x);
    };

    const stopDrag = () => {
      curveDragSide = null;
      curveDragPoint = null;
      if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
      target.removeEventListener('pointermove', applyPoint);
      target.removeEventListener('pointerup', stopDrag);
      target.removeEventListener('pointercancel', stopDrag);
    };

    applyPoint(event);
    target.addEventListener('pointermove', applyPoint);
    target.addEventListener('pointerup', stopDrag);
    target.addEventListener('pointercancel', stopDrag);
  };

  const handleCurvePointPointer = (event: PointerEvent, side: TriggerSide, index: number) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const frame = (event.currentTarget as SVGElement).closest('.dm-curve-frame') as HTMLElement | null;
    if (!frame) return;
    curveDragSide = side;
    curveDragPoint = { side, index };
    frame.setPointerCapture(event.pointerId);

    const applyPoint = (pointerEvent: PointerEvent) => {
      const { x, output } = curveGraphPointFromPointer(pointerEvent, frame);
      setCurvePoint(side, index, curvePointFromGraphPoint(side, x, output));
      setCurveHover(side, x);
    };
    const stopDrag = () => {
      curveDragSide = null;
      curveDragPoint = null;
      if (frame.hasPointerCapture(event.pointerId)) frame.releasePointerCapture(event.pointerId);
      frame.removeEventListener('pointermove', applyPoint);
      frame.removeEventListener('pointerup', stopDrag);
      frame.removeEventListener('pointercancel', stopDrag);
    };

    frame.addEventListener('pointermove', applyPoint);
    frame.addEventListener('pointerup', stopDrag);
    frame.addEventListener('pointercancel', stopDrag);
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
    vibrationMode = config.trigger.vibrationMode ?? 'Balanced';
    lightbarEnabled = config.lightbar?.enabled ?? true;
    lightbarColor = config.lightbar?.color ?? '#4cc9f0';
    rpmColor = config.lightbar?.rpmColor ?? '#ff3a2e';
    lightbarBrightness = config.lightbar?.brightness ?? 72;
    leftStickDeadzone = normalizeStickDeadzone(config.sticks?.leftDeadzone ?? 0);
    rightStickDeadzone = normalizeStickDeadzone(config.sticks?.rightDeadzone ?? 0);
    forzaBodyRumbleMode = normalizeForzaBodyRumbleMode(config.forza?.bodyRumbleMode);
    forzaEffects = normalizeForzaEffects(config.forza?.effects);
    forzaAbsTuning = normalizeForzaAbsTuning(config.forza?.abs);
    forzaThrottleTuning = normalizeForzaThrottleTuning(config.forza?.throttle);
    forzaShiftTuning = normalizeForzaShiftTuning(config.forza?.shift);
    forzaRevLimiterTuning = normalizeForzaRevLimiterTuning(config.forza?.revLimiter);
  };
  const applyControllerConfig = (config: ControllerConfiguration, updateProfileBaseline = true) => {
    currentControllerConfig = config;
    applyEditableConfig(config);
    if (updateProfileBaseline) profileSaveBaselineSignature = profileConfigSignature(buildControllerConfig());
  };

  const loadControllerConfig = async (controllerId: string) => {
    configLoadedFor = controllerId;
    configLoadError = '';
    currentControllerConfig = null;
    profileSaveBaselineSignature = '';
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
    if (!force && edgeProfilesLoadedFor === controllerId && (edgeProfiles || edgeProfilesLoading)) return;
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
    edgeProfilesLoadedFor = '';
    edgeProfiles = null;
    edgeProfilesLoading = false;
    edgeProfilesBusySlot = '';
    edgeProfilesError = '';
  };

  const edgeSlotStatus = (slot: EdgeProfileSlot) => {
    if (slot.state === 'default') return 'default';
    if (slot.hardwareSynced) return 'on controller';
    if (slot.staged) return 'staged';
    return slot.state.replaceAll('_', ' ');
  };

  const edgeSlotName = (slot: EdgeProfileSlot) => slot.name || slot.staged?.name || 'Empty Slot';
  const edgeSlotsReadTooltip =
    'Reads onboard slots from a DualSense Edge over USB or Bluetooth when Windows exposes HID feature-report access. Fallback controllers only show local staged status.';
  const edgeSlotInfoTooltip = (slot: EdgeProfileSlot) => {
    if (slot.state === 'default') {
      return 'The Fn + Triangle default profile is readable but not writable from DSCC.';
    }
    if (slot.hardwareSynced) {
      return `${slot.shortcut} is currently synced with controller memory.`;
    }
    if (slot.staged) {
      return `${slot.shortcut} has local staged settings that still need a controller hardware write.`;
    }
    return `${slot.shortcut} has no synced profile data available yet. Connect over USB or Bluetooth and read slots to refresh controller memory state.`;
  };
  const edgeSlotWriteTooltip = (slot: EdgeProfileSlot) =>
    edgeProfiles?.supportState === 'read_write'
      ? `Writes the current trigger ranges, lightbar color, stick presets, and supported button remaps to ${slot.shortcut}. Live telemetry effects still require DSCC to be running.`
      : `Stages the current trigger ranges, lightbar color, stick presets, and supported button remaps for ${slot.shortcut}. Connect the DualSense Edge over USB or Bluetooth, then read slots again to sync controller memory.`;
  const edgeSlotWriteLabel = () => (edgeProfiles?.supportState === 'read_write' ? 'Write' : 'Stage');

  const edgeProfileNameForSlot = (slot: EdgeProfileSlot) => {
    const profileName = activeProfileHeaderName || activeProfile?.name || 'DSCC Profile';
    return `${profileName} ${slot.shortcut.replace('Fn + ', '')}`.trim().slice(0, 64);
  };

  const writeCurrentConfigToEdgeSlot = async (slot: EdgeProfileSlot) => {
    if (!controller || !slot.editable || edgeProfilesBusySlot) return;
    edgeProfilesBusySlot = slot.slotId;
    edgeProfilesError = '';
    try {
      const config = buildControllerConfig();
      const response = await writeEdgeProfile(controller.id, slot.slotId, {
        name: edgeProfileNameForSlot(slot),
        trigger: config.trigger,
        lightbar: config.lightbar,
        sticks: config.sticks,
        buttons: config.buttons
      });
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
    profileSaveBaselineSignature = profileConfigSignature(buildControllerConfig());
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
    vibrationMode = trigger.vibrationMode ?? 'Balanced';
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
    forzaBodyRumbleMode,
    forzaEffects,
    forzaAbsTuning,
    forzaThrottleTuning,
    forzaShiftTuning,
    forzaRevLimiterTuning,
    leftStickDeadzone,
    rightStickDeadzone
  });

  const buildControllerConfig = (): EditableControllerConfig => {
    const base = currentControllerConfig
      ? editableConfigFromController(currentControllerConfig)
      : buildDefaultControllerConfig();

    return buildControllerConfigDraft(base, currentProfileDraftValues(), {
      isEdge: isEdgeController(),
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

  const activateProfileById = async (id: string) => {
    // Optimistic UI update so rapid clicks feel instant: flip the active flag
    // locally and align the dropdown BEFORE the server round-trip resolves.
    if (snapshot) {
      snapshot = {
        ...snapshot,
        profiles: snapshot.profiles.map((profile) => ({ ...profile, active: profile.id === id }))
      };
    }
    selectedOverrideProfileId = id;
    lastSyncedActiveProfileId = id;
    try {
      await activateProfile(id);
      // After activation, reload the active controller's config so the
      // Forza effect table reflects the profile's preset values immediately.
      if (controller?.id) {
        configLoadedFor = '';
        await loadControllerConfig(controller.id);
      }
      await refresh();
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Failed to activate profile');
      // On failure, force a refresh so the UI snaps back to server truth.
      await refresh();
    }
  };

  const createProfileFromInput = async () => {
    const name = newProfileName.trim();
    if (!name) return;
    try {
      await createProfile(name, { gameId: selectedTuningScope === 'game' ? profileContextGameId : null });
      newProfileName = '';
      await refresh();
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Failed to create profile');
    }
  };

  const beginRenameSelectedProfile = () => {
    if (!selectedActionProfile || selectedActionProfile.builtIn) return;
    saveAsProfileOpen = false;
    saveAsProfileName = '';
    renameProfileId = selectedActionProfile.id;
    renameProfileName = selectedActionProfile.name;
  };

  const cancelRenameProfile = () => {
    renameProfileId = '';
    renameProfileName = '';
  };

  const submitRenameProfile = async () => {
    const profile = profiles.find((item) => item.id === renameProfileId);
    const name = renameProfileName.trim();
    if (!profile || profile.builtIn) {
      cancelRenameProfile();
      return;
    }
    if (!name) {
      setApplyMessage('Profile name cannot be empty', 'error');
      return;
    }
    if (name === profile.name) {
      cancelRenameProfile();
      return;
    }
    if (profiles.some((item) => item.id !== profile.id && item.name.trim().toLowerCase() === name.toLowerCase())) {
      setApplyMessage('A profile with that name already exists', 'error');
      return;
    }

    profileRenameBusy = true;
    try {
      const renamed = await renameProfile(profile.id, name);
      if (snapshot) {
        snapshot = {
          ...snapshot,
          profiles: snapshot.profiles.map((item) => (item.id === renamed.id ? { ...item, name: renamed.name } : item))
        };
      }
      cancelRenameProfile();
      await refresh();
      setApplyMessage(`Renamed profile to ${renamed.name}`, 'success');
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to rename profile', 'error');
      await refresh();
    } finally {
      profileRenameBusy = false;
    }
  };

  const handleRenameProfileKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submitRenameProfile();
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      cancelRenameProfile();
    }
  };

  const beginSaveAsProfile = () => {
    if (!selectedActionProfile) {
      setApplyMessage('No profile selected', 'error');
      return;
    }
    cancelRenameProfile();
    saveAsProfileName = uniqueProfileName(`${selectedActionProfile.name} copy`, profiles);
    saveAsProfileOpen = true;
  };

  const cancelSaveAsProfile = () => {
    saveAsProfileOpen = false;
    saveAsProfileName = '';
  };

  const submitSaveAsProfile = async () => {
    const name = saveAsProfileName.trim();
    if (!selectedActionProfile || profileSaveAsBusy) {
      if (!selectedActionProfile) setApplyMessage('No profile selected', 'error');
      return;
    }
    if (!name) {
      setApplyMessage('Profile name cannot be empty', 'error');
      return;
    }
    if (profiles.some((profile) => profile.name.trim().toLowerCase() === name.toLowerCase())) {
      setApplyMessage('A profile with that name already exists', 'error');
      return;
    }

    profileSaveAsBusy = true;
    try {
      const config = buildControllerConfig();
      const created = await createProfile(name, { gameId: selectedTuningScope === 'game' ? profileContextGameId : null });
      const response = await saveProfileConfig(created.id, config);
      await saveControllerConfigForProfileTargets(config);
      const resolution = await setProfileOverrideForTargets(created.id, profileContextGameId);
      if (snapshot && resolution) snapshot = { ...snapshot, profileResolution: resolution };
      profileSaveBaselineSignature = profileConfigSignature(config);
      selectedOverrideProfileId = created.id;
      cancelSaveAsProfile();
      await refresh();
      selectedOverrideProfileId = created.id;
      setApplyMessage(response.message || `Saved ${created.name}`, 'success');
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to save profile copy', 'error');
      await refresh();
    } finally {
      profileSaveAsBusy = false;
    }
  };

  const handleSaveAsProfileKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submitSaveAsProfile();
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      cancelSaveAsProfile();
    }
  };

  const deleteProfileById = async (id: string, name: string) => {
    const fallbackProfileId =
      profiles.find((profile) => profile.id === 'global')?.id ??
      profiles.find((profile) => profile.id !== id && profile.scope === 'Built-in')?.id ??
      profiles.find((profile) => profile.id !== id)?.id ??
      '';
    if (renameProfileId === id) cancelRenameProfile();
    profileFileBusy = true;
    try {
      if (snapshot) {
        snapshot = {
          ...snapshot,
          profiles: snapshot.profiles.filter((profile) => profile.id !== id)
        };
      }
      const response = await deleteProfile(id);
      await refresh();
      if (selectedOverrideProfileId === id) selectedOverrideProfileId = fallbackProfileId;
      setApplyMessage(response?.message ?? `Deleted ${name}`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Failed to delete profile');
      await refresh();
    } finally {
      profileFileBusy = false;
    }
  };

  const telemetryRateStatusText = (item: AppSnapshot['adapters'][number] | undefined) => {
    if (!item) return 'no active stream';
    if (item.state === 'running') return `${item.name} / live packets`;
    if (item.state === 'needs_setup') return `${item.name} / waiting for UDP`;
    if (item.state === 'ready') return `${item.name} / listening`;
    if (item.state === 'faulted') return `${item.name} / blocked`;
    return item.name;
  };

  const exportSelectedProfile = async () => {
    const profileId = selectedOverrideProfileId || activeProfileId;
    if (!profileId || profileFileBusy) {
      if (!profileId) setApplyMessage('Select a profile to export');
      return;
    }
    profileFileBusy = true;
    try {
      const exported = await exportProfile(profileId);
      const body = JSON.stringify(exported, null, 2);
      const url = URL.createObjectURL(new Blob([body], { type: 'application/json' }));
      const link = document.createElement('a');
      link.href = url;
      link.download = `${sanitizeFileName(exported.name)}.dscc-profile.json`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      setApplyMessage(`Exported ${exported.name}`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to export profile');
    } finally {
      profileFileBusy = false;
    }
  };

  const handleProfileImport = async (event: Event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file || profileFileBusy) return;

    profileFileBusy = true;
    try {
      const payload = profileImportPayload(JSON.parse(await file.text()), profiles);
      const imported = await importProfile(payload);
      selectedOverrideProfileId = imported.id;
      await refresh();
      setApplyMessage(`Imported ${imported.name}`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to import profile');
    } finally {
      profileFileBusy = false;
    }
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
        activeView === 'haptics' &&
        typeof window !== 'undefined' &&
        typeof document !== 'undefined' &&
        !document.hidden
    );
  }

  function syncTriggerInputPolling() {
    triggerInputPoller.sync();
  }

  $: if (
    typeof window !== 'undefined' &&
    (window.location.hash === '#/controllers' ||
      window.location.hash === '#/adaptive-triggers-haptics' ||
      window.location.hash === '#/button-mapping')
  ) {
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
      const result = await runEffectTest(baseFeelTestRequest(), controller?.id);

      snapshot = {
        ...snapshot,
        effectState: {
          ...snapshot.effectState,
          output: result.output
        }
      };
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
      const result = await runEffectTest(
        {
          target: 'base_feel',
          mode: 'off',
          intensity: 0,
          durationMs: 100
        },
        controller?.id
      );
      snapshot = {
        ...snapshot,
        effectState: {
          ...snapshot.effectState,
          output: result.output
        }
      };
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
      const result = await runEffectTest(
        {
          target: 'rumble',
          mode: vibrationModeRequest(vibrationMode),
          intensity,
          durationMs: 900
        },
        controller?.id
      );
      snapshot = {
        ...snapshot,
        effectState: {
          ...snapshot.effectState,
          output: result.output
        }
      };
      setApplyMessage(`${vibrationMode} body haptics previewed`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Body haptics preview failed');
    }
  };

  const saveActiveProfile = async () => {
    if (!selectedActionProfile || profileSaveBusy) {
      if (!selectedActionProfile) setApplyMessage('No profile selected');
      return;
    }

    profileSaveBusy = true;
    try {
      const sourceProfileName = selectedActionProfile.name;
      let targetProfile = selectedActionProfile;
      let preservingStockProfile = false;
      if (targetProfile?.builtIn) {
        const name = uniqueProfileName(
          profileContextGame ? `${profileContextGame.name} ${targetProfile.name} custom` : `${targetProfile.name} custom`,
          profiles
        );
        targetProfile = await createProfile(name, { gameId: profileContextGameId });
        preservingStockProfile = true;
      }
      if (!targetProfile) throw new Error('No profile selected');

      const config = buildControllerConfig();
      await saveControllerConfigForProfileTargets(config);
      const response = await saveProfileConfig(targetProfile.id, config);
      profileSaveBaselineSignature = profileConfigSignature(config);
      const resolution = await setProfileOverrideForTargets(targetProfile.id, profileContextGameId);
      if (snapshot && resolution) snapshot = { ...snapshot, profileResolution: resolution };
      selectedOverrideProfileId = targetProfile.id;
      await refresh();
      selectedOverrideProfileId = targetProfile.id;
      setApplyMessage(
        preservingStockProfile
          ? `Saved ${targetProfile.name}; stock ${sourceProfileName} preserved`
          : response.message || `Saved ${targetProfile.name}`
      );
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to save profile');
    } finally {
      profileSaveBusy = false;
    }
  };

  const previewLightbarColor = async (color: string, label: string) => {
    // /test-effect takes parameters in the request body, so preview first
    // and only persist the config if the preview is accepted by the agent.
    if (!snapshot) return;

    const intensity = lightbarEnabled ? lightbarBrightness : 0;
    try {
      const result = await runEffectTest(
        {
          target: 'lightbar',
          mode: color,
          intensity,
          durationMs: 650
        },
        controller?.id
      );

      snapshot = {
        ...snapshot,
        effectState: {
          ...snapshot.effectState,
          output: result.output
        }
      };
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
  const previewRpmColor = async () => previewLightbarColor(rpmColor, 'Redline Blink');

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
  $: if (controller?.id && activeView === 'haptics') {
    startTriggerInputPolling();
  } else {
    stopTriggerInputPolling();
  }

  $: if (controller?.id && controller.id !== configLoadedFor) {
    void loadControllerConfig(controller.id);
  }

  $: if (controller?.id && controller.family === 'DualSense Edge') {
    void loadEdgeProfiles(controller.id);
  } else if (edgeProfilesLoadedFor || edgeProfiles) {
    resetEdgeProfiles();
  }
</script>

<main class="ops-shell">
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
    <header class="dm-hud" aria-label="Global command state">
      <div class="dm-hardware-state">
        <span class="dm-controller-glyph" aria-hidden="true"></span>
        <div>
          <h1>DualSense Command Center</h1>
          <p><span class="dm-app-tagline">Adaptive triggers, haptics, and live telemetry &mdash; tuned locally.</span></p>
        </div>
      </div>

      <ViewNav
        views={appViews}
        {activeView}
        tooltips={viewTooltips}
        {tuningReady}
        {buttonMappingReady}
        onNavigate={navigateToView}
      />

      <div class="dm-system-cluster">
        <div class="dm-system-readout" title={selectedTuningScope === 'global' ? systemReadoutDetail : adapter?.setupHint ?? telemetryRateDetail}>
          <span>{systemReadoutTitle}</span>
          <strong>{systemReadoutValue}</strong>
          <small>{systemReadoutDetail}</small>
        </div>
        <Tooltip text="Open the quick start guide again. It explains Profiles, trigger tests, telemetry safety, and support bundles." side="bottom" align="end">
          <button
            class="dm-support-trigger"
            type="button"
            aria-label="Open quick start guide"
            onclick={openOnboarding}
          >
            <CircleHelp size={14} /> Guide
          </button>
        </Tooltip>
        <Tooltip text="Copy or export a sanitized support bundle for GitHub issues or Discord help. Raw hardware ids are excluded." side="bottom" align="end">
          <button
            class:active={supportPanelOpen}
            class="dm-support-trigger"
            type="button"
            aria-expanded={supportPanelOpen}
            aria-controls="support-bundle-panel"
            onclick={() => {
              supportPanelOpen = !supportPanelOpen;
            }}
          >
            <LifeBuoy size={14} /> Support
          </button>
        </Tooltip>
      </div>
    </header>

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

    {#if activeView === 'games' || (!tuningReady && activeView !== 'controllers')}
      <GamesView
        {controller}
        {connectedControllers}
        {selectedTuningScope}
        {selectedTuningGameId}
        {globalProfilePreview}
        {profileTargetsAllConnected}
        {profileTargetControllerIds}
        {discoveredGames}
        {detectionSignalText}
        {gameArtwork}
        {gameMediaDetails}
        {profileScopeCount}
        {gameAccentColor}
        {gameTileStatus}
        onSelectGlobal={selectGlobalTuning}
        onSelectGame={selectTuningGame}
        onOpenAddGame={openAddGameDialog}
        onPickAllControllers={pickAllControllers}
        onPickControllerTarget={pickControllerTarget}
      />
    {:else}
      <ContextRibbon
        {controller}
        {connectedControllers}
        {connectedControllerIds}
        {profileTargetsAllConnected}
        {profileTargetControllerIds}
        {selectedTuningScope}
        {selectedTuningGameId}
        {steamContextGame}
        {steamContextArt}
        {steamContextBackdropArt}
        {steamContextMeta}
        {discoveredGames}
        {profileContextProfiles}
        {selectedOverrideProfileId}
        {activeProfileId}
        {activeProfileHeaderName}
        {activeProfileHeaderMeta}
        {listenOnAllInterfaces}
        {appSettingsBusy}
        {lanRestartRequired}
        desiredBindAddress={appSettings?.desiredBindAddress}
        currentBindAddress={status?.bindAddress}
        {glyphOverrideEnabled}
        glyphStatus={forzaGlyphs?.lastStatus ?? glyphInstallPath}
        {gameAccentColor}
        onPickGlobal={selectGlobalTuning}
        onPickGame={selectTuningGame}
        onPickProfile={selectProfileForScope}
        onPickAllControllers={pickAllControllers}
        onPickController={pickControllerTarget}
        onUpdateLanAccess={updateLanAccess}
        onUpdateGlyphOverride={updateForzaGlyphOverride}
      />

      {#if activeView === 'controllers'}
        <ControllersView
          active={activeView === 'controllers'}
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
          {edgeProfiles}
          {edgeProfilesLoading}
          {edgeProfilesBusySlot}
          {edgeProfilesError}
          {edgeSlotsReadTooltip}
          edgeSlotWriteLabel={edgeSlotWriteLabel()}
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
          onRefreshEdgeProfiles={() => controller && void loadEdgeProfiles(controller.id, true)}
          onWriteEdgeSlot={writeCurrentConfigToEdgeSlot}
          {edgeSlotName}
          {edgeSlotStatus}
          {edgeSlotInfoTooltip}
          {edgeSlotWriteTooltip}
        />
      {/if}

      {#if activeView === 'haptics'}
      <HapticsView active>
      <TriggerCurvesPanel
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
        {setTriggerEffect}
        {setTriggerIntensity}
        {setVibrationIntensity}
        {setVibrationMode}
      />
      <HapticsAside
        {selectedTuningScope}
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
        {enabledForzaEffectCount}
        {allForzaEffectsEnabled}
        {forzaEffectMetas}
        {forzaEffectsById}
        {effectStatusById}
        {forzaBodyRumbleMode}
        {forzaAbsTuning}
        {forzaThrottleTuning}
        {forzaShiftTuning}
        {forzaRevLimiterTuning}
        {bodyRumbleModeOptions}
        {forzaRoutes}
        {forzaEffect}
        {toggleAllForzaEffects}
        {setForzaBodyRumbleMode}
        {updateForzaAbsTuning}
        {updateForzaThrottleTuning}
        {updateForzaShiftTuning}
        {updateForzaRevLimiterTuning}
        {updateForzaEffect}
        {intensityTooltip}
        {routeTooltip}
        {forzaIntensityPercent}
        {forzaIntensityFromPercent}
        {lightbarEnabled}
        bind:lightbarColor
        bind:rpmColor
        {lightbarBrightness}
        onColorChange={handleLightbarColorChange}
        {setLightbarBrightness}
        {setLightbarEnabled}
        {previewLightbar}
        {previewRpmColor}
        {profileContextProfiles}
        {activeProfileId}
        {selectedOverrideProfileId}
        {selectedActionProfile}
        selectedGameName={steamContextGame?.name ?? 'game'}
        {canRenameSelectedProfile}
        {canDeleteSelectedProfile}
        {profileConfigDirty}
        {profileSaveBusy}
        {profileFileBusy}
        {profileSaveAsBusy}
        {profileRenameBusy}
        {saveAsProfileOpen}
        bind:saveAsProfileName
        {renameProfileId}
        bind:renameProfileName
        onSelectProfile={selectProfileForScope}
        onImportFile={handleProfileImport}
        onExportProfile={exportSelectedProfile}
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
        onSaveProfile={saveActiveProfile}
      />
      </HapticsView>
      {/if}
    <ButtonMappingView
      active={activeView === 'buttonMapping'}
      steamInputRunning={Boolean(steamInputStatus?.running)}
      providerLabel={mappingProviderLabel}
      providerKind={bridgeContextActive ? 'bridge' : 'steam'}
      providerOnline={mappingProviderOnline}
      mappingAvailabilityMessage={mappingAvailabilityMessage}
      {controllerHeaderName}
      controllerTransport={controller?.transport}
      gameName={selectedTuningScope === 'global' ? 'Global Profile' : steamContextGame?.name ?? 'No supported game selected'}
      {steamLayoutTitle}
      {mappedVisibleChipCount}
      {steamMirrorGroups}
      {focusedSlotMeta}
      {focusedSlotBinding}
      {focusedSlotSelectedBinding}
      {steamBindingBusy}
      steamInputLayoutAvailable={bridgeContextActive ? Boolean(inputBridgeStatus?.available) : Boolean(steamInputLayout)}
      mappingReadOnly={bridgeContextActive ? !inputBridgeStatus?.available : !steamInputLayout}
      defaultMirrorOnly={buttonMappingActive && !bridgeContextActive && !steamInputLayout}
      paddlePresetVisible={steamPaddlePresetVisible}
      paddlePresetAvailable={steamPaddlePresetAvailable}
      paddlePresetStatus={steamPaddlePresetStatus}
      {paddlePresetLeftKey}
      {paddlePresetRightKey}
      {steamBindingDraft}
      {steamBindingLabelDraft}
      bindingLabelFieldLabel={bridgeContextActive ? 'Label' : 'Label (Steam UI)'}
      rawFieldLabel={bridgeContextActive ? 'Bridge mapping' : 'Raw VDF'}
      rawFieldPlaceholder={bridgeContextActive ? 'xinput_button a, , A' : 'xinput_button ... / key_press ...'}
      targetGroups={activeBindingTargetGroups}
      onSelectSlot={selectSteamSlot}
      onHoverSlot={hoverSteamSlot}
      onPaddlePresetLeftKeyChange={setPaddlePresetLeftKey}
      onPaddlePresetRightKeyChange={setPaddlePresetRightKey}
      onApplyPaddlePreset={() => void applySteamPaddlePreset(false)}
      onTargetChange={applySteamBindingTargetChange}
      onLabelChange={applySteamBindingLabelChange}
      onRawDraftChange={applySteamBindingRawChange}
      onResetDraft={resetSteamBindingDraft}
      onSaveBinding={() => void saveSteamBinding(false)}
    />
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
