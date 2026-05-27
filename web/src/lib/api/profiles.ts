import type { ControllerConfiguration, ExportedProfile, ProfileResolution, ProfileSummary } from '../types';
import { ApiRequestError, apiFetch, isMockApiEnabled, loadMockApi, type ActionAcceptedDto } from './client';
import { mapProfile, type ProfileDto } from './snapshot';

export async function saveProfileConfig(
  profileId: string,
  config: Omit<ControllerConfiguration, 'controllerId' | 'model'>
): Promise<ActionAcceptedDto> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).saveMockProfileConfig(profileId, config);
  return apiFetch<ActionAcceptedDto>(`/profiles/${encodeURIComponent(profileId)}/config`, {
    method: 'PUT',
    body: JSON.stringify(config)
  });
}

export async function setProfileOverride(request: {
  controllerId?: string | null;
  gameId?: string | null;
  profileId: string;
}): Promise<ProfileResolution> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).setMockProfileOverride(request);
  return apiFetch<ProfileResolution>('/profile-resolution/override', {
    method: 'PUT',
    body: JSON.stringify(request)
  });
}

export async function clearProfileOverride(request?: {
  controllerId?: string | null;
  gameId?: string | null;
}): Promise<ProfileResolution> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).clearMockProfileOverride(request);
  const params = new URLSearchParams();
  if (request?.controllerId) params.set('controllerId', request.controllerId);
  if (request?.gameId) params.set('gameId', request.gameId);
  const query = params.toString();
  return apiFetch<ProfileResolution>(`/profile-resolution/override${query ? `?${query}` : ''}`, {
    method: 'DELETE'
  });
}

export async function activateProfile(profileId: string): Promise<ActionAcceptedDto> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).activateMockProfile(profileId);
  return apiFetch<ActionAcceptedDto>(`/profiles/${encodeURIComponent(profileId)}/activate`, {
    method: 'POST'
  });
}

export async function createProfile(name: string, options?: { gameId?: string | null }): Promise<ProfileSummary> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).createMockProfile(name, options);
  const dto = await apiFetch<ProfileDto>('/profiles', {
    method: 'POST',
    body: JSON.stringify({ name, gameId: options?.gameId ?? null })
  });
  return mapProfile(dto);
}

export async function renameProfile(profileId: string, name: string): Promise<ProfileSummary> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).renameMockProfile(profileId, name);
  const dto = await apiFetch<ProfileDto>(`/profiles/${encodeURIComponent(profileId)}`, {
    method: 'PUT',
    body: JSON.stringify({ name })
  });
  return mapProfile(dto);
}

export async function exportProfile(profileId: string): Promise<ExportedProfile> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).exportMockProfile(profileId);
  return apiFetch<ExportedProfile>(`/profiles/${encodeURIComponent(profileId)}/export`);
}

export async function importProfile(profile: {
  schema: string;
  id?: string | null;
  name: string;
  config?: ExportedProfile['config'];
}): Promise<ProfileSummary> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).importMockProfile(profile);
  const dto = await apiFetch<ProfileDto>('/profiles/import', {
    method: 'POST',
    body: JSON.stringify(profile)
  });
  return mapProfile(dto);
}

export async function deleteProfile(profileId: string): Promise<ActionAcceptedDto | void> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).deleteMockProfile(profileId);
  try {
    return await apiFetch<ActionAcceptedDto | void>(`/profiles/${encodeURIComponent(profileId)}`, {
      method: 'DELETE'
    });
  } catch (caught) {
    if (caught instanceof ApiRequestError && caught.status === 404) {
      return { accepted: true, message: 'Profile was already deleted' };
    }
    throw caught;
  }
}
