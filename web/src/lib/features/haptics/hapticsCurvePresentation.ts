import type {
  ForzaEffectConfiguration,
  ForzaEffectRoute,
  ForzaBrakeTuningConfiguration,
  ForzaThrottleTuningConfiguration,
  TriggerCurvePoint
} from '../../types';
import {
  TRIGGER_CURVE_SAMPLE_POSITIONS,
  clampUnit,
  normalizeTriggerCurve,
  normalizeTriggerCurvePoints,
  normalizeTriggerPercent,
  triggerCurvePointOutput
} from './hapticsModel';
import {
  FORZA_BRAKE_BASELINE_FORCE,
  FORZA_BRAKE_ENDSTOP_FORCE,
  FORZA_BRAKE_ENDSTOP_FORCE_BOOST,
  FORZA_BRAKE_FULL_FORCE_INPUT,
  FORZA_BRAKE_NORMAL_FORCE,
  FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE,
  FORZA_BRAKE_OVERTRAVEL_MIN_POSITION,
  FORZA_BRAKE_OVERTRAVEL_WALL_POSITION,
  FORZA_ENDSTOP_WALL_OFFSET,
  FORZA_THROTTLE_BASELINE_FORCE,
  FORZA_THROTTLE_ENDSTOP_FORCE,
  FORZA_THROTTLE_ENDSTOP_FORCE_BOOST,
  FORZA_THROTTLE_NORMAL_FORCE,
  FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION,
  FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE,
  FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH,
  FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION,
  forzaEffectMetas,
  routeTooltips,
  vibrationModeOptions
} from './hapticsOptions';
import type { ForzaEffectMeta } from './hapticsModel';

export type TriggerSide = 'l2' | 'r2';
export type TriggerCurveDisplayMode = 'base' | 'forza';
export type ForzaTriggerForceModel = {
  start: number;
  end: number;
  wall: number;
  rampStart?: number;
  finalStopInput?: number;
  finalStopPosition?: number;
  curve: number;
  rampCurve?: number;
  baselineForce: number;
  normalForce: number;
  endstopForce: number;
  points: TriggerCurvePoint[];
};

const clamp = (value: number, min = 0, max = 100) => Math.max(min, Math.min(max, value));
const DEFAULT_FORZA_EFFECT_BY_ID = new Map(
  forzaEffectMetas.map((meta) => [
    meta.id,
    { id: meta.id, enabled: true, intensity: meta.defaultIntensity, route: meta.defaultRoute }
  ])
);

export const clampForzaIntensity = (value: number) => Math.round(clamp(Number(value) || 0, 0, 255));
export const forzaIntensityPercent = (intensity: number) => Math.round((clampForzaIntensity(intensity) / 255) * 100);
export const triggerStrengthScalarFor = (effect: string, intensity: string) => {
    if (effect === 'Off' || intensity === 'Off') return 0;
    if (intensity === 'Weak') return 0.36;
    if (intensity === 'Medium') return 0.68;
    return 1;
  };

export const vibrationIntensityPercent = (value: string) => {
    if (value === 'Off') return 0;
    if (value === 'Low') return 48;
    if (value === 'High') return 100;
    return 82;
  };

export const vibrationModeRequest = (value: string) =>
    vibrationModeOptions.find((option) => option.label === value)?.mode ?? 'balanced';

export const triggerRangeValuesFor = (fromRaw: number | string, toRaw: number | string) => {
    const from = normalizeTriggerPercent(fromRaw);
    const to = Math.max(from, normalizeTriggerPercent(toRaw));
    return { from, to, width: Math.max(0, to - from) };
  };

export const triggerRangeUnitValuesFor = (fromRaw: number | string, toRaw: number | string) => {
    const range = triggerRangeValuesFor(fromRaw, toRaw);
    const start = range.from / 100;
    const end = Math.max(start + 0.01, range.to / 100);
    return { start, end };
  };

export const scaledUnitForGraph = (value: number, scalar: number) => clampUnit(value * scalar);
export const signalCurveForGraph = (input: number, inputMin: number, inputMax: number, outputMin: number, outputMax: number, exponent: number) => {
    if (inputMin === inputMax || exponent <= 0) return outputMin;
    const ratio = clampUnit((input - inputMin) / (inputMax - inputMin));
    return outputMin + (outputMax - outputMin) * Math.pow(ratio, exponent);
  };

export const defaultForzaBrakeTuningForGraph = (): ForzaBrakeTuningConfiguration => ({
    baselineForce: FORZA_BRAKE_BASELINE_FORCE,
    normalForce: FORZA_BRAKE_NORMAL_FORCE,
    endstopForce: FORZA_BRAKE_ENDSTOP_FORCE,
    endstopBoost: FORZA_BRAKE_ENDSTOP_FORCE_BOOST,
    wallPosition: FORZA_BRAKE_OVERTRAVEL_WALL_POSITION,
    guardMinEnd: FORZA_BRAKE_OVERTRAVEL_MIN_POSITION,
    fullForceAt: FORZA_BRAKE_FULL_FORCE_INPUT,
    rampCurve: FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE
  });

export const defaultForzaThrottleTuningForGraph = (): ForzaThrottleTuningConfiguration => ({
    baselineForce: FORZA_THROTTLE_BASELINE_FORCE,
    normalForce: FORZA_THROTTLE_NORMAL_FORCE,
    endstopForce: FORZA_THROTTLE_ENDSTOP_FORCE,
    endstopBoost: FORZA_THROTTLE_ENDSTOP_FORCE_BOOST,
    wallPosition: FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION,
    guardMinEnd: FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION,
    rampWidth: FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH,
    rampCurve: FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE
  });

const finiteClamp = (value: number | undefined, min: number, max: number, fallback: number) => {
    const numeric = Number(value);
    return clamp(Number.isFinite(numeric) ? numeric : fallback, min, max);
  };

export const normalizeForzaThrottleTuningForGraph = (
    tuning: Partial<ForzaThrottleTuningConfiguration> | undefined | null
  ): ForzaThrottleTuningConfiguration => {
    const defaults = defaultForzaThrottleTuningForGraph();
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

export const normalizeForzaBrakeTuningForGraph = (
    tuning: Partial<ForzaBrakeTuningConfiguration> | undefined | null
  ): ForzaBrakeTuningConfiguration => {
    const defaults = defaultForzaBrakeTuningForGraph();
    const baselineForce = finiteClamp(tuning?.baselineForce, 0, 1, defaults.baselineForce);
    return {
      baselineForce,
      normalForce: finiteClamp(tuning?.normalForce, baselineForce, 1, defaults.normalForce),
      endstopForce: finiteClamp(tuning?.endstopForce, 0, 1, defaults.endstopForce),
      endstopBoost: finiteClamp(tuning?.endstopBoost, 0, 5, defaults.endstopBoost),
      wallPosition: finiteClamp(tuning?.wallPosition, 0, 1, defaults.wallPosition),
      guardMinEnd: finiteClamp(tuning?.guardMinEnd, 0, 1, defaults.guardMinEnd),
      fullForceAt: finiteClamp(tuning?.fullForceAt, 0, 1, defaults.fullForceAt),
      rampCurve: finiteClamp(tuning?.rampCurve, 0.4, 4, defaults.rampCurve)
    };
  };

export const endstopWallPosition = (start: number, end: number) => clamp(end - FORZA_ENDSTOP_WALL_OFFSET, start, end);
export const brakeOvertravelGuardActive = (end: number, guardMinEnd = FORZA_BRAKE_OVERTRAVEL_MIN_POSITION) =>
    end >= clampUnit(guardMinEnd);
export const brakeOvertravelWallPosition = (
    start: number,
    end: number,
    wallPosition = FORZA_BRAKE_OVERTRAVEL_WALL_POSITION,
    guardMinEnd = FORZA_BRAKE_OVERTRAVEL_MIN_POSITION
  ) =>
    brakeOvertravelGuardActive(end, guardMinEnd)
      ? clamp(Math.min(end, clampUnit(wallPosition)), start, end)
      : endstopWallPosition(start, end);
export const brakeFullForcePosition = (wall: number, end: number, fullForceAt = FORZA_BRAKE_FULL_FORCE_INPUT) =>
    clamp(fullForceAt, wall, end);
export const throttleOvertravelGuardActive = (end: number, guardMinEnd = FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION) =>
    end >= clampUnit(guardMinEnd);
export const throttleOvertravelWallPosition = (
    start: number,
    end: number,
    wallPosition = FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION,
    guardMinEnd = FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION
  ) =>
    throttleOvertravelGuardActive(end, guardMinEnd)
      ? clamp(Math.min(end, clampUnit(wallPosition)), start, end)
      : endstopWallPosition(start, end);
export const throttleOvertravelRampStart = (start: number, wall: number, rampWidth = FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH) =>
    clamp(Math.round((wall - clamp(rampWidth, 0.01, 0.8)) * 1000) / 1000, start, wall);
export const routeHasL2 = (route: ForzaEffectRoute) => route === 'l2' || route === 'both_triggers' || route === 'body_and_triggers';
export const routeHasR2 = (route: ForzaEffectRoute) =>
    route === 'r2' || route === 'both_triggers' || route === 'body_and_triggers' || route === 'r2_and_body';
export const forzaEffectScalarForGraph = (effect: ForzaEffectConfiguration | undefined) =>
    effect?.enabled ? clampForzaIntensity(effect.intensity) / 100 : 0;
export const forzaEffectForGraph = (id: string, effects: ForzaEffectConfiguration[]) =>
    effects.find((effect) => effect.id === id) ?? DEFAULT_FORZA_EFFECT_BY_ID.get(id);

export const forzaTriggerForceModelFor = (
    side: TriggerSide,
    fromRaw: number | string,
    toRaw: number | string,
    curveRaw: number | string,
    pointsRaw: TriggerCurvePoint[],
    fallbackCurve: number,
    effect: string,
    intensity: string,
    effects: ForzaEffectConfiguration[],
    brakeTuningRaw?: Partial<ForzaBrakeTuningConfiguration> | null,
    throttleTuningRaw?: Partial<ForzaThrottleTuningConfiguration> | null
  ): ForzaTriggerForceModel | null => {
    const triggerScalar = triggerStrengthScalarFor(effect, intensity);
    if (effect === 'Off' || triggerScalar <= 0) return null;

    const { start, end } = triggerRangeUnitValuesFor(fromRaw, toRaw);
    const curve = normalizeTriggerCurve(curveRaw, fallbackCurve);
    const points = normalizeTriggerCurvePoints(pointsRaw, curve);

    if (side === 'l2') {
      const brake = forzaEffectForGraph('brake_resistance', effects);
      if (!brake || !routeHasL2(brake.route)) return null;
      const scalar = forzaEffectScalarForGraph(brake) * triggerScalar;
      if (scalar <= 0) return null;
      const brakeTuning = normalizeForzaBrakeTuningForGraph(brakeTuningRaw);
      const wall = brakeOvertravelWallPosition(
        start,
        end,
        brakeTuning.wallPosition,
        brakeTuning.guardMinEnd
      );
      const finalStopInput = brakeFullForcePosition(wall, end, brakeTuning.fullForceAt);
      const finalStopPosition = finalStopInput;
      return {
        start,
        end,
        wall,
        finalStopInput,
        finalStopPosition,
        curve,
        rampCurve: brakeTuning.rampCurve,
        points,
        baselineForce: scaledUnitForGraph(brakeTuning.baselineForce, scalar),
        normalForce: scaledUnitForGraph(brakeTuning.normalForce, scalar),
        endstopForce: scaledUnitForGraph(brakeTuning.endstopForce, scalar * brakeTuning.endstopBoost)
      };
    }

    const throttle = forzaEffectForGraph('throttle_resistance', effects);
    if (!throttle || !routeHasR2(throttle.route)) return null;
    const throttleTuning = normalizeForzaThrottleTuningForGraph(throttleTuningRaw);
    const scalar = forzaEffectScalarForGraph(throttle) * triggerScalar;
    if (scalar <= 0) return null;
    const wall = throttleOvertravelWallPosition(
      start,
      end,
      throttleTuning.wallPosition,
      throttleTuning.guardMinEnd
    );
    const rampStart = throttleOvertravelGuardActive(end, throttleTuning.guardMinEnd)
      ? throttleOvertravelRampStart(start, wall, throttleTuning.rampWidth)
      : undefined;
    return {
      start,
      end,
      wall,
      rampStart,
      curve,
      rampCurve: throttleTuning.rampCurve,
      points,
      baselineForce: scaledUnitForGraph(throttleTuning.baselineForce, scalar),
      normalForce: scaledUnitForGraph(throttleTuning.normalForce, scalar),
      endstopForce: scaledUnitForGraph(throttleTuning.endstopForce, scalar * throttleTuning.endstopBoost)
    };
  };

export const forzaTriggerCurveValueFor = (side: TriggerSide, position: number, model: ForzaTriggerForceModel | null) => {
    if (!model) return 0;
    const x = clampUnit(position);
    if (x <= model.start) return 0;
    if (side === 'l2') {
      if (model.finalStopInput !== undefined && x >= model.finalStopInput) return model.endstopForce;
      if (x >= model.end) return model.endstopForce;
      if (x >= model.wall) {
        return clampUnit(signalCurveForGraph(x, model.wall, model.finalStopInput ?? model.end, model.normalForce, model.endstopForce, model.rampCurve ?? FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE));
      }
      const active = clampUnit((x - model.start) / (Math.max(model.start + 0.01, model.wall) - model.start));
      const curved = triggerCurvePointOutput(model.points, active);
      return clampUnit(model.baselineForce + (model.normalForce - model.baselineForce) * curved);
    }
    if (x >= model.wall) return model.endstopForce;
    if (model.rampStart !== undefined && model.rampStart < model.wall && x >= model.rampStart) {
      return clampUnit(signalCurveForGraph(x, model.rampStart, model.wall, model.normalForce, model.endstopForce, model.rampCurve ?? FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE));
    }
    const editableEnd = model.rampStart ?? model.wall;
    const active = clampUnit((x - model.start) / (Math.max(model.start + 0.01, editableEnd) - model.start));
    const curved = triggerCurvePointOutput(model.points, active);
    return clampUnit(model.baselineForce + (model.normalForce - model.baselineForce) * curved);
  };

export const baseTriggerCurveValueFromParts = (
    position: number,
    start: number,
    end: number,
    points: TriggerCurvePoint[],
    strength: number
  ) => {
    if (strength <= 0) return 0;
    const x = clampUnit(position);
    const active = x <= start ? 0 : triggerCurvePointOutput(points, clampUnit((x - start) / (end - start)));
    return clampUnit(active * strength);
  };

export const triggerCurveValueFor = (
    side: TriggerSide,
    position: number,
    fromRaw: number | string,
    toRaw: number | string,
    curveRaw: number | string,
    pointsRaw: TriggerCurvePoint[],
    fallbackCurve: number,
    effect: string,
    intensity: string,
    displayMode: TriggerCurveDisplayMode,
    effects: ForzaEffectConfiguration[],
    brakeTuningRaw?: Partial<ForzaBrakeTuningConfiguration> | null,
    throttleTuningRaw?: Partial<ForzaThrottleTuningConfiguration> | null
  ) => {
    if (displayMode === 'forza') {
      return forzaTriggerCurveValueFor(
        side,
        position,
        forzaTriggerForceModelFor(side, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, effects, brakeTuningRaw, throttleTuningRaw)
      );
    }

    const range = triggerRangeValuesFor(fromRaw, toRaw);
    const start = range.from / 100;
    const end = Math.max(start + 0.01, range.to / 100);
    const curve = normalizeTriggerCurve(curveRaw, fallbackCurve);
    const points = normalizeTriggerCurvePoints(pointsRaw, curve);
    const strength = triggerStrengthScalarFor(effect, intensity);
    return baseTriggerCurveValueFromParts(position, start, end, points, strength);
  };

export const triggerCurvePathFor = (
    side: TriggerSide,
    fromRaw: number | string,
    toRaw: number | string,
    curveRaw: number | string,
    pointsRaw: TriggerCurvePoint[],
    fallbackCurve: number,
    effect: string,
    intensity: string,
    displayMode: TriggerCurveDisplayMode,
    effects: ForzaEffectConfiguration[],
    brakeTuningRaw?: Partial<ForzaBrakeTuningConfiguration> | null,
    throttleTuningRaw?: Partial<ForzaThrottleTuningConfiguration> | null
  ) => {
    const samplePositions = [...TRIGGER_CURVE_SAMPLE_POSITIONS];
    const model =
      displayMode === 'forza'
        ? forzaTriggerForceModelFor(side, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, effects, brakeTuningRaw, throttleTuningRaw)
        : null;
    if (model) {
      samplePositions.push(model.start, model.end, model.wall);
      if (model.rampStart !== undefined) samplePositions.push(model.rampStart);
      if (model.finalStopInput !== undefined) {
        samplePositions.push(Math.max(0, model.finalStopInput - 0.001), model.finalStopInput);
      }
    }

    const range = displayMode === 'base' ? triggerRangeValuesFor(fromRaw, toRaw) : null;
    const start = range ? range.from / 100 : 0;
    const end = range ? Math.max(start + 0.01, range.to / 100) : 1;
    const curve = displayMode === 'base' ? normalizeTriggerCurve(curveRaw, fallbackCurve) : fallbackCurve;
    const basePoints = displayMode === 'base' ? normalizeTriggerCurvePoints(pointsRaw, curve) : [];
    const strength = displayMode === 'base' ? triggerStrengthScalarFor(effect, intensity) : 0;
    const valueAt = (x: number) =>
      displayMode === 'forza'
        ? forzaTriggerCurveValueFor(side, x, model)
        : baseTriggerCurveValueFromParts(x, start, end, basePoints, strength);

    const pathPoints = [...new Set(samplePositions)]
      .sort((a, b) => a - b)
      .map((x) => {
        const y = 1 - valueAt(x);
        return `${(x * 100).toFixed(2)},${(y * 100).toFixed(2)}`;
      });
    return `M ${pathPoints.join(' L ')}`;
  };

export const curveControlPointsFor = (
    side: TriggerSide,
    fromRaw: number | string,
    toRaw: number | string,
    curveRaw: number | string,
    pointsRaw: TriggerCurvePoint[],
    fallbackCurve: number,
    effect: string,
    intensity: string,
    displayMode: TriggerCurveDisplayMode,
    effects: ForzaEffectConfiguration[],
    brakeTuningRaw?: Partial<ForzaBrakeTuningConfiguration> | null,
    throttleTuningRaw?: Partial<ForzaThrottleTuningConfiguration> | null
  ) => {
    const range = triggerRangeValuesFor(fromRaw, toRaw);
    const start = range.from / 100;
    const end = Math.max(start + 0.01, range.to / 100);
    const curve = normalizeTriggerCurve(curveRaw, fallbackCurve);
    const points = normalizeTriggerCurvePoints(pointsRaw, curve);
    const model =
      displayMode === 'forza'
        ? forzaTriggerForceModelFor(side, fromRaw, toRaw, curveRaw, points, fallbackCurve, effect, intensity, effects, brakeTuningRaw, throttleTuningRaw)
        : null;
    const editableEnd = model ? model.rampStart ?? model.wall : end;
    const span = Math.max(0.01, editableEnd - start);
    const strength = displayMode === 'base' ? triggerStrengthScalarFor(effect, intensity) : 0;
    const valueAt = (x: number) =>
      displayMode === 'forza'
        ? forzaTriggerCurveValueFor(side, x, model)
        : baseTriggerCurveValueFromParts(x, start, end, points, strength);

    return points.map((point, index) => {
      const active = point.input / 100;
      const x = clampUnit(start + span * active);
      const y = 1 - valueAt(x);
      return {
        index,
        input: point.input,
        output: point.output,
        locked: index === 0 || index === points.length - 1,
        x: (x * 100).toFixed(2),
        y: (clampUnit(y) * 100).toFixed(2)
      };
    });
  };

export const triggerCurveShapeView = (
    side: TriggerSide,
    fromRaw: number | string,
    toRaw: number | string,
    curveRaw: number | string,
    pointsRaw: TriggerCurvePoint[],
    fallbackCurve: number,
    effect: string,
    intensity: string,
    displayMode: TriggerCurveDisplayMode,
    effects: ForzaEffectConfiguration[],
    brakeTuningRaw?: Partial<ForzaBrakeTuningConfiguration> | null,
    throttleTuningRaw?: Partial<ForzaThrottleTuningConfiguration> | null
  ) => {
    const range = triggerRangeValuesFor(fromRaw, toRaw);
    const curvePoints = curveControlPointsFor(side, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, displayMode, effects, brakeTuningRaw, throttleTuningRaw);
    return {
      rangeStart: range.from.toFixed(2),
      rangeEnd: range.to.toFixed(2),
      rangeWidth: range.width.toFixed(2),
      path: triggerCurvePathFor(side, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, displayMode, effects, brakeTuningRaw, throttleTuningRaw),
      curvePoints
    };
  };

export const triggerCurveLiveView = (
    side: TriggerSide,
    fromRaw: number | string,
    toRaw: number | string,
    curveRaw: number | string,
    pointsRaw: TriggerCurvePoint[],
    fallbackCurve: number,
    livePress: number,
    effect: string,
    intensity: string,
    displayMode: TriggerCurveDisplayMode,
    effects: ForzaEffectConfiguration[],
    brakeTuningRaw?: Partial<ForzaBrakeTuningConfiguration> | null,
    throttleTuningRaw?: Partial<ForzaThrottleTuningConfiguration> | null
  ) => {
    const liveX = clampUnit(livePress) * 100;
    const liveY = 100 - triggerCurveValueFor(side, livePress, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, displayMode, effects, brakeTuningRaw, throttleTuningRaw) * 100;
    return {
      liveX: liveX.toFixed(2),
      liveY: liveY.toFixed(2)
    };
  };

export const triggerPressLabel = (value: number) => `${Math.round(clampUnit(value) * 100)}%`;
export const intensityTooltip = (meta: ForzaEffectMeta, intensity: number) =>
    `${meta.label} intensity is ${forzaIntensityPercent(intensity)}% (${clampForzaIntensity(intensity)} / 255 raw). This scales trigger, rumble, or LED output depending on signal and route.`;

export const routeTooltip = (route: ForzaEffectRoute) => routeTooltips[route] ?? 'Selects where DSCC sends this telemetry effect.';

export const brakeOvertravelWallPoint = (
    end: number,
    start = 0,
    brakeTuningRaw?: Partial<ForzaBrakeTuningConfiguration> | null
  ) => {
    const brakeTuning = normalizeForzaBrakeTuningForGraph(brakeTuningRaw);
    const startUnit = clampUnit(start / 100);
    const endUnit = Math.max(startUnit + 0.01, end / 100);
    return Math.round(
      brakeOvertravelWallPosition(
        startUnit,
        endUnit,
        brakeTuning.wallPosition,
        brakeTuning.guardMinEnd
      ) * 100
    );
  };
export const brakeFinalStopPoint = (
    start: number,
    end: number,
    brakeTuningRaw?: Partial<ForzaBrakeTuningConfiguration> | null
  ) => {
    const brakeTuning = normalizeForzaBrakeTuningForGraph(brakeTuningRaw);
    const startUnit = clampUnit(start / 100);
    const endUnit = Math.max(startUnit + 0.01, end / 100);
    const wall = brakeOvertravelWallPosition(
      startUnit,
      endUnit,
      brakeTuning.wallPosition,
      brakeTuning.guardMinEnd
    );
    return Math.round(brakeFullForcePosition(wall, endUnit, brakeTuning.fullForceAt) * 100);
  };
export const throttleOvertravelWallPoint = (
    end: number,
    start = 0,
    throttleTuningRaw?: Partial<ForzaThrottleTuningConfiguration> | null
  ) => {
    const throttleTuning = normalizeForzaThrottleTuningForGraph(throttleTuningRaw);
    const startUnit = clampUnit(start / 100);
    const endUnit = Math.max(startUnit + 0.01, end / 100);
    return Math.round(
      throttleOvertravelWallPosition(
        startUnit,
        endUnit,
        throttleTuning.wallPosition,
        throttleTuning.guardMinEnd
      ) * 100
    );
  };
export const throttleOvertravelRampPoint = (
    end: number,
    start = 0,
    throttleTuningRaw?: Partial<ForzaThrottleTuningConfiguration> | null
  ) => {
    const throttleTuning = normalizeForzaThrottleTuningForGraph(throttleTuningRaw);
    const startUnit = clampUnit(start / 100);
    const endUnit = Math.max(startUnit + 0.01, end / 100);
    const wall = throttleOvertravelWallPosition(
      startUnit,
      endUnit,
      throttleTuning.wallPosition,
      throttleTuning.guardMinEnd
    );
    if (!throttleOvertravelGuardActive(endUnit, throttleTuning.guardMinEnd)) {
      return Math.round(endstopWallPosition(startUnit, endUnit) * 100);
    }
    return Math.round(throttleOvertravelRampStart(startUnit, wall, throttleTuning.rampWidth) * 100);
  };

export const triggerRangeTooltip = (
    side: 'L2' | 'R2',
    edge: 'from' | 'to',
    value: number,
    startValue = 0,
    brakeTuningRaw?: Partial<ForzaBrakeTuningConfiguration> | null,
    throttleTuningRaw?: Partial<ForzaThrottleTuningConfiguration> | null
  ) =>
    edge === 'from'
      ? `${side} starts building force at ${value}% trigger travel. Raising this creates more free travel before resistance begins.`
      : side === 'L2'
        ? `${side} end-load ramp begins near ${brakeOvertravelWallPoint(value, startValue, brakeTuningRaw)}%, then full brake force takes over near ${brakeFinalStopPoint(startValue, value, brakeTuningRaw)}%. ABS/handbrake priority effects can still take over.`
        : `${side} stays light first, ramps from about ${throttleOvertravelRampPoint(value, startValue, throttleTuningRaw)}%, then holds max resistance from about ${throttleOvertravelWallPoint(value, startValue, throttleTuningRaw)}% through full travel unless shift/rev priority effects take over.`;

export const triggerCurveTooltip = (side: 'L2' | 'R2', value: number) =>
    `${side} curve is ${value.toFixed(2)}. Drag the dots for a custom response, or move this slider to regenerate a smooth exponent curve.`;
