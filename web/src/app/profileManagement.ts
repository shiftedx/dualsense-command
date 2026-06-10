import {
  activateProfile,
  createProfile,
  deleteProfile,
  exportProfile,
  importProfile,
  renameProfile,
  saveProfileConfig
} from '../lib/api';
import {
  profileImportPayload,
  sanitizeFileName,
  uniqueProfileName
} from '../lib/features/profiles/profileSelection';
import type { EditableControllerConfig } from './profileDraft';
import type { ToastTone } from './toastState';
import type { TuningScope } from './profileWorkspace';
import type { AppSnapshot, ProfileResolution, ProfileSummary, SupportedGame } from '../lib/types';

// UI state for the Profile Resolution workflows: create, rename, save,
// save-as, delete, import, and export of DSCC Software Profiles.
export type ProfileManagementState = {
  renameProfileId: string;
  renameProfileName: string;
  renameBusy: boolean;
  saveBusy: boolean;
  saveAsOpen: boolean;
  saveAsName: string;
  saveAsBusy: boolean;
  fileBusy: boolean;
};

export const createProfileManagementState = (): ProfileManagementState => ({
  renameProfileId: '',
  renameProfileName: '',
  renameBusy: false,
  saveBusy: false,
  saveAsOpen: false,
  saveAsName: '',
  saveAsBusy: false,
  fileBusy: false
});

export type ProfileManagementStateStore = {
  get: () => ProfileManagementState;
  set: (next: ProfileManagementState) => void;
};

export type ProfileManagementDeps = {
  store: ProfileManagementStateStore;
  getSnapshot: () => AppSnapshot | null;
  setSnapshot: (next: AppSnapshot) => void;
  getProfiles: () => ProfileSummary[];
  getActiveProfileId: () => string;
  getSelectedActionProfile: () => ProfileSummary | null;
  getProfileContextGame: () => SupportedGame | null;
  getProfileContextGameId: () => string | null;
  getSelectedTuningScope: () => TuningScope;
  getSelectedOverrideProfileId: () => string;
  setSelectedOverrideProfileId: (id: string) => void;
  markActiveProfileSynced: (id: string) => void;
  getControllerId: () => string | undefined;
  resetConfigLoaded: () => void;
  loadControllerConfig: (controllerId: string) => Promise<void>;
  buildControllerConfig: () => EditableControllerConfig;
  profileConfigSignature: (config: EditableControllerConfig) => string;
  setProfileSaveBaseline: (signature: string) => void;
  saveControllerConfigForProfileTargets: (config: EditableControllerConfig) => Promise<void>;
  setProfileOverrideForTargets: (profileId: string, gameId: string | null) => Promise<ProfileResolution | null>;
  refresh: () => Promise<void>;
  notify: (message: string, tone?: ToastTone) => void;
};

export const createProfileManagement = (deps: ProfileManagementDeps) => {
  const state = () => deps.store.get();
  const patch = (partial: Partial<ProfileManagementState>) => {
    deps.store.set({ ...deps.store.get(), ...partial });
  };

  const applyResolution = (resolution: ProfileResolution | null) => {
    const snapshot = deps.getSnapshot();
    if (snapshot && resolution) deps.setSnapshot({ ...snapshot, profileResolution: resolution });
  };

  const activateProfileById = async (id: string) => {
    // Optimistic UI update so rapid clicks feel instant: flip the active flag
    // locally and align the dropdown BEFORE the server round-trip resolves.
    const snapshot = deps.getSnapshot();
    if (snapshot) {
      deps.setSnapshot({
        ...snapshot,
        profiles: snapshot.profiles.map((profile) => ({ ...profile, active: profile.id === id }))
      });
    }
    deps.setSelectedOverrideProfileId(id);
    deps.markActiveProfileSynced(id);
    try {
      await activateProfile(id);
      // After activation, reload the active controller's config so the
      // Forza effect table reflects the profile's preset values immediately.
      const controllerId = deps.getControllerId();
      if (controllerId) {
        deps.resetConfigLoaded();
        await deps.loadControllerConfig(controllerId);
      }
      await deps.refresh();
    } catch (caught) {
      deps.notify(caught instanceof Error ? caught.message : 'Failed to activate profile');
      // On failure, force a refresh so the UI snaps back to server truth.
      await deps.refresh();
    }
  };

  const cancelRenameProfile = () => {
    patch({ renameProfileId: '', renameProfileName: '' });
  };

  const beginRenameSelectedProfile = () => {
    const selected = deps.getSelectedActionProfile();
    if (!selected || selected.builtIn) return;
    patch({
      saveAsOpen: false,
      saveAsName: '',
      renameProfileId: selected.id,
      renameProfileName: selected.name
    });
  };

  const submitRenameProfile = async () => {
    const profiles = deps.getProfiles();
    const profile = profiles.find((item) => item.id === state().renameProfileId);
    const name = state().renameProfileName.trim();
    if (!profile || profile.builtIn) {
      cancelRenameProfile();
      return;
    }
    if (!name) {
      deps.notify('Profile name cannot be empty', 'error');
      return;
    }
    if (name === profile.name) {
      cancelRenameProfile();
      return;
    }
    if (profiles.some((item) => item.id !== profile.id && item.name.trim().toLowerCase() === name.toLowerCase())) {
      deps.notify('A profile with that name already exists', 'error');
      return;
    }

    patch({ renameBusy: true });
    try {
      const renamed = await renameProfile(profile.id, name);
      const snapshot = deps.getSnapshot();
      if (snapshot) {
        deps.setSnapshot({
          ...snapshot,
          profiles: snapshot.profiles.map((item) => (item.id === renamed.id ? { ...item, name: renamed.name } : item))
        });
      }
      cancelRenameProfile();
      await deps.refresh();
      deps.notify(`Renamed profile to ${renamed.name}`, 'success');
    } catch (caught) {
      deps.notify(caught instanceof Error ? caught.message : 'Unable to rename profile', 'error');
      await deps.refresh();
    } finally {
      patch({ renameBusy: false });
    }
  };

  const handleRenameProfileKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submitRenameProfile();
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      cancelRenameProfile();
    }
  };

  const cancelSaveAsProfile = () => {
    patch({ saveAsOpen: false, saveAsName: '' });
  };

  const beginSaveAsProfile = () => {
    const selected = deps.getSelectedActionProfile();
    if (!selected) {
      deps.notify('No profile selected', 'error');
      return;
    }
    cancelRenameProfile();
    patch({
      saveAsName: uniqueProfileName(`${selected.name} copy`, deps.getProfiles()),
      saveAsOpen: true
    });
  };

  const submitSaveAsProfile = async () => {
    const selected = deps.getSelectedActionProfile();
    const name = state().saveAsName.trim();
    if (!selected || state().saveAsBusy) {
      if (!selected) deps.notify('No profile selected', 'error');
      return;
    }
    if (!name) {
      deps.notify('Profile name cannot be empty', 'error');
      return;
    }
    if (deps.getProfiles().some((profile) => profile.name.trim().toLowerCase() === name.toLowerCase())) {
      deps.notify('A profile with that name already exists', 'error');
      return;
    }

    patch({ saveAsBusy: true });
    try {
      const config = deps.buildControllerConfig();
      const created = await createProfile(name, {
        gameId: deps.getSelectedTuningScope() === 'game' ? deps.getProfileContextGameId() : null
      });
      const response = await saveProfileConfig(created.id, config);
      await deps.saveControllerConfigForProfileTargets(config);
      const resolution = await deps.setProfileOverrideForTargets(created.id, deps.getProfileContextGameId());
      applyResolution(resolution);
      deps.setProfileSaveBaseline(deps.profileConfigSignature(config));
      deps.setSelectedOverrideProfileId(created.id);
      cancelSaveAsProfile();
      await deps.refresh();
      deps.setSelectedOverrideProfileId(created.id);
      deps.notify(response.message || `Saved ${created.name}`, 'success');
    } catch (caught) {
      deps.notify(caught instanceof Error ? caught.message : 'Unable to save profile copy', 'error');
      await deps.refresh();
    } finally {
      patch({ saveAsBusy: false });
    }
  };

  const handleSaveAsProfileKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submitSaveAsProfile();
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      cancelSaveAsProfile();
    }
  };

  const saveActiveProfile = async () => {
    const selected = deps.getSelectedActionProfile();
    if (!selected || state().saveBusy) {
      if (!selected) deps.notify('No profile selected');
      return;
    }

    patch({ saveBusy: true });
    try {
      const sourceProfileName = selected.name;
      let targetProfile: ProfileSummary = selected;
      let preservingStockProfile = false;
      if (targetProfile?.builtIn) {
        const contextGame = deps.getProfileContextGame();
        const name = uniqueProfileName(
          contextGame ? `${contextGame.name} ${targetProfile.name} custom` : `${targetProfile.name} custom`,
          deps.getProfiles()
        );
        targetProfile = await createProfile(name, { gameId: deps.getProfileContextGameId() });
        preservingStockProfile = true;
      }
      if (!targetProfile) throw new Error('No profile selected');

      const config = deps.buildControllerConfig();
      await deps.saveControllerConfigForProfileTargets(config);
      const response = await saveProfileConfig(targetProfile.id, config);
      deps.setProfileSaveBaseline(deps.profileConfigSignature(config));
      const resolution = await deps.setProfileOverrideForTargets(targetProfile.id, deps.getProfileContextGameId());
      applyResolution(resolution);
      deps.setSelectedOverrideProfileId(targetProfile.id);
      await deps.refresh();
      deps.setSelectedOverrideProfileId(targetProfile.id);
      deps.notify(
        preservingStockProfile
          ? `Saved ${targetProfile.name}; stock ${sourceProfileName} preserved`
          : response.message || `Saved ${targetProfile.name}`
      );
    } catch (caught) {
      deps.notify(caught instanceof Error ? caught.message : 'Unable to save profile');
    } finally {
      patch({ saveBusy: false });
    }
  };

  const deleteProfileById = async (id: string, name: string) => {
    const profiles = deps.getProfiles();
    const fallbackProfileId =
      profiles.find((profile) => profile.id === 'global')?.id ??
      profiles.find((profile) => profile.id !== id && profile.scope === 'Built-in')?.id ??
      profiles.find((profile) => profile.id !== id)?.id ??
      '';
    if (state().renameProfileId === id) cancelRenameProfile();
    patch({ fileBusy: true });
    try {
      const snapshot = deps.getSnapshot();
      if (snapshot) {
        deps.setSnapshot({
          ...snapshot,
          profiles: snapshot.profiles.filter((profile) => profile.id !== id)
        });
      }
      const response = await deleteProfile(id);
      await deps.refresh();
      if (deps.getSelectedOverrideProfileId() === id) deps.setSelectedOverrideProfileId(fallbackProfileId);
      deps.notify(response?.message ?? `Deleted ${name}`);
    } catch (caught) {
      deps.notify(caught instanceof Error ? caught.message : 'Failed to delete profile');
      await deps.refresh();
    } finally {
      patch({ fileBusy: false });
    }
  };

  const exportSelectedProfile = async () => {
    const profileId = deps.getSelectedOverrideProfileId() || deps.getActiveProfileId();
    if (!profileId || state().fileBusy) {
      if (!profileId) deps.notify('Select a profile to export');
      return;
    }
    patch({ fileBusy: true });
    try {
      const exported = await exportProfile(profileId);
      const body = JSON.stringify(exported, null, 2);
      const url = URL.createObjectURL(new Blob([body], { type: 'application/json' }));
      const link = document.createElement('a');
      link.href = url;
      link.download = `${sanitizeFileName(exported.name)}.dscc-profile.json`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      deps.notify(`Exported ${exported.name}`);
    } catch (caught) {
      deps.notify(caught instanceof Error ? caught.message : 'Unable to export profile');
    } finally {
      patch({ fileBusy: false });
    }
  };

  const handleProfileImport = async (event: Event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file || state().fileBusy) return;

    patch({ fileBusy: true });
    try {
      const payload = profileImportPayload(JSON.parse(await file.text()), deps.getProfiles());
      const imported = await importProfile(payload);
      deps.setSelectedOverrideProfileId(imported.id);
      await deps.refresh();
      deps.notify(`Imported ${imported.name}`);
    } catch (caught) {
      deps.notify(caught instanceof Error ? caught.message : 'Unable to import profile');
    } finally {
      patch({ fileBusy: false });
    }
  };

  return {
    activateProfileById,
    beginRenameSelectedProfile,
    cancelRenameProfile,
    submitRenameProfile,
    handleRenameProfileKeydown,
    beginSaveAsProfile,
    cancelSaveAsProfile,
    submitSaveAsProfile,
    handleSaveAsProfileKeydown,
    saveActiveProfile,
    deleteProfileById,
    exportSelectedProfile,
    handleProfileImport
  };
};

export type ProfileManagement = ReturnType<typeof createProfileManagement>;
