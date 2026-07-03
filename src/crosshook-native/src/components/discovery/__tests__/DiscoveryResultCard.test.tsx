import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { open as shellOpen } from '@/lib/plugin-stubs/shell';
import { axe } from '@/test/setup';
import type { CatalogEntry } from '@/types/discovery';
import { DiscoveryResultCard } from '../DiscoveryResultCard';

vi.mock('@/lib/plugin-stubs/shell', () => ({
  open: vi.fn().mockResolvedValue(undefined),
}));

const SHA256 = 'f'.repeat(32) + 'a'.repeat(32);

const writeTextMock = vi.fn().mockResolvedValue(undefined);

function buildEntry(overrides: Partial<CatalogEntry> = {}): CatalogEntry {
  return {
    id: 1,
    tapUrl: 'https://example.com/tap.git',
    tapLocalPath: '/tmp/tap',
    relativePath: 'elden/community-profile.json',
    manifestPath: '/tmp/tap/elden/community-profile.json',
    gameName: 'Elden Ring',
    trainerName: 'FLiNG Trainer',
    trainerVersion: '1.2',
    compatibilityRating: 'working',
    trainerLoadingMode: 'copy_to_prefix',
    schemaVersion: 1,
    sources: [
      {
        sourceName: 'FLiNG',
        sourceUrl: 'https://example.com/elden.exe',
        sha256: SHA256,
      },
      {
        sourceName: 'Mirror',
        sourceUrl: 'https://mirror.example.com/elden.exe',
        sha256: null,
      },
    ],
    ...overrides,
  };
}

describe('DiscoveryResultCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: writeTextMock },
      configurable: true,
    });
  });

  it('renders one source row per catalog source', () => {
    render(<DiscoveryResultCard entry={buildEntry()} onImport={vi.fn()} importing={false} />);

    expect(screen.getAllByRole('button', { name: 'Get Trainer' })).toHaveLength(2);
    expect(screen.getByText('FLiNG')).toBeInTheDocument();
    expect(screen.getByText('Mirror')).toBeInTheDocument();
  });

  it('copy button copies the full sha256 and names the game', async () => {
    const { container } = render(<DiscoveryResultCard entry={buildEntry()} onImport={vi.fn()} importing={false} />);

    // Only the source that ships a sha256 renders a copy button.
    const copy = screen.getByRole('button', { name: 'Copy SHA-256 checksum for Elden Ring' });
    await userEvent.click(copy);

    expect(writeTextMock).toHaveBeenCalledWith(SHA256);
    await waitFor(() => {
      expect(copy).toHaveTextContent('Copied');
    });

    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });

  it('get trainer opens the source url externally', async () => {
    render(<DiscoveryResultCard entry={buildEntry()} onImport={vi.fn()} importing={false} />);

    const [first, second] = screen.getAllByRole('button', { name: 'Get Trainer' });
    await userEvent.click(first);
    expect(shellOpen).toHaveBeenCalledWith('https://example.com/elden.exe');

    await userEvent.click(second);
    expect(shellOpen).toHaveBeenCalledWith('https://mirror.example.com/elden.exe');
  });

  it('import passes the entry manifestPath', async () => {
    const onImport = vi.fn();
    render(<DiscoveryResultCard entry={buildEntry()} onImport={onImport} importing={false} />);

    await userEvent.click(screen.getByRole('button', { name: 'Import Profile' }));

    expect(onImport).toHaveBeenCalledTimes(1);
    expect(onImport.mock.calls[0][0].manifestPath).toBe('/tmp/tap/elden/community-profile.json');
  });

  it('sourceless entry renders import-only', () => {
    render(<DiscoveryResultCard entry={buildEntry({ sources: [] })} onImport={vi.fn()} importing={false} />);

    expect(screen.queryByRole('button', { name: 'Get Trainer' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import Profile' })).toBeInTheDocument();
  });

  it('source-only entry without a manifest hides the import button but keeps sources', () => {
    render(
      <DiscoveryResultCard
        entry={buildEntry({ manifestPath: '', relativePath: 'trainer-sources/elden-ring' })}
        onImport={vi.fn()}
        importing={false}
      />
    );

    expect(screen.queryByRole('button', { name: /Import/ })).not.toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Get Trainer' })).toHaveLength(2);
  });

  it('shows the failure copy when the clipboard write fails', async () => {
    writeTextMock.mockRejectedValueOnce(new Error('denied'));
    render(<DiscoveryResultCard entry={buildEntry()} onImport={vi.fn()} importing={false} />);

    const copy = screen.getByRole('button', { name: 'Copy SHA-256 checksum for Elden Ring' });
    await userEvent.click(copy);

    await waitFor(() => {
      expect(copy).toHaveTextContent('Copy failed');
    });
  });

  it('expand toggle wires aria-expanded and aria-controls to the details region', async () => {
    render(<DiscoveryResultCard entry={buildEntry()} onImport={vi.fn()} importing={false} />);

    const toggle = screen.getByRole('button', { name: 'Expand details' });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    const detailsId = toggle.getAttribute('aria-controls');
    expect(detailsId).toBeTruthy();
    expect(document.getElementById(detailsId as string)).toBeNull();

    await userEvent.click(toggle);

    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(toggle).toHaveAccessibleName('Collapse details');
    const details = document.getElementById(detailsId as string);
    expect(details).not.toBeNull();
    expect(details).toHaveTextContent('FLiNG Trainer');
  });

  it('renders compatibility badge and loading mode tag', async () => {
    render(<DiscoveryResultCard entry={buildEntry()} onImport={vi.fn()} importing={false} />);

    expect(screen.getByText('Working')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Expand details' }));
    expect(screen.getByText('Loading mode: Copy into prefix')).toBeInTheDocument();
  });

  it('unknown ratings and loading modes fall back to the unknown labels', async () => {
    render(
      <DiscoveryResultCard
        entry={buildEntry({ compatibilityRating: 'garbage', trainerLoadingMode: null })}
        onImport={vi.fn()}
        importing={false}
      />
    );

    expect(screen.getByText('Unknown')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Expand details' }));
    expect(screen.getByText('Loading mode: Unknown')).toBeInTheDocument();
  });
});
