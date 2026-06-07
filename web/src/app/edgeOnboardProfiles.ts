import type {
  ControllerStatus,
  EdgeProfileSlot,
  EdgeProfilesResponse,
  UpdateEdgeProfileRequest
} from '../lib/types';
import type { EditableControllerConfig } from './profileDraft';

export type EdgeOnboardProfileState = {
  loadedFor: string;
  profiles: EdgeProfilesResponse | null;
  loading: boolean;
  busySlot: string;
  error: string;
};

export const EDGE_ONBOARD_SLOTS_READ_TOOLTIP =
  'Reads onboard slots from a DualSense Edge over USB or Bluetooth when Windows exposes HID feature-report access. Fallback controllers only show local staged status.';

export function emptyEdgeOnboardProfileState(): EdgeOnboardProfileState {
  return {
    loadedFor: '',
    profiles: null,
    loading: false,
    busySlot: '',
    error: ''
  };
}

export function isEdgeTargetController(controller: ControllerStatus | null | undefined): boolean {
  return controller?.family === 'DualSense Edge';
}

export function shouldReadEdgeOnboardProfiles(options: {
  controller: ControllerStatus | null | undefined;
  loadedFor: string;
  profiles: EdgeProfilesResponse | null;
  loading: boolean;
  force?: boolean;
}): boolean {
  if (!isEdgeTargetController(options.controller) || !options.controller?.id) return false;
  if (!options.force && options.loadedFor === options.controller.id && (options.profiles || options.loading)) {
    return false;
  }
  return true;
}

export function shouldResetEdgeOnboardProfiles(options: {
  controller: ControllerStatus | null | undefined;
  loadedFor: string;
  profiles: EdgeProfilesResponse | null;
}): boolean {
  return !isEdgeTargetController(options.controller) && Boolean(options.loadedFor || options.profiles);
}

export function edgeSlotStatus(slot: EdgeProfileSlot): string {
  if (slot.state === 'default') return 'default';
  if (slot.hardwareSynced) return 'on controller';
  if (slot.staged) return 'staged';
  return slot.state.replaceAll('_', ' ');
}

export function edgeSlotName(slot: EdgeProfileSlot): string {
  return slot.name || slot.staged?.name || 'Empty Slot';
}

export function edgeSlotInfoTooltip(slot: EdgeProfileSlot): string {
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
}

export function edgeSlotWriteTooltip(
  slot: EdgeProfileSlot,
  edgeProfiles: EdgeProfilesResponse | null
): string {
  return edgeProfiles?.supportState === 'read_write'
    ? `Writes the current trigger ranges, lightbar color, stick presets, and supported button remaps to ${slot.shortcut}. Live telemetry effects still require DSCC to be running.`
    : `Stages the current trigger ranges, lightbar color, stick presets, and supported button remaps for ${slot.shortcut}. Connect the DualSense Edge over USB or Bluetooth, then read slots again to sync controller memory.`;
}

export function edgeSlotWriteLabel(edgeProfiles: EdgeProfilesResponse | null): string {
  return edgeProfiles?.supportState === 'read_write' ? 'Write' : 'Stage';
}

export function edgeProfileNameForSlot(slot: EdgeProfileSlot, profileName: string): string {
  const sourceName = profileName || 'DSCC Profile';
  return `${sourceName} ${slot.shortcut.replace('Fn + ', '')}`.trim().slice(0, 64);
}

export function edgeProfileWriteRequest(options: {
  slot: EdgeProfileSlot;
  profileName: string;
  config: EditableControllerConfig;
}): UpdateEdgeProfileRequest {
  return {
    name: edgeProfileNameForSlot(options.slot, options.profileName),
    trigger: options.config.trigger,
    lightbar: options.config.lightbar,
    sticks: options.config.sticks,
    buttons: options.config.buttons
  };
}
