import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { makeLibraryCardData } from '@/test/fixtures';
import { HeroDetailHeader } from '../HeroDetailHeader';

type HeroDetailHeaderProps = ComponentProps<typeof HeroDetailHeader>;

function renderHeader(overrides: Partial<HeroDetailHeaderProps> = {}) {
  const summary = makeLibraryCardData({ name: 'Synthetic Quest', gameName: 'Synthetic Quest' });
  return render(
    <HeroDetailHeader
      summary={summary}
      displayName={summary.gameName ?? summary.name}
      profile={null}
      loadState="idle"
      profileError={null}
      methodLabel={null}
      heroResolved={{ url: null, showSkeleton: false }}
      portraitArt={{ coverArtUrl: null, loading: false }}
      heroImgBroken={false}
      setHeroImgBroken={vi.fn()}
      portraitImgBroken={false}
      setPortraitImgBroken={vi.fn()}
      onBack={vi.fn()}
      onLaunch={vi.fn()}
      onEdit={vi.fn()}
      onToggleFavorite={vi.fn()}
      {...overrides}
    />
  );
}

describe('HeroDetailHeader', () => {
  it('renders breadcrumb nav with Library crumb as a button', () => {
    renderHeader();
    const nav = screen.getByRole('navigation', { name: 'Breadcrumb' });
    expect(nav).toBeInTheDocument();
    const libraryButton = within(nav).getByRole('button', { name: 'Library' });
    expect(libraryButton).toBeInTheDocument();
  });

  it('clicking Library crumb fires onBack exactly once', async () => {
    const user = userEvent.setup();
    const onBack = vi.fn();
    renderHeader({ onBack });

    const nav = screen.getByRole('navigation', { name: 'Breadcrumb' });
    const libraryButton = within(nav).getByRole('button', { name: 'Library' });
    await user.click(libraryButton);

    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('game displayName is the terminal segment with aria-current="page" and is not a button', () => {
    renderHeader({ displayName: 'Synthetic Quest' });

    const nav = screen.getByRole('navigation', { name: 'Breadcrumb' });
    const currentSegment = within(nav).getByText('Synthetic Quest');
    expect(currentSegment).toHaveAttribute('aria-current', 'page');
    expect(currentSegment.tagName.toLowerCase()).not.toBe('button');
  });

  it('standalone Back button renders and fires onBack when clicked', async () => {
    const user = userEvent.setup();
    const onBack = vi.fn();
    renderHeader({ onBack });

    const backButton = screen.getByRole('button', { name: 'Back' });
    expect(backButton).toBeInTheDocument();
    await user.click(backButton);

    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
