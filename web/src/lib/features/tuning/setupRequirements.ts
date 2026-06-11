import type { AdapterStatus, SupportedGame } from '../../types';

// Pure setup-walkthrough model for the Tuning canvas (Task 8). Each Game
// Module's metadata plus live telemetry state derive `{required, steps,
// verified}` — no fetching, no storage, no component state. Persistence of
// "verified once" lives in app/setupVerification.ts.

export const TELEMETRY_TARGET_IP = '127.0.0.1';
export const DEFAULT_TELEMETRY_PORT = 5300;

export type SetupStepState = 'done' | 'now' | 'todo';

export type SetupCopyValue = {
  label: string;
  value: string;
};

export type SetupStep = {
  id: 'found' | 'data-out' | 'drive';
  state: SetupStepState;
  title: string;
  detail: string;
  /** Exact in-game menu path, rendered emphasized before `detailAfterPath`. */
  menuPath?: string;
  detailAfterPath?: string;
  copyValues?: SetupCopyValue[];
};

export type SetupModel = {
  /** False for games DSCC reads without any in-game configuration. */
  required: boolean;
  /** Telemetry packets have been seen for this game at least once. */
  verified: boolean;
  /** Live freshness: packets are arriving right now. */
  fresh: boolean;
  port: number;
  steps: SetupStep[];
};

/**
 * The UDP port the active Telemetry Adapter listens on. The agent reports its
 * bound address inside the adapter's config string (for example
 * `127.0.0.1:5300`); fall back to the well-known Forza Data Out default when
 * no adapter or no port is visible (the mock fixture, early startup).
 */
export function telemetryPortFromAdapter(adapter: AdapterStatus | null | undefined): number {
  for (const text of [adapter?.config, adapter?.setupHint]) {
    const match = text?.match(/:(\d{2,5})(?!\d)/);
    if (match) {
      const port = Number(match[1]);
      if (Number.isInteger(port) && port > 0 && port <= 65535) return port;
    }
  }
  return DEFAULT_TELEMETRY_PORT;
}

const foundDetail = (game: SupportedGame): string => {
  if (game.running) return 'Running now.';
  if (game.source === 'local_app') return 'Added as a local app.';
  if (game.installed) return 'Installed via Steam.';
  return 'DSCC spots the game on its own once it starts.';
};

export function deriveSetupModel(options: {
  game: SupportedGame;
  /** Fresh Telemetry attributed to this game (it is the detected game). */
  telemetryFresh: boolean;
  /** Packets were seen for this game at least once (persisted). */
  verified: boolean;
  port?: number;
}): SetupModel {
  const { game, telemetryFresh, verified } = options;
  const port = options.port ?? DEFAULT_TELEMETRY_PORT;

  if (game.supportLevel !== 'telemetry') {
    return { required: false, verified, fresh: telemetryFresh, port, steps: [] };
  }

  const found = game.installed || game.running;
  const steps: SetupStep[] = [
    {
      id: 'found',
      state: found ? 'done' : 'now',
      title: `${game.name} found`,
      detail: foundDetail(game)
    },
    {
      id: 'data-out',
      state: telemetryFresh ? 'done' : found ? 'now' : 'todo',
      title: "Turn on the game's telemetry feed",
      detail: `In ${game.name}: `,
      menuPath: 'Settings → HUD and Gameplay → Data Out',
      detailAfterPath: '. Some versions call it UDP Race Telemetry. Set:',
      copyValues: [
        { label: 'IP address', value: TELEMETRY_TARGET_IP },
        { label: 'Port', value: String(port) }
      ]
    },
    {
      id: 'drive',
      state: telemetryFresh ? 'done' : 'todo',
      title: 'Drive',
      detail: 'Get in a car. The moment data arrives, this page becomes the tuning canvas.'
    }
  ];

  return { required: true, verified, fresh: telemetryFresh, port, steps };
}
