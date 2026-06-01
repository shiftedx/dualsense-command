import type { ForzaAbsMode, ForzaAbsSlipSource, ForzaBodyRumbleMode, ForzaEffectRoute } from '../../types';
import type { ForzaEffectMeta } from './hapticsModel';
export const forzaRoutes: Array<{ value: ForzaEffectRoute; label: string }> = [
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
export const FORZA_SHIFT_THUMP_DEFAULT_INTENSITY = 255;
// Mirrors the backend Forza runtime profile so the graph shows felt force, not just exponent shape.
export const FORZA_BRAKE_BASELINE_FORCE = 76 / 255;
export const FORZA_BRAKE_NORMAL_FORCE = 1;
export const FORZA_BRAKE_ENDSTOP_FORCE = 255 / 255;
export const FORZA_THROTTLE_BASELINE_FORCE = 3 / 255;
export const FORZA_THROTTLE_NORMAL_FORCE = 28 / 255;
export const FORZA_THROTTLE_ENDSTOP_FORCE = 106 / 255;
export const FORZA_ENDSTOP_WALL_OFFSET = 0.03;
export const FORZA_BRAKE_OVERTRAVEL_WALL_POSITION = 0.48;
export const FORZA_BRAKE_OVERTRAVEL_MIN_POSITION = 0.48;
export const FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE = 0.8;
export const FORZA_BRAKE_FULL_FORCE_INPUT = 0.80;
export const FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION = 0.80;
export const FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION = 0.80;
export const FORZA_BRAKE_ENDSTOP_FORCE_BOOST = 1.25;
export const FORZA_THROTTLE_ENDSTOP_FORCE_BOOST = 3.0;
export const FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH = 0.20;
export const FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE = 2.4;

export const shiftThumpPresets = [
    { label: 'Soft', intensity: 35 },
    { label: 'Medium', intensity: 65 },
    { label: 'Strong', intensity: 180 },
    { label: 'Max', intensity: FORZA_SHIFT_THUMP_DEFAULT_INTENSITY }
  ];

export const shiftThumpPresetHelp: Record<string, string> = {
    Soft: 'A lighter mechanical cue for users who want shift feedback without a big kick through the controller.',
    Medium: 'A moderate shift kick that is easy to feel but less abrupt than the stock strong profile.',
    Strong: 'A firmer R2 kick with reduced body feedback for a more physical gear change.',
    Max: 'The Base shift thump: the strongest cue, using the full 255 effect ceiling so gear changes punch through road texture and engine cues.'
  };

export const routeTooltips: Record<ForzaEffectRoute, string> = {
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

export const triggerEffectHelp: Record<string, string> = {
    'Adaptive resistance': 'A smooth force ramp that increases resistance as the trigger moves. This is the default because it feels closest to pedal load.',
    Pulse: 'A vibration-like trigger pulse. Useful for alerts, but less pedal-like than adaptive resistance.',
    Wall: 'Creates a hard stop at the trigger position. Best for binary actions such as a handbrake wall.',
    'Wall pulse': 'A pulsing trigger pattern with a wall-form kick. This exposes the same hardware mode DSCC uses for strong shift thumps.',
    Off: 'Disables base trigger force. Telemetry effects can still run if their individual rows are enabled.'
  };

export const triggerEffectOptions = [
    { label: 'Adaptive resistance', badge: 'Ramp' },
    { label: 'Pulse', badge: 'Pulse' },
    { label: 'Wall', badge: 'Stop' },
    { label: 'Wall pulse', badge: 'Kick' },
    { label: 'Off', badge: 'Mute' }
  ];

export const triggerStrengthHelp: Record<string, string> = {
    Off: 'No base trigger resistance is applied.',
    Weak: 'Light resistance for users who want subtle feedback or less hand fatigue.',
    Medium: 'Moderate resistance that keeps cues clear without making the triggers heavy.',
    'Strong (Standard)': 'The intended DSCC baseline. Strong enough to feel the curve clearly while staying within comfortable DualSense force levels.'
  };

export const vibrationHelp: Record<string, string> = {
    Off: 'Disables body rumble output while leaving adaptive triggers and LEDs available.',
    Low: 'Keeps grip motors quiet and battery-friendly. Good for long sessions.',
    Medium: 'Moderate body feedback for road texture and event thumps.',
    High: 'Stronger grip feedback. Use when you want road, impact, and shift cues to stand out more.'
  };

export const vibrationModeHelp: Record<string, string> = {
    Balanced: 'Keeps low and high motors blended for general-purpose body feedback.',
    'Deep thump': 'Leans into the low-frequency motor for heavier grip movement and impact cues.',
    'Fine buzz': 'Leans into the high-frequency motor for sharper texture and alert cues.'
  };

export const vibrationModeOptions = [
    { label: 'Balanced', mode: 'balanced', badge: 'Blend' },
    { label: 'Deep thump', mode: 'deep_thump', badge: 'Low' },
    { label: 'Fine buzz', mode: 'fine_buzz', badge: 'High' }
  ];

export const bodyRumbleModeOptions: Array<{ value: ForzaBodyRumbleMode; label: string; badge: string; help: string }> = [
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

export const forzaAbsModeOptions: Array<{ value: ForzaAbsMode; label: string; badge: string; help: string }> = [
    {
      value: 'strong_pulse',
      label: 'Strong pulse',
      badge: 'Direct',
      help: 'Uses direct trigger pulse output for a clear ABS chatter when brake slip crosses the threshold.'
    },
    {
      value: 'fine_flutter',
      label: 'Fine flutter',
      badge: 'Wall-form',
      help: 'Uses the wall-form pulse mode for a sharper but more segmented ABS feel on some controllers.'
    }
  ];

export const forzaAbsSlipSourceOptions: Array<{ value: ForzaAbsSlipSource; label: string; help: string }> = [
    {
      value: 'auto_front_first',
      label: 'Auto',
      help: 'Watches front slip first, then tire slip ratio and wheel slip as fallbacks.'
    },
    {
      value: 'front',
      label: 'Front slip',
      help: 'Only uses front wheel slip for ABS activation.'
    },
    {
      value: 'tire',
      label: 'Tire ratio',
      help: 'Only uses normalized tire slip ratio for ABS activation.'
    },
    {
      value: 'wheel',
      label: 'Wheel slip',
      help: 'Only uses maximum wheel slip for ABS activation.'
    }
  ];

export const forzaEffectMetas: ForzaEffectMeta[] = [
    {
      id: 'brake_resistance',
      label: 'Brake pressure',
      signal: 'input.brake',
      group: 'Trigger',
      defaultIntensity: 100,
      defaultRoute: 'l2',
      help: 'Maps brake input to L2 resistance. Advanced tuning controls initial force, ramp force, wall position, full-force point, and ramp shape; best left on L2 for a natural brake pedal feel.'
    },
    {
      id: 'abs_slip_pulse',
      label: 'ABS / front slip',
      signal: 'wheel.slip.front_max',
      group: 'Trigger',
      defaultIntensity: 100,
      defaultRoute: 'l2',
      help: 'Adds a strong, fast L2 trigger pulse when front tires lose grip under braking. It is tuned to be obvious enough to read as ABS modulation during hard braking.'
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
      help: 'Fires a short kick when DSCC detects a gear change. Clutch bite is the percent that counts as pressed; clean shift controls clutch-assisted feedback, missed clutch controls no-clutch feedback, and clutch unload cuts drivetrain body rumble while the clutch is pressed.'
    },
    {
      id: 'rev_limiter_buzz',
      label: 'Rev limiter buzz',
      signal: 'vehicle.rpm_ratio',
      group: 'Cue',
      defaultIntensity: 120,
      defaultRoute: 'r2',
      help: 'Adds a high-RPM buzz as the engine approaches the limiter. It is meant as a shift cue, so keep intensity moderate if you already use the redline blink.'
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
      label: 'Redline blink',
      signal: 'vehicle.rpm_ratio',
      group: 'Light',
      defaultIntensity: 100,
      defaultRoute: 'light_led',
      help: 'Flashes the lightbar and all five player LEDs at the rev-limiter threshold. Below redline, the lightbar stays on the selected profile color instead of acting like a constant RPM bar.'
    }
  ];
