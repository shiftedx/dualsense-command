import type { AppSettingsResponse, AppUpdateCheck, SupportBundle } from '../types';
import { ApiRequestError, apiFetch, isMockApiEnabled, loadMockApi } from './client';

const UPDATE_RELEASE_PAGE_URL = 'https://github.com/shiftedx/dualsense-command/releases/latest';
const UPDATE_CHECK_API_URL = 'https://api.github.com/repos/shiftedx/dualsense-command/releases/latest';

type AppUpdateCheckDto = {
  currentVersion?: unknown;
  current_version?: unknown;
  latestVersion?: unknown;
  latest_version?: unknown;
  updateAvailable?: unknown;
  update_available?: unknown;
  releaseUrl?: unknown;
  release_url?: unknown;
  checkedAt?: unknown;
  checked_at?: unknown;
  message?: unknown;
  error?: unknown;
  state?: unknown;
};

type GitHubReleaseResponse = {
  tag_name?: unknown;
  html_url?: unknown;
};

export async function getSupportBundle(): Promise<SupportBundle> {
  try {
    return await apiFetch<SupportBundle>('/diagnostics/support-bundle');
  } catch (caught) {
    if (caught instanceof ApiRequestError && caught.status === 404) {
      return apiFetch<SupportBundle>('/support-bundle');
    }
    throw caught;
  }
}

export async function getAppUpdateCheck(currentVersion: string): Promise<AppUpdateCheck> {
  const normalizedCurrent = normalizeVersion(currentVersion);
  if (!normalizedCurrent || normalizedCurrent.toLowerCase() === 'unknown') {
    throw new Error('Current app version is unavailable.');
  }

  if (!import.meta.env.DEV || !isMockApiEnabled()) {
    const params = new URLSearchParams({ currentVersion: normalizedCurrent });
    try {
      const dto = await apiFetch<AppUpdateCheckDto>(`/update-check?${params.toString()}`);
      return normalizeUpdateCheckDto(dto, normalizedCurrent, 'agent');
    } catch (caught) {
      if (!shouldFallbackToGitHubUpdateCheck(caught)) throw caught;
    }
  }

  return getGitHubUpdateCheck(normalizedCurrent);
}

export async function saveAppSettings(request: {
  listenOnAllInterfaces?: boolean;
  forzaPlaystationGlyphs?: {
    enabled: boolean;
    installPath?: string | null;
  };
}): Promise<AppSettingsResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).saveMockAppSettings(request);
  return apiFetch<AppSettingsResponse>('/app-settings', {
    method: 'PUT',
    body: JSON.stringify(request)
  });
}

async function getGitHubUpdateCheck(currentVersion: string): Promise<AppUpdateCheck> {
  const response = await fetch(UPDATE_CHECK_API_URL, {
    headers: { Accept: 'application/vnd.github+json' },
    cache: 'no-store'
  });
  if (!response.ok) throw new Error(`Release lookup returned ${response.status}`);

  const release = (await response.json()) as GitHubReleaseResponse;
  const latestVersion = normalizeVersion(typeof release.tag_name === 'string' ? release.tag_name : '');
  if (!latestVersion) throw new Error('Release response did not include a tag.');

  return {
    currentVersion,
    latestVersion,
    updateAvailable: isVersionNewer(latestVersion, currentVersion),
    releaseUrl: typeof release.html_url === 'string' ? release.html_url : UPDATE_RELEASE_PAGE_URL,
    source: 'github',
    checkedAt: null,
    message: null
  };
}

function shouldFallbackToGitHubUpdateCheck(caught: unknown): boolean {
  return (
    caught instanceof ApiRequestError &&
    (caught.networkFailure || caught.status === 404 || caught.status === 405 || caught.status === 501)
  );
}

function normalizeUpdateCheckDto(
  dto: AppUpdateCheckDto,
  currentVersion: string,
  source: AppUpdateCheck['source']
): AppUpdateCheck {
  const dtoCurrentVersion = normalizeVersion(stringValue(dto.currentVersion ?? dto.current_version)) || currentVersion;
  const state = stringValue(dto.state);
  const latestVersion = normalizeVersion(stringValue(dto.latestVersion ?? dto.latest_version)) || dtoCurrentVersion;
  const updateAvailableValue = dto.updateAvailable ?? dto.update_available;
  const updateAvailable =
    typeof updateAvailableValue === 'boolean'
      ? updateAvailableValue
      : state === 'update_available' || isVersionNewer(latestVersion, dtoCurrentVersion);

  return {
    currentVersion: dtoCurrentVersion,
    latestVersion,
    updateAvailable,
    releaseUrl: stringValue(dto.releaseUrl ?? dto.release_url) || UPDATE_RELEASE_PAGE_URL,
    source,
    checkedAt: stringValue(dto.checkedAt ?? dto.checked_at) || null,
    message: stringValue(dto.message) || stringValue(dto.error) || null
  };
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function normalizeVersion(value: string | undefined | null): string {
  return (value ?? '').trim().replace(/^v/i, '');
}

function versionParts(value: string): number[] {
  return normalizeVersion(value).match(/\d+/g)?.slice(0, 4).map(Number) ?? [];
}

function isVersionNewer(candidate: string, current: string): boolean {
  const candidateParts = versionParts(candidate);
  const currentParts = versionParts(current);
  if (!candidateParts.length || !currentParts.length) return false;
  const length = Math.max(candidateParts.length, currentParts.length);
  for (let index = 0; index < length; index += 1) {
    const left = candidateParts[index] ?? 0;
    const right = currentParts[index] ?? 0;
    if (left !== right) return left > right;
  }
  return false;
}
