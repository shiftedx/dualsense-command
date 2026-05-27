import type { ForzaEffectConfiguration, ForzaEffectRoute } from '../lib/types';
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
