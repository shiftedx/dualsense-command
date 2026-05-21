import type {
  AddCustomGameResponse,
  AppUpdateCheck,
  AppSnapshot,
  AppSettingsResponse,
  ControllerConfiguration,
  ControllerInputState,
  ControllerProfileAssignment,
  ControllerStatus,
  ControllerOutputFrame,
  CurrentEffectState,
  DiagnosticsCheck,
  EffectTestRequest,
  ExportedProfile,
  GameDetection,
  HealthState,
  AdapterStatus,
  LogEntry,
  ModuleSummary,
  ProfileResolution,
  ProfileSummary,
  SnapshotPartialError,
  SteamInputBindingWriteRequest,
  SteamInputBindingWriteResponse,
  SteamInputStatus,
  SteamLibraryBrowseResponse,
  SteamLibraryResponse,
  SupportedGame,
  TelemetrySignal
} from './types';
import {
  activateMockProfile,
  addMockCustomGame,
  browseMockSteamLibrary,
  clearMockProfileOverride,
  connectMockAppSnapshotSocket,
  createMockProfile,
  deleteMockProfile,
  exportMockProfile,
  getMockAppSnapshot,
  getMockControllerConfig,
  getMockControllerInput,
  getMockSteamLibrary,
  importMockProfile,
  isMockApiEnabled,
  removeMockCustomGame,
  renameMockProfile,
  renameMockController,
  runMockEffectTest,
  saveMockAppSettings,
  saveMockControllerConfig,
  saveMockProfileConfig,
  setMockProfileOverride,
  writeMockSteamInputBinding
} from './mock/api';

const API_BASE = '/api';
const UPDATE_RELEASE_PAGE_URL = 'https://github.com/shiftedx/dualsense-command/releases/latest';
const UPDATE_CHECK_API_URL = 'https://api.github.com/repos/shiftedx/dualsense-command/releases/latest';

const jsonHeaders = {
  'Content-Type': 'application/json'
};

interface AgentStatusDto {
  version: string;
  healthy: boolean;
  bind_address?: string;
  uptime_seconds: number;
  active_profile_id: string | null;
  active_adapter_id: string | null;
}

interface ControllerDto {
  id: string;
  name: string;
  model: string;
  transport: string;
  connected: boolean;
  connection_state?: string;
  battery_percent: number | null;
  battery_state?: string;
  permission?: 'unknown' | 'granted' | 'denied';
  diagnostic_state?: ControllerStatus['diagnosticState'];
}

interface ProfileDto {
  id: string;
  name: string;
  built_in: boolean;
  game_id?: string | null;
  gameId?: string | null;
  active: boolean;
}

interface AdapterDto {
  id: string;
  name: string;
  enabled: boolean;
  state: string;
  packet_rate_hz: number | null;
  setup_hint?: string;
  setup_url?: string | null;
  protocol?: string;
}

interface TelemetrySignalDto {
  name: string;
  value: string | number | boolean;
  unit?: string | null;
  updated_ms_ago?: number;
}

interface AgentLogDto {
  level: string;
  message: string;
  timestamp: string;
}

interface DiagnosticsDto {
  checks: Array<{
    name: string;
    status: string;
    detail: string;
  }>;
}

interface ActionAcceptedDto {
  accepted: boolean;
  message: string;
}

interface EffectTestResponseDto {
  accepted: boolean;
  message: string;
  dry_run: boolean;
  duration_ms: number;
  output: ControllerOutputFrame;
}

interface ControllerInputResponseDto {
  controllerId?: string;
  controller_id?: string;
  available: boolean;
  source: string;
  message: string;
  l2: number;
  r2: number;
}

interface AgentSnapshotDto {
  status: AgentStatusDto;
  appSettings: AppSettingsResponse;
  controllers: ControllerDto[];
  profiles: ProfileDto[];
  adapters: AdapterDto[];
  modules: ModuleSummary[];
  steamInput: SteamInputStatus;
  gameDetection: GameDetection;
  profileResolution: ProfileResolution;
  effectState: CurrentEffectState;
  telemetry: TelemetrySignalDto[];
  logs: AgentLogDto[];
  diagnostics: DiagnosticsDto;
  partialErrors: SnapshotPartialError[];
}

type AppUpdateCheckDto = {
  currentVersion?: unknown;
  current_version?: unknown;
  latestVersion?: unknown;
  latest_version?: unknown;
  updateAvailable?: unknown;
  update_available?: unknown;
  releaseUrl?: unknown;
  release_url?: unknown;
  checkedAt?: unknown;
  checked_at?: unknown;
  message?: unknown;
  error?: unknown;
  state?: unknown;
};

type GitHubReleaseResponse = {
  tag_name?: unknown;
  html_url?: unknown;
};

type SnapshotSocketMessage = {
  type?: string;
  snapshot?: unknown;
  [key: string]: unknown;
};

export type AppSnapshotSocketCallbacks = {
  onSnapshot?: (snapshot: AppSnapshot) => void;
  onInvalidate: () => void;
  onUnavailable?: () => void;
  onClosed?: () => void;
};

class ApiRequestError extends Error {
  constructor(
    message: string,
    readonly status: number | null = null,
    readonly networkFailure = false
  ) {
    super(message);
  }
}

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      ...init,
      headers: {
        ...jsonHeaders,
        ...init?.headers
      }
    });
  } catch (caught) {
    const detail = caught instanceof Error ? caught.message : 'network request failed';
    throw new ApiRequestError(`API request failed: ${detail}`, null, true);
  }

  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new ApiRequestError(
      `API request failed: ${response.status} ${response.statusText}${detail ? ` / ${detail}` : ''}`,
      response.status
    );
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

const FALLBACK_AGENT_STATUS: AgentStatusDto = {
  version: 'unknown',
  healthy: false,
  uptime_seconds: 0,
  active_profile_id: null,
  active_adapter_id: null
};

const FALLBACK_STEAM_INPUT: SteamInputStatus = {
  running: false,
  available: false,
  steamPath: null,
  layouts: [],
  warnings: []
};

const FALLBACK_GAME_DETECTION: GameDetection = {
  activeGameId: null,
  activeGameName: null,
  source: 'unknown',
  confidence: 0,
  processName: null,
  moduleId: null,
  adapterId: null,
  profileId: null,
  candidates: []
};

const FALLBACK_PROFILE_RESOLUTION: ProfileResolution = {
  controllerId: null,
  detectedGameId: null,
  activeAdapterId: null,
  selectedProfileId: null,
  reason: 'unavailable',
  overrideProfileId: null,
  validation: 'unavailable'
};

const FALLBACK_EFFECT_STATE: CurrentEffectState = {
  controllerId: null,
  selectedProfileId: null,
  selectedProfileName: null,
  reason: 'unavailable',
  dryRun: false,
  hardwareOutputEnabled: false,
  output: {
    l2: { type: 'off' },
    r2: { type: 'off' },
    lightbar: null,
    playerLeds: null,
    rumble: null
  },
  parityEffects: [],
  warnings: []
};

const FALLBACK_DIAGNOSTICS: DiagnosticsDto = { checks: [] };

const FALLBACK_APP_SETTINGS: AppSettingsResponse = {
  settings: {
    listenOnAllInterfaces: false,
    forzaPlaystationGlyphs: {
      enabled: false,
      installPath: null,
      lastStatus: 'not_installed',
      lastMessage: 'PlayStation glyph override has not been applied.'
    }
  },
  effectiveBindAddress: '127.0.0.1:43473',
  desiredBindAddress: '127.0.0.1:43473',
  restartRequired: false
};

export async function getAppSnapshot(): Promise<AppSnapshot> {
  if (isMockApiEnabled()) return getMockAppSnapshot();
  return mapSnapshotDto(await apiFetch<AgentSnapshotDto | AppSnapshot>('/snapshot'));
}

export async function getAppUpdateCheck(currentVersion: string): Promise<AppUpdateCheck> {
  const normalizedCurrent = normalizeVersion(currentVersion);
  if (!normalizedCurrent || normalizedCurrent.toLowerCase() === 'unknown') {
    throw new Error('Current app version is unavailable.');
  }

  if (!isMockApiEnabled()) {
    const params = new URLSearchParams({ currentVersion: normalizedCurrent });
    try {
      const dto = await apiFetch<AppUpdateCheckDto>(`/update-check?${params.toString()}`);
      return normalizeUpdateCheckDto(dto, normalizedCurrent, 'agent');
    } catch (caught) {
      if (!shouldFallbackToGitHubUpdateCheck(caught)) throw caught;
    }
  }

  return getGitHubUpdateCheck(normalizedCurrent);
}

export async function saveAppSettings(request: {
  listenOnAllInterfaces?: boolean;
  forzaPlaystationGlyphs?: {
    enabled: boolean;
    installPath?: string | null;
  };
}): Promise<AppSettingsResponse> {
  if (isMockApiEnabled()) return saveMockAppSettings(request);
  return apiFetch<AppSettingsResponse>('/app-settings', {
    method: 'PUT',
    body: JSON.stringify(request)
  });
}

async function getGitHubUpdateCheck(currentVersion: string): Promise<AppUpdateCheck> {
  const response = await fetch(UPDATE_CHECK_API_URL, {
    headers: { Accept: 'application/vnd.github+json' },
    cache: 'no-store'
  });
  if (!response.ok) throw new Error(`Release lookup returned ${response.status}`);

  const release = (await response.json()) as GitHubReleaseResponse;
  const latestVersion = normalizeVersion(typeof release.tag_name === 'string' ? release.tag_name : '');
  if (!latestVersion) throw new Error('Release response did not include a tag.');

  return {
    currentVersion,
    latestVersion,
    updateAvailable: isVersionNewer(latestVersion, currentVersion),
    releaseUrl: typeof release.html_url === 'string' ? release.html_url : UPDATE_RELEASE_PAGE_URL,
    source: 'github',
    checkedAt: null,
    message: null
  };
}

function shouldFallbackToGitHubUpdateCheck(caught: unknown): boolean {
  return (
    caught instanceof ApiRequestError &&
    (caught.networkFailure || caught.status === 404 || caught.status === 405 || caught.status === 501)
  );
}

function normalizeUpdateCheckDto(
  dto: AppUpdateCheckDto,
  currentVersion: string,
  source: AppUpdateCheck['source']
): AppUpdateCheck {
  const dtoCurrentVersion = normalizeVersion(stringValue(dto.currentVersion ?? dto.current_version)) || currentVersion;
  const state = stringValue(dto.state);
  const latestVersion = normalizeVersion(stringValue(dto.latestVersion ?? dto.latest_version)) || dtoCurrentVersion;
  const updateAvailableValue = dto.updateAvailable ?? dto.update_available;
  const updateAvailable =
    typeof updateAvailableValue === 'boolean'
      ? updateAvailableValue
      : state === 'update_available' || isVersionNewer(latestVersion, dtoCurrentVersion);

  return {
    currentVersion: dtoCurrentVersion,
    latestVersion,
    updateAvailable,
    releaseUrl: stringValue(dto.releaseUrl ?? dto.release_url) || UPDATE_RELEASE_PAGE_URL,
    source,
    checkedAt: stringValue(dto.checkedAt ?? dto.checked_at) || null,
    message: stringValue(dto.message) || stringValue(dto.error) || null
  };
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

export function connectAppSnapshotSocket(callbacks: AppSnapshotSocketCallbacks): () => void {
  if (isMockApiEnabled()) return connectMockAppSnapshotSocket(callbacks);
  if (typeof window === 'undefined' || typeof WebSocket === 'undefined') {
    callbacks.onUnavailable?.();
    return () => {};
  }

  let socket: WebSocket;
  let closedByClient = false;
  try {
    socket = new WebSocket(webSocketUrl('/ws'));
  } catch {
    callbacks.onUnavailable?.();
    return () => {};
  }

  socket.addEventListener('message', (event) => {
    const message = parseSocketMessage(event.data);
    if (!message) return;

    const snapshot = snapshotFromSocketMessage(message);
    if (snapshot) {
      callbacks.onSnapshot?.(snapshot);
      return;
    }

    if (message.type === 'snapshot' || isInvalidationMessage(message)) {
      callbacks.onInvalidate();
    }
  });
  socket.addEventListener('error', () => {
    if (!closedByClient) callbacks.onUnavailable?.();
  });
  socket.addEventListener('close', () => {
    if (!closedByClient) callbacks.onClosed?.();
  });

  return () => {
    closedByClient = true;
    socket.close();
  };
}

export async function runEffectTest(
  request: EffectTestRequest,
  controllerId?: string | null
): Promise<{
  accepted: true;
  message: string;
  dryRun: boolean;
  durationMs: number;
  output: ControllerOutputFrame;
}> {
  const safeRequest: EffectTestRequest = {
    ...request,
    intensity: Math.max(0, Math.min(100, request.intensity)),
    startPosition:
      request.startPosition === undefined ? undefined : Math.max(0, Math.min(1, request.startPosition)),
    l2Position: request.l2Position === undefined ? undefined : Math.max(0, Math.min(1, request.l2Position)),
    r2Position: request.r2Position === undefined ? undefined : Math.max(0, Math.min(1, request.r2Position)),
    durationMs: Math.max(100, Math.min(60000, request.durationMs))
  };

  if (isMockApiEnabled()) return runMockEffectTest(safeRequest);

  const endpoint = controllerId
    ? `/controllers/${encodeURIComponent(controllerId)}/test-effect`
    : '/controllers/current/test-effect';
  const response = await apiFetch<EffectTestResponseDto>(endpoint, {
    method: 'POST',
    body: JSON.stringify(safeRequest)
  });

  if (!response.accepted) {
    throw new Error(response.message);
  }

  return {
    accepted: true,
    message: response.message,
    dryRun: response.dry_run,
    durationMs: response.duration_ms,
    output: response.output
  };
}

export async function getControllerInput(controllerId?: string | null): Promise<ControllerInputState> {
  if (isMockApiEnabled()) return getMockControllerInput(controllerId);
  const endpoint = controllerId
    ? `/controllers/${encodeURIComponent(controllerId)}/input`
    : '/controllers/current/input';
  const response = await apiFetch<ControllerInputResponseDto>(endpoint);

  return {
    controllerId: response.controllerId ?? response.controller_id ?? '',
    available: response.available,
    source: response.source,
    message: response.message,
    l2: response.l2,
    r2: response.r2
  };
}

export async function getControllerConfig(controllerId: string): Promise<ControllerConfiguration> {
  if (isMockApiEnabled()) return getMockControllerConfig(controllerId);
  return apiFetch<ControllerConfiguration>(`/controllers/${encodeURIComponent(controllerId)}/config`);
}

export async function saveControllerConfig(
  controllerId: string,
  config: Omit<ControllerConfiguration, 'controllerId' | 'model'>
): Promise<ControllerConfiguration> {
  if (isMockApiEnabled()) return saveMockControllerConfig(controllerId, config);
  return apiFetch<ControllerConfiguration>(`/controllers/${encodeURIComponent(controllerId)}/config`, {
    method: 'PUT',
    body: JSON.stringify(config)
  });
}

export async function updateControllerName(controllerId: string, name: string): Promise<ControllerStatus> {
  if (isMockApiEnabled()) return renameMockController(controllerId, name);
  const dto = await apiFetch<ControllerDto>(`/controllers/${encodeURIComponent(controllerId)}`, {
    method: 'PUT',
    body: JSON.stringify({ name })
  });
  return mapController(dto);
}

export async function saveProfileConfig(
  profileId: string,
  config: Omit<ControllerConfiguration, 'controllerId' | 'model'>
): Promise<ActionAcceptedDto> {
  if (isMockApiEnabled()) return saveMockProfileConfig(profileId, config);
  return apiFetch<ActionAcceptedDto>(`/profiles/${encodeURIComponent(profileId)}/config`, {
    method: 'PUT',
    body: JSON.stringify(config)
  });
}

export async function setProfileOverride(request: {
  controllerId?: string | null;
  gameId?: string | null;
  profileId: string;
}): Promise<ProfileResolution> {
  if (isMockApiEnabled()) return setMockProfileOverride(request);
  return apiFetch<ProfileResolution>('/profile-resolution/override', {
    method: 'PUT',
    body: JSON.stringify(request)
  });
}

export async function clearProfileOverride(request?: {
  controllerId?: string | null;
  gameId?: string | null;
}): Promise<ProfileResolution> {
  if (isMockApiEnabled()) return clearMockProfileOverride(request);
  const params = new URLSearchParams();
  if (request?.controllerId) params.set('controllerId', request.controllerId);
  if (request?.gameId) params.set('gameId', request.gameId);
  const query = params.toString();
  return apiFetch<ProfileResolution>(`/profile-resolution/override${query ? `?${query}` : ''}`, {
    method: 'DELETE'
  });
}

export async function activateProfile(profileId: string): Promise<ActionAcceptedDto> {
  if (isMockApiEnabled()) return activateMockProfile(profileId);
  return apiFetch<ActionAcceptedDto>(`/profiles/${encodeURIComponent(profileId)}/activate`, {
    method: 'POST'
  });
}

export async function createProfile(name: string, options?: { gameId?: string | null }): Promise<ProfileSummary> {
  if (isMockApiEnabled()) return createMockProfile(name, options);
  const dto = await apiFetch<ProfileDto>('/profiles', {
    method: 'POST',
    body: JSON.stringify({ name, gameId: options?.gameId ?? null })
  });
  return mapProfile(dto);
}

export async function renameProfile(profileId: string, name: string): Promise<ProfileSummary> {
  if (isMockApiEnabled()) return renameMockProfile(profileId, name);
  const dto = await apiFetch<ProfileDto>(`/profiles/${encodeURIComponent(profileId)}`, {
    method: 'PUT',
    body: JSON.stringify({ name })
  });
  return mapProfile(dto);
}

export async function exportProfile(profileId: string): Promise<ExportedProfile> {
  if (isMockApiEnabled()) return exportMockProfile(profileId);
  return apiFetch<ExportedProfile>(`/profiles/${encodeURIComponent(profileId)}/export`);
}

export async function importProfile(profile: {
  schema: string;
  id?: string | null;
  name: string;
  config?: ExportedProfile['config'];
}): Promise<ProfileSummary> {
  if (isMockApiEnabled()) return importMockProfile(profile);
  const dto = await apiFetch<ProfileDto>('/profiles/import', {
    method: 'POST',
    body: JSON.stringify(profile)
  });
  return mapProfile(dto);
}

export async function deleteProfile(profileId: string): Promise<ActionAcceptedDto | void> {
  if (isMockApiEnabled()) return deleteMockProfile(profileId);
  try {
    return await apiFetch<ActionAcceptedDto | void>(`/profiles/${encodeURIComponent(profileId)}`, {
      method: 'DELETE'
    });
  } catch (caught) {
    if (caught instanceof ApiRequestError && caught.status === 404) {
      return { accepted: true, message: 'Profile was already deleted' };
    }
    throw caught;
  }
}

export async function writeSteamInputBinding(
  request: SteamInputBindingWriteRequest
): Promise<SteamInputBindingWriteResponse> {
  if (isMockApiEnabled()) return writeMockSteamInputBinding(request);
  return apiFetch<SteamInputBindingWriteResponse>('/steam-input/bindings', {
    method: 'POST',
    body: JSON.stringify(request)
  });
}

export async function getSteamLibrary(): Promise<SteamLibraryResponse> {
  if (isMockApiEnabled()) return getMockSteamLibrary();
  return apiFetch<SteamLibraryResponse>('/games/steam-library');
}

export async function addCustomGame(
  appId: string,
  processNames: string[] = []
): Promise<AddCustomGameResponse> {
  if (isMockApiEnabled()) return addMockCustomGame(appId, processNames);
  const body: { appId: string; processNames?: string[] } = { appId };
  if (processNames.length > 0) body.processNames = processNames;
  return apiFetch<AddCustomGameResponse>('/games/custom', {
    method: 'POST',
    headers: jsonHeaders,
    body: JSON.stringify(body)
  });
}

export async function removeCustomGame(gameId: string): Promise<void> {
  if (isMockApiEnabled()) return removeMockCustomGame(gameId);
  await apiFetch<void>(`/games/custom/${encodeURIComponent(gameId)}`, {
    method: 'DELETE'
  });
}

export async function browseSteamLibrary(
  appId: string,
  path = ''
): Promise<SteamLibraryBrowseResponse> {
  if (isMockApiEnabled()) return browseMockSteamLibrary(appId, path);
  const qs = new URLSearchParams({ appId });
  if (path) qs.set('path', path);
  return apiFetch<SteamLibraryBrowseResponse>(`/games/steam-library/browse?${qs.toString()}`);
}

function mapSnapshotDto(dto: AgentSnapshotDto | AppSnapshot): AppSnapshot {
  if (isAppSnapshot(dto)) return normalizeAppSnapshot(dto);

  const snapshot = mapAgentSnapshot(
    dto.status ?? FALLBACK_AGENT_STATUS,
    dto.appSettings ?? FALLBACK_APP_SETTINGS,
    dto.controllers ?? [],
    dto.profiles ?? [],
    dto.adapters ?? [],
    dto.modules ?? [],
    dto.steamInput ?? FALLBACK_STEAM_INPUT,
    dto.gameDetection ?? FALLBACK_GAME_DETECTION,
    dto.profileResolution ?? FALLBACK_PROFILE_RESOLUTION,
    dto.effectState ?? FALLBACK_EFFECT_STATE,
    dto.telemetry ?? [],
    dto.logs ?? [],
    dto.diagnostics ?? FALLBACK_DIAGNOSTICS
  );
  snapshot.partialErrors = normalizePartialErrors(dto.partialErrors);
  return snapshot;
}

function normalizeAppSnapshot(snapshot: AppSnapshot): AppSnapshot {
  return {
    ...snapshot,
    gameDetection: normalizeGameDetection(snapshot.gameDetection),
    partialErrors: normalizePartialErrors(snapshot.partialErrors)
  };
}

function normalizePartialErrors(errors: SnapshotPartialError[] | undefined): SnapshotPartialError[] {
  return Array.isArray(errors) ? errors : [];
}

function isAppSnapshot(value: unknown): value is AppSnapshot {
  if (!value || typeof value !== 'object') return false;
  const snapshot = value as Partial<AppSnapshot>;
  return Boolean(
    snapshot.status &&
      typeof snapshot.status.uptime === 'string' &&
      Array.isArray(snapshot.controllerProfileAssignments) &&
      snapshot.effectState
  );
}

function isCompleteSnapshotPayload(value: unknown): value is AgentSnapshotDto | AppSnapshot {
  if (isAppSnapshot(value)) return true;
  if (!value || typeof value !== 'object') return false;
  const snapshot = value as Record<string, unknown>;
  return Boolean(
    snapshot.status &&
      Array.isArray(snapshot.controllers) &&
      snapshot.appSettings &&
      Array.isArray(snapshot.profiles) &&
      Array.isArray(snapshot.adapters) &&
      Array.isArray(snapshot.modules) &&
      snapshot.steamInput &&
      snapshot.gameDetection &&
      snapshot.profileResolution &&
      snapshot.effectState &&
      Array.isArray(snapshot.telemetry) &&
      Array.isArray(snapshot.logs) &&
      snapshot.diagnostics
  );
}

function webSocketUrl(path: string): string {
  const url = new URL(`${API_BASE}${path}`, window.location.href);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}

function parseSocketMessage(data: unknown): SnapshotSocketMessage | null {
  if (typeof data !== 'string') return null;
  try {
    const value = JSON.parse(data);
    return value && typeof value === 'object' ? (value as SnapshotSocketMessage) : null;
  } catch {
    return null;
  }
}

function snapshotFromSocketMessage(message: SnapshotSocketMessage): AppSnapshot | null {
  if (message.type !== 'snapshot') return null;
  return isCompleteSnapshotPayload(message.snapshot) ? mapSnapshotDto(message.snapshot) : null;
}

function isInvalidationMessage(message: SnapshotSocketMessage): boolean {
  const type = typeof message.type === 'string' ? message.type : '';
  return type.length > 0 && type !== 'snapshot' && type !== 'ping' && type !== 'pong';
}

function mapAgentSnapshot(
  status: AgentStatusDto,
  appSettings: AppSettingsResponse,
  controllers: ControllerDto[],
  profiles: ProfileDto[],
  adapters: AdapterDto[],
  modules: ModuleSummary[],
  steamInput: SteamInputStatus,
  gameDetection: GameDetection,
  profileResolution: ProfileResolution,
  effectState: CurrentEffectState,
  telemetry: TelemetrySignalDto[],
  logs: AgentLogDto[],
  diagnostics: DiagnosticsDto
): AppSnapshot {
  const profileMap = new Map(profiles.map((profile) => [profile.id, profile.name]));
  const adapterMap = new Map(adapters.map((adapter) => [adapter.id, adapter.name]));
  const normalizedGameDetection = normalizeGameDetection(gameDetection);

  return {
    status: {
      version: status.version,
      uptime: formatDuration(status.uptime_seconds),
      bindAddress: status.bind_address ?? appSettings.effectiveBindAddress,
      mode: 'agent',
      health: status.healthy ? 'running' : 'faulted',
      activeProfile: profileMap.get(status.active_profile_id ?? '') ?? status.active_profile_id ?? 'none',
      activeAdapter:
        adapterMap.get(status.active_adapter_id ?? '') ?? status.active_adapter_id ?? 'none'
    },
    appSettings,
    controllers: controllers.map(mapController),
    profiles: profiles.map(mapProfile),
    controllerProfileAssignments: makeControllerProfileAssignments(
      controllers,
      profiles,
      profileResolution,
      normalizedGameDetection
    ),
    adapters: adapters.map(mapAdapter),
    modules,
    steamInput,
    gameDetection: normalizedGameDetection,
    profileResolution,
    effectState,
    telemetry: mapTelemetry(status, adapters, telemetry),
    logs: logs.map(mapLog).slice(0, 8),
    diagnostics: diagnostics.checks.map(mapDiagnostic),
    partialErrors: []
  };
}

function normalizeGameDetection(gameDetection: GameDetection | undefined): GameDetection {
  const source = gameDetection ?? FALLBACK_GAME_DETECTION;
  const supportedGames = source.supportedGames ?? [];
  const selectedGame = source.selectedGame ?? null;

  return {
    ...source,
    activeGameId: source.activeGameId ?? null,
    activeGameName: source.activeGameName ?? null,
    source: source.source ?? 'unknown',
    confidence: source.confidence ?? 0,
    processName: source.processName ?? null,
    moduleId: source.moduleId ?? null,
    adapterId: source.adapterId ?? null,
    profileId: source.profileId ?? null,
    candidates: Array.isArray(source.candidates) ? source.candidates : [],
    supportedGames: Array.isArray(supportedGames) ? supportedGames : [],
    selectedGame
  };
}

function mapController(controller: ControllerDto): ControllerStatus {
  return {
    id: controller.id,
    name: controller.name,
    family: mapFamily(controller.model),
    transport: mapTransport(controller.transport),
    connected: controller.connected,
    battery: controller.battery_percent,
    batteryState: mapBatteryState(controller.battery_state),
    charging: controller.battery_state === 'charging',
    permission: controller.permission ?? 'unknown',
    diagnosticState:
      controller.diagnostic_state ??
      (controller.connected || controller.connection_state === 'connected' ? 'ok' : 'disconnected'),
    capabilities: ['adaptive triggers', 'lightbar', 'player leds', 'rumble']
  };
}

function mapProfile(profile: ProfileDto): ProfileSummary {
  const gameId = profile.game_id ?? profile.gameId ?? null;
  return {
    id: profile.id,
    name: profile.name,
    scope: profile.built_in ? 'Built-in' : gameId ? 'Game' : 'Global',
    gameId: gameId ?? 'all',
    active: profile.active,
    rules: 0,
    updatedAt: 'agent'
  };
}

function makeControllerProfileAssignments(
  controllers: ControllerDto[],
  profiles: ProfileDto[],
  profileResolution: ProfileResolution,
  gameDetection: GameDetection
): ControllerProfileAssignment[] {
  const activeControllerId =
    profileResolution.controllerId ?? controllers.find((controller) => controller.connected)?.id ?? controllers[0]?.id;
  const selectedProfileId = profileResolution.selectedProfileId;
  if (!activeControllerId || !selectedProfileId) return [];

  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId);
  const gameId = profileResolution.detectedGameId ?? gameDetection.activeGameId ?? 'global';
  const gameName = gameDetection.activeGameName ?? gameId;

  return [
    {
      controllerId: activeControllerId,
      gameId,
      gameName,
      profileId: selectedProfileId,
      profileName: selectedProfile?.name ?? selectedProfileId,
      state: 'active',
      detail: profileResolution.reason.replaceAll('_', ' ')
    }
  ];
}

function mapAdapter(adapter: AdapterDto): AdapterStatus {
  return {
    id: adapter.id,
    name: adapter.name,
    state: mapAdapterState(adapter),
    packetRateHz: adapter.packet_rate_hz ?? 0,
    config: adapter.enabled ? `${adapter.protocol ?? 'adapter'} / agent managed` : 'Disabled',
    setupHint:
      adapter.setup_hint ??
      (adapter.enabled ? 'Agent reports this adapter as enabled.' : 'Enable this adapter to start telemetry.')
  };
}

function mapTelemetry(
  status: AgentStatusDto,
  adapters: AdapterDto[],
  telemetry: TelemetrySignalDto[]
): TelemetrySignal[] {
  void status;
  void adapters;
  return telemetry.map((signal) => ({
    name: signal.name,
    value: signal.value,
    unit: signal.unit ?? undefined,
    updatedMsAgo: signal.updated_ms_ago ?? 0
  }));
}

function mapLog(log: AgentLogDto): LogEntry {
  return {
    level: mapLogLevel(log.level),
    time: formatTime(log.timestamp),
    source: 'agent',
    message: log.message
  };
}

function mapDiagnostic(check: DiagnosticsDto['checks'][number]): DiagnosticsCheck {
  return {
    label: check.name,
    state:
      check.status === 'ok' || check.status === 'hidapi'
        ? 'pass'
        : check.status === 'warning' || check.status === 'blocked' || check.status === 'error'
          ? 'warn'
          : 'pending',
    detail: check.detail
  };
}

function mapFamily(model: string): ControllerStatus['family'] {
  if (model.toLowerCase().includes('edge')) return 'DualSense Edge';
  if (model.toLowerCase().includes('dualsense')) return 'DualSense';
  return 'Unknown Sony';
}

function mapTransport(transport: string): ControllerStatus['transport'] {
  if (transport === 'usb') return 'USB';
  if (transport === 'bluetooth') return 'Bluetooth';
  return 'Unknown';
}

function mapBatteryState(state?: string): ControllerStatus['batteryState'] {
  if (state === 'discharging' || state === 'charging' || state === 'full') return state;
  return 'unknown';
}

function mapAdapterState(adapter: AdapterDto): HealthState {
  if (!adapter.enabled || adapter.state === 'disabled') return 'unavailable';
  if (adapter.state === 'connected') return 'running';
  if (adapter.state === 'ready') return 'ready';
  if (adapter.state === 'needs_setup') return 'needs_setup';
  if (adapter.state === 'faulted') return 'faulted';
  return 'ready';
}

function mapLogLevel(level: string): LogEntry['level'] {
  if (level === 'trace' || level === 'debug' || level === 'info' || level === 'warn' || level === 'error') {
    return level;
  }
  return 'info';
}

function normalizeVersion(value: string | undefined | null): string {
  return (value ?? '').trim().replace(/^v/i, '');
}

function versionParts(value: string): number[] {
  return normalizeVersion(value).match(/\d+/g)?.slice(0, 4).map(Number) ?? [];
}

function isVersionNewer(candidate: string, current: string): boolean {
  const candidateParts = versionParts(candidate);
  const currentParts = versionParts(current);
  if (!candidateParts.length || !currentParts.length) return false;
  const length = Math.max(candidateParts.length, currentParts.length);
  for (let index = 0; index < length; index += 1) {
    const left = candidateParts[index] ?? 0;
    const right = currentParts[index] ?? 0;
    if (left !== right) return left > right;
  }
  return false;
}

function formatDuration(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes.toString().padStart(2, '0')}m`;
  }

  return `${minutes}m ${seconds.toString().padStart(2, '0')}s`;
}

function formatTime(timestamp: string): string {
  const parsed = new Date(timestamp);
  if (Number.isNaN(parsed.getTime())) return timestamp;
  return parsed.toLocaleTimeString([], { hour12: false });
}
