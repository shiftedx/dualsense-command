import type {
  AdapterStatus,
  AppSettingsResponse,
  AppSnapshot,
  ControllerPowerDiagnostics,
  ControllerProfileAssignment,
  ControllerStatus,
  CurrentEffectState,
  DiagnosticsCheck,
  GameDetection,
  HealthState,
  InputBridgeStatus,
  LogEntry,
  ModuleSummary,
  ProfileResolution,
  ProfileSummary,
  SnapshotPartialError,
  SteamInputStatus,
  SupportedGame,
  TelemetrySignal
} from '../types';
import { apiFetch, isMockApiEnabled, loadMockApi, webSocketUrl } from './client';

export interface AgentStatusDto {
  version: string;
  healthy: boolean;
  bind_address?: string;
  uptime_seconds: number;
  active_profile_id: string | null;
  active_adapter_id: string | null;
}

export interface ControllerDto {
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
  power_diagnostics?: ControllerPowerDiagnosticsDto | null;
  powerDiagnostics?: ControllerPowerDiagnostics | null;
}

interface ControllerPowerDiagnosticsDto {
  written_reports?: number | null;
  writtenReports?: number | null;
  output_write_rate_hz?: number | null;
  outputWriteRateHz?: number | null;
  output_cadence_ms?: number | null;
  outputCadenceMs?: number | null;
  suppressed_redundant_reports?: number | null;
  suppressedRedundantReports?: number | null;
  keepalive_interval_ms?: number | null;
  keepaliveIntervalMs?: number | null;
  last_write_age_ms?: number | null;
  lastWriteAgeMs?: number | null;
  last_suppressed_age_ms?: number | null;
  lastSuppressedAgeMs?: number | null;
  native_rumble_passthrough?: boolean | null;
  nativeRumblePassthrough?: boolean | null;
  adaptive_triggers_retained?: boolean | null;
  adaptiveTriggersRetained?: boolean | null;
}

export interface ProfileDto {
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

interface AgentSnapshotDto {
  status: AgentStatusDto;
  appSettings: AppSettingsResponse;
  controllers: ControllerDto[];
  profiles: ProfileDto[];
  adapters: AdapterDto[];
  modules: ModuleSummary[];
  steamInput: SteamInputStatus;
  inputBridge?: InputBridgeStatus;
  gameDetection: GameDetection;
  profileResolution: ProfileResolution;
  effectState: CurrentEffectState;
  telemetry: TelemetrySignalDto[];
  logs: AgentLogDto[];
  diagnostics: DiagnosticsDto;
  partialErrors: SnapshotPartialError[];
}

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

const FALLBACK_INPUT_BRIDGE: InputBridgeStatus = {
  available: false,
  backendId: 'unavailable',
  provider: 'none',
  state: 'unavailable',
  message: 'DSCC Input Bridge status is unavailable.',
  supportedKinds: [],
  sessions: [],
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
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).getMockAppSnapshot();
  return mapSnapshotDto(await apiFetch<AgentSnapshotDto | AppSnapshot>('/snapshot'));
}

export function connectAppSnapshotSocket(callbacks: AppSnapshotSocketCallbacks): () => void {
  if (import.meta.env.DEV && isMockApiEnabled()) {
    let cleanup: (() => void) | undefined;
    let closed = false;
    void loadMockApi()
      .then((mockApi) => {
        if (closed) return;
        cleanup = mockApi.connectMockAppSnapshotSocket(callbacks);
      })
      .catch(() => {
        if (!closed) callbacks.onUnavailable?.();
      });
    return () => {
      closed = true;
      cleanup?.();
    };
  }
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
    dto.inputBridge ?? FALLBACK_INPUT_BRIDGE,
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
    inputBridge: snapshot.inputBridge ?? FALLBACK_INPUT_BRIDGE,
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
  inputBridge: InputBridgeStatus,
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
      activeAdapter: adapterMap.get(status.active_adapter_id ?? '') ?? status.active_adapter_id ?? 'none'
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
    inputBridge,
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
    supportedGames: Array.isArray(supportedGames) ? supportedGames.map(normalizeSupportedGame) : [],
    selectedGame: selectedGame ? normalizeSupportedGame(selectedGame) : null
  };
}

function normalizeSupportedGame(game: SupportedGame): SupportedGame {
  const local = game.gameId.startsWith('local-') || game.source === 'local_app';
  return {
    ...game,
    source: game.source ?? (game.supportLevel === 'custom' ? 'steam' : 'built_in'),
    inputProvider:
      game.inputProvider ??
      (local ? 'dscc_input_bridge' : game.supportLevel === 'custom' ? 'steam_input' : 'native_dualsense'),
    processNames: Array.isArray(game.processNames) ? game.processNames : [],
    executableName: game.executableName ?? game.processNames?.[0] ?? null
  };
}

export function mapController(controller: ControllerDto): ControllerStatus {
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
    capabilities: ['adaptive triggers', 'lightbar', 'player leds', 'rumble'],
    powerDiagnostics: mapControllerPowerDiagnostics(controller.power_diagnostics ?? controller.powerDiagnostics)
  };
}

function mapControllerPowerDiagnostics(
  diagnostics: ControllerPowerDiagnosticsDto | null | undefined
): ControllerPowerDiagnostics | null {
  if (!diagnostics) return null;
  const normalized: ControllerPowerDiagnostics = {
    writtenReports: optionalNumber(diagnostics.written_reports ?? diagnostics.writtenReports),
    outputWriteRateHz: optionalNumber(diagnostics.output_write_rate_hz ?? diagnostics.outputWriteRateHz),
    outputCadenceMs: optionalNumber(diagnostics.output_cadence_ms ?? diagnostics.outputCadenceMs),
    suppressedRedundantReports: optionalNumber(
      diagnostics.suppressed_redundant_reports ?? diagnostics.suppressedRedundantReports
    ),
    keepaliveIntervalMs: optionalNumber(diagnostics.keepalive_interval_ms ?? diagnostics.keepaliveIntervalMs),
    lastWriteAgeMs: optionalNumber(diagnostics.last_write_age_ms ?? diagnostics.lastWriteAgeMs),
    lastSuppressedAgeMs: optionalNumber(diagnostics.last_suppressed_age_ms ?? diagnostics.lastSuppressedAgeMs),
    nativeRumblePassthrough: optionalBoolean(
      diagnostics.native_rumble_passthrough ?? diagnostics.nativeRumblePassthrough
    ),
    adaptiveTriggersRetained: optionalBoolean(
      diagnostics.adaptive_triggers_retained ?? diagnostics.adaptiveTriggersRetained
    )
  };

  return Object.values(normalized).some((value) => value !== null && value !== undefined) ? normalized : null;
}

function optionalNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function optionalBoolean(value: unknown): boolean | null {
  return typeof value === 'boolean' ? value : null;
}

export function mapProfile(profile: ProfileDto): ProfileSummary {
  const gameId = profile.game_id ?? profile.gameId ?? null;
  const stockGlobal = profile.built_in && profile.id === 'global';
  return {
    id: profile.id,
    name: profile.name,
    builtIn: profile.built_in,
    scope: stockGlobal ? 'Global' : profile.built_in ? 'Built-in' : gameId ? 'Game' : 'Global',
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
