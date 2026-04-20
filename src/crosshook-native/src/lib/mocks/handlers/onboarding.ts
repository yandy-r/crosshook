import type { TrainerGuidanceContent } from '../../../types/onboarding';
import { getStore } from '../store';
import { maybeSynthesizeOnboardingEvent } from './onboarding-events';
import {
  applyReadinessOverlays,
  buildBaseReadinessPayload,
  buildMockReadinessResult,
  deriveMockCapabilities,
  getHostToolDetails,
  isDismissReadinessNagArgs,
  persistReadinessSnapshot,
  requireProbeToolId,
  sanitizeCachedSnapshot,
} from './onboarding-readiness';
import {
  getCachedHostReadinessSnapshot,
  markOnboardingDismissed,
  onboardingDismissed,
  resetOnboardingState,
} from './onboarding-state';
import { buildTrainerGuidanceContent } from './onboarding-trainer';
import type { Handler } from './types';

maybeSynthesizeOnboardingEvent();

export function registerOnboarding(map: Map<string, Handler>): void {
  maybeSynthesizeOnboardingEvent();

  map.set('check_readiness', async () => {
    return buildMockReadinessResult();
  });

  map.set('check_generalized_readiness', async () => {
    const raw = buildBaseReadinessPayload();
    persistReadinessSnapshot(raw);
    return applyReadinessOverlays(raw);
  });

  map.set('probe_host_tool_details', async (args: unknown) => {
    return getHostToolDetails(requireProbeToolId(args));
  });

  map.set('get_cached_host_readiness_snapshot', async () => {
    return sanitizeCachedSnapshot(getCachedHostReadinessSnapshot());
  });

  map.set('get_capabilities', async () => {
    if (getCachedHostReadinessSnapshot() == null) {
      persistReadinessSnapshot(buildBaseReadinessPayload());
    }
    const sanitized = sanitizeCachedSnapshot(getCachedHostReadinessSnapshot());
    if (sanitized == null) {
      return [];
    }
    return structuredClone(deriveMockCapabilities(sanitized.tool_checks));
  });

  map.set('dismiss_onboarding', async () => {
    markOnboardingDismissed();
    const store = getStore();
    store.settings.onboarding_completed = true;
    return null;
  });

  map.set('dismiss_umu_install_nag', async () => {
    const store = getStore();
    store.settings.install_nag_dismissed_at = new Date().toISOString();
    return null;
  });

  map.set('dismiss_steam_deck_caveats', async () => {
    getStore().settings.steam_deck_caveats_dismissed_at = new Date().toISOString();
    return null;
  });

  map.set('dismiss_readiness_nag', async (args: unknown) => {
    if (!isDismissReadinessNagArgs(args)) {
      throw new Error('dismiss_readiness_nag requires a non-empty toolId');
    }
    getStore().dismissedReadinessToolIds.add(args.toolId);
    return null;
  });

  map.set('get_trainer_guidance', async (): Promise<TrainerGuidanceContent> => {
    return buildTrainerGuidanceContent();
  });
}

export function resetOnboardingMockState(): void {
  resetOnboardingState();
}

export { onboardingDismissed };
