import assert from 'node:assert/strict';
import { performance } from 'node:perf_hooks';

import {
  buildSteamBindingBySlotKey,
  chipDisplayLabel,
  createMappingChipModels,
  createSteamMirrorGroups,
  parseSteamBindingTriple,
  steamBindingKey,
  steamBindingSlots,
  steamBindingTargetPart
} from '../src/lib/features/buttonMapping/buttonMapping.ts';

const samples = 300;
const bindings = steamBindingSlots.flatMap((slot, index) => {
  const inputId = slot.inputIds.at(-1) ?? slot.key;
  return [
    {
      input: slot.label,
      inputId,
      binding: `${slot.label} Binding`,
      rawBinding: `key_press KEY_${index}, , ${slot.label} action`,
      kind: 'Key',
      source: slot.source ?? null,
      sourceMode: slot.group,
      activator: index % 3 === 0 ? 'Long Press' : 'Full Press',
      groupId: `${index % 5}`
    }
  ];
});

const p95 = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(sorted.length * 0.95) - 1)];
};

const time = (fn) => {
  const start = performance.now();
  fn();
  return performance.now() - start;
};

const lookupDurations = [];
const modelDurations = [];
const parseDurations = [];

for (let i = 0; i < samples; i += 1) {
  lookupDurations.push(time(() => {
    const bySlot = buildSteamBindingBySlotKey(bindings, steamBindingSlots);
    assert.equal(bySlot.get('cross')?.input, 'Cross');
    assert.equal(bySlot.get('edgeBackLeft')?.input, 'Back Left');
  }));

  const bySlot = buildSteamBindingBySlotKey(bindings, steamBindingSlots);
  modelDurations.push(time(() => {
    const chips = createMappingChipModels({
      bindingBySlotKey: bySlot,
      controllerFamily: i % 2 === 0 ? 'DualSense Edge' : 'DualSense',
      selectedBindingKey: steamBindingKey(bindings[i % bindings.length]),
      activeSlotKey: i % 4 === 0 ? steamBindingSlots[i % steamBindingSlots.length].key : ''
    });
    assert.ok(chips.length >= 22);
    assert.ok(chips.every((chip) => chip.displayLabel.length > 0));
  }));

  parseDurations.push(time(() => {
    const parsed = parseSteamBindingTriple(`key_press M, icon_${i}, Label ${i}, extra`);
    assert.equal(parsed.command, 'key_press');
    assert.equal(parsed.param, 'M');
    assert.equal(parsed.label, `Label ${i}, extra`);
    assert.equal(steamBindingTargetPart('mouse_button left, , Primary'), 'mouse_button left, , ');
  }));
}

const labelBinding = {
  input: 'Cross',
  inputId: 'button_a',
  binding: 'A Button',
  rawBinding: 'key_press SPACE, , Jump',
  kind: 'Key'
};
assert.equal(chipDisplayLabel(labelBinding), 'Jump');
assert.equal(chipDisplayLabel(null), 'Unassigned');

const fh6ParityBindings = [
  {
    input: 'Create',
    inputId: 'button_menu',
    binding: 'Select',
    rawBinding: 'xinput_button select, , ',
    kind: 'XInput',
    source: 'Switches',
    sourceMode: 'Switches',
    activator: 'Full Press',
    groupId: '7'
  },
  {
    input: 'Options',
    inputId: 'button_escape',
    binding: 'Start',
    rawBinding: 'xinput_button start, , ',
    kind: 'XInput',
    source: 'Switches',
    sourceMode: 'Switches',
    activator: 'Full Press',
    groupId: '7'
  },
  {
    input: 'Swipe Up',
    inputId: 'dpad_north',
    binding: '= Key',
    rawBinding: 'key_press EQUALS, , ',
    kind: 'Key',
    source: 'Center Trackpad',
    sourceMode: 'Directional Swipe',
    activator: 'Full Press',
    groupId: '14'
  },
  {
    input: 'Swipe Down',
    inputId: 'dpad_south',
    binding: '- Key',
    rawBinding: 'key_press DASH, , ',
    kind: 'Key',
    source: 'Center Trackpad',
    sourceMode: 'Directional Swipe',
    activator: 'Full Press',
    groupId: '14'
  },
  {
    input: 'D-Pad Up',
    inputId: 'dpad_north',
    binding: 'DPad Up',
    rawBinding: 'xinput_button DPAD_UP, , ',
    kind: 'XInput',
    source: 'Directional Pad',
    sourceMode: 'Directional Pad',
    activator: 'Full Press',
    groupId: '9'
  }
];
const fh6BySlot = buildSteamBindingBySlotKey(fh6ParityBindings, steamBindingSlots);
assert.equal(fh6BySlot.get('create')?.binding, 'Select');
assert.equal(fh6BySlot.get('options')?.binding, 'Start');
assert.equal(fh6BySlot.get('centerSwipeUp')?.binding, '= Key');
assert.equal(fh6BySlot.get('centerSwipeDown')?.binding, '- Key');
assert.equal(fh6BySlot.get('dpadUp')?.binding, 'DPad Up');

const mirrorGroups = createSteamMirrorGroups({
  bindingBySlotKey: fh6BySlot,
  controllerFamily: 'DualSense Edge',
  selectedBindingKey: steamBindingKey(fh6ParityBindings[0]),
  activeSlotKey: ''
});
assert.ok(mirrorGroups.some((group) => group.key === 'center-trackpad' && group.rows.length === 2));
assert.ok(mirrorGroups.some((group) => group.key === 'right-rail' && group.rows.some((row) => row.displayLabel === 'Start')));

const summary = {
  samples,
  lookupP95Ms: p95(lookupDurations),
  chipModelP95Ms: p95(modelDurations),
  parseP95Ms: p95(parseDurations)
};

const budgets = {
  lookupP95Ms: 8,
  chipModelP95Ms: 8,
  parseP95Ms: 2
};

for (const [metric, budget] of Object.entries(budgets)) {
  assert.ok(
    summary[metric] <= budget,
    `${metric} ${summary[metric].toFixed(3)}ms exceeded ${budget}ms p95 budget`
  );
}

console.log(
  `button mapping p95: lookup=${summary.lookupP95Ms.toFixed(3)}ms, chips=${summary.chipModelP95Ms.toFixed(3)}ms, parse=${summary.parseP95Ms.toFixed(3)}ms (${samples} samples)`
);
