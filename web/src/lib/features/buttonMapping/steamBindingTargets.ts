import type { ControllerStatus, SteamInputBinding } from '../../types';
import { buildDefaultSteamBindingBySlotKey, type SteamMirrorGroup } from './buttonMapping';
import {
  prepareBindingTargetGroups,
  type PreparedSteamBindingTargetGroup,
  type RawBindingTargetGroup
} from './buttonMappingState';

export const EMPTY_STEAM_INPUT_BINDINGS: SteamInputBinding[] = [];
export const EMPTY_STEAM_BINDING_MAP = new Map<string, SteamInputBinding>();
export const EMPTY_STEAM_MIRROR_GROUPS: SteamMirrorGroup[] = [];

let defaultSteamBindingCacheReady = false;
let defaultSteamBindingCacheFamily: ControllerStatus['family'] | undefined | null;
let defaultSteamBindingCache = EMPTY_STEAM_BINDING_MAP;

export function defaultSteamBindingsForFamily(family: ControllerStatus['family'] | undefined | null) {
  if (!defaultSteamBindingCacheReady || defaultSteamBindingCacheFamily !== family) {
    defaultSteamBindingCacheReady = true;
    defaultSteamBindingCacheFamily = family;
    defaultSteamBindingCache = buildDefaultSteamBindingBySlotKey(family);
  }
  return defaultSteamBindingCache;
}

const keyboardLetterOptions = Array.from({ length: 26 }, (_, i) => {
  const letter = String.fromCharCode(65 + i);
  return { label: `${letter} Key`, raw: `key_press ${letter}, , ` };
});

const keyboardNumberOptions = Array.from({ length: 10 }, (_, i) => ({
  label: `${i} Key`,
  raw: `key_press ${i}, , `
}));

const keyboardFunctionOptions = Array.from({ length: 12 }, (_, i) => ({
  label: `F${i + 1}`,
  raw: `key_press F${i + 1}, , `
}));

const keyboardNumpadOptions = [
  ...Array.from({ length: 10 }, (_, i) => ({
    label: `Numpad ${i}`,
    raw: `key_press KP_${i}, , `
  })),
  { label: 'Numpad /', raw: 'key_press KP_DIVIDE, , ' },
  { label: 'Numpad *', raw: 'key_press KP_MULTIPLY, , ' },
  { label: 'Numpad -', raw: 'key_press KP_MINUS, , ' },
  { label: 'Numpad +', raw: 'key_press KP_PLUS, , ' },
  { label: 'Numpad .', raw: 'key_press KP_PERIOD, , ' },
  { label: 'Numpad Enter', raw: 'key_press KP_ENTER, , ' }
];

// Steam Input target catalog. The raw VDF form for every binding is
// `<command> <param>, <icon>, <label>`. The third field is the free-form
// label that Steam shows in its UI; leave it blank here for user-authored text.
export const steamBindingTargetGroups: RawBindingTargetGroup[] = [
  {
    label: 'Gamepad — Face / D-Pad',
    options: [
      { label: 'A / Cross', raw: 'xinput_button a, , ' },
      { label: 'B / Circle', raw: 'xinput_button b, , ' },
      { label: 'X / Square', raw: 'xinput_button x, , ' },
      { label: 'Y / Triangle', raw: 'xinput_button y, , ' },
      { label: 'D-Pad Up', raw: 'xinput_button dpad_up, , ' },
      { label: 'D-Pad Down', raw: 'xinput_button dpad_down, , ' },
      { label: 'D-Pad Left', raw: 'xinput_button dpad_left, , ' },
      { label: 'D-Pad Right', raw: 'xinput_button dpad_right, , ' }
    ]
  },
  {
    label: 'Gamepad — Shoulders / Triggers / Sticks',
    options: [
      { label: 'Left Bumper (LB)', raw: 'xinput_button shoulder_left, , ' },
      { label: 'Right Bumper (RB)', raw: 'xinput_button shoulder_right, , ' },
      { label: 'Left Trigger (LT)', raw: 'xinput_button trigger_left, , ' },
      { label: 'Right Trigger (RT)', raw: 'xinput_button trigger_right, , ' },
      { label: 'Left Stick Click (LS)', raw: 'xinput_button joystick_left, , ' },
      { label: 'Right Stick Click (RS)', raw: 'xinput_button joystick_right, , ' }
    ]
  },
  {
    label: 'Gamepad — System',
    options: [
      { label: 'Start / Options', raw: 'xinput_button start, , ' },
      { label: 'Select / Create', raw: 'xinput_button select, , ' },
      { label: 'Guide / PS Button', raw: 'xinput_button guide, , ' }
    ]
  },
  {
    label: 'Keyboard — Letters',
    options: keyboardLetterOptions
  },
  {
    label: 'Keyboard — Numbers',
    options: keyboardNumberOptions
  },
  {
    label: 'Keyboard — Function Keys',
    options: keyboardFunctionOptions
  },
  {
    label: 'Keyboard — Modifiers',
    options: [
      { label: 'Left Shift', raw: 'key_press LSHIFT, , ' },
      { label: 'Right Shift', raw: 'key_press RSHIFT, , ' },
      { label: 'Left Ctrl', raw: 'key_press LCONTROL, , ' },
      { label: 'Right Ctrl', raw: 'key_press RCONTROL, , ' },
      { label: 'Left Alt', raw: 'key_press LALT, , ' },
      { label: 'Right Alt', raw: 'key_press RALT, , ' },
      { label: 'Left Win', raw: 'key_press LWIN, , ' },
      { label: 'Right Win', raw: 'key_press RWIN, , ' }
    ]
  },
  {
    label: 'Keyboard — Navigation',
    options: [
      { label: 'Tab', raw: 'key_press TAB, , ' },
      { label: 'Space', raw: 'key_press SPACE, , ' },
      { label: 'Enter / Return', raw: 'key_press RETURN, , ' },
      { label: 'Esc', raw: 'key_press ESCAPE, , ' },
      { label: 'Backspace', raw: 'key_press BACKSPACE, , ' },
      { label: 'Delete', raw: 'key_press DELETE, , ' },
      { label: 'Insert', raw: 'key_press INSERT, , ' },
      { label: 'Home', raw: 'key_press HOME, , ' },
      { label: 'End', raw: 'key_press END, , ' },
      { label: 'Page Up', raw: 'key_press PAGE_UP, , ' },
      { label: 'Page Down', raw: 'key_press PAGE_DOWN, , ' },
      { label: 'Caps Lock', raw: 'key_press CAPSLOCK, , ' },
      { label: 'Print Screen', raw: 'key_press PRINT_SCREEN, , ' },
      { label: 'Scroll Lock', raw: 'key_press SCROLL_LOCK, , ' },
      { label: 'Pause / Break', raw: 'key_press PAUSE, , ' }
    ]
  },
  {
    label: 'Keyboard — Arrows',
    options: [
      { label: 'Up Arrow', raw: 'key_press UP_ARROW, , ' },
      { label: 'Down Arrow', raw: 'key_press DOWN_ARROW, , ' },
      { label: 'Left Arrow', raw: 'key_press LEFT_ARROW, , ' },
      { label: 'Right Arrow', raw: 'key_press RIGHT_ARROW, , ' }
    ]
  },
  {
    label: 'Keyboard — Punctuation',
    options: [
      { label: ', (Comma)', raw: 'key_press COMMA, , ' },
      { label: '. (Period)', raw: 'key_press PERIOD, , ' },
      { label: '; (Semicolon)', raw: 'key_press SEMICOLON, , ' },
      { label: "' (Apostrophe)", raw: 'key_press SINGLE_QUOTE, , ' },
      { label: '/ (Slash)', raw: 'key_press FORWARD_SLASH, , ' },
      { label: '\\ (Backslash)', raw: 'key_press BACK_SLASH, , ' },
      { label: '[ Left Bracket', raw: 'key_press LEFT_BRACKET, , ' },
      { label: '] Right Bracket', raw: 'key_press RIGHT_BRACKET, , ' },
      { label: '- (Minus)', raw: 'key_press DASH, , ' },
      { label: '= (Equals)', raw: 'key_press EQUALS, , ' },
      { label: '` (Backquote)', raw: 'key_press BACK_TICK, , ' }
    ]
  },
  {
    label: 'Keyboard — Numpad',
    options: keyboardNumpadOptions
  },
  {
    label: 'Mouse — Buttons',
    options: [
      { label: 'Left Click', raw: 'mouse_button left, , ' },
      { label: 'Right Click', raw: 'mouse_button right, , ' },
      { label: 'Middle Click', raw: 'mouse_button middle, , ' },
      { label: 'Mouse Button 4 (X1)', raw: 'mouse_button x1, , ' },
      { label: 'Mouse Button 5 (X2)', raw: 'mouse_button x2, , ' }
    ]
  },
  {
    label: 'Mouse — Wheel',
    options: [
      { label: 'Wheel Up', raw: 'mouse_wheel up, , ' },
      { label: 'Wheel Down', raw: 'mouse_wheel down, , ' }
    ]
  }
];

export const preparedSteamBindingTargetGroups: PreparedSteamBindingTargetGroup[] =
  prepareBindingTargetGroups(steamBindingTargetGroups);
