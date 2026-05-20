<script lang="ts">
  import { Cable, ChevronDown, RefreshCw, RotateCcw, Save, Search } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import Tooltip from './components/Tooltip.svelte';
  import {
    activateProfile,
    clearProfileOverride,
    connectAppSnapshotSocket,
    createProfile,
    deleteProfile,
    exportProfile,
    getAppSnapshot,
    getControllerInput,
    getControllerConfig,
    importProfile,
    renameProfile,
    runEffectTest,
    saveAppSettings,
    saveControllerConfig,
    saveProfileConfig,
    setProfileOverride,
    writeSteamInputBinding
  } from './lib/api';
  import type {
    AppSnapshot,
    ControllerConfiguration,
    ControllerStatus,
    CurrentEffectState,
    EffectTestRequest,
    ExportedProfile,
    ForzaEffectConfiguration,
    ForzaEffectRoute,
    GameDetection,
    ProfileAssignmentConfiguration,
    ProfileSummary,
    SteamInputBinding,
    SteamInputLayout,
    SupportedGame
  } from './lib/types';

  type ForzaEffectMeta = {
    id: string;
    label: string;
    signal: string;
    group: 'Trigger' | 'Body' | 'Cue' | 'Light';
    defaultIntensity: number;
    defaultRoute: ForzaEffectRoute;
    help: string;
  };
  type ColorPickerTarget = 'lightbar' | 'rpm';
  type AppView = 'haptics' | 'buttonMapping';
  type ToastTone = 'success' | 'info' | 'error';
  type ToastMessage = {
    id: number;
    tone: ToastTone;
    message: string;
  };
  type EditableControllerConfig = Omit<ControllerConfiguration, 'controllerId' | 'model'>;
  type SteamBindingSlot = {
    key: string;
    label: string;
    group: string;
    source?: string;
    inputIds: string[];
  };
  type SteamBindingTargetGroup = {
    label: string;
    options: Array<{ label: string; raw: string }>;
  };

  const appViews: Array<{ id: AppView; label: string; hash: string }> = [
    { id: 'haptics', label: 'Adaptive Triggers & Haptics', hash: '#/adaptive-triggers-haptics' },
    { id: 'buttonMapping', label: 'Button Mapping', hash: '#/button-mapping' }
  ];

  const steamBindingSlots: SteamBindingSlot[] = [
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
    { key: 'create', label: 'Create', group: 'System', source: 'Switches', inputIds: ['button_escape'] },
    { key: 'options', label: 'Options', group: 'System', source: 'Switches', inputIds: ['button_menu'] },
    { key: 'l3', label: 'L3', group: 'Sticks', source: 'Left Joystick', inputIds: ['click:left_joystick', 'left_joystick:click', 'click:joystick', 'joystick:click'] },
    { key: 'r3', label: 'R3', group: 'Sticks', source: 'Right Joystick', inputIds: ['click:right_joystick', 'right_joystick:click'] },
    { key: 'touchPressLeft', label: 'Touchpad Left Press', group: 'Trackpad', source: 'Left Trackpad', inputIds: ['click:left_trackpad', 'left_trackpad:click'] },
    { key: 'touchPressRight', label: 'Touchpad Right Press', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['click:right_trackpad', 'right_trackpad:click'] },
    { key: 'swipeUp', label: 'Touchpad Swipe Up', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['dpad_up', 'dpad_north:right_trackpad', 'dpad_up:right_trackpad'] },
    { key: 'swipeDown', label: 'Touchpad Swipe Down', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['dpad_down', 'dpad_south:right_trackpad', 'dpad_down:right_trackpad'] },
    { key: 'swipeLeft', label: 'Touchpad Swipe Left', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['dpad_left', 'dpad_west:right_trackpad', 'dpad_left:right_trackpad'] },
    { key: 'swipeRight', label: 'Touchpad Swipe Right', group: 'Trackpad', source: 'Right Trackpad', inputIds: ['dpad_right', 'dpad_east:right_trackpad', 'dpad_right:right_trackpad'] },
    { key: 'gyro', label: 'Gyro', group: 'Motion', source: 'Gyro', inputIds: ['gyro', 'click:gyro'] },
    { key: 'edgeBackLeft', label: 'Back Left', group: 'DualSense Edge', source: 'Switches', inputIds: ['button_back_left'] },
    { key: 'edgeBackRight', label: 'Back Right', group: 'DualSense Edge', source: 'Switches', inputIds: ['button_back_right'] },
    { key: 'edgeFnLeft', label: 'Fn Left', group: 'DualSense Edge', source: 'Switches', inputIds: ['button_back_left_upper'] },
    { key: 'edgeFnRight', label: 'Fn Right', group: 'DualSense Edge', source: 'Switches', inputIds: ['button_back_right_upper'] }
  ];

  // Steam Input target catalog. The raw VDF form for every binding is
  // `<command> <param>, <icon>, <label>` — the third field is a free-form
  // label that Steam shows in its UI (e.g. "Next radio station") and we
  // leave blank here so the user can author one if they want. Anything
  // not in this catalog can still be set verbatim through the Raw VDF
  // field below the dropdown.
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

  const steamBindingTargetGroups: SteamBindingTargetGroup[] = [
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
    },
    {
      label: 'Steam Actions',
      options: [
        { label: 'Show Steam Keyboard', raw: 'controller_action SHOW_KEYBOARD, , ' },
        { label: 'Toggle Voice Chat', raw: 'controller_action VOICE_CHAT, , ' },
        { label: 'Start Recording', raw: 'controller_action START_RECORDING, , ' },
        { label: 'Take Screenshot', raw: 'controller_action TAKE_SCREENSHOT, , ' },
        { label: 'Open Steam Overlay', raw: 'controller_action SHOW_STEAM_OVERLAY, , ' },
        { label: 'Pause / Resume Game', raw: 'pause_game, , ' }
      ]
    }
  ];

  // Steam VDF bindings have the form `<command> <param>, <icon>, <label>`.
  // The third field is the user-authored label Steam shows in its UI (e.g.
  // "Next radio station"). We expose it as a separate editable field so it
  // round-trips cleanly when reading or writing layouts.
  type SteamBindingTriple = {
    command: string;
    param: string;
    icon: string;
    label: string;
  };
  const parseSteamBindingTriple = (raw: string | null | undefined): SteamBindingTriple => {
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
  const assembleSteamBindingRaw = (triple: SteamBindingTriple): string => {
    const head = triple.param ? `${triple.command} ${triple.param}` : triple.command;
    return `${head}, ${triple.icon ?? ''}, ${triple.label ?? ''}`;
  };
  const steamBindingTargetPart = (raw: string): string => {
    const { command, param } = parseSteamBindingTriple(raw);
    return assembleSteamBindingRaw({ command, param, icon: '', label: '' });
  };
  const steamBindingUserLabel = (binding: { rawBinding?: string | null } | null | undefined) =>
    parseSteamBindingTriple(binding?.rawBinding ?? '').label;
  // Chip-facing label: user-authored Steam label wins, fall back to the
  // friendly parsed name, then "Unassigned" when nothing is bound.
  const chipDisplayLabel = (binding: SteamInputBinding | null | undefined) => {
    if (!binding) return 'Unassigned';
    const userLabel = steamBindingUserLabel(binding);
    return userLabel || binding.binding;
  };

  // Sony-style DualSense artwork mapping. Each entry maps a Steam-input slot to
  // the official-look icon (shown in callouts/source rows) and the focus PNG that
  // lights up the relevant button on the controller stage.
  type SteamSlotGlyph = {
    icon?: string;
    focus?: string;
    region?: 'face' | 'dpad' | 'shoulder' | 'trigger' | 'stick' | 'touch' | 'system' | 'edge' | 'motion';
  };
  const steamSlotGlyphs: Record<string, SteamSlotGlyph> = {
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
    // Swipes use the directional arrow glyphs so each chip is self-describing.
    // They're on a different rail than the D-pad, and their leader lines fan
    // out to the touchpad — position + line direction prevent confusion.
    swipeUp: { icon: 'arrow_up', focus: 'touchpad', region: 'touch' },
    swipeDown: { icon: 'arrow_down', focus: 'touchpad', region: 'touch' },
    swipeLeft: { icon: 'arrow_left', focus: 'touchpad', region: 'touch' },
    swipeRight: { icon: 'arrow_right', focus: 'touchpad', region: 'touch' },
    gyro: { icon: 'analog_stick_l', region: 'motion' },
    // Rear paddles use the official rear focus highlights (they visually
    // light up the paddle area on the front-view artwork). The front Fn
    // nubs sit under each analog stick and don't have a dedicated focus
    // artwork — chip + leader line alone communicate the selection there.
    edgeBackLeft: { icon: 'fn', focus: 'rear_L', region: 'edge' },
    edgeBackRight: { icon: 'fn', focus: 'rear_R', region: 'edge' },
    edgeFnLeft: { icon: 'fn', region: 'edge' },
    edgeFnRight: { icon: 'fn', region: 'edge' }
  };
  const steamSlotIconUrl = (key: string): string | null => {
    const icon = steamSlotGlyphs[key]?.icon;
    return icon ? `/dualsense/icons/iconid_controller_key_${icon}.png` : null;
  };

  // Chip layout for the new mapping stage. Each chip floats at (chipX, chipY)
  // on the stage (in percentages) and the leader line travels to (anchorX, anchorY)
  // on the controller artwork. Sides control which way the chip extends and how
  // its label is aligned.
  type MappingChipPos = {
    key: string;
    side: 'left' | 'right' | 'top' | 'bottom';
    chipX: number;
    chipY: number;
    anchorX: number;
    anchorY: number;
  };
  // Coordinates are calibrated for a stage with aspect-ratio 2/1 and a controller
  // figure that occupies 54% of stage width centered horizontally + vertically.
  // Figure spans roughly 23-77% horizontally and 14.5-85.5% vertically.
  // Each button anchor below corresponds to its position on the controller_front
  // artwork measured in stage-relative percent.
  // Chip-Y values are aligned with each button's anchor-Y so leader lines
  // run nearly horizontally. Where two buttons share an anchor-Y (DPad
  // Left/Right, Square/Circle) chips are staggered just enough to clear each
  // other without lines crossing. Chip-X is pulled inward to ~16/84% so the
  // controller has more room and lines stay short.
  const mappingChipLayout: MappingChipPos[] = [
    // left rail
    { key: 'l2',        side: 'left',  chipX: 16, chipY: 18, anchorX: 35.5, anchorY: 18 },
    { key: 'l1',        side: 'left',  chipX: 16, chipY: 28, anchorX: 35.5, anchorY: 28 },
    { key: 'create',    side: 'left',  chipX: 16, chipY: 36, anchorX: 42.0, anchorY: 35 },
    { key: 'dpadUp',    side: 'left',  chipX: 16, chipY: 43, anchorX: 38.5, anchorY: 41 },
    { key: 'dpadLeft',  side: 'left',  chipX: 16, chipY: 50, anchorX: 35.0, anchorY: 47 },
    { key: 'dpadRight', side: 'left',  chipX: 16, chipY: 57, anchorX: 42.0, anchorY: 47 },
    { key: 'dpadDown',  side: 'left',  chipX: 16, chipY: 64, anchorX: 38.5, anchorY: 53 },
    { key: 'l3',        side: 'left',  chipX: 16, chipY: 72, anchorX: 44.5, anchorY: 60 },
    // right rail
    { key: 'r2',        side: 'right', chipX: 84, chipY: 18, anchorX: 64.5, anchorY: 18 },
    { key: 'r1',        side: 'right', chipX: 84, chipY: 28, anchorX: 64.5, anchorY: 28 },
    { key: 'options',   side: 'right', chipX: 84, chipY: 36, anchorX: 58.0, anchorY: 35 },
    { key: 'triangle',  side: 'right', chipX: 84, chipY: 43, anchorX: 61.5, anchorY: 41 },
    { key: 'circle',    side: 'right', chipX: 84, chipY: 50, anchorX: 65.0, anchorY: 47 },
    { key: 'square',   side: 'right', chipX: 84, chipY: 57, anchorX: 58.0, anchorY: 47 },
    { key: 'cross',    side: 'right', chipX: 84, chipY: 64, anchorX: 61.5, anchorY: 53 },
    { key: 'r3',        side: 'right', chipX: 84, chipY: 72, anchorX: 55.5, anchorY: 60 },
    // top edge — trackpad cluster. Chip order is left-to-right matching each
    // chip's anchor-X on the touchpad so the leader lines fan out cleanly
    // without crossing. Swipe chips use arrow glyphs; click chips use the
    // tap glyph.
    { key: 'swipeLeft',       side: 'top', chipX: 30, chipY: 8, anchorX: 44.0, anchorY: 36 },
    { key: 'touchPressLeft',  side: 'top', chipX: 38, chipY: 8, anchorX: 47.0, anchorY: 36 },
    { key: 'swipeUp',         side: 'top', chipX: 46, chipY: 8, anchorX: 50.0, anchorY: 30 },
    { key: 'swipeDown',       side: 'top', chipX: 54, chipY: 8, anchorX: 50.0, anchorY: 42 },
    { key: 'touchPressRight', side: 'top', chipX: 62, chipY: 8, anchorX: 53.0, anchorY: 36 },
    { key: 'swipeRight',      side: 'top', chipX: 70, chipY: 8, anchorX: 56.0, anchorY: 36 },
    // bottom edge — DualSense Edge buttons (only render when Edge is detected)
    // Back paddles outboard, Fn front nubs inboard pointing straight up at the
    // little Fn button below each analog stick so the lines are unmistakable.
    { key: 'edgeBackLeft',  side: 'bottom', chipX: 30, chipY: 92, anchorX: 45, anchorY: 53 },
    { key: 'edgeFnLeft',    side: 'bottom', chipX: 42, chipY: 92, anchorX: 44, anchorY: 67 },
    { key: 'edgeFnRight',   side: 'bottom', chipX: 58, chipY: 92, anchorX: 56, anchorY: 67 },
    { key: 'edgeBackRight', side: 'bottom', chipX: 70, chipY: 92, anchorX: 55, anchorY: 53 }
  ];
  const resetSteamBindingDraft = () => {
    if (selectedSteamBinding) {
      steamBindingDraft = selectedSteamBinding.rawBinding;
      steamBindingLabelDraft = parseSteamBindingTriple(selectedSteamBinding.rawBinding).label;
      lastSteamBindingDraftKey = steamBindingKey(selectedSteamBinding);
      clearSteamBindingMessage();
    }
  };
  const focusedSlotsByKey = Object.keys(steamSlotGlyphs).reduce<Array<{ key: string; focus: string }>>(
    (acc, key) => {
      const focus = steamSlotGlyphs[key]?.focus;
      if (focus && !acc.some((entry) => entry.focus === focus && entry.key === key)) {
        acc.push({ key, focus });
      }
      return acc;
    },
    []
  );

  const forzaRoutes: Array<{ value: ForzaEffectRoute; label: string }> = [
    { value: 'body_both', label: 'Both grips' },
    { value: 'body_left', label: 'Left grip' },
    { value: 'body_right', label: 'Right grip' },
    { value: 'l2', label: 'L2 trigger' },
    { value: 'r2', label: 'R2 trigger' },
    { value: 'both_triggers', label: 'Both triggers' },
    { value: 'body_and_triggers', label: 'Body + triggers' },
    { value: 'r2_and_body', label: 'R2 + body' },
    { value: 'light_led', label: 'Light / LEDs' }
  ];
  const shiftThumpPresets = [
    { label: 'Soft', intensity: 35 },
    { label: 'Medium', intensity: 65 },
    { label: 'Strong', intensity: 150 },
    { label: 'Max', intensity: 255 }
  ];

  const shiftThumpPresetHelp: Record<string, string> = {
    Soft: 'A lighter mechanical cue for users who want shift feedback without a big kick through the controller.',
    Medium: 'A moderate shift kick that is easy to feel but less abrupt than the stock strong profile.',
    Strong: 'The stock Horizon shift thump: a firmer R2 kick with reduced body feedback for a more physical gear change.',
    Max: 'The strongest shift cue. Uses the full 255 effect ceiling for users who want every gear change to punch through road texture and engine cues.'
  };

  const routeTooltips: Record<ForzaEffectRoute, string> = {
    body_both: 'Sends the effect to both grip motors. Good for road, impacts, and whole-car events.',
    body_left: 'Sends most of the effect to the left grip. Useful when you want to separate a cue from throttle-side feedback.',
    body_right: 'Sends most of the effect to the right grip. Useful for traction or throttle-related cues.',
    l2: 'Sends the effect only to the left adaptive trigger, usually brake-side feedback.',
    r2: 'Sends the effect only to the right adaptive trigger, usually throttle-side feedback.',
    both_triggers: 'Sends trigger feedback to both L2 and R2 without body rumble.',
    body_and_triggers: 'Combines adaptive trigger feedback with a short body thump. Best for gear shifts and other physical events.',
    r2_and_body: 'Combines R2 trigger feedback with a slightly reduced body thump. This is the stock Horizon shift route.',
    light_led: 'Routes the effect to LEDs or the lightbar instead of trigger/body haptics.'
  };

  const triggerEffectHelp: Record<string, string> = {
    'Adaptive resistance': 'A smooth force ramp that increases resistance as the trigger moves. This is the default because it feels closest to pedal load.',
    Pulse: 'A vibration-like trigger pulse. Useful for alerts, but less pedal-like than adaptive resistance.',
    Wall: 'Creates a hard stop at the trigger position. Best for binary actions such as a handbrake wall.',
    Off: 'Disables base trigger force. Telemetry effects can still run if their individual rows are enabled.'
  };

  const triggerStrengthHelp: Record<string, string> = {
    Off: 'No base trigger resistance is applied.',
    Weak: 'Light resistance for users who want subtle feedback or less hand fatigue.',
    Medium: 'Moderate resistance that keeps cues clear without making the triggers heavy.',
    'Strong (Standard)': 'The intended DSCC baseline. Strong enough to feel the curve clearly while staying within comfortable DualSense force levels.'
  };

  const vibrationHelp: Record<string, string> = {
    Off: 'Disables body rumble output while leaving adaptive triggers and LEDs available.',
    Low: 'Keeps grip motors quiet and battery-friendly. Good for long sessions.',
    Medium: 'Moderate body feedback for road texture and event thumps.',
    High: 'Stronger grip feedback. Use when you want road, impact, and shift cues to stand out more.'
  };

  const forzaEffectMetas: ForzaEffectMeta[] = [
    {
      id: 'brake_resistance',
      label: 'Brake pressure',
      signal: 'input.brake',
      group: 'Trigger',
      defaultIntensity: 100,
      defaultRoute: 'l2',
      help: 'Maps brake input to L2 resistance. Higher intensity makes the brake trigger push back harder as braking increases; best left on L2 for a natural brake pedal feel.'
    },
    {
      id: 'abs_slip_pulse',
      label: 'ABS / front slip',
      signal: 'wheel.slip.front_max',
      group: 'Trigger',
      defaultIntensity: 100,
      defaultRoute: 'l2',
      help: 'Adds a quick L2 pulse when front tires lose grip under braking. It is useful for sensing ABS or front lockup without relying on screen or audio cues.'
    },
    {
      id: 'handbrake_wall',
      label: 'Handbrake wall',
      signal: 'input.handbrake',
      group: 'Trigger',
      defaultIntensity: 100,
      defaultRoute: 'l2',
      help: 'Creates a hard L2 wall while the handbrake signal is active. This is an event cue, so it should feel distinct without adding constant body rumble.'
    },
    {
      id: 'throttle_resistance',
      label: 'Throttle load',
      signal: 'input.throttle',
      group: 'Trigger',
      defaultIntensity: 100,
      defaultRoute: 'r2',
      help: 'Maps throttle load to R2 resistance. The Horizon default uses a curved ramp so early throttle remains controllable and force builds toward full throttle.'
    },
    {
      id: 'gear_shift_thump',
      label: 'Paddle shift thump',
      signal: 'drivetrain.shift_pulse',
      group: 'Cue',
      defaultIntensity: 150,
      defaultRoute: 'r2_and_body',
      help: 'Fires a short kick when DSCC detects a gear change. The stock Horizon route uses R2 plus a slightly reduced body thump so shifts feel physical without hitting both triggers.'
    },
    {
      id: 'rev_limiter_buzz',
      label: 'Rev limiter buzz',
      signal: 'vehicle.rpm_ratio',
      group: 'Cue',
      defaultIntensity: 120,
      defaultRoute: 'r2',
      help: 'Adds a high-RPM buzz as the engine approaches the limiter. It is meant as a shift cue, so keep intensity moderate if you already use RPM LEDs.'
    },
    {
      id: 'road_texture',
      label: 'Road texture',
      signal: 'surface.rumble.max',
      group: 'Body',
      defaultIntensity: 60,
      defaultRoute: 'body_both',
      help: 'Uses road surface rumble and speed to add low continuous texture through the grips. It is enabled in the stock Horizon profile at a conservative level.'
    },
    {
      id: 'rumble_strip',
      label: 'Rumble strips',
      signal: 'surface.rumble_strip.max',
      group: 'Body',
      defaultIntensity: 72,
      defaultRoute: 'body_both',
      help: 'Adds stronger body pulses for curbs and rumble strips. It can be informative but uses more continuous motor output, so enable only if you want that extra surface cue.'
    },
    {
      id: 'tire_slip',
      label: 'Tire slip',
      signal: 'wheel.slip.max',
      group: 'Body',
      defaultIntensity: 95,
      defaultRoute: 'body_right',
      help: 'Turns tire slip into body feedback. Routing right keeps it separated from brake cues; raise intensity carefully because sustained sliding can become busy.'
    },
    {
      id: 'puddle_drag',
      label: 'Puddle drag',
      signal: 'surface.puddle.max',
      group: 'Body',
      defaultIntensity: 75,
      defaultRoute: 'body_left',
      help: 'Adds drag feedback when puddle telemetry rises. This helps water feel different from normal road texture without overpowering throttle and shift cues.'
    },
    {
      id: 'suspension_impact',
      label: 'Suspension / impact',
      signal: 'vehicle.acceleration.magnitude',
      group: 'Body',
      defaultIntensity: 115,
      defaultRoute: 'body_both',
      help: 'Uses acceleration spikes and suspension travel to create impact thumps. It is best for jumps, crashes, and hard landings, but can be noisy on rough terrain.'
    },
    {
      id: 'rpm_leds',
      label: 'Gear LEDs + RPM bar',
      signal: 'vehicle.rpm_ratio',
      group: 'Light',
      defaultIntensity: 100,
      defaultRoute: 'light_led',
      help: 'Maps current gear to the five touchpad LEDs and blends the lightbar toward red as RPM approaches redline. Disabled leaves the lightbar on the user-selected profile color.'
    }
  ];

  const FALLBACK_POLL_INTERVAL_MS = 5000;
  const TRIGGER_INPUT_POLL_INTERVAL_MS = 40;
  const BASE_FEEL_TEST_DURATION_MS = 30000;
  const BASE_FEEL_TEST_REFRESH_INTERVAL_MS = 35;
  const SNAPSHOT_INVALIDATION_DEBOUNCE_MS = 500;
  const LIVE_CONFIG_SYNC_DEBOUNCE_MS = 120;

  let snapshot: AppSnapshot | null = null;
  let loading = true;
  let error = '';
  let selectedControllerId = '';
  let applyMessage = '';
  let appSettingsMessage = '';
  let appSettingsBusy = false;
  let profileOverrideMessage = '';
  let toastMessages: ToastMessage[] = [];
  let nextToastId = 1;
  let selectedOverrideProfileId = '';
  let selectedProfileGameId = '';
  let configLoadedFor = '';
  let configLoadError = '';
  let currentControllerConfig: ControllerConfiguration | null = null;
  let profileSaveBaselineSignature = '';
  let profileConfigDirty = false;
  let effectActivityUntil: Record<string, number> = {};
  let partialErrorsDismissed = false;
  let lastPartialErrorSignature = '';
  let newProfileName = '';
  let renameProfileId = '';
  let renameProfileName = '';
  let profileRenameBusy = false;
  let profileSaveBusy = false;
  let profileFileBusy = false;
  let profileImportInput: HTMLInputElement | undefined;
  let profilePanelEl: HTMLDivElement | undefined;
  let manualProfileGameSelection = false;
  let refreshDebounceTimer: number | undefined;
  let fallbackPollTimer: number | undefined;
  let stopSnapshotSocket: (() => void) | undefined;
  let appRuntimeStarted = false;
  let liveConfigSyncTimer: number | undefined;
  let liveConfigSyncInFlight = false;
  let liveConfigSyncQueued = false;
  let pendingVisibilityRefresh = false;
  let baseFeelTestActive = false;
  let baseFeelTestBusy = false;
  let baseFeelTestTimer: number | undefined;
  let baseFeelTestRefreshTimer: number | undefined;
  let baseFeelTestRefreshInFlight = false;
  let baseFeelTestRefreshQueued = false;
  let lastBaseFeelTestRefreshAt = 0;
  let triggerInputPollTimer: number | undefined;
  let triggerInputBusy = false;
  let l2ControllerPress = 0;
  let r2ControllerPress = 0;
  let controllerInputFresh = false;
  let selectedSteamBindingKey = '';
  let selectedSteamBinding: SteamInputBinding | null = null;
  let steamBindingDraft = '';
  let steamBindingLabelDraft = '';
  let lastSteamBindingDraftKey = '';
  let steamBindingBusy = false;
  let steamBindingMessage = '';
  let hoveredSteamSlotKey = '';
  let activeSteamSlotKey = '';
  // Searchable Target combobox state
  let targetPickerOpen = false;
  let targetSearchQuery = '';
  let targetSearchInputEl: HTMLInputElement | null = null;

  let l2From = 20;
  let l2To = 100;
  let r2From = 0;
  let r2To = 100;
  let l2Curve = 1.35;
  let r2Curve = 2.25;
  let curveHover: { side: TriggerSide; x: number; y: number; left: number; top: number } | null = null;
  let curveDragSide: TriggerSide | null = null;
  let activeView: AppView = 'haptics';
  let triggerEffect = 'Adaptive resistance';
  let triggerIntensity = 'Strong (Standard)';
  let vibrationIntensity = 'Medium';
  let lightbarEnabled = true;
  let lightbarColor = '#4cc9f0';
  let rpmColor = '#ff3a2e';
  let lightbarBrightness = 72;

  // Theme-styled color picker (replaces the native OS color dialog).
  const colorPresets = [
    '#3BAEFF', // PS5 vibrant blue (theme accent)
    '#003791', // PlayStation classic blue
    '#4cc9f0', // Cyan
    '#ffffff', // White
    '#ec4899', // Pink
    '#a855f7', // Purple
    '#fb923c', // Orange
    '#ef4444', // Red
    '#4ade80', // Green
    '#facc15'  // Yellow
  ];
  let pickerOpen = false;
  let pickerTarget: ColorPickerTarget = 'lightbar';
  let pickerHue = 195;
  let pickerSat = 0.7;
  let pickerVal = 0.94;
  let pickerHex = lightbarColor;
  let pickerColor = lightbarColor;
  let pickerEl: HTMLDivElement | undefined;
  let lightbarPillEl: HTMLButtonElement | undefined;
  let rpmPillEl: HTMLButtonElement | undefined;

  // Keep the displayed hex in sync with external color changes (profile load).
  $: pickerColor = pickerTarget === 'rpm' ? rpmColor : lightbarColor;
  $: if (!pickerOpen) pickerHex = pickerColor;

  function hsvToHex(h: number, s: number, v: number): string {
    const hh = (((h % 360) + 360) % 360) / 60;
    const c = v * s;
    const x = c * (1 - Math.abs((hh % 2) - 1));
    const m = v - c;
    let r = 0, g = 0, b = 0;
    if (hh < 1) { r = c; g = x; }
    else if (hh < 2) { r = x; g = c; }
    else if (hh < 3) { g = c; b = x; }
    else if (hh < 4) { g = x; b = c; }
    else if (hh < 5) { r = x; b = c; }
    else { r = c; b = x; }
    const toHex = (n: number) => Math.round((n + m) * 255).toString(16).padStart(2, '0');
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
  }
  function hexToHsv(hex: string): { h: number; s: number; v: number } | null {
    const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
    if (!m) return null;
    const r = parseInt(m[1].slice(0, 2), 16) / 255;
    const g = parseInt(m[1].slice(2, 4), 16) / 255;
    const b = parseInt(m[1].slice(4, 6), 16) / 255;
    const max = Math.max(r, g, b);
    const d = max - Math.min(r, g, b);
    let h = 0;
    if (d !== 0) {
      if (max === r) h = ((g - b) / d) % 6;
      else if (max === g) h = (b - r) / d + 2;
      else h = (r - g) / d + 4;
      h *= 60;
      if (h < 0) h += 360;
    }
    return { h, s: max === 0 ? 0 : d / max, v: max };
  }

  function setPickerColor(hex: string) {
    if (pickerTarget === 'rpm') {
      rpmColor = hex;
    } else {
      lightbarColor = hex;
    }
    pickerHex = hex;
    scheduleLiveControllerConfigSync();
  }
  function pickerFallback(target: ColorPickerTarget) {
    return target === 'rpm' ? { h: 4, s: 0.82, v: 1 } : { h: 195, s: 0.7, v: 0.94 };
  }
  function openPicker(target: ColorPickerTarget = 'lightbar') {
    if (!lightbarEnabled) return;
    pickerTarget = target;
    const color = target === 'rpm' ? rpmColor : lightbarColor;
    const hsv = hexToHsv(color) ?? pickerFallback(target);
    pickerHue = hsv.h;
    pickerSat = hsv.s;
    pickerVal = hsv.v;
    pickerHex = color;
    pickerOpen = true;
  }
  function closePicker() { pickerOpen = false; }
  function togglePicker(target: ColorPickerTarget = 'lightbar') {
    pickerOpen && pickerTarget === target ? closePicker() : openPicker(target);
  }

  function commitHsv() {
    const hex = hsvToHex(pickerHue, pickerSat, pickerVal);
    setPickerColor(hex);
  }
  function commitPreset(hex: string) {
    setPickerColor(hex);
    const hsv = hexToHsv(hex) ?? { h: 0, s: 0, v: 0 };
    pickerHue = hsv.h;
    pickerSat = hsv.s;
    pickerVal = hsv.v;
  }
  function commitHex() {
    const m = /^#?([0-9a-f]{6})$/i.exec(pickerHex.trim());
    if (!m) { pickerHex = pickerColor; return; }
    const hex = '#' + m[1].toLowerCase();
    setPickerColor(hex);
    const hsv = hexToHsv(hex) ?? { h: 0, s: 0, v: 0 };
    pickerHue = hsv.h;
    pickerSat = hsv.s;
    pickerVal = hsv.v;
  }
  function handleHueInput(event: Event) {
    pickerHue = +(event.target as HTMLInputElement).value;
    commitHsv();
  }
  function clampUnit(value: number) {
    return Math.max(0, Math.min(1, value));
  }
  function handleSvPointer(event: PointerEvent) {
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    const apply = (e: PointerEvent) => {
      const rect = target.getBoundingClientRect();
      pickerSat = clampUnit((e.clientX - rect.left) / rect.width);
      pickerVal = 1 - clampUnit((e.clientY - rect.top) / rect.height);
      commitHsv();
    };
    apply(event);
    const move = (e: PointerEvent) => apply(e);
    const up = (e: PointerEvent) => {
      try { target.releasePointerCapture(e.pointerId); } catch {}
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerup', up);
      target.removeEventListener('pointercancel', up);
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerup', up);
    target.addEventListener('pointercancel', up);
  }
  function handleSvKeydown(event: KeyboardEvent) {
    const step = event.shiftKey ? 0.1 : 0.01;
    if (event.key === 'ArrowLeft') pickerSat = clampUnit(pickerSat - step);
    else if (event.key === 'ArrowRight') pickerSat = clampUnit(pickerSat + step);
    else if (event.key === 'ArrowDown') pickerVal = clampUnit(pickerVal - step);
    else if (event.key === 'ArrowUp') pickerVal = clampUnit(pickerVal + step);
    else return;

    event.preventDefault();
    commitHsv();
  }
  function handleColorDocClick(event: MouseEvent) {
    if (!pickerOpen) return;
    const t = event.target as Node;
    if (pickerEl?.contains(t) || lightbarPillEl?.contains(t) || rpmPillEl?.contains(t)) return;
    closePicker();
  }
  function handleColorKey(event: KeyboardEvent) {
    if (event.key === 'Escape' && pickerOpen) closePicker();
  }
  let forzaEffects: ForzaEffectConfiguration[] = defaultForzaEffects();
  $: enabledForzaEffectCount = forzaEffects.filter((effect) => effect.enabled).length;
  $: allForzaEffectsEnabled = enabledForzaEffectCount === forzaEffectMetas.length;
  // Reactive lookup map so {@const tuning = ...} inside {#each} re-evaluates
  // when forzaEffects is reassigned (Svelte can't statically trace the
  // dependency through a plain function call to forzaEffect()).
  $: forzaEffectsById = new Map(forzaEffects.map((effect) => [effect.id, effect]));

  $: controllers = snapshot?.controllers ?? [];
  $: if (controllers.length > 0 && !controllers.some((item) => item.id === selectedControllerId)) {
    selectedControllerId = controllers[0].id;
  }
  $: controller = controllers.find((item) => item.id === selectedControllerId) ?? controllers[0];
  $: status = snapshot?.status;
  $: profiles = snapshot?.profiles ?? [];
  $: activeProfileId = profiles.find((profile) => profile.active)?.id ?? snapshot?.profileResolution.selectedProfileId ?? '';
  $: logs = snapshot?.logs ?? [];
  $: diagnostics = snapshot?.diagnostics ?? [];
  $: telemetry = snapshot?.telemetry ?? [];
  $: telemetryByName = new Map(telemetry.map((item) => [item.name, item]));
  $: effectState = snapshot?.effectState;
  $: l2LivePress = controllerInputFresh ? l2ControllerPress : telemetryUnitValue('input.brake');
  $: r2LivePress = controllerInputFresh ? r2ControllerPress : telemetryUnitValue('input.throttle');
  $: appSettings = snapshot?.appSettings;
  $: forzaGlyphs = appSettings?.settings.forzaPlaystationGlyphs;
  $: listenOnAllInterfaces = appSettings?.settings.listenOnAllInterfaces ?? false;
  $: lanRestartRequired = appSettings?.restartRequired ?? false;
  $: glyphOverrideEnabled = forzaGlyphs?.enabled ?? false;
  $: glyphInstallPath =
    forzaGlyphs?.installPath ?? 'C:\\Program Files (x86)\\Steam\\steamapps\\common\\ForzaHorizon6';
  $: integration =
    snapshot?.integrations.find((item) => item.id === snapshot?.profileResolution.activeIntegrationId || item.name === status?.activeIntegration) ??
    snapshot?.integrations[0];
  $: displayedParityEffects = (effectState?.parityEffects ?? []).map((effect) => {
    const id = normalizeEffectId(effect.id);
    return effect.state !== 'disabled' && (effect.state === 'active' || (effectActivityUntil[id] ?? 0) > Date.now())
      ? { ...effect, state: 'active' }
      : effect;
  });
  $: effectStatusById = new Map(displayedParityEffects.map((effect) => [normalizeEffectId(effect.id), effect]));
  $: activeProfileName = effectState?.selectedProfileName ?? status?.activeProfile ?? 'None';
  $: activeProfile = profiles.find((profile) => profile.id === activeProfileId);
  $: selectedOverrideProfile = profiles.find((profile) => profile.id === selectedOverrideProfileId);
  $: selectedActionProfile =
    profiles.find((profile) => profile.id === (selectedOverrideProfileId || activeProfileId)) ??
    activeProfile ??
    null;
  $: canDeleteSelectedProfile = Boolean(selectedActionProfile && selectedActionProfile.scope !== 'Built-in');
  $: canRenameSelectedProfile = Boolean(selectedActionProfile && selectedActionProfile.scope !== 'Built-in');
  $: controllerHeaderName = controllerModelText(controller);
  $: controllerHeaderMeta = controllerConnectionText(controller);
  $: controllerHeaderBatteryReadable = controllerBatteryReadable(controller);
  $: overrideActive = Boolean(snapshot?.profileResolution.overrideProfileId);
  $: detectedGameLabel = snapshot?.gameDetection.activeGameName ?? snapshot?.profileResolution.detectedGameId ?? 'current game';
  $: supportedGames = snapshot?.gameDetection.supportedGames ?? [];
  $: if (selectedProfileGameId && supportedGames.length && !supportedGames.some((game) => game.gameId === selectedProfileGameId)) {
    selectedProfileGameId = '';
  }
  $: if (manualProfileGameSelection && supportedGames.length && !selectedProfileGameId) {
    selectedProfileGameId =
      supportedGames.find((game) => game.running)?.gameId ??
      supportedGames.find((game) => game.installed)?.gameId ??
      supportedGames[0].gameId;
  }
  $: selectedGame =
    snapshot?.gameDetection.selectedGame ??
    supportedGames.find((game) => game.gameId === snapshot?.gameDetection.activeGameId) ??
    null;
  $: profileContextGame =
    (manualProfileGameSelection && selectedProfileGameId
      ? supportedGames.find((game) => game.gameId === selectedProfileGameId)
      : selectedGame) ??
    null;
  $: profileContextGameId =
    profileContextGame?.gameId ?? snapshot?.profileResolution.detectedGameId ?? snapshot?.gameDetection.activeGameId ?? null;
  $: profileContextLabel = profileContextGame?.name ?? detectedGameLabel;
  $: profileContextAssignment = assignmentForGame(profileContextGame);
  $: profileContextDefaultProfileId =
    profileContextAssignment?.profileId ?? defaultProfileIdForGame(profileContextGame);
  $: profileContextDefaultProfile = profiles.find((profile) => profile.id === profileContextDefaultProfileId);
  $: profileContextProfiles = profilesForGame(
    profiles,
    profileContextGame,
    profileContextDefaultProfileId,
    selectedOverrideProfileId,
    activeProfileId
  );
  $: buttonMapButtons = currentControllerConfig?.buttons ?? [];
  $: profileContextBadgeProfile = activeProfile ?? profileContextProfiles[0];
  $: activeProfileContextLabel =
    profileContextGame && profileContextBadgeProfile ? profileContextTag(profileContextBadgeProfile) : 'global scope';
  $: profileContextDetail = profileContextGame
    ? [
        gameTileStatus(profileContextGame),
        formatPlaytime(profileContextGame.stats?.playtimeMinutes),
        achievementText(profileContextGame),
        profileContextDefaultProfile ? `${profileContextDefaultProfile.name} profile` : ''
      ]
        .filter(Boolean)
        .join(' / ')
    : overrideScope;
  $: detectionSignalText = gameDetectionStatusText(snapshot?.gameDetection);
  $: steamContextGame =
    profileContextGame ??
    selectedGame ??
    supportedGames.find((game) => game.running) ??
    supportedGames.find((game) => game.installed) ??
    supportedGames[0] ??
    null;
  $: steamContextArt =
    gameArtwork(steamContextGame, 'capsule') ??
    gameArtwork(steamContextGame, 'banner') ??
    gameArtwork(steamContextGame, 'icon') ??
    '';
  $: steamContextMeta = steamContextGame
    ? [
        steamContextGame.appId ? `Steam ${steamContextGame.appId}` : '',
        formatPlaytime(steamContextGame.stats?.playtimeMinutes),
        achievementText(steamContextGame),
        formatLastPlayed(steamContextGame.stats?.lastPlayedUnix),
        gameTileStatus(steamContextGame)
      ]
        .filter(Boolean)
        .join(' / ')
    : detectionSignalText || 'Steam library data unavailable';
  $: steamInputStatus = snapshot?.steamInput;
  $: steamInputLayout = selectSteamInputLayout(steamInputStatus?.layouts ?? [], steamContextGame, controllerHeaderName);
  $: steamInputBindings = steamInputLayout?.bindings ?? [];
  $: if (steamInputBindings.length && !steamInputBindings.some((binding) => steamBindingKey(binding) === selectedSteamBindingKey)) {
    selectedSteamBindingKey = steamBindingKey(steamInputBindings[0]);
  }
  $: if (!steamInputBindings.length && selectedSteamBindingKey) {
    selectedSteamBindingKey = '';
  }
  $: selectedSteamBinding =
    steamInputBindings.find((binding) => steamBindingKey(binding) === selectedSteamBindingKey) ??
    steamInputBindings[0] ??
    null;
  $: if (selectedSteamBinding && steamBindingKey(selectedSteamBinding) !== lastSteamBindingDraftKey) {
    lastSteamBindingDraftKey = steamBindingKey(selectedSteamBinding);
    steamBindingDraft = selectedSteamBinding.rawBinding;
    steamBindingLabelDraft = parseSteamBindingTriple(selectedSteamBinding.rawBinding).label;
    clearSteamBindingMessage();
  }
  $: steamMappedSlots = steamBindingSlots
    .map((slot) => ({ ...slot, binding: bindingForSteamSlot(steamInputBindings, slot) }))
    .filter((slot) => slot.binding);
  $: steamUnmappedSlots = steamBindingSlots.filter(
    (slot) => !bindingForSteamSlot(steamInputBindings, slot)
  );
  $: steamLayoutTitle = steamInputLayout?.title ?? 'Steam Input Layout';
  $: steamLayoutMeta = steamInputLayout
    ? [
        steamInputLayout.controllerLabel ?? steamInputLayout.controllerType ?? controllerHeaderName,
        steamInputLayout.appId ? `Steam ${steamInputLayout.appId}` : 'global',
        `${steamInputLayout.bindingCount} bindings`
      ]
        .filter(Boolean)
        .join(' / ')
    : steamInputStatus?.available
      ? 'No per-game layout file found'
      : 'Steam config path unavailable';
  $: steamFaceButtonSlots = steamBindingSlots.filter((slot) => slot.group === 'Face Buttons');
  $: steamDpadSlots = steamBindingSlots.filter((slot) => slot.group === 'Directional Pad');
  $: steamShoulderSlots = steamBindingSlots.filter((slot) => slot.group === 'Shoulders' || slot.group === 'Triggers');
  $: steamTrackpadSlots = steamBindingSlots.filter((slot) => slot.group === 'Trackpad');
  $: steamStickSlots = steamBindingSlots.filter((slot) => slot.group === 'Sticks' || slot.group === 'Motion');
  $: steamEdgeSlots = steamBindingSlots.filter((slot) => slot.group === 'DualSense Edge');
  $: steamSystemSlots = steamBindingSlots.filter((slot) => slot.group === 'System');
  // Focused slot drives the controller-stage focus highlight. Hover wins, then
  // explicitly-clicked slot, then the slot owning the currently selected binding.
  $: focusedSlotKey = (() => {
    if (hoveredSteamSlotKey) return hoveredSteamSlotKey;
    if (activeSteamSlotKey) return activeSteamSlotKey;
    const fromBinding = steamBindingSlots.find((slot) => {
      const binding = bindingForSteamSlot(steamInputBindings, slot);
      return Boolean(binding && steamBindingKey(binding) === selectedSteamBindingKey);
    });
    return fromBinding?.key ?? '';
  })();
  $: focusedFocusKey = focusedSlotKey ? steamSlotGlyphs[focusedSlotKey]?.focus ?? '' : '';
  $: focusedSlotMeta = focusedSlotKey
    ? steamBindingSlots.find((slot) => slot.key === focusedSlotKey) ?? null
    : null;
  // Materialised chip list joined with current slot/binding state. Edge chips
  // are hidden when the controller is not an Edge and nothing is mapped to them
  // yet — keeps the stage uncluttered for stock DualSense users.
  $: visibleMappingChips = mappingChipLayout
    .map((chip) => {
      const slot = steamBindingSlots.find((s) => s.key === chip.key);
      if (!slot) return null;
      if (slot.group === 'DualSense Edge') {
        const isEdge = controller?.family === 'DualSense Edge';
        if (!isEdge && !bindingForSteamSlot(steamInputBindings, slot)) return null;
      }
      return { ...chip, slot };
    })
    .filter((value): value is MappingChipPos & { slot: SteamBindingSlot } => value !== null);
  // Count only the chips actually shown on the stage — counting hidden Edge
  // slots or "gyro" gives users a "5 missing" mystery that doesn't match the
  // page they're looking at.
  $: mappedVisibleChipCount = visibleMappingChips.filter((chip) =>
    Boolean(bindingForSteamSlot(steamInputBindings, chip.slot))
  ).length;
  $: telemetryPacketRate = integration?.packetRateHz ?? 0;
  $: telemetryRateText = `${telemetryPacketRate >= 100 ? telemetryPacketRate.toFixed(0) : telemetryPacketRate.toFixed(1)} Hz`;
  $: telemetryRateDetail = telemetryRateStatusText(integration);
  $: overrideScope =
    controller && snapshot
      ? `${controller.name} / ${profileContextLabel}`
      : profileContextLabel;
  // Sync the override dropdown when the ACTIVE profile changes (server-side
  // activation, override flip, snapshot refresh) — but never fight the user
  // who is manually picking from the dropdown. The tracker remembers the last
  // active profile we mirrored, so the reactive block only fires on a real
  // change.
  let lastSyncedActiveProfileId = '';
  $: if (!manualProfileGameSelection && activeProfileId && activeProfileId !== lastSyncedActiveProfileId) {
    selectedOverrideProfileId = activeProfileId;
    lastSyncedActiveProfileId = activeProfileId;
  }
  $: if (profiles.length > 0 && !profiles.some((profile) => profile.id === selectedOverrideProfileId)) {
    selectedOverrideProfileId =
      profileContextDefaultProfileId ||
      activeProfileId ||
      snapshot?.profileResolution.overrideProfileId ||
      snapshot?.profileResolution.selectedProfileId ||
      profiles[0].id;
  }

  function defaultForzaEffects(): ForzaEffectConfiguration[] {
    return forzaEffectMetas.map((effect) => ({
      id: effect.id,
      enabled: true,
      intensity: effect.defaultIntensity,
      route: effect.defaultRoute
    }));
  }

  const trackEffectActivity = (effect: CurrentEffectState) => {
    const now = Date.now();
    const nextActivity = { ...effectActivityUntil };
    for (const item of effect.parityEffects) {
      const id = normalizeEffectId(item.id);
      if (item.state === 'disabled') {
        delete nextActivity[id];
      } else if (item.state === 'active') {
        nextActivity[id] = now + 550;
      } else if ((nextActivity[id] ?? 0) <= now) {
        delete nextActivity[id];
      }
    }
    effectActivityUntil = nextActivity;
  };

  const applySnapshot = (next: AppSnapshot) => {
    trackEffectActivity(next.effectState);
    const signature = (next.partialErrors ?? []).map((entry) => entry.endpoint).sort().join('|');
    if (signature !== lastPartialErrorSignature) {
      partialErrorsDismissed = false;
      lastPartialErrorSignature = signature;
    }
    snapshot = next;
    error = '';
    loading = false;
  };

  const refresh = async () => {
    try {
      applySnapshot(await getAppSnapshot());
      error = '';
    } catch (caught) {
      error = caught instanceof Error ? caught.message : 'Unable to load live command center state.';
    } finally {
      loading = false;
    }
  };

  const scheduleRefresh = () => {
    if (document.hidden) {
      pendingVisibilityRefresh = true;
      return;
    }
    if (refreshDebounceTimer !== undefined) return;
    if (typeof window.setTimeout !== 'function') {
      void refresh();
      return;
    }
    refreshDebounceTimer = window.setTimeout(() => {
      refreshDebounceTimer = undefined;
      void refresh();
    }, SNAPSHOT_INVALIDATION_DEBOUNCE_MS);
  };

  $: partialErrors = snapshot?.partialErrors ?? [];
  $: showPartialErrorBanner = partialErrors.length > 0 && !partialErrorsDismissed;
  const dismissPartialErrors = () => {
    partialErrorsDismissed = true;
  };

  const clamp = (value: number, min = 0, max = 100) => Math.max(min, Math.min(max, value));
  const clampForzaIntensity = (value: number) => Math.round(clamp(Number(value) || 0, 0, 255));
  const clampForzaPercent = (value: number | string) => {
    const numeric = typeof value === 'number' ? value : Number(value);
    return Math.round(clamp(Number.isFinite(numeric) ? numeric : 0, 0, 100));
  };
  const forzaIntensityPercent = (intensity: number) => Math.round((clampForzaIntensity(intensity) / 255) * 100);
  const forzaIntensityFromPercent = (percent: number | string) => Math.round(clampForzaPercent(percent) * 2.55);
  type TriggerSide = 'l2' | 'r2';
  type TriggerRangeEdge = 'from' | 'to';
  const defaultTriggerCurve = (side: TriggerSide) => (side === 'l2' ? 1.35 : 2.25);

  const appViewFromHash = (): AppView =>
    typeof window !== 'undefined' && window.location.hash === '#/button-mapping'
      ? 'buttonMapping'
      : 'haptics';

  const navigateToView = (view: AppView) => {
    activeView = view;
    if (typeof window === 'undefined') return;
    const nextHash = appViews.find((item) => item.id === view)?.hash ?? appViews[0].hash;
    if (window.location.hash !== nextHash) window.location.hash = nextHash;
  };

  const normalizeTriggerPercent = (value: number | string) => {
    const numeric = typeof value === 'number' ? value : Number(value);
    return Math.round(clamp(Number.isFinite(numeric) ? numeric : 0, 0, 100));
  };

  const normalizeTriggerCurve = (value: number | string | undefined, fallback = 1.35) => {
    const numeric = typeof value === 'number' ? value : Number(value);
    const safe = Number.isFinite(numeric) ? numeric : fallback;
    return Math.round(clamp(safe, 0.5, 3.5) * 100) / 100;
  };

  const toastToneForMessage = (message: string, fallback: ToastTone = 'success'): ToastTone => {
    if (/(unable|failed|error|blocked|denied|unavailable|not found|cannot|could not|requires|invalid|refusing)/i.test(message)) {
      return 'error';
    }
    if (/(saving|validating|loading|testing|waiting|restart)/i.test(message)) {
      return 'info';
    }
    return fallback;
  };

  const dismissToast = (id: number) => {
    toastMessages = toastMessages.filter((toast) => toast.id !== id);
  };

  const showToast = (message: string, tone: ToastTone = toastToneForMessage(message)) => {
    const text = message.trim();
    if (!text) return;
    const id = nextToastId++;
    toastMessages = [
      ...toastMessages.filter((toast) => toast.message !== text),
      { id, tone, message: text }
    ].slice(-4);
    window.setTimeout(() => dismissToast(id), tone === 'error' ? 6500 : 4200);
  };

  const normalizedSteamControllerType = (label: string | null | undefined) => {
    const value = (label ?? '').toLowerCase();
    if (value.includes('edge')) return 'controller_ps5_edge';
    if (value.includes('dualsense') || value.includes('ps5')) return 'controller_ps5';
    if (value.includes('dualshock') || value.includes('ps4')) return 'controller_ps4';
    return '';
  };

  const selectSteamInputLayout = (
    layouts: SteamInputLayout[],
    game: SupportedGame | null | undefined,
    controllerName: string
  ) => {
    if (!layouts.length) return null;
    const appId = game?.appId ?? null;
    const controllerType = normalizedSteamControllerType(controllerName);
    const sameApp = appId ? layouts.filter((layout) => layout.appId === appId) : [];
    const candidates = sameApp.length ? sameApp : layouts;
    return (
      candidates.find((layout) => layout.controllerType === controllerType) ??
      candidates.find((layout) => layout.controllerType === 'controller_ps5_edge') ??
      candidates.find((layout) => layout.controllerType === 'controller_ps5') ??
      candidates[0] ??
      null
    );
  };

  const steamBindingKey = (binding: SteamInputBinding) =>
    [
      binding.groupId ?? '',
      binding.source ?? '',
      binding.sourceMode ?? '',
      binding.inputId ?? '',
      binding.activator ?? ''
    ].join('|');

  const steamBindingSignature = (binding: SteamInputBinding) => {
    const inputId = binding.inputId ?? '';
    const source = (binding.source ?? '').toLowerCase().replaceAll(' ', '_');
    return source ? [`${inputId}:${source}`, `${source}:${inputId}`, inputId] : [inputId];
  };

  const bindingForSteamSlot = (bindings: SteamInputBinding[], slot: SteamBindingSlot) =>
    bindings.find((binding) => {
      const signatures = steamBindingSignature(binding);
      return slot.inputIds.some((inputId) => signatures.includes(inputId));
    }) ?? null;

  const compactSteamBindingLabel = (binding: SteamInputBinding | null | undefined) =>
    binding ? binding.binding : 'Unassigned';

  const steamBindingKindLabel = (binding: SteamInputBinding | null | undefined) =>
    [binding?.kind, binding?.activator].filter(Boolean).join(' / ') || 'Steam Input';

  const steamBindingTargetKnown = (rawBinding: string) => {
    const targetOnly = steamBindingTargetPart(rawBinding);
    return steamBindingTargetGroups.some((group) =>
      group.options.some((option) => steamBindingTargetPart(option.raw) === targetOnly)
    );
  };
  // Update steamBindingDraft when one of the structured fields (target / label)
  // is edited, preserving the rest. Touching the raw VDF input still wins.
  const applySteamBindingTargetChange = (nextTargetRaw: string) => {
    const next = parseSteamBindingTriple(nextTargetRaw);
    const current = parseSteamBindingTriple(steamBindingDraft);
    steamBindingDraft = assembleSteamBindingRaw({
      command: next.command,
      param: next.param,
      icon: current.icon,
      label: current.label
    });
  };
  const applySteamBindingLabelChange = (nextLabel: string) => {
    steamBindingLabelDraft = nextLabel;
    const current = parseSteamBindingTriple(steamBindingDraft);
    steamBindingDraft = assembleSteamBindingRaw({
      ...current,
      label: nextLabel
    });
  };
  const syncSteamBindingLabelFromRaw = () => {
    steamBindingLabelDraft = parseSteamBindingTriple(steamBindingDraft).label;
  };

  // Filtered groups for the searchable Target combobox. Empty query keeps
  // every group. A non-empty query narrows each group's options by label,
  // raw VDF, or category name, and drops groups that end up empty.
  $: filteredTargetGroups = (() => {
    const q = targetSearchQuery.trim().toLowerCase();
    if (!q) return steamBindingTargetGroups;
    return steamBindingTargetGroups
      .map((group) => {
        const groupMatches = group.label.toLowerCase().includes(q);
        const options = groupMatches
          ? group.options
          : group.options.filter(
              (option) =>
                option.label.toLowerCase().includes(q) ||
                option.raw.toLowerCase().includes(q)
            );
        return { ...group, options };
      })
      .filter((group) => group.options.length > 0);
  })();

  // Label shown on the closed combobox trigger.
  const currentTargetLabel = (): string => {
    if (!steamBindingDraft) return 'Select target…';
    const current = steamBindingTargetPart(steamBindingDraft);
    for (const group of steamBindingTargetGroups) {
      for (const option of group.options) {
        if (steamBindingTargetPart(option.raw) === current) return option.label;
      }
    }
    const { command, param } = parseSteamBindingTriple(steamBindingDraft);
    if (!command) return 'Select target…';
    return param ? `Custom: ${command} ${param}` : `Custom: ${command}`;
  };

  const openTargetPicker = () => {
    targetPickerOpen = true;
    targetSearchQuery = '';
    // Focus the search input shortly after the panel mounts.
    queueMicrotask(() => targetSearchInputEl?.focus());
  };
  const closeTargetPicker = () => {
    targetPickerOpen = false;
    targetSearchQuery = '';
  };
  const toggleTargetPicker = () => {
    if (targetPickerOpen) closeTargetPicker();
    else openTargetPicker();
  };
  const pickTargetOption = (rawOption: string) => {
    applySteamBindingTargetChange(rawOption);
    closeTargetPicker();
  };
  const handleTargetPickerKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      closeTargetPicker();
    }
  };

  // Svelte action: invoke callback on mousedown outside the node.
  function clickOutside(node: HTMLElement, callback: () => void) {
    const onMouseDown = (event: MouseEvent) => {
      if (!node.contains(event.target as Node)) callback();
    };
    document.addEventListener('mousedown', onMouseDown);
    return {
      destroy() {
        document.removeEventListener('mousedown', onMouseDown);
      }
    };
  }

  const clearSteamBindingMessage = () => {
    steamBindingMessage = '';
  };

  const setSteamBindingMessage = (message: string, tone: ToastTone = toastToneForMessage(message, 'info')) => {
    steamBindingMessage = message;
    showToast(message, tone);
  };

  const selectSteamBinding = (binding: SteamInputBinding | null | undefined) => {
    if (!binding) {
      setSteamBindingMessage('That Steam input is not present in the loaded layout yet.', 'info');
      return;
    }
    selectedSteamBindingKey = steamBindingKey(binding);
    lastSteamBindingDraftKey = selectedSteamBindingKey;
    steamBindingDraft = binding.rawBinding;
    steamBindingLabelDraft = parseSteamBindingTriple(binding.rawBinding).label;
    clearSteamBindingMessage();
  };

  const selectSteamSlot = (slot: SteamBindingSlot) => {
    activeSteamSlotKey = slot.key;
    const binding = bindingForSteamSlot(steamInputBindings, slot);
    if (binding) {
      selectSteamBinding(binding);
    } else {
      setSteamBindingMessage(`${slot.label} has no Steam Input binding in this layout yet.`, 'info');
    }
  };

  const isSteamSlotSelected = (slot: SteamBindingSlot) => {
    if (activeSteamSlotKey && activeSteamSlotKey === slot.key) return true;
    const binding = bindingForSteamSlot(steamInputBindings, slot);
    return Boolean(binding && steamBindingKey(binding) === selectedSteamBindingKey);
  };

  const hoverSteamSlot = (slot: SteamBindingSlot | null) => {
    hoveredSteamSlotKey = slot?.key ?? '';
  };

  const saveSteamBinding = async (dryRun = false) => {
    if (!steamInputLayout || !selectedSteamBinding) {
      setSteamBindingMessage('Load a Steam Input layout and select a binding first.', 'error');
      return;
    }
    const rawBinding = steamBindingDraft.trim();
    if (!rawBinding) {
      setSteamBindingMessage('Choose a target binding before saving.', 'error');
      return;
    }
    steamBindingBusy = true;
    setSteamBindingMessage(dryRun ? 'Validating Steam Input write...' : 'Saving Steam Input binding...', 'info');
    try {
      const response = await writeSteamInputBinding({
        layoutSource: steamInputLayout.source,
        appId: steamInputLayout.appId ?? steamContextGame?.appId ?? null,
        inputId: selectedSteamBinding.inputId,
        groupId: selectedSteamBinding.groupId ?? null,
        activator: selectedSteamBinding.activator ?? null,
        rawBinding,
        profileName: activeProfileName || profileContextGame?.name || steamContextGame?.name || null,
        dryRun
      });
      setSteamBindingMessage(
        response.backupPath ? `${response.message} Backup: ${response.backupPath}` : response.message,
        'success'
      );
      selectedSteamBindingKey = steamBindingKey(response.binding);
      lastSteamBindingDraftKey = selectedSteamBindingKey;
      steamBindingDraft = response.binding.rawBinding;
      steamBindingLabelDraft = parseSteamBindingTriple(response.binding.rawBinding).label;
      if (!dryRun) await refresh();
    } catch (caught) {
      setSteamBindingMessage(caught instanceof Error ? caught.message : 'Unable to write Steam Input binding.', 'error');
    } finally {
      steamBindingBusy = false;
    }
  };

  const setTriggerRangeValue = (side: TriggerSide, edge: TriggerRangeEdge, rawValue: number | string) => {
    const value = normalizeTriggerPercent(rawValue);
    if (side === 'l2') {
      if (edge === 'from') {
        l2From = Math.min(value, l2To);
      } else {
        l2To = Math.max(value, l2From);
      }
    } else {
      if (edge === 'from') {
        r2From = Math.min(value, r2To);
      } else {
        r2To = Math.max(value, r2From);
      }
    }
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  const setTriggerCurveValue = (side: TriggerSide, rawValue: number | string) => {
    const value = normalizeTriggerCurve(rawValue, defaultTriggerCurve(side));
    if (side === 'l2') {
      l2Curve = value;
    } else {
      r2Curve = value;
    }
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };
  const normalizeEffectId = (id: string) => id.replaceAll('-', '_');
  const gameArtwork = (
    game: SupportedGame | null | undefined,
    kind: 'icon' | 'banner' | 'hero' | 'capsule'
  ): string | null => {
    if (!game?.artwork) return null;
    if (kind === 'icon') return game.artwork.iconUrl ?? game.artwork.capsuleUrl ?? game.artwork.bannerUrl ?? null;
    if (kind === 'banner') return game.artwork.bannerUrl ?? game.artwork.heroUrl ?? game.artwork.capsuleUrl ?? null;
    if (kind === 'hero') return game.artwork.heroUrl ?? game.artwork.bannerUrl ?? game.artwork.capsuleUrl ?? null;
    return game.artwork.capsuleUrl ?? game.artwork.bannerUrl ?? game.artwork.heroUrl ?? null;
  };

  const isForzaHorizonGame = (game: SupportedGame | null | undefined) =>
    Boolean(game?.gameId.toLowerCase().startsWith('forza-horizon'));

  const profileAssignmentMatchesGame = (assignment: ProfileAssignmentConfiguration, game: SupportedGame) => {
    const assignmentGameId = assignment.gameId.trim().toLowerCase();
    const gameId = game.gameId.trim().toLowerCase();
    return assignmentGameId === gameId || gameId.startsWith(`${assignmentGameId}-`);
  };

  const assignmentForGame = (game: SupportedGame | null | undefined) => {
    if (!game) return undefined;
    return currentControllerConfig?.profileAssignments.find((assignment) =>
      profileAssignmentMatchesGame(assignment, game)
    );
  };

  const defaultProfileIdForGame = (game: SupportedGame | null | undefined) => {
    const assignment = assignmentForGame(game);
    if (assignment?.profileId && profiles.some((profile) => profile.id === assignment.profileId)) {
      return assignment.profileId;
    }
    if (isForzaHorizonGame(game)) {
      return profiles.find((profile) => profile.id === 'forza-horizon')?.id ?? activeProfileId ?? profiles[0]?.id ?? '';
    }
    return activeProfileId || profiles[0]?.id || '';
  };

  const profilesForGame = (
    source: ProfileSummary[],
    game: SupportedGame | null | undefined,
    defaultProfileId: string,
    selectedProfileId: string,
    activeId: string
  ) =>
    source
      .map((profile, index) => ({ profile, index }))
      .sort((left, right) => {
        const rank = (profile: ProfileSummary) => {
          if (game && profile.id === defaultProfileId) return 0;
          if (profile.id === selectedProfileId) return 1;
          if (profile.id === activeId) return 2;
          if (profile.scope === 'Built-in') return 3;
          return 4;
        };
        return rank(left.profile) - rank(right.profile) || left.index - right.index;
      })
      .map(({ profile }) => profile);

  const profileContextTag = (profile: ProfileSummary) => {
    if (profileContextGame && profile.id === profileContextDefaultProfileId) return 'recommended';
    if (profile.id === activeProfileId) return 'active';
    return profile.scope === 'Built-in' ? 'built-in' : profile.scope.toLowerCase();
  };

  const gameLauncherLabel = (game: SupportedGame) =>
    [
      game.name,
      game.appId ? `Steam ${game.appId}` : '',
      game.running ? 'running' : game.installed ? 'installed' : 'not installed'
    ]
      .filter(Boolean)
      .join(' / ');

  const setProfileGameSelectionMode = (manual: boolean) => {
    manualProfileGameSelection = manual;
    if (!manual) {
      selectedProfileGameId = '';
      if (activeProfileId) {
        selectedOverrideProfileId = activeProfileId;
        lastSyncedActiveProfileId = activeProfileId;
      }
      return;
    }

    const preferredGame =
      (selectedProfileGameId ? supportedGames.find((game) => game.gameId === selectedProfileGameId) : null) ??
      selectedGame ??
      supportedGames.find((game) => game.running) ??
      supportedGames.find((game) => game.installed) ??
      supportedGames[0];
    if (preferredGame) {
      selectedProfileGameId = preferredGame.gameId;
      const preferredProfileId = defaultProfileIdForGame(preferredGame);
      if (preferredProfileId) selectedOverrideProfileId = preferredProfileId;
    }
  };

  const selectProfileGame = (game: SupportedGame) => {
    manualProfileGameSelection = true;
    selectedProfileGameId = game.gameId;
    const preferredProfileId = defaultProfileIdForGame(game);
    if (preferredProfileId) selectedOverrideProfileId = preferredProfileId;
    window.requestAnimationFrame(() => {
      profilePanelEl?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    });
  };

  const normalizeForzaEffects = (effects: ForzaEffectConfiguration[] | undefined): ForzaEffectConfiguration[] => {
    const source = new Map((effects ?? []).map((effect) => [effect.id, effect]));
    return forzaEffectMetas.map((meta) => {
      const effect = source.get(meta.id);
      const route = effect?.route && forzaRoutes.some((item) => item.value === effect.route) ? effect.route : meta.defaultRoute;
      return {
        id: meta.id,
        enabled: effect?.enabled ?? true,
        intensity: clampForzaIntensity(effect?.intensity ?? meta.defaultIntensity),
        route
      };
    });
  };

  const editableConfigFromController = (config: ControllerConfiguration): EditableControllerConfig => ({
    inputMode: config.inputMode,
    trigger: config.trigger,
    lightbar: config.lightbar,
    forza: config.forza,
    sticks: config.sticks,
    buttons: config.buttons,
    profileAssignments: config.profileAssignments
  });

  const profileConfigSignature = (config: EditableControllerConfig | ControllerConfiguration): string =>
    JSON.stringify({
      inputMode: config.inputMode,
      trigger: {
        sameRange: false,
        l2From: normalizeTriggerPercent(config.trigger.l2From),
        l2To: normalizeTriggerPercent(config.trigger.l2To),
        r2From: normalizeTriggerPercent(config.trigger.r2From),
        r2To: normalizeTriggerPercent(config.trigger.r2To),
        l2Curve: normalizeTriggerCurve(config.trigger.l2Curve, defaultTriggerCurve('l2')),
        r2Curve: normalizeTriggerCurve(config.trigger.r2Curve, defaultTriggerCurve('r2')),
        effect: config.trigger.effect,
        intensity: config.trigger.intensity,
        vibration: config.trigger.vibration
      },
      lightbar: {
        enabled: config.lightbar?.enabled ?? true,
        color: config.lightbar?.color ?? '#4cc9f0',
        rpmColor: config.lightbar?.rpmColor ?? '#ff3a2e',
        brightness: normalizeTriggerPercent(config.lightbar?.brightness ?? 72)
      },
      forza: {
        effects: normalizeForzaEffects(config.forza?.effects).map((effect) => ({
          id: effect.id,
          enabled: effect.enabled,
          intensity: forzaIntensityPercent(effect.intensity),
          route: effect.route
        }))
      },
      sticks: config.sticks,
      buttons: config.buttons,
      profileAssignments: config.profileAssignments
    });

  $: profileConfigDirty =
    Boolean(currentControllerConfig && profileSaveBaselineSignature) &&
    profileConfigSignature(buildControllerConfig()) !== profileSaveBaselineSignature;

  const forzaEffect = (id: string): ForzaEffectConfiguration =>
    forzaEffects.find((effect) => effect.id === id) ??
    defaultForzaEffects().find((effect) => effect.id === id) ??
    defaultForzaEffects()[0];

  const updateForzaEffect = (id: string, patch: Partial<ForzaEffectConfiguration>) => {
    forzaEffects = normalizeForzaEffects(
      forzaEffects.map((effect) =>
        effect.id === id
          ? {
              ...effect,
              ...patch,
              intensity:
                patch.intensity === undefined ? effect.intensity : clampForzaIntensity(patch.intensity)
            }
          : effect
      )
    );
    scheduleLiveControllerConfigSync();
  };

  const applyShiftThumpPreset = (intensity: number) => {
    updateForzaEffect('gear_shift_thump', {
      enabled: intensity > 0,
      intensity,
      route: 'r2_and_body'
    });
  };

  const setAllForzaEffects = (enabled: boolean) => {
    forzaEffects = normalizeForzaEffects(forzaEffects.map((effect) => ({ ...effect, enabled })));
    scheduleLiveControllerConfigSync();
  };
  const toggleAllForzaEffects = () => {
    setAllForzaEffects(!allForzaEffectsEnabled);
  };

  const telemetryUnitValue = (signal: string) => {
    const value = telemetryByName.get(signal)?.value;
    return typeof value === 'number' && Number.isFinite(value) ? clampUnit(value) : 0;
  };

  const triggerStrengthScalarFor = (effect: string, intensity: string) => {
    if (effect === 'Off' || intensity === 'Off') return 0;
    if (intensity === 'Weak') return 0.36;
    if (intensity === 'Medium') return 0.68;
    return 1;
  };

  const triggerStrengthScalar = () => triggerStrengthScalarFor(triggerEffect, triggerIntensity);

  const triggerRangeValuesFor = (fromRaw: number | string, toRaw: number | string) => {
    const from = normalizeTriggerPercent(fromRaw);
    const to = Math.max(from, normalizeTriggerPercent(toRaw));
    return { from, to, width: Math.max(0, to - from) };
  };

  const triggerRangeValues = (side: TriggerSide) => {
    return side === 'l2' ? triggerRangeValuesFor(l2From, l2To) : triggerRangeValuesFor(r2From, r2To);
  };

  const triggerCurveValueFor = (
    position: number,
    fromRaw: number | string,
    toRaw: number | string,
    curveRaw: number | string,
    fallbackCurve: number,
    effect: string,
    intensity: string
  ) => {
    const range = triggerRangeValuesFor(fromRaw, toRaw);
    const start = range.from / 100;
    const end = Math.max(start + 0.01, range.to / 100);
    const curve = normalizeTriggerCurve(curveRaw, fallbackCurve);
    const strength = triggerStrengthScalarFor(effect, intensity);
    if (strength <= 0) return 0;
    const x = clampUnit(position);
    const active = x <= start ? 0 : Math.pow(clampUnit((x - start) / (end - start)), curve);
    return clampUnit(active * strength);
  };

  const triggerCurveValue = (side: TriggerSide, position: number) =>
    side === 'l2'
      ? triggerCurveValueFor(position, l2From, l2To, l2Curve, defaultTriggerCurve('l2'), triggerEffect, triggerIntensity)
      : triggerCurveValueFor(position, r2From, r2To, r2Curve, defaultTriggerCurve('r2'), triggerEffect, triggerIntensity);

  const triggerCurvePathFor = (
    fromRaw: number | string,
    toRaw: number | string,
    curveRaw: number | string,
    fallbackCurve: number,
    effect: string,
    intensity: string,
    livePress?: number
  ) => {
    const samplePositions = Array.from({ length: 101 }, (_, index) => index / 100);
    if (livePress !== undefined) {
      samplePositions.push(clampUnit(livePress));
    }
    const points = [...new Set(samplePositions)]
      .sort((a, b) => a - b)
      .map((x) => {
        const y = 1 - triggerCurveValueFor(x, fromRaw, toRaw, curveRaw, fallbackCurve, effect, intensity);
        return `${(x * 100).toFixed(2)},${(y * 100).toFixed(2)}`;
      });
    return `M ${points.join(' L ')}`;
  };

  const triggerCurveView = (
    fromRaw: number | string,
    toRaw: number | string,
    curveRaw: number | string,
    fallbackCurve: number,
    livePress: number,
    effect: string,
    intensity: string
  ) => {
    const range = triggerRangeValuesFor(fromRaw, toRaw);
    const liveX = clampUnit(livePress) * 100;
    const liveY = 100 - triggerCurveValueFor(livePress, fromRaw, toRaw, curveRaw, fallbackCurve, effect, intensity) * 100;
    return {
      rangeStart: range.from.toFixed(2),
      rangeEnd: range.to.toFixed(2),
      rangeWidth: range.width.toFixed(2),
      path: triggerCurvePathFor(fromRaw, toRaw, curveRaw, fallbackCurve, effect, intensity, livePress),
      liveX: liveX.toFixed(2),
      liveY: liveY.toFixed(2)
    };
  };

  $: l2CurveView = triggerCurveView(l2From, l2To, l2Curve, defaultTriggerCurve('l2'), l2LivePress, triggerEffect, triggerIntensity);
  $: r2CurveView = triggerCurveView(r2From, r2To, r2Curve, defaultTriggerCurve('r2'), r2LivePress, triggerEffect, triggerIntensity);

  const triggerPressLabel = (value: number) => `${Math.round(clampUnit(value) * 100)}%`;
  const showTriggerPress = (_side: 'l2' | 'r2', value: number) =>
    baseFeelTestActive || clampUnit(value) > 0.01;

  const intensityTooltip = (meta: ForzaEffectMeta, intensity: number) =>
    `${meta.label} intensity is ${forzaIntensityPercent(intensity)}% (${clampForzaIntensity(intensity)} / 255 raw). This scales trigger, rumble, or LED output depending on signal and route.`;

  const routeTooltip = (route: ForzaEffectRoute) => routeTooltips[route] ?? 'Selects where DSCC sends this telemetry effect.';

  const triggerRangeTooltip = (side: 'L2' | 'R2', edge: 'from' | 'to', value: number) =>
    edge === 'from'
      ? `${side} starts building force at ${value}% trigger travel. Raising this creates more free travel before resistance begins.`
      : `${side} reaches full configured force at ${value}% trigger travel. Lowering this makes the force curve finish earlier.`;

  const triggerCurveTooltip = (side: 'L2' | 'R2', value: number) =>
    `${side} curve is ${value.toFixed(2)}. 1.00 is linear; lower values bring resistance in earlier, while higher values keep the pedal lighter at first and ramp harder near the end.`;

  const curveGraphPointFromPointer = (event: PointerEvent, target: HTMLElement) => {
    const rect = target.getBoundingClientRect();
    const x = clampUnit((event.clientX - rect.left) / Math.max(1, rect.width));
    const output = clampUnit(1 - (event.clientY - rect.top) / Math.max(1, rect.height));
    return { x, output };
  };

  const setCurveHover = (side: TriggerSide, x: number) => {
    const y = triggerCurveValue(side, x);
    curveHover = {
      side,
      x,
      y,
      left: x * 100,
      top: (1 - y) * 100
    };
  };

  const curveValueFromGraphPoint = (side: TriggerSide, input: number, output: number) => {
    const range = triggerRangeValues(side);
    const start = range.from / 100;
    const end = Math.max(start + 0.01, range.to / 100);
    const activeTravel = clamp((input - start) / (end - start), 0.03, 0.97);
    const strength = triggerStrengthScalar();
    const normalizedOutput = clamp(strength > 0 ? output / strength : output, 0.02, 0.98);
    return normalizeTriggerCurve(Math.log(normalizedOutput) / Math.log(activeTravel), defaultTriggerCurve(side));
  };

  const updateCurveHover = (event: PointerEvent, side: TriggerSide) => {
    const target = event.currentTarget as HTMLElement;
    const { x } = curveGraphPointFromPointer(event, target);
    setCurveHover(side, x);
  };

  const handleCurvePointer = (event: PointerEvent, side: TriggerSide) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    event.preventDefault();

    const target = event.currentTarget as HTMLElement;
    curveDragSide = side;
    target.setPointerCapture(event.pointerId);

    const applyPoint = (pointerEvent: PointerEvent) => {
      const { x, output } = curveGraphPointFromPointer(pointerEvent, target);
      setTriggerCurveValue(side, curveValueFromGraphPoint(side, x, output));
      setCurveHover(side, x);
    };

    const stopDrag = () => {
      curveDragSide = null;
      if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
      target.removeEventListener('pointermove', applyPoint);
      target.removeEventListener('pointerup', stopDrag);
      target.removeEventListener('pointercancel', stopDrag);
    };

    applyPoint(event);
    target.addEventListener('pointermove', applyPoint);
    target.addEventListener('pointerup', stopDrag);
    target.addEventListener('pointercancel', stopDrag);
  };

  const clearCurveHover = (side: TriggerSide) => {
    if (curveDragSide === side) return;
    if (curveHover?.side === side) curveHover = null;
  };

  const applyEditableConfig = (config: Omit<ControllerConfiguration, 'controllerId' | 'model'>) => {
    l2From = normalizeTriggerPercent(config.trigger.l2From);
    l2To = Math.max(l2From, normalizeTriggerPercent(config.trigger.l2To));
    r2From = normalizeTriggerPercent(config.trigger.r2From);
    r2To = Math.max(r2From, normalizeTriggerPercent(config.trigger.r2To));
    l2Curve = normalizeTriggerCurve(config.trigger.l2Curve, defaultTriggerCurve('l2'));
    r2Curve = normalizeTriggerCurve(config.trigger.r2Curve, defaultTriggerCurve('r2'));
    triggerEffect = config.trigger.effect;
    triggerIntensity = config.trigger.intensity;
    vibrationIntensity = config.trigger.vibration;
    lightbarEnabled = config.lightbar?.enabled ?? true;
    lightbarColor = config.lightbar?.color ?? '#4cc9f0';
    rpmColor = config.lightbar?.rpmColor ?? '#ff3a2e';
    lightbarBrightness = config.lightbar?.brightness ?? 72;
    forzaEffects = normalizeForzaEffects(config.forza?.effects);
  };
  const applyControllerConfig = (config: ControllerConfiguration, updateProfileBaseline = true) => {
    currentControllerConfig = config;
    applyEditableConfig(config);
    if (updateProfileBaseline) profileSaveBaselineSignature = profileConfigSignature(config);
  };

  const loadControllerConfig = async (controllerId: string) => {
    configLoadedFor = controllerId;
    configLoadError = '';
    currentControllerConfig = null;
    profileSaveBaselineSignature = '';
    try {
      applyControllerConfig(await getControllerConfig(controllerId));
    } catch (caught) {
      configLoadError = caught instanceof Error ? caught.message : 'Unable to load controller configuration.';
      showToast(configLoadError, 'error');
    }
  };

  const buildDefaultControllerConfig = (): EditableControllerConfig => ({
    inputMode: 'native_dualsense',
    trigger: {
      sameRange: false,
      l2From: 0,
      l2To: 100,
      r2From: 0,
      r2To: 100,
      l2Curve: 1.35,
      r2Curve: 2.25,
      effect: 'Adaptive resistance',
      intensity: 'Strong (Standard)',
      vibration: 'Medium'
    },
    lightbar: {
      enabled: true,
      color: '#4cc9f0',
      rpmColor: '#ff3a2e',
      brightness: 72
    },
    forza: {
      effects: defaultForzaEffects()
    },
    sticks: {
      leftCurve: 'Default',
      leftCurveAmount: 50,
      leftDeadzone: 0,
      rightCurve: 'Default',
      rightCurveAmount: 50,
      rightDeadzone: 0
    },
    buttons: [],
    profileAssignments: []
  });

  const baseForzaTriggerDefaults = (): EditableControllerConfig['trigger'] => ({
    sameRange: false,
    l2From: 0,
    l2To: 100,
    r2From: 0,
    r2To: 100,
    l2Curve: defaultTriggerCurve('l2'),
    r2Curve: defaultTriggerCurve('r2'),
    effect: 'Adaptive resistance',
    intensity: 'Strong (Standard)',
    vibration: 'Medium'
  });

  const applyTriggerConfig = (trigger: EditableControllerConfig['trigger']) => {
    l2From = normalizeTriggerPercent(trigger.l2From);
    l2To = Math.max(l2From, normalizeTriggerPercent(trigger.l2To));
    r2From = normalizeTriggerPercent(trigger.r2From);
    r2To = Math.max(r2From, normalizeTriggerPercent(trigger.r2To));
    l2Curve = normalizeTriggerCurve(trigger.l2Curve, defaultTriggerCurve('l2'));
    r2Curve = normalizeTriggerCurve(trigger.r2Curve, defaultTriggerCurve('r2'));
    triggerEffect = trigger.effect;
    triggerIntensity = trigger.intensity;
    vibrationIntensity = trigger.vibration;
  };

  const resetTriggerCurvesToProfileDefaults = () => {
    applyTriggerConfig(baseForzaTriggerDefaults());
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
    const profileLabel = activeProfile?.scope === 'Built-in' ? activeProfile.name : 'Forza Horizon';
    setApplyMessage(`Reset trigger curves to ${profileLabel} defaults`);
  };

  const buildControllerConfig = (): EditableControllerConfig => {
    const base = currentControllerConfig
      ? editableConfigFromController(currentControllerConfig)
      : buildDefaultControllerConfig();

    return {
      ...base,
      trigger: {
        sameRange: false,
        l2From: normalizeTriggerPercent(l2From),
        l2To: Math.max(normalizeTriggerPercent(l2From), normalizeTriggerPercent(l2To)),
        r2From: normalizeTriggerPercent(r2From),
        r2To: Math.max(normalizeTriggerPercent(r2From), normalizeTriggerPercent(r2To)),
        l2Curve: normalizeTriggerCurve(l2Curve, defaultTriggerCurve('l2')),
        r2Curve: normalizeTriggerCurve(r2Curve, defaultTriggerCurve('r2')),
        effect: triggerEffect,
        intensity: triggerIntensity,
        vibration: vibrationIntensity
      },
      lightbar: {
        enabled: lightbarEnabled,
        color: lightbarColor,
        rpmColor,
        brightness: lightbarBrightness
      },
      forza: {
        effects: normalizeForzaEffects(forzaEffects)
      }
    };
  };

  const saveCurrentConfig = async () => {
    if (!controller) return false;
    try {
      currentControllerConfig = await saveControllerConfig(controller.id, buildControllerConfig());
      return true;
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to save config');
      return false;
    }
  };

  const syncLiveControllerConfig = async () => {
    if (!controller || !currentControllerConfig) return;
    if (liveConfigSyncInFlight) {
      liveConfigSyncQueued = true;
      return;
    }

    liveConfigSyncInFlight = true;
    liveConfigSyncQueued = false;
    try {
      currentControllerConfig = await saveControllerConfig(controller.id, buildControllerConfig());
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to update live controller config');
    } finally {
      liveConfigSyncInFlight = false;
      if (liveConfigSyncQueued) scheduleLiveControllerConfigSync();
    }
  };

  function scheduleLiveControllerConfigSync() {
    if (!controller || !currentControllerConfig) return;
    liveConfigSyncQueued = true;
    if (liveConfigSyncTimer !== undefined) window.clearTimeout(liveConfigSyncTimer);
    liveConfigSyncTimer = window.setTimeout(() => {
      liveConfigSyncTimer = undefined;
      void syncLiveControllerConfig();
    }, LIVE_CONFIG_SYNC_DEBOUNCE_MS);
  }

  const setTriggerEffect = (value: string) => {
    triggerEffect = value;
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  const setTriggerIntensity = (value: string) => {
    triggerIntensity = value;
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  const setVibrationIntensity = (value: string) => {
    vibrationIntensity = value;
    scheduleLiveControllerConfigSync();
  };

  const setLightbarEnabled = (enabled: boolean) => {
    lightbarEnabled = enabled;
    scheduleLiveControllerConfigSync();
  };

  const setLightbarBrightness = (value: number | string) => {
    lightbarBrightness = normalizeTriggerPercent(value);
    scheduleLiveControllerConfigSync();
  };
  const restoreDefaults = async () => {
    const selectedProfile = profiles.find(
      (profile) => profile.id === (snapshot?.profileResolution.selectedProfileId ?? activeProfileId)
    );
    const profileId = selectedProfile && selectedProfile.scope !== 'Built-in' ? 'forza-horizon' : selectedProfile?.id ?? activeProfileId;
    if (!profileId) {
      setApplyMessage('No active profile selected');
      return;
    }
    const profileName = profiles.find((profile) => profile.id === profileId)?.name ?? activeProfileName;

    try {
      await activateProfile(profileId);
      if (controller?.id) {
        configLoadedFor = '';
        await loadControllerConfig(controller.id);
      }
      await refresh();
      setApplyMessage(`Restored ${profileName}`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to restore active profile');
    }
  };

  const setApplyMessage = (message: string, tone: ToastTone = toastToneForMessage(message)) => {
    applyMessage = message;
    showToast(message, tone);
    window.setTimeout(() => {
      if (applyMessage === message) applyMessage = '';
    }, 2600);
  };

  const setAppSettingsMessage = (message: string, tone: ToastTone = toastToneForMessage(message)) => {
    appSettingsMessage = message;
    showToast(message, tone);
    window.setTimeout(() => {
      if (appSettingsMessage === message) appSettingsMessage = '';
    }, 4200);
  };

  const setProfileOverrideMessage = (message: string, tone: ToastTone = toastToneForMessage(message)) => {
    profileOverrideMessage = message;
    showToast(message, tone);
  };

  const updateLanAccess = async (nextListenOnAllInterfaces = !listenOnAllInterfaces) => {
    if (!snapshot || appSettingsBusy) return;
    if (nextListenOnAllInterfaces === listenOnAllInterfaces) return;
    appSettingsBusy = true;
    try {
      const updated = await saveAppSettings({ listenOnAllInterfaces: nextListenOnAllInterfaces });
      snapshot = {
        ...snapshot,
        appSettings: updated,
        status: { ...snapshot.status, bindAddress: updated.effectiveBindAddress }
      };
      setAppSettingsMessage(
        updated.restartRequired
          ? `Saved. Restart DSCC to use ${updated.desiredBindAddress}.`
          : `Web UI is listening on ${updated.effectiveBindAddress}.`,
        updated.restartRequired ? 'info' : 'success'
      );
      await refresh();
    } catch (caught) {
      setAppSettingsMessage(caught instanceof Error ? caught.message : 'Unable to update LAN access.', 'error');
    } finally {
      appSettingsBusy = false;
    }
  };

  const updateForzaGlyphOverride = async () => {
    if (!snapshot || appSettingsBusy) return;
    appSettingsBusy = true;
    try {
      const updated = await saveAppSettings({
        forzaPlaystationGlyphs: {
          enabled: !glyphOverrideEnabled,
          installPath: forzaGlyphs?.installPath ?? null
        }
      });
      snapshot = { ...snapshot, appSettings: updated };
      setAppSettingsMessage(updated.settings.forzaPlaystationGlyphs.lastMessage, 'success');
      await refresh();
    } catch (caught) {
      setAppSettingsMessage(caught instanceof Error ? caught.message : 'Unable to update controller button glyphs.', 'error');
    } finally {
      appSettingsBusy = false;
    }
  };

  const applyProfileOverride = async () => {
    if (!snapshot || !selectedOverrideProfileId) return;
    try {
      const resolution = await setProfileOverride({
        controllerId: controller?.id ?? null,
        gameId: profileContextGameId,
        profileId: selectedOverrideProfileId
      });
      snapshot = { ...snapshot, profileResolution: resolution };
      setProfileOverrideMessage(`${selectedOverrideProfile?.name ?? selectedOverrideProfileId} is now used for ${overrideScope}`, 'success');
      await refresh();
    } catch (caught) {
      setProfileOverrideMessage(caught instanceof Error ? caught.message : 'Unable to set profile override.', 'error');
    }
  };

  const returnToAutomaticProfile = async () => {
    if (!snapshot) return;
    const previousScope = overrideScope;
    try {
      const resolution = await clearProfileOverride({
        controllerId: controller?.id ?? null,
        gameId: profileContextGameId
      });
      setProfileGameSelectionMode(false);
      snapshot = { ...snapshot, profileResolution: resolution };
      setProfileOverrideMessage(`Automatic profile selection restored for ${previousScope}`, 'success');
      await refresh();
    } catch (caught) {
      setProfileOverrideMessage(caught instanceof Error ? caught.message : 'Unable to clear profile override.', 'error');
    }
  };

  const activateProfileById = async (id: string) => {
    // Optimistic UI update so rapid clicks feel instant: flip the active flag
    // locally and align the dropdown BEFORE the server round-trip resolves.
    if (snapshot) {
      snapshot = {
        ...snapshot,
        profiles: snapshot.profiles.map((profile) => ({ ...profile, active: profile.id === id }))
      };
    }
    selectedOverrideProfileId = id;
    lastSyncedActiveProfileId = id;
    try {
      await activateProfile(id);
      // After activation, reload the active controller's config so the
      // Forza effect table reflects the profile's preset values immediately.
      if (controller?.id) {
        configLoadedFor = '';
        await loadControllerConfig(controller.id);
      }
      await refresh();
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Failed to activate profile');
      // On failure, force a refresh so the UI snaps back to server truth.
      await refresh();
    }
  };

  const createProfileFromInput = async () => {
    const name = newProfileName.trim();
    if (!name) return;
    try {
      await createProfile(name);
      newProfileName = '';
      await refresh();
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Failed to create profile');
    }
  };

  const beginRenameSelectedProfile = () => {
    if (!selectedActionProfile || selectedActionProfile.scope === 'Built-in') return;
    renameProfileId = selectedActionProfile.id;
    renameProfileName = selectedActionProfile.name;
  };

  const cancelRenameProfile = () => {
    renameProfileId = '';
    renameProfileName = '';
  };

  const submitRenameProfile = async () => {
    const profile = profiles.find((item) => item.id === renameProfileId);
    const name = renameProfileName.trim();
    if (!profile || profile.scope === 'Built-in') {
      cancelRenameProfile();
      return;
    }
    if (!name) {
      setApplyMessage('Profile name cannot be empty', 'error');
      return;
    }
    if (name === profile.name) {
      cancelRenameProfile();
      return;
    }
    if (profiles.some((item) => item.id !== profile.id && item.name.trim().toLowerCase() === name.toLowerCase())) {
      setApplyMessage('A profile with that name already exists', 'error');
      return;
    }

    profileRenameBusy = true;
    try {
      const renamed = await renameProfile(profile.id, name);
      if (snapshot) {
        snapshot = {
          ...snapshot,
          profiles: snapshot.profiles.map((item) => (item.id === renamed.id ? { ...item, name: renamed.name } : item))
        };
      }
      cancelRenameProfile();
      await refresh();
      setApplyMessage(`Renamed profile to ${renamed.name}`, 'success');
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to rename profile', 'error');
      await refresh();
    } finally {
      profileRenameBusy = false;
    }
  };

  const handleRenameProfileKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submitRenameProfile();
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      cancelRenameProfile();
    }
  };

  const deleteProfileById = async (id: string, name: string) => {
    const fallbackProfileId =
      profiles.find((profile) => profile.id === 'forza-horizon')?.id ??
      profiles.find((profile) => profile.id !== id && profile.scope === 'Built-in')?.id ??
      profiles.find((profile) => profile.id !== id)?.id ??
      '';
    if (renameProfileId === id) cancelRenameProfile();
    profileFileBusy = true;
    try {
      if (snapshot) {
        snapshot = {
          ...snapshot,
          profiles: snapshot.profiles.filter((profile) => profile.id !== id)
        };
      }
      const response = await deleteProfile(id);
      await refresh();
      if (selectedOverrideProfileId === id) selectedOverrideProfileId = fallbackProfileId;
      setApplyMessage(response?.message ?? `Deleted ${name}`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Failed to delete profile');
      await refresh();
    } finally {
      profileFileBusy = false;
    }
  };

  const controllerModelText = (item: ControllerStatus | undefined) => {
    if (!item) return 'No DualSense Connected';
    if (item.family === 'Unknown Sony') return 'Unknown Sony Controller';
    return item.family;
  };

  const controllerConnectionText = (item: ControllerStatus | undefined) => {
    if (!item) return 'No controller detected';
    if (!item.connected) {
      if (item.diagnosticState === 'permission_denied') return 'Permission denied';
      if (item.diagnosticState === 'cannot_open') return 'Cannot open controller';
      return 'Controller disconnected';
    }
    return item.transport === 'Unknown' ? 'Connected' : item.transport;
  };

  const controllerBatteryReadable = (item: ControllerStatus | undefined) =>
    Boolean(item?.connected && typeof item.battery === 'number' && item.batteryState !== 'unknown');

  const controllerBatteryFillWidth = (item: ControllerStatus | undefined) =>
    controllerBatteryReadable(item) ? Math.max(2, Math.round(((item?.battery ?? 0) / 100) * 20)) : 0;

  const controllerBatteryText = (item: ControllerStatus | undefined) => {
    const battery = item?.battery;
    if (!item || typeof battery !== 'number' || item.batteryState === 'unknown') return '';
    if (item.batteryState === 'full') return `${battery}% / full`;
    if (item.batteryState === 'charging') return `${battery}% / charging`;
    return `${battery}% battery`;
  };

  const telemetryRateStatusText = (item: AppSnapshot['integrations'][number] | undefined) => {
    if (!item) return 'no active stream';
    if (item.state === 'running') return `${item.name} / live packets`;
    if (item.state === 'needs_setup') return `${item.name} / waiting for UDP`;
    if (item.state === 'ready') return `${item.name} / listening`;
    if (item.state === 'faulted') return `${item.name} / blocked`;
    return item.name;
  };

  const formatPlaytime = (minutes: number | null | undefined) => {
    if (minutes === null || minutes === undefined || !Number.isFinite(minutes) || minutes <= 0) return '';
    if (minutes < 60) return `${Math.round(minutes)}m played`;
    const hours = minutes / 60;
    return `${hours < 100 ? hours.toFixed(1) : Math.round(hours)}h played`;
  };

  const formatLastPlayed = (unixSeconds: number | null | undefined) => {
    if (!unixSeconds || !Number.isFinite(unixSeconds)) return '';
    const then = unixSeconds * 1000;
    const days = Math.max(0, Math.floor((Date.now() - then) / 86_400_000));
    if (days === 0) return 'played today';
    if (days === 1) return 'played yesterday';
    if (days < 14) return `played ${days}d ago`;
    return `played ${new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' }).format(new Date(then))}`;
  };

  const achievementText = (game: SupportedGame) => {
    const achievements = game.stats?.achievements;
    if (!achievements || achievements.total <= 0) return '';
    return `${achievements.unlocked}/${achievements.total} achievements`;
  };

  const gameTileStatus = (game: SupportedGame) => {
    if (game.running) return 'running';
    if (game.installed) return 'installed';
    return 'not installed';
  };

  const gameDetectionStatusText = (detection: GameDetection | undefined) => {
    if (!detection?.activeGameId && !detection?.activeGameName) return '';

    const source = detection.source.split(':', 1)[0];
    switch (source) {
      case 'process_scan':
        return 'Running on this PC';
      case 'process_scan_disabled':
        return 'Game detection paused';
      case 'process_scan_unavailable':
        return 'Game detection unavailable';
      case 'built_in':
        return 'Built-in game support';
      case 'none':
      case 'unknown':
      case '':
        return 'Detected';
      default:
        return source.replaceAll('_', ' ');
    }
  };

  const gameStatBadges = (game: SupportedGame) =>
    [formatPlaytime(game.stats?.playtimeMinutes), achievementText(game)].filter(Boolean);

  const sanitizeFileName = (value: string) =>
    value
      .trim()
      .replace(/[^a-z0-9._-]+/gi, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 80) || 'profile';

  const profileSlug = (value: string) =>
    value
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '');

  const uniqueProfileName = (baseName: string) => {
    const existingNames = new Set(profiles.map((profile) => profile.name.toLowerCase()));
    let candidate = baseName.trim() || 'Imported profile';
    if (!existingNames.has(candidate.toLowerCase()) && !profiles.some((profile) => profile.id === profileSlug(candidate))) {
      return candidate;
    }
    const root = candidate.replace(/\s+copy(?:\s+\d+)?$/i, '').trim() || 'Imported profile';
    for (let index = 2; index < 1000; index += 1) {
      candidate = `${root} copy ${index}`;
      if (!existingNames.has(candidate.toLowerCase()) && !profiles.some((profile) => profile.id === profileSlug(candidate))) {
        return candidate;
      }
    }
    return `${root} copy ${Date.now()}`;
  };

  const profileImportPayload = (value: unknown) => {
    if (!value || typeof value !== 'object') throw new Error('Profile file is not valid JSON.');
    const profile = value as Partial<ExportedProfile>;
    const name = typeof profile.name === 'string' ? profile.name.trim() : '';
    if (!name) throw new Error('Profile file is missing a profile name.');

    const id = typeof profile.id === 'string' ? profile.id.trim() : '';
    const existingIds = new Set(profiles.map((item) => item.id));
    const idAvailable = Boolean(id) && !existingIds.has(id);
    return {
      id: idAvailable ? id : undefined,
      name: idAvailable ? name : uniqueProfileName(`${name} copy`),
      config: profile.config ?? undefined
    };
  };

  const exportSelectedProfile = async () => {
    const profileId = selectedOverrideProfileId || activeProfileId;
    if (!profileId || profileFileBusy) {
      if (!profileId) setApplyMessage('Select a profile to export');
      return;
    }
    profileFileBusy = true;
    try {
      const exported = await exportProfile(profileId);
      const body = JSON.stringify(exported, null, 2);
      const url = URL.createObjectURL(new Blob([body], { type: 'application/json' }));
      const link = document.createElement('a');
      link.href = url;
      link.download = `${sanitizeFileName(exported.name)}.dscc-profile.json`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      setApplyMessage(`Exported ${exported.name}`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to export profile');
    } finally {
      profileFileBusy = false;
    }
  };

  const requestProfileImport = () => {
    if (!profileFileBusy) profileImportInput?.click();
  };

  const handleProfileImport = async (event: Event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file || profileFileBusy) return;

    profileFileBusy = true;
    try {
      const payload = profileImportPayload(JSON.parse(await file.text()));
      const imported = await importProfile(payload);
      selectedOverrideProfileId = imported.id;
      await refresh();
      setApplyMessage(`Imported ${imported.name}`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to import profile');
    } finally {
      profileFileBusy = false;
    }
  };

  function stopTriggerInputPolling() {
    if (triggerInputPollTimer !== undefined) {
      window.clearInterval(triggerInputPollTimer);
      triggerInputPollTimer = undefined;
    }
    triggerInputBusy = false;
    controllerInputFresh = false;
    l2ControllerPress = 0;
    r2ControllerPress = 0;
  }

  function clearBaseFeelTestTimers() {
    if (baseFeelTestTimer !== undefined) {
      window.clearTimeout(baseFeelTestTimer);
      baseFeelTestTimer = undefined;
    }
    if (baseFeelTestRefreshTimer !== undefined) {
      window.clearTimeout(baseFeelTestRefreshTimer);
      baseFeelTestRefreshTimer = undefined;
    }
    baseFeelTestRefreshQueued = false;
  }

  function markBaseFeelTestInactive() {
    baseFeelTestActive = false;
    baseFeelTestBusy = false;
    clearBaseFeelTestTimers();
    stopTriggerInputPolling();
  }

  async function pollTriggerInput() {
    if (triggerInputBusy || !controller?.id || typeof document === 'undefined' || document.hidden) return;
    triggerInputBusy = true;
    try {
      const input = await getControllerInput(controller?.id);
      if (input.available) {
        const wasFresh = controllerInputFresh;
        const previousL2 = l2ControllerPress;
        const previousR2 = r2ControllerPress;
        const nextL2 = clampUnit(input.l2);
        const nextR2 = clampUnit(input.r2);
        l2ControllerPress = nextL2;
        r2ControllerPress = nextR2;
        controllerInputFresh = true;
        const triggerMoved = Math.abs(nextL2 - previousL2) >= 0.01 || Math.abs(nextR2 - previousR2) >= 0.01;
        if (baseFeelTestActive && (!wasFresh || triggerMoved)) {
          scheduleBaseFeelTestRefresh();
        }
      } else {
        controllerInputFresh = false;
      }
    } catch {
      controllerInputFresh = false;
    } finally {
      triggerInputBusy = false;
    }
  }

  function startTriggerInputPolling() {
    if (!controller?.id || typeof document === 'undefined' || document.hidden) return;
    void pollTriggerInput();
    if (triggerInputPollTimer !== undefined) return;
    triggerInputPollTimer = window.setInterval(() => void pollTriggerInput(), TRIGGER_INPUT_POLL_INTERVAL_MS);
  }

  function armBaseFeelTestTimer() {
    if (baseFeelTestTimer !== undefined) window.clearTimeout(baseFeelTestTimer);
    baseFeelTestTimer = window.setTimeout(() => {
      markBaseFeelTestInactive();
    }, BASE_FEEL_TEST_DURATION_MS);
  }

  function scheduleBaseFeelTestRefresh() {
    if (!baseFeelTestActive) return;
    baseFeelTestRefreshQueued = true;
    if (baseFeelTestRefreshInFlight || baseFeelTestRefreshTimer !== undefined) return;
    const elapsed = performance.now() - lastBaseFeelTestRefreshAt;
    const waitMs = Math.max(0, BASE_FEEL_TEST_REFRESH_INTERVAL_MS - elapsed);
    baseFeelTestRefreshTimer = window.setTimeout(() => {
      baseFeelTestRefreshTimer = undefined;
      void flushBaseFeelTestRefresh();
    }, waitMs);
  }

  async function flushBaseFeelTestRefresh() {
    if (!baseFeelTestActive || baseFeelTestRefreshInFlight) return;
    baseFeelTestRefreshQueued = false;
    baseFeelTestRefreshInFlight = true;
    lastBaseFeelTestRefreshAt = performance.now();
    try {
      await startBaseFeelTest(true);
    } finally {
      baseFeelTestRefreshInFlight = false;
      if (baseFeelTestRefreshQueued && baseFeelTestActive) scheduleBaseFeelTestRefresh();
    }
  }

  const baseFeelTestRequest = (): EffectTestRequest => ({
    target: 'base_feel',
    mode: 'hold',
    intensity: 100,
    durationMs: BASE_FEEL_TEST_DURATION_MS,
    l2Position: controllerInputFresh ? l2ControllerPress : undefined,
    r2Position: controllerInputFresh ? r2ControllerPress : undefined,
    trigger: buildControllerConfig().trigger
  });

  const startBaseFeelTest = async (refreshOnly = false) => {
    if (!snapshot) return;
    if (!refreshOnly) baseFeelTestBusy = true;
    try {
      if (!refreshOnly) await pollTriggerInput();
      const result = await runEffectTest(baseFeelTestRequest(), controller?.id);

      snapshot = {
        ...snapshot,
        effectState: {
          ...snapshot.effectState,
          output: result.output
        }
      };
      baseFeelTestActive = true;
      startTriggerInputPolling();
      armBaseFeelTestTimer();
      if (!refreshOnly) {
        setApplyMessage('Base feel test is live. Squeeze L2/R2 while adjusting curves; hardware output now follows the same curve shown in the graph.');
      }
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Base feel test failed');
      markBaseFeelTestInactive();
    } finally {
      if (!refreshOnly) baseFeelTestBusy = false;
    }
  };

  const stopBaseFeelTest = async () => {
    if (!snapshot) {
      markBaseFeelTestInactive();
      return;
    }
    baseFeelTestBusy = true;
    if (baseFeelTestRefreshTimer !== undefined) {
      window.clearTimeout(baseFeelTestRefreshTimer);
      baseFeelTestRefreshTimer = undefined;
    }
    try {
      const result = await runEffectTest(
        {
          target: 'base_feel',
          mode: 'off',
          intensity: 0,
          durationMs: 100
        },
        controller?.id
      );
      snapshot = {
        ...snapshot,
        effectState: {
          ...snapshot.effectState,
          output: result.output
        }
      };
      setApplyMessage('Base feel test stopped');
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to stop Base feel test');
    } finally {
      markBaseFeelTestInactive();
    }
  };

  const toggleBaseFeelTest = async () => {
    if (baseFeelTestBusy) return;
    if (baseFeelTestActive) {
      await stopBaseFeelTest();
    } else {
      await startBaseFeelTest();
    }
  };

  const saveActiveProfile = async () => {
    if (!activeProfileId || profileSaveBusy) {
      if (!activeProfileId) setApplyMessage('No active profile selected');
      return;
    }

    profileSaveBusy = true;
    try {
      let targetProfile = activeProfile;
      let preservingStockProfile = false;
      if (targetProfile?.scope === 'Built-in') {
        const name = uniqueProfileName(`${targetProfile.name} custom`);
        targetProfile = await createProfile(name);
        preservingStockProfile = true;
      }
      if (!targetProfile) throw new Error('No profile selected');

      const config = buildControllerConfig();
      if (controller) {
        currentControllerConfig = await saveControllerConfig(controller.id, config);
      }
      const response = await saveProfileConfig(targetProfile.id, config);
      profileSaveBaselineSignature = profileConfigSignature(config);
      if (targetProfile.id !== activeProfileId) {
        await activateProfile(targetProfile.id);
        selectedOverrideProfileId = targetProfile.id;
        lastSyncedActiveProfileId = targetProfile.id;
      }
      await refresh();
      setApplyMessage(
        preservingStockProfile
          ? `Saved ${targetProfile.name}; stock ${activeProfile?.name ?? activeProfileName} preserved`
          : response.message || `Saved ${targetProfile.name}`
      );
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to save profile');
    } finally {
      profileSaveBusy = false;
    }
  };

  const previewLightbarColor = async (color: string, label: string) => {
    // /test-effect takes parameters in the request body, so preview first
    // and only persist the config if the preview is accepted by the agent.
    if (!snapshot) return;

    const intensity = lightbarEnabled ? lightbarBrightness : 0;
    try {
      const result = await runEffectTest(
        {
          target: 'lightbar',
          mode: color,
          intensity,
          durationMs: 650
        },
        controller?.id
      );

      snapshot = {
        ...snapshot,
        effectState: {
          ...snapshot.effectState,
          output: result.output
        }
      };
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : `${label} preview failed`);
      return;
    }

    const saved = await saveCurrentConfig();
    if (!saved) return;
    await refresh();
    setApplyMessage(`${label} ${color} previewed`);
  };

  const previewLightbar = async () => previewLightbarColor(lightbarColor, 'Lightbar');
  const previewRpmColor = async () => previewLightbarColor(rpmColor, 'Max RPM');

  const startFallbackPolling = () => {
    if (typeof window === 'undefined' || fallbackPollTimer !== undefined) return;
    if (!document.hidden) void refresh();
    if (typeof window.setInterval !== 'function') return;
    fallbackPollTimer = window.setInterval(() => {
      if (!document.hidden) void refresh();
    }, FALLBACK_POLL_INTERVAL_MS);
  };

  const handleVisibilityChange = () => {
    if (document.hidden) {
      stopTriggerInputPolling();
      return;
    }
    startTriggerInputPolling();
    if (!pendingVisibilityRefresh) return;
    pendingVisibilityRefresh = false;
    void refresh();
  };

  const handleHashChange = () => {
    activeView = appViewFromHash();
  };

  const startAppRuntime = () => {
    if (typeof window === 'undefined' || appRuntimeStarted) return;
    appRuntimeStarted = true;
    activeView = appViewFromHash();
    void refresh();
    stopSnapshotSocket = connectAppSnapshotSocket({
      onSnapshot: applySnapshot,
      onInvalidate: scheduleRefresh,
      onUnavailable: startFallbackPolling,
      onClosed: startFallbackPolling
    });
    document.addEventListener('mousedown', handleColorDocClick);
    document.addEventListener('keydown', handleColorKey);
    document.addEventListener('visibilitychange', handleVisibilityChange);
    window.addEventListener('hashchange', handleHashChange);
  };

  const stopAppRuntime = () => {
    if (typeof window === 'undefined' || !appRuntimeStarted) return;
    appRuntimeStarted = false;
    stopSnapshotSocket?.();
    stopSnapshotSocket = undefined;
    if (fallbackPollTimer !== undefined) window.clearInterval(fallbackPollTimer);
    fallbackPollTimer = undefined;
    if (refreshDebounceTimer !== undefined) window.clearTimeout(refreshDebounceTimer);
    refreshDebounceTimer = undefined;
    if (liveConfigSyncTimer !== undefined) window.clearTimeout(liveConfigSyncTimer);
    liveConfigSyncTimer = undefined;
    clearBaseFeelTestTimers();
    stopTriggerInputPolling();
    document.removeEventListener('mousedown', handleColorDocClick);
    document.removeEventListener('keydown', handleColorKey);
    document.removeEventListener('visibilitychange', handleVisibilityChange);
    window.removeEventListener('hashchange', handleHashChange);
  };

  onMount(() => {
    startAppRuntime();
    return stopAppRuntime;
  });

  $: if (controller?.id) {
    startTriggerInputPolling();
  } else {
    stopTriggerInputPolling();
  }

  $: if (controller?.id && controller.id !== configLoadedFor) {
    void loadControllerConfig(controller.id);
  }
</script>

<main class="ops-shell">
  {#if loading}
    <section class="ops-state">
      <RefreshCw class="spin" size={24} />
      <strong>Initializing command surface</strong>
      <span>Synchronizing controller, profile, and telemetry state</span>
    </section>
  {:else if error}
    <section class="ops-state">
      <Cable size={26} />
      <strong>Agent unavailable</strong>
      <span>{error}</span>
      <button class="solid-action compact" type="button" onclick={refresh}>Retry</button>
    </section>
  {:else if snapshot}
    <header class="dm-hud" aria-label="Global command state">
      <div class="dm-hardware-state">
        <span class="dm-controller-glyph" aria-hidden="true"></span>
        <div>
          <h1>{controllerHeaderName}</h1>
          <p>
            <span>{controllerHeaderMeta}</span>
            {#if controllerHeaderBatteryReadable}
              <span class="dm-battery-pill">
                <svg class="dm-battery" viewBox="0 0 32 16" aria-hidden="true">
                  <rect x="1" y="3" width="26" height="10" rx="2" />
                  <path d="M28 6h2.5v4H28z" />
                  <rect class="dm-battery-fill" x="4" y="5.5" width={controllerBatteryFillWidth(controller)} height="5" rx="1" />
                </svg>
                <span>{controllerBatteryText(controller)}</span>
              </span>
            {/if}
          </p>
        </div>
      </div>

      <nav class="dm-view-nav" aria-label="Command center views">
        {#each appViews as view}
          <button
            class:active={activeView === view.id}
            type="button"
            aria-current={activeView === view.id ? 'page' : undefined}
            onclick={() => navigateToView(view.id)}
          >
            {view.label}
          </button>
        {/each}
      </nav>

      <div class="dm-system-readout" title={integration?.setupHint ?? telemetryRateDetail}>
        <span>Telemetry Rate</span>
        <strong>{telemetryRateText}</strong>
        <small>{telemetryRateDetail}</small>
      </div>
    </header>

    <section class="dm-steam-ribbon" aria-label="Steam game context and production controls">
      <div class="dm-steam-identity">
        {#if steamContextArt}
          <img src={steamContextArt} alt="" loading="lazy" aria-hidden="true" />
        {/if}
        <div>
          <span>Steam Context</span>
          <strong>{steamContextGame?.name ?? 'No supported game selected'}</strong>
          <p>{steamContextMeta}</p>
        </div>
      </div>

      {#if supportedGames.length}
        <div class="dm-steam-library" aria-label="Supported Steam games">
          {#each supportedGames.slice(0, 4) as game}
            {@const tileArt = gameArtwork(game, 'icon') ?? gameArtwork(game, 'banner') ?? gameArtwork(game, 'capsule')}
            <button
              type="button"
              class:active={game.gameId === steamContextGame?.gameId}
              class:running={game.running}
              aria-pressed={game.gameId === steamContextGame?.gameId}
              onclick={() => selectProfileGame(game)}
            >
              {#if tileArt}<img src={tileArt} alt="" loading="lazy" />{/if}
              <span>{game.name}</span>
              <code>{game.running ? 'LIVE' : game.installed ? 'READY' : 'LIB'}</code>
            </button>
          {/each}
        </div>
      {/if}

      <div class="dm-system-toggles" aria-label="Production system controls">
        <Tooltip block text="Local keeps the web UI bound to this PC. LAN exposes it on your network so you can tune from another device; a restart may be required after changing the bind address." side="bottom" align="end">
          <div class="dm-location-line">
            <label>
              <span>Web UI Location</span>
              <select
                value={listenOnAllInterfaces ? 'lan' : 'local'}
                disabled={appSettingsBusy}
                aria-label="Web UI location"
                onchange={(event) => void updateLanAccess(event.currentTarget.value === 'lan')}
              >
                <option value="local">Local Only</option>
                <option value="lan">LAN Access</option>
              </select>
              <small>{lanRestartRequired ? `restart -> ${appSettings?.desiredBindAddress}` : status?.bindAddress}</small>
            </label>
          </div>
        </Tooltip>
        <Tooltip block text="Installs or restores PlayStation-style button glyphs for supported games. DSCC keeps backups so the game can be returned to its default glyph files." side="bottom" align="end">
          <div class="dm-switch-line dm-glyph-switch">
            <div>
              <span>Controller Glyphs</span>
              <strong>{glyphOverrideEnabled ? 'PlayStation Icons' : 'Game Default'}</strong>
              <small>{forzaGlyphs?.lastStatus ?? glyphInstallPath}</small>
            </div>
            <button
              class:active={glyphOverrideEnabled}
              class="dm-toggle"
              type="button"
              disabled={appSettingsBusy}
              aria-label="Toggle PlayStation controller button glyphs"
              aria-pressed={glyphOverrideEnabled}
              onclick={updateForzaGlyphOverride}
            ><span></span></button>
          </div>
        </Tooltip>
      </div>
    </section>

    {#if showPartialErrorBanner}
      <aside class="ops-warning dm-warning" role="status" aria-live="polite">
        <span>Partial agent data: {partialErrors.map((entry) => entry.endpoint).join(', ')} unavailable.</span>
        <button type="button" aria-label="Dismiss partial agent data notice" onclick={dismissPartialErrors}>dismiss</button>
      </aside>
    {/if}
    <section
      class:dm-view-hidden={activeView !== 'haptics'}
      class="dm-deck"
      aria-label="Adaptive triggers and haptics"
      aria-hidden={activeView !== 'haptics'}
    >
      <section class="dm-physics" aria-label="Actuation curve tuning">
        <div class="dm-section-head">
          <div>
            <span>Actuation Engine</span>
            <h2>Trigger Curves</h2>
          </div>
          <div class="dm-section-actions">
            <Tooltip text="Restores L2/R2 range, curve, base force, and body feel to the active profile defaults. Custom profiles reset to the stock Forza Horizon curve." side="top" align="end">
              <button
                class="dm-test-button"
                type="button"
                disabled={!snapshot}
                onclick={resetTriggerCurvesToProfileDefaults}
              >
                <RotateCcw size={14} /> Reset
              </button>
            </Tooltip>
            <Tooltip text="Holds the current L2 and R2 base resistance on the controller without needing a game." side="top" align="end">
              <button
                class:active={baseFeelTestActive}
                class="dm-test-button"
                type="button"
                aria-pressed={baseFeelTestActive}
                disabled={baseFeelTestBusy || !snapshot}
                onclick={() => void toggleBaseFeelTest()}
              >
                {baseFeelTestActive ? 'Testing Actuation' : 'Test Actuation'}
              </button>
            </Tooltip>
          </div>
        </div>

        <div class="dm-curve-stack">
          <article class="dm-curve-module" aria-label="L2 brake actuation curve">
            <div class="dm-module-title">
              <div>
                <span>L2</span>
                <strong>Brake Pressure</strong>
              </div>
              <code>{triggerPressLabel(l2LivePress)}</code>
            </div>
            <div
              class="dm-curve-frame"
              role="img"
              aria-label="L2 actuation response curve with live input crosshair"
              onpointerdown={(event) => handleCurvePointer(event, 'l2')}
              onpointermove={(event) => updateCurveHover(event, 'l2')}
              onpointerleave={() => clearCurveHover('l2')}
            >
              <svg class="dm-trigger-curve" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
                <defs>
                  <filter id="dm-blue-glow" x="-20%" y="-20%" width="140%" height="140%">
                    <feGaussianBlur stdDeviation="1.1" result="blur" />
                    <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
                  </filter>
                </defs>
                <path class="curve-grid" d="M 0 75 H 100 M 0 50 H 100 M 0 25 H 100 M 25 0 V 100 M 50 0 V 100 M 75 0 V 100" />
                <path class="curve-linear" d="M 0 100 L 100 0" />
                <rect class="curve-range-fill" x={l2CurveView.rangeStart} y="96" width={l2CurveView.rangeWidth} height="2.5" rx="1.25" />
                <line class="curve-range-edge" x1={l2CurveView.rangeStart} y1="0" x2={l2CurveView.rangeStart} y2="100" />
                <line class="curve-range-edge" x1={l2CurveView.rangeEnd} y1="0" x2={l2CurveView.rangeEnd} y2="100" />
                <path class="curve-force" d={l2CurveView.path} />
                {#if curveHover?.side === 'l2'}
                  <line class="curve-crosshair" x1={curveHover.left.toFixed(2)} y1="0" x2={curveHover.left.toFixed(2)} y2="100" />
                {/if}
                {#if showTriggerPress('l2', l2LivePress)}
                  <line class="curve-live" x1={l2CurveView.liveX} y1="0" x2={l2CurveView.liveX} y2="100" />
                  <circle class="curve-live-dot" cx={l2CurveView.liveX} cy={l2CurveView.liveY} r="1.75" />
                {/if}
              </svg>
              {#if curveHover?.side === 'l2'}
                <div class="dm-curve-tooltip" style="left:{curveHover.left}%;top:{curveHover.top}%;">
                  <code>IN {Math.round(curveHover.x * 100).toString().padStart(3, '0')}</code>
                  <code>OUT {Math.round(curveHover.y * 100).toString().padStart(3, '0')}</code>
                </div>
              {/if}
            </div>
            <div class="dm-slider-bank">
              <Tooltip block text={triggerRangeTooltip('L2', 'from', l2From)} side="top" align="start">
                <label class="dm-slider-row">
                  <span>Start</span>
                  <input class="dm-range" style="--value:{l2From}%" value={l2From} max={l2To} min="0" type="range" oninput={(event) => setTriggerRangeValue('l2', 'from', event.currentTarget.valueAsNumber)} />
                  <code>{l2From.toString().padStart(3, '0')}</code>
                </label>
              </Tooltip>
              <Tooltip block text={triggerRangeTooltip('L2', 'to', l2To)} side="top" align="start">
                <label class="dm-slider-row">
                  <span>End</span>
                  <input class="dm-range" style="--value:{l2To}%" value={l2To} max="100" min={l2From} type="range" oninput={(event) => setTriggerRangeValue('l2', 'to', event.currentTarget.valueAsNumber)} />
                  <code>{l2To.toString().padStart(3, '0')}</code>
                </label>
              </Tooltip>
              <Tooltip block text={triggerCurveTooltip('L2', l2Curve)} side="top" align="start">
                <label class="dm-slider-row">
                  <span>Curve</span>
                  <input class="dm-range" style="--value:{((l2Curve - 0.5) / 3) * 100}%" value={l2Curve} max="3.5" min="0.5" step="0.05" type="range" oninput={(event) => setTriggerCurveValue('l2', event.currentTarget.valueAsNumber)} />
                  <code>{l2Curve.toFixed(2)}</code>
                </label>
              </Tooltip>
            </div>
          </article>

          <article class="dm-curve-module" aria-label="R2 throttle actuation curve">
            <div class="dm-module-title">
              <div>
                <span>R2</span>
                <strong>Throttle Load</strong>
              </div>
              <code>{triggerPressLabel(r2LivePress)}</code>
            </div>
            <div
              class="dm-curve-frame"
              role="img"
              aria-label="R2 actuation response curve with live input crosshair"
              onpointerdown={(event) => handleCurvePointer(event, 'r2')}
              onpointermove={(event) => updateCurveHover(event, 'r2')}
              onpointerleave={() => clearCurveHover('r2')}
            >
              <svg class="dm-trigger-curve" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
                <path class="curve-grid" d="M 0 75 H 100 M 0 50 H 100 M 0 25 H 100 M 25 0 V 100 M 50 0 V 100 M 75 0 V 100" />
                <path class="curve-linear" d="M 0 100 L 100 0" />
                <rect class="curve-range-fill" x={r2CurveView.rangeStart} y="96" width={r2CurveView.rangeWidth} height="2.5" rx="1.25" />
                <line class="curve-range-edge" x1={r2CurveView.rangeStart} y1="0" x2={r2CurveView.rangeStart} y2="100" />
                <line class="curve-range-edge" x1={r2CurveView.rangeEnd} y1="0" x2={r2CurveView.rangeEnd} y2="100" />
                <path class="curve-force" d={r2CurveView.path} />
                {#if curveHover?.side === 'r2'}
                  <line class="curve-crosshair" x1={curveHover.left.toFixed(2)} y1="0" x2={curveHover.left.toFixed(2)} y2="100" />
                {/if}
                {#if showTriggerPress('r2', r2LivePress)}
                  <line class="curve-live" x1={r2CurveView.liveX} y1="0" x2={r2CurveView.liveX} y2="100" />
                  <circle class="curve-live-dot" cx={r2CurveView.liveX} cy={r2CurveView.liveY} r="1.75" />
                {/if}
              </svg>
              {#if curveHover?.side === 'r2'}
                <div class="dm-curve-tooltip" style="left:{curveHover.left}%;top:{curveHover.top}%;">
                  <code>IN {Math.round(curveHover.x * 100).toString().padStart(3, '0')}</code>
                  <code>OUT {Math.round(curveHover.y * 100).toString().padStart(3, '0')}</code>
                </div>
              {/if}
            </div>
            <div class="dm-slider-bank">
              <Tooltip block text={triggerRangeTooltip('R2', 'from', r2From)} side="top" align="start">
                <label class="dm-slider-row">
                  <span>Start</span>
                  <input class="dm-range" style="--value:{r2From}%" value={r2From} max={r2To} min="0" type="range" oninput={(event) => setTriggerRangeValue('r2', 'from', event.currentTarget.valueAsNumber)} />
                  <code>{r2From.toString().padStart(3, '0')}</code>
                </label>
              </Tooltip>
              <Tooltip block text={triggerRangeTooltip('R2', 'to', r2To)} side="top" align="start">
                <label class="dm-slider-row">
                  <span>End</span>
                  <input class="dm-range" style="--value:{r2To}%" value={r2To} max="100" min={r2From} type="range" oninput={(event) => setTriggerRangeValue('r2', 'to', event.currentTarget.valueAsNumber)} />
                  <code>{r2To.toString().padStart(3, '0')}</code>
                </label>
              </Tooltip>
              <Tooltip block text={triggerCurveTooltip('R2', r2Curve)} side="top" align="start">
                <label class="dm-slider-row">
                  <span>Curve</span>
                  <input class="dm-range" style="--value:{((r2Curve - 0.5) / 3) * 100}%" value={r2Curve} max="3.5" min="0.5" step="0.05" type="range" oninput={(event) => setTriggerCurveValue('r2', event.currentTarget.valueAsNumber)} />
                  <code>{r2Curve.toFixed(2)}</code>
                </label>
              </Tooltip>
            </div>
          </article>
        </div>

        <div class="dm-parameter-strip" aria-label="Base force and light routing">
          <Tooltip block text={triggerEffectHelp[triggerEffect] ?? 'Selects the base adaptive trigger behavior.'} side="top" align="start">
            <label>
              <span>Mode</span>
              <select value={triggerEffect} onchange={(event) => setTriggerEffect(event.currentTarget.value)}>
                <option>Adaptive resistance</option><option>Pulse</option><option>Wall</option><option>Off</option>
              </select>
            </label>
          </Tooltip>
          <Tooltip block text={triggerStrengthHelp[triggerIntensity] ?? 'Controls the base trigger force multiplier.'} side="top" align="start">
            <label>
              <span>Force</span>
              <select value={triggerIntensity} onchange={(event) => setTriggerIntensity(event.currentTarget.value)}>
                <option>Off</option><option>Weak</option><option>Medium</option><option>Strong (Standard)</option>
              </select>
            </label>
          </Tooltip>
          <Tooltip block text={vibrationHelp[vibrationIntensity] ?? 'Controls the body rumble multiplier.'} side="top" align="start">
            <label>
              <span>Body</span>
              <select value={vibrationIntensity} onchange={(event) => setVibrationIntensity(event.currentTarget.value)}>
                <option>Off</option><option>Low</option><option>Medium</option><option>High</option>
              </select>
            </label>
          </Tooltip>
        </div>
      </section>

      <aside class="dm-routing" aria-label="Telemetry haptic routing">
        <div class="dm-section-head compact">
          <div>
            <span>Haptic Routing</span>
            <h2>Telemetry Stream</h2>
          </div>
          <div class="dm-effects-count">
            <code>{enabledForzaEffectCount}/{forzaEffectMetas.length}</code>
            <button class:active={allForzaEffectsEnabled} class="dm-toggle" type="button" aria-label="Toggle all effects" aria-pressed={allForzaEffectsEnabled} onclick={toggleAllForzaEffects}><span></span></button>
          </div>
        </div>

        <div class="dm-channel-list">
          {#each forzaEffectMetas as meta (meta.id)}
            {@const tuning = forzaEffectsById.get(meta.id) ?? forzaEffect(meta.id)}
            {@const status = effectStatusById.get(meta.id)}
            <article
              class:active={status?.state === 'active'}
              class:disabled={!tuning.enabled || status?.state === 'disabled'}
              class="dm-channel-strip"
            >
              <Tooltip text={(tuning.enabled ? 'Disable ' : 'Enable ') + meta.label + '.'} side="right" align="start">
                <button
                  class:active={tuning.enabled}
                  class="dm-toggle"
                  type="button"
                  aria-label={meta.label + ' enabled'}
                  aria-pressed={tuning.enabled}
                  onclick={() => updateForzaEffect(meta.id, { enabled: !tuning.enabled })}
                ><span></span></button>
              </Tooltip>
              <Tooltip block text={meta.help} side="bottom" align="start">
                <div class="dm-channel-name">
                  <strong>{meta.label}</strong>
                </div>
              </Tooltip>
              <Tooltip block text={intensityTooltip(meta, tuning.intensity)} side="bottom" align="center">
                <label class="dm-fader">
                  <input
                    class="dm-range"
                    style="--value:{forzaIntensityPercent(tuning.intensity)}%"
                    aria-label={meta.label + ' intensity slider'}
                    max="100"
                    min="0"
                    type="range"
                    value={forzaIntensityPercent(tuning.intensity)}
                    oninput={(event) => updateForzaEffect(meta.id, { intensity: forzaIntensityFromPercent(event.currentTarget.valueAsNumber) })}
                  />
                  <input
                    class="dm-fader-value"
                    aria-label={meta.label + ' intensity value'}
                    max="100"
                    min="0"
                    step="1"
                    type="number"
                    value={forzaIntensityPercent(tuning.intensity)}
                    oninput={(event) => updateForzaEffect(meta.id, { intensity: forzaIntensityFromPercent(event.currentTarget.value) })}
                  />
                </label>
              </Tooltip>
              <Tooltip block text={routeTooltip(tuning.route)} side="bottom" align="end">
                <label class="dm-route-select-wrap">
                  <span>Route</span>
                  <select
                    class="dm-route-select"
                    aria-label={meta.label + ' route'}
                    value={tuning.route}
                    onchange={(event) => updateForzaEffect(meta.id, { route: event.currentTarget.value as ForzaEffectRoute })}
                  >
                    {#each forzaRoutes as route}
                      <option value={route.value}>{route.label}</option>
                    {/each}
                  </select>
                </label>
              </Tooltip>
            </article>
          {/each}
        </div>

        <div class="dm-rgb-console" aria-label="RGB output controls">
          <div class="dm-console-title">
            <span>RGB Controls</span>
            <strong>Lightbar & RPM</strong>
          </div>
          <div class="dm-led-controls">
            <div class="dm-led-row">
              <span>LED</span>
              <div class="ops-lightbar-popover-wrap">
                <button
                  bind:this={lightbarPillEl}
                  type="button"
                  class="dm-color-pill ops-lightbar-preview"
                  class:on={lightbarEnabled}
                  class:disabled={!lightbarEnabled}
                  class:open={pickerOpen && pickerTarget === 'lightbar'}
                  aria-label="Lightbar color"
                  aria-expanded={pickerOpen && pickerTarget === 'lightbar'}
                  aria-haspopup="dialog"
                  style="--lb-color: {lightbarColor}; --lb-alpha: {lightbarEnabled ? lightbarBrightness / 100 : 0};"
                  onclick={() => togglePicker('lightbar')}
                ><span class="ops-lightbar-glow" aria-hidden="true"></span></button>
                {#if pickerOpen && pickerTarget === 'lightbar'}
                  <div bind:this={pickerEl} class="ops-color-popover" role="dialog" aria-label="Lightbar color picker">
                    <div class="ops-color-sv" style="background-color: hsl({pickerHue}, 100%, 50%);" role="slider" tabindex="0" aria-label="Saturation and brightness" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(pickerVal * 100)} aria-valuetext="Saturation {Math.round(pickerSat * 100)}%, brightness {Math.round(pickerVal * 100)}%" onpointerdown={handleSvPointer} onkeydown={handleSvKeydown}>
                      <div class="ops-color-sv-overlay"></div>
                      <div class="ops-color-sv-cursor" style="left: {pickerSat * 100}%; top: {(1 - pickerVal) * 100}%; background: {pickerHex};"></div>
                    </div>
                    <input type="range" min="0" max="360" value={pickerHue} oninput={handleHueInput} class="ops-color-hue" aria-label="Hue" />
                    <div class="ops-color-row">
                      <span class="ops-color-row-swatch" style="background: {pickerHex};"></span>
                      <input type="text" bind:value={pickerHex} onchange={commitHex} onkeydown={(e) => { if (e.key === 'Enter') { commitHex(); closePicker(); } }} maxlength="7" class="ops-color-hex" aria-label="Hex color" spellcheck="false" />
                    </div>
                    <div class="ops-color-presets" role="group" aria-label="Color presets">
                      {#each colorPresets as preset (preset)}
                        <button type="button" class="ops-color-preset" class:selected={pickerHex.toLowerCase() === preset.toLowerCase()} style="background: {preset};" title={preset} aria-label="Preset {preset}" onclick={() => commitPreset(preset)}></button>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>
              <input class="dm-mini-range" style="--value:{lightbarBrightness}%" value={lightbarBrightness} disabled={!lightbarEnabled} max="100" min="0" type="range" aria-label="Lightbar brightness" oninput={(event) => setLightbarBrightness(event.currentTarget.valueAsNumber)} />
              <code>{normalizeTriggerPercent(lightbarBrightness).toString().padStart(3, '0')}</code>
              <button class:active={lightbarEnabled} class="dm-toggle" type="button" aria-label="Toggle lightbar" aria-pressed={lightbarEnabled} onclick={() => setLightbarEnabled(!lightbarEnabled)}><span></span></button>
              <button class="dm-mini-button" type="button" onclick={previewLightbar}>Preview</button>
            </div>
            <div class="dm-led-row">
              <span>Max RPM</span>
              <div class="ops-lightbar-popover-wrap">
                <button
                  bind:this={rpmPillEl}
                  type="button"
                  class="dm-color-pill ops-lightbar-preview"
                  class:on={lightbarEnabled}
                  class:disabled={!lightbarEnabled}
                  class:open={pickerOpen && pickerTarget === 'rpm'}
                  disabled={!lightbarEnabled}
                  aria-label="Max RPM indicator color"
                  aria-expanded={pickerOpen && pickerTarget === 'rpm'}
                  aria-haspopup="dialog"
                  style="--lb-color: {rpmColor}; --lb-alpha: {lightbarEnabled ? lightbarBrightness / 100 : 0};"
                  onclick={() => togglePicker('rpm')}
                ><span class="ops-lightbar-glow" aria-hidden="true"></span></button>
                {#if pickerOpen && pickerTarget === 'rpm'}
                  <div bind:this={pickerEl} class="ops-color-popover" role="dialog" aria-label="Max RPM color picker">
                    <div class="ops-color-sv" style="background-color: hsl({pickerHue}, 100%, 50%);" role="slider" tabindex="0" aria-label="Saturation and brightness" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(pickerVal * 100)} aria-valuetext="Saturation {Math.round(pickerSat * 100)}%, brightness {Math.round(pickerVal * 100)}%" onpointerdown={handleSvPointer} onkeydown={handleSvKeydown}>
                      <div class="ops-color-sv-overlay"></div>
                      <div class="ops-color-sv-cursor" style="left: {pickerSat * 100}%; top: {(1 - pickerVal) * 100}%; background: {pickerHex};"></div>
                    </div>
                    <input type="range" min="0" max="360" value={pickerHue} oninput={handleHueInput} class="ops-color-hue" aria-label="Hue" />
                    <div class="ops-color-row">
                      <span class="ops-color-row-swatch" style="background: {pickerHex};"></span>
                      <input type="text" bind:value={pickerHex} onchange={commitHex} onkeydown={(e) => { if (e.key === 'Enter') { commitHex(); closePicker(); } }} maxlength="7" class="ops-color-hex" aria-label="Hex color" spellcheck="false" />
                    </div>
                    <div class="ops-color-presets" role="group" aria-label="Color presets">
                      {#each colorPresets as preset (preset)}
                        <button type="button" class="ops-color-preset" class:selected={pickerHex.toLowerCase() === preset.toLowerCase()} style="background: {preset};" title={preset} aria-label="Preset {preset}" onclick={() => commitPreset(preset)}></button>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>
              <input class="dm-mini-range" style="--value:{lightbarBrightness}%" value={lightbarBrightness} disabled={!lightbarEnabled} max="100" min="0" type="range" aria-label="Max RPM indicator brightness" oninput={(event) => setLightbarBrightness(event.currentTarget.valueAsNumber)} />
              <code>{normalizeTriggerPercent(lightbarBrightness).toString().padStart(3, '0')}</code>
              <button class:active={lightbarEnabled} class="dm-toggle" type="button" aria-label="Toggle Max RPM indicator" aria-pressed={lightbarEnabled} onclick={() => setLightbarEnabled(!lightbarEnabled)}><span></span></button>
              <button class="dm-mini-button" type="button" onclick={previewRpmColor}>Preview</button>
            </div>
          </div>
        </div>

        <div class="dm-profile-console" bind:this={profilePanelEl}>
          <div class="dm-profile-line">
            <label>
              <span>Profile</span>
              <select value={selectedOverrideProfileId || activeProfileId} disabled={!profiles.length} onchange={(event) => void activateProfileById(event.currentTarget.value)}>
                {#each profileContextProfiles as profile}
                  <option value={profile.id}>{profile.name}{profile.id === activeProfileId ? ' / active' : ''}</option>
                {/each}
              </select>
            </label>
            <div class="dm-action-row">
              <button class="dm-mini-button" type="button" onclick={requestProfileImport}>Import</button>
              <input bind:this={profileImportInput} class="ops-hidden-file" type="file" accept="application/json,.json,.dscc-profile" onchange={(event) => void handleProfileImport(event)} />
              <button class="dm-mini-button" type="button" disabled={!activeProfileId || profileFileBusy} onclick={() => void exportSelectedProfile()}>Export</button>
              <button
                class="dm-mini-button"
                type="button"
                disabled={!canRenameSelectedProfile || profileRenameBusy || !selectedActionProfile}
                title={canRenameSelectedProfile ? 'Rename selected custom profile' : 'Built-in profiles cannot be renamed'}
                onclick={beginRenameSelectedProfile}
              >Rename</button>
              <button
                class="dm-mini-button"
                type="button"
                disabled={!canDeleteSelectedProfile || profileFileBusy || !selectedActionProfile}
                title={canDeleteSelectedProfile ? 'Delete selected custom profile' : 'Built-in profiles cannot be deleted'}
                onclick={() => selectedActionProfile && void deleteProfileById(selectedActionProfile.id, selectedActionProfile.name)}
              >Delete</button>
              <button class="dm-mini-button" type="button" onclick={restoreDefaults}><RotateCcw size={14} /> Reset</button>
              <button
                class:dirty={profileConfigDirty}
                class="dm-apply-button"
                type="button"
                disabled={!activeProfileId || profileSaveBusy || !profileConfigDirty}
                onclick={() => void saveActiveProfile()}
              ><Save size={14} /> {profileSaveBusy ? 'Saving' : 'Save'}</button>
            </div>
          </div>
          {#if renameProfileId}
            <div class="dm-profile-rename">
              <label>
                <span>Name</span>
                <input
                  bind:value={renameProfileName}
                  disabled={profileRenameBusy}
                  maxlength="80"
                  spellcheck="false"
                  onkeydown={handleRenameProfileKeydown}
                  aria-label="Profile name"
                />
              </label>
              <div class="dm-action-row">
                <button class="dm-mini-button" type="button" disabled={profileRenameBusy} onclick={cancelRenameProfile}>Cancel</button>
                <button class="dm-mini-button primary" type="button" disabled={profileRenameBusy || !renameProfileName.trim()} onclick={() => void submitRenameProfile()}>
                  {profileRenameBusy ? 'Saving' : 'Apply'}
                </button>
              </div>
            </div>
          {/if}
        </div>
      </aside>
    </section>
    <section
      class:dm-view-hidden={activeView !== 'buttonMapping'}
      class="dm-button-map-page"
      aria-label="Button mapping workspace"
      aria-hidden={activeView !== 'buttonMapping'}
    >
      <header class="dm-mapping-header">
        <div class="dm-mapping-titleblock">
          <span class="dm-mapping-eyebrow">
            {steamInputStatus?.running ? 'Steam Input · Online' : 'Steam Input · Offline'}
            <em>·</em>
            {controllerHeaderName.toUpperCase()}
            {#if controller?.transport && controller.transport !== 'Unknown'}
              <em>·</em>
              {controller.transport}
            {/if}
          </span>
          <h2>Customize Button Assignments</h2>
        </div>
        <p class="dm-mapping-context">
          <strong>{steamContextGame?.name ?? 'No supported game selected'}</strong>
          <em>· {steamLayoutTitle}</em>
          <em class="dm-mapping-context-count">· {mappedVisibleChipCount}/{visibleMappingChips.length} inputs mapped</em>
        </p>
      </header>

      <div class="dm-mapping-stage" aria-label="DualSense button mapping workspace">
        <svg
          class="dm-mapping-lines"
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          {#each visibleMappingChips as chip (chip.key)}
            <line
              x1={chip.chipX}
              y1={chip.chipY}
              x2={chip.anchorX}
              y2={chip.anchorY}
              class:focused={focusedSlotKey === chip.key}
              class:active={isSteamSlotSelected(chip.slot)}
            />
          {/each}
        </svg>

        <div class="dm-mapping-figure">
          <div class="dm-controller-glow" aria-hidden="true"></div>
          <img class="dm-controller-base" src="/dualsense/controller_front.png" alt="DualSense controller front view" />
          {#each focusedSlotsByKey as entry (entry.key)}
            <img
              class="dm-controller-focus"
              class:visible={focusedFocusKey === entry.focus && focusedSlotKey === entry.key}
              src={`/dualsense/focus/focus_${entry.focus}.png`}
              alt=""
              aria-hidden="true"
            />
          {/each}

        </div>

        {#each visibleMappingChips as chip (chip.key)}
          <button
            class="dm-mapping-chip {chip.side}"
            class:active={isSteamSlotSelected(chip.slot)}
            class:focused={focusedSlotKey === chip.key}
            class:edge={chip.slot.group === 'DualSense Edge'}
            style:--chip-x="{chip.chipX}%"
            style:--chip-y="{chip.chipY}%"
            type="button"
            onclick={() => selectSteamSlot(chip.slot)}
            onmouseenter={() => hoverSteamSlot(chip.slot)}
            onmouseleave={() => hoverSteamSlot(null)}
            onfocus={() => hoverSteamSlot(chip.slot)}
            onblur={() => hoverSteamSlot(null)}
            aria-label="{chip.slot.label}: {chipDisplayLabel(bindingForSteamSlot(steamInputBindings, chip.slot))}"
          >
            <span class="dm-mapping-chip-icon">
              {#if steamSlotIconUrl(chip.key)}
                <img src={steamSlotIconUrl(chip.key)} alt="" aria-hidden="true" />
              {:else}
                <span class="dm-mapping-chip-glyph">{chip.slot.label.replace(/^D-Pad\s+/i, '').slice(0, 2).toUpperCase()}</span>
              {/if}
            </span>
            <span class="dm-mapping-chip-text">
              <strong class="dm-mapping-chip-binding">{chipDisplayLabel(bindingForSteamSlot(steamInputBindings, chip.slot))}</strong>
            </span>
          </button>
        {/each}
      </div>

      <div class="dm-mapping-tray" class:populated={Boolean(focusedSlotMeta)}>
        <div class="dm-mapping-tray-info">
          {#if focusedSlotMeta}
            {#if steamSlotIconUrl(focusedSlotMeta.key)}
              <img class="dm-key-icon lg" src={steamSlotIconUrl(focusedSlotMeta.key)} alt="" aria-hidden="true" />
            {:else}
              <span class="dm-key-icon lg placeholder" aria-hidden="true">{focusedSlotMeta.label.slice(0, 2).toUpperCase()}</span>
            {/if}
            <div class="dm-mapping-tray-labels">
              <span>{focusedSlotMeta.group}</span>
              <strong>{focusedSlotMeta.label}</strong>
              <em>{compactSteamBindingLabel(bindingForSteamSlot(steamInputBindings, focusedSlotMeta))}</em>
            </div>
          {:else}
            <div class="dm-mapping-tray-labels">
              <span>Select an input</span>
              <strong>Hover or click any chip to edit its Steam Input binding</strong>
            </div>
          {/if}
        </div>

        {#if focusedSlotMeta && selectedSteamBinding}
          <div class="dm-mapping-tray-controls">
            <div class="dm-mapping-tray-field">
              <span>Target</span>
              <div class="dm-target-combo" use:clickOutside={closeTargetPicker}>
                <button
                  type="button"
                  class="dm-target-combo-trigger"
                  class:open={targetPickerOpen}
                  disabled={steamBindingBusy}
                  onclick={toggleTargetPicker}
                  aria-haspopup="listbox"
                  aria-expanded={targetPickerOpen}
                >
                  <span class="dm-target-combo-value">{currentTargetLabel()}</span>
                  <ChevronDown size={14} aria-hidden="true" />
                </button>
                {#if targetPickerOpen}
                  <div class="dm-target-combo-panel" onkeydown={handleTargetPickerKeydown} role="listbox" tabindex="-1">
                    <div class="dm-target-combo-searchbar">
                      <Search size={13} aria-hidden="true" />
                      <input
                        bind:this={targetSearchInputEl}
                        bind:value={targetSearchQuery}
                        type="search"
                        spellcheck="false"
                        placeholder="Search bindings…"
                        aria-label="Search Steam Input bindings"
                      />
                    </div>
                    <div class="dm-target-combo-list">
                      {#each filteredTargetGroups as group (group.label)}
                        <div class="dm-target-combo-group">{group.label}</div>
                        {#each group.options as option (option.raw)}
                          {@const optionTarget = steamBindingTargetPart(option.raw)}
                          <button
                            type="button"
                            class="dm-target-combo-option"
                            class:active={optionTarget === steamBindingTargetPart(steamBindingDraft)}
                            onclick={() => pickTargetOption(option.raw)}
                            role="option"
                            aria-selected={optionTarget === steamBindingTargetPart(steamBindingDraft)}
                          >
                            {option.label}
                          </button>
                        {/each}
                      {:else}
                        <div class="dm-target-combo-empty">
                          No matches for <strong>{targetSearchQuery}</strong>
                        </div>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>
            </div>
            <label class="dm-mapping-tray-field">
              <span>Label (Steam UI)</span>
              <input
                value={steamBindingLabelDraft}
                oninput={(event) => applySteamBindingLabelChange((event.currentTarget as HTMLInputElement).value)}
                disabled={steamBindingBusy}
                spellcheck="false"
                placeholder="e.g. Next radio station"
              />
            </label>
            <label class="dm-mapping-tray-field grow">
              <span>Raw VDF</span>
              <input
                bind:value={steamBindingDraft}
                oninput={syncSteamBindingLabelFromRaw}
                disabled={steamBindingBusy}
                spellcheck="false"
                placeholder="xinput_button … / key_press …"
              />
            </label>
          </div>
        {/if}

        <div class="dm-mapping-tray-actions">
          <button
            class="dm-mapping-action ghost"
            type="button"
            disabled={!selectedSteamBinding || steamBindingBusy}
            onclick={resetSteamBindingDraft}
          >Reset</button>
          <button
            class="dm-mapping-action primary"
            type="button"
            disabled={steamBindingBusy || !steamInputLayout || !selectedSteamBinding}
            onclick={() => void saveSteamBinding(false)}
          >Apply</button>
        </div>
      </div>
    </section>
  {/if}
  {#if toastMessages.length}
    <div class="dm-toast-stack" aria-live="polite" aria-atomic="false">
      {#each toastMessages as toast (toast.id)}
        <button class="dm-toast {toast.tone}" type="button" onclick={() => dismissToast(toast.id)}>
          <span>{toast.tone}</span>
          <strong>{toast.message}</strong>
        </button>
      {/each}
    </div>
  {/if}
</main>
