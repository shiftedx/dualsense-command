export type HealthState = 'ready' | 'running' | 'needs_setup' | 'unavailable' | 'faulted';

interface AgentStatus {
  version: string;
  uptime: string;
  bindAddress: string;
  mode: 'agent';
  health: HealthState;
  activeProfile: string;
  activeAdapter: string;
}

export interface AppSettingsResponse {
  settings: AppSettings;
  effectiveBindAddress: string;
  desiredBindAddress: string;
  restartRequired: boolean;
}

interface AppSettings {
  listenOnAllInterfaces: boolean;
  forzaPlaystationGlyphs: ForzaGlyphOverrideSettings;
}

interface ForzaGlyphOverrideSettings {
  enabled: boolean;
  installPath?: string | null;
  lastStatus: string;
  lastMessage: string;
}

export interface ControllerStatus {
  id: string;
  name: string;
  family: 'DualSense' | 'DualSense Edge' | 'Unknown Sony';
  transport: 'USB' | 'Bluetooth' | 'Unknown';
  connected: boolean;
  battery: number | null;
  batteryState: 'unknown' | 'discharging' | 'charging' | 'full';
  charging: boolean;
  permission: 'unknown' | 'granted' | 'denied';
  diagnosticState:
    | 'ok'
    | 'disconnected'
    | 'permission_denied'
    | 'cannot_open'
    | 'unsupported'
    | 'faulted'
    | 'unknown';
  capabilities: string[];
}

export interface ProfileSummary {
  id: string;
  name: string;
  builtIn: boolean;
  scope: 'Built-in' | 'Global' | 'Game';
  gameId: string;
  active: boolean;
  rules: number;
  updatedAt: string;
}

export interface ControllerProfileAssignment {
  controllerId: string;
  gameId: string;
  gameName: string;
  profileId: string;
  profileName: string;
  state: 'active' | 'ready' | 'manual';
  detail: string;
}

export interface AdapterStatus {
  id: string;
  name: string;
  state: HealthState;
  packetRateHz: number;
  config: string;
  setupHint: string;
}

export interface ModuleSummary {
  id: string;
  name: string;
  kind: 'adapter' | 'game' | string;
  version: string;
  source: 'built_in' | 'community' | string;
  trusted: boolean;
  protocol: string;
  setupHint: string;
  setupUrl?: string | null;
  profileTemplates: string[];
}

export interface ProfileResolution {
  controllerId?: string | null;
  detectedGameId?: string | null;
  activeAdapterId?: string | null;
  selectedProfileId?: string | null;
  reason: string;
  overrideProfileId?: string | null;
  validation: string;
}

export interface GameDetection {
  activeGameId?: string | null;
  activeGameName?: string | null;
  source: string;
  confidence: number;
  processName?: string | null;
  moduleId?: string | null;
  adapterId?: string | null;
  profileId?: string | null;
  supportedGames?: SupportedGame[];
  selectedGame?: SupportedGame | null;
  candidates: GameDetectionCandidate[];
}

export interface SupportedGame {
  gameId: string;
  name: string;
  source?: 'built_in' | 'steam' | 'local_app' | string;
  inputProvider?: 'native_dualsense' | 'steam_input' | 'dscc_input_bridge' | string;
  appId?: string | null;
  installPath?: string | null;
  processNames?: string[];
  executableName?: string | null;
  installed: boolean;
  running: boolean;
  supportLevel: 'telemetry' | 'custom';
  artwork?: {
    iconUrl?: string | null;
    bannerUrl?: string | null;
    heroUrl?: string | null;
    capsuleUrl?: string | null;
  };
  stats?: SteamGameStats;
}

export interface SteamLibraryEntry {
  appId: string;
  name: string;
  installDir: string;
  installPath: string;
  artwork?: {
    iconUrl?: string | null;
    bannerUrl?: string | null;
    heroUrl?: string | null;
    capsuleUrl?: string | null;
  };
  stats?: SteamGameStats;
  alreadyInCatalog: boolean;
  suggestedGameId: string;
  processCandidates: string[];
}

export interface SteamLibraryResponse {
  games: SteamLibraryEntry[];
}

export interface AddCustomGameRequest {
  appId: string;
  processNames?: string[];
}

export interface AddCustomGameResponse {
  game: SupportedGame;
}

export interface ValidateLocalAppRequest {
  name?: string | null;
  executablePath: string;
  processNames?: string[];
}

export interface ValidateLocalAppResponse {
  valid: boolean;
  name: string;
  executableName: string;
  processNames: string[];
  warnings: string[];
}

export interface AddLocalAppRequest {
  name: string;
  executablePath: string;
  processNames?: string[];
}

export interface SteamLibraryBrowseEntry {
  name: string;
  kind: 'dir' | 'exe' | string;
  sizeBytes?: number | null;
}

export interface SteamLibraryBrowseResponse {
  appId: string;
  installPath: string;
  relativePath: string;
  entries: SteamLibraryBrowseEntry[];
  truncated: boolean;
}

interface SteamGameStats {
  playtimeMinutes?: number | null;
  lastPlayedUnix?: number | null;
  achievements?: {
    unlocked: number;
    total: number;
  } | null;
}

export interface ExportedProfile {
  schema: string;
  id: string;
  name: string;
  built_in?: boolean;
  builtIn?: boolean;
  game_id?: string | null;
  gameId?: string | null;
  active?: boolean;
  config?: ProfileConfigPayload | null;
}

interface ProfileConfigPayload {
  inputMode: ControllerInputMode;
  trigger: TriggerConfiguration;
  lightbar: LightbarConfiguration;
  forza: ForzaTelemetryConfiguration;
  sticks: StickConfiguration;
  buttons: ButtonAssignmentConfiguration[];
  inputBridge?: InputBridgeConfig;
}

interface GameDetectionCandidate {
  gameId: string;
  name: string;
  processName: string;
  moduleId: string;
  adapterId: string;
  profileId: string;
  confidence: number;
}

export interface TelemetrySignal {
  name: string;
  value: string | number | boolean;
  unit?: string;
  updatedMsAgo: number;
}

export interface LogEntry {
  level: 'trace' | 'debug' | 'info' | 'warn' | 'error';
  time: string;
  source: string;
  message: string;
}

export interface DiagnosticsCheck {
  label: string;
  state: 'pass' | 'warn' | 'pending';
  detail: string;
}

export interface SnapshotPartialError {
  endpoint: string;
  message: string;
}

export interface AppUpdateCheck {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  releaseUrl: string;
  source: 'agent' | 'github';
  checkedAt?: string | null;
  message?: string | null;
}

export interface AppSnapshot {
  status: AgentStatus;
  appSettings: AppSettingsResponse;
  controllers: ControllerStatus[];
  profiles: ProfileSummary[];
  controllerProfileAssignments: ControllerProfileAssignment[];
  adapters: AdapterStatus[];
  modules: ModuleSummary[];
  steamInput: SteamInputStatus;
  inputBridge: InputBridgeStatus;
  gameDetection: GameDetection;
  profileResolution: ProfileResolution;
  effectState: CurrentEffectState;
  telemetry: TelemetrySignal[];
  logs: LogEntry[];
  diagnostics: DiagnosticsCheck[];
  partialErrors: SnapshotPartialError[];
}

export interface SupportBundle {
  schema?: string;
  generatedAt?: string;
  source?: string;
  privacy?: unknown;
  [key: string]: unknown;
}

export interface EffectTestRequest {
  target: 'l2' | 'r2' | 'base_feel' | 'lightbar' | 'rumble';
  mode: string;
  intensity: number;
  startPosition?: number;
  l2Position?: number;
  r2Position?: number;
  durationMs: number;
  trigger?: TriggerConfiguration;
}

export interface ControllerConfiguration {
  controllerId: string;
  model: string;
  inputMode: ControllerInputMode;
  trigger: TriggerConfiguration;
  lightbar: LightbarConfiguration;
  forza: ForzaTelemetryConfiguration;
  sticks: StickConfiguration;
  buttons: ButtonAssignmentConfiguration[];
  inputBridge: InputBridgeConfig;
  profileAssignments: ProfileAssignmentConfiguration[];
}

export interface ActionAccepted {
  accepted: boolean;
  message: string;
  dryRun?: boolean;
}

export type EdgeProfileSupportState = 'unsupported' | 'unknown' | 'read_only' | 'read_write' | 'blocked';
export type EdgeProfileSlotState = 'default' | 'assigned' | 'empty' | 'active' | 'unknown' | 'faulted';

export interface EdgeProfilesResponse {
  controllerId: string;
  supportState: EdgeProfileSupportState;
  warning: string;
  slots: EdgeProfileSlot[];
}

export interface EdgeProfileSlot {
  slotId: string;
  shortcut: string;
  name?: string | null;
  state: EdgeProfileSlotState;
  editable: boolean;
  hardwareSynced: boolean;
  staged?: EdgeProfileSlotConfig | null;
}

export interface EdgeProfileSlotConfig {
  name: string;
  trigger: ControllerConfiguration['trigger'];
  lightbar: ControllerConfiguration['lightbar'];
  sticks: ControllerConfiguration['sticks'];
  buttons: ControllerConfiguration['buttons'];
  updatedAt: string;
  hardwareSynced: boolean;
}

export interface UpdateEdgeProfileRequest {
  name: string;
  trigger: ControllerConfiguration['trigger'];
  lightbar: ControllerConfiguration['lightbar'];
  sticks: ControllerConfiguration['sticks'];
  buttons: ControllerConfiguration['buttons'];
}

export type ControllerInputMode = 'native_dualsense' | 'steam_input_companion' | 'dscc_input_bridge';

export interface InputBridgeConfig {
  enabled: boolean;
  outputKind: 'xbox360' | string;
  autoStart: boolean;
  hidePhysical: boolean;
  bindings: InputBridgeBindingConfig[];
}

export interface InputBridgeBindingConfig {
  source: unknown;
  target: unknown;
}

export interface InputBridgeStatus {
  available: boolean;
  backendId: string;
  provider: string;
  state: 'available' | 'unavailable' | 'faulted' | string;
  message: string;
  supportedKinds: string[];
  sessions: InputBridgeSessionSummary[];
  warnings: string[];
}

export interface InputBridgeSessionSummary {
  controllerId: string;
  state: 'disabled' | 'starting' | 'ready' | 'active' | 'stale' | 'stopping' | 'faulted';
  sessionId?: string | null;
  outputKind?: string | null;
  message: string;
  updatedAtMs: number;
}

export interface InputBridgeBindingWriteRequest {
  controllerId?: string | null;
  profileId?: string | null;
  inputId: string;
  target: string;
  dryRun?: boolean;
}

export interface InputBridgeBindingWriteResponse {
  accepted: boolean;
  message: string;
  dryRun: boolean;
  warnings: string[];
}

export interface ControllerInputState {
  controllerId: string;
  available: boolean;
  source: 'hid' | 'mock' | string;
  message: string;
  sampledAtMs: number | null;
  ageMs: number | null;
  axes: {
    leftStick: ControllerInputStickState;
    rightStick: ControllerInputStickState;
  };
  triggers: {
    l2: number;
    r2: number;
  };
  buttons: ControllerInputButtonState[];
}

export interface ControllerInputStickState {
  x: number;
  y: number;
  magnitude: number;
}

export interface ControllerInputButtonState {
  id: string;
  label: string;
  pressed: boolean;
  value: number;
}

interface TriggerConfiguration {
  sameRange: boolean;
  l2From: number;
  l2To: number;
  r2From: number;
  r2To: number;
  l2Curve: number;
  r2Curve: number;
  l2CurvePoints: TriggerCurvePoint[];
  r2CurvePoints: TriggerCurvePoint[];
  effect: string;
  intensity: string;
  vibration: string;
  vibrationMode: string;
}

export interface TriggerCurvePoint {
  input: number;
  output: number;
}

interface LightbarConfiguration {
  enabled: boolean;
  color: string;
  rpmColor: string;
  brightness: number;
}

interface ForzaTelemetryConfiguration {
  bodyRumbleMode?: ForzaBodyRumbleMode;
  effects: ForzaEffectConfiguration[];
}

export type ForzaBodyRumbleMode = 'native_passthrough' | 'dscc_full_control';

export interface ForzaEffectConfiguration {
  id: string;
  enabled: boolean;
  intensity: number;
  route: ForzaEffectRoute;
}

export type ForzaEffectRoute =
  | 'body_both'
  | 'body_left'
  | 'body_right'
  | 'l2'
  | 'r2'
  | 'both_triggers'
  | 'body_and_triggers'
  | 'r2_and_body'
  | 'light_led';

interface StickConfiguration {
  leftCurve: string;
  leftCurveAmount: number;
  leftDeadzone: number;
  rightCurve: string;
  rightCurveAmount: number;
  rightDeadzone: number;
}

interface ButtonAssignmentConfiguration {
  key: string;
  label: string;
}

export interface ProfileAssignmentConfiguration {
  gameId: string;
  gameName: string;
  profileId: string;
  profileName: string;
  state: 'active' | 'ready' | 'manual';
  detail: string;
}

export interface SteamInputStatus {
  running: boolean;
  available: boolean;
  steamPath?: string | null;
  layouts: SteamInputLayout[];
  warnings: string[];
}

export interface SteamInputLayout {
  appId?: string | null;
  title: string;
  controllerType?: string | null;
  controllerLabel?: string | null;
  source: string;
  bindingCount: number;
  bindings: SteamInputBinding[];
}

export interface SteamInputBinding {
  input: string;
  inputId: string;
  binding: string;
  rawBinding: string;
  kind: string;
  source?: string | null;
  sourceMode?: string | null;
  activator?: string | null;
  groupId?: string | null;
  synthetic?: boolean;
}

export interface SteamInputBindingWriteRequest {
  layoutSource: string;
  appId?: string | null;
  inputId: string;
  groupId?: string | null;
  activator?: string | null;
  rawBinding: string;
  profileName?: string | null;
  dryRun?: boolean;
}

export interface SteamInputBindingWriteResponse {
  accepted: boolean;
  message: string;
  dryRun: boolean;
  source: string;
  targetPath: string;
  backupPath?: string | null;
  binding: SteamInputBinding;
  warnings: string[];
}

export interface SteamInputPaddlePresetRequest {
  layoutSource: string;
  appId?: string | null;
  leftKey?: string | null;
  rightKey?: string | null;
  profileName?: string | null;
  dryRun?: boolean;
}

export interface SteamInputPaddlePresetResponse {
  accepted: boolean;
  message: string;
  dryRun: boolean;
  source: string;
  targetPath: string;
  backupPath?: string | null;
  paddles: SteamInputPaddlePresetPaddleResult[];
  warnings: string[];
}

export interface SteamInputPaddlePresetPaddleResult {
  paddle: string;
  inputId: string;
  key: string;
  rawBinding: string;
  changed: boolean;
  binding: SteamInputBinding;
  message: string;
}

export interface CurrentEffectState {
  controllerId?: string | null;
  selectedProfileId?: string | null;
  selectedProfileName?: string | null;
  reason: string;
  dryRun: boolean;
  hardwareOutputEnabled: boolean;
  output: ControllerOutputFrame;
  parityEffects: EffectMappingStatus[];
  warnings: string[];
}

export interface ControllerOutputFrame {
  l2: TriggerOutput;
  r2: TriggerOutput;
  lightbar?: LightbarOutput | null;
  playerLeds?: PlayerLedsOutput | null;
  rumble?: RumbleOutput | null;
}

type TriggerOutput =
  | { type: 'off' }
  | { type: 'adaptive_resistance'; start_position: number; strength: number }
  | { type: 'pulse'; amplitude: number; frequency_hz: number }
  | { type: 'pulse_ab'; strength: number; frequency_hz: number; wall_zones: number }
  | { type: 'wall'; position: number; strength: number };

interface LightbarOutput {
  color: { red: number; green: number; blue: number };
  brightness: number;
}

interface PlayerLedsOutput {
  count: number;
}

interface RumbleOutput {
  low_frequency: number;
  high_frequency: number;
}

interface EffectMappingStatus {
  id: string;
  target: string;
  label: string;
  signal: string;
  state: 'active' | 'ready' | string;
}
