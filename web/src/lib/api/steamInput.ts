import type {
  SteamInputBindingWriteRequest,
  SteamInputBindingWriteResponse,
  SteamInputPaddlePresetRequest,
  SteamInputPaddlePresetResponse
} from '../types';
import { apiFetch, isMockApiEnabled, loadMockApi } from './client';

export async function writeSteamInputBinding(
  request: SteamInputBindingWriteRequest
): Promise<SteamInputBindingWriteResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).writeMockSteamInputBinding(request);
  return apiFetch<SteamInputBindingWriteResponse>('/steam-input/bindings', {
    method: 'POST',
    body: JSON.stringify(request)
  });
}

export async function writeSteamInputPaddlePreset(
  request: SteamInputPaddlePresetRequest
): Promise<SteamInputPaddlePresetResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) {
    throw new Error('Steam Input paddle presets require the real DSCC agent.');
  }
  return apiFetch<SteamInputPaddlePresetResponse>('/steam-input/paddle-preset', {
    method: 'POST',
    body: JSON.stringify(request)
  });
}
