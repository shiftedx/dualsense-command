import { steamBindingTargetPart } from './buttonMapping';
import type { SteamBindingSlot, SteamMirrorGroup } from './buttonMapping';
import type { ControllerStatus, SteamInputBinding } from '../../types';

export type RawBindingTargetGroup = {
  label: string;
  options: Array<{ label: string; raw: string }>;
};

export type PreparedSteamBindingTargetGroup = {
  label: string;
  options: Array<{ label: string; raw: string; targetKey: string; searchText: string }>;
};

export type ButtonMappingProviderKind = 'steam' | 'bridge';

export type ButtonMappingViewSession = {
  active: boolean;
  steamInputRunning: boolean;
  providerLabel: string;
  providerKind: ButtonMappingProviderKind;
  providerOnline: boolean;
  mappingAvailabilityMessage: string;
  mappingReadOnly: boolean;
  defaultMirrorOnly: boolean;
  controllerHeaderName: string;
  controllerTransport: ControllerStatus['transport'] | undefined;
  gameName: string;
  steamLayoutTitle: string;
  mappedVisibleChipCount: number;
  steamMirrorGroups: SteamMirrorGroup[];
  focusedSlotMeta: SteamBindingSlot | null;
  focusedSlotBinding: SteamInputBinding | null;
  focusedSlotSelectedBinding: SteamInputBinding | null;
  steamBindingBusy: boolean;
  steamInputLayoutAvailable: boolean;
  paddlePresetVisible: boolean;
  paddlePresetAvailable: boolean;
  paddlePresetStatus: string;
  paddlePresetLeftKey: string;
  paddlePresetRightKey: string;
  steamBindingDraft: string;
  steamBindingLabelDraft: string;
  bindingLabelFieldLabel: string;
  rawFieldLabel: string;
  rawFieldPlaceholder: string;
  targetGroups: PreparedSteamBindingTargetGroup[];
  onSelectSlot: (slot: SteamBindingSlot) => void;
  onHoverSlot: (slot: SteamBindingSlot | null) => void;
  onPaddlePresetLeftKeyChange: (nextKey: string) => void;
  onPaddlePresetRightKeyChange: (nextKey: string) => void;
  onApplyPaddlePreset: () => void | Promise<void>;
  onTargetChange: (rawOption: string) => void;
  onLabelChange: (nextLabel: string) => void;
  onRawDraftChange: (nextRaw: string) => void;
  onResetDraft: () => void;
  onSaveBinding: () => void | Promise<void>;
};

export const EMPTY_BUTTON_MAPPING_VIEW_SESSION: ButtonMappingViewSession = {
  active: false,
  steamInputRunning: false,
  providerLabel: 'Steam Input',
  providerKind: 'steam',
  providerOnline: false,
  mappingAvailabilityMessage: '',
  mappingReadOnly: false,
  defaultMirrorOnly: false,
  controllerHeaderName: '',
  controllerTransport: undefined,
  gameName: 'No supported game selected',
  steamLayoutTitle: 'Steam Input Layout',
  mappedVisibleChipCount: 0,
  steamMirrorGroups: [],
  focusedSlotMeta: null,
  focusedSlotBinding: null,
  focusedSlotSelectedBinding: null,
  steamBindingBusy: false,
  steamInputLayoutAvailable: false,
  paddlePresetVisible: false,
  paddlePresetAvailable: false,
  paddlePresetStatus: '',
  paddlePresetLeftKey: 'Q',
  paddlePresetRightKey: 'E',
  steamBindingDraft: '',
  steamBindingLabelDraft: '',
  bindingLabelFieldLabel: 'Label (Steam UI)',
  rawFieldLabel: 'Raw VDF',
  rawFieldPlaceholder: 'xinput_button ... / key_press ...',
  targetGroups: [],
  onSelectSlot: () => {},
  onHoverSlot: () => {},
  onPaddlePresetLeftKeyChange: () => {},
  onPaddlePresetRightKeyChange: () => {},
  onApplyPaddlePreset: () => {},
  onTargetChange: () => {},
  onLabelChange: () => {},
  onRawDraftChange: () => {},
  onResetDraft: () => {},
  onSaveBinding: () => {}
};

export function prepareBindingTargetGroups(groups: RawBindingTargetGroup[]): PreparedSteamBindingTargetGroup[] {
  return groups.map((group) => ({
    label: group.label,
    options: group.options.map((option) => ({
      ...option,
      targetKey: steamBindingTargetPart(option.raw),
      searchText: `${group.label} ${option.label} ${option.raw}`.toLowerCase()
    }))
  }));
}

export function bindingTargetGroupsForProvider(
  groups: PreparedSteamBindingTargetGroup[],
  provider: ButtonMappingProviderKind
): PreparedSteamBindingTargetGroup[] {
  if (provider === 'steam') return groups;
  return groups
    .map((group) => ({
      ...group,
      options: group.options.filter((option) => {
        const raw = option.raw.trim().toLowerCase();
        return raw.startsWith('xinput_button ') && !raw.includes('touchpad');
      })
    }))
    .filter((group) => group.options.length > 0);
}
