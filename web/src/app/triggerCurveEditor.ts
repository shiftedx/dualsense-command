import {
  TRIGGER_CURVE_POINT_MAX,
  TRIGGER_CURVE_POINT_MIN,
  clampUnit,
  defaultTriggerCurve,
  normalizeTriggerCurvePoints,
  normalizeTriggerPercent
} from '../lib/features/haptics/hapticsModel';
import {
  forzaTriggerForceModelFor,
  triggerCurveLiveView,
  triggerCurveShapeView,
  triggerCurveValueFor,
  triggerRangeValuesFor,
  triggerStrengthScalarFor,
  type TriggerCurveDisplayMode,
  type TriggerSide
} from '../lib/features/haptics/hapticsCurvePresentation';
import { clamp } from './hapticsState';
import type {
  ForzaBrakeTuningConfiguration,
  ForzaEffectConfiguration,
  ForzaThrottleTuningConfiguration,
  TriggerCurvePoint
} from '../lib/types';

export type TriggerCurveEditorContext = {
  side: TriggerSide;
  from: number;
  to: number;
  curve: number;
  points: TriggerCurvePoint[];
  triggerEffect: string;
  triggerIntensity: string;
  displayMode: TriggerCurveDisplayMode;
  forzaEffects: ForzaEffectConfiguration[];
  forzaBrakeTuning: ForzaBrakeTuningConfiguration;
  forzaThrottleTuning: ForzaThrottleTuningConfiguration;
};

// Identity helper so callers can build the context inside a Svelte reactive
// statement (object literals keep the dependency tracing that a plain
// function call would hide).
export const triggerCurveEditorContext = (context: TriggerCurveEditorContext): TriggerCurveEditorContext =>
  context;

export type CurveHoverState = {
  side: TriggerSide;
  x: number;
  y: number;
  left: number;
  top: number;
};

export type CurveDragPoint = { side: TriggerSide; index: number };

export type TriggerRangeEdge = 'from' | 'to';

export const triggerRangeWithEdgeSet = (
  range: { from: number; to: number },
  edge: TriggerRangeEdge,
  rawValue: number | string
): { from: number; to: number } => {
  const value = normalizeTriggerPercent(rawValue);
  return edge === 'from'
    ? { from: Math.min(value, range.to), to: range.to }
    : { from: range.from, to: Math.max(value, range.from) };
};

export const curveValueAt = (context: TriggerCurveEditorContext, position: number) =>
  triggerCurveValueFor(
    context.side,
    position,
    context.from,
    context.to,
    context.curve,
    context.points,
    defaultTriggerCurve(context.side),
    context.triggerEffect,
    context.triggerIntensity,
    context.displayMode,
    context.forzaEffects,
    context.forzaBrakeTuning,
    context.forzaThrottleTuning
  );

export const curveShapeViewFor = (context: TriggerCurveEditorContext) =>
  triggerCurveShapeView(
    context.side,
    context.from,
    context.to,
    context.curve,
    context.points,
    defaultTriggerCurve(context.side),
    context.triggerEffect,
    context.triggerIntensity,
    context.displayMode,
    context.forzaEffects,
    context.forzaBrakeTuning,
    context.forzaThrottleTuning
  );

export const curveLiveViewFor = (context: TriggerCurveEditorContext, livePress: number) =>
  triggerCurveLiveView(
    context.side,
    context.from,
    context.to,
    context.curve,
    context.points,
    defaultTriggerCurve(context.side),
    livePress,
    context.triggerEffect,
    context.triggerIntensity,
    context.displayMode,
    context.forzaEffects,
    context.forzaBrakeTuning,
    context.forzaThrottleTuning
  );

export const curveHoverFor = (context: TriggerCurveEditorContext, x: number): CurveHoverState => {
  const y = curveValueAt(context, x);
  return {
    side: context.side,
    x,
    y,
    left: x * 100,
    top: (1 - y) * 100
  };
};

export const curveGraphPointFromPointer = (event: PointerEvent, target: HTMLElement) => {
  const rect = target.getBoundingClientRect();
  const x = clampUnit((event.clientX - rect.left) / Math.max(1, rect.width));
  const output = clampUnit(1 - (event.clientY - rect.top) / Math.max(1, rect.height));
  return { x, output };
};

export const curvePointFromGraphPoint = (
  context: TriggerCurveEditorContext,
  input: number,
  output: number
): TriggerCurvePoint => {
  const range = triggerRangeValuesFor(context.from, context.to);
  const start = range.from / 100;
  const end = Math.max(start + 0.01, range.to / 100);
  let activeTravel = clamp((input - start) / (end - start), 0.01, 0.99);
  let normalizedOutput = output;

  if (context.displayMode === 'forza') {
    const model = forzaTriggerForceModelFor(
      context.side,
      context.from,
      context.to,
      context.curve,
      context.points,
      defaultTriggerCurve(context.side),
      context.triggerEffect,
      context.triggerIntensity,
      context.forzaEffects,
      context.forzaBrakeTuning,
      context.forzaThrottleTuning
    );
    if (model && model.normalForce > model.baselineForce) {
      const editableEnd = model.rampStart ?? model.wall;
      const editableInput = clamp(input, model.start + 0.0001, Math.max(model.start + 0.0001, editableEnd - 0.0001));
      activeTravel = clamp((editableInput - model.start) / (editableEnd - model.start), 0.01, 0.99);
      normalizedOutput = clamp((Math.min(output, model.normalForce) - model.baselineForce) / (model.normalForce - model.baselineForce), 0.01, 0.99);
    }
  } else {
    const strength = triggerStrengthScalarFor(context.triggerEffect, context.triggerIntensity);
    normalizedOutput = clamp(strength > 0 ? output / strength : output, 0.01, 0.99);
  }

  return {
    input: normalizeTriggerPercent(activeTravel * 100),
    output: normalizeTriggerPercent(normalizedOutput * 100)
  };
};

export type CurvePointEdit = {
  points: TriggerCurvePoint[] | null;
  index: number;
};

const normalizedContextPoints = (context: TriggerCurveEditorContext) =>
  normalizeTriggerCurvePoints(context.points, context.curve);

export const withCurvePointSet = (
  context: TriggerCurveEditorContext,
  index: number,
  point: TriggerCurvePoint
): CurvePointEdit => {
  const current = normalizedContextPoints(context);
  if (index <= 0 || index >= current.length - 1) return { points: null, index };
  const previous = current[index - 1];
  const next = current[index + 1];
  current[index] = {
    input: normalizeTriggerPercent(clamp(point.input, previous.input + 1, next.input - 1)),
    output: normalizeTriggerPercent(point.output)
  };
  return { points: current, index };
};

export const withCurvePointAddedOrSelected = (
  context: TriggerCurveEditorContext,
  point: TriggerCurvePoint
): CurvePointEdit => {
  const current = normalizedContextPoints(context);
  if (current.length >= TRIGGER_CURVE_POINT_MAX) {
    let nearest = 1;
    let distance = Number.POSITIVE_INFINITY;
    for (let index = 1; index < current.length - 1; index += 1) {
      const nextDistance = Math.abs(current[index].input - point.input);
      if (nextDistance < distance) {
        distance = nextDistance;
        nearest = index;
      }
    }
    return withCurvePointSet(context, nearest, point);
  }

  const nextPoints = [...current, point].sort((a, b) => a.input - b.input);
  const index = Math.max(1, Math.min(nextPoints.length - 2, nextPoints.findIndex((candidate) => candidate === point)));
  return { points: nextPoints, index };
};

export const withCurvePointAdded = (context: TriggerCurveEditorContext): TriggerCurvePoint[] | null => {
  const current = normalizedContextPoints(context);
  if (current.length >= TRIGGER_CURVE_POINT_MAX) return null;

  let bestIndex = 0;
  let bestGap = 0;
  for (let index = 0; index < current.length - 1; index += 1) {
    const gap = current[index + 1].input - current[index].input;
    if (gap > bestGap) {
      bestGap = gap;
      bestIndex = index;
    }
  }
  const left = current[bestIndex];
  const right = current[bestIndex + 1];
  const input = normalizeTriggerPercent((left.input + right.input) / 2);
  const output = normalizeTriggerPercent((left.output + right.output) / 2);
  return [...current, { input, output }];
};

export const withCurvePointRemoved = (context: TriggerCurveEditorContext): TriggerCurvePoint[] | null => {
  const current = normalizedContextPoints(context);
  if (current.length <= TRIGGER_CURVE_POINT_MIN) return null;

  let removeIndex = current.length - 2;
  let smallestBend = Number.POSITIVE_INFINITY;
  for (let index = 1; index < current.length - 1; index += 1) {
    const left = current[index - 1];
    const point = current[index];
    const right = current[index + 1];
    const expected = left.output + ((right.output - left.output) * (point.input - left.input)) / Math.max(1, right.input - left.input);
    const bend = Math.abs(point.output - expected);
    if (bend < smallestBend) {
      smallestBend = bend;
      removeIndex = index;
    }
  }
  return current.filter((_, index) => index !== removeIndex);
};

export type CurveDragOptions = {
  applyInitialEvent?: boolean;
  onPoint: (point: { x: number; output: number }) => void;
  onEnd: () => void;
};

export const beginCurveDrag = (event: PointerEvent, target: HTMLElement, options: CurveDragOptions) => {
  target.setPointerCapture(event.pointerId);

  // Pointer capture pins the target for the whole drag, so one rect read at
  // drag start replaces a forced layout per pointermove event.
  const rect = target.getBoundingClientRect();
  const pointFrom = (pointerEvent: PointerEvent) => ({
    x: clampUnit((pointerEvent.clientX - rect.left) / Math.max(1, rect.width)),
    output: clampUnit(1 - (pointerEvent.clientY - rect.top) / Math.max(1, rect.height))
  });

  // High-rate mice deliver up to 1000 pointermove events/s; coalesce to one
  // application per animation frame.
  let pending: PointerEvent | null = null;
  let frame = 0;
  const flush = () => {
    frame = 0;
    if (pending) {
      const next = pending;
      pending = null;
      options.onPoint(pointFrom(next));
    }
  };
  const applyPoint = (pointerEvent: PointerEvent) => {
    pending = pointerEvent;
    if (!frame) frame = requestAnimationFrame(flush);
  };

  const stopDrag = () => {
    if (frame) cancelAnimationFrame(frame);
    flush();
    options.onEnd();
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    target.removeEventListener('pointermove', applyPoint);
    target.removeEventListener('pointerup', stopDrag);
    target.removeEventListener('pointercancel', stopDrag);
  };

  if (options.applyInitialEvent) options.onPoint(pointFrom(event));
  target.addEventListener('pointermove', applyPoint);
  target.addEventListener('pointerup', stopDrag);
  target.addEventListener('pointercancel', stopDrag);
};
