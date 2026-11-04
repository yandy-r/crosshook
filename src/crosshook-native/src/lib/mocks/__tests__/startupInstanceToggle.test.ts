import { describe, expect, it, vi } from 'vitest';
import type { StartupReport } from '../../../types/startup';
import type { DebugToggles } from '../../toggles';

const toggles: DebugToggles = {
  delayMs: 0,
  forceErrors: false,
  showOnboarding: false,
  showSteamDeckCaveats: false,
  metadataState: 'ok',
  autoLoadProfile: null,
  showInstanceUnguarded: false,
};

vi.mock('../../toggles', () => ({
  getActiveToggles: () => toggles,
  togglesToChipFragments: () => [],
}));

async function mockReport(): Promise<StartupReport> {
  // The test setup may have loaded the handler with the real toggles; reload
  // it so it binds to the mocked module.
  vi.resetModules();
  const { registerStartup } = await import('../handlers/startup');
  const map = new Map();
  registerStartup(map);
  return map.get('get_startup_report')() as Promise<StartupReport>;
}

describe('mock startup report instance toggle', () => {
  it('reports a guarded instance by default', async () => {
    toggles.showInstanceUnguarded = false;
    expect((await mockReport()).degraded.instance_unguarded).toBe(false);
  });

  it('reports an unguarded instance with ?instanceUnguarded=show', async () => {
    toggles.showInstanceUnguarded = true;
    expect((await mockReport()).degraded.instance_unguarded).toBe(true);
  });
});
