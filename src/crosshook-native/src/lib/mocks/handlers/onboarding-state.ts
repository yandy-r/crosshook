import type { CachedHostReadinessSnapshot } from './onboarding-constants';

export let onboardingDismissed = false;
export let onboardingEventSynthesized = false;
export let onboardingSynthesisScheduled = false;
export let cachedHostReadinessSnapshot: CachedHostReadinessSnapshot | null = null;
export const pendingOnboardingTimers = new Set<number>();

export function markOnboardingDismissed(): void {
  onboardingDismissed = true;
}

export function resetOnboardingDismissed(): void {
  onboardingDismissed = false;
}

export function markOnboardingEventSynthesized(): void {
  onboardingEventSynthesized = true;
}

export function resetOnboardingEventSynthesized(): void {
  onboardingEventSynthesized = false;
}

export function markOnboardingSynthesisScheduled(): void {
  onboardingSynthesisScheduled = true;
}

export function resetOnboardingSynthesisScheduled(): void {
  onboardingSynthesisScheduled = false;
}

export function setCachedHostReadinessSnapshot(snapshot: CachedHostReadinessSnapshot | null): void {
  cachedHostReadinessSnapshot = snapshot;
}

export function getCachedHostReadinessSnapshot(): CachedHostReadinessSnapshot | null {
  return cachedHostReadinessSnapshot;
}

export function trackOnboardingTimer(id: number): void {
  pendingOnboardingTimers.add(id);
}

export function untrackOnboardingTimer(id: number): void {
  pendingOnboardingTimers.delete(id);
}

export function cancelPendingOnboardingTimers(): void {
  for (const timerId of pendingOnboardingTimers) {
    window.clearTimeout(timerId);
  }
  pendingOnboardingTimers.clear();
}

export function resetOnboardingState(): void {
  resetOnboardingDismissed();
  resetOnboardingEventSynthesized();
  resetOnboardingSynthesisScheduled();
  setCachedHostReadinessSnapshot(null);
  cancelPendingOnboardingTimers();
}
