// Per-game setup verification flags. A game is "verified" once telemetry
// packets have been seen for it at least once; after that the setup guide
// never auto-shows again (re-entry stays manual via the telemetry chip or the
// game dropdown). Persists in localStorage alongside the other UI preferences
// (see onboardingState.ts / updateState.ts for the same pattern).

const SETUP_VERIFIED_KEY = 'dscc-setup-verified-v1';

export type VerifiedSetupGameIds = Record<string, true>;

export function loadVerifiedSetupGameIds(): VerifiedSetupGameIds {
  if (typeof window === 'undefined') return {};
  try {
    const raw = window.localStorage.getItem(SETUP_VERIFIED_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    const next: VerifiedSetupGameIds = {};
    for (const key of Object.keys(parsed)) {
      if ((parsed as Record<string, unknown>)[key]) next[key] = true;
    }
    return next;
  } catch {
    return {};
  }
}

export function markSetupVerified(
  current: VerifiedSetupGameIds,
  gameId: string
): VerifiedSetupGameIds {
  if (!gameId || current[gameId]) return current;
  const next: VerifiedSetupGameIds = { ...current, [gameId]: true };
  if (typeof window !== 'undefined') {
    try {
      window.localStorage.setItem(SETUP_VERIFIED_KEY, JSON.stringify(next));
    } catch {
      // Verification is a convenience flag; the guide simply auto-shows again.
    }
  }
  return next;
}
