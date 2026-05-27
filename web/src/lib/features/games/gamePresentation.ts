import type { GameDetection, ProfileSummary, SupportedGame } from '../../types';

export function gameArtwork(
  game: SupportedGame | null | undefined,
  kind: 'icon' | 'banner' | 'hero' | 'capsule'
): string | null {
  if (!game?.artwork) return null;
  if (kind === 'icon') return game.artwork.iconUrl ?? game.artwork.capsuleUrl ?? game.artwork.bannerUrl ?? null;
  if (kind === 'banner') return game.artwork.bannerUrl ?? game.artwork.heroUrl ?? game.artwork.capsuleUrl ?? null;
  if (kind === 'hero') return game.artwork.heroUrl ?? game.artwork.bannerUrl ?? game.artwork.capsuleUrl ?? null;
  return game.artwork.capsuleUrl ?? game.artwork.bannerUrl ?? game.artwork.heroUrl ?? null;
}

export function gameProviderMeta(game: SupportedGame | null | undefined): string {
  if (!game?.appId) return '';
  if (game.source === 'local_app' || game.inputProvider === 'dscc_input_bridge') return 'Local app';
  return `Steam ${game.appId}`;
}

export function gameLauncherLabel(game: SupportedGame): string {
  return [
    game.name,
    gameProviderMeta(game),
    game.running ? 'running' : game.installed ? 'installed' : 'not installed'
  ]
    .filter(Boolean)
    .join(' / ');
}

export function formatPlaytime(minutes: number | null | undefined): string {
  if (minutes === null || minutes === undefined || !Number.isFinite(minutes) || minutes <= 0) return '';
  if (minutes < 60) return `${Math.round(minutes)}m played`;
  const hours = minutes / 60;
  return `${hours < 100 ? hours.toFixed(1) : Math.round(hours)}h played`;
}

export function formatLastPlayed(unixSeconds: number | null | undefined): string {
  if (!unixSeconds || !Number.isFinite(unixSeconds)) return '';
  const then = unixSeconds * 1000;
  const days = Math.max(0, Math.floor((Date.now() - then) / 86_400_000));
  if (days === 0) return 'played today';
  if (days === 1) return 'played yesterday';
  if (days < 14) return `played ${days}d ago`;
  return `played ${new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' }).format(new Date(then))}`;
}

export function achievementText(game: SupportedGame): string {
  const achievements = game.stats?.achievements;
  if (!achievements || achievements.total <= 0) return '';
  return `${achievements.unlocked}/${achievements.total} achievements`;
}

export function gameTileStatus(game: SupportedGame): string {
  if (game.running) return 'running';
  if (game.installed) return 'installed';
  return 'not installed';
}

export function gameDetectionStatusText(detection: GameDetection | undefined): string {
  if (!detection?.activeGameId && !detection?.activeGameName) return '';

  const source = detection.source.split(':', 1)[0];
  switch (source) {
    case 'process_scan':
      return 'Running on this PC';
    case 'process_scan_disabled':
      return 'Game detection paused';
    case 'process_scan_unavailable':
      return 'Game detection unavailable';
    case 'built_in':
      return 'Built-in game support';
    case 'none':
    case 'unknown':
    case '':
      return 'Detected';
    default:
      return source.replaceAll('_', ' ');
  }
}

export function gameMediaDetails(game: SupportedGame): string[] {
  return [
    gameProviderMeta(game),
    formatPlaytime(game.stats?.playtimeMinutes),
    achievementText(game),
    formatLastPlayed(game.stats?.lastPlayedUnix)
  ].filter(Boolean);
}

export function profileScopeCount(game: SupportedGame, profiles: ProfileSummary[]): number {
  return profiles.filter((profile) => profile.scope === 'Game' && profile.gameId === game.gameId).length;
}

const SCOPE_ACCENT_BUILT_IN = '#3BA0FF';
const SCOPE_ACCENT_CUSTOM = '#E0B341';

export function gameAccentColor(game: SupportedGame | null | undefined): string {
  return game?.supportLevel === 'custom' ? SCOPE_ACCENT_CUSTOM : SCOPE_ACCENT_BUILT_IN;
}
