import { render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_APP_SETTINGS } from '@/types/settings';
import { ProtonManagerDefaultsSection } from '../ProtonManagerDefaultsSection';

// Tauri v2 maps the Rust `steam_client_install_path` param to the top-level
// invoke key `steamClientInstallPath`; a snake_case key is dropped and the
// override path is always None.
const callCommandMock = vi.fn();
vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

const STEAM_PATH = '/home/user/.local/share/Steam';

describe('ProtonManagerDefaultsSection IPC contract', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    callCommandMock.mockResolvedValue([]);
  });

  it('resolves install roots with a camelCase steamClientInstallPath key', async () => {
    render(
      <ProtonManagerDefaultsSection
        settings={DEFAULT_APP_SETTINGS}
        steamClientInstallPath={STEAM_PATH}
        onPersistSettings={vi.fn().mockResolvedValue(undefined)}
      />
    );

    await waitFor(() =>
      expect(callCommandMock).toHaveBeenCalledWith('protonup_resolve_install_roots', expect.anything())
    );
    const call = callCommandMock.mock.calls.find(([name]) => name === 'protonup_resolve_install_roots');
    const args = call?.[1] as Record<string, unknown>;
    expect(args).toHaveProperty('steamClientInstallPath', STEAM_PATH);
    expect(args).not.toHaveProperty('steam_client_install_path');
  });
});
