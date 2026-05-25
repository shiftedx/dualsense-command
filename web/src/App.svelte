<script lang="ts">
  import { Cable, ClipboardCopy, CopyPlus, Download, ExternalLink, LifeBuoy, Minus, Plus, RefreshCw, RotateCcw, Save } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import Tooltip from './components/Tooltip.svelte';
  import InitialBadge from './components/InitialBadge.svelte';
  import AddGameDialog from './components/AddGameDialog.svelte';
  import ControllerCard from './components/ControllerCard.svelte';
  import { createAppRuntime } from './lib/appRuntime';
  import {
    ButtonMappingView,
    assembleSteamBindingRaw,
    buildDefaultSteamBindingBySlotKey,
    buildSteamBindingBySlotKey,
    createSteamMirrorGroups,
    parseSteamBindingTriple,
    steamBindingKey,
    steamBindingSlots,
    steamBindingTargetPart,
    steamSlotGlyphs
  } from './lib/features/buttonMapping';
  import HapticsView from './lib/features/haptics/HapticsView.svelte';
  import type { SteamBindingSlot, SteamMirrorGroup } from './lib/features/buttonMapping';
  import {
    activateProfile,
    addCustomGame,
    clearProfileOverride,
    connectAppSnapshotSocket,
    createProfile,
    deleteProfile,
    exportProfile,
    getAppSnapshot,
    getSupportBundle,
    getAppUpdateCheck,
    getControllerInput,
    getControllerConfig,
    getEdgeProfiles,
    getSteamLibrary,
    importProfile,
    removeCustomGame,
    renameProfile,
    runEffectTest,
    saveAppSettings,
    saveControllerConfig,
    writeEdgeProfile,
    saveProfileConfig,
    setProfileOverride,
    updateControllerName,
    writeSteamInputBinding,
    writeSteamInputPaddlePreset
  } from './lib/api';
  import {
    controllerBatteryReadable,
    controllerConnectionText,
    controllerModelText
  } from './lib/controllerDisplay';
  import type {
    AppSnapshot,
    ControllerConfiguration,
    ControllerStatus,
    CurrentEffectState,
    EdgeProfileSlot,
    EdgeProfilesResponse,
    EffectTestRequest,
    ExportedProfile,
    ForzaBodyRumbleMode,
    ForzaEffectConfiguration,
    ForzaEffectRoute,
    GameDetection,
    ProfileAssignmentConfiguration,
    ProfileSummary,
    SteamInputBinding,
    SteamInputLayout,
    SteamLibraryEntry,
    SupportBundle,
    SupportedGame,
    TriggerCurvePoint
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
  type AppView = 'games' | 'haptics' | 'buttonMapping';
  type TuningScope = 'none' | 'global' | 'game';
  type ToastTone = 'success' | 'info' | 'error';
  type ToastMessage = {
    id: number;
    tone: ToastTone;
    message: string;
  };
  type SupportBundleBusy = 'copy' | 'download' | '';
  type UpdateCheckState = {
    state: 'idle' | 'checking' | 'current' | 'available' | 'error';
    currentVersion?: string;
    latestVersion?: string;
    releaseUrl?: string;
    message?: string;
  };
  type EditableControllerConfig = Omit<ControllerConfiguration, 'controllerId' | 'model'>;
  type SteamBindingTargetGroup = {
    label: string;
    options: Array<{ label: string; raw: string }>;
  };

  const appViews: Array<{ id: AppView; label: string; hash: string }> = [
    { id: 'games', label: 'Profiles', hash: '#/games' },
    { id: 'haptics', label: 'Adaptive Triggers & Haptics', hash: '#/adaptive-triggers-haptics' },
    { id: 'buttonMapping', label: 'Button Mapping', hash: '#/button-mapping' }
  ];
  const EMPTY_STEAM_INPUT_BINDINGS: SteamInputBinding[] = [];
  const EMPTY_STEAM_BINDING_MAP = new Map<string, SteamInputBinding>();
  const EMPTY_STEAM_MIRROR_GROUPS: SteamMirrorGroup[] = [];
  let defaultSteamBindingCacheReady = false;
  let defaultSteamBindingCacheFamily: ControllerStatus['family'] | undefined | null;
  let defaultSteamBindingCache = EMPTY_STEAM_BINDING_MAP;

  const defaultSteamBindingsForFamily = (family: ControllerStatus['family'] | undefined | null) => {
    if (!defaultSteamBindingCacheReady || defaultSteamBindingCacheFamily !== family) {
      defaultSteamBindingCacheReady = true;
      defaultSteamBindingCacheFamily = family;
      defaultSteamBindingCache = buildDefaultSteamBindingBySlotKey(family);
    }
    return defaultSteamBindingCache;
  };

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
    }
  ];

  type PreparedSteamBindingTargetGroup = {
    label: string;
    options: Array<{ label: string; raw: string; targetKey: string; searchText: string }>;
  };

  const defaultButtonAssignments = (edge = false): EditableControllerConfig['buttons'] => [
    { key: 'Cross', label: 'Cross' },
    { key: 'Circle', label: 'Circle' },
    { key: 'Square', label: 'Square' },
    { key: 'Triangle', label: 'Triangle' },
    { key: 'D-Pad', label: 'D-Pad' },
    { key: 'L1', label: 'L1' },
    { key: 'R1', label: 'R1' },
    { key: 'L2', label: 'L2' },
    { key: 'R2', label: 'R2' },
    { key: 'L3', label: 'L3' },
    { key: 'R3', label: 'R3' },
    { key: 'Create', label: 'Create' },
    { key: 'Options', label: 'Options' },
    { key: 'Touch Pad', label: 'Touch Pad Press' },
    { key: 'Mute', label: 'Mute' },
    ...(edge
      ? [
          { key: 'Back Left', label: 'L3' },
          { key: 'Back Right', label: 'R3' },
          { key: 'Fn Left', label: 'Previous DSCC Profile' },
          { key: 'Fn Right', label: 'Next DSCC Profile' }
        ]
      : [])
  ];

  const canonicalButtonKey = (key: string) => {
    const trimmed = key.trim();
    const legacy: Record<string, string> = {
      cross: 'Cross',
      circle: 'Circle',
      square: 'Square',
      triangle: 'Triangle',
      dpad: 'D-Pad',
      dpadUp: 'D-Pad Up',
      dpadDown: 'D-Pad Down',
      dpadLeft: 'D-Pad Left',
      dpadRight: 'D-Pad Right',
      l1: 'L1',
      r1: 'R1',
      l2: 'L2',
      r2: 'R2',
      l3: 'L3',
      r3: 'R3',
      create: 'Create',
      options: 'Options',
      touchPad: 'Touch Pad',
      mute: 'Mute',
      edgeBackLeft: 'Back Left',
      edgeBackRight: 'Back Right',
      edgeFnLeft: 'Fn Left',
      edgeFnRight: 'Fn Right'
    };
    return legacy[trimmed] ?? trimmed;
  };

  const normalizeButtonAssignments = (
    buttons: EditableControllerConfig['buttons'] | undefined,
    edge = false
  ): EditableControllerConfig['buttons'] => {
    const byKey = new Map(
      (buttons ?? [])
        .map((button) => ({
          key: canonicalButtonKey(button.key ?? ''),
          label: (button.label ?? '').trim()
        }))
        .filter((button) => button.key)
        .map((button) => [button.key, button])
    );
    const defaults = defaultButtonAssignments(edge);
    const ordered = defaults.map((button) => byKey.get(button.key) ?? button);
    const defaultKeys = new Set(defaults.map((button) => button.key));
    const extras = [...byKey.values()].filter((button) => !defaultKeys.has(button.key)).slice(0, Math.max(0, 24 - ordered.length));
    return [...ordered, ...extras];
  };
  const preparedSteamBindingTargetGroups: PreparedSteamBindingTargetGroup[] = steamBindingTargetGroups.map((group) => ({
    label: group.label,
    options: group.options.map((option) => ({
      ...option,
      targetKey: steamBindingTargetPart(option.raw),
      searchText: `${group.label} ${option.label} ${option.raw}`.toLowerCase()
    }))
  }));

  const resetSteamBindingDraft = () => {
    if (selectedSteamBinding) {
      steamBindingDraft = selectedSteamBinding.rawBinding;
      steamBindingLabelDraft = parseSteamBindingTriple(selectedSteamBinding.rawBinding).label;
      lastSteamBindingDraftKey = steamBindingKey(selectedSteamBinding);
      clearSteamBindingMessage();
    }
  };
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
  const FORZA_SHIFT_THUMP_DEFAULT_INTENSITY = 255;
  // Mirrors the backend Forza runtime profile so the graph shows felt force, not just exponent shape.
  const FORZA_BRAKE_BASELINE_FORCE = 42 / 255;
  const FORZA_BRAKE_NORMAL_FORCE = 164 / 255;
  const FORZA_BRAKE_ENDSTOP_FORCE = 238 / 255;
  const FORZA_THROTTLE_BASELINE_FORCE = 3 / 255;
  const FORZA_THROTTLE_NORMAL_FORCE = 28 / 255;
  const FORZA_THROTTLE_ENDSTOP_FORCE = 106 / 255;
  const FORZA_ENDSTOP_WALL_OFFSET = 0.03;
  const FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET = 0.28;
  const FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION = 0.70;
  const FORZA_BRAKE_OVERTRAVEL_RAMP_WIDTH = 0.16;
  const FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE = 2.0;
  const FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION = 0.80;
  const FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION = 0.80;
  const FORZA_BRAKE_ENDSTOP_FORCE_BOOST = 1.25;
  const FORZA_THROTTLE_ENDSTOP_FORCE_BOOST = 3.0;
  const FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH = 0.20;
  const FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE = 2.4;

  const shiftThumpPresets = [
    { label: 'Soft', intensity: 35 },
    { label: 'Medium', intensity: 65 },
    { label: 'Strong', intensity: 180 },
    { label: 'Max', intensity: FORZA_SHIFT_THUMP_DEFAULT_INTENSITY }
  ];

  const shiftThumpPresetHelp: Record<string, string> = {
    Soft: 'A lighter mechanical cue for users who want shift feedback without a big kick through the controller.',
    Medium: 'A moderate shift kick that is easy to feel but less abrupt than the stock strong profile.',
    Strong: 'A firmer R2 kick with reduced body feedback for a more physical gear change.',
    Max: 'The Base shift thump: the strongest cue, using the full 255 effect ceiling so gear changes punch through road texture and engine cues.'
  };

  const routeTooltips: Record<ForzaEffectRoute, string> = {
    body_both: 'Sends the effect to both grip motors. Good for road, impacts, and whole-car events.',
    body_left: 'Sends most of the effect to the left grip. Useful when you want to separate a cue from throttle-side feedback.',
    body_right: 'Sends most of the effect to the right grip. Useful for traction or throttle-related cues.',
    l2: 'Sends the effect only to the left adaptive trigger, usually brake-side feedback.',
    r2: 'Sends the effect only to the right adaptive trigger, usually throttle-side feedback.',
    both_triggers: 'Sends trigger feedback to both L2 and R2 without body rumble.',
    body_and_triggers: 'Combines adaptive trigger feedback with a short body thump. Best for gear shifts and other physical events.',
    r2_and_body: 'Combines R2 trigger feedback with a slightly reduced body thump. This is the Base shift route.',
    light_led: 'Routes the effect to LEDs or the lightbar instead of trigger/body haptics.'
  };

  const triggerEffectHelp: Record<string, string> = {
    'Adaptive resistance': 'A smooth force ramp that increases resistance as the trigger moves. This is the default because it feels closest to pedal load.',
    Pulse: 'A vibration-like trigger pulse. Useful for alerts, but less pedal-like than adaptive resistance.',
    Wall: 'Creates a hard stop at the trigger position. Best for binary actions such as a handbrake wall.',
    'Wall pulse': 'A pulsing trigger pattern with a wall-form kick. This exposes the same hardware mode DSCC uses for strong shift thumps.',
    Off: 'Disables base trigger force. Telemetry effects can still run if their individual rows are enabled.'
  };

  const triggerEffectOptions = [
    { label: 'Adaptive resistance', badge: 'Ramp' },
    { label: 'Pulse', badge: 'Pulse' },
    { label: 'Wall', badge: 'Stop' },
    { label: 'Wall pulse', badge: 'Kick' },
    { label: 'Off', badge: 'Mute' }
  ];

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

  const vibrationModeHelp: Record<string, string> = {
    Balanced: 'Keeps low and high motors blended for general-purpose body feedback.',
    'Deep thump': 'Leans into the low-frequency motor for heavier grip movement and impact cues.',
    'Fine buzz': 'Leans into the high-frequency motor for sharper texture and alert cues.'
  };

  const vibrationModeOptions = [
    { label: 'Balanced', mode: 'balanced', badge: 'Blend' },
    { label: 'Deep thump', mode: 'deep_thump', badge: 'Low' },
    { label: 'Fine buzz', mode: 'fine_buzz', badge: 'High' }
  ];

  const bodyRumbleModeOptions: Array<{ value: ForzaBodyRumbleMode; label: string; badge: string; help: string }> = [
    {
      value: 'native_passthrough',
      label: 'Native game',
      badge: 'Default',
      help: 'Leaves continuous engine and road rumble to the game while DSCC adds adaptive triggers, LEDs, shift thumps, and impact thumps.'
    },
    {
      value: 'dscc_full_control',
      label: 'DSCC mix',
      badge: 'Full body',
      help: 'Lets DSCC replace continuous body rumble with telemetry-driven road, slip, curb, puddle, and drivetrain layers.'
    }
  ];

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
      defaultIntensity: FORZA_SHIFT_THUMP_DEFAULT_INTENSITY,
      defaultRoute: 'r2_and_body',
      help: 'Fires a short kick when DSCC detects a gear change. The Base route uses R2 plus a slightly reduced body thump so shifts feel physical without hitting both triggers.'
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
      help: 'Uses road surface rumble and speed to add low continuous texture through the grips. It is enabled in the Base profile at a conservative level.'
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
      signal: 'suspension.impact_pulse',
      group: 'Body',
      defaultIntensity: 115,
      defaultRoute: 'body_both',
      help: 'Latches hard suspension and acceleration spikes into a short body thump through the grip motors. Best for jumps, crashes, and hard landings.'
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
  const UPDATE_RELEASE_PAGE_URL = 'https://github.com/shiftedx/dualsense-command/releases/latest';
  const UPDATE_DISMISSED_VERSION_KEY = 'dscc-update-dismissed-version';

  let snapshot: AppSnapshot | null = null;
  let loading = true;
  let error = '';
  let selectedControllerId = '';
  let controllerRenameId = '';
  let controllerRenameName = '';
  let controllerRenameBusy = false;
  let addGameOpen = false;
  let addGameLoading = false;
  let addGameEntries: SteamLibraryEntry[] = [];
  let addGameError = '';
  let addGameBusyAppId = '';
  let scopePickerOpen = false;
  let profilePickerOpen = false;
  let scopeTriggerEl: HTMLButtonElement | null = null;
  let profileTriggerEl: HTMLButtonElement | null = null;
  let scopeMenuPos = { left: 0, top: 0, minWidth: 240 };
  let profileMenuPos = { left: 0, top: 0, minWidth: 240 };
  let applyMessage = '';
  let appSettingsMessage = '';
  let appSettingsBusy = false;
  let supportPanelOpen = false;
  let supportBundleBusy: SupportBundleBusy = '';
  let supportBundleMessage = '';
  let supportBundleTone: ToastTone = 'info';
  let profileOverrideMessage = '';
  let toastMessages: ToastMessage[] = [];
  let nextToastId = 1;
  let selectedOverrideProfileId = '';
  let selectedTuningScope: TuningScope = 'global';
  let selectedTuningGameId = '';
  let configLoadedFor = '';
  let configLoadError = '';
  let currentControllerConfig: ControllerConfiguration | null = null;
  let edgeProfilesLoadedFor = '';
  let edgeProfiles: EdgeProfilesResponse | null = null;
  let edgeProfilesLoading = false;
  let edgeProfilesBusySlot = '';
  let edgeProfilesError = '';
  let profileSaveBaselineSignature = '';
  let profileConfigDirty = false;
  let effectActivityUntil: Record<string, number> = {};
  let partialErrorsDismissed = false;
  let lastPartialErrorSignature = '';
  let updateCheck: UpdateCheckState = { state: 'idle' };
  let checkedUpdateVersion = '';
  let updateDismissedVersion = '';
  let updateDismissalLoaded = false;
  let newProfileName = '';
  let renameProfileId = '';
  let renameProfileName = '';
  let profileRenameBusy = false;
  let profileSaveBusy = false;
  let saveAsProfileOpen = false;
  let saveAsProfileName = '';
  let profileSaveAsBusy = false;
  let profileFileBusy = false;
  let profileImportInput: HTMLInputElement | undefined;
  let profilePanelEl: HTMLDivElement | undefined;
  let appRuntime: ReturnType<typeof createAppRuntime> | undefined;
  let liveConfigSyncTimer: number | undefined;
  let liveConfigSyncInFlight = false;
  let liveConfigSyncQueued = false;
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
  let optimisticSteamInputBindings: SteamInputBinding[] | null = null;
  let activeSteamMappingContextKey = '';
  let steamBindingBusy = false;
  let steamBindingMessage = '';
  let paddlePresetLeftKey = 'Q';
  let paddlePresetRightKey = 'E';
  let hoveredSteamSlotKey = '';
  let activeSteamSlotKey = '';

  let l2From = 20;
  let l2To = 100;
  let r2From = 0;
  let r2To = 100;
  let l2Curve = 1.35;
  let r2Curve = 2.25;
  let l2CurvePoints: TriggerCurvePoint[] = [];
  let r2CurvePoints: TriggerCurvePoint[] = [];
  let curveHover: { side: TriggerSide; x: number; y: number; left: number; top: number } | null = null;
  let curveDragSide: TriggerSide | null = null;
  let curveDragPoint: { side: TriggerSide; index: number } | null = null;
  let triggerCurveDisplayMode: TriggerCurveDisplayMode = 'base';
  let activeView: AppView = 'games';
  let triggerEffect = 'Adaptive resistance';
  let triggerIntensity = 'Strong (Standard)';
  let vibrationIntensity = 'Medium';
  let vibrationMode = 'Balanced';
  let forzaBodyRumbleMode: ForzaBodyRumbleMode = 'native_passthrough';
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
  const DEFAULT_FORZA_EFFECT_BY_ID = new Map(defaultForzaEffects().map((effect) => [effect.id, effect]));
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
  $: globalProfilePreview =
    profiles.find((profile) => profile.scope === 'Global') ??
    profiles.find((profile) => profile.scope === 'Built-in' && profile.id === 'forza-horizon-immersive') ??
    profiles.find((profile) => profile.scope === 'Built-in');
  $: logs = snapshot?.logs ?? [];
  $: diagnostics = snapshot?.diagnostics ?? [];
  $: telemetry = snapshot?.telemetry ?? [];
  $: telemetryByName = new Map(telemetry.map((item) => [item.name, item]));
  $: effectState = snapshot?.effectState;
  $: l2LivePress = controllerInputFresh ? l2ControllerPress : selectedTuningScope === 'global' ? 0 : telemetryUnitValue('input.brake');
  $: r2LivePress = controllerInputFresh ? r2ControllerPress : selectedTuningScope === 'global' ? 0 : telemetryUnitValue('input.throttle');
  $: triggerCurveDisplayMode = selectedTuningScope === 'game' && usesForzaRuntimeProfile(selectedTuningGame) ? 'forza' : 'base';
  $: appSettings = snapshot?.appSettings;
  $: forzaGlyphs = appSettings?.settings.forzaPlaystationGlyphs;
  $: listenOnAllInterfaces = appSettings?.settings.listenOnAllInterfaces ?? false;
  $: lanRestartRequired = appSettings?.restartRequired ?? false;
  $: glyphOverrideEnabled = forzaGlyphs?.enabled ?? false;
  $: glyphInstallPath =
    forzaGlyphs?.installPath ?? 'C:\\Program Files (x86)\\Steam\\steamapps\\common\\ForzaHorizon6';
  $: adapter =
    snapshot?.adapters.find((item) => item.id === snapshot?.profileResolution.activeAdapterId || item.name === status?.activeAdapter) ??
    snapshot?.adapters[0];
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
  $: canDeleteSelectedProfile = Boolean(selectedActionProfile && !selectedActionProfile.builtIn);
  $: canRenameSelectedProfile = Boolean(selectedActionProfile && !selectedActionProfile.builtIn);
  $: controllerHeaderName = controllerModelText(controller);
  $: controllerHeaderMeta = controllerConnectionText(controller);
  $: controllerHeaderBatteryReadable = controllerBatteryReadable(controller);
  $: overrideActive = Boolean(snapshot?.profileResolution.overrideProfileId);
  $: detectedGameLabel = snapshot?.gameDetection.activeGameName ?? snapshot?.profileResolution.detectedGameId ?? 'current game';
  $: supportedGames = snapshot?.gameDetection.supportedGames ?? [];
  $: if (selectedTuningGameId && supportedGames.length && !supportedGames.some((game) => game.gameId === selectedTuningGameId)) {
    selectedTuningGameId = '';
    if (selectedTuningScope === 'game') selectedTuningScope = 'global';
  }
  $: selectedGame =
    snapshot?.gameDetection.selectedGame ??
    supportedGames.find((game) => game.gameId === snapshot?.gameDetection.activeGameId) ??
    null;
  $: discoveredGames = supportedGames
    .filter((game) => game.running || game.installed || game.gameId === selectedGame?.gameId)
    .sort((left, right) =>
      Number(right.running) - Number(left.running) ||
      Number(right.installed) - Number(left.installed) ||
      left.name.localeCompare(right.name)
    );
  $: selectedTuningGame = selectedTuningGameId
    ? supportedGames.find((game) => game.gameId === selectedTuningGameId) ?? null
    : null;
  $: if (selectedTuningScope !== 'game' && selectedTuningGameId) {
    selectedTuningGameId = '';
  }
  $: tuningReady = Boolean(controller && (selectedTuningScope === 'global' || selectedTuningGame));
  $: buttonMappingReady = Boolean(controller && (selectedTuningGame || selectedTuningScope === 'global'));
  $: if (!tuningReady && activeView !== 'games') {
    activeView = 'games';
  } else if (activeView === 'buttonMapping' && !buttonMappingReady) {
    activeView = 'haptics';
  }
  $: profileContextGame = selectedTuningScope === 'game' ? selectedTuningGame : null;
  $: profileContextGameId = profileContextGame?.gameId ?? null;
  $: profileContextLabel = selectedTuningScope === 'global' ? 'Global Profile' : profileContextGame?.name ?? detectedGameLabel;
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
  $: profileContextBadgeProfile = selectedOverrideProfile ?? profileContextProfiles[0] ?? activeProfile;
  $: activeProfileContextLabel =
    selectedTuningScope === 'global'
      ? 'global scope'
      : profileContextGame && profileContextBadgeProfile
        ? profileContextTag(profileContextBadgeProfile)
        : 'game scope';
  $: profileContextDetail =
    selectedTuningScope === 'global'
      ? 'Controller-wide tuning'
      : profileContextGame
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
  $: steamContextGame = profileContextGame;
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
    : selectedTuningScope === 'global'
      ? 'Controller-wide haptics'
      : detectionSignalText || 'Steam library data unavailable';
  $: activeProfileHeader = selectedActionProfile ?? profiles.find((profile) => profile.id === activeProfileId) ?? null;
  $: activeProfileHeaderName = activeProfileHeader?.name ?? activeProfileName ?? 'None';
  $: activeProfileHeaderMeta = (() => {
    if (!activeProfileHeader) return profiles.length ? 'No profile resolved' : 'Profiles loading';
    const scope = activeProfileHeader.builtIn && activeProfileHeader.scope === 'Built-in'
      ? 'Built-in template'
      : activeProfileHeader.builtIn
        ? 'Stock / Global'
      : activeProfileHeader.scope === 'Game'
        ? `Custom / ${steamContextGame?.name ?? 'game'}`
        : 'Custom / Global';
    return activeProfileHeader.id === activeProfileId ? `${scope} / live` : `${scope} / editing`;
  })();
  $: buttonMappingActive = activeView === 'buttonMapping';
  $: steamInputStatus = snapshot?.steamInput;
  $: steamInputLayout = buttonMappingActive
    ? selectSteamInputLayout(steamInputStatus?.layouts ?? [], steamContextGame, controller?.family)
    : null;
  $: steamPaddlePresetVisible =
    buttonMappingActive && controller?.family === 'DualSense Edge' && selectedTuningScope === 'game';
  $: rawSteamInputBindings = buttonMappingActive
    ? steamInputLayout?.bindings ?? EMPTY_STEAM_INPUT_BINDINGS
    : EMPTY_STEAM_INPUT_BINDINGS;
  $: steamMappingContextKey = [
    buttonMappingActive ? steamInputLayout?.source ?? '' : '',
    buttonMappingActive ? steamContextGame?.gameId ?? '' : '',
    buttonMappingActive ? controller?.id ?? '' : '',
    buttonMappingActive ? controller?.family ?? '' : ''
  ].join('|');
  $: if (buttonMappingActive && steamMappingContextKey !== activeSteamMappingContextKey) {
    activeSteamMappingContextKey = steamMappingContextKey;
    optimisticSteamInputBindings = null;
    activeSteamSlotKey = '';
    hoveredSteamSlotKey = '';
  }
  $: realSteamInputBindings = buttonMappingActive
    ? optimisticSteamInputBindings ?? rawSteamInputBindings
    : EMPTY_STEAM_INPUT_BINDINGS;
  $: realSteamBindingBySlotKey = buttonMappingActive
    ? buildSteamBindingBySlotKey(realSteamInputBindings, steamBindingSlots)
    : EMPTY_STEAM_BINDING_MAP;
  $: steamPaddlePresetLeftBinding = steamPaddlePresetVisible
    ? realSteamBindingBySlotKey.get('edgeBackLeft') ?? null
    : null;
  $: steamPaddlePresetRightBinding = steamPaddlePresetVisible
    ? realSteamBindingBySlotKey.get('edgeBackRight') ?? null
    : null;
  $: steamPaddlePresetAvailable = Boolean(
    steamPaddlePresetVisible &&
    steamInputLayout &&
    steamPaddlePresetLeftBinding &&
    steamPaddlePresetRightBinding &&
    !steamPaddlePresetLeftBinding.synthetic &&
    !steamPaddlePresetRightBinding.synthetic
  );
  $: steamPaddlePresetStatus = !steamPaddlePresetVisible
    ? 'DualSense Edge controller required.'
    : !steamInputLayout
      ? 'Select a Steam game with a loaded Steam Input layout.'
      : !steamPaddlePresetAvailable
        ? 'Steam Input must expose both Edge back paddles before DSCC can write them.'
        : 'Writes Back Left and Back Right as Steam Input keyboard bindings for this game. This is PC-local and does not change onboard Edge memory.';
  $: defaultSteamBindingBySlotKey = buttonMappingActive
    ? defaultSteamBindingsForFamily(controller?.family)
    : EMPTY_STEAM_BINDING_MAP;
  $: steamBindingBySlotKey = buttonMappingActive
    ? new Map([...defaultSteamBindingBySlotKey, ...realSteamBindingBySlotKey])
    : EMPTY_STEAM_BINDING_MAP;
  $: steamInputBindings = buttonMappingActive
    ? [
        ...realSteamInputBindings,
        ...[...defaultSteamBindingBySlotKey.entries()]
          .filter(([slotKey]) => !realSteamBindingBySlotKey.has(slotKey))
          .map(([, binding]) => binding)
      ]
    : EMPTY_STEAM_INPUT_BINDINGS;
  $: if (
    buttonMappingActive &&
    steamInputBindings.length &&
    !activeSteamSlotKey &&
    !steamInputBindings.some((binding) => steamBindingKey(binding) === selectedSteamBindingKey)
  ) {
    selectedSteamBindingKey = steamBindingKey(steamInputBindings[0]);
  }
  $: if (buttonMappingActive && !steamInputBindings.length && selectedSteamBindingKey) {
    selectedSteamBindingKey = '';
  }
  $: selectedSteamBinding =
    buttonMappingActive && selectedSteamBindingKey
      ? steamInputBindings.find((binding) => steamBindingKey(binding) === selectedSteamBindingKey) ?? null
      : null;
  $: if (buttonMappingActive && selectedSteamBinding && steamBindingKey(selectedSteamBinding) !== lastSteamBindingDraftKey) {
    lastSteamBindingDraftKey = steamBindingKey(selectedSteamBinding);
    steamBindingDraft = selectedSteamBinding.rawBinding;
    steamBindingLabelDraft = parseSteamBindingTriple(selectedSteamBinding.rawBinding).label;
    clearSteamBindingMessage();
  }
  $: steamLayoutTitle = buttonMappingActive ? steamInputLayout?.title ?? 'Steam Input Layout' : 'Steam Input Layout';
  // Focused slot drives the controller-stage focus highlight. Hover wins, then
  // explicitly-clicked slot, then the slot owning the currently selected binding.
  $: focusedSlotKey = buttonMappingActive ? (() => {
    if (hoveredSteamSlotKey) return hoveredSteamSlotKey;
    if (activeSteamSlotKey) return activeSteamSlotKey;
    const fromBinding = steamBindingSlots.find((slot) => {
      const binding = steamBindingBySlotKey.get(slot.key);
      return Boolean(binding && steamBindingKey(binding) === selectedSteamBindingKey);
    });
    return fromBinding?.key ?? '';
  })() : '';
  $: focusedSlotMeta = focusedSlotKey
    ? steamBindingSlots.find((slot) => slot.key === focusedSlotKey) ?? null
    : null;
  // Materialised chip list joined with current slot/binding state. Edge chips
  // are hidden when the controller is not an Edge and nothing is mapped to them
  // yet — keeps the stage uncluttered for stock DualSense users.
  $: focusedSlotBinding = focusedSlotMeta ? steamBindingBySlotKey.get(focusedSlotMeta.key) ?? null : null;
  $: focusedSlotSelectedBinding =
    focusedSlotBinding && steamBindingKey(focusedSlotBinding) === selectedSteamBindingKey ? focusedSlotBinding : null;
  $: steamMirrorGroups = buttonMappingActive
    ? createSteamMirrorGroups({
        bindingBySlotKey: steamBindingBySlotKey,
        controllerFamily: controller?.family,
        selectedBindingKey: selectedSteamBindingKey,
        activeSlotKey: activeSteamSlotKey
      })
    : EMPTY_STEAM_MIRROR_GROUPS;
  $: mappedVisibleChipCount = steamMirrorGroups.reduce(
    (count, group) => count + group.rows.filter((row) => row.binding).length,
    0
  );
  $: telemetryPacketRate = adapter?.packetRateHz ?? 0;
  $: telemetryRateText = `${telemetryPacketRate >= 100 ? telemetryPacketRate.toFixed(0) : telemetryPacketRate.toFixed(1)} Hz`;
  $: telemetryRateDetail = telemetryRateStatusText(adapter);
  $: systemReadoutTitle = selectedTuningScope === 'global' ? 'Profile Scope' : 'Telemetry Rate';
  $: systemReadoutValue = selectedTuningScope === 'global' ? 'Global' : telemetryRateText;
  $: systemReadoutDetail =
    selectedTuningScope === 'global'
      ? 'Controller-only tuning'
      : telemetryRateDetail;
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
  $: if (!profileSaveBusy && selectedTuningScope === 'none' && activeProfileId && activeProfileId !== lastSyncedActiveProfileId) {
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

  const forzaPresetEffects = (preset: 'base' | 'immersive'): ForzaEffectConfiguration[] => {
    const entries: Array<[string, boolean, number, ForzaEffectRoute]> =
      preset === 'immersive'
        ? [
            ['brake_resistance', true, 100, 'l2'],
            ['throttle_resistance', true, 100, 'r2'],
            ['abs_slip_pulse', true, 100, 'l2'],
            ['handbrake_wall', true, 100, 'l2'],
            ['rev_limiter_buzz', true, 62, 'r2'],
            ['gear_shift_thump', true, FORZA_SHIFT_THUMP_DEFAULT_INTENSITY, 'r2_and_body'],
            ['road_texture', true, 35, 'body_both'],
            ['rumble_strip', true, 38, 'body_both'],
            ['tire_slip', true, 30, 'body_right'],
            ['puddle_drag', true, 32, 'body_left'],
            ['suspension_impact', true, 82, 'body_both'],
            ['rpm_leds', false, 100, 'light_led']
          ]
        : [
            ['brake_resistance', true, 100, 'l2'],
            ['throttle_resistance', true, 100, 'r2'],
            ['abs_slip_pulse', true, 100, 'l2'],
            ['handbrake_wall', true, 100, 'l2'],
            ['rev_limiter_buzz', true, 55, 'r2'],
            ['gear_shift_thump', true, FORZA_SHIFT_THUMP_DEFAULT_INTENSITY, 'r2_and_body'],
            ['road_texture', true, 40, 'body_both'],
            ['rumble_strip', false, 55, 'body_both'],
            ['tire_slip', false, 65, 'body_right'],
            ['puddle_drag', false, 50, 'body_left'],
            ['suspension_impact', false, 70, 'body_both'],
            ['rpm_leds', false, 100, 'light_led']
          ];
    return normalizeForzaEffects(entries.map(([id, enabled, intensity, route]) => ({ id, enabled, intensity, route })));
  };

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

  $: partialErrors = snapshot?.partialErrors ?? [];
  $: showPartialErrorBanner = partialErrors.length > 0 && !partialErrorsDismissed;
  $: showUpdateBanner =
    updateCheck.state === 'available' &&
    Boolean(updateCheck.latestVersion) &&
    updateCheck.latestVersion !== updateDismissedVersion;
  $: if (status?.version) {
    void checkForAppUpdate(status.version);
  }

  const dismissPartialErrors = () => {
    partialErrorsDismissed = true;
  };

  const normalizeVersion = (value: string | undefined | null) => (value ?? '').trim().replace(/^v/i, '');

  const loadDismissedUpdateVersion = () => {
    if (typeof window === 'undefined' || updateDismissalLoaded) return;
    updateDismissalLoaded = true;
    try {
      updateDismissedVersion = window.localStorage.getItem(UPDATE_DISMISSED_VERSION_KEY) ?? '';
    } catch {
      updateDismissedVersion = '';
    }
  };

  const dismissUpdateBanner = () => {
    const version = updateCheck.latestVersion ?? '';
    updateDismissedVersion = version;
    if (typeof window === 'undefined' || !version) return;
    try {
      window.localStorage.setItem(UPDATE_DISMISSED_VERSION_KEY, version);
    } catch {
      // Dismissal is convenience state; failing to persist it should not block use.
    }
  };

  const checkForAppUpdate = async (currentVersionRaw: string) => {
    if (typeof window === 'undefined' || typeof fetch !== 'function') return;
    const currentVersion = normalizeVersion(currentVersionRaw);
    if (!currentVersion || currentVersion.toLowerCase() === 'unknown' || checkedUpdateVersion === currentVersion) return;

    checkedUpdateVersion = currentVersion;
    updateCheck = { state: 'checking', currentVersion };
    try {
      const result = await getAppUpdateCheck(currentVersion);
      updateCheck = result.updateAvailable
        ? {
            state: 'available',
            currentVersion: result.currentVersion,
            latestVersion: result.latestVersion,
            releaseUrl: result.releaseUrl
          }
        : {
            state: 'current',
            currentVersion: result.currentVersion,
            latestVersion: result.latestVersion,
            releaseUrl: result.releaseUrl
          };
    } catch (caught) {
      updateCheck = {
        state: 'error',
        currentVersion,
        message: caught instanceof Error ? caught.message : 'Update check failed'
      };
      console.warn('DSCC update check failed', caught);
    }
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
  type TriggerCurveDisplayMode = 'base' | 'forza';
  type ForzaTriggerForceModel = {
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
  const defaultTriggerCurve = (side: TriggerSide) => (side === 'l2' ? 1.35 : 2.25);
  const TRIGGER_CURVE_POINT_MIN = 4;
  const TRIGGER_CURVE_POINT_MAX = 8;
  const TRIGGER_CURVE_SAMPLE_POSITIONS = Array.from({ length: 101 }, (_, index) => index / 100);

  const appViewFromHash = (): AppView => {
    if (typeof window === 'undefined') return 'games';
    if (window.location.hash === '#/button-mapping') return buttonMappingReady ? 'buttonMapping' : tuningReady ? 'haptics' : 'games';
    if (window.location.hash === '#/adaptive-triggers-haptics') return tuningReady ? 'haptics' : 'games';
    return 'games';
  };

  const setViewHash = (view: AppView) => {
    if (typeof window === 'undefined') return;
    const nextHash = appViews.find((item) => item.id === view)?.hash ?? appViews[0].hash;
    if (window.location.hash !== nextHash) window.location.hash = nextHash;
  };

  const navigateToView = (view: AppView) => {
    if (view !== 'games' && !tuningReady) view = 'games';
    if (view === 'buttonMapping' && !buttonMappingReady) view = tuningReady ? 'haptics' : 'games';
    activeView = view;
    setViewHash(view);
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

  const triggerCurvePointsFromCurve = (curve: number): TriggerCurvePoint[] =>
    [0, 25, 50, 75, 100].map((input) => ({
      input,
      output: normalizeTriggerPercent(Math.pow(input / 100, curve) * 100)
    }));

  const normalizeTriggerCurvePoints = (
    points: TriggerCurvePoint[] | undefined,
    fallbackCurve = 1.35
  ): TriggerCurvePoint[] => {
    if (!points || points.length < TRIGGER_CURVE_POINT_MIN) {
      return triggerCurvePointsFromCurve(fallbackCurve);
    }

    const deduped = new Map<number, TriggerCurvePoint>();
    for (const point of points) {
      const input = normalizeTriggerPercent(point?.input ?? 0);
      const output = normalizeTriggerPercent(point?.output ?? 0);
      deduped.set(input, { input, output });
    }
    deduped.set(0, { input: 0, output: 0 });
    deduped.set(100, { input: 100, output: 100 });
    const sorted = [...deduped.values()].sort((a, b) => a.input - b.input);
    if (sorted.length < TRIGGER_CURVE_POINT_MIN) return triggerCurvePointsFromCurve(fallbackCurve);
    if (sorted.length <= TRIGGER_CURVE_POINT_MAX) return sorted;
    return [sorted[0], ...sorted.slice(1, TRIGGER_CURVE_POINT_MAX - 1), sorted[sorted.length - 1]];
  };

  const triggerCurvePointOutput = (points: TriggerCurvePoint[], active: number) => {
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

  const normalizedSteamControllerType = (controllerLike: string | null | undefined) => {
    const value = (controllerLike ?? '').toLowerCase();
    if (value.includes('edge')) return 'controller_ps5_edge';
    if (value.includes('dualsense') || value.includes('ps5')) return 'controller_ps5';
    if (value.includes('dualshock') || value.includes('ps4')) return 'controller_ps4';
    return '';
  };

  const selectSteamInputLayout = (
    layouts: SteamInputLayout[],
    game: SupportedGame | null | undefined,
    controllerFamily: ControllerStatus['family'] | string | null | undefined
  ) => {
    if (!layouts.length) return null;
    const appId = game?.appId ?? null;
    const controllerType = normalizedSteamControllerType(controllerFamily);
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

  const applySteamBindingRawChange = (nextRaw: string) => {
    steamBindingDraft = nextRaw;
    syncSteamBindingLabelFromRaw();
  };

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
    const binding = steamBindingBySlotKey.get(slot.key) ?? null;
    if (binding) {
      selectSteamBinding(binding);
    } else {
      selectedSteamBindingKey = '';
      lastSteamBindingDraftKey = '';
      steamBindingDraft = '';
      steamBindingLabelDraft = '';
      setSteamBindingMessage(`${slot.label} has no Steam Input binding in this layout yet.`, 'info');
    }
  };

  const hoverSteamSlot = (slot: SteamBindingSlot | null) => {
    hoveredSteamSlotKey = slot?.key ?? '';
  };

  const applyOptimisticSteamBinding = (updatedBinding: SteamInputBinding) => {
    const updatedKey = steamBindingKey(updatedBinding);
    const baseBindings = optimisticSteamInputBindings ?? rawSteamInputBindings;
    let replaced = false;
    optimisticSteamInputBindings = baseBindings.map((binding) => {
      if (steamBindingKey(binding) !== updatedKey) return binding;
      replaced = true;
      return updatedBinding;
    });
    if (!replaced) optimisticSteamInputBindings = [...optimisticSteamInputBindings, updatedBinding];
  };

  const saveSteamBinding = async (dryRun = false) => {
    const bindingToSave = focusedSlotSelectedBinding ?? selectedSteamBinding;
    if (!steamInputLayout || !bindingToSave) {
      setSteamBindingMessage('Load a Steam Input layout and select a binding first.', 'error');
      return;
    }
    if (bindingToSave.synthetic) {
      setSteamBindingMessage('This input is using DSCC default mapping. Open or create a Steam Input layout for this game before saving a custom binding.', 'error');
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
        inputId: bindingToSave.inputId,
        groupId: bindingToSave.groupId ?? null,
        activator: bindingToSave.activator ?? null,
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
      if (!dryRun) {
        applyOptimisticSteamBinding(response.binding);
        void refresh().finally(() => {
          optimisticSteamInputBindings = null;
        });
      }
    } catch (caught) {
      setSteamBindingMessage(caught instanceof Error ? caught.message : 'Unable to write Steam Input binding.', 'error');
    } finally {
      steamBindingBusy = false;
    }
  };

  const normalizePaddlePresetKey = (value: string) =>
    value
      .trim()
      .replaceAll(' ', '_')
      .replaceAll('-', '_')
      .toUpperCase()
      .replace(/[^A-Z0-9_]/g, '')
      .slice(0, 32);

  const setPaddlePresetLeftKey = (value: string) => {
    paddlePresetLeftKey = normalizePaddlePresetKey(value);
  };

  const setPaddlePresetRightKey = (value: string) => {
    paddlePresetRightKey = normalizePaddlePresetKey(value);
  };

  const applySteamPaddlePreset = async (dryRun = false) => {
    if (!steamInputLayout) {
      setSteamBindingMessage('Load a Steam Input layout before applying the paddle preset.', 'error');
      return;
    }
    if (controller?.family !== 'DualSense Edge') {
      setSteamBindingMessage('Steam Input paddle presets require a DualSense Edge layout.', 'error');
      return;
    }
    if (!steamPaddlePresetAvailable) {
      setSteamBindingMessage(steamPaddlePresetStatus, 'error');
      return;
    }
    steamBindingBusy = true;
    setSteamBindingMessage(dryRun ? 'Validating Edge paddle preset...' : 'Saving Edge paddle preset...', 'info');
    try {
      const response = await writeSteamInputPaddlePreset({
        layoutSource: steamInputLayout.source,
        appId: steamInputLayout.appId ?? steamContextGame?.appId ?? null,
        leftKey: paddlePresetLeftKey || 'Q',
        rightKey: paddlePresetRightKey || 'E',
        profileName: activeProfileName || profileContextGame?.name || steamContextGame?.name || null,
        dryRun
      });
      const warningText = response.warnings.length ? ` ${response.warnings.join(' ')}` : '';
      setSteamBindingMessage(
        response.backupPath ? `${response.message} Backup: ${response.backupPath}${warningText}` : `${response.message}${warningText}`,
        'success'
      );
      if (!dryRun) {
        for (const paddle of response.paddles) {
          applyOptimisticSteamBinding(paddle.binding);
        }
        const selectedPaddle = response.paddles[0]?.binding;
        if (selectedPaddle) {
          selectedSteamBindingKey = steamBindingKey(selectedPaddle);
          lastSteamBindingDraftKey = selectedSteamBindingKey;
          steamBindingDraft = selectedPaddle.rawBinding;
          steamBindingLabelDraft = parseSteamBindingTriple(selectedPaddle.rawBinding).label;
        }
        void refresh().finally(() => {
          optimisticSteamInputBindings = null;
        });
      }
    } catch (caught) {
      setSteamBindingMessage(caught instanceof Error ? caught.message : 'Unable to save Edge paddle preset.', 'error');
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
      l2CurvePoints = triggerCurvePointsFromCurve(value);
    } else {
      r2Curve = value;
      r2CurvePoints = triggerCurvePointsFromCurve(value);
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

  const builtInProfileIdForGame = (game: SupportedGame | null | undefined): string | null => {
    const gameId = game?.gameId.toLowerCase() ?? '';
    if (gameId === 'assetto-corsa-rally') return 'assetto-corsa-rally';
    if (gameId.startsWith('forza-horizon')) {
      return (
        profiles.find((profile) => profile.id === 'forza-horizon-immersive')?.id ??
        profiles.find((profile) => profile.id === 'forza-horizon')?.id ??
        null
      );
    }
    return null;
  };

  const usesForzaRuntimeProfile = (game: SupportedGame | null | undefined) => {
    const gameId = game?.gameId.toLowerCase() ?? '';
    return gameId.startsWith('forza') || gameId === 'assetto-corsa-rally';
  };

  const profileAssignmentMatchesGame = (assignment: ProfileAssignmentConfiguration, game: SupportedGame) => {
    const assignmentGameId = assignment.gameId.trim().toLowerCase();
    const gameId = game.gameId.trim().toLowerCase();
    return assignmentGameId === gameId;
  };

  const assignmentForGame = (game: SupportedGame | null | undefined) => {
    if (!game) return undefined;
    return currentControllerConfig?.profileAssignments.find((assignment) =>
      profileAssignmentMatchesGame(assignment, game)
    );
  };

  const defaultProfileIdForGame = (game: SupportedGame | null | undefined) => {
    if (!game) {
      return (
        profiles.find((profile) => profile.id === 'global')?.id ??
        profiles.find((profile) => profile.scope === 'Global')?.id ??
        profiles.find((profile) => profile.id === activeProfileId && profile.scope !== 'Game')?.id ??
        profiles[0]?.id ??
        ''
      );
    }
    const scopedProfile = profiles.find((profile) => profile.scope === 'Game' && profile.gameId === game.gameId);
    if (scopedProfile) return scopedProfile.id;
    const assignment = assignmentForGame(game);
    if (assignment?.profileId && profiles.some((profile) => profile.id === assignment.profileId)) {
      return assignment.profileId;
    }
    const builtInProfileId = builtInProfileIdForGame(game);
    if (builtInProfileId) return builtInProfileId;
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
      .filter((profile) => {
        if (profile.scope !== 'Game') return true;
        if (game && profile.gameId === game.gameId) return true;
        return profile.id === selectedProfileId || profile.id === activeId;
      })
      .map((profile, index) => ({ profile, index }))
      .sort((left, right) => {
        const rank = (profile: ProfileSummary) => {
          if (profile.id === selectedProfileId) return 0;
          if (game && profile.scope === 'Game' && profile.gameId === game.gameId) return 1;
          if (game && profile.id === defaultProfileId) return 2;
          if (profile.scope === 'Global' && !game) return 1;
          if (profile.id === activeId) return 3;
          if (profile.scope === 'Built-in') return 4;
          return 5;
        };
        return rank(left.profile) - rank(right.profile) || left.index - right.index;
      })
      .map(({ profile }) => profile);

  const profileContextTag = (profile: ProfileSummary) => {
    if (profile.scope === 'Game') return 'game';
    if (profileContextGame && profile.id === profileContextDefaultProfileId) return 'recommended';
    if (profile.id === activeProfileId) return 'active';
    return profile.builtIn ? (profile.scope === 'Global' ? 'stock global' : 'built-in') : profile.scope.toLowerCase();
  };

  const gameLauncherLabel = (game: SupportedGame) =>
    [
      game.name,
      game.appId ? `Steam ${game.appId}` : '',
      game.running ? 'running' : game.installed ? 'installed' : 'not installed'
    ]
      .filter(Boolean)
      .join(' / ');

  const selectTargetController = (controllerId: string) => {
    if (!controllerId || controllerId === selectedControllerId) return;
    selectedControllerId = controllerId;
    configLoadedFor = '';
    stopTriggerInputPolling();
  };

  const openAddGameDialog = async () => {
    addGameOpen = true;
    addGameError = '';
    addGameLoading = true;
    try {
      const response = await getSteamLibrary();
      addGameEntries = response.games;
    } catch (caught) {
      addGameError = caught instanceof Error ? caught.message : 'Unable to load Steam library.';
      addGameEntries = [];
    } finally {
      addGameLoading = false;
    }
  };

  const closeAddGameDialog = () => {
    if (addGameBusyAppId) return;
    addGameOpen = false;
    addGameError = '';
  };

  const addGameFromLibrary = async (entry: SteamLibraryEntry, processNames?: string[]) => {
    if (addGameBusyAppId) return;
    addGameBusyAppId = entry.appId;
    addGameError = '';
    try {
      const response = await addCustomGame(entry.appId, processNames ?? []);
      await refresh();
      addGameEntries = addGameEntries.map((item) =>
        item.appId === entry.appId ? { ...item, alreadyInCatalog: true } : item
      );
      setApplyMessage(`Added ${response.game.name}. Tune a profile, and DSCC will auto-load it when the game launches.`);
    } catch (caught) {
      addGameError = caught instanceof Error ? caught.message : 'Unable to add game.';
    } finally {
      addGameBusyAppId = '';
    }
  };

  const updateRibbonMenuPositions = () => {
    if (scopeTriggerEl) {
      const rect = scopeTriggerEl.getBoundingClientRect();
      scopeMenuPos = {
        left: rect.left,
        top: rect.bottom + 6,
        minWidth: Math.max(240, rect.width)
      };
    }
    if (profileTriggerEl) {
      const rect = profileTriggerEl.getBoundingClientRect();
      profileMenuPos = {
        left: rect.left,
        top: rect.bottom + 6,
        minWidth: Math.max(260, rect.width)
      };
    }
  };

  const toggleScopePicker = () => {
    if (!scopePickerOpen) updateRibbonMenuPositions();
    scopePickerOpen = !scopePickerOpen;
    if (scopePickerOpen) profilePickerOpen = false;
  };

  const toggleProfilePicker = () => {
    if (!profilePickerOpen) updateRibbonMenuPositions();
    profilePickerOpen = !profilePickerOpen;
    if (profilePickerOpen) scopePickerOpen = false;
  };

  const closeRibbonPickers = () => {
    scopePickerOpen = false;
    profilePickerOpen = false;
  };

  const handleRibbonPickerWindowChange = () => {
    if (scopePickerOpen || profilePickerOpen) updateRibbonMenuPositions();
  };

  const handleRibbonPickerKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape' && (scopePickerOpen || profilePickerOpen)) {
      event.preventDefault();
      closeRibbonPickers();
    }
  };

  const handleRibbonPickerDocumentClick = (event: MouseEvent) => {
    if (!scopePickerOpen && !profilePickerOpen) return;
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (target.closest('.dm-ribbon-picker-host')) return;
    closeRibbonPickers();
  };

  const pickScopeGlobal = async () => {
    closeRibbonPickers();
    if (selectedTuningScope !== 'global') await selectGlobalTuning();
  };

  const pickScopeGame = async (game: SupportedGame) => {
    closeRibbonPickers();
    if (selectedTuningScope === 'game' && selectedTuningGameId === game.gameId) return;
    await selectTuningGame(game);
  };

  const pickProfile = async (profileId: string) => {
    closeRibbonPickers();
    if (!profileId || profileId === selectedOverrideProfileId) return;
    await selectProfileForScope(profileId);
  };

  const beginControllerRename = (item: ControllerStatus) => {
    controllerRenameId = item.id;
    controllerRenameName = item.name || controllerModelText(item);
  };

  const cancelControllerRename = () => {
    controllerRenameId = '';
    controllerRenameName = '';
  };

  const submitControllerRename = async () => {
    const id = controllerRenameId;
    const name = controllerRenameName.trim();
    if (!id || !name || controllerRenameBusy) return;
    controllerRenameBusy = true;
    try {
      const updated = await updateControllerName(id, name);
      if (snapshot) {
        snapshot = {
          ...snapshot,
          controllers: snapshot.controllers.map((item) => (item.id === updated.id ? { ...item, name: updated.name } : item))
        };
      }
      cancelControllerRename();
      await refresh();
      showToast(`Renamed controller to ${updated.name}`, 'success');
    } catch (caught) {
      showToast(caught instanceof Error ? caught.message : 'Unable to rename controller.', 'error');
    } finally {
      controllerRenameBusy = false;
    }
  };

  const handleControllerRenameKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submitControllerRename();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      cancelControllerRename();
    }
  };

  const selectGlobalTuning = async () => {
    selectedTuningScope = 'global';
    selectedTuningGameId = '';
    const profileId =
      profiles.find((profile) => profile.id === 'global')?.id ??
      profiles.find((profile) => profile.scope === 'Global')?.id ??
      profiles.find((profile) => profile.scope !== 'Game' && profile.id === activeProfileId)?.id ??
      profiles[0]?.id ??
      '';
    selectedOverrideProfileId = profileId;
    activeView = 'haptics';
    setViewHash('haptics');
    if (profileId) await selectProfileForScope(profileId, null, 'Global Profile');
  };

  const selectTuningGame = async (game: SupportedGame) => {
    selectedTuningScope = 'game';
    selectedTuningGameId = game.gameId;
    const preferredProfileId = defaultProfileIdForGame(game);
    if (preferredProfileId) selectedOverrideProfileId = preferredProfileId;
    activeView = 'haptics';
    setViewHash('haptics');
    if (preferredProfileId) await selectProfileForScope(preferredProfileId, game.gameId, game.name);
  };

  const selectProfileForScope = async (
    profileId: string,
    gameId: string | null = profileContextGameId,
    scopeLabel: string = profileContextLabel
  ) => {
    const profile = profiles.find((item) => item.id === profileId);
    if (!snapshot || !profile) return;
    selectedOverrideProfileId = profileId;
    try {
      const resolution = await setProfileOverride({
        controllerId: controller?.id ?? null,
        gameId,
        profileId
      });
      snapshot = { ...snapshot, profileResolution: resolution };
      await loadProfileConfigForEditor(profile);
      await refresh();
      setProfileOverrideMessage(`${profile.name} selected for ${scopeLabel}`, 'success');
    } catch (caught) {
      setProfileOverrideMessage(caught instanceof Error ? caught.message : 'Unable to select profile.', 'error');
      await refresh();
    }
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
  const normalizeForzaBodyRumbleMode = (mode: string | undefined | null): ForzaBodyRumbleMode =>
    mode === 'dscc_full_control' ? 'dscc_full_control' : 'native_passthrough';

  const editableConfigFromController = (config: ControllerConfiguration): EditableControllerConfig => ({
    inputMode: config.inputMode,
    trigger: config.trigger,
    lightbar: config.lightbar,
    forza: config.forza,
    sticks: config.sticks,
    buttons: normalizeButtonAssignments(config.buttons, config.model === 'DualSense Edge' || controller?.family === 'DualSense Edge'),
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
        l2CurvePoints: normalizeTriggerCurvePoints(config.trigger.l2CurvePoints, normalizeTriggerCurve(config.trigger.l2Curve, defaultTriggerCurve('l2'))),
        r2CurvePoints: normalizeTriggerCurvePoints(config.trigger.r2CurvePoints, normalizeTriggerCurve(config.trigger.r2Curve, defaultTriggerCurve('r2'))),
        effect: config.trigger.effect,
        intensity: config.trigger.intensity,
        vibration: config.trigger.vibration,
        vibrationMode: config.trigger.vibrationMode ?? 'Balanced'
      },
      lightbar: {
        enabled: config.lightbar?.enabled ?? true,
        color: config.lightbar?.color ?? '#4cc9f0',
        rpmColor: config.lightbar?.rpmColor ?? '#ff3a2e',
        brightness: normalizeTriggerPercent(config.lightbar?.brightness ?? 72)
      },
      forza: {
        bodyRumbleMode: normalizeForzaBodyRumbleMode(config.forza?.bodyRumbleMode),
        effects: normalizeForzaEffects(config.forza?.effects).map((effect) => ({
          id: effect.id,
          enabled: effect.enabled,
          intensity: forzaIntensityPercent(effect.intensity),
          route: effect.route
        }))
      },
      sticks: config.sticks,
      buttons: normalizeButtonAssignments(config.buttons, controller?.family === 'DualSense Edge'),
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

  const setForzaBodyRumbleMode = (mode: ForzaBodyRumbleMode) => {
    forzaBodyRumbleMode = normalizeForzaBodyRumbleMode(mode);
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

  const vibrationIntensityPercent = (value: string) => {
    if (value === 'Off') return 0;
    if (value === 'Low') return 48;
    if (value === 'High') return 100;
    return 82;
  };

  const vibrationModeRequest = (value: string) =>
    vibrationModeOptions.find((option) => option.label === value)?.mode ?? 'balanced';

  const triggerRangeValuesFor = (fromRaw: number | string, toRaw: number | string) => {
    const from = normalizeTriggerPercent(fromRaw);
    const to = Math.max(from, normalizeTriggerPercent(toRaw));
    return { from, to, width: Math.max(0, to - from) };
  };

  const triggerRangeValues = (side: TriggerSide) => {
    return side === 'l2' ? triggerRangeValuesFor(l2From, l2To) : triggerRangeValuesFor(r2From, r2To);
  };

  const triggerRangeUnitValuesFor = (fromRaw: number | string, toRaw: number | string) => {
    const range = triggerRangeValuesFor(fromRaw, toRaw);
    const start = range.from / 100;
    const end = Math.max(start + 0.01, range.to / 100);
    return { start, end };
  };

  const scaledUnitForGraph = (value: number, scalar: number) => clampUnit(value * scalar);
  const signalCurveForGraph = (input: number, inputMin: number, inputMax: number, outputMin: number, outputMax: number, exponent: number) => {
    if (inputMin === inputMax || exponent <= 0) return outputMin;
    const ratio = clampUnit((input - inputMin) / (inputMax - inputMin));
    return outputMin + (outputMax - outputMin) * Math.pow(ratio, exponent);
  };

  const endstopWallPosition = (start: number, end: number) => clamp(end - FORZA_ENDSTOP_WALL_OFFSET, start, end);
  const brakeOvertravelGuardActive = (end: number) => end >= FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION;
  const brakeOvertravelWallPosition = (start: number, end: number) =>
    brakeOvertravelGuardActive(end)
      ? clamp(Math.max(FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION, end - FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET), start, end)
      : endstopWallPosition(start, end);
  const brakeOvertravelRampStart = (start: number, wall: number) =>
    clamp(wall - FORZA_BRAKE_OVERTRAVEL_RAMP_WIDTH, start, wall);
  const throttleOvertravelGuardActive = (end: number) => end >= FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION;
  const throttleOvertravelWallPosition = (start: number, end: number) =>
    throttleOvertravelGuardActive(end)
      ? clamp(Math.min(end, FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION), start, end)
      : endstopWallPosition(start, end);
  const throttleOvertravelRampStart = (start: number, wall: number) =>
    clamp(Math.round((wall - FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH) * 1000) / 1000, start, wall);
  const routeHasL2 = (route: ForzaEffectRoute) => route === 'l2' || route === 'both_triggers' || route === 'body_and_triggers';
  const routeHasR2 = (route: ForzaEffectRoute) =>
    route === 'r2' || route === 'both_triggers' || route === 'body_and_triggers' || route === 'r2_and_body';
  const forzaEffectScalarForGraph = (effect: ForzaEffectConfiguration | undefined) =>
    effect?.enabled ? clampForzaIntensity(effect.intensity) / 100 : 0;
  const forzaEffectForGraph = (id: string, effects: ForzaEffectConfiguration[]) =>
    effects.find((effect) => effect.id === id) ?? DEFAULT_FORZA_EFFECT_BY_ID.get(id);

  const forzaTriggerForceModelFor = (
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

  const forzaTriggerCurveValueFor = (side: TriggerSide, position: number, model: ForzaTriggerForceModel | null) => {
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

  const baseTriggerCurveValueFromParts = (
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

  const triggerCurveValueFor = (
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

  const triggerCurveValue = (side: TriggerSide, position: number) =>
    side === 'l2'
      ? triggerCurveValueFor(side, position, l2From, l2To, l2Curve, l2CurvePoints, defaultTriggerCurve('l2'), triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects)
      : triggerCurveValueFor(side, position, r2From, r2To, r2Curve, r2CurvePoints, defaultTriggerCurve('r2'), triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects);

  const triggerCurvePathFor = (
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

  const curveControlPointsFor = (
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

  const triggerCurveShapeView = (
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

  const triggerCurveLiveView = (
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

  $: l2CurveShape = triggerCurveShapeView('l2', l2From, l2To, l2Curve, l2CurvePoints, defaultTriggerCurve('l2'), triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects);
  $: r2CurveShape = triggerCurveShapeView('r2', r2From, r2To, r2Curve, r2CurvePoints, defaultTriggerCurve('r2'), triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects);
  $: l2CurveLive = triggerCurveLiveView('l2', l2From, l2To, l2Curve, l2CurvePoints, defaultTriggerCurve('l2'), l2LivePress, triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects);
  $: r2CurveLive = triggerCurveLiveView('r2', r2From, r2To, r2Curve, r2CurvePoints, defaultTriggerCurve('r2'), r2LivePress, triggerEffect, triggerIntensity, triggerCurveDisplayMode, forzaEffects);

  const triggerPressLabel = (value: number) => `${Math.round(clampUnit(value) * 100)}%`;
  const showTriggerPress = (_side: 'l2' | 'r2', value: number) =>
    baseFeelTestActive || clampUnit(value) > 0.01;

  const intensityTooltip = (meta: ForzaEffectMeta, intensity: number) =>
    `${meta.label} intensity is ${forzaIntensityPercent(intensity)}% (${clampForzaIntensity(intensity)} / 255 raw). This scales trigger, rumble, or LED output depending on signal and route.`;

  const routeTooltip = (route: ForzaEffectRoute) => routeTooltips[route] ?? 'Selects where DSCC sends this telemetry effect.';

  const brakeOvertravelWallPoint = (end: number) =>
    Math.round(end >= FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION * 100
      ? Math.min(end, Math.max(FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION * 100, end - FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET * 100))
      : Math.max(0, end - FORZA_ENDSTOP_WALL_OFFSET * 100));
  const throttleOvertravelWallPoint = (end: number) =>
    Math.round(end >= FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION * 100
      ? Math.min(end, FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION * 100)
      : Math.max(0, end - FORZA_ENDSTOP_WALL_OFFSET * 100));
  const throttleOvertravelRampPoint = (end: number) => {
    if (end < FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION * 100) return Math.max(0, end - 3);
    const wall = throttleOvertravelWallPoint(end);
    return Math.round(Math.min(end, Math.max(0, wall - FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH * 100)));
  };

  const triggerRangeTooltip = (side: 'L2' | 'R2', edge: 'from' | 'to', value: number) =>
    edge === 'from'
      ? `${side} starts building force at ${value}% trigger travel. Raising this creates more free travel before resistance begins.`
      : side === 'L2'
        ? `${side} max resistance begins near ${brakeOvertravelWallPoint(value)}% and holds through the end of travel, while ABS/handbrake priority effects can still take over.`
        : `${side} stays light first, ramps from about ${throttleOvertravelRampPoint(value)}%, then holds max resistance from about ${throttleOvertravelWallPoint(value)}% through full travel unless shift/rev priority effects take over.`;

  const triggerCurveTooltip = (side: 'L2' | 'R2', value: number) =>
    `${side} curve is ${value.toFixed(2)}. Drag the dots for a custom response, or move this slider to regenerate a smooth exponent curve.`;

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

  const curvePointFromGraphPoint = (side: TriggerSide, input: number, output: number) => {
    const range = triggerRangeValues(side);
    const start = range.from / 100;
    const end = Math.max(start + 0.01, range.to / 100);
    let activeTravel = clamp((input - start) / (end - start), 0.01, 0.99);
    let normalizedOutput = output;

    if (triggerCurveDisplayMode === 'forza') {
      const model =
        side === 'l2'
          ? forzaTriggerForceModelFor(side, l2From, l2To, l2Curve, l2CurvePoints, defaultTriggerCurve(side), triggerEffect, triggerIntensity, forzaEffects)
          : forzaTriggerForceModelFor(side, r2From, r2To, r2Curve, r2CurvePoints, defaultTriggerCurve(side), triggerEffect, triggerIntensity, forzaEffects);
      if (model && model.normalForce > model.baselineForce) {
        const editableEnd = model.rampStart ?? model.wall;
        const editableInput = clamp(input, model.start + 0.0001, Math.max(model.start + 0.0001, editableEnd - 0.0001));
        activeTravel = clamp((editableInput - model.start) / (editableEnd - model.start), 0.01, 0.99);
        normalizedOutput = clamp((Math.min(output, model.normalForce) - model.baselineForce) / (model.normalForce - model.baselineForce), 0.01, 0.99);
      }
    } else {
      const strength = triggerStrengthScalar();
      normalizedOutput = clamp(strength > 0 ? output / strength : output, 0.01, 0.99);
    }

    return {
      input: normalizeTriggerPercent(activeTravel * 100),
      output: normalizeTriggerPercent(normalizedOutput * 100)
    };
  };

  const pointsForSide = (side: TriggerSide) => (side === 'l2' ? l2CurvePoints : r2CurvePoints);
  const setPointsForSide = (side: TriggerSide, points: TriggerCurvePoint[]) => {
    const normalized = normalizeTriggerCurvePoints(points, side === 'l2' ? l2Curve : r2Curve);
    if (side === 'l2') {
      l2CurvePoints = normalized;
    } else {
      r2CurvePoints = normalized;
    }
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
  };

  const setCurvePoint = (side: TriggerSide, index: number, point: TriggerCurvePoint) => {
    const current = normalizeTriggerCurvePoints(pointsForSide(side), side === 'l2' ? l2Curve : r2Curve);
    if (index <= 0 || index >= current.length - 1) return index;
    const previous = current[index - 1];
    const next = current[index + 1];
    current[index] = {
      input: normalizeTriggerPercent(clamp(point.input, previous.input + 1, next.input - 1)),
      output: normalizeTriggerPercent(point.output)
    };
    setPointsForSide(side, current);
    return index;
  };

  const addOrSelectCurvePoint = (side: TriggerSide, point: TriggerCurvePoint) => {
    const current = normalizeTriggerCurvePoints(pointsForSide(side), side === 'l2' ? l2Curve : r2Curve);
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
      return setCurvePoint(side, nearest, point);
    }

    const nextPoints = [...current, point].sort((a, b) => a.input - b.input);
    const index = Math.max(1, Math.min(nextPoints.length - 2, nextPoints.findIndex((candidate) => candidate === point)));
    setPointsForSide(side, nextPoints);
    return index;
  };

  const addCurvePoint = (side: TriggerSide) => {
    const current = normalizeTriggerCurvePoints(pointsForSide(side), side === 'l2' ? l2Curve : r2Curve);
    if (current.length >= TRIGGER_CURVE_POINT_MAX) return;

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
    setPointsForSide(side, [...current, { input, output }]);
  };

  const removeCurvePoint = (side: TriggerSide) => {
    const current = normalizeTriggerCurvePoints(pointsForSide(side), side === 'l2' ? l2Curve : r2Curve);
    if (current.length <= TRIGGER_CURVE_POINT_MIN) return;

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
    setPointsForSide(side, current.filter((_, index) => index !== removeIndex));
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
    let pointIndex = -1;

    const applyPoint = (pointerEvent: PointerEvent) => {
      const { x, output } = curveGraphPointFromPointer(pointerEvent, target);
      const point = curvePointFromGraphPoint(side, x, output);
      pointIndex = pointIndex < 0 ? addOrSelectCurvePoint(side, point) : setCurvePoint(side, pointIndex, point);
      curveDragPoint = { side, index: pointIndex };
      setCurveHover(side, x);
    };

    const stopDrag = () => {
      curveDragSide = null;
      curveDragPoint = null;
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

  const handleCurvePointPointer = (event: PointerEvent, side: TriggerSide, index: number) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const frame = (event.currentTarget as SVGElement).closest('.dm-curve-frame') as HTMLElement | null;
    if (!frame) return;
    curveDragSide = side;
    curveDragPoint = { side, index };
    frame.setPointerCapture(event.pointerId);

    const applyPoint = (pointerEvent: PointerEvent) => {
      const { x, output } = curveGraphPointFromPointer(pointerEvent, frame);
      setCurvePoint(side, index, curvePointFromGraphPoint(side, x, output));
      setCurveHover(side, x);
    };
    const stopDrag = () => {
      curveDragSide = null;
      curveDragPoint = null;
      if (frame.hasPointerCapture(event.pointerId)) frame.releasePointerCapture(event.pointerId);
      frame.removeEventListener('pointermove', applyPoint);
      frame.removeEventListener('pointerup', stopDrag);
      frame.removeEventListener('pointercancel', stopDrag);
    };

    frame.addEventListener('pointermove', applyPoint);
    frame.addEventListener('pointerup', stopDrag);
    frame.addEventListener('pointercancel', stopDrag);
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
    l2CurvePoints = normalizeTriggerCurvePoints(config.trigger.l2CurvePoints, l2Curve);
    r2CurvePoints = normalizeTriggerCurvePoints(config.trigger.r2CurvePoints, r2Curve);
    triggerEffect = config.trigger.effect;
    triggerIntensity = config.trigger.intensity;
    vibrationIntensity = config.trigger.vibration;
    vibrationMode = config.trigger.vibrationMode ?? 'Balanced';
    lightbarEnabled = config.lightbar?.enabled ?? true;
    lightbarColor = config.lightbar?.color ?? '#4cc9f0';
    rpmColor = config.lightbar?.rpmColor ?? '#ff3a2e';
    lightbarBrightness = config.lightbar?.brightness ?? 72;
    forzaBodyRumbleMode = normalizeForzaBodyRumbleMode(config.forza?.bodyRumbleMode);
    forzaEffects = normalizeForzaEffects(config.forza?.effects);
  };
  const applyControllerConfig = (config: ControllerConfiguration, updateProfileBaseline = true) => {
    currentControllerConfig = config;
    applyEditableConfig(config);
    if (updateProfileBaseline) profileSaveBaselineSignature = profileConfigSignature(buildControllerConfig());
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

  const loadEdgeProfiles = async (controllerId: string, force = false) => {
    if (!force && edgeProfilesLoadedFor === controllerId && (edgeProfiles || edgeProfilesLoading)) return;
    edgeProfilesLoadedFor = controllerId;
    edgeProfilesLoading = true;
    edgeProfilesError = '';
    try {
      edgeProfiles = await getEdgeProfiles(controllerId);
    } catch (caught) {
      edgeProfiles = null;
      edgeProfilesError = caught instanceof Error ? caught.message : 'Unable to read Edge onboard slots.';
    } finally {
      edgeProfilesLoading = false;
    }
  };

  const resetEdgeProfiles = () => {
    edgeProfilesLoadedFor = '';
    edgeProfiles = null;
    edgeProfilesLoading = false;
    edgeProfilesBusySlot = '';
    edgeProfilesError = '';
  };

  const edgeSlotStatus = (slot: EdgeProfileSlot) => {
    if (slot.state === 'default') return 'default';
    if (slot.hardwareSynced) return 'on controller';
    if (slot.staged) return 'staged';
    return slot.state.replaceAll('_', ' ');
  };

  const edgeSlotName = (slot: EdgeProfileSlot) => slot.name || slot.staged?.name || 'Empty Slot';
  const edgeSlotsReadTooltip =
    'Reads onboard slots from a DualSense Edge over USB or Bluetooth when Windows exposes HID feature-report access. Fallback controllers only show local staged status.';
  const edgeSlotInfoTooltip = (slot: EdgeProfileSlot) => {
    if (slot.state === 'default') {
      return 'The Fn + Triangle default profile is readable but not writable from DSCC.';
    }
    if (slot.hardwareSynced) {
      return `${slot.shortcut} is currently synced with controller memory.`;
    }
    if (slot.staged) {
      return `${slot.shortcut} has local staged settings that still need a controller hardware write.`;
    }
    return `${slot.shortcut} has no synced profile data available yet. Connect over USB or Bluetooth and read slots to refresh controller memory state.`;
  };
  const edgeSlotWriteTooltip = (slot: EdgeProfileSlot) =>
    edgeProfiles?.supportState === 'read_write'
      ? `Writes the current trigger ranges, lightbar color, stick presets, and supported button remaps to ${slot.shortcut}. Live telemetry effects still require DSCC to be running.`
      : `Stages the current trigger ranges, lightbar color, stick presets, and supported button remaps for ${slot.shortcut}. Connect the DualSense Edge over USB to sync controller memory.`;
  const edgeSlotWriteLabel = () => (edgeProfiles?.supportState === 'read_write' ? 'Write' : 'Stage');

  const edgeProfileNameForSlot = (slot: EdgeProfileSlot) => {
    const profileName = activeProfileHeaderName || activeProfile?.name || 'DSCC Profile';
    return `${profileName} ${slot.shortcut.replace('Fn + ', '')}`.trim().slice(0, 64);
  };

  const writeCurrentConfigToEdgeSlot = async (slot: EdgeProfileSlot) => {
    if (!controller || !slot.editable || edgeProfilesBusySlot) return;
    edgeProfilesBusySlot = slot.slotId;
    edgeProfilesError = '';
    try {
      const config = buildControllerConfig();
      const response = await writeEdgeProfile(controller.id, slot.slotId, {
        name: edgeProfileNameForSlot(slot),
        trigger: config.trigger,
        lightbar: config.lightbar,
        sticks: config.sticks,
        buttons: config.buttons
      });
      showToast(response.message, response.accepted ? 'success' : 'error');
      await loadEdgeProfiles(controller.id, true);
    } catch (caught) {
      edgeProfilesError = caught instanceof Error ? caught.message : 'Unable to write Edge onboard slot.';
      showToast(edgeProfilesError, 'error');
    } finally {
      edgeProfilesBusySlot = '';
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
      l2CurvePoints: triggerCurvePointsFromCurve(1.35),
      r2CurvePoints: triggerCurvePointsFromCurve(2.25),
      effect: 'Adaptive resistance',
      intensity: 'Strong (Standard)',
      vibration: 'Medium',
      vibrationMode: 'Balanced'
    },
    lightbar: {
      enabled: true,
      color: '#4cc9f0',
      rpmColor: '#ff3a2e',
      brightness: 72
    },
    forza: {
      bodyRumbleMode: 'native_passthrough',
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
    buttons: defaultButtonAssignments(controller?.family === 'DualSense Edge'),
    profileAssignments: []
  });

  const builtInProfileConfig = (profileId: string): EditableControllerConfig => {
    const base = buildDefaultControllerConfig();
    if (profileId === 'global') {
      return {
        ...base,
        profileAssignments: currentControllerConfig?.profileAssignments ?? []
      };
    }

    return {
      ...base,
      trigger: baseForzaTriggerDefaults(),
      forza: {
        bodyRumbleMode: 'native_passthrough',
        effects: forzaPresetEffects(profileId === 'forza-horizon-immersive' ? 'immersive' : 'base')
      },
      profileAssignments: currentControllerConfig?.profileAssignments ?? []
    };
  };

  const editableConfigFromProfileExport = (config: NonNullable<ExportedProfile['config']>): EditableControllerConfig => ({
    ...buildDefaultControllerConfig(),
    inputMode: config.inputMode,
    trigger: config.trigger,
    lightbar: config.lightbar,
    forza: config.forza,
    sticks: config.sticks,
    buttons: normalizeButtonAssignments(config.buttons, controller?.family === 'DualSense Edge'),
    profileAssignments: currentControllerConfig?.profileAssignments ?? []
  });

  const loadProfileConfigForEditor = async (profile: ProfileSummary) => {
    let config: EditableControllerConfig | null = null;
    if (profile.builtIn) {
      config = builtInProfileConfig(profile.id);
    } else {
      const exported = await exportProfile(profile.id);
      config = exported.config ? editableConfigFromProfileExport(exported.config) : buildControllerConfig();
    }

    applyEditableConfig(config);
    profileSaveBaselineSignature = profileConfigSignature(buildControllerConfig());
  };

  const baseForzaTriggerDefaults = (): EditableControllerConfig['trigger'] => ({
    sameRange: false,
    l2From: 0,
    l2To: 100,
    r2From: 4,
    r2To: 100,
    l2Curve: defaultTriggerCurve('l2'),
    r2Curve: defaultTriggerCurve('r2'),
    l2CurvePoints: triggerCurvePointsFromCurve(defaultTriggerCurve('l2')),
    r2CurvePoints: triggerCurvePointsFromCurve(defaultTriggerCurve('r2')),
    effect: 'Adaptive resistance',
    intensity: 'Strong (Standard)',
    vibration: 'Medium',
    vibrationMode: 'Balanced'
  });

  const applyTriggerConfig = (trigger: EditableControllerConfig['trigger']) => {
    l2From = normalizeTriggerPercent(trigger.l2From);
    l2To = Math.max(l2From, normalizeTriggerPercent(trigger.l2To));
    r2From = normalizeTriggerPercent(trigger.r2From);
    r2To = Math.max(r2From, normalizeTriggerPercent(trigger.r2To));
    l2Curve = normalizeTriggerCurve(trigger.l2Curve, defaultTriggerCurve('l2'));
    r2Curve = normalizeTriggerCurve(trigger.r2Curve, defaultTriggerCurve('r2'));
    l2CurvePoints = normalizeTriggerCurvePoints(trigger.l2CurvePoints, l2Curve);
    r2CurvePoints = normalizeTriggerCurvePoints(trigger.r2CurvePoints, r2Curve);
    triggerEffect = trigger.effect;
    triggerIntensity = trigger.intensity;
    vibrationIntensity = trigger.vibration;
    vibrationMode = trigger.vibrationMode ?? 'Balanced';
  };

  const resetTriggerCurvesToProfileDefaults = () => {
    applyTriggerConfig(baseForzaTriggerDefaults());
    scheduleBaseFeelTestRefresh();
    scheduleLiveControllerConfigSync();
    const profileLabel = activeProfile?.builtIn ? activeProfile.name : 'Base';
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
        l2CurvePoints: normalizeTriggerCurvePoints(l2CurvePoints, l2Curve),
        r2CurvePoints: normalizeTriggerCurvePoints(r2CurvePoints, r2Curve),
        effect: triggerEffect,
        intensity: triggerIntensity,
        vibration: vibrationIntensity,
        vibrationMode
      },
      lightbar: {
        enabled: lightbarEnabled,
        color: lightbarColor,
        rpmColor,
        brightness: lightbarBrightness
      },
      forza: {
        bodyRumbleMode: normalizeForzaBodyRumbleMode(forzaBodyRumbleMode),
        effects: normalizeForzaEffects(forzaEffects)
      },
      buttons: normalizeButtonAssignments(base.buttons, controller?.family === 'DualSense Edge')
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

  const setVibrationMode = (value: string) => {
    vibrationMode = value;
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
    const selectedProfile = profiles.find((profile) => profile.id === (selectedOverrideProfileId || activeProfileId));
    const profileId = selectedProfile && !selectedProfile.builtIn
      ? defaultProfileIdForGame(profileContextGame) || 'global'
      : selectedProfile?.id ?? defaultProfileIdForGame(profileContextGame);
    if (!profileId) {
      setApplyMessage('No active profile selected');
      return;
    }
    const profileName = profiles.find((profile) => profile.id === profileId)?.name ?? activeProfileName;

    try {
      await selectProfileForScope(profileId);
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

  const setSupportBundleMessage = (message: string, tone: ToastTone = toastToneForMessage(message, 'info')) => {
    supportBundleMessage = message;
    supportBundleTone = tone;
    showToast(message, tone);
  };

  const sanitizeSupportText = (value: string | undefined | null) =>
    (value ?? '').replace(/[A-Z]:\\[^"'\s]+/gi, '[local-path]').replace(/\/(?:home|Users)\/[^"'\s]+/gi, '[local-path]');

  const buildUiSupportBundle = (agentBundleError?: string): SupportBundle => ({
    schema: 'dev.dscc.support-bundle.ui.v1',
    generatedAt: new Date().toISOString(),
    source: 'web-ui-sanitized-fallback',
    privacy: {
      sanitized: true,
      omitted: ['raw HID paths', 'raw controller hardware IDs', 'serial numbers', 'Bluetooth addresses', 'Steam install paths']
    },
    app: {
      version: status?.version ?? 'unknown',
      health: status?.health ?? 'unavailable',
      uptime: status?.uptime ?? 'unknown',
      apiBinding: listenOnAllInterfaces ? 'lan' : 'loopback',
      activeAdapter: status?.activeAdapter ?? 'None'
    },
    environment: {
      userAgent: navigator.userAgent,
      language: navigator.language,
      viewport: `${window.innerWidth}x${window.innerHeight}`
    },
    selectedContext: {
      scope: selectedTuningScope,
      game: selectedTuningGame ? { gameId: selectedTuningGame.gameId, name: selectedTuningGame.name } : null,
      profile: activeProfile ? { scope: activeProfile.scope, builtIn: activeProfile.builtIn, name: activeProfile.name } : null
    },
    controllers: controllers.map((item, index) => ({
      index,
      family: item.family,
      transport: item.transport,
      connected: item.connected,
      batteryState: item.batteryState,
      battery: item.battery,
      permission: item.permission,
      diagnosticState: item.diagnosticState,
      capabilities: item.capabilities
    })),
    adapters: snapshot?.adapters.map((item) => ({
      id: item.id,
      name: item.name,
      state: item.state,
      packetRateHz: item.packetRateHz,
      setupHint: item.setupHint
    })) ?? [],
    diagnostics,
    partialErrors: snapshot?.partialErrors ?? [],
    gameDetection: {
      activeGameName: snapshot?.gameDetection.activeGameName ?? null,
      source: snapshot?.gameDetection.source ?? 'unknown',
      confidence: snapshot?.gameDetection.confidence ?? 0,
      moduleId: snapshot?.gameDetection.moduleId ?? null,
      adapterId: snapshot?.gameDetection.adapterId ?? null,
      supportedGames: supportedGames.map((game) => ({
        gameId: game.gameId,
        name: game.name,
        installed: game.installed,
        running: game.running,
        supportLevel: game.supportLevel
      }))
    },
    profileResolution: {
      reason: snapshot?.profileResolution.reason ?? 'unknown',
      validation: snapshot?.profileResolution.validation ?? 'unknown',
      detectedGameId: snapshot?.profileResolution.detectedGameId ?? null,
      activeAdapterId: snapshot?.profileResolution.activeAdapterId ?? null
    },
    steamInput: {
      running: snapshot?.steamInput.running ?? false,
      available: snapshot?.steamInput.available ?? false,
      layoutCount: snapshot?.steamInput.layouts.length ?? 0,
      warnings: snapshot?.steamInput.warnings.map(sanitizeSupportText) ?? []
    },
    effectState: effectState
      ? {
        reason: effectState.reason,
        dryRun: effectState.dryRun,
        hardwareOutputEnabled: effectState.hardwareOutputEnabled,
        warnings: effectState.warnings.map(sanitizeSupportText),
        parityEffects: effectState.parityEffects
      }
      : null,
    logs: logs.slice(-40).map((entry) => ({
      level: entry.level,
      time: entry.time,
      source: entry.source,
      message: sanitizeSupportText(entry.message)
    })),
    agentBundleError: agentBundleError ? sanitizeSupportText(agentBundleError) : undefined
  });

  const loadSupportBundle = async (): Promise<{ bundle: SupportBundle; fallback: boolean }> => {
    try {
      return { bundle: await getSupportBundle(), fallback: false };
    } catch (caught) {
      if (!snapshot) throw caught;
      const message = caught instanceof Error ? caught.message : 'Support bundle endpoint unavailable.';
      return { bundle: buildUiSupportBundle(message), fallback: true };
    }
  };

  const supportBundleFileName = () =>
    `dscc-support-${new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)}.json`;

  const downloadSupportBundleText = (text: string) => {
    const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
    const link = document.createElement('a');
    link.href = url;
    link.download = supportBundleFileName();
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  };

  const copySupportBundle = async () => {
    if (supportBundleBusy) return;
    supportBundleBusy = 'copy';
    try {
      const { bundle, fallback } = await loadSupportBundle();
      const body = JSON.stringify(bundle, null, 2);
      if (!navigator.clipboard?.writeText) {
        downloadSupportBundleText(body);
        setSupportBundleMessage('Clipboard unavailable. Exported a sanitized support bundle instead.', 'info');
        return;
      }
      await navigator.clipboard.writeText(body);
      setSupportBundleMessage(
        fallback ? 'Copied a sanitized UI support bundle. The agent bundle endpoint was unavailable.' : 'Copied sanitized support bundle.',
        fallback ? 'info' : 'success'
      );
    } catch (caught) {
      setSupportBundleMessage(caught instanceof Error ? caught.message : 'Unable to copy support bundle.', 'error');
    } finally {
      supportBundleBusy = '';
    }
  };

  const exportSupportBundle = async () => {
    if (supportBundleBusy) return;
    supportBundleBusy = 'download';
    try {
      const { bundle, fallback } = await loadSupportBundle();
      downloadSupportBundleText(JSON.stringify(bundle, null, 2));
      setSupportBundleMessage(
        fallback ? 'Exported a sanitized UI support bundle. The agent bundle endpoint was unavailable.' : 'Exported sanitized support bundle.',
        fallback ? 'info' : 'success'
      );
    } catch (caught) {
      setSupportBundleMessage(caught instanceof Error ? caught.message : 'Unable to export support bundle.', 'error');
    } finally {
      supportBundleBusy = '';
    }
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
      await createProfile(name, { gameId: selectedTuningScope === 'game' ? profileContextGameId : null });
      newProfileName = '';
      await refresh();
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Failed to create profile');
    }
  };

  const beginRenameSelectedProfile = () => {
    if (!selectedActionProfile || selectedActionProfile.builtIn) return;
    saveAsProfileOpen = false;
    saveAsProfileName = '';
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
    if (!profile || profile.builtIn) {
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

  const beginSaveAsProfile = () => {
    if (!selectedActionProfile) {
      setApplyMessage('No profile selected', 'error');
      return;
    }
    cancelRenameProfile();
    saveAsProfileName = uniqueProfileName(`${selectedActionProfile.name} copy`);
    saveAsProfileOpen = true;
  };

  const cancelSaveAsProfile = () => {
    saveAsProfileOpen = false;
    saveAsProfileName = '';
  };

  const submitSaveAsProfile = async () => {
    const name = saveAsProfileName.trim();
    if (!selectedActionProfile || profileSaveAsBusy) {
      if (!selectedActionProfile) setApplyMessage('No profile selected', 'error');
      return;
    }
    if (!name) {
      setApplyMessage('Profile name cannot be empty', 'error');
      return;
    }
    if (profiles.some((profile) => profile.name.trim().toLowerCase() === name.toLowerCase())) {
      setApplyMessage('A profile with that name already exists', 'error');
      return;
    }

    profileSaveAsBusy = true;
    try {
      const config = buildControllerConfig();
      const created = await createProfile(name, { gameId: selectedTuningScope === 'game' ? profileContextGameId : null });
      const response = await saveProfileConfig(created.id, config);
      if (controller) {
        currentControllerConfig = await saveControllerConfig(controller.id, config);
      }
      const resolution = await setProfileOverride({
        controllerId: controller?.id ?? null,
        gameId: profileContextGameId,
        profileId: created.id
      });
      if (snapshot) snapshot = { ...snapshot, profileResolution: resolution };
      profileSaveBaselineSignature = profileConfigSignature(config);
      selectedOverrideProfileId = created.id;
      cancelSaveAsProfile();
      await refresh();
      selectedOverrideProfileId = created.id;
      setApplyMessage(response.message || `Saved ${created.name}`, 'success');
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Unable to save profile copy', 'error');
      await refresh();
    } finally {
      profileSaveAsBusy = false;
    }
  };

  const handleSaveAsProfileKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submitSaveAsProfile();
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      cancelSaveAsProfile();
    }
  };

  const deleteProfileById = async (id: string, name: string) => {
    const fallbackProfileId =
      profiles.find((profile) => profile.id === 'global')?.id ??
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

  const telemetryRateStatusText = (item: AppSnapshot['adapters'][number] | undefined) => {
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

  const gameMediaDetails = (game: SupportedGame) =>
    [
      game.appId ? `Steam ${game.appId}` : '',
      formatPlaytime(game.stats?.playtimeMinutes),
      achievementText(game),
      formatLastPlayed(game.stats?.lastPlayedUnix)
    ].filter(Boolean);

  const profileScopeCount = (game: SupportedGame) =>
    profiles.filter((profile) => profile.scope === 'Game' && profile.gameId === game.gameId).length;

  const SCOPE_ACCENT_BUILT_IN = '#3BA0FF';
  const SCOPE_ACCENT_GAME = '#5DD68C';
  const SCOPE_ACCENT_GLOBAL = '#C18BEF';
  const SCOPE_ACCENT_CUSTOM = '#E0B341';

  const profileAccentColor = (scope: ProfileSummary['scope']): string => {
    if (scope === 'Built-in') return SCOPE_ACCENT_BUILT_IN;
    if (scope === 'Game') return SCOPE_ACCENT_GAME;
    return SCOPE_ACCENT_GLOBAL;
  };

  const gameAccentColor = (game: SupportedGame | null | undefined): string =>
    game?.supportLevel === 'custom' ? SCOPE_ACCENT_CUSTOM : SCOPE_ACCENT_BUILT_IN;

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
    if (profile.schema !== 'dev.dscc.profile.v1') throw new Error('Unsupported DSCC profile schema.');
    const name = typeof profile.name === 'string' ? profile.name.trim() : '';
    if (!name) throw new Error('Profile file is missing a profile name.');

    const id = typeof profile.id === 'string' ? profile.id.trim() : '';
    const existingIds = new Set(profiles.map((item) => item.id));
    const idAvailable = Boolean(id) && !existingIds.has(id);
    return {
      id: idAvailable ? id : undefined,
      schema: profile.schema,
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

  function shouldPollTriggerInput() {
    return Boolean(
      controller?.id &&
        activeView === 'haptics' &&
        typeof window !== 'undefined' &&
        typeof document !== 'undefined' &&
        !document.hidden
    );
  }

  function syncTriggerInputPolling() {
    if (shouldPollTriggerInput()) startTriggerInputPolling();
    else stopTriggerInputPolling();
  }

  $: if (
    tuningReady &&
    typeof window !== 'undefined' &&
    (window.location.hash === '#/adaptive-triggers-haptics' || window.location.hash === '#/button-mapping')
  ) {
    const routeView = appViewFromHash();
    if (routeView !== activeView) {
      activeView = routeView;
      syncTriggerInputPolling();
    }
  }

  async function pollTriggerInput() {
    if (triggerInputBusy || !shouldPollTriggerInput()) return;
    triggerInputBusy = true;
    try {
      const input = await getControllerInput(controller?.id);
      if (input.available) {
        const nextL2 = clampUnit(input.l2);
        const nextR2 = clampUnit(input.r2);
        l2ControllerPress = nextL2;
        r2ControllerPress = nextR2;
        controllerInputFresh = true;
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
    if (!shouldPollTriggerInput()) return;
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

  const previewBodyHaptics = async () => {
    if (!snapshot) return;
    const intensity = vibrationIntensityPercent(vibrationIntensity);
    if (intensity <= 0) {
      setApplyMessage('Body haptics are off; raise Body strength to preview.');
      return;
    }

    try {
      const result = await runEffectTest(
        {
          target: 'rumble',
          mode: vibrationModeRequest(vibrationMode),
          intensity,
          durationMs: 900
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
      setApplyMessage(`${vibrationMode} body haptics previewed`);
    } catch (caught) {
      setApplyMessage(caught instanceof Error ? caught.message : 'Body haptics preview failed');
    }
  };

  const saveActiveProfile = async () => {
    if (!selectedActionProfile || profileSaveBusy) {
      if (!selectedActionProfile) setApplyMessage('No profile selected');
      return;
    }

    profileSaveBusy = true;
    try {
      const sourceProfileName = selectedActionProfile.name;
      let targetProfile = selectedActionProfile;
      let preservingStockProfile = false;
      if (targetProfile?.builtIn) {
        const name = uniqueProfileName(
          profileContextGame ? `${profileContextGame.name} ${targetProfile.name} custom` : `${targetProfile.name} custom`
        );
        targetProfile = await createProfile(name, { gameId: profileContextGameId });
        preservingStockProfile = true;
      }
      if (!targetProfile) throw new Error('No profile selected');

      const config = buildControllerConfig();
      if (controller) {
        currentControllerConfig = await saveControllerConfig(controller.id, config);
      }
      const response = await saveProfileConfig(targetProfile.id, config);
      profileSaveBaselineSignature = profileConfigSignature(config);
      const resolution = await setProfileOverride({
        controllerId: controller?.id ?? null,
        gameId: profileContextGameId,
        profileId: targetProfile.id
      });
      if (snapshot) snapshot = { ...snapshot, profileResolution: resolution };
      selectedOverrideProfileId = targetProfile.id;
      await refresh();
      selectedOverrideProfileId = targetProfile.id;
      setApplyMessage(
        preservingStockProfile
          ? `Saved ${targetProfile.name}; stock ${sourceProfileName} preserved`
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

  const startAppRuntime = () => {
    if (typeof window === 'undefined' || appRuntime?.isStarted()) return;
    appRuntime = createAppRuntime({
      fallbackPollIntervalMs: FALLBACK_POLL_INTERVAL_MS,
      snapshotInvalidationDebounceMs: SNAPSHOT_INVALIDATION_DEBOUNCE_MS,
      refresh,
      applySnapshot,
      connectSnapshotSocket: connectAppSnapshotSocket,
      onStart: () => {
        loadDismissedUpdateVersion();
        activeView = appViewFromHash();
        syncTriggerInputPolling();
      },
      onVisible: syncTriggerInputPolling,
      onHidden: syncTriggerInputPolling,
      onHashChange: () => {
        activeView = appViewFromHash();
        syncTriggerInputPolling();
      },
      onDocumentMouseDown: handleColorDocClick,
      onDocumentKeyDown: handleColorKey,
      onStop: () => {
        if (liveConfigSyncTimer !== undefined) window.clearTimeout(liveConfigSyncTimer);
        liveConfigSyncTimer = undefined;
        clearBaseFeelTestTimers();
        stopTriggerInputPolling();
      }
    });
    appRuntime.start();
  };

  const stopAppRuntime = () => {
    appRuntime?.stop();
    appRuntime = undefined;
  };

  onMount(() => {
    startAppRuntime();
    return stopAppRuntime;
  });

  // Live trigger polling only feeds the haptics curve cursor (l2LivePress /
  // r2LivePress). On the Games tab and Button Mapping tab those values aren't
  // consumed, so running the 25Hz poll there just thrashes the renderer and
  // makes unrelated clicks (e.g. the controller card's Show details) feel
  // laggy. Restrict polling to the haptics view; the base-feel test runs
  // inside that view too so its needs are covered.
  $: if (controller?.id && activeView === 'haptics') {
    startTriggerInputPolling();
  } else {
    stopTriggerInputPolling();
  }

  $: if (controller?.id && controller.id !== configLoadedFor) {
    void loadControllerConfig(controller.id);
  }

  $: if (controller?.id && controller.family === 'DualSense Edge') {
    void loadEdgeProfiles(controller.id);
  } else if (edgeProfilesLoadedFor || edgeProfiles) {
    resetEdgeProfiles();
  }
</script>

<svelte:window
  onkeydown={handleRibbonPickerKeydown}
  onclick={handleRibbonPickerDocumentClick}
  onresize={handleRibbonPickerWindowChange}
  onscroll={handleRibbonPickerWindowChange}
/>

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
          <h1>DualSense Command Center</h1>
          <p><span class="dm-app-tagline">Adaptive triggers, haptics, and live telemetry &mdash; tuned locally.</span></p>
        </div>
      </div>

      <nav class="dm-view-nav" aria-label="Command center views">
        {#each appViews as view}
          <button
            class:active={activeView === view.id}
            disabled={(view.id === 'haptics' && !tuningReady) || (view.id === 'buttonMapping' && !buttonMappingReady)}
            type="button"
            aria-current={activeView === view.id ? 'page' : undefined}
            onclick={() => navigateToView(view.id)}
          >
            {view.label}
          </button>
        {/each}
      </nav>

      <div class="dm-system-cluster">
        <div class="dm-system-readout" title={selectedTuningScope === 'global' ? systemReadoutDetail : adapter?.setupHint ?? telemetryRateDetail}>
          <span>{systemReadoutTitle}</span>
          <strong>{systemReadoutValue}</strong>
          <small>{systemReadoutDetail}</small>
        </div>
        <Tooltip text="Copy or export a sanitized support bundle for GitHub issues or Discord help. Raw hardware ids are excluded." side="bottom" align="end">
          <button
            class:active={supportPanelOpen}
            class="dm-support-trigger"
            type="button"
            aria-expanded={supportPanelOpen}
            aria-controls="support-bundle-panel"
            onclick={() => {
              supportPanelOpen = !supportPanelOpen;
            }}
          >
            <LifeBuoy size={14} /> Support
          </button>
        </Tooltip>
      </div>
    </header>

    {#if showPartialErrorBanner}
      <aside class="ops-warning dm-warning" role="status" aria-live="polite">
        <span>Partial agent data: {partialErrors.map((entry) => entry.endpoint).join(', ')} unavailable.</span>
        <button type="button" aria-label="Dismiss partial agent data notice" onclick={dismissPartialErrors}>dismiss</button>
      </aside>
    {/if}

    {#if showUpdateBanner}
      <aside class="ops-warning dm-warning update" role="status" aria-live="polite">
        <span>Update available: {updateCheck.latestVersion}. Current build {updateCheck.currentVersion}.</span>
        <div class="dm-warning-actions">
          <a href={updateCheck.releaseUrl ?? UPDATE_RELEASE_PAGE_URL} target="_blank" rel="noreferrer">
            <ExternalLink size={13} /> Download
          </a>
          <button type="button" aria-label="Dismiss update notice" onclick={dismissUpdateBanner}>dismiss</button>
        </div>
      </aside>
    {/if}

    {#if supportPanelOpen}
      <aside id="support-bundle-panel" class="dm-support-panel" aria-label="Support diagnostics bundle">
        <div class="dm-support-copy">
          <span>Support Diagnostics</span>
          <strong>Sanitized support bundle</strong>
          <p>No raw HID paths, raw hardware IDs, serial numbers, or Bluetooth addresses are included.</p>
        </div>
        <div class="dm-support-actions">
          <button class="dm-mini-button" type="button" disabled={Boolean(supportBundleBusy)} onclick={() => void copySupportBundle()}>
            <ClipboardCopy size={13} /> {supportBundleBusy === 'copy' ? 'Copying' : 'Copy JSON'}
          </button>
          <button class="dm-mini-button" type="button" disabled={Boolean(supportBundleBusy)} onclick={() => void exportSupportBundle()}>
            <Download size={13} /> {supportBundleBusy === 'download' ? 'Exporting' : 'Export JSON'}
          </button>
        </div>
        {#if supportBundleMessage}
          <p class:error={supportBundleTone === 'error'} class:success={supportBundleTone === 'success'} class="dm-support-message">{supportBundleMessage}</p>
        {/if}
      </aside>
    {/if}

    {#if activeView === 'games' || !tuningReady}
      <section class="dm-games-page" aria-label="Supported games and target controller">
        <div class="dm-games-column">
          <div class="dm-games-head">
            <span>Target</span>
            <h2>Controller</h2>
          </div>
          <div class="dm-controller-choice-list">
            {#if controllers.length}
              {#each controllers as item, index (item.id)}
                <ControllerCard
                  {item}
                  {index}
                  selected={item.id === selectedControllerId}
                  renameActive={controllerRenameId === item.id}
                  bind:renameName={controllerRenameName}
                  renameBusy={controllerRenameBusy}
                  onSelect={selectTargetController}
                  onBeginRename={beginControllerRename}
                  onSubmitRename={submitControllerRename}
                  onCancelRename={cancelControllerRename}
                  onRenameKeydown={handleControllerRenameKeydown}
                />
              {/each}
            {:else}
              <div class="dm-empty-choice">
                <strong>No controller detected</strong>
                <span>Controller unavailable</span>
              </div>
            {/if}
          </div>

          {#if controller?.family === 'DualSense Edge'}
            <section class="dm-edge-slots" aria-label="DualSense Edge onboard profiles">
              <div class="dm-edge-slots-head">
                <div>
                  <span>Onboard Memory</span>
                  <strong>Edge Slots</strong>
                </div>
                <Tooltip text={edgeSlotsReadTooltip} side="bottom" align="end">
                  <button
                    type="button"
                    class="dm-mini-button"
                    disabled={edgeProfilesLoading}
                    aria-label="Refresh DualSense Edge onboard slots"
                    onclick={() => controller && void loadEdgeProfiles(controller.id, true)}
                  >
                    {edgeProfilesLoading ? '...' : 'Read'}
                  </button>
                </Tooltip>
              </div>

              {#if edgeProfilesError}
                <p class="dm-edge-slots-note error">{edgeProfilesError}</p>
              {:else if edgeProfiles?.warning}
                <p class="dm-edge-slots-note">{edgeProfiles.warning}</p>
              {/if}

              <div class="dm-edge-slot-list">
                {#if edgeProfiles?.slots.length}
                  {#each edgeProfiles.slots as slot (slot.slotId)}
                    <div class="dm-edge-slot-row" class:disabled={!slot.editable}>
                      <Tooltip block text={edgeSlotInfoTooltip(slot)} side="right" align="start">
                        <div class="dm-edge-slot-copy">
                          <span>{slot.shortcut}</span>
                          <strong>{edgeSlotName(slot)}</strong>
                          <small>{edgeSlotStatus(slot)}</small>
                        </div>
                      </Tooltip>
                      {#if slot.editable}
                        <Tooltip text={edgeSlotWriteTooltip(slot)} side="left" align="center">
                          <button
                            type="button"
                            class="dm-mini-button primary"
                            disabled={!currentControllerConfig || edgeProfilesBusySlot === slot.slotId}
                            onclick={() => void writeCurrentConfigToEdgeSlot(slot)}
                          >
                            {edgeProfilesBusySlot === slot.slotId ? '...' : edgeSlotWriteLabel()}
                          </button>
                        </Tooltip>
                      {/if}
                    </div>
                  {/each}
                {:else}
                  <div class="dm-edge-slot-row disabled">
                    <div>
                      <span>Fn Slots</span>
                      <strong>{edgeProfilesLoading ? 'Reading slots' : 'No slot data'}</strong>
                      <small>{edgeProfilesLoading ? 'usb scan' : 'unavailable'}</small>
                    </div>
                  </div>
                {/if}
              </div>
            </section>
          {/if}
        </div>

        <div class="dm-games-column wide">
          <div class="dm-games-head">
            <span>Tuning Scope</span>
            <h2>Games</h2>
          </div>
          <div class="dm-scope-strip">
            <Tooltip
              text="Per-game profiles auto-load when the game launches via Steam. Global tunes the controller when nothing is detected or an unsupported game is running."
              side="bottom"
              align="start"
            >
              <button
                type="button"
                class:active={selectedTuningScope === 'global'}
                disabled={!controller}
                onclick={() => void selectGlobalTuning()}
              >
                <span class="dm-controller-glyph small" aria-hidden="true"></span>
                <span class="dm-scope-chip">
                  <span class="dm-scope-chip-label">Profile Scope</span>
                  <strong class="dm-scope-chip-value">Global</strong>
                  <small class="dm-scope-chip-detail">{globalProfilePreview?.name ?? 'Base'} · Controller-only tuning</small>
                </span>
              </button>
            </Tooltip>
          </div>
          {#if discoveredGames.length}
            <div class="dm-game-grid">
              {#each discoveredGames as game (game.gameId)}
                {@const heroArt = gameArtwork(game, 'hero') ?? gameArtwork(game, 'banner')}
                {@const tileArt = gameArtwork(game, 'banner') ?? gameArtwork(game, 'capsule') ?? gameArtwork(game, 'icon')}
                {@const details = gameMediaDetails(game)}
                {@const scopedProfiles = profileScopeCount(game)}
                <button
                  type="button"
                  class="dm-game-card"
                  class:active={selectedTuningScope === 'game' && game.gameId === selectedTuningGameId}
                  class:running={game.running}
                  class:custom={game.supportLevel === 'custom'}
                  disabled={!controller}
                  aria-pressed={selectedTuningScope === 'game' && game.gameId === selectedTuningGameId}
                  style={heroArt ? `--game-hero: url("${heroArt}")` : ''}
                  onclick={() => void selectTuningGame(game)}
                >
                  <span class="dm-game-card-media">
                    {#if tileArt}
                      <img
                        src={tileArt}
                        alt=""
                        loading="lazy"
                        aria-hidden="true"
                        onerror={(event) => {
                          const img = event.currentTarget;
                          if (img instanceof HTMLImageElement) img.style.display = 'none';
                        }}
                      />
                    {/if}
                    <span class="dm-game-art-fallback" aria-hidden="true">
                      <InitialBadge label={game.name} accent={gameAccentColor(game)} />
                    </span>
                    <code>{game.running ? 'LIVE' : game.supportLevel === 'custom' ? 'CUSTOM' : game.installed ? 'READY' : 'SUPPORTED'}</code>
                  </span>
                  <span class="dm-game-copy">
                    <strong>{game.name}</strong>
                    <span class="dm-game-meta">
                      {#each details as detail}
                        <em>{detail}</em>
                      {/each}
                    </span>
                    <small>{scopedProfiles ? `${scopedProfiles} game profile${scopedProfiles === 1 ? '' : 's'}` : game.supportLevel === 'custom' ? 'custom / no telemetry adapter' : `${gameTileStatus(game)} / telemetry`}</small>
                  </span>
                </button>
              {/each}
              <button
                type="button"
                class="dm-game-card dm-game-card-add"
                disabled={!controller}
                aria-label="Add a custom game from your Steam library"
                onclick={() => void openAddGameDialog()}
              >
                <span class="dm-add-game-icon" aria-hidden="true">+</span>
                <span class="dm-game-copy">
                  <strong>Add a Game</strong>
                  <small>Pick from your installed Steam library &mdash; DSCC will save a profile per game and auto-load it on launch.</small>
                </span>
              </button>
            </div>
          {:else}
            <div class="dm-empty-choice wide">
              <strong>No supported games discovered</strong>
              <span>{detectionSignalText || 'Steam library data unavailable'}</span>
              <button
                type="button"
                class="dm-mini-button"
                style="margin-top: 8px;"
                disabled={!controller}
                onclick={() => void openAddGameDialog()}
              >Add a game manually</button>
            </div>
          {/if}
        </div>
      </section>
    {:else}
      <section class="dm-tuning-ribbon" aria-label="Selected game context and production controls">
        <div class="dm-steam-identity">
          {#if steamContextArt}
            <img src={steamContextArt} alt="" loading="lazy" aria-hidden="true" />
          {:else}
            <span class="dm-controller-glyph small" aria-hidden="true"></span>
          {/if}
          <div class="dm-ribbon-picker-host">
            <button
              bind:this={scopeTriggerEl}
              type="button"
              class="dm-steam-identity-cell dm-ribbon-picker-trigger"
              class:open={scopePickerOpen}
              aria-haspopup="listbox"
              aria-expanded={scopePickerOpen}
              onclick={toggleScopePicker}
            >
              <span>{selectedTuningScope === 'global' ? 'Selected Scope' : 'Selected Game'}</span>
              <strong>{selectedTuningScope === 'global' ? 'Global Profile' : steamContextGame?.name ?? 'No supported game selected'}</strong>
              <p>{steamContextMeta}</p>
              <span class="dm-ribbon-picker-caret" aria-hidden="true">▾</span>
            </button>
            {#if scopePickerOpen}
              <div
                class="dm-ribbon-picker-menu"
                role="listbox"
                aria-label="Select tuning scope"
                style:left="{scopeMenuPos.left}px"
                style:top="{scopeMenuPos.top}px"
                style:min-width="{scopeMenuPos.minWidth}px"
              >
                <button
                  type="button"
                  class="dm-ribbon-picker-item"
                  class:active={selectedTuningScope === 'global'}
                  role="option"
                  aria-selected={selectedTuningScope === 'global'}
                  onclick={() => void pickScopeGlobal()}
                >
                  <span class="dm-ribbon-picker-thumb art" aria-hidden="true">
                    <InitialBadge label="G" accent={SCOPE_ACCENT_GLOBAL} />
                  </span>
                  <span class="dm-ribbon-picker-copy">
                    <strong>Global Profile</strong>
                    <small>Controller-only tuning</small>
                  </span>
                </button>
                {#if discoveredGames.length}
                  <div class="dm-ribbon-picker-divider" role="separator"></div>
                  {#each discoveredGames as game (game.gameId)}
                    {@const gameArt = gameArtwork(game, 'capsule') ?? gameArtwork(game, 'banner') ?? gameArtwork(game, 'icon')}
                    <button
                      type="button"
                      class="dm-ribbon-picker-item"
                      class:active={selectedTuningScope === 'game' && game.gameId === selectedTuningGameId}
                      role="option"
                      aria-selected={selectedTuningScope === 'game' && game.gameId === selectedTuningGameId}
                      onclick={() => void pickScopeGame(game)}
                    >
                      <span class="dm-ribbon-picker-thumb art" aria-hidden="true">
                        {#if gameArt}
                          <img src={gameArt} alt="" loading="lazy" />
                        {:else}
                          <InitialBadge label={game.name} accent={gameAccentColor(game)} />
                        {/if}
                      </span>
                      <span class="dm-ribbon-picker-copy">
                        <strong>{game.name}</strong>
                        <small>{game.supportLevel === 'custom' ? 'custom game' : game.running ? 'running' : game.installed ? 'installed' : 'discovered'}</small>
                      </span>
                    </button>
                  {/each}
                {/if}
              </div>
            {/if}
          </div>
          <div class="dm-ribbon-picker-host">
            <button
              bind:this={profileTriggerEl}
              type="button"
              class="dm-steam-identity-cell dm-active-profile-cell dm-ribbon-picker-trigger"
              class:open={profilePickerOpen}
              aria-haspopup="listbox"
              aria-expanded={profilePickerOpen}
              aria-live="polite"
              disabled={profileContextProfiles.length === 0}
              onclick={toggleProfilePicker}
            >
              <span>Active Profile</span>
              <strong>{activeProfileHeaderName}</strong>
              <p>{activeProfileHeaderMeta}</p>
              <span class="dm-ribbon-picker-caret" aria-hidden="true">▾</span>
            </button>
            {#if profilePickerOpen && profileContextProfiles.length}
              <div
                class="dm-ribbon-picker-menu profile"
                role="listbox"
                aria-label="Select active profile"
                style:left="{profileMenuPos.left}px"
                style:top="{profileMenuPos.top}px"
                style:min-width="{profileMenuPos.minWidth}px"
              >
                {#each profileContextProfiles as profile (profile.id)}
                  <button
                    type="button"
                    class="dm-ribbon-picker-item"
                    class:active={profile.id === (selectedOverrideProfileId || activeProfileId)}
                    role="option"
                    aria-selected={profile.id === (selectedOverrideProfileId || activeProfileId)}
                    onclick={() => void pickProfile(profile.id)}
                  >
                    <span class="dm-ribbon-picker-thumb art" aria-hidden="true">
                      <InitialBadge label={profile.name} accent={profileAccentColor(profile.scope)} />
                    </span>
                    <span class="dm-ribbon-picker-copy">
                      <strong>{profile.name}</strong>
                      <small>{profile.builtIn ? (profile.scope === 'Global' ? 'Stock / Global' : 'Built-in template') : profile.scope === 'Game' ? `Custom / ${steamContextGame?.name ?? 'game'}` : 'Custom / Global'}{profile.id === activeProfileId ? ' · live' : ''}</small>
                    </span>
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        </div>

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

      {#if activeView === 'haptics'}
      <HapticsView active>
      <section class="dm-physics" aria-label="Actuation curve tuning">
        <div class="dm-section-head">
          <div>
            <span>Actuation Engine</span>
            <h2>Trigger Curves</h2>
          </div>
          <div class="dm-section-actions">
            <Tooltip text="Restores L2/R2 range, curve, base force, and body feel to the active profile defaults. Custom profiles reset to the Base curve." side="top" align="end">
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
                <rect class="curve-range-fill" x={l2CurveShape.rangeStart} y="96" width={l2CurveShape.rangeWidth} height="2.5" rx="1.25" />
                <line class="curve-range-edge" x1={l2CurveShape.rangeStart} y1="0" x2={l2CurveShape.rangeStart} y2="100" />
                <line class="curve-range-edge" x1={l2CurveShape.rangeEnd} y1="0" x2={l2CurveShape.rangeEnd} y2="100" />
                <path class="curve-force" d={l2CurveShape.path} />
                {#if curveHover?.side === 'l2'}
                  <line class="curve-crosshair" x1={curveHover.left.toFixed(2)} y1="0" x2={curveHover.left.toFixed(2)} y2="100" />
                {/if}
                {#if showTriggerPress('l2', l2LivePress)}
                  <line class="curve-live" x1={l2CurveLive.liveX} y1="0" x2={l2CurveLive.liveX} y2="100" />
                  <circle class="curve-live-dot" cx={l2CurveLive.liveX} cy={l2CurveLive.liveY} r="1.75" />
                {/if}
              </svg>
              {#each l2CurveShape.curvePoints as point}
                <button
                  class:active={curveDragPoint?.side === 'l2' && curveDragPoint.index === point.index}
                  class:locked={point.locked}
                  class="dm-curve-control-handle"
                  style="--point-x:{point.x}%;--point-y:{point.y}%;"
                  type="button"
                  aria-label="L2 curve control point"
                  aria-disabled={point.locked}
                  onpointerdown={(event) => point.locked ? (event.preventDefault(), event.stopPropagation()) : handleCurvePointPointer(event, 'l2', point.index)}
                ></button>
              {/each}
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
              <div class="dm-curve-point-row">
                <span>Points</span>
                <div class="dm-curve-point-actions">
                  <Tooltip text="Remove the least dramatic editable control point." side="top" align="center">
                    <button class="dm-icon-button" type="button" aria-label="Remove L2 curve point" disabled={l2CurvePoints.length <= TRIGGER_CURVE_POINT_MIN} onclick={() => removeCurvePoint('l2')}>
                      <Minus size={14} />
                    </button>
                  </Tooltip>
                  <code>{l2CurvePoints.length}</code>
                  <Tooltip text="Add an editable control point to the widest curve segment." side="top" align="center">
                    <button class="dm-icon-button" type="button" aria-label="Add L2 curve point" disabled={l2CurvePoints.length >= TRIGGER_CURVE_POINT_MAX} onclick={() => addCurvePoint('l2')}>
                      <Plus size={14} />
                    </button>
                  </Tooltip>
                </div>
              </div>
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
                <rect class="curve-range-fill" x={r2CurveShape.rangeStart} y="96" width={r2CurveShape.rangeWidth} height="2.5" rx="1.25" />
                <line class="curve-range-edge" x1={r2CurveShape.rangeStart} y1="0" x2={r2CurveShape.rangeStart} y2="100" />
                <line class="curve-range-edge" x1={r2CurveShape.rangeEnd} y1="0" x2={r2CurveShape.rangeEnd} y2="100" />
                <path class="curve-force" d={r2CurveShape.path} />
                {#if curveHover?.side === 'r2'}
                  <line class="curve-crosshair" x1={curveHover.left.toFixed(2)} y1="0" x2={curveHover.left.toFixed(2)} y2="100" />
                {/if}
                {#if showTriggerPress('r2', r2LivePress)}
                  <line class="curve-live" x1={r2CurveLive.liveX} y1="0" x2={r2CurveLive.liveX} y2="100" />
                  <circle class="curve-live-dot" cx={r2CurveLive.liveX} cy={r2CurveLive.liveY} r="1.75" />
                {/if}
              </svg>
              {#each r2CurveShape.curvePoints as point}
                <button
                  class:active={curveDragPoint?.side === 'r2' && curveDragPoint.index === point.index}
                  class:locked={point.locked}
                  class="dm-curve-control-handle"
                  style="--point-x:{point.x}%;--point-y:{point.y}%;"
                  type="button"
                  aria-label="R2 curve control point"
                  aria-disabled={point.locked}
                  onpointerdown={(event) => point.locked ? (event.preventDefault(), event.stopPropagation()) : handleCurvePointPointer(event, 'r2', point.index)}
                ></button>
              {/each}
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
              <div class="dm-curve-point-row">
                <span>Points</span>
                <div class="dm-curve-point-actions">
                  <Tooltip text="Remove the least dramatic editable control point." side="top" align="center">
                    <button class="dm-icon-button" type="button" aria-label="Remove R2 curve point" disabled={r2CurvePoints.length <= TRIGGER_CURVE_POINT_MIN} onclick={() => removeCurvePoint('r2')}>
                      <Minus size={14} />
                    </button>
                  </Tooltip>
                  <code>{r2CurvePoints.length}</code>
                  <Tooltip text="Add an editable control point to the widest curve segment." side="top" align="center">
                    <button class="dm-icon-button" type="button" aria-label="Add R2 curve point" disabled={r2CurvePoints.length >= TRIGGER_CURVE_POINT_MAX} onclick={() => addCurvePoint('r2')}>
                      <Plus size={14} />
                    </button>
                  </Tooltip>
                </div>
              </div>
            </div>
          </article>
        </div>

        <div class="dm-parameter-strip" aria-label="Base force and light routing">
          <Tooltip block text={triggerEffectHelp[triggerEffect] ?? 'Selects the base adaptive trigger behavior.'} side="top" align="start">
            <label>
              <span>Mode</span>
              <select value={triggerEffect} onchange={(event) => setTriggerEffect(event.currentTarget.value)}>
                {#each triggerEffectOptions as option}
                  <option>{option.label}</option>
                {/each}
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
          <Tooltip block text={vibrationModeHelp[vibrationMode] ?? 'Controls the body haptic motor blend.'} side="top" align="start">
            <label>
              <span>Feel</span>
              <select value={vibrationMode} onchange={(event) => setVibrationMode(event.currentTarget.value)}>
                {#each vibrationModeOptions as option}
                  <option>{option.label}</option>
                {/each}
              </select>
            </label>
          </Tooltip>
        </div>
      </section>

      <aside class:dm-global-feel={selectedTuningScope === 'global'} class="dm-routing" aria-label={selectedTuningScope === 'global' ? 'Controller haptic tuning' : 'Telemetry haptic routing'}>
        {#if selectedTuningScope === 'global'}
          <div class="dm-section-head compact">
            <div>
              <span>Controller Feel</span>
              <h2>Base Haptics</h2>
            </div>
          </div>
          <div class="dm-global-feel-panel">
            <article>
              <div class="dm-global-feel-heading">
                <strong>Trigger pattern</strong>
                <code>{triggerIntensity}</code>
              </div>
              <span>L2 and R2 use the selected hardware pattern with the curves configured on the left.</span>
              <div class="dm-pattern-grid" aria-label="Trigger haptic pattern">
                {#each triggerEffectOptions as option}
                  <Tooltip block text={triggerEffectHelp[option.label] ?? 'Selects the base adaptive trigger behavior.'} side="bottom" align="start">
                    <button
                      class:active={triggerEffect === option.label}
                      class="dm-pattern-option"
                      type="button"
                      aria-pressed={triggerEffect === option.label}
                      onclick={() => setTriggerEffect(option.label)}
                    >
                      <strong>{option.label}</strong>
                      <span>{option.badge}</span>
                    </button>
                  </Tooltip>
                {/each}
              </div>
              <button class:active={baseFeelTestActive} class="dm-test-button" type="button" disabled={baseFeelTestBusy || !snapshot} onclick={() => void toggleBaseFeelTest()}>
                {baseFeelTestActive ? 'Stop Preview' : 'Preview Triggers'}
              </button>
            </article>
            <article>
              <div class="dm-global-feel-heading">
                <strong>Body haptics</strong>
                <code>{vibrationMode}</code>
              </div>
              <span>Global profiles keep game telemetry off while storing controller-level body strength and motor blend.</span>
              <div class="dm-global-feel-controls">
                <label>
                  <span>Strength</span>
                  <select value={vibrationIntensity} onchange={(event) => setVibrationIntensity(event.currentTarget.value)}>
                    <option>Off</option><option>Low</option><option>Medium</option><option>High</option>
                  </select>
                </label>
                <label>
                  <span>Motor blend</span>
                  <select value={vibrationMode} onchange={(event) => setVibrationMode(event.currentTarget.value)}>
                    {#each vibrationModeOptions as option}
                      <option>{option.label}</option>
                    {/each}
                  </select>
                </label>
              </div>
              <div class="dm-vibration-mode-grid" aria-label="Body haptic character">
                {#each vibrationModeOptions as option}
                  <Tooltip block text={vibrationModeHelp[option.label] ?? 'Controls the body haptic motor blend.'} side="bottom" align="start">
                    <button
                      class:active={vibrationMode === option.label}
                      class="dm-pattern-option"
                      type="button"
                      aria-pressed={vibrationMode === option.label}
                      onclick={() => setVibrationMode(option.label)}
                    >
                      <strong>{option.label}</strong>
                      <span>{option.badge}</span>
                    </button>
                  </Tooltip>
                {/each}
              </div>
              <button class="dm-test-button" type="button" disabled={!snapshot || vibrationIntensity === 'Off'} onclick={() => void previewBodyHaptics()}>
                Preview Body
              </button>
            </article>
          </div>
        {:else}
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

          <div class="dm-body-mode-panel" aria-label="Body rumble source">
            <div class="dm-body-mode-title">
              <span>Body Source</span>
              <code>{forzaBodyRumbleMode === 'native_passthrough' ? 'Native' : 'DSCC'}</code>
            </div>
            <div class="dm-body-mode-toggle" role="radiogroup" aria-label="Forza body rumble mode">
              {#each bodyRumbleModeOptions as option}
                <Tooltip block text={option.help} side="bottom" align="start">
                  <button
                    class:active={forzaBodyRumbleMode === option.value}
                    class="dm-body-mode-option"
                    type="button"
                    role="radio"
                    aria-checked={forzaBodyRumbleMode === option.value}
                    onclick={() => setForzaBodyRumbleMode(option.value)}
                  >
                    <strong>{option.label}</strong>
                    <span>{option.badge}</span>
                  </button>
                </Tooltip>
              {/each}
            </div>
          </div>

          <div class="dm-channel-list">
            {#each forzaEffectMetas as meta (meta.id)}
              {@const tuning = forzaEffectsById.get(meta.id) ?? forzaEffect(meta.id)}
              {@const status = effectStatusById.get(meta.id)}
              <article
                class:active={tuning.enabled && status?.state === 'active'}
                class:disabled={!tuning.enabled}
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
        {/if}

        <div class="dm-rgb-console" aria-label="RGB output controls">
          <div class="dm-console-title">
            <span>RGB Controls</span>
            <strong>{selectedTuningScope === 'global' ? 'Lightbar' : 'Lightbar & RPM'}</strong>
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
            {#if selectedTuningScope === 'game'}
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
            {/if}
          </div>
        </div>

        <div class="dm-profile-console" bind:this={profilePanelEl}>
          <div class="dm-profile-line">
            <label>
              <span>Profile</span>
              <select value={selectedOverrideProfileId || activeProfileId} disabled={!profiles.length} onchange={(event) => void selectProfileForScope(event.currentTarget.value)}>
                {#each profileContextProfiles as profile}
                  <option value={profile.id}>{profile.name}{profile.scope === 'Game' ? ' / game' : profile.id === activeProfileId ? ' / active' : ''}</option>
                {/each}
              </select>
            </label>
            <div class="dm-action-row">
              <button class="dm-mini-button" type="button" onclick={requestProfileImport}>Import</button>
              <input bind:this={profileImportInput} class="ops-hidden-file" type="file" accept="application/json,.json,.dscc-profile" onchange={(event) => void handleProfileImport(event)} />
              <button class="dm-mini-button" type="button" disabled={!activeProfileId || profileFileBusy} onclick={() => void exportSelectedProfile()}>Export</button>
              <button
                class="dm-mini-button wide"
                type="button"
                disabled={!selectedActionProfile || profileSaveAsBusy}
                title="Save the current tuning into a new profile"
                onclick={beginSaveAsProfile}
              ><CopyPlus size={14} /> Save As</button>
              <button
                class="dm-mini-button"
                type="button"
                disabled={!canRenameSelectedProfile || profileRenameBusy || !selectedActionProfile}
                title={canRenameSelectedProfile ? 'Rename selected custom profile' : 'Stock profiles cannot be renamed'}
                onclick={beginRenameSelectedProfile}
              >Rename</button>
              <button
                class="dm-mini-button"
                type="button"
                disabled={!canDeleteSelectedProfile || profileFileBusy || !selectedActionProfile}
                title={canDeleteSelectedProfile ? 'Delete selected custom profile' : 'Stock profiles cannot be deleted'}
                onclick={() => selectedActionProfile && void deleteProfileById(selectedActionProfile.id, selectedActionProfile.name)}
              >Delete</button>
              <button class="dm-mini-button" type="button" onclick={restoreDefaults}><RotateCcw size={14} /> Reset</button>
              <button
                class:dirty={profileConfigDirty}
                class="dm-apply-button"
                type="button"
                disabled={!selectedActionProfile || profileSaveBusy || !profileConfigDirty}
                onclick={() => void saveActiveProfile()}
              ><Save size={14} /> {profileSaveBusy ? 'Saving' : 'Save'}</button>
            </div>
          </div>
          {#if saveAsProfileOpen}
            <div class="dm-profile-rename">
              <label>
                <span>Save As</span>
                <input
                  bind:value={saveAsProfileName}
                  disabled={profileSaveAsBusy}
                  maxlength="80"
                  spellcheck="false"
                  onkeydown={handleSaveAsProfileKeydown}
                  aria-label="New profile name"
                />
              </label>
              <div class="dm-action-row">
                <button class="dm-mini-button" type="button" disabled={profileSaveAsBusy} onclick={cancelSaveAsProfile}>Cancel</button>
                <button class="dm-mini-button primary" type="button" disabled={profileSaveAsBusy || !saveAsProfileName.trim()} onclick={() => void submitSaveAsProfile()}>
                  {profileSaveAsBusy ? 'Saving' : 'Create'}
                </button>
              </div>
            </div>
          {/if}
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
      </HapticsView>
      {/if}
    <ButtonMappingView
      active={activeView === 'buttonMapping'}
      steamInputRunning={Boolean(steamInputStatus?.running)}
      {controllerHeaderName}
      controllerTransport={controller?.transport}
      gameName={selectedTuningScope === 'global' ? 'Global Profile' : steamContextGame?.name ?? 'No supported game selected'}
      {steamLayoutTitle}
      {mappedVisibleChipCount}
      {steamMirrorGroups}
      {focusedSlotMeta}
      {focusedSlotBinding}
      {focusedSlotSelectedBinding}
      {steamBindingBusy}
      steamInputLayoutAvailable={Boolean(steamInputLayout)}
      paddlePresetVisible={steamPaddlePresetVisible}
      paddlePresetAvailable={steamPaddlePresetAvailable}
      paddlePresetStatus={steamPaddlePresetStatus}
      {paddlePresetLeftKey}
      {paddlePresetRightKey}
      {steamBindingDraft}
      {steamBindingLabelDraft}
      targetGroups={preparedSteamBindingTargetGroups}
      onSelectSlot={selectSteamSlot}
      onHoverSlot={hoverSteamSlot}
      onPaddlePresetLeftKeyChange={setPaddlePresetLeftKey}
      onPaddlePresetRightKeyChange={setPaddlePresetRightKey}
      onApplyPaddlePreset={() => void applySteamPaddlePreset(false)}
      onTargetChange={applySteamBindingTargetChange}
      onLabelChange={applySteamBindingLabelChange}
      onRawDraftChange={applySteamBindingRawChange}
      onResetDraft={resetSteamBindingDraft}
      onSaveBinding={() => void saveSteamBinding(false)}
    />
    {/if}
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

  <AddGameDialog
    open={addGameOpen}
    entries={addGameEntries}
    loading={addGameLoading}
    busyAppId={addGameBusyAppId}
    errorMessage={addGameError}
    onClose={closeAddGameDialog}
    onAdd={(entry, processNames) => void addGameFromLibrary(entry, processNames)}
  />
</main>
