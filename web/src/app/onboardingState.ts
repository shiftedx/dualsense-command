const ONBOARDING_DISMISSED_KEY = 'dscc-onboarding-v1-dismissed';

export function shouldOpenOnboarding(): boolean {
  if (typeof window === 'undefined') return false;
  try {
    return window.localStorage.getItem(ONBOARDING_DISMISSED_KEY) !== '1';
  } catch {
    return false;
  }
}

export function markOnboardingDismissed(): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(ONBOARDING_DISMISSED_KEY, '1');
  } catch {
    // First-run help is optional convenience state.
  }
}

