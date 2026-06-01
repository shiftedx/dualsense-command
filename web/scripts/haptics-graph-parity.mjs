import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(scriptDir, '..');
const repoRoot = resolve(scriptDir, '..', '..');
const server = await createServer({
  root: webRoot,
  appType: 'custom',
  logLevel: 'silent',
  server: { middlewareMode: true }
});
const hapticsOptions = await server.ssrLoadModule('/src/lib/features/haptics/hapticsOptions.ts');
const hapticsCurvePresentation = await server.ssrLoadModule('/src/lib/features/haptics/hapticsCurvePresentation.ts');
const { forzaTriggerCurveValueFor, forzaTriggerForceModelFor } = hapticsCurvePresentation;
const runtimeConstantsPath = resolve(repoRoot, 'crates/dscc-agent/src/runtime_constants.rs');
const runtimeConstants = readFileSync(runtimeConstantsPath, 'utf8');

const frontendConstants = {
  FORZA_BRAKE_BASELINE_FORCE: hapticsOptions.FORZA_BRAKE_BASELINE_FORCE,
  FORZA_BRAKE_ENDSTOP_FORCE: hapticsOptions.FORZA_BRAKE_ENDSTOP_FORCE,
  FORZA_BRAKE_ENDSTOP_FORCE_BOOST: hapticsOptions.FORZA_BRAKE_ENDSTOP_FORCE_BOOST,
  FORZA_BRAKE_FULL_FORCE_INPUT: hapticsOptions.FORZA_BRAKE_FULL_FORCE_INPUT,
  FORZA_BRAKE_NORMAL_FORCE: hapticsOptions.FORZA_BRAKE_NORMAL_FORCE,
  FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE: hapticsOptions.FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE,
  FORZA_BRAKE_OVERTRAVEL_MIN_POSITION: hapticsOptions.FORZA_BRAKE_OVERTRAVEL_MIN_POSITION,
  FORZA_BRAKE_OVERTRAVEL_WALL_POSITION: hapticsOptions.FORZA_BRAKE_OVERTRAVEL_WALL_POSITION,
  FORZA_ENDSTOP_WALL_OFFSET: hapticsOptions.FORZA_ENDSTOP_WALL_OFFSET,
  FORZA_THROTTLE_BASELINE_FORCE: hapticsOptions.FORZA_THROTTLE_BASELINE_FORCE,
  FORZA_THROTTLE_ENDSTOP_FORCE: hapticsOptions.FORZA_THROTTLE_ENDSTOP_FORCE,
  FORZA_THROTTLE_ENDSTOP_FORCE_BOOST: hapticsOptions.FORZA_THROTTLE_ENDSTOP_FORCE_BOOST,
  FORZA_THROTTLE_NORMAL_FORCE: hapticsOptions.FORZA_THROTTLE_NORMAL_FORCE,
  FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION: hapticsOptions.FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION,
  FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE: hapticsOptions.FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE,
  FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH: hapticsOptions.FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH,
  FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION: hapticsOptions.FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION
};

const constRegex = /pub\(crate\)\s+const\s+(FORZA_[A-Z0-9_]+):\s+(?:f64|u8)\s+=\s+([^;]+);/g;
const rustConstants = new Map();
for (const match of runtimeConstants.matchAll(constRegex)) {
  const [, name, expression] = match;
  if (!/^[\d\s./*+\-()]+$/.test(expression)) continue;
  rustConstants.set(name, Function(`"use strict"; return (${expression});`)());
}

const assertNear = (actual, expected, label, tolerance = 1e-9) => {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${label}: expected ${expected}, got ${actual}`
  );
};

for (const [name, value] of Object.entries(frontendConstants)) {
  assert.ok(rustConstants.has(name), `Rust runtime constant ${name} is missing`);
  assertNear(value, rustConstants.get(name), name, 1e-12);
}

const clamp = (value, min = 0, max = 1) => Math.max(min, Math.min(max, value));
const triggerRangeEnd = (from, to) => {
  const startPercent = Math.min(from, 100);
  const end = Math.min(Math.max(to, startPercent), 100) / 100;
  return Math.max(startPercent / 100 + 0.01, end);
};
const signalCurve = (input, inputMin, inputMax, outputMin, outputMax, exponent) => {
  if (inputMin === inputMax || exponent <= 0) return outputMin;
  const ratio = clamp((input - inputMin) / (inputMax - inputMin));
  return outputMin + (outputMax - outputMin) * Math.pow(ratio, exponent);
};
const signalPoints = (points, active) => {
  const x = clamp(active);
  for (let index = 0; index < points.length - 1; index += 1) {
    const left = points[index];
    const right = points[index + 1];
    const leftInput = left.input / 100;
    const rightInput = right.input / 100;
    if (x >= leftInput && x <= rightInput) {
      if (rightInput <= leftInput) return right.output / 100;
      const ratio = (x - leftInput) / (rightInput - leftInput);
      return (left.output + (right.output - left.output) * ratio) / 100;
    }
  }
  return points.at(-1).output / 100;
};
const scaledUnit = (value, scalar) => clamp(value * scalar);
const defaultThrottleTuning = {
  baselineForce: rustConstants.get('FORZA_THROTTLE_BASELINE_FORCE'),
  normalForce: rustConstants.get('FORZA_THROTTLE_NORMAL_FORCE'),
  endstopForce: rustConstants.get('FORZA_THROTTLE_ENDSTOP_FORCE'),
  endstopBoost: rustConstants.get('FORZA_THROTTLE_ENDSTOP_FORCE_BOOST'),
  wallPosition: rustConstants.get('FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION'),
  guardMinEnd: rustConstants.get('FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION'),
  rampWidth: rustConstants.get('FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH'),
  rampCurve: rustConstants.get('FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE')
};
const defaultBrakeTuning = {
  baselineForce: rustConstants.get('FORZA_BRAKE_BASELINE_FORCE'),
  normalForce: rustConstants.get('FORZA_BRAKE_NORMAL_FORCE'),
  endstopForce: rustConstants.get('FORZA_BRAKE_ENDSTOP_FORCE'),
  endstopBoost: rustConstants.get('FORZA_BRAKE_ENDSTOP_FORCE_BOOST'),
  wallPosition: rustConstants.get('FORZA_BRAKE_OVERTRAVEL_WALL_POSITION'),
  guardMinEnd: rustConstants.get('FORZA_BRAKE_OVERTRAVEL_MIN_POSITION'),
  fullForceAt: rustConstants.get('FORZA_BRAKE_FULL_FORCE_INPUT'),
  rampCurve: rustConstants.get('FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE')
};
const brakeWall = (start, end, brakeTuning) =>
  end >= brakeTuning.guardMinEnd
    ? clamp(Math.min(end, brakeTuning.wallPosition), start, end)
    : clamp(end - rustConstants.get('FORZA_ENDSTOP_WALL_OFFSET'), start, end);
const brakeFullForce = (wall, end, brakeTuning) => clamp(brakeTuning.fullForceAt, wall, end);
const throttleWall = (start, end, throttleTuning) =>
  end >= throttleTuning.guardMinEnd
    ? clamp(Math.min(end, throttleTuning.wallPosition), start, end)
    : clamp(end - rustConstants.get('FORZA_ENDSTOP_WALL_OFFSET'), start, end);
const throttleRampStart = (start, wall, throttleTuning) =>
  clamp(Math.round((wall - throttleTuning.rampWidth) * 1000) / 1000, start, wall);

const expectedBackendForce = (side, position, config) => {
  const x = clamp(position);
  const triggerScalar = config.triggerIntensityScalar;
  if (triggerScalar <= 0) return 0;
  const effect = side === 'l2' ? config.brakeEffect : config.throttleEffect;
  if (!effect.enabled) return 0;
  const effectScalar = effect.intensity / 100;
  if (effectScalar <= 0) return 0;

  const start = config[`${side}From`] / 100;
  const end = triggerRangeEnd(config[`${side}From`], config[`${side}To`]);
  if (x <= start) return 0;

  if (side === 'l2') {
    const brakeTuning = config.brakeTuning ?? defaultBrakeTuning;
    const wall = brakeWall(start, end, brakeTuning);
    const finalStopInput = brakeFullForce(wall, end, brakeTuning);
    const baseline = scaledUnit(brakeTuning.baselineForce, effectScalar * triggerScalar);
    const normal = scaledUnit(brakeTuning.normalForce, effectScalar * triggerScalar);
    const endstop = scaledUnit(
      brakeTuning.endstopForce,
      effectScalar * triggerScalar * brakeTuning.endstopBoost
    );
    if (x >= finalStopInput) return endstop;
    if (end >= brakeTuning.guardMinEnd && wall < finalStopInput && x >= wall) {
      return clamp(signalCurve(x, wall, finalStopInput, normal, endstop, brakeTuning.rampCurve));
    }
    const normalEnd = Math.max(start + 0.01, end >= brakeTuning.guardMinEnd ? wall : Math.min(wall, end));
    const active = clamp((x - start) / (normalEnd - start));
    return clamp(baseline + (normal - baseline) * signalPoints(config.l2Points, active));
  }

  const throttleTuning = config.throttleTuning ?? defaultThrottleTuning;
  const wall = throttleWall(start, end, throttleTuning);
  const rampStart =
    end >= throttleTuning.guardMinEnd
      ? throttleRampStart(start, wall, throttleTuning)
      : undefined;
  const baseline = scaledUnit(throttleTuning.baselineForce, effectScalar * triggerScalar);
  const normal = scaledUnit(throttleTuning.normalForce, effectScalar * triggerScalar);
  const endstop = scaledUnit(
    throttleTuning.endstopForce,
    effectScalar * triggerScalar * throttleTuning.endstopBoost
  );
  if (x >= wall) return endstop;
  if (rampStart !== undefined && rampStart < wall && x >= rampStart) {
    return clamp(signalCurve(x, rampStart, wall, normal, endstop, throttleTuning.rampCurve));
  }
  const normalEnd = Math.max(start + 0.01, rampStart !== undefined && rampStart < wall ? rampStart : wall);
  const active = clamp((x - start) / (normalEnd - start));
  return clamp(baseline + (normal - baseline) * signalPoints(config.r2Points, active));
};

const l2Points = [
  { input: 0, output: 0 },
  { input: 12, output: 18 },
  { input: 25, output: 44 },
  { input: 40, output: 68 },
  { input: 58, output: 86 },
  { input: 78, output: 96 },
  { input: 100, output: 100 }
];
const r2Points = [
  { input: 0, output: 0 },
  { input: 25, output: 3 },
  { input: 50, output: 21 },
  { input: 75, output: 52 },
  { input: 100, output: 100 }
];
const effects = [
  { id: 'brake_resistance', enabled: true, intensity: 100, route: 'l2' },
  { id: 'throttle_resistance', enabled: true, intensity: 100, route: 'r2' }
];
const scenarios = [
  {
    name: 'default forza pedal ranges',
    l2From: 6,
    l2To: 100,
    r2From: 4,
    r2To: 100,
    l2Points,
    r2Points,
    triggerIntensityScalar: 1,
    brakeEffect: effects[0],
    throttleEffect: effects[1]
  },
  {
    name: 'custom shortened pedal ranges',
    l2From: 20,
    l2To: 60,
    r2From: 10,
    r2To: 50,
    l2Points,
    r2Points,
    triggerIntensityScalar: 1,
    brakeEffect: effects[0],
    throttleEffect: effects[1]
  },
  {
    name: 'custom throttle tuning',
    l2From: 6,
    l2To: 100,
    r2From: 4,
    r2To: 100,
    l2Points,
    r2Points,
    triggerIntensityScalar: 1,
    brakeEffect: effects[0],
    throttleEffect: effects[1],
    throttleTuning: {
      ...defaultThrottleTuning,
      baselineForce: 0.05,
      normalForce: 0.2,
      endstopForce: 0.5,
      endstopBoost: 1.5,
      wallPosition: 0.72,
      guardMinEnd: 0.65,
      rampWidth: 0.12,
      rampCurve: 1.1
    }
  },
  {
    name: 'custom brake wall tuning',
    l2From: 5,
    l2To: 100,
    r2From: 4,
    r2To: 100,
    l2Points,
    r2Points,
    triggerIntensityScalar: 1,
    brakeEffect: effects[0],
    throttleEffect: effects[1],
    brakeTuning: {
      ...defaultBrakeTuning,
      baselineForce: 0.4,
      normalForce: 0.72,
      endstopForce: 1,
      endstopBoost: 1.3,
      wallPosition: 0.58,
      guardMinEnd: 0.52,
      fullForceAt: 0.86,
      rampCurve: 1.45
    }
  }
];

for (const scenario of scenarios) {
  for (const side of ['l2', 'r2']) {
    const points = side === 'l2' ? scenario.l2Points : scenario.r2Points;
    const model = forzaTriggerForceModelFor(
      side,
      scenario[`${side}From`],
      scenario[`${side}To`],
      side === 'l2' ? 1.45 : 2.25,
      points,
      side === 'l2' ? 1.45 : 2.25,
      'Adaptive resistance',
      'Strong (Standard)',
      effects,
      scenario.brakeTuning,
      scenario.throttleTuning
    );
    assert.ok(model, `${scenario.name} ${side}: frontend model should be present`);
    const samples = new Set([0, 0.03, 0.06, 0.2, 0.4, 0.48, 0.6, 0.75, 0.8, 0.84, 0.94, 1]);
    if (model.finalStopInput !== undefined) {
      samples.add(Math.max(0, model.finalStopInput - 0.001));
      samples.add(model.finalStopInput);
    }
    if (model.rampStart !== undefined) samples.add(model.rampStart);
    samples.add(model.wall);
    samples.add(model.end);

    for (const position of [...samples].sort((a, b) => a - b)) {
      const actual = forzaTriggerCurveValueFor(side, position, model);
      const expected = expectedBackendForce(side, position, scenario);
      assertNear(actual, expected, `${scenario.name} ${side} graph @ ${position.toFixed(3)}`, 1e-6);
    }
  }
}

await server.close();
console.log(`haptics graph parity: ${Object.keys(frontendConstants).length} constants and ${scenarios.length * 2} trigger models match backend runtime math`);
