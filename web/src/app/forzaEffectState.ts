import {
  clampForzaIntensity,
  defaultForzaAbsTuning,
  defaultForzaBrakeTuning,
  defaultForzaEffects,
  defaultForzaRevLimiterTuning,
  defaultForzaShiftTuning,
  defaultForzaThrottleTuning,
  normalizeForzaAbsTuning,
  normalizeForzaBrakeTuning,
  normalizeForzaEffects,
  normalizeForzaRevLimiterTuning,
  normalizeForzaShiftTuning,
  normalizeForzaThrottleTuning
} from './hapticsState';
import { normalizeForzaBodyRumbleMode } from './profileDraft';
import type {
  ControllerConfiguration,
  ForzaAbsTuningConfiguration,
  ForzaBodyRumbleMode,
  ForzaBrakeTuningConfiguration,
  ForzaEffectConfiguration,
  ForzaRevLimiterTuningConfiguration,
  ForzaShiftTuningConfiguration,
  ForzaThrottleTuningConfiguration
} from '../lib/types';

// Game Module-specific tuning state for the Forza runtime profile: the body
// rumble routing plus every per-effect and per-signal tuning block. Future
// supported games can follow the same store shape.
export type ForzaTuningValues = {
  bodyRumbleMode: ForzaBodyRumbleMode;
  effects: ForzaEffectConfiguration[];
  abs: ForzaAbsTuningConfiguration;
  brake: ForzaBrakeTuningConfiguration;
  throttle: ForzaThrottleTuningConfiguration;
  shift: ForzaShiftTuningConfiguration;
  revLimiter: ForzaRevLimiterTuningConfiguration;
};

export const defaultForzaTuningValues = (): ForzaTuningValues => ({
  bodyRumbleMode: 'native_passthrough',
  effects: defaultForzaEffects(),
  abs: defaultForzaAbsTuning(),
  brake: defaultForzaBrakeTuning(),
  throttle: defaultForzaThrottleTuning(),
  shift: defaultForzaShiftTuning(),
  revLimiter: defaultForzaRevLimiterTuning()
});

export const forzaTuningFromConfig = (
  forza: ControllerConfiguration['forza'] | undefined
): ForzaTuningValues => ({
  bodyRumbleMode: normalizeForzaBodyRumbleMode(forza?.bodyRumbleMode),
  effects: normalizeForzaEffects(forza?.effects),
  abs: normalizeForzaAbsTuning(forza?.abs),
  brake: normalizeForzaBrakeTuning(forza?.brake),
  throttle: normalizeForzaThrottleTuning(forza?.throttle),
  shift: normalizeForzaShiftTuning(forza?.shift),
  revLimiter: normalizeForzaRevLimiterTuning(forza?.revLimiter)
});

export const forzaEffectById = (effects: ForzaEffectConfiguration[], id: string): ForzaEffectConfiguration =>
  effects.find((effect) => effect.id === id) ??
  defaultForzaEffects().find((effect) => effect.id === id) ??
  defaultForzaEffects()[0];

export const withForzaEffectUpdated = (
  effects: ForzaEffectConfiguration[],
  id: string,
  patch: Partial<ForzaEffectConfiguration>
): ForzaEffectConfiguration[] =>
  normalizeForzaEffects(
    effects.map((effect) =>
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

export type ForzaTuningStore = {
  get: () => ForzaTuningValues;
  set: (next: ForzaTuningValues) => void;
};

export type ForzaEffectStateOptions = {
  store: ForzaTuningStore;
  onChanged: () => void;
};

export const createForzaEffectState = ({ store, onChanged }: ForzaEffectStateOptions) => {
  const patchValues = (partial: Partial<ForzaTuningValues>) => {
    store.set({ ...store.get(), ...partial });
    onChanged();
  };

  const updateEffect = (id: string, patch: Partial<ForzaEffectConfiguration>) => {
    patchValues({ effects: withForzaEffectUpdated(store.get().effects, id, patch) });
  };

  return {
    updateEffect,
    applyShiftThumpPreset: (intensity: number) => {
      updateEffect('gear_shift_thump', {
        enabled: intensity > 0,
        intensity,
        route: 'r2_and_body'
      });
    },
    setAllEffectsEnabled: (enabled: boolean) => {
      patchValues({
        effects: normalizeForzaEffects(store.get().effects.map((effect) => ({ ...effect, enabled })))
      });
    },
    setBodyRumbleMode: (mode: ForzaBodyRumbleMode) => {
      patchValues({ bodyRumbleMode: normalizeForzaBodyRumbleMode(mode) });
    },
    updateAbsTuning: (patch: Partial<ForzaAbsTuningConfiguration>) => {
      patchValues({ abs: normalizeForzaAbsTuning({ ...store.get().abs, ...patch }) });
    },
    updateBrakeTuning: (patch: Partial<ForzaBrakeTuningConfiguration>) => {
      patchValues({ brake: normalizeForzaBrakeTuning({ ...store.get().brake, ...patch }) });
    },
    updateThrottleTuning: (patch: Partial<ForzaThrottleTuningConfiguration>) => {
      patchValues({ throttle: normalizeForzaThrottleTuning({ ...store.get().throttle, ...patch }) });
    },
    updateShiftTuning: (patch: Partial<ForzaShiftTuningConfiguration>) => {
      patchValues({ shift: normalizeForzaShiftTuning({ ...store.get().shift, ...patch }) });
    },
    updateRevLimiterTuning: (patch: Partial<ForzaRevLimiterTuningConfiguration>) => {
      patchValues({ revLimiter: normalizeForzaRevLimiterTuning({ ...store.get().revLimiter, ...patch }) });
    }
  };
};

export type ForzaEffectState = ReturnType<typeof createForzaEffectState>;
