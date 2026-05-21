import type { AppSnapshot } from './types';

type SnapshotSocketCallbacks = {
  onSnapshot?: (snapshot: AppSnapshot) => void;
  onInvalidate: () => void;
  onUnavailable?: () => void;
  onClosed?: () => void;
};

type AppRuntimeOptions = {
  fallbackPollIntervalMs: number;
  snapshotInvalidationDebounceMs: number;
  refresh: () => void | Promise<void>;
  applySnapshot: (snapshot: AppSnapshot) => void;
  connectSnapshotSocket: (callbacks: SnapshotSocketCallbacks) => () => void;
  onStart?: () => void;
  onStop?: () => void;
  onVisible?: () => void;
  onHidden?: () => void;
  onHashChange?: () => void;
  onDocumentMouseDown?: (event: MouseEvent) => void;
  onDocumentKeyDown?: (event: KeyboardEvent) => void;
};

export type AppRuntime = {
  start: () => void;
  stop: () => void;
  scheduleRefresh: () => void;
  startFallbackPolling: () => void;
  isStarted: () => boolean;
};

export function createAppRuntime(options: AppRuntimeOptions): AppRuntime {
  let started = false;
  let refreshDebounceTimer: number | undefined;
  let fallbackPollTimer: number | undefined;
  let stopSnapshotSocket: (() => void) | undefined;
  let pendingVisibilityRefresh = false;

  const isHidden = () => typeof document !== 'undefined' && document.hidden;
  const runRefresh = () => {
    void options.refresh();
  };

  const scheduleRefresh = () => {
    if (isHidden()) {
      pendingVisibilityRefresh = true;
      return;
    }
    if (refreshDebounceTimer !== undefined) return;
    if (typeof window.setTimeout !== 'function') {
      runRefresh();
      return;
    }
    refreshDebounceTimer = window.setTimeout(() => {
      refreshDebounceTimer = undefined;
      runRefresh();
    }, options.snapshotInvalidationDebounceMs);
  };

  const startFallbackPolling = () => {
    if (typeof window === 'undefined' || fallbackPollTimer !== undefined) return;
    if (!isHidden()) runRefresh();
    if (typeof window.setInterval !== 'function') return;
    fallbackPollTimer = window.setInterval(() => {
      if (!isHidden()) runRefresh();
    }, options.fallbackPollIntervalMs);
  };

  const handleVisibilityChange = () => {
    if (isHidden()) {
      options.onHidden?.();
      return;
    }

    options.onVisible?.();
    if (!pendingVisibilityRefresh) return;
    pendingVisibilityRefresh = false;
    runRefresh();
  };

  const handleHashChange = () => {
    options.onHashChange?.();
  };

  const start = () => {
    if (typeof window === 'undefined' || started) return;
    started = true;
    options.onStart?.();
    runRefresh();
    stopSnapshotSocket = options.connectSnapshotSocket({
      onSnapshot: options.applySnapshot,
      onInvalidate: scheduleRefresh,
      onUnavailable: startFallbackPolling,
      onClosed: startFallbackPolling
    });
    document.addEventListener('visibilitychange', handleVisibilityChange);
    window.addEventListener('hashchange', handleHashChange);
    if (options.onDocumentMouseDown) document.addEventListener('mousedown', options.onDocumentMouseDown);
    if (options.onDocumentKeyDown) document.addEventListener('keydown', options.onDocumentKeyDown);
  };

  const stop = () => {
    if (typeof window === 'undefined' || !started) return;
    started = false;
    stopSnapshotSocket?.();
    stopSnapshotSocket = undefined;
    if (fallbackPollTimer !== undefined) window.clearInterval(fallbackPollTimer);
    fallbackPollTimer = undefined;
    if (refreshDebounceTimer !== undefined) window.clearTimeout(refreshDebounceTimer);
    refreshDebounceTimer = undefined;
    pendingVisibilityRefresh = false;
    document.removeEventListener('visibilitychange', handleVisibilityChange);
    window.removeEventListener('hashchange', handleHashChange);
    if (options.onDocumentMouseDown) document.removeEventListener('mousedown', options.onDocumentMouseDown);
    if (options.onDocumentKeyDown) document.removeEventListener('keydown', options.onDocumentKeyDown);
    options.onStop?.();
  };

  return {
    start,
    stop,
    scheduleRefresh,
    startFallbackPolling,
    isStarted: () => started
  };
}
