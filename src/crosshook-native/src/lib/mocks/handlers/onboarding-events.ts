import type { OnboardingCheckPayload } from '../../../types/onboarding';
import { getActiveToggles } from '../../toggles';
import { emitMockEvent } from '../eventBus';
import { getStore } from '../store';
import {
  ONBOARDING_EMIT_INITIAL_MS,
  ONBOARDING_EMIT_MAX_ATTEMPTS,
  ONBOARDING_EMIT_RETRY_MS,
} from './onboarding-constants';
import {
  markOnboardingEventSynthesized,
  markOnboardingSynthesisScheduled,
  onboardingEventSynthesized,
  onboardingSynthesisScheduled,
  trackOnboardingTimer,
  untrackOnboardingTimer,
} from './onboarding-state';

function scheduleOnboardingTimeout(callback: () => void, delayMs: number): void {
  const id = window.setTimeout(() => {
    untrackOnboardingTimer(id);
    callback();
  }, delayMs);
  trackOnboardingTimer(id);
}

function buildOnboardingPayload(): OnboardingCheckPayload {
  const store = getStore();
  return {
    show: true,
    has_profiles: store.profiles.size > 0,
  };
}

export function maybeSynthesizeOnboardingEvent(): void {
  if (onboardingEventSynthesized) return;
  if (!getActiveToggles().showOnboarding) return;
  if (onboardingSynthesisScheduled) return;
  markOnboardingSynthesisScheduled();

  let attempts = 0;

  const tryEmit = (): void => {
    if (onboardingEventSynthesized) return;
    if (emitMockEvent('onboarding-check', buildOnboardingPayload())) {
      markOnboardingEventSynthesized();
      return;
    }
    attempts += 1;
    if (attempts >= ONBOARDING_EMIT_MAX_ATTEMPTS) {
      return;
    }
    scheduleOnboardingTimeout(tryEmit, ONBOARDING_EMIT_RETRY_MS);
  };

  scheduleOnboardingTimeout(tryEmit, ONBOARDING_EMIT_INITIAL_MS);
}
