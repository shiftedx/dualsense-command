import type { SupportedGame } from '../../types';

// Pure presentation model for the Tuning header's game dropdown. Builds the
// grouped menu (Running now / Everyday / Supported games / footer actions)
// from the discovered games list plus the current selection. No fetching, no
// component state: types in, types out.

export type TuningScopeKind = 'none' | 'global' | 'game';

export type GameSelectEntry =
  | { kind: 'game'; id: string; game: SupportedGame; label: string; running: boolean; current: boolean }
  | { kind: 'everyday'; id: 'everyday'; label: string; detail: string; current: boolean }
  | { kind: 'setup-guide'; id: 'setup-guide'; label: string; enabled: boolean }
  | { kind: 'add-game'; id: 'add-game'; label: string };

export type GameSelectGroup = {
  id: 'running' | 'everyday' | 'supported' | 'actions';
  label: string | null;
  entries: GameSelectEntry[];
};

export type GameSelectModel = {
  groups: GameSelectGroup[];
  /** Header title for the closed state of the dropdown. */
  title: string;
};

const gameEntry = (game: SupportedGame, options: BuildGameSelectOptions): GameSelectEntry => ({
  kind: 'game',
  id: game.gameId,
  game,
  label: game.name,
  running: Boolean(game.running),
  current: options.scope === 'game' && options.selectedGameId === game.gameId
});

export type BuildGameSelectOptions = {
  /** Discovered games, already sorted running > installed > name. */
  games: SupportedGame[];
  scope: TuningScopeKind;
  selectedGameId: string;
  /**
   * Game whose setup guide the footer should offer (usually the selected
   * game when it has telemetry support). Null hides the entry.
   */
  setupGuideGame?: SupportedGame | null;
  /** Whether the setup-guide entry is actionable yet. */
  setupGuideEnabled?: boolean;
};

export function buildGameSelectModel(options: BuildGameSelectOptions): GameSelectModel {
  const games = options.games ?? [];
  const running = games.filter((game) => game.running);
  const rest = games.filter((game) => !game.running);

  const groups: GameSelectGroup[] = [];

  if (running.length) {
    groups.push({
      id: 'running',
      label: 'Running now',
      entries: running.map((game) => gameEntry(game, options))
    });
  }

  groups.push({
    id: 'everyday',
    label: 'Everyday',
    entries: [
      {
        kind: 'everyday',
        id: 'everyday',
        label: 'Everyday (no game)',
        detail: 'Global Profile',
        current: options.scope !== 'game'
      }
    ]
  });

  if (rest.length) {
    groups.push({
      id: 'supported',
      label: 'Supported games',
      entries: rest.map((game) => gameEntry(game, options))
    });
  }

  const actions: GameSelectEntry[] = [];
  if (options.setupGuideGame) {
    actions.push({
      kind: 'setup-guide',
      id: 'setup-guide',
      label: `Setup guide for ${options.setupGuideGame.name}…`,
      enabled: Boolean(options.setupGuideEnabled)
    });
  }
  actions.push({ kind: 'add-game', id: 'add-game', label: '+ Add a game manually…' });
  groups.push({ id: 'actions', label: null, entries: actions });

  return {
    groups,
    title: headerTitle(options)
  };
}

export function headerTitle(options: {
  games: SupportedGame[];
  scope: TuningScopeKind;
  selectedGameId: string;
}): string {
  if (options.scope === 'game' && options.selectedGameId) {
    const game = (options.games ?? []).find((item) => item.gameId === options.selectedGameId);
    if (game) return game.name;
  }
  /* "Everyday" is a presentation label and must always pair with its
     domain term, Global Profile (CONTEXT.md / spec §13). */
  return 'Everyday · Global Profile';
}

export type TelemetryChipState = 'fresh' | 'quiet' | 'setup' | 'none' | null;

/**
 * Telemetry chip state for the header band. The chip is the door back into
 * the setup guide:
 * - `fresh` (green): packets are arriving for the running game.
 * - `quiet` (yellow): the game is up but its data feed is silent.
 * - `setup` (accent): a telemetry game that has never verified — one-time
 *   setup is in progress.
 * - `none` (neutral): the selected game needs no telemetry feed at all.
 */
export function telemetryChipState(options: {
  scope: TuningScopeKind;
  selectedGame: SupportedGame | null;
  adapterRunning: boolean;
  packetRateHz: number;
  /** Telemetry packets have been seen for this game at least once. */
  setupVerified: boolean;
}): TelemetryChipState {
  if (options.scope !== 'game') return null;
  const game = options.selectedGame;
  if (!game) return null;
  if (game.supportLevel !== 'telemetry') return 'none';
  if (!options.setupVerified) return 'setup';
  if (!game.running) return null;
  return options.adapterRunning && options.packetRateHz > 0 ? 'fresh' : 'quiet';
}
