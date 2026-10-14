import type { ControllerStatus } from './types';

export const controllerModelText = (item: ControllerStatus | undefined) => {
  if (!item) return 'No DualSense Connected';
  if (item.family === 'Unknown Sony') return 'Unknown Sony Controller';
  return item.name || item.family;
};

export const controllerConnectionText = (item: ControllerStatus | undefined) => {
  if (!item) return 'No controller detected';
  if (!item.connected) {
    if (item.diagnosticState === 'permission_denied') return 'Permission denied';
    if (item.diagnosticState === 'cannot_open') return 'Cannot open controller';
    return 'Controller disconnected';
  }
  return item.transport === 'Unknown' ? 'Connected' : item.transport;
};

export const shortControllerId = (id: string) => (id.length > 14 ? `${id.slice(0, 6)}...${id.slice(-5)}` : id);

export const controllerTransportDetail = (item: ControllerStatus): string => {
  if (!item.connected) {
    if (item.diagnosticState === 'permission_denied') return 'Permission denied';
    if (item.diagnosticState === 'cannot_open') return 'Cannot open';
    return 'Disconnected';
  }
  return item.transport === 'Unknown' ? 'Connected (transport unknown)' : `Connected via ${item.transport}`;
};

export const controllerBatteryDetail = (item: ControllerStatus): string => {
  if (typeof item.battery !== 'number' || item.batteryState === 'unknown') return 'Battery unknown';
  if (item.batteryState === 'full') return `${item.battery}% / full charge`;
  if (item.batteryState === 'charging') return `${item.battery}% / charging`;
  return `${item.battery}% / discharging`;
};

export const controllerPermissionDetail = (item: ControllerStatus): string => {
  if (item.permission === 'granted') return 'HID access granted';
  if (item.permission === 'denied') return 'HID access denied';
  return 'HID access status unknown';
};

export const controllerDiagnosticDetail = (item: ControllerStatus): string => {
  switch (item.diagnosticState) {
    case 'ok':
      return 'Healthy';
    case 'disconnected':
      return 'Disconnected from host';
    case 'permission_denied':
      return 'OS denied HID access';
    case 'cannot_open':
      return 'Could not open the HID device';
    case 'unsupported':
      return 'Hardware not recognised by DSCC';
    case 'faulted':
      return 'Device fault reported';
    default:
      return 'Unknown';
  }
};

export const controllerBatteryReadable = (item: ControllerStatus | undefined) =>
  Boolean(item?.connected && typeof item.battery === 'number' && item.batteryState !== 'unknown');

export const controllerBatteryFillWidth = (item: ControllerStatus | undefined) =>
  controllerBatteryReadable(item) ? Math.max(2, Math.round(((item?.battery ?? 0) / 100) * 20)) : 0;

export const controllerBatteryText = (item: ControllerStatus | undefined) => {
  const battery = item?.battery;
  if (!item || typeof battery !== 'number' || item.batteryState === 'unknown') return '';
  if (item.batteryState === 'full') return `${battery}% / full`;
  if (item.batteryState === 'charging') return `${battery}% / charging`;
  return `${battery}% battery`;
};
