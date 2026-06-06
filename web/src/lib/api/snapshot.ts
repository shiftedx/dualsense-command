// Snapshot transport: HTTP fetch + WebSocket lifecycle. All DTO->UI mapping and
// frame classification lives in the pure ./snapshotMapping module so it can be
// fixture-tested without a socket; this file only owns I/O and callback routing.
import type { AppSnapshot } from '../types';
import { apiFetch, isMockApiEnabled, loadMockApi, webSocketUrl } from './client';
import { classifySnapshotFrame, mapSnapshotDto, type AgentSnapshotDto } from './snapshotMapping';

export type AppSnapshotSocketCallbacks = {
  onSnapshot?: (snapshot: AppSnapshot) => void;
  onInvalidate: () => void;
  onUnavailable?: () => void;
  onClosed?: () => void;
};

export async function getAppSnapshot(): Promise<AppSnapshot> {
  if (import.meta.env.DEV && isMockApiEnabled()) return (await loadMockApi()).getMockAppSnapshot();
  return mapSnapshotDto(await apiFetch<AgentSnapshotDto | AppSnapshot>('/snapshot'));
}

export function connectAppSnapshotSocket(callbacks: AppSnapshotSocketCallbacks): () => void {
  if (import.meta.env.DEV && isMockApiEnabled()) {
    let cleanup: (() => void) | undefined;
    let closed = false;
    void loadMockApi()
      .then((mockApi) => {
        if (closed) return;
        cleanup = mockApi.connectMockAppSnapshotSocket(callbacks);
      })
      .catch(() => {
        if (!closed) callbacks.onUnavailable?.();
      });
    return () => {
      closed = true;
      cleanup?.();
    };
  }
  if (typeof window === 'undefined' || typeof WebSocket === 'undefined') {
    callbacks.onUnavailable?.();
    return () => {};
  }

  let socket: WebSocket;
  let closedByClient = false;
  try {
    socket = new WebSocket(webSocketUrl('/ws'));
  } catch {
    callbacks.onUnavailable?.();
    return () => {};
  }

  socket.addEventListener('message', (event) => {
    const frame = classifySnapshotFrame(event.data);
    if (frame.kind === 'snapshot') {
      callbacks.onSnapshot?.(frame.snapshot);
    } else if (frame.kind === 'invalidate') {
      callbacks.onInvalidate();
    }
    // 'ignore' (ping/pong) and 'malformed' frames are intentionally dropped,
    // preserving the transport's prior behavior.
  });
  socket.addEventListener('error', () => {
    if (!closedByClient) callbacks.onUnavailable?.();
  });
  socket.addEventListener('close', () => {
    if (!closedByClient) callbacks.onClosed?.();
  });

  return () => {
    closedByClient = true;
    socket.close();
  };
}
