import type {
  AppSnapshot,
  ControllerStatus,
  CurrentEffectState,
  DiagnosticsCheck,
  LogEntry,
  ProfileSummary,
  SupportBundle,
  SupportedGame
} from '../lib/types';

export type UiSupportBundleContext = {
  snapshot: AppSnapshot | null;
  status: AppSnapshot['status'] | undefined;
  listenOnAllInterfaces: boolean;
  selectedTuningScope: string;
  selectedTuningGame: SupportedGame | null;
  activeProfile: ProfileSummary | undefined;
  controllers: ControllerStatus[];
  diagnostics: DiagnosticsCheck[];
  supportedGames: SupportedGame[];
  effectState: CurrentEffectState | undefined;
  logs: LogEntry[];
  agentBundleError?: string;
};

export function sanitizeSupportText(value: string | undefined | null): string {
  return (value ?? '')
    .replace(/[A-Z]:\\[^"'\r\n]+/gi, '[local-path]')
    .replace(/\/(?:home|Users)\/[^"'\r\n]+/gi, '[local-path]');
}

export function buildUiSupportBundle(context: UiSupportBundleContext): SupportBundle {
  const {
    snapshot,
    status,
    listenOnAllInterfaces,
    selectedTuningScope,
    selectedTuningGame,
    activeProfile,
    controllers,
    diagnostics,
    supportedGames,
    effectState,
    logs,
    agentBundleError
  } = context;

  return {
    schema: 'dev.dscc.support-bundle.ui.v1',
    generatedAt: new Date().toISOString(),
    source: 'web-ui-sanitized-fallback',
    privacy: {
      sanitized: true,
      omitted: [
        'raw HID paths',
        'raw controller hardware IDs',
        'serial numbers',
        'Bluetooth addresses',
        'Steam install paths',
        'local app executable paths',
        'virtual-output provider private paths'
      ]
    },
    app: {
      version: status?.version ?? 'unknown',
      health: status?.health ?? 'unavailable',
      uptime: status?.uptime ?? 'unknown',
      apiBinding: listenOnAllInterfaces ? 'lan' : 'loopback',
      activeAdapter: status?.activeAdapter ?? 'None'
    },
    environment: {
      userAgent: navigator.userAgent,
      language: navigator.language,
      viewport: `${window.innerWidth}x${window.innerHeight}`
    },
    selectedContext: {
      scope: selectedTuningScope,
      game: selectedTuningGame ? { gameId: selectedTuningGame.gameId, name: selectedTuningGame.name } : null,
      profile: activeProfile ? { scope: activeProfile.scope, builtIn: activeProfile.builtIn, name: activeProfile.name } : null
    },
    controllers: controllers.map((item, index) => ({
      index,
      family: item.family,
      transport: item.transport,
      connected: item.connected,
      batteryState: item.batteryState,
      battery: item.battery,
      permission: item.permission,
      diagnosticState: item.diagnosticState,
      capabilities: item.capabilities
    })),
    adapters: snapshot?.adapters.map((item) => ({
      id: item.id,
      name: item.name,
      state: item.state,
      packetRateHz: item.packetRateHz,
      setupHint: item.setupHint
    })) ?? [],
    diagnostics,
    partialErrors: snapshot?.partialErrors ?? [],
    gameDetection: {
      activeGameName: snapshot?.gameDetection.activeGameName ?? null,
      source: snapshot?.gameDetection.source ?? 'unknown',
      confidence: snapshot?.gameDetection.confidence ?? 0,
      moduleId: snapshot?.gameDetection.moduleId ?? null,
      adapterId: snapshot?.gameDetection.adapterId ?? null,
      supportedGames: supportedGames.map((game) => ({
        gameId: game.gameId,
        name: game.name,
        source: game.source,
        inputProvider: game.inputProvider,
        installed: game.installed,
        running: game.running,
        supportLevel: game.supportLevel
      }))
    },
    profileResolution: {
      reason: snapshot?.profileResolution.reason ?? 'unknown',
      validation: snapshot?.profileResolution.validation ?? 'unknown',
      detectedGameId: snapshot?.profileResolution.detectedGameId ?? null,
      activeAdapterId: snapshot?.profileResolution.activeAdapterId ?? null
    },
    steamInput: {
      running: snapshot?.steamInput.running ?? false,
      available: snapshot?.steamInput.available ?? false,
      layoutCount: snapshot?.steamInput.layouts.length ?? 0,
      warnings: snapshot?.steamInput.warnings.map(sanitizeSupportText) ?? []
    },
    inputBridge: snapshot?.inputBridge
      ? {
        available: snapshot.inputBridge.available,
        backendId: snapshot.inputBridge.backendId,
        provider: snapshot.inputBridge.provider,
        state: snapshot.inputBridge.state,
        message: sanitizeSupportText(snapshot.inputBridge.message),
        sessionCount: snapshot.inputBridge.sessions.length,
        warnings: snapshot.inputBridge.warnings.map(sanitizeSupportText)
      }
      : null,
    effectState: effectState
      ? {
        reason: effectState.reason,
        dryRun: effectState.dryRun,
        hardwareOutputEnabled: effectState.hardwareOutputEnabled,
        warnings: effectState.warnings.map(sanitizeSupportText),
        parityEffects: effectState.parityEffects
      }
      : null,
    logs: logs.slice(-40).map((entry) => ({
      level: entry.level,
      time: entry.time,
      source: entry.source,
      message: sanitizeSupportText(entry.message)
    })),
    agentBundleError: agentBundleError ? sanitizeSupportText(agentBundleError) : undefined
  };
}

export function supportBundleFileName(): string {
  return `dscc-support-${new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)}.json`;
}

export function downloadSupportBundleText(text: string): void {
  const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = supportBundleFileName();
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
