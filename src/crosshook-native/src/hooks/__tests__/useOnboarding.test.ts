import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeReadinessResult } from '@/test/fixtures';
import { configureMockHandlers, mockCallCommand } from '@/test/render';
import { useOnboarding } from '../useOnboarding';

vi.mock('@/lib/ipc', () => ({
  callCommand: mockCallCommand,
}));

describe('useOnboarding', () => {
  beforeEach(() => {
    configureMockHandlers();
  });

  it('stores readiness results after runChecks resolves', async () => {
    const readinessResult = makeReadinessResult({
      all_passed: true,
      critical_failures: 0,
    });
    configureMockHandlers({
      handlerOverrides: {
        check_generalized_readiness: async () => readinessResult,
      },
    });

    const { result } = renderHook(() => useOnboarding());

    await act(async () => {
      await result.current.runChecks();
    });

    await waitFor(() => {
      expect(result.current.readinessResult).toEqual(readinessResult);
    });
    expect(result.current.checkError).toBeNull();
    expect(result.current.lastCheckedAt).not.toBeNull();
  });

  it('skips the trainer stage for native launch methods', () => {
    const { result } = renderHook(() => useOnboarding());

    act(() => {
      result.current.advanceOrSkip('native');
    });
    expect(result.current.stage).toBe('runtime');

    act(() => {
      result.current.advanceOrSkip('native');
    });
    expect(result.current.stage).toBe('media');
  });

  it('surfaces readiness dismissal errors via checkError', async () => {
    configureMockHandlers({
      handlerOverrides: {
        dismiss_readiness_nag: async () => {
          throw new Error('mock dismiss failure');
        },
      },
    });

    const { result } = renderHook(() => useOnboarding());

    await act(async () => {
      await result.current.dismissReadinessNag('umu_run');
    });

    expect(result.current.checkError).toBe('mock dismiss failure');
  });
});
