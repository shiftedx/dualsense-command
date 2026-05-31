import type { ActionAccepted } from '../types';

export const API_BASE = '/api';
export const jsonHeaders = {
  'Content-Type': 'application/json'
};

const MOCK_STORAGE_KEY = 'dscc.mockApi.enabled';
const AGENT_HOME_URL = 'http://127.0.0.1:43473/';

type MockApiModule = typeof import('../mock/api');
let mockApiPromise: Promise<MockApiModule> | null = null;
let queryMockToggleApplied = false;

export interface ActionAcceptedDto {
  accepted: boolean;
  message: string;
  dry_run?: boolean;
  dryRun?: boolean;
}

export class ApiRequestError extends Error {
  constructor(
    message: string,
    readonly status: number | null = null,
    readonly networkFailure = false
  ) {
    super(message);
  }
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      ...init,
      headers: {
        ...jsonHeaders,
        ...init?.headers
      }
    });
  } catch (caught) {
    const detail = caught instanceof Error ? caught.message : 'network request failed';
    throw new ApiRequestError(apiNetworkFailureMessage(detail), null, true);
  }

  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new ApiRequestError(apiHttpFailureMessage(response, detail), response.status);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

export async function apiAction(path: string, init?: RequestInit): Promise<ActionAccepted> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      ...init,
      headers: {
        ...jsonHeaders,
        ...init?.headers
      }
    });
  } catch (caught) {
    const detail = caught instanceof Error ? caught.message : 'network request failed';
    throw new ApiRequestError(apiNetworkFailureMessage(detail), null, true);
  }

  const text = await response.text().catch(() => '');
  const parsed = parseActionAccepted(text);
  if (!response.ok && !parsed) {
    throw new ApiRequestError(apiHttpFailureMessage(response, text), response.status);
  }

  return (
    parsed ?? {
      accepted: response.ok,
      message: response.ok ? 'Action accepted.' : `Request failed: ${response.status} ${response.statusText}`
    }
  );
}

export async function loadMockApi(): Promise<MockApiModule> {
  if (!import.meta.env.DEV) {
    throw new Error('Mock API is unavailable in production builds.');
  }
  mockApiPromise ??= import('../mock/api');
  return mockApiPromise;
}

export function isMockApiEnabled(): boolean {
  if (!import.meta.env.DEV) return false;
  return queryMockModeSetting() ?? storedMockModeSetting() ?? envMockModeDefault();
}

export function webSocketUrl(path: string): string {
  const url = new URL(`${API_BASE}${path}`, window.location.href);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}

function apiNetworkFailureMessage(detail: string): string {
  return `DSCC agent is not reachable. ${browserLocationHint()} Details: ${detail}`;
}

function apiHttpFailureMessage(response: Response, detail: string): string {
  if (response.status === 403) {
    return `DSCC blocked this request because the browser address does not match the local agent. Open DSCC from the tray or ${AGENT_HOME_URL}, then try again.`;
  }

  const action = parseActionAccepted(detail);
  if (action?.message) return action.message;

  return `API request failed: ${response.status} ${response.statusText}${detail ? ` / ${detail}` : ''}`;
}

function browserLocationHint(): string {
  if (typeof window === 'undefined') {
    return `Start DSCC and open ${AGENT_HOME_URL}.`;
  }

  if (window.location.protocol === 'file:') {
    return `The UI was opened from a file. Open DSCC from the tray or use ${AGENT_HOME_URL}.`;
  }

  const allowedHosts = new Set(['127.0.0.1:43473', 'localhost:43473']);
  if (!allowedHosts.has(window.location.host)) {
    return `This page is running at ${window.location.host}. Open the packaged DSCC UI from the tray or use ${AGENT_HOME_URL}.`;
  }

  return `Start DSCC from the Start menu, then open ${AGENT_HOME_URL}.`;
}

function parseActionAccepted(text: string): ActionAccepted | null {
  if (!text.trim()) return null;
  try {
    const dto = JSON.parse(text) as ActionAcceptedDto;
    if (typeof dto.accepted !== 'boolean' || typeof dto.message !== 'string') return null;
    return {
      accepted: dto.accepted,
      message: dto.message,
      dryRun: dto.dryRun ?? dto.dry_run
    };
  } catch {
    return null;
  }
}

function queryMockModeSetting(): boolean | null {
  if (queryMockToggleApplied || typeof window === 'undefined') return null;
  queryMockToggleApplied = true;

  const raw = new URLSearchParams(window.location.search).get('mock');
  const parsed = parseToggleValue(raw);
  if (parsed === null) return null;

  writeStoredMockModeSetting(parsed);
  return parsed;
}

function storedMockModeSetting(): boolean | null {
  if (typeof window === 'undefined') return null;
  try {
    return parseToggleValue(window.localStorage.getItem(MOCK_STORAGE_KEY));
  } catch {
    return null;
  }
}

function writeStoredMockModeSetting(enabled: boolean): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(MOCK_STORAGE_KEY, enabled ? '1' : '0');
  } catch {
    // localStorage can be unavailable in locked-down browser contexts.
  }
}

function envMockModeDefault(): boolean {
  return (
    parseToggleValue(import.meta.env.VITE_DSCC_MOCK_API ?? import.meta.env.VITE_DSCC_MOCK) ??
    import.meta.env.MODE === 'mock'
  );
}

function parseToggleValue(value: string | null | undefined): boolean | null {
  if (value === null || value === undefined) return null;
  const normalized = value.trim().toLowerCase();
  if (['1', 'true', 'yes', 'on'].includes(normalized)) return true;
  if (['0', 'false', 'no', 'off'].includes(normalized)) return false;
  return null;
}
