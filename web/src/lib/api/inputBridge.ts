import type {
  InputBridgeBindingWriteRequest,
  InputBridgeBindingWriteResponse,
  InputBridgeSessionSummary,
  InputBridgeStatus
} from '../types';
import { apiFetch, isMockApiEnabled, loadMockApi } from './client';

export async function getInputBridgeStatus(): Promise<InputBridgeStatus> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).getMockInputBridgeStatus();
  return apiFetch<InputBridgeStatus>('/input-bridge');
}

export async function writeInputBridgeBinding(
  request: InputBridgeBindingWriteRequest
): Promise<InputBridgeBindingWriteResponse> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).writeMockInputBridgeBinding(request);
  return apiFetch<InputBridgeBindingWriteResponse>('/input-bridge/bindings', {
    method: 'POST',
    body: JSON.stringify(request)
  });
}

export async function startInputBridgeSession(controllerId: string): Promise<InputBridgeSessionSummary> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).startMockInputBridgeSession(controllerId);
  return apiFetch<InputBridgeSessionSummary>(`/input-bridge/sessions/${encodeURIComponent(controllerId)}/start`, {
    method: 'POST'
  });
}

export async function stopInputBridgeSession(controllerId: string): Promise<InputBridgeSessionSummary> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).stopMockInputBridgeSession(controllerId);
  return apiFetch<InputBridgeSessionSummary>(`/input-bridge/sessions/${encodeURIComponent(controllerId)}/stop`, {
    method: 'POST'
  });
}
