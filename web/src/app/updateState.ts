import type { AppUpdateCheck } from '../lib/types';

export const UPDATE_RELEASE_PAGE_URL = 'https://github.com/shiftedx/dualsense-command/releases/latest';
export const UPDATE_DISMISSED_VERSION_KEY = 'dscc-update-dismissed-version';

export type UpdateCheckState = {
  state: 'idle' | 'checking' | 'current' | 'available' | 'error';
  currentVersion?: string;
  latestVersion?: string;
  releaseUrl?: string;
  message?: string;
};

export const normalizeVersion = (value: string | undefined | null) => (value ?? '').trim().replace(/^v/i, '');

export function updateCheckStateFromResult(result: AppUpdateCheck): UpdateCheckState {
  return result.updateAvailable
    ? {
        state: 'available',
        currentVersion: result.currentVersion,
        latestVersion: result.latestVersion,
        releaseUrl: result.releaseUrl
      }
    : {
        state: 'current',
        currentVersion: result.currentVersion,
        latestVersion: result.latestVersion,
        releaseUrl: result.releaseUrl
      };
}

export function updateCheckErrorState(currentVersion: string, caught: unknown): UpdateCheckState {
  return {
    state: 'error',
    currentVersion,
    message: caught instanceof Error ? caught.message : 'Update check failed'
  };
}

export function readDismissedUpdateVersion(): string {
  if (typeof window === 'undefined') return '';
  try {
    return window.localStorage.getItem(UPDATE_DISMISSED_VERSION_KEY) ?? '';
  } catch {
    return '';
  }
}

export function writeDismissedUpdateVersion(version: string): void {
  if (typeof window === 'undefined' || !version) return;
  try {
    window.localStorage.setItem(UPDATE_DISMISSED_VERSION_KEY, version);
  } catch {
    // Dismissal is convenience state; failing to persist it should not block use.
  }
}

