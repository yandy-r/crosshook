import { screen, waitFor } from '@testing-library/react';
import type { PanelImperativeHandle } from 'react-resizable-panels';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { startReplayProbe } from '@/lib/bench/replayProbe';
import { renderWithMocks } from '@/test/render';
import { ConsoleDrawer } from '../ConsoleDrawer';

type BenchCall = { name: string; args?: Record<string, unknown> };

const benchCalls: BenchCall[] = [];

// Only the bench commands are recorded; every other command still reaches the real IPC layer, so the
// providers around the real drawer behave as they do in the other ConsoleDrawer tests.
vi.mock('@/lib/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/ipc')>();
  return {
    ...actual,
    callCommand: (name: string, args?: Record<string, unknown>) => {
      if (name.startsWith('bench_')) {
        benchCalls.push({ name, args });
        return Promise.resolve(undefined);
      }
      return (actual.callCommand as (n: string, a?: Record<string, unknown>) => Promise<unknown>)(name, args);
    },
  };
});

const benchNames = () => benchCalls.map((call) => call.name);
const benchMarkPhases = () =>
  benchCalls.filter((call) => call.name === 'bench_mark').map((call) => (call.args as { phase: string }).phase);

function renderDrawer(defaultCollapsed: boolean) {
  const panelRef = { current: { collapse: vi.fn(), expand: vi.fn() } as unknown as PanelImperativeHandle };
  renderWithMocks(
    <HostReadinessProvider>
      <ConsoleDrawer panelRef={panelRef} mode="drawer" defaultCollapsed={defaultCollapsed} />
    </HostReadinessProvider>
  );
}

describe('startReplayProbe against the real ConsoleDrawer', () => {
  beforeEach(() => {
    benchCalls.length = 0;
  });

  it('reports console_hidden and never begins while the drawer is collapsed', async () => {
    renderDrawer(true);
    expect(screen.getByTestId('console-drawer')).toHaveClass('crosshook-console-drawer--collapsed');

    await startReplayProbe({ timeoutMs: 50 });

    expect(benchMarkPhases()).toEqual(['console_hidden']);
    expect(benchNames()).not.toContain('bench_begin');
  });

  it('reports the follow state and calls bench_begin once the drawer is expanded', async () => {
    renderDrawer(false);
    const drawer = screen.getByTestId('console-drawer');
    expect(drawer).not.toHaveClass('crosshook-console-drawer--collapsed');
    await waitFor(() => expect(drawer.querySelector('.crosshook-console__body')).not.toBeNull());

    await startReplayProbe({ timeoutMs: 1_000 });

    expect(benchMarkPhases()).toEqual(['follow_on']);
    expect(benchNames()).toEqual(['bench_mark', 'bench_begin']);
  });
});
