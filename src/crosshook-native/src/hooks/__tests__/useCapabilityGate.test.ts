import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import * as readinessContext from '@/context/HostReadinessContext';
import { makeCapability, makeHostToolCheck, makeInstallHint } from '@/test/fixtures';
import * as clipboard from '@/utils/clipboard';
import { useCapabilityGate } from '../useCapabilityGate';

describe('useCapabilityGate', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('returns unavailable defaults when the capability is missing', () => {
    vi.spyOn(readinessContext, 'useHostReadinessContext').mockReturnValue({
      snapshot: null,
      capabilities: [],
      isStale: false,
      lastCheckedAt: null,
      isRefreshing: false,
      error: null,
      refresh: vi.fn(),
      probeTool: vi.fn(),
    });

    const { result } = renderHook(() => useCapabilityGate('missing'));

    expect(result.current.state).toBe('unavailable');
    expect(result.current.missingRequired).toEqual([]);
    expect(result.current.missingToolIds).toEqual([]);
    expect(result.current.installHint).toBeNull();
    expect(result.current.onCopyCommand).toBeUndefined();
    expect(result.current.docsUrl).toBeNull();
  });

  it('derives missing tools and prefers docs from missing required tools', () => {
    const required = makeHostToolCheck({
      tool_id: 'gamescope',
      docs_url: 'https://docs.example.invalid/gamescope',
    });
    const optional = makeHostToolCheck({
      tool_id: 'mangohud',
      is_required: false,
      docs_url: 'https://docs.example.invalid/mangohud',
    });

    vi.spyOn(readinessContext, 'useHostReadinessContext').mockReturnValue({
      snapshot: null,
      capabilities: [
        makeCapability({
          id: 'gamescope',
          missing_required: [required],
          missing_optional: [optional],
          install_hints: [makeInstallHint()],
        }),
      ],
      isStale: false,
      lastCheckedAt: null,
      isRefreshing: false,
      error: null,
      refresh: vi.fn(),
      probeTool: vi.fn(),
    });

    const { result } = renderHook(() => useCapabilityGate('gamescope'));

    expect(result.current.missingToolIds).toEqual(['gamescope', 'mangohud']);
    expect(result.current.docsUrl).toBe('https://docs.example.invalid/gamescope');
    expect(result.current.installHint?.command).toBe('sudo pacman -S gamescope');
    expect(result.current.onCopyCommand).toBeTypeOf('function');
  });

  it('copies the install command when one is present', async () => {
    const copyToClipboard = vi.spyOn(clipboard, 'copyToClipboard').mockResolvedValue();

    vi.spyOn(readinessContext, 'useHostReadinessContext').mockReturnValue({
      snapshot: null,
      capabilities: [
        makeCapability({
          id: 'gamescope',
          install_hints: [makeInstallHint({ command: 'sudo pacman -S gamescope' })],
        }),
      ],
      isStale: false,
      lastCheckedAt: null,
      isRefreshing: false,
      error: null,
      refresh: vi.fn(),
      probeTool: vi.fn(),
    });

    const { result } = renderHook(() => useCapabilityGate('gamescope'));

    await act(async () => {
      await result.current.onCopyCommand?.();
    });

    expect(copyToClipboard).toHaveBeenCalledWith('sudo pacman -S gamescope');
  });
});
