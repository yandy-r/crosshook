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
    expect(result.current.stage).toBe('complete');

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
});
