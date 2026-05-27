import type { ForzaEffectConfiguration, ForzaEffectRoute, TriggerCurvePoint } from '../../types';
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
  FORZA_BRAKE_NORMAL_FORCE,
  FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE,
  FORZA_BRAKE_OVERTRAVEL_RAMP_WIDTH,
  FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION,
  FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET,
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
  curve: number;
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

export const endstopWallPosition = (start: number, end: number) => clamp(end - FORZA_ENDSTOP_WALL_OFFSET, start, end);
export const brakeOvertravelGuardActive = (end: number) => end >= FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION;
export const brakeOvertravelWallPosition = (start: number, end: number) =>
    brakeOvertravelGuardActive(end)
      ? clamp(Math.max(FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION, end - FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET), start, end)
      : endstopWallPosition(start, end);
export const brakeOvertravelRampStart = (start: number, wall: number) =>
    clamp(wall - FORZA_BRAKE_OVERTRAVEL_RAMP_WIDTH, start, wall);
export const throttleOvertravelGuardActive = (end: number) => end >= FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION;
export const throttleOvertravelWallPosition = (start: number, end: number) =>
    throttleOvertravelGuardActive(end)
      ? clamp(Math.min(end, FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION), start, end)
      : endstopWallPosition(start, end);
export const throttleOvertravelRampStart = (start: number, wall: number) =>
    clamp(Math.round((wall - FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH) * 1000) / 1000, start, wall);
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
    effects: ForzaEffectConfiguration[]
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
      const wall = brakeOvertravelWallPosition(start, end);
      const rampStart = brakeOvertravelGuardActive(end) ? brakeOvertravelRampStart(start, wall) : undefined;
      return {
        start,
        end,
        wall,
        rampStart,
        curve,
        points,
        baselineForce: scaledUnitForGraph(FORZA_BRAKE_BASELINE_FORCE, scalar),
        normalForce: scaledUnitForGraph(FORZA_BRAKE_NORMAL_FORCE, scalar),
        endstopForce: scaledUnitForGraph(FORZA_BRAKE_ENDSTOP_FORCE, scalar * FORZA_BRAKE_ENDSTOP_FORCE_BOOST)
      };
    }

    const throttle = forzaEffectForGraph('throttle_resistance', effects);
    if (!throttle || !routeHasR2(throttle.route)) return null;
    const scalar = forzaEffectScalarForGraph(throttle) * triggerScalar;
    if (scalar <= 0) return null;
    const wall = throttleOvertravelWallPosition(start, end);
    const rampStart = throttleOvertravelGuardActive(end) ? throttleOvertravelRampStart(start, wall) : undefined;
    return {
      start,
      end,
      wall,
      rampStart,
      curve,
      points,
      baselineForce: scaledUnitForGraph(FORZA_THROTTLE_BASELINE_FORCE, scalar),
      normalForce: scaledUnitForGraph(FORZA_THROTTLE_NORMAL_FORCE, scalar),
      endstopForce: scaledUnitForGraph(FORZA_THROTTLE_ENDSTOP_FORCE, scalar * FORZA_THROTTLE_ENDSTOP_FORCE_BOOST)
    };
  };

export const forzaTriggerCurveValueFor = (side: TriggerSide, position: number, model: ForzaTriggerForceModel | null) => {
    if (!model) return 0;
    const x = clampUnit(position);
    if (x <= model.start) return 0;
    if (x >= model.wall) return model.endstopForce;
    if (model.rampStart !== undefined && model.rampStart < model.wall && x >= model.rampStart) {
      const rampCurve = side === 'l2' ? FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE : FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE;
      return clampUnit(signalCurveForGraph(x, model.rampStart, model.wall, model.normalForce, model.endstopForce, rampCurve));
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
    effects: ForzaEffectConfiguration[]
  ) => {
    if (displayMode === 'forza') {
      return forzaTriggerCurveValueFor(
        side,
        position,
        forzaTriggerForceModelFor(side, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, effects)
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
    effects: ForzaEffectConfiguration[]
  ) => {
    const samplePositions = [...TRIGGER_CURVE_SAMPLE_POSITIONS];
    const model =
      displayMode === 'forza'
        ? forzaTriggerForceModelFor(side, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, effects)
        : null;
    if (model) {
      samplePositions.push(model.start, model.end, model.wall);
      if (model.rampStart !== undefined) samplePositions.push(model.rampStart);
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
    effects: ForzaEffectConfiguration[]
  ) => {
    const range = triggerRangeValuesFor(fromRaw, toRaw);
    const start = range.from / 100;
    const end = Math.max(start + 0.01, range.to / 100);
    const curve = normalizeTriggerCurve(curveRaw, fallbackCurve);
    const points = normalizeTriggerCurvePoints(pointsRaw, curve);
    const model =
      displayMode === 'forza'
        ? forzaTriggerForceModelFor(side, fromRaw, toRaw, curveRaw, points, fallbackCurve, effect, intensity, effects)
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
    effects: ForzaEffectConfiguration[]
  ) => {
    const range = triggerRangeValuesFor(fromRaw, toRaw);
    const curvePoints = curveControlPointsFor(side, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, displayMode, effects);
    return {
      rangeStart: range.from.toFixed(2),
      rangeEnd: range.to.toFixed(2),
      rangeWidth: range.width.toFixed(2),
      path: triggerCurvePathFor(side, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, displayMode, effects),
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
    effects: ForzaEffectConfiguration[]
  ) => {
    const liveX = clampUnit(livePress) * 100;
    const liveY = 100 - triggerCurveValueFor(side, livePress, fromRaw, toRaw, curveRaw, pointsRaw, fallbackCurve, effect, intensity, displayMode, effects) * 100;
    return {
      liveX: liveX.toFixed(2),
      liveY: liveY.toFixed(2)
    };
  };

export const triggerPressLabel = (value: number) => `${Math.round(clampUnit(value) * 100)}%`;
export const intensityTooltip = (meta: ForzaEffectMeta, intensity: number) =>
    `${meta.label} intensity is ${forzaIntensityPercent(intensity)}% (${clampForzaIntensity(intensity)} / 255 raw). This scales trigger, rumble, or LED output depending on signal and route.`;

export const routeTooltip = (route: ForzaEffectRoute) => routeTooltips[route] ?? 'Selects where DSCC sends this telemetry effect.';

export const brakeOvertravelWallPoint = (end: number) =>
    Math.round(end >= FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION * 100
      ? Math.min(end, Math.max(FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION * 100, end - FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET * 100))
      : Math.max(0, end - FORZA_ENDSTOP_WALL_OFFSET * 100));
export const throttleOvertravelWallPoint = (end: number) =>
    Math.round(end >= FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION * 100
      ? Math.min(end, FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION * 100)
      : Math.max(0, end - FORZA_ENDSTOP_WALL_OFFSET * 100));
export const throttleOvertravelRampPoint = (end: number) => {
    if (end < FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION * 100) return Math.max(0, end - 3);
    const wall = throttleOvertravelWallPoint(end);
    return Math.round(Math.min(end, Math.max(0, wall - FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH * 100)));
  };

export const triggerRangeTooltip = (side: 'L2' | 'R2', edge: 'from' | 'to', value: number) =>
    edge === 'from'
      ? `${side} starts building force at ${value}% trigger travel. Raising this creates more free travel before resistance begins.`
      : side === 'L2'
        ? `${side} max resistance begins near ${brakeOvertravelWallPoint(value)}% and holds through the end of travel, while ABS/handbrake priority effects can still take over.`
        : `${side} stays light first, ramps from about ${throttleOvertravelRampPoint(value)}%, then holds max resistance from about ${throttleOvertravelWallPoint(value)}% through full travel unless shift/rev priority effects take over.`;

export const triggerCurveTooltip = (side: 'L2' | 'R2', value: number) =>
    `${side} curve is ${value.toFixed(2)}. Drag the dots for a custom response, or move this slider to regenerate a smooth exponent curve.`;
