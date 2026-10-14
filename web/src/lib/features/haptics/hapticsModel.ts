import type { ForzaBodyRumbleMode, ForzaEffectRoute, TriggerCurvePoint } from '../../types';

// Semantic tuning columns (Task 6): effects group by what is being tuned —
// brake, throttle, road feel, or lights — never by control type.
export type TuningColumnId = 'brake' | 'throttle' | 'road' | 'lights';

export type ForzaEffectMeta = {
  id: string;
  label: string;
  signal: string;
  group: 'Trigger' | 'Body' | 'Cue' | 'Light';
  defaultIntensity: number;
  defaultRoute: ForzaEffectRoute;
  help: string;
  /** Tuning canvas column. Omitted: Light-group effects land in Lights, everything else in Road feel. */
  column?: TuningColumnId;
};

export type LightbarColorTarget = 'lightbar' | 'rpm';

export const LIGHTBAR_COLOR_PRESETS = [
  '#3BAEFF',
  '#003791',
  '#4cc9f0',
  '#ffffff',
  '#ec4899',
  '#a855f7',
  '#fb923c',
  '#ef4444',
  '#4ade80',
  '#facc15'
];

// Shared profile defaults: the fallback values every config reader/writer
// (profileDraft signature, saved rail diff, App's applyEditableConfig) must
// agree on when a saved profile omits a field.
export const DEFAULT_LIGHTBAR_COLOR = '#4cc9f0';
export const DEFAULT_REDLINE_COLOR = '#ff3a2e';
export const DEFAULT_LIGHTBAR_BRIGHTNESS = 72;
export const DEFAULT_BODY_FEEL = 'Balanced';
export const DEFAULT_BODY_RUMBLE_MODE: ForzaBodyRumbleMode = 'native_passthrough';

export const TRIGGER_CURVE_POINT_MIN = 4;
export const TRIGGER_CURVE_POINT_MAX = 8;
export const TRIGGER_CURVE_SAMPLE_POSITIONS = Array.from({ length: 101 }, (_, index) => index / 100);

export function clampUnit(value: number) {
  return Math.max(0, Math.min(1, value));
}

export function normalizeTriggerPercent(value: number | string) {
  const numeric = typeof value === 'number' ? value : Number.parseFloat(value);
  if (!Number.isFinite(numeric)) return 0;
  return Math.max(0, Math.min(100, Math.round(numeric)));
}

export function normalizeTriggerCurve(value: number | string | undefined, fallback = 1.45) {
  const numeric = typeof value === 'number' ? value : Number.parseFloat(String(value ?? ''));
  if (!Number.isFinite(numeric)) return fallback;
  return Math.round(Math.max(0.5, Math.min(3.5, numeric)) * 100) / 100;
}

export function defaultTriggerCurve(side: 'l2' | 'r2') {
  return side === 'l2' ? 1 : 2.25;
}

export function normalizeStickDeadzone(value: number | string | undefined | null) {
  const numeric = typeof value === 'number' ? value : Number.parseFloat(String(value ?? ''));
  if (!Number.isFinite(numeric)) return 0;
  return Math.max(0, Math.min(40, Math.round(numeric)));
}

export function triggerCurvePointsFromCurve(curve: number): TriggerCurvePoint[] {
  const normalized = normalizeTriggerCurve(curve);
  return [0, 25, 50, 75, 100].map((input) => ({
    input,
    output: Math.round(Math.pow(input / 100, normalized) * 100)
  }));
}

export function defaultTriggerCurvePoints(side: 'l2' | 'r2'): TriggerCurvePoint[] {
  return triggerCurvePointsFromCurve(defaultTriggerCurve(side));
}

export function normalizeTriggerCurvePoints(
  points: TriggerCurvePoint[] | undefined,
  fallbackCurve: number
): TriggerCurvePoint[] {
  const fallback = triggerCurvePointsFromCurve(fallbackCurve);
  const normalized = (points ?? [])
    .map((point) => ({
      input: normalizeTriggerPercent(point.input),
      output: normalizeTriggerPercent(point.output)
    }))
    .filter((point) => point.input >= 0 && point.input <= 100)
    .sort((a, b) => a.input - b.input);

  const byInput = new Map(normalized.map((point) => [point.input, point]));
  byInput.set(0, { input: 0, output: 0 });
  byInput.set(100, { input: 100, output: 100 });

  const deduped = [...byInput.values()].sort((a, b) => a.input - b.input);
  if (deduped.length < TRIGGER_CURVE_POINT_MIN) return fallback;
  if (deduped.length <= TRIGGER_CURVE_POINT_MAX) return deduped;
  return [deduped[0], ...deduped.slice(1, TRIGGER_CURVE_POINT_MAX - 1), deduped[deduped.length - 1]];
}

export function triggerCurvePointOutput(points: TriggerCurvePoint[], active: number) {
  const normalized = normalizeTriggerCurvePoints(points, 1);
  const x = clampUnit(active);
  for (let index = 0; index < normalized.length - 1; index += 1) {
    const left = normalized[index];
    const right = normalized[index + 1];
    const leftInput = left.input / 100;
    const rightInput = right.input / 100;
    if (x >= leftInput && x <= rightInput) {
      if (rightInput <= leftInput) return right.output / 100;
      const ratio = (x - leftInput) / (rightInput - leftInput);
      return (left.output + (right.output - left.output) * ratio) / 100;
    }
  }
  return (normalized[normalized.length - 1]?.output ?? 0) / 100;
}
