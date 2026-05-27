<script lang="ts">
  import GlobalFeelPanel from './GlobalFeelPanel.svelte';
  import LightbarControls from './LightbarControls.svelte';
  import TelemetryRoutingPanel from './TelemetryRoutingPanel.svelte';
  import ProfileConsole from '../profiles/ProfileConsole.svelte';
  import type {
    AppSnapshot,
    ForzaBodyRumbleMode,
    ForzaEffectConfiguration,
    ForzaEffectRoute,
    ProfileSummary
  } from '../../types';
  import type { ForzaEffectMeta, LightbarColorTarget } from './hapticsModel';

  type TuningScope = 'none' | 'global' | 'game';
  type PatternOption = {
    label: string;
    badge?: string;
  };
  type BodyRumbleModeOption = {
    value: ForzaBodyRumbleMode;
    label: string;
    badge: string;
    help: string;
  };
  type RouteOption = {
    value: ForzaEffectRoute;
    label: string;
  };

  const noop = () => undefined;

  export let selectedTuningScope: TuningScope = 'none';
  export let snapshot: AppSnapshot | null = null;
  export let baseFeelTestActive = false;
  export let baseFeelTestBusy = false;
  export let triggerEffect = 'Adaptive resistance';
  export let triggerIntensity = 'Strong (Standard)';
  export let vibrationIntensity = 'Medium';
  export let vibrationMode = 'Balanced';
  export let triggerEffectOptions: PatternOption[] = [];
  export let vibrationModeOptions: PatternOption[] = [];
  export let triggerEffectHelp: Record<string, string> = {};
  export let vibrationModeHelp: Record<string, string> = {};
  export let setTriggerEffect: (value: string) => void = noop as (value: string) => void;
  export let setVibrationIntensity: (value: string) => void = noop as (value: string) => void;
  export let setVibrationMode: (value: string) => void = noop as (value: string) => void;
  export let toggleBaseFeelTest: () => Promise<void> | void = noop;
  export let previewBodyHaptics: () => Promise<void> | void = noop;

  export let enabledForzaEffectCount = 0;
  export let allForzaEffectsEnabled = false;
  export let forzaEffectMetas: ForzaEffectMeta[] = [];
  export let forzaEffectsById: ReadonlyMap<string, ForzaEffectConfiguration> = new Map();
  export let effectStatusById: ReadonlyMap<string, { state?: string }> = new Map();
  export let forzaBodyRumbleMode: ForzaBodyRumbleMode = 'native_passthrough';
  export let bodyRumbleModeOptions: BodyRumbleModeOption[] = [];
  export let forzaRoutes: RouteOption[] = [];
  export let forzaEffect: (id: string) => ForzaEffectConfiguration = (id) => ({
    id,
    enabled: false,
    intensity: 0,
    route: 'body_both'
  });
  export let toggleAllForzaEffects: () => void = noop;
  export let setForzaBodyRumbleMode: (value: ForzaBodyRumbleMode) => void = noop as (value: ForzaBodyRumbleMode) => void;
  export let updateForzaEffect: (id: string, patch: Partial<ForzaEffectConfiguration>) => void = noop as (
    id: string,
    patch: Partial<ForzaEffectConfiguration>
  ) => void;
  export let intensityTooltip: (meta: ForzaEffectMeta, intensity: number) => string = () => '';
  export let routeTooltip: (route: ForzaEffectRoute) => string = () => '';
  export let forzaIntensityPercent: (intensity: number) => number = () => 0;
  export let forzaIntensityFromPercent: (value: number | string) => number = () => 0;

  export let lightbarEnabled = true;
  export let lightbarColor = '#4cc9f0';
  export let rpmColor = '#ff3a2e';
  export let lightbarBrightness = 72;
  export let onColorChange: (target: LightbarColorTarget, color: string) => void = noop as (
    target: LightbarColorTarget,
    color: string
  ) => void;
  export let setLightbarBrightness: (value: number | string) => void = noop as (value: number | string) => void;
  export let setLightbarEnabled: (enabled: boolean) => void = noop as (enabled: boolean) => void;
  export let previewLightbar: () => Promise<void> | void = noop;
  export let previewRpmColor: () => Promise<void> | void = noop;

  export let profileContextProfiles: ProfileSummary[] = [];
  export let activeProfileId = '';
  export let selectedOverrideProfileId = '';
  export let selectedActionProfile: ProfileSummary | null | undefined = null;
  export let selectedGameName = 'game';
  export let canRenameSelectedProfile = false;
  export let canDeleteSelectedProfile = false;
  export let profileConfigDirty = false;
  export let profileSaveBusy = false;
  export let profileFileBusy = false;
  export let profileSaveAsBusy = false;
  export let profileRenameBusy = false;
  export let saveAsProfileOpen = false;
  export let saveAsProfileName = '';
  export let renameProfileId = '';
  export let renameProfileName = '';
  export let onSelectProfile: (profileId: string) => void | Promise<void> = noop as (profileId: string) => void;
  export let onImportFile: (event: Event) => void | Promise<void> = noop as (event: Event) => void;
  export let onExportProfile: () => void | Promise<void> = noop;
  export let onBeginSaveAs: () => void = noop;
  export let onCancelSaveAs: () => void = noop;
  export let onSubmitSaveAs: () => void | Promise<void> = noop;
  export let onSaveAsKeydown: (event: KeyboardEvent) => void = noop as (event: KeyboardEvent) => void;
  export let onBeginRename: () => void = noop;
  export let onCancelRename: () => void = noop;
  export let onSubmitRename: () => void | Promise<void> = noop;
  export let onRenameKeydown: (event: KeyboardEvent) => void = noop as (event: KeyboardEvent) => void;
  export let onDeleteProfile: (profile: ProfileSummary) => void | Promise<void> = noop as (profile: ProfileSummary) => void;
  export let onRestoreDefaults: () => void | Promise<void> = noop;
  export let onSaveProfile: () => void | Promise<void> = noop;
</script>

<aside
  class:dm-global-feel={selectedTuningScope === 'global'}
  class="dm-routing"
  aria-label={selectedTuningScope === 'global' ? 'Controller haptic tuning' : 'Telemetry haptic routing'}
>
  {#if selectedTuningScope === 'global'}
    <GlobalFeelPanel
      {snapshot}
      {baseFeelTestActive}
      {baseFeelTestBusy}
      {triggerEffect}
      {triggerIntensity}
      {vibrationIntensity}
      {vibrationMode}
      {triggerEffectOptions}
      {vibrationModeOptions}
      {triggerEffectHelp}
      {vibrationModeHelp}
      {setTriggerEffect}
      {setVibrationIntensity}
      {setVibrationMode}
      {toggleBaseFeelTest}
      {previewBodyHaptics}
    />
  {:else}
    <TelemetryRoutingPanel
      {enabledForzaEffectCount}
      {allForzaEffectsEnabled}
      {forzaEffectMetas}
      {forzaEffectsById}
      {effectStatusById}
      {forzaBodyRumbleMode}
      {bodyRumbleModeOptions}
      {forzaRoutes}
      {forzaEffect}
      {toggleAllForzaEffects}
      {setForzaBodyRumbleMode}
      {updateForzaEffect}
      {intensityTooltip}
      {routeTooltip}
      {forzaIntensityPercent}
      {forzaIntensityFromPercent}
    />
  {/if}

  <LightbarControls
    {selectedTuningScope}
    {lightbarEnabled}
    bind:lightbarColor
    bind:rpmColor
    {lightbarBrightness}
    {onColorChange}
    {setLightbarBrightness}
    {setLightbarEnabled}
    {previewLightbar}
    {previewRpmColor}
  />

  <ProfileConsole
    {profileContextProfiles}
    {activeProfileId}
    {selectedOverrideProfileId}
    {selectedActionProfile}
    {selectedGameName}
    {canRenameSelectedProfile}
    {canDeleteSelectedProfile}
    {profileConfigDirty}
    {profileSaveBusy}
    {profileFileBusy}
    {profileSaveAsBusy}
    {profileRenameBusy}
    {saveAsProfileOpen}
    bind:saveAsProfileName
    {renameProfileId}
    bind:renameProfileName
    onSelectProfile={onSelectProfile}
    onImportFile={onImportFile}
    onExportProfile={onExportProfile}
    onBeginSaveAs={onBeginSaveAs}
    onCancelSaveAs={onCancelSaveAs}
    onSubmitSaveAs={onSubmitSaveAs}
    onSaveAsKeydown={onSaveAsKeydown}
    onBeginRename={onBeginRename}
    onCancelRename={onCancelRename}
    onSubmitRename={onSubmitRename}
    onRenameKeydown={onRenameKeydown}
    onDeleteProfile={onDeleteProfile}
    onRestoreDefaults={onRestoreDefaults}
    onSaveProfile={onSaveProfile}
  />
</aside>
