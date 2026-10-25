import type { StartupReport } from '../../../types/startup';
import { getActiveFixture } from '../../fixture';
import { getActiveToggles } from '../../toggles';
import { getStore } from '../store';
import { buildMockHealthSummary, buildMockVersionScanSummary } from './health';
import { seedDemoProfiles } from './profile-core';
import type { Handler } from './types';

export function registerStartup(map: Map<string, Handler>): void {
  map.set('get_startup_report', async (): Promise<StartupReport> => {
    const store = getStore();
    const settings = store.settings;
    const lastUsed = settings.last_used_profile?.trim() ?? '';
    const { metadataState, showOnboarding, autoLoadProfile: autoLoadToggle } = getActiveToggles();
    const configuredAutoLoad = autoLoadToggle ?? (settings.auto_load_last_profile ? lastUsed : '');
    // Resolving an auto-load target needs the demo library (same seeding rule as `profile_list`).
    if (configuredAutoLoad !== '' && getActiveFixture() !== 'empty') seedDemoProfiles();
    const autoLoadProfile =
      configuredAutoLoad !== '' && store.profiles.has(configuredAutoLoad) ? configuredAutoLoad : null;
    const metadata =
      metadataState === 'newer'
        ? { state: 'newer_schema' as const, found: 99, supported: 27 }
        : metadataState === 'disabled'
          ? { state: 'disabled' as const, reason: '[dev-mock] metadata database could not be opened' }
          : { state: 'ok' as const };
    // The demo library is seeded lazily by `profile_list`; report it as present unless the `empty` fixture.
    const hasProfiles = store.profiles.size > 0 || getActiveFixture() !== 'empty';

    return {
      auto_load_profile: autoLoadProfile,
      onboarding: {
        show: !settings.onboarding_completed || showOnboarding,
        // `?onboarding=show` previews the wizard, which only opens for an empty library.
        has_profiles: !showOnboarding && hasProfiles,
      },
      flatpak: { mode: 'native', migration: null },
      degraded: {
        metadata,
        metadata_error: metadataState === 'disabled' ? '[dev-mock] metadata database could not be opened' : null,
        settings_unreadable: false,
        logging_unavailable: false,
      },
      jobs: {
        health: { state: 'complete', data: buildMockHealthSummary() },
        offline_readiness: { state: 'complete', data: { total_profiles: store.profiles.size } },
        version_scan: { state: 'complete', data: buildMockVersionScanSummary() },
        umu_refresh: {
          state: 'complete',
          data: { refreshed: false, cached_at: null, reason: '' },
        },
        background_portal: { state: 'complete', data: 'not_applicable' },
      },
    };
  });
}
