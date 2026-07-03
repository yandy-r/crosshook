import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { EnvGroup } from '@/utils/derivePipelineDetail';
import { EnvBreakdown } from '../EnvBreakdown';

const copyToClipboardMock = vi.fn();
vi.mock('@/utils/clipboard', () => ({
  copyToClipboard: (text: string) => copyToClipboardMock(text),
}));

const protonGroup: EnvGroup = {
  id: 'proton_runtime',
  label: 'Proton runtime',
  vars: [
    { key: 'WINEPREFIX', value: '/prefixes/synthetic-quest' },
    { key: 'PROTON_VERB', value: 'waitforexitandrun' },
  ],
  cleared: false,
};

const clearedGroup: EnvGroup = {
  id: 'cleared',
  label: 'Cleared before launch',
  vars: [
    { key: 'WINEDLLOVERRIDES', value: null },
    { key: 'WINEESYNC', value: null },
  ],
  cleared: true,
};

describe('EnvBreakdown', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    copyToClipboardMock.mockResolvedValue(undefined);
  });

  it('copies KEY=value lines for a regular group', async () => {
    const user = userEvent.setup();
    render(<EnvBreakdown groups={[protonGroup, clearedGroup]} />);

    await user.click(screen.getAllByRole('button', { name: 'Copy' })[0]);

    expect(copyToClipboardMock).toHaveBeenCalledWith(
      'WINEPREFIX=/prefixes/synthetic-quest\nPROTON_VERB=waitforexitandrun'
    );
  });

  it('copies unset KEY lines for the cleared group', async () => {
    const user = userEvent.setup();
    render(<EnvBreakdown groups={[protonGroup, clearedGroup]} />);

    await user.click(screen.getAllByRole('button', { name: 'Copy' })[1]);

    expect(copyToClipboardMock).toHaveBeenCalledWith('unset WINEDLLOVERRIDES\nunset WINEESYNC');
  });

  it('announces the group name in the role=status line after copy', async () => {
    const user = userEvent.setup();
    render(<EnvBreakdown groups={[protonGroup, clearedGroup]} />);

    await user.click(screen.getAllByRole('button', { name: 'Copy' })[0]);

    expect(await screen.findByText('Copied 2 Proton runtime variables.')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Copied 2 Proton runtime variables.');
  });

  it('announces Failed to copy when the clipboard rejects', async () => {
    copyToClipboardMock.mockRejectedValueOnce(new Error('denied'));
    const user = userEvent.setup();
    render(<EnvBreakdown groups={[protonGroup]} />);

    await user.click(screen.getByRole('button', { name: 'Copy' }));

    expect(await screen.findByText('Failed to copy.')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Failed to copy.');
  });

  it('cleared rows render a visible unset badge', () => {
    render(<EnvBreakdown groups={[protonGroup, clearedGroup]} />);

    expect(screen.getAllByText('unset')).toHaveLength(2);
  });

  it('renders the empty message when there are no groups', () => {
    render(<EnvBreakdown groups={[]} />);

    expect(screen.getByText('No environment variables.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Copy' })).not.toBeInTheDocument();
  });
});
