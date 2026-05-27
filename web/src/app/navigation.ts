export type AppView = 'games' | 'controllers' | 'haptics' | 'buttonMapping';

export type AppViewDefinition = {
  id: AppView;
  label: string;
  hash: string;
};

export type ViewReadiness = {
  tuningReady: boolean;
  buttonMappingReady: boolean;
};

export const appViews: AppViewDefinition[] = [
  { id: 'games', label: 'Profiles', hash: '#/games' },
  { id: 'controllers', label: 'Controllers', hash: '#/controllers' },
  { id: 'haptics', label: 'Adaptive Triggers & Haptics', hash: '#/adaptive-triggers-haptics' },
  { id: 'buttonMapping', label: 'Button Mapping', hash: '#/button-mapping' }
];

export const viewTooltips: Record<AppView, string> = {
  games: 'Choose Global Profile or a supported game scope for tuning.',
  controllers: 'View controller details, live inputs, calibration readings, and DualSense Edge onboard slots.',
  haptics: 'Tune L2/R2 curves, manual trigger tests, body haptics, lightbar colors, and telemetry routes.',
  buttonMapping: 'Edit game/local-app mappings through Steam Input or DSCC Input Bridge.'
};

export function hashForView(view: AppView): string {
  return appViews.find((item) => item.id === view)?.hash ?? appViews[0].hash;
}

export function guardView(view: AppView, readiness: ViewReadiness): AppView {
  if (view === 'buttonMapping' && !readiness.buttonMappingReady) {
    return readiness.tuningReady ? 'haptics' : 'games';
  }
  if (view === 'haptics' && !readiness.tuningReady) return 'games';
  return view;
}

export function viewFromHash(hash: string, readiness: ViewReadiness): AppView {
  if (hash === '#/controllers') return 'controllers';
  if (hash === '#/button-mapping') return guardView('buttonMapping', readiness);
  if (hash === '#/adaptive-triggers-haptics') return guardView('haptics', readiness);
  return 'games';
}
