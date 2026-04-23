import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { makeLibraryCardData } from '@/test/fixtures';
import { type MockRenderOptions, renderWithMocks } from '@/test/render';
import { GameDetail } from '../GameDetail';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

type GameDetailProps = ComponentProps<typeof GameDetail>;

function renderGameDetail(
  props: Partial<GameDetailProps> & Pick<GameDetailProps, 'summary'>,
  options: MockRenderOptions = {}
) {
  return renderWithMocks(
    <ProfileProvider>
      <PreferencesProvider>
        <GameDetail
          onBack={vi.fn()}
          healthByName={{}}
          healthLoading={false}
          offlineReportFor={() => undefined}
          offlineError={null}
          onLaunch={vi.fn()}
          onEdit={vi.fn()}
          onToggleFavorite={vi.fn()}
          {...props}
        />
      </PreferencesProvider>
    </ProfileProvider>,
    options
  );
}

describe('GameDetail', () => {
  it('renders hero detail shell, Back, and default tab', async () => {
    renderGameDetail({ summary: makeLibraryCardData({ name: 'Synthetic Quest', gameName: 'Synthetic Quest' }) });

    expect(screen.getByTestId('game-detail')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Back' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Overview' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Synthetic Quest', level: 2 })).toBeInTheDocument();
  });

  it('calls onBack when Back is pressed', async () => {
    const user = userEvent.setup();
    const onBack = vi.fn();
    renderGameDetail({ summary: makeLibraryCardData(), onBack });

    await user.click(screen.getByRole('button', { name: 'Back' }));
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('switches tabs', async () => {
    const user = userEvent.setup();
    renderGameDetail({ summary: makeLibraryCardData() });

    const historyTab = screen.getByRole('tab', { name: 'History' });
    await user.click(historyTab);
    expect(historyTab).toHaveAttribute('aria-selected', 'true');
  });
});
