import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { RunExecutableResult } from '@/types/run-executable';
import { RUN_LOG_LINE_CAP, type UseRunExecutableResult, useRunExecutable } from '../useRunExecutable';

type EventHandler = (event: { event: string; id: number; payload: unknown }) => void;

const handlers = new Map<string, EventHandler>();
const callCommandMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

vi.mock('@/lib/events', () => ({
  subscribeEvent: vi.fn((name: string, handler: EventHandler): Promise<() => void> => {
    handlers.set(name, handler);
    return Promise.resolve(() => {
      handlers.delete(name);
    });
  }),
}));

const RUN_ID = 'run-42';

const RUN_RESULT: RunExecutableResult = {
  succeeded: true,
  message: 'Started setup.exe.',
  helper_log_path: '/mock/logs/run-executable-42.log',
  resolved_prefix_path: '/mock/prefixes/_run-adhoc/setup',
  run_id: RUN_ID,
};

function emit(name: string, payload: unknown): void {
  const handler = handlers.get(name);
  if (!handler) {
    throw new Error(`no listener for ${name}`);
  }
  handler({ event: name, id: 0, payload });
}

/** `run_executable` stays pending until the returned resolver is called. */
function deferRunExecutable(): (result: RunExecutableResult) => void {
  const gate = Promise.withResolvers<RunExecutableResult>();
  callCommandMock.mockImplementation((name: string) =>
    name === 'run_executable' ? gate.promise : Promise.resolve(undefined)
  );
  return gate.resolve;
}

/** Renders the hook and waits for its listeners; every rendered stage is appended to `stages`. */
async function renderRunHook(stages: string[] = []) {
  const rendered = renderHook(() => {
    const state = useRunExecutable();
    stages.push(state.stage);
    return state;
  });
  await waitFor(() => {
    expect(handlers.has('run-executable-log')).toBe(true);
    expect(handlers.has('run-executable-complete')).toBe(true);
  });
  return rendered;
}

/** Starts a run whose `run_executable` invoke resolves immediately with RUN_RESULT. */
async function startResolvedRun(result: { current: UseRunExecutableResult }): Promise<void> {
  callCommandMock.mockImplementation((name: string) =>
    Promise.resolve(name === 'run_executable' ? RUN_RESULT : undefined)
  );
  await act(async () => {
    await result.current.startRun();
  });
}

describe('useRunExecutable', () => {
  beforeEach(() => {
    handlers.clear();
    callCommandMock.mockReset();
  });

  it('keeps the terminal stage and result when completion arrives before the invoke resolves', async () => {
    const stages: string[] = [];
    const { result } = await renderRunHook(stages);
    const resolveRun = deferRunExecutable();

    let startPromise: Promise<void> = Promise.resolve();
    act(() => {
      startPromise = result.current.startRun();
    });
    await waitFor(() => expect(callCommandMock).toHaveBeenCalledWith('run_executable', expect.anything()));

    act(() => {
      emit('run-executable-complete', { run_id: RUN_ID, code: 0, signal: null });
    });
    // Buffered until run_executable returns this run's id.
    expect(result.current.stage).toBe('preparing');

    await act(async () => {
      resolveRun(RUN_RESULT);
      await startPromise;
    });

    expect(result.current.stage).toBe('complete');
    expect(stages).not.toContain('running');
    expect(result.current.result).toEqual(RUN_RESULT);
    expect(result.current.error).toBeNull();
    expect(result.current.isRunning).toBe(false);
  });

  it('moves from running to complete in the normal event order', async () => {
    const { result } = await renderRunHook();
    await startResolvedRun(result);

    expect(callCommandMock).toHaveBeenCalledWith('validate_run_executable_request', expect.anything());
    expect(result.current.stage).toBe('running');
    expect(result.current.result).toEqual(RUN_RESULT);

    act(() => {
      emit('run-executable-complete', { run_id: RUN_ID, code: 0, signal: null });
    });

    expect(result.current.stage).toBe('complete');
    expect(result.current.statusText).toBe(RUN_RESULT.message);
  });

  it('accumulates lines of its own run, caps the buffer, and ignores a foreign run_id', async () => {
    const { result } = await renderRunHook();
    await startResolvedRun(result);

    act(() => {
      emit('run-executable-log', { run_id: 'someone-else', line: 'foreign line' });
      for (let index = 0; index < RUN_LOG_LINE_CAP + 5; index += 1) {
        emit('run-executable-log', { run_id: RUN_ID, line: `line ${index}` });
      }
      emit('run-executable-complete', { run_id: 'someone-else', code: 1, signal: null });
    });

    expect(result.current.logLines).toHaveLength(RUN_LOG_LINE_CAP);
    expect(result.current.logLines[0]).toBe('line 5');
    expect(result.current.logLines.at(-1)).toBe(`line ${RUN_LOG_LINE_CAP + 4}`);
    expect(result.current.logLines).not.toContain('foreign line');
    expect(result.current.stage).toBe('running');
  });

  it('fails with the signal when the process is killed', async () => {
    const { result } = await renderRunHook();
    await startResolvedRun(result);

    act(() => {
      emit('run-executable-complete', { run_id: RUN_ID, code: null, signal: 15 });
    });

    expect(result.current.stage).toBe('failed');
    expect(result.current.error).toBe('Run process was terminated by signal 15.');
  });

  it('reset clears log lines, result and stage', async () => {
    const { result } = await renderRunHook();
    await startResolvedRun(result);

    act(() => {
      emit('run-executable-log', { run_id: RUN_ID, line: 'hello from wine' });
      emit('run-executable-complete', { run_id: RUN_ID, code: 0, signal: null });
    });
    expect(result.current.logLines).toEqual(['hello from wine']);

    act(() => {
      result.current.reset();
    });

    expect(result.current.logLines).toEqual([]);
    expect(result.current.result).toBeNull();
    expect(result.current.stage).toBe('idle');
    expect(callCommandMock).not.toHaveBeenCalledWith('cancel_run_executable', expect.anything());

    act(() => {
      emit('run-executable-log', { run_id: RUN_ID, line: 'late line' });
    });
    expect(result.current.logLines).toEqual([]);
  });

  it('replays only its own buffered events from the starting window and drops another run', async () => {
    const stages: string[] = [];
    const { result } = await renderRunHook(stages);
    const resolveRun = deferRunExecutable();

    let startPromise: Promise<void> = Promise.resolve();
    act(() => {
      startPromise = result.current.startRun();
    });
    await waitFor(() => expect(callCommandMock).toHaveBeenCalledWith('run_executable', expect.anything()));

    act(() => {
      // A late event of an earlier (reset) run arrives while this run starts.
      emit('run-executable-log', { run_id: 'run-old', line: 'old line' });
      emit('run-executable-complete', { run_id: 'run-old', code: 1, signal: null });
      emit('run-executable-log', { run_id: RUN_ID, line: 'own line' });
    });
    expect(result.current.stage).toBe('preparing');
    expect(result.current.logLines).toEqual([]);

    await act(async () => {
      resolveRun(RUN_RESULT);
      await startPromise;
    });

    expect(result.current.stage).toBe('running');
    expect(result.current.error).toBeNull();
    expect(result.current.isRunning).toBe(true);
    expect(result.current.logLines).toEqual(['own line']);
    expect(stages).not.toContain('failed');
  });

  it('reports a non-busy runtime error from the backend as a run error', async () => {
    const { result } = await renderRunHook();
    callCommandMock.mockImplementation((name: string) =>
      name === 'run_executable' ? Promise.reject({ kind: 'runtime', message: 'boom' }) : Promise.resolve(undefined)
    );

    await act(async () => {
      await result.current.startRun();
    });

    expect(result.current.stage).toBe('failed');
    expect(result.current.error).toBe('boom');
    expect(result.current.canCancel).toBe(false);
  });

  it("keeps Cancel and Stop usable for the window's earlier run after a remount", async () => {
    // The first mount starts a run, then the panel unmounts (route change)
    // while the backend still holds the window's run slot.
    const first = await renderRunHook();
    await startResolvedRun(first.result);
    expect(first.result.current.stage).toBe('running');
    first.unmount();

    const { result } = await renderRunHook();
    let slotHeld = true;
    callCommandMock.mockImplementation((name: string) => {
      if (name === 'run_executable') {
        return slotHeld
          ? Promise.reject({ kind: 'runtime', message: 'another run is already active for this window' })
          : Promise.resolve({ ...RUN_RESULT, run_id: 'run-43' });
      }
      if (name === 'cancel_run_executable') slotHeld = false;
      return Promise.resolve(undefined);
    });
    act(() => {
      result.current.updateField('executable_path', '/mock/setup.exe');
      result.current.updateField('proton_path', '/mock/proton');
    });

    await act(async () => {
      await result.current.startRun();
    });
    expect(result.current.stage).toBe('busy');
    expect(result.current.error).toBeNull();
    expect(result.current.canCancel).toBe(true);
    expect(result.current.canStart).toBe(false);
    expect(result.current.statusText).toMatch(/started earlier in this window is still active/);

    await act(async () => {
      await result.current.cancelRun();
    });
    expect(callCommandMock).toHaveBeenCalledWith('cancel_run_executable', undefined);
    expect(result.current.stage).toBe('idle');
    expect(result.current.canStart).toBe(true);

    await act(async () => {
      await result.current.startRun();
    });
    expect(result.current.stage).toBe('running');
    expect(result.current.result?.run_id).toBe('run-43');
  });

  it("lets Stop end the window's earlier run after a busy start", async () => {
    const { result } = await renderRunHook();
    callCommandMock.mockImplementation((name: string) =>
      name === 'run_executable'
        ? Promise.reject({ kind: 'runtime', message: 'another run is already active for this window' })
        : Promise.resolve(undefined)
    );

    await act(async () => {
      await result.current.startRun();
    });
    expect(result.current.canCancel).toBe(true);

    await act(async () => {
      await result.current.stopRun();
    });
    expect(callCommandMock).toHaveBeenCalledWith('stop_run_executable', undefined);
    expect(result.current.stage).toBe('idle');
  });

  it('detaches and cancels a start that was reset while run_executable was in flight', async () => {
    const { result } = await renderRunHook();
    const resolveRun = deferRunExecutable();

    let startPromise: Promise<void> = Promise.resolve();
    act(() => {
      startPromise = result.current.startRun();
    });
    await waitFor(() => expect(callCommandMock).toHaveBeenCalledWith('run_executable', expect.anything()));

    act(() => {
      result.current.reset();
    });
    expect(callCommandMock).toHaveBeenCalledWith('cancel_run_executable', undefined);
    const cancelsAfterReset = callCommandMock.mock.calls.filter(([name]) => name === 'cancel_run_executable').length;

    await act(async () => {
      resolveRun(RUN_RESULT);
      await startPromise;
    });

    // The detached start cancels the run it started instead of tracking it.
    expect(callCommandMock.mock.calls.filter(([name]) => name === 'cancel_run_executable')).toHaveLength(
      cancelsAfterReset + 1
    );
    expect(result.current.stage).toBe('idle');
    expect(result.current.result).toBeNull();

    act(() => {
      emit('run-executable-log', { run_id: RUN_ID, line: 'stray line' });
      emit('run-executable-complete', { run_id: RUN_ID, code: 0, signal: null });
    });
    expect(result.current.logLines).toEqual([]);
    expect(result.current.stage).toBe('idle');
  });

  it('does not cancel after a detached start whose run already completed', async () => {
    const { result } = await renderRunHook();
    const resolveRun = deferRunExecutable();

    let startPromise: Promise<void> = Promise.resolve();
    act(() => {
      startPromise = result.current.startRun();
    });
    await waitFor(() => expect(callCommandMock).toHaveBeenCalledWith('run_executable', expect.anything()));

    act(() => {
      emit('run-executable-complete', { run_id: RUN_ID, code: 0, signal: null });
      result.current.reset();
    });
    const cancelsAfterReset = callCommandMock.mock.calls.filter(([name]) => name === 'cancel_run_executable').length;

    await act(async () => {
      resolveRun(RUN_RESULT);
      await startPromise;
    });

    expect(callCommandMock.mock.calls.filter(([name]) => name === 'cancel_run_executable')).toHaveLength(
      cancelsAfterReset
    );
    expect(result.current.stage).toBe('idle');
  });
});
