import { controllerModelText } from '../lib/controllerDisplay';
import type { ControllerStatus } from '../lib/types';

export function resolveSelectedControllerId(
  controllers: ControllerStatus[],
  selectedControllerId: string
): string {
  const connectedControllers = controllers.filter((item) => item.connected);
  if (connectedControllers.length > 0 && !connectedControllers.some((item) => item.id === selectedControllerId)) {
    return connectedControllers[0].id;
  }
  if (connectedControllers.length === 0 && controllers.length > 0 && !controllers.some((item) => item.id === selectedControllerId)) {
    return controllers[0].id;
  }
  return selectedControllerId;
}

export function selectCurrentController(
  controllers: ControllerStatus[],
  selectedControllerId: string
): ControllerStatus | undefined {
  const connectedControllers = controllers.filter((item) => item.connected);
  return connectedControllers.find((item) => item.id === selectedControllerId) ?? connectedControllers[0];
}

export function reconcileProfileTargetControllerIds(
  profileTargetControllerIds: string[],
  connectedControllerIds: string[],
  selectedControllerId: string
): string[] {
  const validTargets = profileTargetControllerIds.filter((id) => connectedControllerIds.includes(id));
  const fallbackTarget =
    selectedControllerId && connectedControllerIds.includes(selectedControllerId)
      ? selectedControllerId
      : connectedControllerIds[0];
  return validTargets.length ? validTargets : fallbackTarget ? [fallbackTarget] : [];
}

export function profileTargetsCoverAllConnectedControllers(
  ids: string[],
  connectedControllerIds: string[]
): boolean {
  return (
    connectedControllerIds.length > 1 &&
    ids.length === connectedControllerIds.length &&
    connectedControllerIds.every((id) => ids.includes(id))
  );
}

export function resolveSelectedProfileTargetIds(options: {
  profileTargetControllerIds: string[];
  connectedControllerIds: string[];
  controllerId?: string | null;
}): string[] {
  const validTargets = options.profileTargetControllerIds.filter((id) =>
    options.connectedControllerIds.includes(id)
  );
  if (validTargets.length) return validTargets;
  return options.controllerId ? [options.controllerId] : [];
}

export function profileTargetName(id: string, controllers: ControllerStatus[]): string {
  const target = controllers.find((item) => item.id === id);
  return target?.name || controllerModelText(target);
}

export function summarizeProfileTargets(options: {
  targetIds: string[];
  controllers: ControllerStatus[];
  connectedControllerIds: string[];
}): string {
  if (profileTargetsCoverAllConnectedControllers(options.targetIds, options.connectedControllerIds)) {
    return 'all connected controllers';
  }
  const names = options.targetIds.map((id) => profileTargetName(id, options.controllers)).filter(Boolean);
  if (names.length <= 2) return names.join(', ') || 'selected controller';
  return `${names.slice(0, 2).join(', ')} +${names.length - 2}`;
}
