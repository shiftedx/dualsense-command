import type {
  ForzaAbsMode,
  ForzaAbsSlipSource,
  ForzaAbsTuningConfiguration,
  ForzaEffectConfiguration,
  ForzaEffectRoute,
  ForzaRevLimiterTuningConfiguration,
  ForzaShiftTuningConfiguration,
  ForzaThrottleTuningConfiguration
} from '../lib/types';
import {
  FORZA_SHIFT_THUMP_DEFAULT_INTENSITY,
  forzaEffectMetas,
  forzaRoutes
} from '../lib/features/haptics/hapticsOptions';

export const clamp = (value: number, min = 0, max = 100) => Math.max(min, Math.min(max, value));

export const clampForzaIntensity = (value: number) => Math.round(clamp(Number(value) || 0, 0, 255));

export const clampForzaPercent = (value: number | string) => {
  const numeric = typeof value === 'number' ? value : Number(value);
  return Math.round(clamp(Number.isFinite(numeric) ? numeric : 0, 0, 100));
};

export const forzaIntensityPercent = (intensity: number) =>
  Math.round((clampForzaIntensity(intensity) / 255) * 100);

export const forzaIntensityFromPercent = (percent: number | string) =>
  Math.round(clampForzaPercent(percent) * 2.55);

export const normalizeEffectId = (id: string) => id.replaceAll('-', '_');

const finiteClamp = (value: number | undefined, min: number, max: number, fallback: number) => {
  const numeric = Number(value);
  return clamp(Number.isFinite(numeric) ? numeric : fallback, min, max);
};

export function defaultForzaAbsTuning(): ForzaAbsTuningConfiguration {
  return {
    mode: 'strong_pulse',
    slipSource: 'auto_front_first',
    slipThreshold: 0.68,
    brakeThresholdRatio: 0.38,
    minSpeedKmh: 12,
    minStrength: 48 / 63,
    maxStrength: 1,
    frequencyHz: 34,
    curve: 1
  };
}

export const normalizeForzaAbsTuning = (
  tuning: Partial<ForzaAbsTuningConfiguration> | undefined | null
): ForzaAbsTuningConfiguration => {
  const defaults = defaultForzaAbsTuning();
  const mode: ForzaAbsMode = tuning?.mode === 'fine_flutter' ? 'fine_flutter' : defaults.mode;
  const slipSource: ForzaAbsSlipSource =
    tuning?.slipSource === 'front' ||
    tuning?.slipSource === 'tire' ||
    tuning?.slipSource === 'wheel' ||
    tuning?.slipSource === 'auto_front_first'
      ? tuning.slipSource
      : defaults.slipSource;
  const minStrength = finiteClamp(tuning?.minStrength, 0, 1, defaults.minStrength);
  return {
    mode,
    slipSource,
    slipThreshold: finiteClamp(tuning?.slipThreshold, 0.05, 2, defaults.slipThreshold),
    brakeThresholdRatio: finiteClamp(tuning?.brakeThresholdRatio, 0, 1, defaults.brakeThresholdRatio),
    minSpeedKmh: finiteClamp(tuning?.minSpeedKmh, 0, 250, defaults.minSpeedKmh),
    minStrength,
    maxStrength: finiteClamp(tuning?.maxStrength, minStrength, 1, defaults.maxStrength),
    frequencyHz: finiteClamp(tuning?.frequencyHz, 1, 80, defaults.frequencyHz),
    curve: finiteClamp(tuning?.curve, 0.4, 3, defaults.curve)
  };
};

export function defaultForzaThrottleTuning(): ForzaThrottleTuningConfiguration {
  return {
    baselineForce: 3 / 255,
    normalForce: 28 / 255,
    endstopForce: 106 / 255,
    endstopBoost: 3,
    wallPosition: 0.8,
    guardMinEnd: 0.8,
    rampWidth: 0.2,
    rampCurve: 2.4
  };
}

export const normalizeForzaThrottleTuning = (
  tuning: Partial<ForzaThrottleTuningConfiguration> | undefined | null
): ForzaThrottleTuningConfiguration => {
  const defaults = defaultForzaThrottleTuning();
  const baselineForce = finiteClamp(tuning?.baselineForce, 0, 1, defaults.baselineForce);
  return {
    baselineForce,
    normalForce: finiteClamp(tuning?.normalForce, baselineForce, 1, defaults.normalForce),
    endstopForce: finiteClamp(tuning?.endstopForce, 0, 1, defaults.endstopForce),
    endstopBoost: finiteClamp(tuning?.endstopBoost, 0, 5, defaults.endstopBoost),
    wallPosition: finiteClamp(tuning?.wallPosition, 0, 1, defaults.wallPosition),
    guardMinEnd: finiteClamp(tuning?.guardMinEnd, 0, 1, defaults.guardMinEnd),
    rampWidth: finiteClamp(tuning?.rampWidth, 0.01, 0.8, defaults.rampWidth),
    rampCurve: finiteClamp(tuning?.rampCurve, 0.4, 4, defaults.rampCurve)
  };
};

export function defaultForzaShiftTuning(): ForzaShiftTuningConfiguration {
  return {
    wallFormAt: 0.15,
    frequencyHz: 34,
    wallZones: 4,
    bodyLowWeight: 0.92,
    bodyHighWeight: 0.84
  };
}

export const normalizeForzaShiftTuning = (
  tuning: Partial<ForzaShiftTuningConfiguration> | undefined | null
): ForzaShiftTuningConfiguration => {
  const defaults = defaultForzaShiftTuning();
  return {
    wallFormAt: finiteClamp(tuning?.wallFormAt, 0, 1, defaults.wallFormAt),
    frequencyHz: finiteClamp(tuning?.frequencyHz, 1, 80, defaults.frequencyHz),
    wallZones: finiteClamp(tuning?.wallZones, 1, 8, defaults.wallZones),
    bodyLowWeight: finiteClamp(tuning?.bodyLowWeight, 0, 1.5, defaults.bodyLowWeight),
    bodyHighWeight: finiteClamp(tuning?.bodyHighWeight, 0, 1.5, defaults.bodyHighWeight)
  };
};

export function defaultForzaRevLimiterTuning(): ForzaRevLimiterTuningConfiguration {
  return {
    thresholdRatio: 0.93,
    minStrength: 18 / 63,
    maxStrength: 18 / 63,
    frequencyHz: 42,
    wallFormThrottleAt: 0.6,
    wallZones: 4,
    curve: 1,
    bodyLowWeight: 0.2,
    bodyHighWeight: 0.8
  };
}

export const normalizeForzaRevLimiterTuning = (
  tuning: Partial<ForzaRevLimiterTuningConfiguration> | undefined | null
): ForzaRevLimiterTuningConfiguration => {
  const defaults = defaultForzaRevLimiterTuning();
  const minStrength = finiteClamp(tuning?.minStrength, 0, 1, defaults.minStrength);
  return {
    thresholdRatio: finiteClamp(tuning?.thresholdRatio, 0.5, 1, defaults.thresholdRatio),
    minStrength,
    maxStrength: finiteClamp(tuning?.maxStrength, minStrength, 1, defaults.maxStrength),
    frequencyHz: finiteClamp(tuning?.frequencyHz, 1, 80, defaults.frequencyHz),
    wallFormThrottleAt: finiteClamp(tuning?.wallFormThrottleAt, 0, 1, defaults.wallFormThrottleAt),
    wallZones: finiteClamp(tuning?.wallZones, 1, 8, defaults.wallZones),
    curve: finiteClamp(tuning?.curve, 0.4, 4, defaults.curve),
    bodyLowWeight: finiteClamp(tuning?.bodyLowWeight, 0, 1.5, defaults.bodyLowWeight),
    bodyHighWeight: finiteClamp(tuning?.bodyHighWeight, 0, 1.5, defaults.bodyHighWeight)
  };
};

export function defaultForzaEffects(): ForzaEffectConfiguration[] {
  return forzaEffectMetas.map((effect) => ({
    id: effect.id,
    enabled: true,
    intensity: effect.defaultIntensity,
    route: effect.defaultRoute
  }));
}

export const normalizeForzaEffects = (
  effects: ForzaEffectConfiguration[] | undefined
): ForzaEffectConfiguration[] => {
  const source = new Map((effects ?? []).map((effect) => [effect.id, effect]));
  return forzaEffectMetas.map((meta) => {
    const effect = source.get(meta.id);
    const route =
      effect?.route && forzaRoutes.some((item) => item.value === effect.route)
        ? effect.route
        : meta.defaultRoute;
    return {
      id: meta.id,
      enabled: effect?.enabled ?? true,
      intensity: clampForzaIntensity(effect?.intensity ?? meta.defaultIntensity),
      route
    };
  });
};

export const forzaPresetEffects = (preset: 'base' | 'immersive'): ForzaEffectConfiguration[] => {
  const entries: Array<[string, boolean, number, ForzaEffectRoute]> =
    preset === 'immersive'
      ? [
          ['brake_resistance', true, 100, 'l2'],
          ['throttle_resistance', true, 100, 'r2'],
          ['abs_slip_pulse', true, 100, 'l2'],
          ['handbrake_wall', true, 100, 'l2'],
          ['rev_limiter_buzz', true, 62, 'r2'],
          ['gear_shift_thump', true, FORZA_SHIFT_THUMP_DEFAULT_INTENSITY, 'r2_and_body'],
          ['road_texture', true, 35, 'body_both'],
          ['rumble_strip', true, 38, 'body_both'],
          ['tire_slip', true, 30, 'body_right'],
          ['puddle_drag', true, 32, 'body_left'],
          ['suspension_impact', true, 82, 'body_both'],
          ['rpm_leds', false, 100, 'light_led']
        ]
      : [
          ['brake_resistance', true, 100, 'l2'],
          ['throttle_resistance', true, 100, 'r2'],
          ['abs_slip_pulse', true, 100, 'l2'],
          ['handbrake_wall', true, 100, 'l2'],
          ['rev_limiter_buzz', true, 55, 'r2'],
          ['gear_shift_thump', true, FORZA_SHIFT_THUMP_DEFAULT_INTENSITY, 'r2_and_body'],
          ['road_texture', true, 40, 'body_both'],
          ['rumble_strip', false, 55, 'body_both'],
          ['tire_slip', false, 65, 'body_right'],
          ['puddle_drag', false, 50, 'body_left'],
          ['suspension_impact', false, 70, 'body_both'],
          ['rpm_leds', false, 100, 'light_led']
        ];
  return normalizeForzaEffects(entries.map(([id, enabled, intensity, route]) => ({ id, enabled, intensity, route })));
};
