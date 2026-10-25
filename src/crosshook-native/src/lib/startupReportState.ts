// Module state for `startupReport.ts`, kept in its own ipc-free module so the mock
// environment reset (and the Vitest setup) can clear it without importing `./ipc`
// before a test's `vi.mock('@/lib/ipc')` registration.
import type { StartupReport } from '../types/startup';

export type StartupAction = 'auto_load_profile' | 'onboarding_prompt' | 'flatpak_migration_notice';

export const startupReportState: {
  memoized: Promise<StartupReport> | null;
  claimed: Set<StartupAction>;
} = {
  memoized: null,
  claimed: new Set<StartupAction>(),
};

export function resetStartupReportForTests(): void {
  startupReportState.memoized = null;
  startupReportState.claimed.clear();
}
