import type {
  AppSnapshot,
  ControllerConfiguration,
  ControllerProfileAssignment,
  CurrentEffectState,
  ExportedProfile,
  ForzaEffectConfiguration,
  ProfileSummary,
  SteamInputBinding,
  SteamInputLayout,
  SupportedGame
} from '../types';

export const MOCK_CONTROLLER_ID = 'mock-dualsense-edge-1';
export const MOCK_GAME_ID = 'forza-horizon-6';
export const MOCK_APP_ID = 'mock-fh6';
export const MOCK_PROFILE_ID = 'forza-horizon-mock-track';
export const MOCK_LAYOUT_SOURCE = 'mock://steam/userdata/sanitized/config/controller_fh6_edge.vdf';
export const MOCK_EXPORT_SCHEMA = 'dscc.profile.v1';

export type MockEditableControllerConfig = Omit<ControllerConfiguration, 'controllerId' | 'model'>;

export const mockForzaEffects: ForzaEffectConfiguration[] = [
  { id: 'brake_resistance', enabled: true, intensity: 115, route: 'l2' },
  { id: 'abs_slip_pulse', enabled: true, intensity: 105, route: 'l2' },
  { id: 'handbrake_wall', enabled: true, intensity: 95, route: 'l2' },
  { id: 'throttle_resistance', enabled: true, intensity: 120, route: 'r2' },
  { id: 'gear_shift_thump', enabled: true, intensity: 150, route: 'r2_and_body' },
  { id: 'rev_limiter_buzz', enabled: true, intensity: 126, route: 'r2' },
  { id: 'road_texture', enabled: true, intensity: 68, route: 'body_both' },
  { id: 'rumble_strip', enabled: true, intensity: 80, route: 'body_both' },
  { id: 'tire_slip', enabled: true, intensity: 92, route: 'body_right' },
  { id: 'puddle_drag', enabled: true, intensity: 58, route: 'body_left' },
  { id: 'suspension_impact', enabled: true, intensity: 110, route: 'body_both' },
  { id: 'rpm_leds', enabled: false, intensity: 100, route: 'light_led' }
];

export const mockControllerProfileAssignments: ControllerProfileAssignment[] = [
  {
    controllerId: MOCK_CONTROLLER_ID,
    gameId: MOCK_GAME_ID,
    gameName: 'Forza Horizon 6 (mock)',
    profileId: MOCK_PROFILE_ID,
    profileName: 'Mock Horizon Track',
    state: 'active',
    detail: 'mock game detection'
  }
];

export const mockProfiles: ProfileSummary[] = [
  {
    id: 'global',
    name: 'Global',
    builtIn: true,
    scope: 'Global',
    gameId: 'all',
    active: false,
    rules: 8,
    updatedAt: 'built-in'
  },
  {
    id: 'forza-horizon',
    name: 'Base',
    builtIn: true,
    scope: 'Built-in',
    gameId: 'all',
    active: false,
    rules: 12,
    updatedAt: 'built-in'
  },
  {
    id: 'forza-horizon-immersive',
    name: 'Immersive',
    builtIn: true,
    scope: 'Built-in',
    gameId: 'all',
    active: false,
    rules: 12,
    updatedAt: 'built-in'
  },
  {
    id: MOCK_PROFILE_ID,
    name: 'Mock Horizon Track',
    builtIn: false,
    scope: 'Game',
    gameId: MOCK_GAME_ID,
    active: true,
    rules: 12,
    updatedAt: 'mock'
  },
  {
    id: 'mock-city-cruise',
    name: 'Mock City Cruise',
    builtIn: false,
    scope: 'Global',
    gameId: 'all',
    active: false,
    rules: 8,
    updatedAt: 'mock'
  }
];

export const mockControllerConfig: ControllerConfiguration = {
  controllerId: MOCK_CONTROLLER_ID,
  model: 'DualSense Edge',
  inputMode: 'steam_input_companion',
  trigger: {
    sameRange: false,
    l2From: 6,
    l2To: 92,
    r2From: 2,
    r2To: 100,
    l2Curve: 1.45,
    r2Curve: 2.1,
    l2CurvePoints: [
      { input: 0, output: 0 },
      { input: 25, output: 13 },
      { input: 50, output: 37 },
      { input: 75, output: 66 },
      { input: 100, output: 100 }
    ],
    r2CurvePoints: [
      { input: 0, output: 0 },
      { input: 25, output: 5 },
      { input: 50, output: 23 },
      { input: 75, output: 55 },
      { input: 100, output: 100 }
    ],
    effect: 'Adaptive resistance',
    intensity: 'Strong (Standard)',
    vibration: 'Medium',
    vibrationMode: 'Balanced'
  },
  lightbar: {
    enabled: true,
    color: '#3baeff',
    rpmColor: '#ff3a2e',
    brightness: 76
  },
  forza: {
    bodyRumbleMode: 'native_passthrough',
    effects: mockForzaEffects
  },
  sticks: {
    leftCurve: 'Default',
    leftCurveAmount: 50,
    leftDeadzone: 4,
    rightCurve: 'Precision',
    rightCurveAmount: 42,
    rightDeadzone: 3
  },
  buttons: [
    { key: 'cross', label: 'Confirm / Handbrake' },
    { key: 'circle', label: 'Back / Camera' },
    { key: 'l1', label: 'Clutch' },
    { key: 'r1', label: 'Shift up' }
  ],
  profileAssignments: mockControllerProfileAssignments
};

export const mockProfileConfigs: Record<string, MockEditableControllerConfig> = {
  'forza-horizon': editableConfigFromController(mockControllerConfig),
  [MOCK_PROFILE_ID]: editableConfigFromController(mockControllerConfig),
  'mock-city-cruise': {
    ...editableConfigFromController(mockControllerConfig),
    trigger: {
      ...mockControllerConfig.trigger,
      l2Curve: 1.2,
      r2Curve: 1.65,
      l2CurvePoints: [
        { input: 0, output: 0 },
        { input: 30, output: 24 },
        { input: 60, output: 54 },
        { input: 85, output: 82 },
        { input: 100, output: 100 }
      ],
      r2CurvePoints: [
        { input: 0, output: 0 },
        { input: 35, output: 18 },
        { input: 70, output: 56 },
        { input: 90, output: 82 },
        { input: 100, output: 100 }
      ],
      intensity: 'Medium',
      vibration: 'Low',
      vibrationMode: 'Deep thump'
    },
    lightbar: {
      ...mockControllerConfig.lightbar,
      color: '#4ade80',
      brightness: 62
    },
    forza: {
      bodyRumbleMode: 'native_passthrough',
      effects: mockForzaEffects.map((effect) => ({
        ...effect,
        intensity: effect.id === 'rpm_leds' ? effect.intensity : Math.round(effect.intensity * 0.75)
      }))
    }
  }
};

export const mockSupportedGame: SupportedGame = {
  gameId: MOCK_GAME_ID,
  name: 'Forza Horizon 6 (mock)',
  appId: MOCK_APP_ID,
  installPath: 'mock://steam/steamapps/common/ForzaHorizon6',
  installed: true,
  running: true,
  supportLevel: 'telemetry',
  artwork: {
    iconUrl: 'https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1551360/capsule_231x87.jpg',
    bannerUrl: 'https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1551360/header.jpg',
    heroUrl: 'https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1551360/library_hero.jpg',
    capsuleUrl: 'https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1551360/library_600x900.jpg'
  },
  stats: {
    playtimeMinutes: 1860,
    lastPlayedUnix: 1779294000,
    achievements: {
      unlocked: 42,
      total: 72
    }
  }
};

export const mockSteamBindings: SteamInputBinding[] = [
  steamBinding('button_a', 'Face Buttons', 'Four Buttons', 'Full Press', 'xinput_button A, , A Button'),
  steamBinding('button_b', 'Face Buttons', 'Four Buttons', 'Full Press', 'xinput_button B, , B Button'),
  steamBinding('button_x', 'Face Buttons', 'Four Buttons', 'Full Press', 'xinput_button X, , X Button'),
  steamBinding('button_y', 'Face Buttons', 'Four Buttons', 'Full Press', 'xinput_button Y, , Y Button'),
  steamBinding('dpad_north', 'Directional Pad', 'Directional Pad', 'Full Press', 'xinput_button DPAD_UP, , DPad Up'),
  steamBinding('dpad_south', 'Directional Pad', 'Directional Pad', 'Full Press', 'xinput_button DPAD_DOWN, , DPad Down'),
  steamBinding('dpad_west', 'Directional Pad', 'Directional Pad', 'Full Press', 'xinput_button DPAD_LEFT, , DPad Left'),
  steamBinding('dpad_east', 'Directional Pad', 'Directional Pad', 'Full Press', 'xinput_button DPAD_RIGHT, , DPad Right'),
  steamBinding('left_bumper', 'Switches', 'Switches', 'Full Press', 'xinput_button shoulder_left, , Left Bumper'),
  steamBinding('right_bumper', 'Switches', 'Switches', 'Full Press', 'xinput_button shoulder_right, , Right Bumper'),
  steamBinding('click:left_trigger', 'Left Trigger', 'Analog Trigger', 'Full Press', 'xinput_button TRIGGER_LEFT, , Left Trigger'),
  steamBinding('click:right_trigger', 'Right Trigger', 'Analog Trigger', 'Full Press', 'xinput_button TRIGGER_RIGHT, , Right Trigger'),
  steamBinding('button_menu', 'Switches', 'Switches', 'Full Press', 'xinput_button select, , Select'),
  steamBinding('button_escape', 'Switches', 'Switches', 'Full Press', 'xinput_button start, , Start'),
  steamBinding('click:left_joystick', 'Left Joystick', 'Joystick', 'Full Press', 'xinput_button JOYSTICK_LEFT, , Left Stick Click'),
  steamBinding('click:right_joystick', 'Right Joystick', 'Joystick', 'Full Press', 'xinput_button JOYSTICK_RIGHT, , Right Stick Click'),
  steamBinding('click:left_trackpad', 'Left Trackpad', 'Single Button', 'Full Press', 'xinput_button SELECT, , Select'),
  steamBinding('click:right_trackpad', 'Right Trackpad', 'Single Button', 'Full Press', 'xinput_button START, , Start'),
  steamBinding('dpad_north', 'Center Trackpad', 'Directional Swipe', 'Full Press', 'key_press EQUALS, , = Key'),
  steamBinding('dpad_south', 'Center Trackpad', 'Directional Swipe', 'Full Press', 'key_press DASH, , - Key'),
  steamBinding('button_back_left', 'Switches', 'Switches', 'Full Press', 'key_press Q, , Q Key'),
  steamBinding('button_back_right', 'Switches', 'Switches', 'Full Press', 'key_press E, , E Key'),
  steamBinding('button_back_left_upper', 'Switches', 'Switches', 'Full Press', 'key_press M, , M Key'),
  steamBinding('button_back_right_upper', 'Switches', 'Switches', 'Full Press', 'xinput_button B, , B Button')
];

export const mockSteamLayout: SteamInputLayout = {
  appId: MOCK_APP_ID,
  title: 'Mock Horizon DualSense Edge Layout',
  controllerType: 'controller_ps5_edge',
  controllerLabel: 'DualSense Edge',
  source: MOCK_LAYOUT_SOURCE,
  bindingCount: mockSteamBindings.length,
  bindings: mockSteamBindings
};

export const mockEffectState: CurrentEffectState = {
  controllerId: MOCK_CONTROLLER_ID,
  selectedProfileId: MOCK_PROFILE_ID,
  selectedProfileName: 'Mock Horizon Track',
  reason: 'mock profile resolution',
  dryRun: true,
  hardwareOutputEnabled: false,
  output: {
    l2: { type: 'adaptive_resistance', start_position: 25, strength: 150 },
    r2: { type: 'adaptive_resistance', start_position: 12, strength: 170 },
    lightbar: { color: { red: 59, green: 174, blue: 255 }, brightness: 76 },
    playerLeds: { count: 3 },
    rumble: { low_frequency: 70, high_frequency: 95 }
  },
  parityEffects: mockForzaEffects.map((effect) => ({
    id: effect.id,
    target: effect.route,
    label: effect.id.replaceAll('_', ' '),
    signal: signalForEffect(effect.id),
    state: effect.enabled ? 'active' : 'disabled'
  })),
  warnings: ['Mock mode is active; no HID, Steam, Forza, or agent writes will be attempted.']
};

export const mockAppSnapshot: AppSnapshot = {
  status: {
    version: 'mock-dev',
    uptime: '14m 20s',
    bindAddress: 'mock://api',
    mode: 'agent',
    health: 'running',
    activeProfile: 'Mock Horizon Track',
    activeAdapter: 'Forza Data Out'
  },
  appSettings: {
    settings: {
      listenOnAllInterfaces: false,
      forzaPlaystationGlyphs: {
        enabled: true,
        installPath: 'mock://steam/steamapps/common/ForzaHorizon6',
        lastStatus: 'installed',
        lastMessage: 'Mock PlayStation glyph override is enabled.'
      }
    },
    effectiveBindAddress: 'mock://api',
    desiredBindAddress: 'mock://api',
    restartRequired: false
  },
  controllers: [
    {
      id: MOCK_CONTROLLER_ID,
      name: 'Mock DualSense Edge',
      family: 'DualSense Edge',
      transport: 'Bluetooth',
      connected: true,
      battery: 84,
      batteryState: 'discharging',
      charging: false,
      permission: 'granted',
      diagnosticState: 'ok',
      capabilities: ['adaptive triggers', 'lightbar', 'player leds', 'rumble']
    }
  ],
  profiles: mockProfiles,
  controllerProfileAssignments: mockControllerProfileAssignments,
  adapters: [
    {
      id: 'forza-data-out',
      name: 'Forza Data Out',
      state: 'running',
      packetRateHz: 60,
      config: 'mock telemetry / local fixture',
      setupHint: 'Mock telemetry stream is active.'
    },
    {
      id: 'steam-input',
      name: 'Steam Input',
      state: 'running',
      packetRateHz: 0,
      config: 'mock companion layout',
      setupHint: 'Mock Steam layout is loaded from the in-memory fixture.'
    }
  ],
  modules: [
    {
      id: 'forza-data-out',
      name: 'Forza Data Out',
      version: 'mock',
      source: 'built_in',
      kind: 'adapter',
      trusted: true,
      protocol: 'udp',
      setupHint: 'Mock Data Out packets are synthesized for UI development.',
      setupUrl: null,
      profileTemplates: []
    }
  ],
  steamInput: {
    running: true,
    available: true,
    steamPath: 'mock://steam',
    layouts: [mockSteamLayout],
    warnings: ['Using an in-memory mock Steam Input layout.']
  },
  gameDetection: {
    activeGameId: MOCK_GAME_ID,
    activeGameName: 'Forza Horizon 6 (mock)',
    source: 'mock',
    confidence: 1,
    processName: 'ForzaHorizon6.exe',
    moduleId: MOCK_GAME_ID,
    adapterId: 'forza-data-out',
    profileId: MOCK_PROFILE_ID,
    supportedGames: [mockSupportedGame],
    selectedGame: mockSupportedGame,
    candidates: [
      {
        gameId: MOCK_GAME_ID,
        name: 'Forza Horizon 6 (mock)',
        processName: 'ForzaHorizon6.exe',
        moduleId: MOCK_GAME_ID,
        adapterId: 'forza-data-out',
        profileId: MOCK_PROFILE_ID,
        confidence: 1
      }
    ]
  },
  profileResolution: {
    controllerId: MOCK_CONTROLLER_ID,
    detectedGameId: MOCK_GAME_ID,
    activeAdapterId: 'forza-data-out',
    selectedProfileId: MOCK_PROFILE_ID,
    reason: 'mock_game_detected',
    overrideProfileId: null,
    validation: 'ok'
  },
  effectState: mockEffectState,
  telemetry: [
    { name: 'input.brake', value: 0.34, updatedMsAgo: 12 },
    { name: 'input.throttle', value: 0.68, updatedMsAgo: 12 },
    { name: 'input.handbrake', value: 0, updatedMsAgo: 12 },
    { name: 'wheel.slip.front_max', value: 0.18, updatedMsAgo: 12 },
    { name: 'wheel.slip.max', value: 0.22, updatedMsAgo: 12 },
    { name: 'surface.rumble.max', value: 0.41, updatedMsAgo: 12 },
    { name: 'surface.rumble_strip.max', value: 0.08, updatedMsAgo: 12 },
    { name: 'surface.puddle.max', value: 0.03, updatedMsAgo: 12 },
    { name: 'vehicle.acceleration.magnitude', value: 1.42, unit: 'g', updatedMsAgo: 12 },
    { name: 'vehicle.rpm_ratio', value: 0.72, updatedMsAgo: 12 },
    { name: 'drivetrain.shift_pulse', value: false, updatedMsAgo: 12 }
  ],
  logs: [
    { level: 'info', time: '12:00:00', source: 'mock', message: 'Mock DSCC snapshot loaded.' },
    { level: 'info', time: '12:00:01', source: 'mock', message: 'Mock Steam Input layout ready.' },
    { level: 'debug', time: '12:00:02', source: 'mock', message: 'Mock telemetry stream running at 60 Hz.' }
  ],
  diagnostics: [
    { label: 'Agent', state: 'pass', detail: 'Mock agent state is available in-browser.' },
    { label: 'Controller', state: 'pass', detail: 'Mock DualSense Edge is connected.' },
    { label: 'Steam Input', state: 'pass', detail: 'Mock layout contains editable bindings.' },
    { label: 'Forza Data Out', state: 'pass', detail: 'Mock telemetry signals are populated.' }
  ],
  partialErrors: []
};

export const mockExportedProfiles: Record<string, ExportedProfile> = Object.fromEntries(
  mockProfiles.map((profile) => [
    profile.id,
    {
      schema: MOCK_EXPORT_SCHEMA,
      id: profile.id,
      name: profile.name,
      built_in: profile.builtIn,
      builtIn: profile.builtIn,
      game_id: profile.scope === 'Game' ? profile.gameId : null,
      gameId: profile.scope === 'Game' ? profile.gameId : null,
      active: profile.active,
      config: mockProfileConfigs[profile.id] ?? editableConfigFromController(mockControllerConfig)
    }
  ])
);

function editableConfigFromController(config: ControllerConfiguration): MockEditableControllerConfig {
  return {
    inputMode: config.inputMode,
    trigger: config.trigger,
    lightbar: config.lightbar,
    forza: config.forza,
    sticks: config.sticks,
    buttons: config.buttons,
    profileAssignments: config.profileAssignments
  };
}

function steamBinding(
  inputId: string,
  source: string,
  sourceMode: string,
  activator: string,
  rawBinding: string
): SteamInputBinding {
  return {
    input: inputId,
    inputId,
    binding: bindingLabel(rawBinding),
    rawBinding,
    kind: 'button',
    source,
    sourceMode,
    activator,
    groupId: source.toLowerCase().replaceAll(/\s+/g, '_')
  };
}

function bindingLabel(rawBinding: string): string {
  const parts = rawBinding.split(',');
  const label = parts.slice(2).join(',').trim();
  if (label) return label;
  return parts[0]?.trim() || rawBinding;
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
