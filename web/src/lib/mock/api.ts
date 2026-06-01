import type {
  AddCustomGameResponse,
  AddLocalAppRequest,
  AppSettingsResponse,
  ActionAccepted,
  AppSnapshot,
  ControllerConfiguration,
  ControllerInputState,
  ControllerOutputFrame,
  ControllerStatus,
  EffectTestRequest,
  ExportedProfile,
  ProfileResolution,
  ProfileSummary,
  SteamInputBinding,
  SteamInputBindingWriteRequest,
  SteamInputBindingWriteResponse,
  SteamLibraryBrowseEntry,
  SteamLibraryBrowseResponse,
  SteamLibraryEntry,
  SteamLibraryResponse,
  SupportedGame,
  InputBridgeBindingWriteRequest,
  InputBridgeBindingWriteResponse,
  InputBridgeStatus,
  ValidateLocalAppRequest,
  ValidateLocalAppResponse
} from '../types';
import {
  MOCK_CONTROLLER_ID,
  MOCK_EXPORT_SCHEMA,
  MOCK_GAME_ID,
  MOCK_LAYOUT_SOURCE,
  MOCK_PROFILE_ID,
  mockAppSnapshot,
  mockControllerConfig,
  mockInputBridgeStatus,
  mockProfileConfigs
} from './fixture';
import type { MockEditableControllerConfig } from './fixture';

const mockStartedAt = Date.now();
let profileCounter = 1;

type MockEffectTestResult = {
  accepted: true;
  message: string;
  dryRun: boolean;
  durationMs: number;
  output: ControllerOutputFrame;
};

type MockSnapshotSocketCallbacks = {
  onSnapshot?: (snapshot: AppSnapshot) => void;
  onInvalidate: () => void;
  onUnavailable?: () => void;
  onClosed?: () => void;
};

const state = {
  snapshot: clone(mockAppSnapshot),
  controllerConfigs: new Map<string, ControllerConfiguration>([[MOCK_CONTROLLER_ID, clone(mockControllerConfig)]]),
  profileConfigs: new Map<string, MockEditableControllerConfig>(
    Object.entries(mockProfileConfigs).map(([id, config]) => [id, clone(config)])
  )
};

export async function getMockAppSnapshot(): Promise<AppSnapshot> {
  updateMockTelemetry();
  return clone(state.snapshot);
}

export function connectMockAppSnapshotSocket(callbacks: MockSnapshotSocketCallbacks): () => void {
  if (typeof window === 'undefined') {
    callbacks.onSnapshot?.(clone(state.snapshot));
    return () => {};
  }

  let closed = false;
  const publish = () => {
    if (closed) return;
    void getMockAppSnapshot().then((snapshot) => callbacks.onSnapshot?.(snapshot));
  };

  publish();
  const timer = window.setInterval(publish, 1000);
  return () => {
    closed = true;
    window.clearInterval(timer);
  };
}

export async function saveMockAppSettings(request: {
  listenOnAllInterfaces?: boolean;
  forzaPlaystationGlyphs?: {
    enabled: boolean;
    installPath?: string | null;
  };
}): Promise<AppSettingsResponse> {
  const next = clone(state.snapshot.appSettings);
  if (request.listenOnAllInterfaces !== undefined) {
    next.settings.listenOnAllInterfaces = request.listenOnAllInterfaces;
    next.restartRequired = false;
  }
  if (request.forzaPlaystationGlyphs) {
    next.settings.forzaPlaystationGlyphs = {
      enabled: request.forzaPlaystationGlyphs.enabled,
      installPath: request.forzaPlaystationGlyphs.installPath ?? 'mock://steam/steamapps/common/ForzaHorizon6',
      lastStatus: request.forzaPlaystationGlyphs.enabled ? 'installed' : 'not_installed',
      lastMessage: request.forzaPlaystationGlyphs.enabled
        ? 'Mock PlayStation glyph override is enabled.'
        : 'Mock PlayStation glyph override is disabled.'
    };
  }
  state.snapshot.appSettings = next;
  return clone(next);
}

export async function runMockEffectTest(request: EffectTestRequest): Promise<MockEffectTestResult> {
  const output = outputForEffectRequest(request);
  state.snapshot.effectState = {
    ...state.snapshot.effectState,
    dryRun: true,
    hardwareOutputEnabled: false,
    output,
    warnings: ['Mock mode accepted the effect test without writing HID output.']
  };
  return {
    accepted: true,
    message: `${request.target} mock effect accepted`,
    dryRun: true,
    durationMs: request.durationMs,
    output: clone(output)
  };
}

export async function getMockControllerInput(controllerId?: string | null): Promise<ControllerInputState> {
  const seconds = (Date.now() - mockStartedAt) / 1000;
  const leftX = signedWave(seconds, 0.9, 0.12);
  const leftY = signedWave(seconds, 0.65, 1.6);
  const rightX = signedWave(seconds, 0.48, 2.4);
  const rightY = signedWave(seconds, 0.72, 3.1);
  const l2 = roundedUnit(wave(seconds, 0.16, 0.86, 0.25));
  const r2 = roundedUnit(wave(seconds, 0.08, 0.94, 1.1));
  const faceBeat = Math.floor(seconds * 2) % 4;
  return {
    controllerId: controllerId || MOCK_CONTROLLER_ID,
    available: true,
    source: 'mock',
    message: 'Mock controller input is synthesized in-browser.',
    sampledAtMs: Date.now(),
    ageMs: 0,
    axes: {
      leftStick: stickState(leftX, leftY),
      rightStick: stickState(rightX, rightY)
    },
    triggers: { l2, r2 },
    buttons: [
      mockButton('cross', 'Cross', faceBeat === 0),
      mockButton('circle', 'Circle', faceBeat === 1),
      mockButton('square', 'Square', faceBeat === 2),
      mockButton('triangle', 'Triangle', faceBeat === 3),
      mockButton('dpad_up', 'D-Pad Up', Math.sin(seconds * 1.7) > 0.72),
      mockButton('dpad_right', 'D-Pad Right', Math.sin(seconds * 1.3) > 0.72),
      mockButton('dpad_down', 'D-Pad Down', Math.sin(seconds * 1.7) < -0.72),
      mockButton('dpad_left', 'D-Pad Left', Math.sin(seconds * 1.3) < -0.72),
      mockButton('l1', 'L1', Math.sin(seconds * 1.1) > 0.78),
      mockButton('r1', 'R1', Math.cos(seconds * 1.1) > 0.78),
      { id: 'l2', label: 'L2', pressed: l2 > 0.5, value: l2 },
      { id: 'r2', label: 'R2', pressed: r2 > 0.5, value: r2 },
      mockButton('create', 'Create', false),
      mockButton('options', 'Options', false),
      mockButton('l3', 'L3', Math.hypot(leftX, leftY) > 0.84),
      mockButton('r3', 'R3', Math.hypot(rightX, rightY) > 0.84),
      mockButton('ps', 'PS', false),
      mockButton('touchpad', 'Touchpad', Math.sin(seconds * 0.8) > 0.88),
      mockButton('mute', 'Mute', false),
      mockButton('edge_fn_left', 'Fn Left', false),
      mockButton('edge_fn_right', 'Fn Right', false),
      mockButton('edge_back_left', 'Back Left', Math.sin(seconds * 0.95) > 0.84),
      mockButton('edge_back_right', 'Back Right', Math.cos(seconds * 0.95) > 0.84)
    ]
  };
}

export async function getMockControllerConfig(controllerId: string): Promise<ControllerConfiguration> {
  return clone(controllerConfigFor(controllerId));
}

export async function saveMockControllerConfig(
  controllerId: string,
  config: MockEditableControllerConfig
): Promise<ControllerConfiguration> {
  const updated: ControllerConfiguration = {
    ...clone(config),
    controllerId,
    model: controllerConfigFor(controllerId).model
  };
  state.controllerConfigs.set(controllerId, updated);
  syncEffectStateFromConfig(config);
  return clone(updated);
}

export async function renameMockController(controllerId: string, name: string): Promise<ControllerStatus> {
  const normalized = name.trim();
  if (!normalized) throw new Error('Controller name cannot be empty.');
  const controller = state.snapshot.controllers.find((item) => item.id === controllerId);
  if (!controller) throw new Error('Controller not found.');
  const updated = { ...controller, name: normalized.slice(0, 64) };
  state.snapshot.controllers = state.snapshot.controllers.map((item) => (item.id === controllerId ? updated : item));
  return clone(updated);
}

export async function saveMockProfileConfig(
  profileId: string,
  config: MockEditableControllerConfig
): Promise<ActionAccepted> {
  requireProfile(profileId);
  state.profileConfigs.set(profileId, clone(config));
  if (state.snapshot.profileResolution.selectedProfileId === profileId) {
    syncEffectStateFromConfig(config);
  }
  return { accepted: true, message: `Saved ${profileName(profileId)} in mock mode.` };
}

export async function setMockProfileOverride(request: {
  controllerId?: string | null;
  gameId?: string | null;
  profileId: string;
}): Promise<ProfileResolution> {
  activateProfileInState(request.profileId);
  state.snapshot.profileResolution = {
    ...state.snapshot.profileResolution,
    controllerId: request.controllerId ?? MOCK_CONTROLLER_ID,
    detectedGameId: request.gameId ?? MOCK_GAME_ID,
    selectedProfileId: request.profileId,
    overrideProfileId: request.profileId,
    reason: 'mock_manual_override',
    validation: 'ok'
  };
  return clone(state.snapshot.profileResolution);
}

export async function clearMockProfileOverride(request?: {
  controllerId?: string | null;
  gameId?: string | null;
}): Promise<ProfileResolution> {
  const fallbackProfileId = defaultProfileIdForGame(request?.gameId ?? MOCK_GAME_ID);
  activateProfileInState(fallbackProfileId);
  state.snapshot.profileResolution = {
    ...state.snapshot.profileResolution,
    controllerId: request?.controllerId ?? MOCK_CONTROLLER_ID,
    detectedGameId: request?.gameId ?? MOCK_GAME_ID,
    selectedProfileId: fallbackProfileId,
    overrideProfileId: null,
    reason: 'mock_game_detected',
    validation: 'ok'
  };
  return clone(state.snapshot.profileResolution);
}

export async function activateMockProfile(profileId: string): Promise<ActionAccepted> {
  activateProfileInState(profileId);
  state.snapshot.profileResolution = {
    ...state.snapshot.profileResolution,
    selectedProfileId: profileId,
    reason: 'mock_profile_activated',
    validation: 'ok'
  };
  return { accepted: true, message: `Activated ${profileName(profileId)} in mock mode.` };
}

export async function createMockProfile(name: string, options?: { gameId?: string | null }): Promise<ProfileSummary> {
  const gameId = normalizeProfileGameId(options?.gameId);
  const profile: ProfileSummary = {
    id: uniqueProfileId(name),
    name,
    builtIn: false,
    scope: gameId ? 'Game' : 'Global',
    gameId: gameId ?? 'all',
    active: false,
    rules: 0,
    updatedAt: 'mock'
  };
  state.snapshot.profiles = [...state.snapshot.profiles, profile];
  state.profileConfigs.set(profile.id, clone(editableConfigFromController(controllerConfigFor(MOCK_CONTROLLER_ID))));
  return clone(profile);
}

export async function renameMockProfile(profileId: string, name: string): Promise<ProfileSummary> {
  const profile = requireProfile(profileId);
  const updated = { ...profile, name, updatedAt: 'mock' };
  state.snapshot.profiles = state.snapshot.profiles.map((item) => (item.id === profileId ? updated : item));
  if (state.snapshot.profileResolution.selectedProfileId === profileId) {
    state.snapshot.status.activeProfile = name;
    state.snapshot.effectState.selectedProfileName = name;
  }
  state.snapshot.controllerProfileAssignments = state.snapshot.controllerProfileAssignments.map((assignment) =>
    assignment.profileId === profileId ? { ...assignment, profileName: name } : assignment
  );
  return clone(updated);
}

export async function exportMockProfile(profileId: string): Promise<ExportedProfile> {
  const profile = requireProfile(profileId);
  return {
    schema: MOCK_EXPORT_SCHEMA,
    id: profile.id,
    name: profile.name,
    built_in: profile.builtIn,
    builtIn: profile.builtIn,
    game_id: profile.scope === 'Game' ? profile.gameId : null,
    gameId: profile.scope === 'Game' ? profile.gameId : null,
    active: profile.active,
    config: state.profileConfigs.get(profileId) ?? editableConfigFromController(controllerConfigFor(MOCK_CONTROLLER_ID))
  };
}

export async function importMockProfile(profile: {
  schema: string;
  id?: string | null;
  name: string;
  game_id?: string | null;
  gameId?: string | null;
  config?: ExportedProfile['config'];
}): Promise<ProfileSummary> {
  if (profile.schema !== MOCK_EXPORT_SCHEMA) throw new Error('Unsupported profile schema.');
  const gameId = normalizeProfileGameId(profile.game_id ?? profile.gameId);
  const id = profile.id && !state.snapshot.profiles.some((item) => item.id === profile.id)
    ? profile.id
    : uniqueProfileId(profile.name);
  const imported: ProfileSummary = {
    id,
    name: profile.name,
    builtIn: false,
    scope: gameId ? 'Game' : 'Global',
    gameId: gameId ?? 'all',
    active: false,
    rules: 0,
    updatedAt: 'mock'
  };
  state.snapshot.profiles = [...state.snapshot.profiles, imported];
  state.profileConfigs.set(id, editableConfigFromExport(profile.config));
  return clone(imported);
}

export async function deleteMockProfile(profileId: string): Promise<ActionAccepted> {
  const profile = state.snapshot.profiles.find((item) => item.id === profileId);
  if (!profile) return { accepted: true, message: 'Profile was already deleted' };
  if (profile.builtIn) throw new Error('Built-in mock profiles cannot be deleted.');

  state.snapshot.profiles = state.snapshot.profiles.filter((item) => item.id !== profileId);
  state.profileConfigs.delete(profileId);
  if (profile.active || state.snapshot.profileResolution.selectedProfileId === profileId) {
    activateProfileInState(defaultProfileIdForGame(MOCK_GAME_ID));
  }
  return { accepted: true, message: `Deleted ${profile.name} in mock mode.` };
}

const MOCK_STEAM_LIBRARY: SteamLibraryEntry[] = [
  {
    appId: '1086940',
    name: 'Baldur’s Gate 3',
    installDir: 'BaldursGate3',
    installPath: 'C:/SteamLibrary/steamapps/common/BaldursGate3',
    artwork: {
      capsuleUrl: 'https://cdn.cloudflare.steamstatic.com/steam/apps/1086940/capsule_184x69.jpg',
      bannerUrl: 'https://cdn.cloudflare.steamstatic.com/steam/apps/1086940/header.jpg',
      heroUrl: 'https://cdn.cloudflare.steamstatic.com/steam/apps/1086940/library_hero.jpg'
    },
    stats: { playtimeMinutes: 4620, lastPlayedUnix: Math.floor(Date.now() / 1000) - 86400 * 4 },
    alreadyInCatalog: false,
    suggestedGameId: 'custom-1086940',
    processCandidates: ['bg3.exe', 'bg3_dx11.exe']
  },
  {
    appId: '292030',
    name: 'The Witcher 3: Wild Hunt',
    installDir: 'The Witcher 3',
    installPath: 'C:/SteamLibrary/steamapps/common/The Witcher 3',
    artwork: {
      capsuleUrl: 'https://cdn.cloudflare.steamstatic.com/steam/apps/292030/capsule_184x69.jpg',
      bannerUrl: 'https://cdn.cloudflare.steamstatic.com/steam/apps/292030/header.jpg'
    },
    stats: { playtimeMinutes: 13800, lastPlayedUnix: Math.floor(Date.now() / 1000) - 86400 * 30 },
    alreadyInCatalog: false,
    suggestedGameId: 'custom-292030',
    processCandidates: ['witcher3.exe']
  },
  {
    appId: '1551360',
    name: 'Forza Horizon 5',
    installDir: 'ForzaHorizon5',
    installPath: 'C:/SteamLibrary/steamapps/common/ForzaHorizon5',
    artwork: {
      capsuleUrl: 'https://cdn.cloudflare.steamstatic.com/steam/apps/1551360/capsule_184x69.jpg'
    },
    stats: { playtimeMinutes: 3960 },
    alreadyInCatalog: true,
    suggestedGameId: 'forza-horizon-5',
    processCandidates: ['ForzaHorizon5.exe']
  }
];

const mockCustomGames = new Map<string, SupportedGame>();

export async function getMockSteamLibrary(): Promise<SteamLibraryResponse> {
  return {
    games: MOCK_STEAM_LIBRARY.map((entry) => ({
      ...entry,
      alreadyInCatalog: entry.alreadyInCatalog || mockCustomGames.has(entry.suggestedGameId)
    }))
  };
}

export async function addMockCustomGame(
  appId: string,
  _processNames: string[] = []
): Promise<AddCustomGameResponse> {
  const entry = MOCK_STEAM_LIBRARY.find((item) => item.appId === appId);
  if (!entry) {
    throw new Error(`Steam app ${appId} is not installed in the mock library.`);
  }
  if (mockCustomGames.has(entry.suggestedGameId)) {
    throw new Error(`${entry.name} is already added.`);
  }
  const game: SupportedGame = {
    gameId: entry.suggestedGameId,
    name: entry.name,
    source: 'steam',
    inputProvider: 'steam_input',
    appId: entry.appId,
    installPath: entry.installPath,
    installed: true,
    running: false,
    supportLevel: 'custom',
    artwork: entry.artwork,
    stats: entry.stats
  };
  mockCustomGames.set(entry.suggestedGameId, game);
  const detection = state.snapshot.gameDetection;
  const supportedGames = [...(detection.supportedGames ?? []), game];
  state.snapshot = {
    ...state.snapshot,
    gameDetection: { ...detection, supportedGames }
  };
  return { game: clone(game) };
}

export async function getMockInputBridgeStatus(): Promise<InputBridgeStatus> {
  return clone(state.snapshot.inputBridge ?? mockInputBridgeStatus);
}

export async function writeMockInputBridgeBinding(
  request: InputBridgeBindingWriteRequest
): Promise<InputBridgeBindingWriteResponse> {
  if (!request.inputId.trim() || !request.target.trim()) {
    throw new Error('inputId and target are required');
  }
  return {
    accepted: true,
    message: request.dryRun
      ? `Validated mock DSCC Bridge binding for ${request.inputId}.`
      : `Staged mock DSCC Bridge binding for ${request.inputId}.`,
    dryRun: Boolean(request.dryRun),
    warnings: ['Mock mode does not write a real virtual controller.']
  };
}

export async function startMockInputBridgeSession(controllerId: string) {
  const session = {
    controllerId,
    state: 'active' as const,
    sessionId: `${controllerId}-mock-bridge`,
    outputKind: 'xbox360',
    message: 'Mock DSCC Input Bridge session is active.',
    updatedAtMs: Date.now()
  };
  state.snapshot = {
    ...state.snapshot,
    inputBridge: {
      ...state.snapshot.inputBridge,
      sessions: [
        ...state.snapshot.inputBridge.sessions.filter((item) => item.controllerId !== controllerId),
        session
      ]
    }
  };
  return clone(session);
}

export async function stopMockInputBridgeSession(controllerId: string) {
  const session = {
    controllerId,
    state: 'disabled' as const,
    sessionId: null,
    outputKind: null,
    message: 'Mock DSCC Input Bridge stopped and neutralized its output.',
    updatedAtMs: Date.now()
  };
  state.snapshot = {
    ...state.snapshot,
    inputBridge: {
      ...state.snapshot.inputBridge,
      sessions: state.snapshot.inputBridge.sessions.filter((item) => item.controllerId !== controllerId)
    }
  };
  return clone(session);
}

export async function validateMockLocalApp(
  request: ValidateLocalAppRequest
): Promise<ValidateLocalAppResponse> {
  const executableName = fileNameFromPath(request.executablePath || 'NightDriveLab.exe');
  if (!executableName.toLowerCase().endsWith('.exe')) {
    throw new Error('Local app path must point to a .exe file.');
  }
  return {
    valid: true,
    name: request.name?.trim() || executableName.replace(/\.exe$/i, ''),
    executableName,
    processNames: request.processNames?.length ? request.processNames : [executableName],
    warnings: []
  };
}

export async function addMockLocalApp(
  request: AddLocalAppRequest
): Promise<AddCustomGameResponse> {
  const validation = await validateMockLocalApp(request);
  const gameId = mockLocalGameId(validation.name, request.executablePath);
  const game: SupportedGame = {
    gameId,
    name: validation.name,
    source: 'local_app',
    inputProvider: 'dscc_input_bridge',
    appId: 'local:mock',
    installPath: null,
    processNames: validation.processNames,
    executableName: validation.executableName,
    installed: true,
    running: false,
    supportLevel: 'custom'
  };
  mockCustomGames.set(gameId, game);
  const detection = state.snapshot.gameDetection;
  const existing = detection.supportedGames ?? [];
  const supportedGames = existing.some((item) => item.gameId === gameId)
    ? existing.map((item) => (item.gameId === gameId ? game : item))
    : [...existing, game];
  state.snapshot = {
    ...state.snapshot,
    gameDetection: { ...detection, supportedGames }
  };
  return { game: clone(game) };
}

// A toy filesystem so the modal feels real in dev:mock mode.
const MOCK_BROWSE_TREE: Record<string, Record<string, SteamLibraryBrowseEntry[]>> = {
  '1086940': {
    '': [
      { name: 'Binaries', kind: 'dir' },
      { name: 'Data', kind: 'dir' },
      { name: 'bg3.exe', kind: 'exe', sizeBytes: 12_345_678 },
      { name: 'bg3_dx11.exe', kind: 'exe', sizeBytes: 10_222_334 }
    ],
    Binaries: [
      { name: 'Win64', kind: 'dir' }
    ],
    'Binaries/Win64': [
      { name: 'bg3-Shipping.exe', kind: 'exe', sizeBytes: 14_220_000 }
    ],
    Data: []
  }
};

export async function browseMockSteamLibrary(
  appId: string,
  path = ''
): Promise<SteamLibraryBrowseResponse> {
  const tree = MOCK_BROWSE_TREE[appId];
  if (!tree) {
    throw new Error(`Mock browse: no fixture for app ${appId}`);
  }
  const normalized = path.replace(/^[/\\]+|[/\\]+$/g, '').replace(/\\+/g, '/');
  const entries = tree[normalized];
  if (!entries) {
    throw new Error(`Mock browse: path "${normalized}" not in fixture`);
  }
  return {
    appId,
    installPath: `C:/SteamLibrary/steamapps/common/MockApp${appId}`,
    relativePath: normalized,
    entries: clone(entries),
    truncated: false
  };
}

export async function removeMockCustomGame(gameId: string): Promise<void> {
  mockCustomGames.delete(gameId);
  const detection = state.snapshot.gameDetection;
  state.snapshot = {
    ...state.snapshot,
    gameDetection: {
      ...detection,
      supportedGames: (detection.supportedGames ?? []).filter((game) => game.gameId !== gameId)
    }
  };
}

export async function writeMockSteamInputBinding(
  request: SteamInputBindingWriteRequest
): Promise<SteamInputBindingWriteResponse> {
  const layout =
    state.snapshot.steamInput.layouts.find((item) => item.source === request.layoutSource) ??
    state.snapshot.steamInput.layouts[0];
  if (!layout) throw new Error('No mock Steam Input layout is loaded.');

  const existingIndex = layout.bindings.findIndex((binding) => bindingMatchesWriteRequest(binding, request));
  const existing = existingIndex >= 0 ? layout.bindings[existingIndex] : null;
  const updated: SteamInputBinding = {
    input: existing?.input ?? request.inputId,
    inputId: request.inputId,
    binding: bindingLabel(request.rawBinding),
    rawBinding: request.rawBinding,
    kind: existing?.kind ?? 'button',
    source: existing?.source ?? 'Switches',
    sourceMode: existing?.sourceMode ?? 'Button',
    activator: request.activator ?? existing?.activator ?? 'regular',
    groupId: request.groupId ?? existing?.groupId ?? 'switches'
  };

  if (!request.dryRun) {
    if (existingIndex >= 0) {
      layout.bindings = layout.bindings.map((binding, index) => (index === existingIndex ? updated : binding));
    } else {
      layout.bindings = [...layout.bindings, updated];
    }
    layout.bindingCount = layout.bindings.length;
  }

  return {
    accepted: true,
    message: request.dryRun ? 'Mock Steam Input binding validated.' : 'Mock Steam Input binding saved.',
    dryRun: Boolean(request.dryRun),
    source: layout.source,
    targetPath: MOCK_LAYOUT_SOURCE,
    backupPath: null,
    binding: clone(updated),
    warnings: ['Mock mode did not read or write Steam VDF files.']
  };
}

function normalizeProfileGameId(gameId: string | null | undefined): string | null {
  const normalized = gameId?.trim();
  return normalized && normalized !== 'all' && normalized !== 'global' ? normalized : null;
}

function updateMockTelemetry(): void {
  const seconds = (Date.now() - mockStartedAt) / 1000;
  const shiftPulse = Math.floor(seconds) % 8 === 0;
  const clutchPressed = Math.floor(seconds + 2) % 10 === 0;
  const shiftPulseValue = shiftPulse ? (clutchPressed ? 0.58 : 1) : 0;
  state.snapshot.status.uptime = formatDuration(860 + Math.floor(seconds));
  state.snapshot.adapters = state.snapshot.adapters.map((adapter) =>
    adapter.id === 'forza-data-out'
      ? { ...adapter, packetRateHz: Math.round(wave(seconds, 57, 62, 0.4) * 10) / 10 }
      : adapter
  );
  state.snapshot.telemetry = [
    { name: 'input.brake', value: roundedUnit(wave(seconds, 0.18, 0.72, 0.25)), updatedMsAgo: 10 },
    { name: 'input.throttle', value: roundedUnit(wave(seconds, 0.28, 0.96, 1.15)), updatedMsAgo: 10 },
    { name: 'input.clutch', value: clutchPressed ? 0.86 : 0.04, updatedMsAgo: 10 },
    { name: 'input.handbrake', value: shiftPulse ? 1 : 0, updatedMsAgo: 10 },
    { name: 'wheel.slip.front_max', value: roundedUnit(wave(seconds, 0.04, 0.28, 2.2)), updatedMsAgo: 10 },
    { name: 'wheel.slip.max', value: roundedUnit(wave(seconds, 0.08, 0.46, 1.7)), updatedMsAgo: 10 },
    { name: 'surface.rumble.max', value: roundedUnit(wave(seconds, 0.18, 0.6, 0.8)), updatedMsAgo: 10 },
    { name: 'surface.rumble_strip.max', value: shiftPulse ? 0.78 : 0.08, updatedMsAgo: 10 },
    { name: 'surface.puddle.max', value: roundedUnit(wave(seconds, 0, 0.18, 3.1)), updatedMsAgo: 10 },
    {
      name: 'vehicle.acceleration.magnitude',
      value: Math.round(wave(seconds, 0.7, 1.8, 2.8) * 100) / 100,
      unit: 'g',
      updatedMsAgo: 10
    },
    { name: 'vehicle.rpm_ratio', value: roundedUnit(wave(seconds, 0.42, 0.98, 1.9)), updatedMsAgo: 10 },
    { name: 'drivetrain.shift_pulse', value: shiftPulseValue, updatedMsAgo: 10 }
  ];
  state.snapshot.effectState.parityEffects = state.snapshot.effectState.parityEffects.map((effect) =>
    effect.id === 'gear_shift_thump' || effect.id === 'rumble_strip'
      ? { ...effect, state: shiftPulse ? 'active' : 'ready' }
      : effect
  );
}

function outputForEffectRequest(request: EffectTestRequest): ControllerOutputFrame {
  const base = clone(state.snapshot.effectState.output);
  const intensity = Math.max(0, Math.min(255, Math.round(request.intensity * 2.55)));

  if (request.mode === 'off' || intensity === 0) {
    return {
      ...base,
      l2: request.target === 'r2' || request.target === 'lightbar' ? base.l2 : { type: 'off' },
      r2: request.target === 'l2' || request.target === 'lightbar' ? base.r2 : { type: 'off' },
      rumble: request.target === 'lightbar' ? base.rumble : null
    };
  }

  if (request.target === 'lightbar') {
    return {
      ...base,
      lightbar: {
        color: parseHexColor(request.mode) ?? parseHexColor(mockControllerConfig.lightbar.color) ?? { red: 59, green: 174, blue: 255 },
        brightness: request.intensity
      }
    };
  }

  if (request.target === 'rumble') {
    return {
      ...base,
      rumble: {
        low_frequency: intensity,
        high_frequency: Math.max(40, intensity - 20)
      }
    };
  }

  const trigger = request.trigger ?? controllerConfigFor(MOCK_CONTROLLER_ID).trigger;
  const l2 = triggerOutputFromRequest(trigger.effect, intensity, request.l2Position ?? request.startPosition ?? 0.25);
  const r2 = triggerOutputFromRequest(trigger.effect, intensity, request.r2Position ?? request.startPosition ?? 0.12);

  if (request.target === 'l2') return { ...base, l2 };
  if (request.target === 'r2') return { ...base, r2 };
  return {
    ...base,
    l2,
    r2,
    rumble: {
      low_frequency: Math.round(intensity * 0.35),
      high_frequency: Math.round(intensity * 0.48)
    }
  };
}

function triggerOutputFromRequest(
  effect: string,
  strength: number,
  position: number
): ControllerOutputFrame['l2'] {
  const safePosition = Math.max(0, Math.min(1, position));
  if (effect === 'Off') return { type: 'off' };
  if (effect === 'Wall') return { type: 'wall', position: Math.round(safePosition * 100), strength };
  if (effect === 'Pulse') return { type: 'pulse', amplitude: strength, frequency_hz: 38 };
  return { type: 'adaptive_resistance', start_position: Math.round(safePosition * 100), strength };
}

function syncEffectStateFromConfig(config: MockEditableControllerConfig): void {
  state.snapshot.effectState.output = {
    ...state.snapshot.effectState.output,
    l2: triggerOutputFromRequest(config.trigger.effect, 160, config.trigger.l2From / 100),
    r2: triggerOutputFromRequest(config.trigger.effect, 178, config.trigger.r2From / 100),
    lightbar: {
      color: parseHexColor(config.lightbar.color) ?? { red: 59, green: 174, blue: 255 },
      brightness: config.lightbar.brightness
    }
  };
  state.snapshot.effectState.parityEffects = config.forza.effects.map((effect) => ({
    id: effect.id,
    target: effect.route,
    label: effect.id.replaceAll('_', ' '),
    signal: signalForEffect(effect.id),
    state: effect.enabled ? 'ready' : 'disabled'
  }));
}

function activateProfileInState(profileId: string): void {
  const profile = requireProfile(profileId);
  state.snapshot.profiles = state.snapshot.profiles.map((item) => ({ ...item, active: item.id === profileId }));
  state.snapshot.status.activeProfile = profile.name;
  state.snapshot.effectState.selectedProfileId = profileId;
  state.snapshot.effectState.selectedProfileName = profile.name;
  state.snapshot.controllerProfileAssignments = [
    {
      controllerId: MOCK_CONTROLLER_ID,
      gameId: MOCK_GAME_ID,
      gameName: 'Forza Horizon 6 (mock)',
      profileId,
      profileName: profile.name,
      state: 'active',
      detail: 'mock profile selection'
    }
  ];

  const config = state.profileConfigs.get(profileId);
  if (config) {
    const current = controllerConfigFor(MOCK_CONTROLLER_ID);
    state.controllerConfigs.set(MOCK_CONTROLLER_ID, { ...clone(config), controllerId: MOCK_CONTROLLER_ID, model: current.model });
    syncEffectStateFromConfig(config);
  }
}

function defaultProfileIdForGame(gameId: string | null | undefined): string {
  return (
    state.snapshot.controllerProfileAssignments.find((assignment) => assignment.gameId === gameId)?.profileId ??
    state.snapshot.profiles.find((profile) => profile.id === MOCK_PROFILE_ID)?.id ??
    state.snapshot.profiles[0]?.id ??
    MOCK_PROFILE_ID
  );
}

function controllerConfigFor(controllerId: string): ControllerConfiguration {
  const config = state.controllerConfigs.get(controllerId) ?? state.controllerConfigs.get(MOCK_CONTROLLER_ID);
  if (!config) return { ...clone(mockControllerConfig), controllerId };
  return { ...config, controllerId };
}

function editableConfigFromController(config: ControllerConfiguration): MockEditableControllerConfig {
  return {
    inputMode: config.inputMode,
    trigger: clone(config.trigger),
    lightbar: clone(config.lightbar),
    forza: clone(config.forza),
    sticks: clone(config.sticks),
    buttons: clone(config.buttons),
    inputBridge: clone(config.inputBridge),
    profileAssignments: clone(config.profileAssignments)
  };
}

function editableConfigFromExport(config: ExportedProfile['config'] | undefined): MockEditableControllerConfig {
  const fallback = editableConfigFromController(controllerConfigFor(MOCK_CONTROLLER_ID));
  if (!config) return fallback;
  return {
    ...fallback,
    ...clone(config),
    profileAssignments: fallback.profileAssignments
  };
}

function requireProfile(profileId: string): ProfileSummary {
  const profile = state.snapshot.profiles.find((item) => item.id === profileId);
  if (!profile) throw new Error(`Mock profile not found: ${profileId}`);
  return profile;
}

function profileName(profileId: string): string {
  return requireProfile(profileId).name;
}

function fileNameFromPath(path: string): string {
  return path.trim().split(/[\\/]/).filter(Boolean).pop() ?? 'LocalApp.exe';
}

function mockLocalGameId(name: string, executablePath: string): string {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '') || 'local-app';
  let hash = 0x811c9dc5;
  for (const char of executablePath.trim().toLowerCase()) {
    hash ^= char.charCodeAt(0);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `local-${slug}-${hash.toString(16).padStart(8, '0').slice(0, 8)}`;
}

function uniqueProfileId(name: string): string {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '') || 'profile';
  let id = `mock-${slug}`;
  while (state.snapshot.profiles.some((profile) => profile.id === id)) {
    profileCounter += 1;
    id = `mock-${slug}-${profileCounter}`;
  }
  return id;
}

function bindingMatchesWriteRequest(binding: SteamInputBinding, request: SteamInputBindingWriteRequest): boolean {
  return (
    binding.inputId === request.inputId &&
    (request.groupId === undefined || request.groupId === null || binding.groupId === request.groupId) &&
    (request.activator === undefined || request.activator === null || binding.activator === request.activator)
  );
}

function bindingLabel(rawBinding: string): string {
  const parts = rawBinding.split(',');
  const label = parts.slice(2).join(',').trim();
  if (label) return label;
  return parts[0]?.trim() || rawBinding;
}

function parseHexColor(value: string): { red: number; green: number; blue: number } | null {
  const match = /^#?([0-9a-f]{6})$/i.exec(value.trim());
  if (!match) return null;
  return {
    red: parseInt(match[1].slice(0, 2), 16),
    green: parseInt(match[1].slice(2, 4), 16),
    blue: parseInt(match[1].slice(4, 6), 16)
  };
}

function signalForEffect(id: string): string {
  const signals: Record<string, string> = {
    brake_resistance: 'input.brake',
    abs_slip_pulse: 'wheel.slip.front_max',
    handbrake_wall: 'input.handbrake',
    throttle_resistance: 'input.throttle',
    gear_shift_thump: 'drivetrain.shift_pulse',
    rev_limiter_buzz: 'vehicle.rpm_ratio',
    road_texture: 'surface.rumble.max',
    rumble_strip: 'surface.rumble_strip.max',
    tire_slip: 'wheel.slip.max',
    puddle_drag: 'surface.puddle.max',
    suspension_impact: 'vehicle.acceleration.magnitude',
    rpm_leds: 'vehicle.rpm_ratio'
  };
  return signals[id] ?? id;
}

function wave(seconds: number, min: number, max: number, offset: number): number {
  const unit = (Math.sin(seconds * 1.35 + offset) + 1) / 2;
  return min + (max - min) * unit;
}

function roundedUnit(value: number): number {
  return Math.round(Math.max(0, Math.min(1, value)) * 100) / 100;
}

function signedWave(seconds: number, radius: number, offset: number): number {
  return Math.round(Math.sin(seconds * 1.45 + offset) * radius * 100) / 100;
}

function stickState(x: number, y: number) {
  return {
    x,
    y,
    magnitude: roundedUnit(Math.hypot(x, y))
  };
}

function mockButton(id: string, label: string, pressed: boolean) {
  return {
    id,
    label,
    pressed,
    value: pressed ? 1 : 0
  };
}

function formatDuration(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return hours > 0 ? `${hours}h ${minutes.toString().padStart(2, '0')}m` : `${minutes}m ${seconds.toString().padStart(2, '0')}s`;
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
