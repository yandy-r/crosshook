import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { callCommand } from '@/lib/ipc';
import { usePrefixDeps } from '../usePrefixDeps';

vi.mock('@/lib/ipc', () => ({
  callCommand: vi.fn(),
}));

const callCommandMock = vi.mocked(callCommand);

describe('usePrefixDeps', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    callCommandMock.mockResolvedValue([]);
  });

  it('reload fetches dependency status again', async () => {
    const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));
    await waitFor(() => expect(callCommandMock).toHaveBeenCalledTimes(1));

    act(() => result.current.reload());

    await waitFor(() => expect(callCommandMock).toHaveBeenCalledTimes(2));
  });
});
