import type { ControllerStatus, SteamInputBinding } from '../../types';

export type SteamBindingSlot = {
  key: string;
  label: string;
  group: string;
  source?: string;
  inputIds: string[];
};

export type SteamBindingTriple = {
  command: string;
  param: string;
  icon: string;
  label: string;
};

export type SteamSlotGlyph = {
  icon?: string;
  focus?: string;
  region?: 'face' | 'dpad' | 'shoulder' | 'trigger' | 'stick' | 'touch' | 'system' | 'edge' | 'motion';
};

export type MappingChipPos = {
  key: string;
  side: 'left' | 'right' | 'top' | 'bottom';
  chipX: number;
  chipY: number;
  anchorX: number;
  anchorY: number;
};

export type MappingChipModel = MappingChipPos & {
  slot: SteamBindingSlot;
  binding: SteamInputBinding | null;
  displayLabel: string;
  iconUrl: string | null;
  selected: boolean;
};

export type SteamMirrorPlacement = 'left' | 'right' | 'center' | 'bottom';

export type SteamMirrorRow = {
  key: string;
  slot: SteamBindingSlot;
  binding: SteamInputBinding | null;
  displayLabel: string;
  iconUrl: string | null;
  selected: boolean;
};

export type SteamMirrorGroup = {
  key: string;
  label: string;
  placement: SteamMirrorPlacement;
  rows: SteamMirrorRow[];
  staticRows?: string[];
};

export const steamBindingSlots: SteamBindingSlot[] = [
  { key: 'cross', label: 'Cross', group: 'Face Buttons', source: 'Face Buttons', inputIds: ['button_a'] },
  { key: 'circle', label: 'Circle', group: 'Face Buttons', source: 'Face Buttons', inputIds: ['button_b'] },
  { key: 'square', label: 'Square', group: 'Face Buttons', source: 'Face Buttons', inputIds: ['button_x'] },
  { key: 'triangle', label: 'Triangle', group: 'Face Buttons', source: 'Face Buttons', inputIds: ['button_y'] },
  { key: 'dpadUp', label: 'D-Pad Up', group: 'Directional Pad', source: 'Directional Pad', inputIds: ['dpad_north'] },
  { key: 'dpadDown', label: 'D-Pad Down', group: 'Directional Pad', source: 'Directional Pad', inputIds: ['dpad_south'] },
  { key: 'dpadLeft', label: 'D-Pad Left', group: 'Directional Pad', source: 'Directional Pad', inputIds: ['dpad_west'] },
  { key: 'dpadRight', label: 'D-Pad Right', group: 'Directional Pad', source: 'Directional Pad', inputIds: ['dpad_east'] },
  { key: 'l1', label: 'L1', group: 'Shoulders', source: 'Switches', inputIds: ['left_bumper', 'button_should_left'] },
  { key: 'r1', label: 'R1', group: 'Shoulders', source: 'Switches', inputIds: ['right_bumper', 'button_should_right'] },
  { key: 'l2', label: 'L2', group: 'Triggers', source: 'Left Trigger', inputIds: ['click:left_trigger', 'left_trigger:click'] },
  { key: 'r2', label: 'R2', group: 'Triggers', source: 'Right Trigger', inputIds: ['click:right_trigger', 'right_trigger:click'] },
  { key: 'create', label: 'Create', group: 'System', source: 'Switches', inputIds: ['button_menu'] },
  { key: 'options', label: 'Options', group: 'System', source: 'Switches', inputIds: ['button_escape'] },
  { key: 'l3', label: 'L3', group: 'Sticks', source: 'Left Joystick', inputIds: ['click:left_joystick', 'left_joystick:click', 'click:joystick', 'joystick:click'] },
  { key: 'r3', label: 'R3', group: 'Sticks', source: 'Right Joystick', inputIds: ['click:right_joystick', 'right_joystick:click'] },
  { key: 'touchPressLeft', label: 'Touchpad Left Press', group: 'Trackpad', source: 'Left Trackpad', inputIds: ['click:left_trackpad', 'left_trackpad:click'] },
  { key: 'touchPressRight', label: 'Touchpad Right Press', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['click:right_trackpad', 'right_trackpad:click'] },
  { key: 'swipeUp', label: 'Touchpad Swipe Up', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['dpad_up', 'dpad_north:right_trackpad', 'dpad_up:right_trackpad'] },
  { key: 'swipeDown', label: 'Touchpad Swipe Down', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['dpad_down', 'dpad_south:right_trackpad', 'dpad_down:right_trackpad'] },
  { key: 'swipeLeft', label: 'Touchpad Swipe Left', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['dpad_left', 'dpad_west:right_trackpad', 'dpad_left:right_trackpad'] },
  { key: 'swipeRight', label: 'Touchpad Swipe Right', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['dpad_right', 'dpad_east:right_trackpad', 'dpad_right:right_trackpad'] },
  { key: 'centerSwipeUp', label: 'Center Swipe Up', group: 'Center Trackpad', source: 'Center Trackpad', inputIds: ['dpad_north:center_trackpad', 'center_trackpad:dpad_north', 'dpad_up:center_trackpad', 'center_trackpad:dpad_up'] },
  { key: 'centerSwipeDown', label: 'Center Swipe Down', group: 'Center Trackpad', source: 'Center Trackpad', inputIds: ['dpad_south:center_trackpad', 'center_trackpad:dpad_south', 'dpad_down:center_trackpad', 'center_trackpad:dpad_down'] },
  { key: 'centerSwipeLeft', label: 'Center Swipe Left', group: 'Center Trackpad', source: 'Center Trackpad', inputIds: ['dpad_west:center_trackpad', 'center_trackpad:dpad_west', 'dpad_left:center_trackpad', 'center_trackpad:dpad_left'] },
  { key: 'centerSwipeRight', label: 'Center Swipe Right', group: 'Center Trackpad', source: 'Center Trackpad', inputIds: ['dpad_east:center_trackpad', 'center_trackpad:dpad_east', 'dpad_right:center_trackpad', 'center_trackpad:dpad_right'] },
  { key: 'gyro', label: 'Gyro', group: 'Motion', source: 'Gyro', inputIds: ['gyro', 'click:gyro'] },
  { key: 'edgeBackLeft', label: 'Back Left', group: 'DualSense Edge', source: 'Switches', inputIds: ['button_back_left'] },
  { key: 'edgeBackRight', label: 'Back Right', group: 'DualSense Edge', source: 'Switches', inputIds: ['button_back_right'] },
  { key: 'edgeFnLeft', label: 'Fn Left', group: 'DualSense Edge', source: 'Switches', inputIds: ['button_back_left_upper'] },
  { key: 'edgeFnRight', label: 'Fn Right', group: 'DualSense Edge', source: 'Switches', inputIds: ['button_back_right_upper'] }
];

export const steamSlotGlyphs: Record<string, SteamSlotGlyph> = {
  cross: { icon: 'face_cross', focus: 'cross', region: 'face' },
  circle: { icon: 'face_circle', focus: 'circle', region: 'face' },
  square: { icon: 'face_square', focus: 'square', region: 'face' },
  triangle: { icon: 'face_triangle', focus: 'triangle', region: 'face' },
  dpadUp: { icon: 'arrow_up', focus: 'up', region: 'dpad' },
  dpadDown: { icon: 'arrow_down', focus: 'down', region: 'dpad' },
  dpadLeft: { icon: 'arrow_left', focus: 'left', region: 'dpad' },
  dpadRight: { icon: 'arrow_right', focus: 'right', region: 'dpad' },
  l1: { icon: 'l1', focus: 'L1', region: 'shoulder' },
  r1: { icon: 'r1', focus: 'R1', region: 'shoulder' },
  l2: { icon: 'l2', focus: 'L2', region: 'trigger' },
  r2: { icon: 'r2', focus: 'R2', region: 'trigger' },
  l3: { icon: 'l3', focus: 'Lstick', region: 'stick' },
  r3: { icon: 'r3', focus: 'Rstick', region: 'stick' },
  create: { icon: 'create', focus: 'create', region: 'system' },
  options: { icon: 'options', focus: 'options', region: 'system' },
  touchPressLeft: { icon: 'tap', focus: 'touchpad', region: 'touch' },
  touchPressRight: { icon: 'tap', focus: 'touchpad', region: 'touch' },
  swipeUp: { icon: 'arrow_up', focus: 'touchpad', region: 'touch' },
  swipeDown: { icon: 'arrow_down', focus: 'touchpad', region: 'touch' },
  swipeLeft: { icon: 'arrow_left', focus: 'touchpad', region: 'touch' },
  swipeRight: { icon: 'arrow_right', focus: 'touchpad', region: 'touch' },
  centerSwipeUp: { icon: 'swipe', focus: 'touchpad', region: 'touch' },
  centerSwipeDown: { icon: 'swipe', focus: 'touchpad', region: 'touch' },
  centerSwipeLeft: { icon: 'swipe', focus: 'touchpad', region: 'touch' },
  centerSwipeRight: { icon: 'swipe', focus: 'touchpad', region: 'touch' },
  gyro: { icon: 'analog_stick_l', region: 'motion' },
  edgeBackLeft: { icon: 'fn', focus: 'rear_L', region: 'edge' },
  edgeBackRight: { icon: 'fn', focus: 'rear_R', region: 'edge' },
  edgeFnLeft: { icon: 'fn', region: 'edge' },
  edgeFnRight: { icon: 'fn', region: 'edge' }
};

export const mappingChipLayout: MappingChipPos[] = [
  { key: 'l2', side: 'left', chipX: 16, chipY: 18, anchorX: 35.5, anchorY: 18 },
  { key: 'l1', side: 'left', chipX: 16, chipY: 28, anchorX: 35.5, anchorY: 28 },
  { key: 'create', side: 'left', chipX: 16, chipY: 36, anchorX: 42.0, anchorY: 35 },
  { key: 'dpadUp', side: 'left', chipX: 16, chipY: 43, anchorX: 38.5, anchorY: 41 },
  { key: 'dpadLeft', side: 'left', chipX: 16, chipY: 50, anchorX: 35.0, anchorY: 47 },
  { key: 'dpadRight', side: 'left', chipX: 16, chipY: 57, anchorX: 42.0, anchorY: 47 },
  { key: 'dpadDown', side: 'left', chipX: 16, chipY: 64, anchorX: 38.5, anchorY: 53 },
  { key: 'l3', side: 'left', chipX: 16, chipY: 72, anchorX: 44.5, anchorY: 60 },
  { key: 'r2', side: 'right', chipX: 84, chipY: 18, anchorX: 64.5, anchorY: 18 },
  { key: 'r1', side: 'right', chipX: 84, chipY: 28, anchorX: 64.5, anchorY: 28 },
  { key: 'options', side: 'right', chipX: 84, chipY: 36, anchorX: 58.0, anchorY: 35 },
  { key: 'triangle', side: 'right', chipX: 84, chipY: 43, anchorX: 61.5, anchorY: 41 },
  { key: 'circle', side: 'right', chipX: 84, chipY: 50, anchorX: 65.0, anchorY: 47 },
  { key: 'square', side: 'right', chipX: 84, chipY: 57, anchorX: 58.0, anchorY: 47 },
  { key: 'cross', side: 'right', chipX: 84, chipY: 64, anchorX: 61.5, anchorY: 53 },
  { key: 'r3', side: 'right', chipX: 84, chipY: 72, anchorX: 55.5, anchorY: 60 },
  { key: 'swipeLeft', side: 'top', chipX: 30, chipY: 8, anchorX: 44.0, anchorY: 36 },
  { key: 'touchPressLeft', side: 'top', chipX: 38, chipY: 8, anchorX: 47.0, anchorY: 36 },
  { key: 'swipeUp', side: 'top', chipX: 46, chipY: 8, anchorX: 50.0, anchorY: 30 },
  { key: 'swipeDown', side: 'top', chipX: 54, chipY: 8, anchorX: 50.0, anchorY: 42 },
  { key: 'touchPressRight', side: 'top', chipX: 62, chipY: 8, anchorX: 53.0, anchorY: 36 },
  { key: 'swipeRight', side: 'top', chipX: 70, chipY: 8, anchorX: 56.0, anchorY: 36 },
  { key: 'centerSwipeUp', side: 'top', chipX: 46, chipY: 15, anchorX: 50.0, anchorY: 30 },
  { key: 'centerSwipeDown', side: 'top', chipX: 54, chipY: 15, anchorX: 50.0, anchorY: 42 },
  { key: 'centerSwipeLeft', side: 'top', chipX: 38, chipY: 15, anchorX: 44.0, anchorY: 36 },
  { key: 'centerSwipeRight', side: 'top', chipX: 62, chipY: 15, anchorX: 56.0, anchorY: 36 },
  { key: 'edgeBackLeft', side: 'bottom', chipX: 30, chipY: 92, anchorX: 45, anchorY: 53 },
  { key: 'edgeFnLeft', side: 'bottom', chipX: 42, chipY: 92, anchorX: 44, anchorY: 67 },
  { key: 'edgeFnRight', side: 'bottom', chipX: 58, chipY: 92, anchorX: 56, anchorY: 67 },
  { key: 'edgeBackRight', side: 'bottom', chipX: 70, chipY: 92, anchorX: 55, anchorY: 53 }
];

const steamMirrorDefinitions: Array<{
  key: string;
  label: string;
  placement: SteamMirrorPlacement;
  slotKeys: string[];
  optionalWhenUnbound?: boolean;
  staticRows?: string[];
}> = [
  {
    key: 'left-rail',
    label: 'Left Controls',
    placement: 'left',
    slotKeys: ['l1', 'l2', 'edgeFnLeft', 'edgeBackLeft', 'create']
  },
  {
    key: 'left-trackpad',
    label: 'Left Trackpad',
    placement: 'left',
    slotKeys: ['touchPressLeft']
  },
  {
    key: 'center-trackpad',
    label: 'Center Trackpad',
    placement: 'center',
    slotKeys: ['centerSwipeUp', 'centerSwipeDown', 'centerSwipeLeft', 'centerSwipeRight'],
    optionalWhenUnbound: true,
    staticRows: ['Directional Swipe']
  },
  {
    key: 'right-rail',
    label: 'Right Controls',
    placement: 'right',
    slotKeys: ['r1', 'r2', 'edgeFnRight', 'edgeBackRight', 'options']
  },
  {
    key: 'right-trackpad',
    label: 'Right Trackpad',
    placement: 'right',
    slotKeys: ['touchPressRight']
  },
  {
    key: 'dpad',
    label: 'Directional Pad',
    placement: 'bottom',
    slotKeys: ['dpadUp', 'dpadDown', 'dpadLeft', 'dpadRight']
  },
  {
    key: 'left-joystick',
    label: 'Left Joystick',
    placement: 'bottom',
    slotKeys: ['l3'],
    staticRows: ['Joystick']
  },
  {
    key: 'gyro',
    label: 'Gyro',
    placement: 'bottom',
    slotKeys: ['gyro'],
    optionalWhenUnbound: true
  },
  {
    key: 'right-joystick',
    label: 'Right Joystick',
    placement: 'bottom',
    slotKeys: ['r3'],
    staticRows: ['Joystick']
  },
  {
    key: 'face-buttons',
    label: 'Face Buttons',
    placement: 'bottom',
    slotKeys: ['cross', 'circle', 'square', 'triangle']
  }
];

export const parseSteamBindingTriple = (raw: string | null | undefined): SteamBindingTriple => {
  const value = (raw ?? '').trim();
  if (!value) return { command: '', param: '', icon: '', label: '' };
  const firstComma = value.indexOf(',');
  const head = firstComma === -1 ? value : value.slice(0, firstComma).trim();
  const tail = firstComma === -1 ? '' : value.slice(firstComma + 1);
  const tailParts = tail.split(',');
  const icon = (tailParts[0] ?? '').trim();
  const label = tailParts.slice(1).join(',').trim();
  const spaceIdx = head.indexOf(' ');
  const command = spaceIdx === -1 ? head : head.slice(0, spaceIdx);
  const param = spaceIdx === -1 ? '' : head.slice(spaceIdx + 1).trim();
  return { command, param, icon, label };
};

export const assembleSteamBindingRaw = (triple: SteamBindingTriple): string => {
  const head = triple.param ? `${triple.command} ${triple.param}` : triple.command;
  return `${head}, ${triple.icon ?? ''}, ${triple.label ?? ''}`;
};

export const steamBindingTargetPart = (raw: string): string => {
  const { command, param } = parseSteamBindingTriple(raw);
  return assembleSteamBindingRaw({ command, param, icon: '', label: '' });
};

export const steamBindingUserLabel = (binding: { rawBinding?: string | null } | null | undefined) =>
  parseSteamBindingTriple(binding?.rawBinding ?? '').label;

export const chipDisplayLabel = (binding: SteamInputBinding | null | undefined) => {
  if (!binding) return 'Unassigned';
  const userLabel = steamBindingUserLabel(binding);
  return userLabel || binding.binding;
};

export const steamSlotIconUrl = (key: string): string | null => {
  const icon = steamSlotGlyphs[key]?.icon;
  return icon ? `/dualsense/icons/iconid_controller_key_${icon}.png` : null;
};

export const steamBindingKey = (binding: SteamInputBinding) =>
  [
    binding.groupId ?? '',
    binding.source ?? '',
    binding.sourceMode ?? '',
    binding.inputId ?? '',
    binding.activator ?? ''
  ].join('|');

export const steamBindingSignature = (binding: SteamInputBinding) => {
  const inputId = binding.inputId ?? '';
  const source = (binding.source ?? '').toLowerCase().replaceAll(' ', '_');
  return source ? [`${inputId}:${source}`, `${source}:${inputId}`, inputId] : [inputId];
};

export function buildSteamBindingBySlotKey(
  bindings: SteamInputBinding[],
  slots: SteamBindingSlot[] = steamBindingSlots
): Map<string, SteamInputBinding> {
  const result = new Map<string, SteamInputBinding>();
  const pendingSlots = slots.slice();

  for (const binding of bindings) {
    const signatures = new Set(steamBindingSignature(binding));
    for (let index = pendingSlots.length - 1; index >= 0; index -= 1) {
      const slot = pendingSlots[index];
      if (slot.inputIds.some((inputId) => signatures.has(inputId))) {
        result.set(slot.key, binding);
        pendingSlots.splice(index, 1);
        break;
      }
    }
    if (!pendingSlots.length) break;
  }

  return result;
}

export function createMappingChipModels(options: {
  bindingBySlotKey: Map<string, SteamInputBinding>;
  controllerFamily?: ControllerStatus['family'] | null;
  selectedBindingKey: string;
  activeSlotKey: string;
  layout?: MappingChipPos[];
  slots?: SteamBindingSlot[];
}): MappingChipModel[] {
  const slots = options.slots ?? steamBindingSlots;
  const slotByKey = new Map(slots.map((slot) => [slot.key, slot]));

  return (options.layout ?? mappingChipLayout)
    .map((chip) => {
      const slot = slotByKey.get(chip.key);
      if (!slot) return null;

      const binding = options.bindingBySlotKey.get(slot.key) ?? null;
      if (slot.group === 'DualSense Edge' && options.controllerFamily !== 'DualSense Edge' && !binding) {
        return null;
      }

      return {
        ...chip,
        slot,
        binding,
        displayLabel: chipDisplayLabel(binding),
        iconUrl: steamSlotIconUrl(chip.key),
        selected:
          options.activeSlotKey === slot.key ||
          Boolean(binding && steamBindingKey(binding) === options.selectedBindingKey)
      };
    })
    .filter((value): value is MappingChipModel => value !== null);
}

export function createSteamMirrorGroups(options: {
  bindingBySlotKey: Map<string, SteamInputBinding>;
  controllerFamily?: ControllerStatus['family'] | null;
  selectedBindingKey: string;
  activeSlotKey: string;
  slots?: SteamBindingSlot[];
}): SteamMirrorGroup[] {
  const slots = options.slots ?? steamBindingSlots;
  const slotByKey = new Map(slots.map((slot) => [slot.key, slot]));
  const groups: SteamMirrorGroup[] = [];

  for (const group of steamMirrorDefinitions) {
    const rows = group.slotKeys
      .map((slotKey) => {
        const slot = slotByKey.get(slotKey);
        if (!slot) return null;
        const binding = options.bindingBySlotKey.get(slot.key) ?? null;
        if (slot.group === 'DualSense Edge' && options.controllerFamily !== 'DualSense Edge' && !binding) {
          return null;
        }
        if (!binding && group.optionalWhenUnbound) {
          return null;
        }
        return {
          key: slot.key,
          slot,
          binding,
          displayLabel: chipDisplayLabel(binding),
          iconUrl: steamSlotIconUrl(slot.key),
          selected:
            options.activeSlotKey === slot.key ||
            Boolean(binding && steamBindingKey(binding) === options.selectedBindingKey)
        };
      })
      .filter((value): value is SteamMirrorRow => value !== null);

    if (!rows.length && !group.staticRows?.length) continue;
    groups.push({
      key: group.key,
      label: group.label,
      placement: group.placement,
      rows,
      ...(group.staticRows ? { staticRows: group.staticRows } : {})
    });
  }

  return groups;
}
