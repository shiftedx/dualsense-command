import type {
  ControllerConfiguration,
  ExportedProfile,
  ProfileAssignmentConfiguration,
  ProfileSummary,
  SupportedGame
} from '../../types';

export type ProfileImportPayload = {
  schema: string;
  id?: string;
  name: string;
  config?: ExportedProfile['config'];
};

export function builtInProfileIdForGame(
  game: SupportedGame | null | undefined,
  profiles: ProfileSummary[]
): string | null {
  const gameId = game?.gameId.toLowerCase() ?? '';
  if (gameId === 'assetto-corsa-rally') return 'assetto-corsa-rally';
  if (gameId.startsWith('forza-horizon')) {
    return (
      profiles.find((profile) => profile.id === 'forza-horizon-immersive')?.id ??
      profiles.find((profile) => profile.id === 'forza-horizon')?.id ??
      null
    );
  }
  return null;
}

export function usesForzaRuntimeProfile(game: SupportedGame | null | undefined): boolean {
  const gameId = game?.gameId.toLowerCase() ?? '';
  return gameId.startsWith('forza') || gameId === 'assetto-corsa-rally';
}

export function profileAssignmentMatchesGame(
  assignment: ProfileAssignmentConfiguration,
  game: SupportedGame
): boolean {
  const assignmentGameId = assignment.gameId.trim().toLowerCase();
  const gameId = game.gameId.trim().toLowerCase();
  return assignmentGameId === gameId;
}

export function assignmentForGame(
  game: SupportedGame | null | undefined,
  currentControllerConfig: ControllerConfiguration | null
): ProfileAssignmentConfiguration | undefined {
  if (!game) return undefined;
  return currentControllerConfig?.profileAssignments.find((assignment) =>
    profileAssignmentMatchesGame(assignment, game)
  );
}

export function defaultProfileIdForGame(
  game: SupportedGame | null | undefined,
  profiles: ProfileSummary[],
  activeProfileId: string,
  currentControllerConfig: ControllerConfiguration | null
): string {
  if (!game) {
    return (
      profiles.find((profile) => profile.id === 'global')?.id ??
      profiles.find((profile) => profile.scope === 'Global')?.id ??
      profiles.find((profile) => profile.id === activeProfileId && profile.scope !== 'Game')?.id ??
      profiles[0]?.id ??
      ''
    );
  }
  const scopedProfile = profiles.find((profile) => profile.scope === 'Game' && profile.gameId === game.gameId);
  if (scopedProfile) return scopedProfile.id;
  const assignment = assignmentForGame(game, currentControllerConfig);
  if (assignment?.profileId && profiles.some((profile) => profile.id === assignment.profileId)) {
    return assignment.profileId;
  }
  const builtInProfileId = builtInProfileIdForGame(game, profiles);
  if (builtInProfileId) return builtInProfileId;
  return activeProfileId || profiles[0]?.id || '';
}

export function profilesForGame(
  source: ProfileSummary[],
  game: SupportedGame | null | undefined,
  defaultProfileId: string,
  selectedProfileId: string,
  activeId: string
): ProfileSummary[] {
  return source
    .filter((profile) => {
      if (profile.scope !== 'Game') return true;
      if (game && profile.gameId === game.gameId) return true;
      return profile.id === selectedProfileId || profile.id === activeId;
    })
    .map((profile, index) => ({ profile, index }))
    .sort((left, right) => {
      const rank = (profile: ProfileSummary) => {
        if (profile.id === selectedProfileId) return 0;
        if (game && profile.scope === 'Game' && profile.gameId === game.gameId) return 1;
        if (game && profile.id === defaultProfileId) return 2;
        if (profile.scope === 'Global' && !game) return 1;
        if (profile.id === activeId) return 3;
        if (profile.scope === 'Built-in') return 4;
        return 5;
      };
      return rank(left.profile) - rank(right.profile) || left.index - right.index;
    })
    .map(({ profile }) => profile);
}

export function profileContextTag(
  profile: ProfileSummary,
  profileContextGame: SupportedGame | null,
  profileContextDefaultProfileId: string,
  activeProfileId: string
): string {
  if (profile.scope === 'Game') return 'game';
  if (profileContextGame && profile.id === profileContextDefaultProfileId) return 'recommended';
  if (profile.id === activeProfileId) return 'active';
  return profile.builtIn ? (profile.scope === 'Global' ? 'stock global' : 'built-in') : profile.scope.toLowerCase();
}

export function sanitizeFileName(value: string): string {
  return (
    value
      .trim()
      .replace(/[^a-z0-9._-]+/gi, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 80) || 'profile'
  );
}

export function profileSlug(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

export function uniqueProfileName(baseName: string, profiles: ProfileSummary[]): string {
  const existingNames = new Set(profiles.map((profile) => profile.name.toLowerCase()));
  let candidate = baseName.trim() || 'Imported profile';
  if (!existingNames.has(candidate.toLowerCase()) && !profiles.some((profile) => profile.id === profileSlug(candidate))) {
    return candidate;
  }
  const root = candidate.replace(/\s+copy(?:\s+\d+)?$/i, '').trim() || 'Imported profile';
  for (let index = 2; index < 1000; index += 1) {
    candidate = `${root} copy ${index}`;
    if (!existingNames.has(candidate.toLowerCase()) && !profiles.some((profile) => profile.id === profileSlug(candidate))) {
      return candidate;
    }
  }
  return `${root} copy ${Date.now()}`;
}

export function profileImportPayload(value: unknown, profiles: ProfileSummary[]): ProfileImportPayload {
  if (!value || typeof value !== 'object') throw new Error('Profile file is not valid JSON.');
  const profile = value as Partial<ExportedProfile>;
  if (profile.schema !== 'dev.dscc.profile.v1') throw new Error('Unsupported DSCC profile schema.');
  const name = typeof profile.name === 'string' ? profile.name.trim() : '';
  if (!name) throw new Error('Profile file is missing a profile name.');

  const id = typeof profile.id === 'string' ? profile.id.trim() : '';
  const existingIds = new Set(profiles.map((item) => item.id));
  const idAvailable = Boolean(id) && !existingIds.has(id);
  return {
    id: idAvailable ? id : undefined,
    schema: profile.schema,
    name: idAvailable ? name : uniqueProfileName(`${name} copy`, profiles),
    config: profile.config ?? undefined
  };
}
