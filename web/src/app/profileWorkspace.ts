import {
  achievementText,
  formatLastPlayed,
  formatPlaytime,
  gameArtwork,
  gameDetectionStatusText,
  gameProviderMeta,
  gameTileStatus
} from '../lib/features/games/gamePresentation';
import {
  assignmentForGame,
  defaultProfileIdForGame,
  profileContextTag,
  profilesForGame
} from '../lib/features/profiles/profileSelection';
import {
  clearProfileOverride,
  saveControllerConfig,
  setProfileOverride
} from '../lib/api';
import type {
  AppSnapshot,
  ControllerConfiguration,
  ControllerStatus,
  ProfileResolution,
  ProfileSummary,
  SupportedGame
} from '../lib/types';
import {
  profileTargetsCoverAllConnectedControllers,
  reconcileProfileTargetControllerIds,
  resolveSelectedControllerId,
  resolveSelectedProfileTargetIds,
  selectCurrentController,
  summarizeProfileTargets
} from './controllerSelection';
import type { EditableControllerConfig } from './profileDraft';

export type TuningScope = 'none' | 'global' | 'game';

export type TargetControllerWorkspace = {
  controllers: ControllerStatus[];
  connectedControllers: ControllerStatus[];
  connectedControllerIds: string[];
  selectedControllerId: string;
  controller: ControllerStatus | undefined;
  profileTargetControllerIds: string[];
  profileTargetsAllConnected: boolean;
};

export type ProfileWorkspace = {
  status: AppSnapshot['status'] | undefined;
  profiles: ProfileSummary[];
  activeProfileId: string;
  globalProfilePreview: ProfileSummary | undefined;
  activeProfileName: string;
  activeProfile: ProfileSummary | undefined;
  selectedOverrideProfile: ProfileSummary | undefined;
  selectedActionProfile: ProfileSummary | null;
  canDeleteSelectedProfile: boolean;
  canRenameSelectedProfile: boolean;
  overrideActive: boolean;
  detectedGameLabel: string;
  supportedGames: SupportedGame[];
  selectedGame: SupportedGame | null;
  discoveredGames: SupportedGame[];
  selectedTuningGame: SupportedGame | null;
  tuningReady: boolean;
  buttonMappingReady: boolean;
  profileContextGame: SupportedGame | null;
  profileContextGameId: string | null;
  profileContextLabel: string;
  profileContextDefaultProfileId: string;
  profileContextDefaultProfile: ProfileSummary | undefined;
  profileContextProfiles: ProfileSummary[];
  activeProfileContextLabel: string;
  profileContextDetail: string;
  detectionSignalText: string;
  steamContextGame: SupportedGame | null;
  steamContextArt: string;
  steamContextBackdropArt: string;
  steamContextMeta: string;
  activeProfileHeader: ProfileSummary | null;
  activeProfileHeaderName: string;
  activeProfileHeaderMeta: string;
  overrideScope: string;
};

export function deriveTargetControllerWorkspace(options: {
  controllers: ControllerStatus[] | undefined;
  selectedControllerId: string;
  profileTargetControllerIds: string[];
}): TargetControllerWorkspace {
  const controllers = options.controllers ?? [];
  const connectedControllers = controllers.filter((item) => item.connected);
  const connectedControllerIds = connectedControllers.map((item) => item.id);
  const selectedControllerId = resolveSelectedControllerId(controllers, options.selectedControllerId);
  const controller = selectCurrentController(controllers, selectedControllerId);
  const profileTargetControllerIds = reconcileProfileTargetControllerIds(
    options.profileTargetControllerIds,
    connectedControllerIds,
    selectedControllerId
  );

  return {
    controllers,
    connectedControllers,
    connectedControllerIds,
    selectedControllerId,
    controller,
    profileTargetControllerIds,
    profileTargetsAllConnected: profileTargetsCoverAllConnectedControllers(
      profileTargetControllerIds,
      connectedControllerIds
    )
  };
}

export function reconcileTargetControllerWorkspaceSelection(options: {
  controllers: ControllerStatus[] | undefined;
  selectedControllerId: string;
  profileTargetControllerIds: string[];
}): { selectedControllerId: string; profileTargetControllerIds: string[] } {
  const workspace = deriveTargetControllerWorkspace(options);
  return {
    selectedControllerId: workspace.selectedControllerId,
    profileTargetControllerIds: workspace.profileTargetControllerIds
  };
}

export function reconcileTuningSelection(options: {
  selectedTuningScope: TuningScope;
  selectedTuningGameId: string;
  supportedGames: SupportedGame[];
}): { selectedTuningScope: TuningScope; selectedTuningGameId: string } {
  if (
    options.selectedTuningGameId &&
    options.supportedGames.length &&
    !options.supportedGames.some((game) => game.gameId === options.selectedTuningGameId)
  ) {
    return {
      selectedTuningScope: options.selectedTuningScope === 'game' ? 'global' : options.selectedTuningScope,
      selectedTuningGameId: ''
    };
  }

  if (options.selectedTuningScope !== 'game' && options.selectedTuningGameId) {
    return { selectedTuningScope: options.selectedTuningScope, selectedTuningGameId: '' };
  }

  return {
    selectedTuningScope: options.selectedTuningScope,
    selectedTuningGameId: options.selectedTuningGameId
  };
}

export function deriveProfileWorkspace(options: {
  snapshot: AppSnapshot | null;
  selectedTuningScope: TuningScope;
  selectedTuningGameId: string;
  selectedOverrideProfileId: string;
  currentControllerConfig: ControllerConfiguration | null;
  profileConfigDirty: boolean;
  controllerSelected: boolean;
  profileTargetSummary: string;
}): ProfileWorkspace {
  const snapshot = options.snapshot;
  const status = snapshot?.status;
  const profiles = snapshot?.profiles ?? [];
  const activeProfileId = profiles.find((profile) => profile.active)?.id ?? snapshot?.profileResolution.selectedProfileId ?? '';
  const globalProfilePreview =
    profiles.find((profile) => profile.scope === 'Global') ??
    profiles.find((profile) => profile.scope === 'Built-in' && profile.id === 'forza-horizon-immersive') ??
    profiles.find((profile) => profile.scope === 'Built-in');
  const activeProfileName = snapshot?.effectState?.selectedProfileName ?? status?.activeProfile ?? 'None';
  const activeProfile = profiles.find((profile) => profile.id === activeProfileId);
  const selectedOverrideProfile = profiles.find((profile) => profile.id === options.selectedOverrideProfileId);
  const selectedActionProfile =
    profiles.find((profile) => profile.id === (options.selectedOverrideProfileId || activeProfileId)) ??
    activeProfile ??
    null;
  const detectedGameLabel = snapshot?.gameDetection.activeGameName ?? snapshot?.profileResolution.detectedGameId ?? 'current game';
  const supportedGames = snapshot?.gameDetection.supportedGames ?? [];
  const selectedGame =
    snapshot?.gameDetection.selectedGame ??
    supportedGames.find((game) => game.gameId === snapshot?.gameDetection.activeGameId) ??
    null;
  const discoveredGames = supportedGames
    .filter((game) => game.running || game.installed || game.gameId === selectedGame?.gameId)
    .sort((left, right) =>
      Number(right.running) - Number(left.running) ||
      Number(right.installed) - Number(left.installed) ||
      left.name.localeCompare(right.name)
    );
  const selectedTuningGame = options.selectedTuningGameId
    ? supportedGames.find((game) => game.gameId === options.selectedTuningGameId) ?? null
    : null;
  const profileContextGame = options.selectedTuningScope === 'game' ? selectedTuningGame : null;
  const profileContextGameId = profileContextGame?.gameId ?? null;
  const profileContextLabel =
    options.selectedTuningScope === 'global' ? 'Global Profile' : profileContextGame?.name ?? detectedGameLabel;
  const profileContextAssignment = assignmentForGame(profileContextGame, options.currentControllerConfig);
  const profileContextDefaultProfileId =
    profileContextAssignment?.profileId ??
    defaultProfileIdForGame(profileContextGame, profiles, activeProfileId, options.currentControllerConfig);
  const profileContextDefaultProfile = profiles.find((profile) => profile.id === profileContextDefaultProfileId);
  const profileContextProfiles = profilesForGame(
    profiles,
    profileContextGame,
    profileContextDefaultProfileId,
    options.selectedOverrideProfileId,
    activeProfileId
  );
  const profileContextBadgeProfile = selectedOverrideProfile ?? profileContextProfiles[0] ?? activeProfile;
  const activeProfileContextLabel =
    options.selectedTuningScope === 'global'
      ? 'global scope'
      : profileContextGame && profileContextBadgeProfile
        ? profileContextTag(profileContextBadgeProfile, profileContextGame, profileContextDefaultProfileId, activeProfileId)
        : 'game scope';
  const detectionSignalText = gameDetectionStatusText(snapshot?.gameDetection);
  const steamContextGame = profileContextGame;
  const steamContextArt =
    gameArtwork(steamContextGame, 'capsule') ??
    gameArtwork(steamContextGame, 'banner') ??
    gameArtwork(steamContextGame, 'icon') ??
    '';
  const steamContextBackdropArt =
    gameArtwork(steamContextGame, 'banner') ??
    gameArtwork(steamContextGame, 'hero') ??
    gameArtwork(steamContextGame, 'capsule') ??
    '';
  const steamContextMeta = steamContextGame
    ? [
        gameProviderMeta(steamContextGame),
        formatPlaytime(steamContextGame.stats?.playtimeMinutes),
        achievementText(steamContextGame),
        formatLastPlayed(steamContextGame.stats?.lastPlayedUnix),
        gameTileStatus(steamContextGame)
      ]
        .filter(Boolean)
        .join(' / ')
    : options.selectedTuningScope === 'global'
      ? 'Controller-wide haptics'
      : detectionSignalText || 'Steam library data unavailable';
  const activeProfileHeader = selectedActionProfile ?? profiles.find((profile) => profile.id === activeProfileId) ?? null;
  const activeProfileHeaderName = activeProfileHeader?.name ?? activeProfileName ?? 'None';
  const activeProfileHeaderMeta = activeProfileHeader
    ? activeProfileHeaderMetaText(activeProfileHeader, {
        activeProfileId,
        steamContextGame,
        profileConfigDirty: options.profileConfigDirty
      })
    : profiles.length
      ? 'No profile resolved'
      : 'Profiles loading';
  const overrideScope =
    options.controllerSelected && snapshot
      ? `${options.profileTargetSummary} / ${profileContextLabel}`
      : profileContextLabel;
  const profileContextDetail =
    options.selectedTuningScope === 'global'
      ? 'Controller-wide tuning'
      : profileContextGame
        ? [
            gameTileStatus(profileContextGame),
            formatPlaytime(profileContextGame.stats?.playtimeMinutes),
            achievementText(profileContextGame),
            profileContextDefaultProfile ? `${profileContextDefaultProfile.name} profile` : ''
          ]
            .filter(Boolean)
            .join(' / ')
        : overrideScope;

  return {
    status,
    profiles,
    activeProfileId,
    globalProfilePreview,
    activeProfileName,
    activeProfile,
    selectedOverrideProfile,
    selectedActionProfile,
    canDeleteSelectedProfile: Boolean(selectedActionProfile && !selectedActionProfile.builtIn),
    canRenameSelectedProfile: Boolean(selectedActionProfile && !selectedActionProfile.builtIn),
    overrideActive: Boolean(snapshot?.profileResolution.overrideProfileId),
    detectedGameLabel,
    supportedGames,
    selectedGame,
    discoveredGames,
    selectedTuningGame,
    tuningReady: Boolean(options.controllerSelected && (options.selectedTuningScope === 'global' || selectedTuningGame)),
    buttonMappingReady: Boolean(options.controllerSelected && options.selectedTuningScope === 'game' && selectedTuningGame),
    profileContextGame,
    profileContextGameId,
    profileContextLabel,
    profileContextDefaultProfileId,
    profileContextDefaultProfile,
    profileContextProfiles,
    activeProfileContextLabel,
    profileContextDetail,
    detectionSignalText,
    steamContextGame,
    steamContextArt,
    steamContextBackdropArt,
    steamContextMeta,
    activeProfileHeader,
    activeProfileHeaderName,
    activeProfileHeaderMeta,
    overrideScope
  };
}

export function reconcileSelectedOverrideProfileId(options: {
  profiles: ProfileSummary[];
  selectedOverrideProfileId: string;
  profileContextDefaultProfileId: string;
  activeProfileId: string;
  profileResolution?: ProfileResolution | null;
}): string {
  if (!options.profiles.length || options.profiles.some((profile) => profile.id === options.selectedOverrideProfileId)) {
    return options.selectedOverrideProfileId;
  }
  return (
    options.profileContextDefaultProfileId ||
    options.activeProfileId ||
    options.profileResolution?.overrideProfileId ||
    options.profileResolution?.selectedProfileId ||
    options.profiles[0].id
  );
}

export function globalTuningProfileSelection(profiles: ProfileSummary[], activeProfileId: string): string {
  return (
    profiles.find((profile) => profile.id === 'global')?.id ??
    profiles.find((profile) => profile.scope === 'Global')?.id ??
    profiles.find((profile) => profile.scope !== 'Game' && profile.id === activeProfileId)?.id ??
    profiles[0]?.id ??
    ''
  );
}

export function gameTuningProfileSelection(options: {
  game: SupportedGame;
  profiles: ProfileSummary[];
  activeProfileId: string;
  currentControllerConfig: ControllerConfiguration | null;
}): string {
  return defaultProfileIdForGame(
    options.game,
    options.profiles,
    options.activeProfileId,
    options.currentControllerConfig
  );
}

export function profileTargetIdsForWorkspace(workspace: TargetControllerWorkspace): string[] {
  return resolveSelectedProfileTargetIds({
    profileTargetControllerIds: workspace.profileTargetControllerIds,
    connectedControllerIds: workspace.connectedControllerIds,
    controllerId: workspace.controller?.id
  });
}

export function profileTargetSummaryForWorkspace(workspace: TargetControllerWorkspace): string {
  return summarizeProfileTargets({
    targetIds: profileTargetIdsForWorkspace(workspace),
    controllers: workspace.controllers,
    connectedControllerIds: workspace.connectedControllerIds
  });
}

export function allProfileTargetsForWorkspace(workspace: TargetControllerWorkspace): string[] {
  return workspace.connectedControllerIds.length ? [...workspace.connectedControllerIds] : workspace.profileTargetControllerIds;
}

export function singleProfileTargetSelection(
  workspace: TargetControllerWorkspace,
  controllerId: string
): { selectedControllerId: string; profileTargetControllerIds: string[] } | null {
  if (!workspace.connectedControllerIds.includes(controllerId)) return null;
  return { selectedControllerId: controllerId, profileTargetControllerIds: [controllerId] };
}

export function targetControllerSelection(
  workspace: TargetControllerWorkspace,
  controllerId: string
): { selectedControllerId: string; profileTargetControllerIds: string[] } | null {
  if (!controllerId || controllerId === workspace.selectedControllerId) return null;
  return {
    selectedControllerId: controllerId,
    profileTargetControllerIds:
      workspace.profileTargetControllerIds.length <= 1
        ? [controllerId]
        : workspace.profileTargetControllerIds
  };
}

export async function setProfileOverrideForWorkspaceTargets(options: {
  workspace: TargetControllerWorkspace;
  profileId: string;
  gameId: string | null;
}): Promise<ProfileResolution> {
  const targetIds = profileTargetIdsForWorkspace(options.workspace);
  let resolution: ProfileResolution | null = null;
  for (const targetId of targetIds) {
    resolution = await setProfileOverride({
      controllerId: targetId,
      gameId: options.gameId,
      profileId: options.profileId
    });
  }
  return resolution ?? setProfileOverride({ controllerId: null, gameId: options.gameId, profileId: options.profileId });
}

export async function clearProfileOverrideForWorkspaceTargets(options: {
  workspace: TargetControllerWorkspace;
  gameId: string | null;
}): Promise<ProfileResolution | null> {
  const targetIds = profileTargetIdsForWorkspace(options.workspace);
  let resolution: ProfileResolution | null = null;
  for (const targetId of targetIds) {
    resolution = await clearProfileOverride({ controllerId: targetId, gameId: options.gameId });
  }
  return resolution ?? (options.gameId ? clearProfileOverride({ gameId: options.gameId }) : null);
}

export async function saveControllerConfigForWorkspaceTargets(options: {
  workspace: TargetControllerWorkspace;
  config: EditableControllerConfig;
}): Promise<ControllerConfiguration | null> {
  let selectedUpdate: ControllerConfiguration | null = null;
  for (const targetId of profileTargetIdsForWorkspace(options.workspace)) {
    const updated = await saveControllerConfig(targetId, options.config);
    if (targetId === options.workspace.selectedControllerId) selectedUpdate = updated;
  }
  return selectedUpdate;
}

export function inputBridgeBindingProfileIdForWorkspace(workspace: ProfileWorkspace): string | null {
  if (!workspace.profileContextGameId) return null;
  const selectedProfileId = workspace.selectedOverrideProfile?.id || workspace.profileContextDefaultProfileId;
  const selectedProfile = workspace.profiles.find((profile) => profile.id === selectedProfileId);
  if (
    selectedProfile &&
    !selectedProfile.builtIn &&
    selectedProfile.scope === 'Game' &&
    selectedProfile.gameId === workspace.profileContextGameId
  ) {
    return selectedProfile.id;
  }
  return (
    workspace.profiles.find(
      (profile) =>
        !profile.builtIn &&
        profile.scope === 'Game' &&
        profile.gameId === workspace.profileContextGameId
    )?.id ?? null
  );
}

function activeProfileHeaderMetaText(profile: ProfileSummary, options: {
  activeProfileId: string;
  steamContextGame: SupportedGame | null;
  profileConfigDirty: boolean;
}): string {
  const scope =
    profile.builtIn && profile.scope === 'Built-in'
      ? 'Built-in template'
      : profile.builtIn
        ? 'Stock / Global'
        : profile.scope === 'Game'
          ? `Custom / ${options.steamContextGame?.name ?? 'game'}`
          : 'Custom / Global';
  const mode = profile.id === options.activeProfileId ? 'Live profile' : 'Editing profile';
  return options.profileConfigDirty ? `${scope} / ${mode} / unsaved changes` : `${scope} / ${mode}`;
}
