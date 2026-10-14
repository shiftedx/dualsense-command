import type {
  AddCustomGameResponse,
  AddLocalAppRequest,
  SteamLibraryBrowseResponse,
  SteamLibraryResponse,
  ValidateLocalAppRequest,
  ValidateLocalAppResponse
} from '../types';
import { apiFetch, isMockApiEnabled, jsonHeaders, loadMockApi } from './client';

export async function getSteamLibrary(): Promise<SteamLibraryResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).getMockSteamLibrary();
  return apiFetch<SteamLibraryResponse>('/games/steam-library');
}

export async function addCustomGame(
  appId: string,
  processNames: string[] = []
): Promise<AddCustomGameResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).addMockCustomGame(appId, processNames);
  const body: { appId: string; processNames?: string[] } = { appId };
  if (processNames.length > 0) body.processNames = processNames;
  return apiFetch<AddCustomGameResponse>('/games/custom', {
    method: 'POST',
    headers: jsonHeaders,
    body: JSON.stringify(body)
  });
}

export async function validateLocalApp(request: ValidateLocalAppRequest): Promise<ValidateLocalAppResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).validateMockLocalApp(request);
  return apiFetch<ValidateLocalAppResponse>('/games/local/validate', {
    method: 'POST',
    body: JSON.stringify(request)
  });
}

export async function addLocalApp(request: AddLocalAppRequest): Promise<AddCustomGameResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).addMockLocalApp(request);
  return apiFetch<AddCustomGameResponse>('/games/local', {
    method: 'POST',
    body: JSON.stringify(request)
  });
}

export async function removeCustomGame(gameId: string): Promise<void> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).removeMockCustomGame(gameId);
  await apiFetch<void>(`/games/custom/${encodeURIComponent(gameId)}`, {
    method: 'DELETE'
  });
}

export async function browseSteamLibrary(appId: string, path = ''): Promise<SteamLibraryBrowseResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).browseMockSteamLibrary(appId, path);
  const qs = new URLSearchParams({ appId });
  if (path) qs.set('path', path);
  return apiFetch<SteamLibraryBrowseResponse>(`/games/steam-library/browse?${qs.toString()}`);
}
