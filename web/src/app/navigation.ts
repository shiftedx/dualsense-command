export type AppView = 'status' | 'tuning' | 'advancedController' | 'advancedButtonMapping' | 'advancedEdgeSlots';

export type AppViewDefinition = {
  id: AppView;
  label: string;
  hash: string;
  group: 'main' | 'advanced';
};

export type ViewReadiness = {
  tuningReady: boolean;
  buttonMappingReady: boolean;
  /** True when the Target Controller is a DualSense Edge. */
  edgeSlotsReady: boolean;
};

export const appViews: AppViewDefinition[] = [
  { id: 'status', label: 'Status', hash: '#/status', group: 'main' },
  { id: 'tuning', label: 'Tuning', hash: '#/tuning', group: 'main' },
  { id: 'advancedController', label: 'Controller details', hash: '#/advanced/controller', group: 'advanced' },
  { id: 'advancedButtonMapping', label: 'Button mapping', hash: '#/advanced/button-mapping', group: 'advanced' },
  { id: 'advancedEdgeSlots', label: 'Edge onboard slots', hash: '#/advanced/edge-slots', group: 'advanced' }
];

export const viewTooltips: Record<AppView, string> = {
  status: 'Check that your controller, game detection, and telemetry are working.',
  tuning: 'Tune trigger feel, rumble, and lights for a game or the Global Profile.',
  advancedController: 'Live inputs, calibration readings, and connection details.',
  advancedButtonMapping: 'Edit game/local-app mappings through Steam Input or DSCC Input Bridge.',
  advancedEdgeSlots: 'Manage DualSense Edge onboard profile slots.'
};

/** Old routes keep working forever; they land on the new home for that content. */
const legacyRedirects: Record<string, string> = {
  '#/games': '#/tuning',
  '#/adaptive-triggers-haptics': '#/tuning',
  '#/controllers': '#/advanced/controller',
  '#/button-mapping': '#/advanced/button-mapping'
};

/** Every hash the router answers to: current view hashes plus legacy redirects. */
export const knownViewHashes: string[] = [
  ...appViews.map((item) => item.hash),
  ...Object.keys(legacyRedirects)
];

export function isViewHash(hash: string): boolean {
  return knownViewHashes.includes(hash);
}

export function hashForView(view: AppView): string {
  return appViews.find((item) => item.id === view)?.hash ?? '#/status';
}

export function guardView(view: AppView, readiness: ViewReadiness): AppView {
  if (view === 'tuning' && !readiness.tuningReady) return 'status';
  if (view === 'advancedButtonMapping' && !readiness.buttonMappingReady) return 'status';
  // Edge onboard slots only exist on a DualSense Edge; direct hash navigation
  // lands on Controller details, which explains the selected controller.
  if (view === 'advancedEdgeSlots' && !readiness.edgeSlotsReady) return 'advancedController';
  return view;
}

export function viewFromHash(rawHash: string, readiness: ViewReadiness): AppView {
  const hash = legacyRedirects[rawHash] ?? rawHash;
  const match = appViews.find((item) => item.hash === hash);
  return guardView(match?.id ?? 'status', readiness);
}
