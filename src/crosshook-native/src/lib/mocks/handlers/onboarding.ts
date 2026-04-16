import type { OnboardingCheckPayload, ReadinessCheckResult, TrainerGuidanceContent } from '../../../types/onboarding';
import { getActiveToggles } from '../../toggles';
import { emitMockEvent } from '../eventBus';
import { getStore } from '../store';
import type { Handler } from './types';

let onboardingDismissed = false;

// Synthesize the `onboarding-check` event ONCE per session when
// `?onboarding=show` is present in the URL. The guard prevents HMR or
// re-imports of this module from re-firing the event. The 500ms delay
// ensures App.tsx has already mounted and called `subscribeEvent()` before
// the emit fans out — without it, the event would race the subscription and
// the listener would miss the payload.
let onboardingEventSynthesized = false;

/** Prevents duplicate retry loops from module init + registerOnboarding(). */
let onboardingSynthesisScheduled = false;

const ONBOARDING_EMIT_INITIAL_MS = 500;
const ONBOARDING_EMIT_RETRY_MS = 200;
const ONBOARDING_EMIT_MAX_ATTEMPTS = 25;

function maybeSynthesizeOnboardingEvent(): void {
  if (onboardingEventSynthesized) return;
  if (!getActiveToggles().showOnboarding) return;
  if (onboardingSynthesisScheduled) return;
  onboardingSynthesisScheduled = true;

  let attempts = 0;

  const tryEmit = (): void => {
    if (onboardingEventSynthesized) return;
    const store = getStore();
    const payload: OnboardingCheckPayload = {
      show: true,
      has_profiles: store.profiles.size > 0,
    };
    if (emitMockEvent('onboarding-check', payload)) {
      onboardingEventSynthesized = true;
      return;
    }
    attempts += 1;
    if (attempts >= ONBOARDING_EMIT_MAX_ATTEMPTS) {
      return;
    }
    setTimeout(tryEmit, ONBOARDING_EMIT_RETRY_MS);
  };

  setTimeout(tryEmit, ONBOARDING_EMIT_INITIAL_MS);
}

// Eagerly schedule the synthesized event at module init so it fires even if
// nothing else triggers `registerOnboarding()` later. The guard above makes
// the second call from `registerOnboarding()` a no-op.
maybeSynthesizeOnboardingEvent();

export function registerOnboarding(map: Map<string, Handler>): void {
  maybeSynthesizeOnboardingEvent();
  map.set('check_readiness', async (): Promise<ReadinessCheckResult> => {
    const store = getStore();
    const toggles = getActiveToggles();
    const dismissed = store.settings.install_nag_dismissed_at != null;
    return {
      checks: [],
      all_passed: true,
      critical_failures: 0,
      warnings: 0,
      umu_install_guidance: dismissed
        ? null
        : {
            install_command: 'pipx install umu-launcher',
            docs_url: 'https://github.com/Open-Wine-Components/umu-launcher',
            description:
              'Install umu-launcher on your host to enable improved Proton runtime bootstrapping for non-Steam launches.',
          },
      steam_deck_caveats:
        toggles.showSteamDeckCaveats && store.settings.steam_deck_caveats_dismissed_at == null
          ? {
              description:
                'CrossHook works on Steam Deck desktop mode today. In gaming mode you may hit these documented upstream issues on SteamOS 3.7+:',
              items: [
                'Black screen until Shader Pre-Caching completes — enable it in Steam Settings → Downloads → Shader Pre-Caching',
                'Steam overlay can render below the game under gamescope + Flatpak',
                'HDR + gamescope + Flatpak regression on SteamOS 3.7.13 (toggle HDR off if the screen tints or flickers)',
              ],
              docs_url: 'https://github.com/ValveSoftware/gamescope/issues',
            }
          : null,
    };
  });

  map.set('dismiss_onboarding', async (): Promise<null> => {
    onboardingDismissed = true;
    const store = getStore();
    store.settings.onboarding_completed = true;
    return null;
  });

  map.set('dismiss_umu_install_nag', async (): Promise<null> => {
    const store = getStore();
    store.settings.install_nag_dismissed_at = new Date().toISOString();
    return null;
  });

  map.set('dismiss_steam_deck_caveats', async (): Promise<null> => {
    getStore().settings.steam_deck_caveats_dismissed_at = new Date().toISOString();
    return null;
  });

  map.set('get_trainer_guidance', async (): Promise<TrainerGuidanceContent> => {
    return {
      loading_modes: [
        {
          id: 'source_directory',
          title: 'Source Directory',
          description: 'Proton reads the trainer directly from its downloaded location. The trainer stays in place.',
          when_to_use: 'Use when the trainer runs standalone without extra DLLs or support files.',
          examples: ['FLiNG single-file .exe trainers'],
        },
        {
          id: 'copy_to_prefix',
          title: 'Copy to Prefix',
          description:
            "CrossHook copies the trainer and support files into the WINE prefix's C:\\ drive before launch.",
          when_to_use: 'Use when the trainer bundles DLLs or support files that must be present in the WINE prefix.',
          examples: ['FLiNG trainers that bundle DLLs', 'Trainers with companion .ini or .dat files'],
        },
      ],
      trainer_sources: [
        {
          id: 'fling',
          title: 'FLiNG Trainers',
          description: 'FLiNG standalone .exe trainers — free, no account required. Primary recommendation.',
          when_to_use: 'Primary recommendation — no account needed, direct .exe download.',
          examples: ['flingtrainer.com standalone executables'],
        },
        {
          id: 'wemod',
          title: 'WeMod',
          description:
            'WeMod extracted trainers — requires a WeMod account and the WeMod desktop app installed under WINE.',
          when_to_use: 'Use only if WeMod is already set up under WINE. See wemod-launcher for setup instructions.',
          examples: ['WeMod extracted trainer DLLs'],
        },
      ],
      verification_steps: [
        'Verify the trainer .exe file exists at the configured path.',
        "Confirm the game version matches the trainer's target version.",
        'For Copy to Prefix mode: ensure companion DLLs and support files are in the same directory.',
        'Launch the game at least once to initialize the WINE prefix before using trainers.',
      ],
    };
  });
}

export { onboardingDismissed };
