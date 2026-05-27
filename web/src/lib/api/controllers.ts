import type {
  ActionAccepted,
  ControllerConfiguration,
  ControllerInputState,
  ControllerOutputFrame,
  ControllerStatus,
  EdgeProfilesResponse,
  EffectTestRequest,
  UpdateEdgeProfileRequest
} from '../types';
import { apiAction, apiFetch, isMockApiEnabled, loadMockApi } from './client';
import { mapController, type ControllerDto } from './snapshot';

interface EffectTestResponseDto {
  accepted: boolean;
  message: string;
  dry_run: boolean;
  duration_ms: number;
  output: ControllerOutputFrame;
}

interface ControllerInputResponseDto {
  controllerId?: string;
  controller_id?: string;
  available: boolean;
  source: string;
  message: string;
  sampledAtMs?: number | null;
  sampled_at_ms?: number | null;
  ageMs?: number | null;
  age_ms?: number | null;
  axes?: {
    leftStick?: { x?: number; y?: number; magnitude?: number };
    left_stick?: { x?: number; y?: number; magnitude?: number };
    rightStick?: { x?: number; y?: number; magnitude?: number };
    right_stick?: { x?: number; y?: number; magnitude?: number };
  };
  triggers?: {
    l2?: number;
    r2?: number;
  };
  buttons?: Array<{
    id?: string;
    label?: string;
    pressed?: boolean;
    value?: number;
  }>;
}

export async function runEffectTest(
  request: EffectTestRequest,
  controllerId?: string | null
): Promise<{
  accepted: true;
  message: string;
  dryRun: boolean;
  durationMs: number;
  output: ControllerOutputFrame;
}> {
  const safeRequest: EffectTestRequest = {
    ...request,
    intensity: Math.max(0, Math.min(100, request.intensity)),
    startPosition:
      request.startPosition === undefined ? undefined : Math.max(0, Math.min(1, request.startPosition)),
    l2Position: request.l2Position === undefined ? undefined : Math.max(0, Math.min(1, request.l2Position)),
    r2Position: request.r2Position === undefined ? undefined : Math.max(0, Math.min(1, request.r2Position)),
    durationMs: Math.max(100, Math.min(60000, request.durationMs))
  };

  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).runMockEffectTest(safeRequest);

  const endpoint = controllerId
    ? `/controllers/${encodeURIComponent(controllerId)}/test-effect`
    : '/controllers/current/test-effect';
  const response = await apiFetch<EffectTestResponseDto>(endpoint, {
    method: 'POST',
    body: JSON.stringify(safeRequest)
  });

  if (!response.accepted) {
    throw new Error(response.message);
  }

  return {
    accepted: true,
    message: response.message,
    dryRun: response.dry_run,
    durationMs: response.duration_ms,
    output: response.output
  };
}

export async function getControllerInput(controllerId?: string | null): Promise<ControllerInputState> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).getMockControllerInput(controllerId);
  const endpoint = controllerId
    ? `/controllers/${encodeURIComponent(controllerId)}/input`
    : '/controllers/current/input';
  const response = await apiFetch<ControllerInputResponseDto>(endpoint);
  const leftStick = response.axes?.leftStick ?? response.axes?.left_stick;
  const rightStick = response.axes?.rightStick ?? response.axes?.right_stick;

  return {
    controllerId: response.controllerId ?? response.controller_id ?? '',
    available: response.available,
    source: response.source,
    message: response.message,
    sampledAtMs: response.sampledAtMs ?? response.sampled_at_ms ?? null,
    ageMs: response.ageMs ?? response.age_ms ?? null,
    axes: {
      leftStick: normalizeInputStick(leftStick),
      rightStick: normalizeInputStick(rightStick)
    },
    triggers: {
      l2: clampUnitNumber(response.triggers?.l2),
      r2: clampUnitNumber(response.triggers?.r2)
    },
    buttons: (response.buttons ?? []).map((button) => ({
      id: button.id ?? '',
      label: button.label ?? button.id ?? 'Input',
      pressed: Boolean(button.pressed),
      value: clampUnitNumber(button.value)
    }))
  };
}

export async function getControllerConfig(controllerId: string): Promise<ControllerConfiguration> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).getMockControllerConfig(controllerId);
  return apiFetch<ControllerConfiguration>(`/controllers/${encodeURIComponent(controllerId)}/config`);
}

export async function saveControllerConfig(
  controllerId: string,
  config: Omit<ControllerConfiguration, 'controllerId' | 'model'>
): Promise<ControllerConfiguration> {
  if (import.meta.env.DEV && isMockApiEnabled()) {
    return (await loadMockApi()).saveMockControllerConfig(controllerId, config);
  }
  return apiFetch<ControllerConfiguration>(`/controllers/${encodeURIComponent(controllerId)}/config`, {
    method: 'PUT',
    body: JSON.stringify(config)
  });
}

export async function getEdgeProfiles(controllerId: string): Promise<EdgeProfilesResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) {
    throw new Error('DualSense Edge onboard profile read/write requires the real DSCC agent.');
  }
  return apiFetch<EdgeProfilesResponse>(`/controllers/${encodeURIComponent(controllerId)}/edge-profiles`);
}

export async function writeEdgeProfile(
  controllerId: string,
  slotId: string,
  request: UpdateEdgeProfileRequest
): Promise<ActionAccepted> {
  if (import.meta.env.DEV && isMockApiEnabled()) {
    throw new Error('DualSense Edge onboard profile read/write requires the real DSCC agent.');
  }
  return apiAction(`/controllers/${encodeURIComponent(controllerId)}/edge-profiles/${encodeURIComponent(slotId)}`, {
    method: 'PUT',
    body: JSON.stringify(request)
  });
}

export async function updateControllerName(controllerId: string, name: string): Promise<ControllerStatus> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).renameMockController(controllerId, name);
  const dto = await apiFetch<ControllerDto>(`/controllers/${encodeURIComponent(controllerId)}`, {
    method: 'PUT',
    body: JSON.stringify({ name })
  });
  return mapController(dto);
}

function normalizeInputStick(stick?: { x?: number; y?: number; magnitude?: number }) {
  const x = clampSignedNumber(stick?.x);
  const y = clampSignedNumber(stick?.y);
  const magnitude =
    typeof stick?.magnitude === 'number' && Number.isFinite(stick.magnitude)
      ? clampUnitNumber(stick.magnitude)
      : clampUnitNumber(Math.hypot(x, y));

  return { x, y, magnitude };
}

function clampUnitNumber(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0;
}

function clampSignedNumber(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.min(1, Math.max(-1, value)) : 0;
}
