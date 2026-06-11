// Saved-vs-draft diff for the tuning saved rail (Task 7).
//
// Pure and total: given the SAVED profile config (the baseline captured when a
// profile is loaded or saved) and the current working draft values, derive
// human-labeled rows covering the tunable surface. Dirty rows carry both the
// saved and current value formatted for display; the rail renders
// `saved (strikethrough) → current`, clean rows show the saved value muted.
//
// Comparison normalizes through the same helpers the draft/save path uses
// (hapticsModel normalizers) so the diff agrees with the signature-based
// `profileConfigDirty` flag in App.svelte.

import {
  DEFAULT_BODY_FEEL,
  DEFAULT_BODY_RUMBLE_MODE,
  DEFAULT_LIGHTBAR_BRIGHTNESS,
  DEFAULT_LIGHTBAR_COLOR,
  DEFAULT_REDLINE_COLOR,
  defaultTriggerCurve,
  normalizeStickDeadzone,
  normalizeTriggerCurve,
  normalizeTriggerCurvePoints,
  normalizeTriggerPercent
} from '../haptics/hapticsModel';
import { forzaEffectMetas, forzaRoutes } from '../haptics/hapticsOptions';
import type {
  ControllerConfiguration,
  ForzaAbsTuningConfiguration,
  ForzaBodyRumbleMode,
  ForzaBrakeTuningConfiguration,
  ForzaEffectConfiguration,
  ForzaRevLimiterTuningConfiguration,
  ForzaShiftTuningConfiguration,
  ForzaThrottleTuningConfiguration,
  TriggerCurvePoint
} from '../../types';

export type SavedDiffRow = {
  id: string;
  label: string;
  savedValue: string;
  currentValue: string;
  dirty: boolean;
};

/** The saved baseline: the editable slice of a controller configuration. */
export type SavedProfileConfig = Pick<
  ControllerConfiguration,
  'trigger' | 'lightbar' | 'forza' | 'sticks'
>;

/** Structural mirror of App's working-draft values (app/profileDraft.ts). */
export type SavedDiffDraft = {
  l2From: number;
  l2To: number;
  r2From: number;
  r2To: number;
  l2Curve: number;
  r2Curve: number;
  l2CurvePoints: TriggerCurvePoint[];
  r2CurvePoints: TriggerCurvePoint[];
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

export type SavedDiffOptions = {
  /** Include the telemetry effect/tuning rows (game scope only). */
  includeForza: boolean;
  /** Maps a raw 0-255 effect intensity to a display percent (hapticsState). */
  intensityPercent: (intensity: number) => number;
};

const routeLabelByValue = new Map(forzaRoutes.map((route) => [route.value as string, route.label]));

const routeLabel = (route: string) => routeLabelByValue.get(route) ?? route;

const roundish = (value: number) => Math.round(value * 1000) / 1000;

const numbersEqual = (a: number, b: number) => roundish(a) === roundish(b);

const pointsEqual = (a: TriggerCurvePoint[], b: TriggerCurvePoint[]) =>
  a.length === b.length &&
  a.every(
    (point, index) =>
      numbersEqual(point.input, b[index].input) && numbersEqual(point.output, b[index].output)
  );

type NormalizedCurve = {
  from: number;
  to: number;
  curve: number;
  points: TriggerCurvePoint[];
};

const normalizedCurve = (
  side: 'l2' | 'r2',
  from: number,
  to: number,
  curve: number,
  points: TriggerCurvePoint[] | undefined
): NormalizedCurve => {
  const safeFrom = normalizeTriggerPercent(from);
  const safeTo = Math.max(safeFrom, normalizeTriggerPercent(to));
  const safeCurve = normalizeTriggerCurve(curve, defaultTriggerCurve(side));
  return {
    from: safeFrom,
    to: safeTo,
    curve: safeCurve,
    points: normalizeTriggerCurvePoints(points, safeCurve)
  };
};

const formatRange = (value: NormalizedCurve) => `${value.from}–${value.to}%`;

const formatCurve = (value: NormalizedCurve) =>
  `x${value.curve.toFixed(2)} · ${value.points.length} pts`;

const row = (
  id: string,
  label: string,
  savedValue: string,
  currentValue: string,
  dirty = savedValue !== currentValue
): SavedDiffRow => ({ id, label, savedValue, currentValue, dirty });

/** A generic per-field comparison for the deep telemetry tuning groups. */
const tuningGroupRow = <T extends object>(
  id: string,
  label: string,
  saved: T,
  current: T
): SavedDiffRow => {
  const keys = Object.keys(saved) as Array<keyof T>;
  let changed = 0;
  for (const key of keys) {
    const savedValue = saved[key];
    const currentValue = current[key];
    if (typeof savedValue === 'number' && typeof currentValue === 'number') {
      if (!numbersEqual(savedValue, currentValue)) changed += 1;
    } else if (savedValue !== currentValue) {
      changed += 1;
    }
  }
  if (changed > 0) {
    // Group rows summarize many fields; there is no single saved value to
    // strike through, so both sides carry the same "N of M edited" text and
    // the rail renders it once, without a strikethrough.
    const edited = `${changed} of ${keys.length} edited`;
    return row(id, label, edited, edited, true);
  }
  const clean = `${keys.length} settings`;
  return row(id, label, clean, clean, false);
};

const effectValue = (
  effect: ForzaEffectConfiguration,
  intensityPercent: SavedDiffOptions['intensityPercent']
) => (effect.enabled ? `${intensityPercent(effect.intensity)}% · ${routeLabel(effect.route)}` : 'Off');

const effectById = (
  effects: ForzaEffectConfiguration[],
  id: string,
  fallback: ForzaEffectConfiguration
) => effects.find((effect) => effect.id === id) ?? fallback;

const curveRows = (
  side: 'l2' | 'r2',
  label: string,
  saved: SavedProfileConfig,
  draft: SavedDiffDraft
): SavedDiffRow[] => {
  const savedCurve = normalizedCurve(
    side,
    saved.trigger[`${side}From`],
    saved.trigger[`${side}To`],
    saved.trigger[`${side}Curve`],
    saved.trigger[`${side}CurvePoints`]
  );
  const draftCurve = normalizedCurve(
    side,
    draft[`${side}From`],
    draft[`${side}To`],
    draft[`${side}Curve`],
    draft[`${side}CurvePoints`]
  );

  const curveDirty =
    !numbersEqual(savedCurve.curve, draftCurve.curve) ||
    !pointsEqual(savedCurve.points, draftCurve.points);
  const savedLabel = formatCurve(savedCurve);
  let currentLabel = formatCurve(draftCurve);
  // A moved point can leave the summary text identical; say so in plain words.
  if (curveDirty && currentLabel === savedLabel) currentLabel = 'reshaped';

  return [
    row(`${side}-range`, `${label} range`, formatRange(savedCurve), formatRange(draftCurve)),
    row(`${side}-curve`, `${label} curve`, savedLabel, currentLabel, curveDirty)
  ];
};

/**
 * Derive the saved-vs-current rows for the saved rail.
 *
 * Returns [] when there is no saved baseline yet (config still loading).
 */
export const savedDiffRows = (
  saved: SavedProfileConfig | null | undefined,
  draft: SavedDiffDraft,
  options: SavedDiffOptions
): SavedDiffRow[] => {
  if (!saved) return [];

  const rows: SavedDiffRow[] = [
    ...curveRows('l2', 'Brake', saved, draft),
    ...curveRows('r2', 'Throttle', saved, draft),
    row('trigger-mode', 'Trigger mode', saved.trigger.effect, draft.triggerEffect),
    row('trigger-force', 'Trigger force', saved.trigger.intensity, draft.triggerIntensity),
    row('body-rumble', 'Body rumble', saved.trigger.vibration, draft.vibrationIntensity),
    row(
      'body-feel',
      'Body feel',
      saved.trigger.vibrationMode ?? DEFAULT_BODY_FEEL,
      draft.vibrationMode
    ),
    row(
      'lightbar',
      'Lightbar',
      saved.lightbar?.enabled ?? true
        ? `On · ${normalizeTriggerPercent(saved.lightbar?.brightness ?? DEFAULT_LIGHTBAR_BRIGHTNESS)}%`
        : 'Off',
      draft.lightbarEnabled ? `On · ${normalizeTriggerPercent(draft.lightbarBrightness)}%` : 'Off'
    ),
    row(
      'lightbar-color',
      'Lightbar color',
      (saved.lightbar?.color ?? DEFAULT_LIGHTBAR_COLOR).toLowerCase(),
      draft.lightbarColor.toLowerCase()
    ),
    row(
      'redline-color',
      'Redline color',
      (saved.lightbar?.rpmColor ?? DEFAULT_REDLINE_COLOR).toLowerCase(),
      draft.rpmColor.toLowerCase()
    ),
    row(
      'left-deadzone',
      'Left stick deadzone',
      `${normalizeStickDeadzone(saved.sticks?.leftDeadzone ?? 0)}%`,
      `${normalizeStickDeadzone(draft.leftStickDeadzone)}%`
    ),
    row(
      'right-deadzone',
      'Right stick deadzone',
      `${normalizeStickDeadzone(saved.sticks?.rightDeadzone ?? 0)}%`,
      `${normalizeStickDeadzone(draft.rightStickDeadzone)}%`
    )
  ];

  if (options.includeForza) {
    const savedMode = saved.forza?.bodyRumbleMode ?? DEFAULT_BODY_RUMBLE_MODE;
    const modeLabel = (mode: ForzaBodyRumbleMode) =>
      mode === 'dscc_full_control' ? 'DSCC full control' : 'Game native';
    rows.push(row('rumble-source', 'Rumble source', modeLabel(savedMode), modeLabel(draft.forzaBodyRumbleMode)));

    const savedEffects = saved.forza?.effects ?? [];
    for (const meta of forzaEffectMetas) {
      const fallback: ForzaEffectConfiguration = {
        id: meta.id,
        enabled: true,
        intensity: meta.defaultIntensity,
        route: meta.defaultRoute
      };
      const savedEffect = effectById(savedEffects, meta.id, fallback);
      const draftEffect = effectById(draft.forzaEffects, meta.id, fallback);
      rows.push(
        row(
          `effect-${meta.id}`,
          meta.label,
          effectValue(savedEffect, options.intensityPercent),
          effectValue(draftEffect, options.intensityPercent)
        )
      );
    }

    if (saved.forza?.brake) rows.push(tuningGroupRow('brake-detail', 'Brake feel detail', saved.forza.brake, draft.forzaBrakeTuning));
    if (saved.forza?.abs) rows.push(tuningGroupRow('abs-detail', 'ABS detail', saved.forza.abs, draft.forzaAbsTuning));
    if (saved.forza?.throttle) rows.push(tuningGroupRow('throttle-detail', 'Throttle feel detail', saved.forza.throttle, draft.forzaThrottleTuning));
    if (saved.forza?.shift) rows.push(tuningGroupRow('shift-detail', 'Gear-shift detail', saved.forza.shift, draft.forzaShiftTuning));
    if (saved.forza?.revLimiter) rows.push(tuningGroupRow('rev-detail', 'Rev limiter detail', saved.forza.revLimiter, draft.forzaRevLimiterTuning));
  }

  return rows;
};

/** Count of dirty rows — drives the header "N unsaved changes" note. */
export const unsavedChangeCount = (rows: SavedDiffRow[]) =>
  rows.reduce((count, item) => count + (item.dirty ? 1 : 0), 0);
