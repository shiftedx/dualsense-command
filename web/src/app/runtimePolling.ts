import { clampUnit } from '../lib/features/haptics/hapticsModel';
import type { ControllerInputState } from '../lib/types';

export type TriggerInputPollerState = {
  fresh: boolean;
  l2: number;
  r2: number;
};

type TriggerInputPollerOptions = {
  intervalMs: number;
  getControllerId: () => string | null | undefined;
  shouldPoll: () => boolean;
  getControllerInput: (controllerId: string) => Promise<ControllerInputState>;
  onState: (state: TriggerInputPollerState) => void;
};

type AsyncTaskOptions = {
  delayMs: number;
  run: () => void | Promise<void>;
};

type QueuedThrottleTaskOptions = {
  minIntervalMs: number;
  shouldRun: () => boolean;
  run: () => void | Promise<void>;
};

const emptyTriggerInputState = (): TriggerInputPollerState => ({
  fresh: false,
  l2: 0,
  r2: 0
});

const sameTriggerInputState = (left: TriggerInputPollerState, right: TriggerInputPollerState) =>
  left.fresh === right.fresh && left.l2 === right.l2 && left.r2 === right.r2;

const now = () =>
  typeof performance !== 'undefined' && typeof performance.now === 'function'
    ? performance.now()
    : Date.now();

export function createTriggerInputPoller(options: TriggerInputPollerOptions) {
  let pollTimer: number | undefined;
  let frame: number | undefined;
  let pendingState: TriggerInputPollerState | undefined;
  let busy = false;
  let state = emptyTriggerInputState();

  const clearFrame = () => {
    if (frame !== undefined && typeof window !== 'undefined') {
      window.cancelAnimationFrame(frame);
    }
    frame = undefined;
  };

  const commit = (next: TriggerInputPollerState) => {
    if (sameTriggerInputState(state, next)) return;
    state = next;
    options.onState(state);
  };

  const flushPendingState = () => {
    frame = undefined;
    const next = pendingState;
    pendingState = undefined;
    if (next) commit(next);
  };

  const publish = (next: TriggerInputPollerState) => {
    pendingState = next;
    if (typeof window === 'undefined' || typeof window.requestAnimationFrame !== 'function') {
      flushPendingState();
      return;
    }
    if (frame === undefined) {
      frame = window.requestAnimationFrame(flushPendingState);
    }
  };

  const reset = () => {
    pendingState = undefined;
    clearFrame();
    commit(emptyTriggerInputState());
  };

  const stop = () => {
    if (pollTimer !== undefined && typeof window !== 'undefined') {
      window.clearInterval(pollTimer);
    }
    pollTimer = undefined;
    busy = false;
    reset();
  };

  const poll = async () => {
    if (busy || !options.shouldPoll()) return;
    const requestedControllerId = options.getControllerId();
    if (!requestedControllerId) return;

    busy = true;
    try {
      const input = await options.getControllerInput(requestedControllerId);
      if (
        !options.shouldPoll() ||
        input.controllerId !== requestedControllerId ||
        options.getControllerId() !== requestedControllerId
      ) {
        return;
      }
      if (input.available) {
        publish({
          fresh: true,
          l2: clampUnit(input.triggers.l2),
          r2: clampUnit(input.triggers.r2)
        });
      } else {
        publish({ ...state, fresh: false });
      }
    } catch {
      if (!options.shouldPoll()) return;
      publish({ ...state, fresh: false });
    } finally {
      busy = false;
    }
  };

  const start = () => {
    if (!options.shouldPoll() || typeof window === 'undefined') return;
    if (pollTimer !== undefined) return;
    void poll();
    pollTimer = window.setInterval(() => void poll(), options.intervalMs);
  };

  const sync = () => {
    if (options.shouldPoll()) start();
    else stop();
  };

  return {
    poll,
    start,
    stop,
    sync
  };
}

export function createOneShotTimer(durationMs: number, callback: () => void) {
  let timer: number | undefined;

  const clear = () => {
    if (timer !== undefined && typeof window !== 'undefined') {
      window.clearTimeout(timer);
    }
    timer = undefined;
  };

  const arm = () => {
    if (typeof window === 'undefined') return;
    clear();
    timer = window.setTimeout(() => {
      timer = undefined;
      callback();
    }, durationMs);
  };

  return {
    arm,
    clear
  };
}

export function createDebouncedAsyncTask(options: AsyncTaskOptions) {
  let timer: number | undefined;
  let inFlight = false;
  let queued = false;

  const clearTimer = () => {
    if (timer !== undefined && typeof window !== 'undefined') {
      window.clearTimeout(timer);
    }
    timer = undefined;
  };

  const flush = async () => {
    clearTimer();
    if (inFlight) {
      queued = true;
      return;
    }

    inFlight = true;
    queued = false;
    try {
      await options.run();
    } finally {
      inFlight = false;
      if (queued) schedule();
    }
  };

  function schedule() {
    queued = true;
    if (inFlight) return;
    clearTimer();
    if (typeof window === 'undefined') {
      void flush();
      return;
    }
    timer = window.setTimeout(() => {
      void flush();
    }, options.delayMs);
  }

  const clear = () => {
    clearTimer();
    queued = false;
  };

  return {
    schedule,
    flush,
    clear
  };
}

export function createQueuedThrottleTask(options: QueuedThrottleTaskOptions) {
  let timer: number | undefined;
  let inFlight = false;
  let queued = false;
  let lastRunAt = 0;

  const clearTimer = () => {
    if (timer !== undefined && typeof window !== 'undefined') {
      window.clearTimeout(timer);
    }
    timer = undefined;
  };

  const flush = async () => {
    clearTimer();
    if (!options.shouldRun() || inFlight) return;

    queued = false;
    inFlight = true;
    lastRunAt = now();
    try {
      await options.run();
    } finally {
      inFlight = false;
      if (queued && options.shouldRun()) schedule();
    }
  };

  function schedule() {
    if (!options.shouldRun()) return;
    queued = true;
    if (inFlight || timer !== undefined) return;

    const elapsed = now() - lastRunAt;
    const waitMs = Math.max(0, options.minIntervalMs - elapsed);
    if (typeof window === 'undefined') {
      void flush();
      return;
    }
    timer = window.setTimeout(() => {
      void flush();
    }, waitMs);
  }

  const clear = () => {
    clearTimer();
    queued = false;
  };

  return {
    schedule,
    flush,
    clear
  };
}
