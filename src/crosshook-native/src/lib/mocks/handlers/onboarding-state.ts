import type { CachedHostReadinessSnapshot } from './onboarding-constants';

export let onboardingDismissed = false;
let cachedHostReadinessSnapshot: CachedHostReadinessSnapshot | null = null;

export function markOnboardingDismissed(): void {
  onboardingDismissed = true;
}

function resetOnboardingDismissed(): void {
  onboardingDismissed = false;
}

export function setCachedHostReadinessSnapshot(snapshot: CachedHostReadinessSnapshot | null): void {
  cachedHostReadinessSnapshot = snapshot;
}

export function getCachedHostReadinessSnapshot(): CachedHostReadinessSnapshot | null {
  return cachedHostReadinessSnapshot;
}

export function resetOnboardingState(): void {
  resetOnboardingDismissed();
  setCachedHostReadinessSnapshot(null);
}
