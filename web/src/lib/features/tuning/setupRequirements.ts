import type { AdapterStatus, ModuleSummary, SupportedGame } from '../../types';

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
  // Try address shape first: ipv4:port (e.g., 127.0.0.1:5300)
  const config = adapter?.config;
  if (config) {
    const addressMatch = config.match(/(?:\d{1,3}\.){3}\d{1,3}:(\d{1,5})/);
    if (addressMatch) {
      const port = Number(addressMatch[1]);
      if (Number.isInteger(port) && port > 0 && port <= 65535) return port;
    }
  }
  // Fall back to hint field if config didn't match
  const hint = adapter?.setupHint;
  if (hint) {
    const addressMatch = hint.match(/(?:\d{1,3}\.){3}\d{1,3}:(\d{1,5})/);
    if (addressMatch) {
      const port = Number(addressMatch[1]);
      if (Number.isInteger(port) && port > 0 && port <= 65535) return port;
    }
  }
  return DEFAULT_TELEMETRY_PORT;
}

export type GameTelemetryAdapterLink = {
  adapterId: string | null;
  protocol: string | null;
};

const GAME_MODULE_PROTOCOL_PREFIX = 'game:';

/**
 * Resolve a Supported Game's Telemetry Adapter from the module catalog. Game
 * modules report `protocol: "game:<adapterId>"`; the adapter module with that
 * id reports the transport protocol (`udp`, `sharedmemory`, ...). Returns
 * nulls when the catalog does not know the game (the mock fixture, custom
 * games).
 */
export function gameTelemetryAdapter(modules: ModuleSummary[], gameId: string): GameTelemetryAdapterLink {
  const gameModule = gameId
    ? modules.find((module) => module.kind === 'game' && module.id === gameId)
    : undefined;
  const linkedId = gameModule?.protocol.startsWith(GAME_MODULE_PROTOCOL_PREFIX)
    ? gameModule.protocol.slice(GAME_MODULE_PROTOCOL_PREFIX.length)
    : '';
  const adapterId = linkedId || null;
  const adapterModule = adapterId
    ? modules.find((module) => module.kind === 'adapter' && module.id === adapterId)
    : undefined;
  return { adapterId, protocol: adapterModule?.protocol ?? null };
}

/**
 * Shared-memory Telemetry Adapters need no in-game configuration: DSCC reads
 * the public shared-memory pages on its own once the game is driving.
 */
export function isSharedMemoryTelemetry(adapter: GameTelemetryAdapterLink | null | undefined): boolean {
  if (!adapter) return false;
  const protocol = (adapter.protocol ?? '').toLowerCase().replace(/[-_\s]/g, '');
  if (protocol === 'sharedmemory') return true;
  return /shared[-_]?memory/.test((adapter.adapterId ?? '').toLowerCase());
}

/**
 * Exact Data Out menu locations by Game Module id. Forza Motorsport files the
 * feed under a differently-ordered menu than the Horizon titles (per the
 * public Forza Data Out documentation).
 */
const DATA_OUT_MENU_PATHS: Record<string, string> = {
  'forza-motorsport': 'Settings → Gameplay & HUD → Data Out'
};
const DEFAULT_DATA_OUT_MENU_PATH = 'Settings → HUD and Gameplay → Data Out';

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
  /** The game's linked Telemetry Adapter, when the module catalog knows it. */
  telemetryAdapter?: GameTelemetryAdapterLink | null;
}): SetupModel {
  const { game, telemetryFresh, verified } = options;
  const port = options.port ?? DEFAULT_TELEMETRY_PORT;

  // Shared-memory games (Assetto Corsa Rally) need no in-game setup: no port,
  // no Data Out menu. They get the same zero-setup variant as custom games.
  if (game.supportLevel !== 'telemetry' || isSharedMemoryTelemetry(options.telemetryAdapter)) {
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
      menuPath: DATA_OUT_MENU_PATHS[game.gameId] ?? DEFAULT_DATA_OUT_MENU_PATH,
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
