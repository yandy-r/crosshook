import type { StartupReport } from '@/types/startup';

export function makeStartupReport(overrides: Partial<StartupReport> = {}): StartupReport {
  return {
    auto_load_profile: null,
    onboarding: { show: false, has_profiles: true },
    flatpak: { mode: 'native', migration: null },
    degraded: {
      metadata: { state: 'ok' },
      metadata_error: null,
      settings_unreadable: false,
      logging_unavailable: false,
    },
    jobs: {
      health: { state: 'pending' },
      offline_readiness: { state: 'pending' },
      version_scan: { state: 'pending' },
      umu_refresh: { state: 'pending' },
      background_portal: { state: 'pending' },
    },
    ...overrides,
  };
}

