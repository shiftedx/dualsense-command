import {
  defaultTriggerCurve,
  defaultTriggerCurvePoints,
  normalizeStickDeadzone,
  normalizeTriggerCurve,
  normalizeTriggerCurvePoints,
  normalizeTriggerPercent
} from '../lib/features/haptics/hapticsModel';
import type {
  ControllerConfiguration,
  ExportedProfile,
  ForzaAbsTuningConfiguration,
  ForzaBrakeTuningConfiguration,
  ForzaBodyRumbleMode,
  ForzaEffectConfiguration,
  ForzaRevLimiterTuningConfiguration,
  ForzaShiftTuningConfiguration,
  ForzaThrottleTuningConfiguration,
  InputBridgeConfig,
  ProfileAssignmentConfiguration
} from '../lib/types';

export type EditableControllerConfig = Omit<ControllerConfiguration, 'controllerId' | 'model'>;

export type ProfileDraftValues = {
  l2From: number;
  l2To: number;
  r2From: number;
  r2To: number;
  l2Curve: number;
  r2Curve: number;
  l2CurvePoints: ControllerConfiguration['trigger']['l2CurvePoints'];
  r2CurvePoints: ControllerConfiguration['trigger']['r2CurvePoints'];
  triggerEffect: string;
  triggerIntensity: string;
  vibrationIntensity: string;
  vibrationMode: string;
  lightbarEnabled: boolean;
  lightbarColor: string;
  rpmColor: string;
  lightbarBrightness: number;
  forzaBodyRumbleMode: ForzaBodyRumbleMode;
  forzaEffects: ForzaEffectConfiguration[];
  forzaBrakeTuning: ForzaBrakeTuningConfiguration;
  forzaAbsTuning: ForzaAbsTuningConfiguration;
  forzaThrottleTuning: ForzaThrottleTuningConfiguration;
  forzaShiftTuning: ForzaShiftTuningConfiguration;
  forzaRevLimiterTuning: ForzaRevLimiterTuningConfiguration;
  leftStickDeadzone: number;
  rightStickDeadzone: number;
};

type DraftConfigOptions = {
  isEdge?: boolean;
  defaultForzaEffects: ForzaEffectConfiguration[];
  defaultForzaBrakeTuning: ForzaBrakeTuningConfiguration;
  defaultForzaAbsTuning: ForzaAbsTuningConfiguration;
  defaultForzaThrottleTuning: ForzaThrottleTuningConfiguration;
  defaultForzaShiftTuning: ForzaShiftTuningConfiguration;
  defaultForzaRevLimiterTuning: ForzaRevLimiterTuningConfiguration;
  profileAssignments?: ProfileAssignmentConfiguration[];
};

type DraftBuildOptions = {
  isEdge?: boolean;
  normalizeForzaEffects: (effects: ForzaEffectConfiguration[] | undefined) => ForzaEffectConfiguration[];
  normalizeForzaBrakeTuning: (
    tuning: Partial<ForzaBrakeTuningConfiguration> | undefined | null
  ) => ForzaBrakeTuningConfiguration;
  normalizeForzaAbsTuning: (
    tuning: Partial<ForzaAbsTuningConfiguration> | undefined | null
  ) => ForzaAbsTuningConfiguration;
  normalizeForzaThrottleTuning: (
    tuning: Partial<ForzaThrottleTuningConfiguration> | undefined | null
  ) => ForzaThrottleTuningConfiguration;
  normalizeForzaShiftTuning: (
    tuning: Partial<ForzaShiftTuningConfiguration> | undefined | null
  ) => ForzaShiftTuningConfiguration;
  normalizeForzaRevLimiterTuning: (
    tuning: Partial<ForzaRevLimiterTuningConfiguration> | undefined | null
  ) => ForzaRevLimiterTuningConfiguration;
};

type ProfileConfigSignatureOptions = DraftBuildOptions & {
  forzaIntensityPercent: (intensity: number) => number;
};

type ForzaTuningDefaults = {
  effects: ForzaEffectConfiguration[];
  brake: ForzaBrakeTuningConfiguration;
  abs: ForzaAbsTuningConfiguration;
  throttle: ForzaThrottleTuningConfiguration;
  shift: ForzaShiftTuningConfiguration;
  revLimiter: ForzaRevLimiterTuningConfiguration;
};

type ForzaTuningNormalizers = {
  effects: DraftBuildOptions['normalizeForzaEffects'];
  brake: DraftBuildOptions['normalizeForzaBrakeTuning'];
  abs: DraftBuildOptions['normalizeForzaAbsTuning'];
  throttle: DraftBuildOptions['normalizeForzaThrottleTuning'];
  shift: DraftBuildOptions['normalizeForzaShiftTuning'];
  revLimiter: DraftBuildOptions['normalizeForzaRevLimiterTuning'];
};

const cloneForzaEffects = (effects: ForzaEffectConfiguration[]): ForzaEffectConfiguration[] =>
  effects.map((effect) => ({ ...effect }));

const cloneForzaTuning = <T extends object>(tuning: T): T => ({
  ...tuning
});

const forzaTuningDefaultsFromOptions = (options: DraftConfigOptions): ForzaTuningDefaults => ({
  effects: options.defaultForzaEffects,
  brake: options.defaultForzaBrakeTuning,
  abs: options.defaultForzaAbsTuning,
  throttle: options.defaultForzaThrottleTuning,
  shift: options.defaultForzaShiftTuning,
  revLimiter: options.defaultForzaRevLimiterTuning
});

const forzaTuningNormalizersFromOptions = (options: DraftBuildOptions): ForzaTuningNormalizers => ({
  effects: options.normalizeForzaEffects,
  brake: options.normalizeForzaBrakeTuning,
  abs: options.normalizeForzaAbsTuning,
  throttle: options.normalizeForzaThrottleTuning,
  shift: options.normalizeForzaShiftTuning,
  revLimiter: options.normalizeForzaRevLimiterTuning
});

const defaultForzaTelemetryConfig = (
  defaults: ForzaTuningDefaults,
  effects: ForzaEffectConfiguration[] = defaults.effects
): EditableControllerConfig['forza'] => ({
  bodyRumbleMode: 'native_passthrough',
  effects: cloneForzaEffects(effects),
  brake: cloneForzaTuning(defaults.brake),
  abs: cloneForzaTuning(defaults.abs),
  throttle: cloneForzaTuning(defaults.throttle),
  shift: cloneForzaTuning(defaults.shift),
  revLimiter: cloneForzaTuning(defaults.revLimiter)
});

const normalizeForzaTelemetryConfig = (
  config: Partial<EditableControllerConfig['forza']> | undefined | null,
  normalizers: ForzaTuningNormalizers
): EditableControllerConfig['forza'] => ({
  bodyRumbleMode: normalizeForzaBodyRumbleMode(config?.bodyRumbleMode),
  effects: normalizers.effects(config?.effects),
  brake: normalizers.brake(config?.brake),
  abs: normalizers.abs(config?.abs),
  throttle: normalizers.throttle(config?.throttle),
  shift: normalizers.shift(config?.shift),
  revLimiter: normalizers.revLimiter(config?.revLimiter)
});

const forzaTelemetryFromDraft = (
  draft: ProfileDraftValues,
  normalizers: ForzaTuningNormalizers
): EditableControllerConfig['forza'] =>
  normalizeForzaTelemetryConfig(
    {
      bodyRumbleMode: draft.forzaBodyRumbleMode,
      effects: draft.forzaEffects,
      brake: draft.forzaBrakeTuning,
      abs: draft.forzaAbsTuning,
      throttle: draft.forzaThrottleTuning,
      shift: draft.forzaShiftTuning,
      revLimiter: draft.forzaRevLimiterTuning
    },
    normalizers
  );

export const normalizeForzaBodyRumbleMode = (mode: string | undefined | null): ForzaBodyRumbleMode =>
  mode === 'dscc_full_control' ? 'dscc_full_control' : 'native_passthrough';

export const defaultButtonAssignments = (edge = false): EditableControllerConfig['buttons'] => [
  { key: 'Cross', label: 'Cross' },
  { key: 'Circle', label: 'Circle' },
  { key: 'Square', label: 'Square' },
  { key: 'Triangle', label: 'Triangle' },
  { key: 'D-Pad', label: 'D-Pad' },
  { key: 'L1', label: 'L1' },
  { key: 'R1', label: 'R1' },
  { key: 'L2', label: 'L2' },
  { key: 'R2', label: 'R2' },
  { key: 'L3', label: 'L3' },
  { key: 'R3', label: 'R3' },
  { key: 'Create', label: 'Create' },
  { key: 'Options', label: 'Options' },
  { key: 'Touch Pad', label: 'Touch Pad Press' },
  { key: 'Mute', label: 'Mute' },
  ...(edge
    ? [
        { key: 'Back Left', label: 'L3' },
        { key: 'Back Right', label: 'R3' },
        { key: 'Fn Left', label: 'Previous DSCC Profile' },
        { key: 'Fn Right', label: 'Next DSCC Profile' }
      ]
    : [])
];

const canonicalButtonKey = (key: string) => {
  const trimmed = key.trim();
  const canonicalAliases: Record<string, string> = {
    cross: 'Cross',
    circle: 'Circle',
    square: 'Square',
    triangle: 'Triangle',
    dpad: 'D-Pad',
    dpadUp: 'D-Pad Up',
    dpadDown: 'D-Pad Down',
    dpadLeft: 'D-Pad Left',
    dpadRight: 'D-Pad Right',
    l1: 'L1',
    r1: 'R1',
    l2: 'L2',
    r2: 'R2',
    l3: 'L3',
    r3: 'R3',
    create: 'Create',
    options: 'Options',
    touchPad: 'Touch Pad',
    mute: 'Mute',
    edgeBackLeft: 'Back Left',
    edgeBackRight: 'Back Right',
    edgeFnLeft: 'Fn Left',
    edgeFnRight: 'Fn Right'
  };
  return canonicalAliases[trimmed] ?? trimmed;
};

export const normalizeButtonAssignments = (
  buttons: EditableControllerConfig['buttons'] | undefined,
  edge = false
): EditableControllerConfig['buttons'] => {
  const byKey = new Map(
    (buttons ?? [])
      .map((button) => ({
        key: canonicalButtonKey(button.key ?? ''),
        label: (button.label ?? '').trim()
      }))
      .filter((button) => button.key)
      .map((button) => [button.key, button])
  );
  const defaults = defaultButtonAssignments(edge);
  const ordered = defaults.map((button) => byKey.get(button.key) ?? button);
  const defaultKeys = new Set(defaults.map((button) => button.key));
  const extras = [...byKey.values()]
    .filter((button) => !defaultKeys.has(button.key))
    .slice(0, Math.max(0, 24 - ordered.length));
  return [...ordered, ...extras];
};

export const defaultInputBridgeConfig = (): InputBridgeConfig => ({
  enabled: false,
  outputKind: 'xbox360',
  autoStart: false,
  bindings: []
});

export const normalizeInputBridgeConfig = (config: InputBridgeConfig | undefined | null): InputBridgeConfig => ({
  ...defaultInputBridgeConfig(),
  ...config,
  bindings: Array.isArray(config?.bindings) ? config.bindings : []
});

export function baseForzaTriggerDefaults(): EditableControllerConfig['trigger'] {
  return {
    sameRange: false,
    l2From: 6,
    l2To: 100,
    r2From: 4,
    r2To: 100,
    l2Curve: defaultTriggerCurve('l2'),
    r2Curve: defaultTriggerCurve('r2'),
    l2CurvePoints: defaultTriggerCurvePoints('l2'),
    r2CurvePoints: defaultTriggerCurvePoints('r2'),
    effect: 'Adaptive resistance',
    intensity: 'Strong (Standard)',
    vibration: 'Medium',
    vibrationMode: 'Balanced'
  };
}

export function buildDefaultControllerConfig(options: DraftConfigOptions): EditableControllerConfig {
  const forzaDefaults = forzaTuningDefaultsFromOptions(options);

  return {
    inputMode: 'native_dualsense',
    trigger: {
      sameRange: false,
      l2From: 6,
      l2To: 100,
      r2From: 0,
      r2To: 100,
      l2Curve: defaultTriggerCurve('l2'),
      r2Curve: defaultTriggerCurve('r2'),
      l2CurvePoints: defaultTriggerCurvePoints('l2'),
      r2CurvePoints: defaultTriggerCurvePoints('r2'),
      effect: 'Adaptive resistance',
      intensity: 'Strong (Standard)',
      vibration: 'Medium',
      vibrationMode: 'Balanced'
    },
    lightbar: {
      enabled: true,
      color: '#4cc9f0',
      rpmColor: '#ff3a2e',
      brightness: 72
    },
    forza: defaultForzaTelemetryConfig(forzaDefaults),
    sticks: {
      leftCurve: 'Default',
      leftCurveAmount: 50,
      leftDeadzone: 0,
      rightCurve: 'Default',
      rightCurveAmount: 50,
      rightDeadzone: 0
    },
    buttons: defaultButtonAssignments(options.isEdge),
    inputBridge: defaultInputBridgeConfig(),
    profileAssignments: options.profileAssignments ?? []
  };
}

export function editableConfigFromController(
  config: ControllerConfiguration,
  edge = false
): EditableControllerConfig {
  return {
    inputMode: config.inputMode,
    trigger: config.trigger,
    lightbar: config.lightbar,
    forza: config.forza,
    sticks: config.sticks,
    buttons: normalizeButtonAssignments(config.buttons, config.model === 'DualSense Edge' || edge),
    inputBridge: normalizeInputBridgeConfig(config.inputBridge),
    profileAssignments: config.profileAssignments
  };
}

export function buildBuiltInProfileConfig(options: DraftConfigOptions & {
  profileId: string;
  builtInForzaEffects: ForzaEffectConfiguration[];
}): EditableControllerConfig {
  const base = buildDefaultControllerConfig(options);
  const forzaDefaults = forzaTuningDefaultsFromOptions(options);
  if (options.profileId === 'global') {
    return {
      ...base,
      profileAssignments: options.profileAssignments ?? []
    };
  }

  return {
    ...base,
    trigger: baseForzaTriggerDefaults(),
    forza: defaultForzaTelemetryConfig(forzaDefaults, options.builtInForzaEffects),
    profileAssignments: options.profileAssignments ?? []
  };
}

export function editableConfigFromProfileExport(
  config: NonNullable<ExportedProfile['config']>,
  options: DraftConfigOptions
): EditableControllerConfig {
  return {
    ...buildDefaultControllerConfig(options),
    inputMode: config.inputMode,
    trigger: config.trigger,
    lightbar: config.lightbar,
    forza: config.forza,
    sticks: config.sticks,
    buttons: normalizeButtonAssignments(config.buttons, options.isEdge),
    inputBridge: normalizeInputBridgeConfig(config.inputBridge),
    profileAssignments: options.profileAssignments ?? []
  };
}

export function buildControllerConfigDraft(
  base: EditableControllerConfig,
  draft: ProfileDraftValues,
  options: DraftBuildOptions
): EditableControllerConfig {
  const forzaNormalizers = forzaTuningNormalizersFromOptions(options);

  return {
    ...base,
    trigger: {
      sameRange: false,
      l2From: normalizeTriggerPercent(draft.l2From),
      l2To: Math.max(normalizeTriggerPercent(draft.l2From), normalizeTriggerPercent(draft.l2To)),
      r2From: normalizeTriggerPercent(draft.r2From),
      r2To: Math.max(normalizeTriggerPercent(draft.r2From), normalizeTriggerPercent(draft.r2To)),
      l2Curve: normalizeTriggerCurve(draft.l2Curve, defaultTriggerCurve('l2')),
      r2Curve: normalizeTriggerCurve(draft.r2Curve, defaultTriggerCurve('r2')),
      l2CurvePoints: normalizeTriggerCurvePoints(draft.l2CurvePoints, draft.l2Curve),
      r2CurvePoints: normalizeTriggerCurvePoints(draft.r2CurvePoints, draft.r2Curve),
      effect: draft.triggerEffect,
      intensity: draft.triggerIntensity,
      vibration: draft.vibrationIntensity,
      vibrationMode: draft.vibrationMode
    },
    lightbar: {
      enabled: draft.lightbarEnabled,
      color: draft.lightbarColor,
      rpmColor: draft.rpmColor,
      brightness: draft.lightbarBrightness
    },
    forza: forzaTelemetryFromDraft(draft, forzaNormalizers),
    sticks: {
      ...base.sticks,
      leftDeadzone: normalizeStickDeadzone(draft.leftStickDeadzone),
      rightDeadzone: normalizeStickDeadzone(draft.rightStickDeadzone)
    },
    buttons: normalizeButtonAssignments(base.buttons, options.isEdge)
  };
}

export function profileConfigSignature(
  config: EditableControllerConfig | ControllerConfiguration,
  options: ProfileConfigSignatureOptions
): string {
  const forzaNormalizers = forzaTuningNormalizersFromOptions(options);
  const forza = normalizeForzaTelemetryConfig(config.forza, forzaNormalizers);

  return JSON.stringify({
    inputMode: config.inputMode,
    trigger: {
      sameRange: false,
      l2From: normalizeTriggerPercent(config.trigger.l2From),
      l2To: normalizeTriggerPercent(config.trigger.l2To),
      r2From: normalizeTriggerPercent(config.trigger.r2From),
      r2To: normalizeTriggerPercent(config.trigger.r2To),
      l2Curve: normalizeTriggerCurve(config.trigger.l2Curve, defaultTriggerCurve('l2')),
      r2Curve: normalizeTriggerCurve(config.trigger.r2Curve, defaultTriggerCurve('r2')),
      l2CurvePoints: normalizeTriggerCurvePoints(
        config.trigger.l2CurvePoints,
        normalizeTriggerCurve(config.trigger.l2Curve, defaultTriggerCurve('l2'))
      ),
      r2CurvePoints: normalizeTriggerCurvePoints(
        config.trigger.r2CurvePoints,
        normalizeTriggerCurve(config.trigger.r2Curve, defaultTriggerCurve('r2'))
      ),
      effect: config.trigger.effect,
      intensity: config.trigger.intensity,
      vibration: config.trigger.vibration,
      vibrationMode: config.trigger.vibrationMode ?? 'Balanced'
    },
    lightbar: {
      enabled: config.lightbar?.enabled ?? true,
      color: config.lightbar?.color ?? '#4cc9f0',
      rpmColor: config.lightbar?.rpmColor ?? '#ff3a2e',
      brightness: normalizeTriggerPercent(config.lightbar?.brightness ?? 72)
    },
    forza: {
      bodyRumbleMode: forza.bodyRumbleMode,
      effects: forza.effects.map((effect) => ({
        id: effect.id,
        enabled: effect.enabled,
        intensity: options.forzaIntensityPercent(effect.intensity),
        route: effect.route
      })),
      brake: forza.brake,
      abs: forza.abs,
      throttle: forza.throttle,
      shift: forza.shift,
      revLimiter: forza.revLimiter
    },
    sticks: config.sticks,
    buttons: normalizeButtonAssignments(config.buttons, options.isEdge),
    inputBridge: normalizeInputBridgeConfig(config.inputBridge),
    profileAssignments: config.profileAssignments
  });
}
