/**
 * Tests for HeroProfileCardList and the exported buildHeroCreateSeed helper.
 *
 * Strategy: vi.mock for ProfileContext and useProfileCardMeta; OnboardingWizard
 * is replaced by a prop-capturing stub that records the latest call props so
 * each test can assert on seed, mode, and callback behaviour.
 */
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLibraryCardData, makeProfileDraft } from '@/test/fixtures';
import type { ProfileSummary } from '@/types/library';
import { buildHeroCreateSeed, HeroProfileCardList } from '../HeroProfileCardList';

// ---------------------------------------------------------------------------
// Prop-capturing wizard stub (mirrors HeroDetailProfilesTab.test.tsx idiom)
// ---------------------------------------------------------------------------

let lastWizardProps: Record<string, unknown> = {};

vi.mock('@/components/OnboardingWizard', () => ({
  OnboardingWizard: (props: Record<string, unknown>) => {
    lastWizardProps = props;
    return props['open'] ? <div role="dialog">Onboarding Wizard</div> : null;
  },
}));

// ---------------------------------------------------------------------------
// Context / hook mocks
// ---------------------------------------------------------------------------

const profileContextMock = vi.fn();
const selectProfileSpy = vi.fn();

vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));

vi.mock('@/hooks/useProfileCardMeta', () => ({
  useProfileCardMeta: () => ({
    metaByProfileName: {},
    loading: false,
  }),
}));

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const summary = makeLibraryCardData({
  name: 'Synthetic Quest',
  gameName: 'Synthetic Quest',
  steamAppId: '9999001',
  customCoverArtPath: '/media/cover.png',
  customPortraitArtPath: '/media/portrait.png',
});

const card1: ProfileSummary = {
  name: 'card1',
  gameName: 'Synthetic Quest',
  steamAppId: '9999001',
  networkIsolation: false,
};

const card2: ProfileSummary = {
  name: 'card2',
  gameName: 'Synthetic Quest',
  steamAppId: '9999001',
  networkIsolation: false,
};

function buildContextValue(overrides: { executablePath?: string; selectedProfile?: string } = {}) {
  return {
    profile: makeProfileDraft({
      game: { name: 'card1', executable_path: overrides.executablePath ?? '' },
    }),
    selectProfile: selectProfileSpy,
    selectedProfile: overrides.selectedProfile ?? 'card1',
  };
}

function renderList(
  props: Partial<React.ComponentProps<typeof HeroProfileCardList>> = {},
  contextOverrides: { executablePath?: string; selectedProfile?: string } = {}
) {
  profileContextMock.mockReturnValue(buildContextValue(contextOverrides));
  return render(
    <HeroProfileCardList
      cards={[card1, card2]}
      summary={summary}
      selectedTrimmed="card1"
      onSelectCard={vi.fn()}
      {...props}
    />
  );
}

// ---------------------------------------------------------------------------
// Setup / teardown
// ---------------------------------------------------------------------------

let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  vi.clearAllMocks();
  lastWizardProps = {};
  selectProfileSpy.mockResolvedValue(undefined);
  consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(() => {
  expect(consoleErrorSpy).not.toHaveBeenCalled();
  consoleErrorSpy.mockRestore();
});

// ---------------------------------------------------------------------------
// buildHeroCreateSeed unit tests
// ---------------------------------------------------------------------------

describe('buildHeroCreateSeed', () => {
  const baseProfile = makeProfileDraft({ game: { name: 'card1', executable_path: '/games/game.exe' } });

  it('maps gameName and numeric steamAppId', () => {
    const seed = buildHeroCreateSeed(summary, [card1], 'card1', baseProfile);
    expect(seed.gameName).toBe('Synthetic Quest');
    expect(seed.steamAppId).toBe('9999001');
  });

  it('maps cover and portrait art paths', () => {
    const seed = buildHeroCreateSeed(summary, [card1], 'card1', baseProfile);
    expect(seed.coverArtPath).toBe('/media/cover.png');
    expect(seed.portraitArtPath).toBe('/media/portrait.png');
  });

  it('includes executablePath when selectedTrimmed matches a card', () => {
    const seed = buildHeroCreateSeed(summary, [card1, card2], 'card1', baseProfile);
    expect(seed.executablePath).toBe('/games/game.exe');
  });

  it('omits executablePath when context profile name does not match selectedTrimmed', () => {
    const staleContext = makeProfileDraft({
      game: { name: 'card2', executable_path: '/games/other.exe' },
    });
    const seed = buildHeroCreateSeed(summary, [card1, card2], 'card1', staleContext);
    expect(seed.executablePath).toBeUndefined();
  });

  it('omits executablePath when selectedTrimmed does not match any card', () => {
    const seed = buildHeroCreateSeed(summary, [card1, card2], 'unknown-profile', baseProfile);
    expect(seed.executablePath).toBeUndefined();
  });

  it('omits executablePath when selectedTrimmed is empty', () => {
    const seed = buildHeroCreateSeed(summary, [card1, card2], '', baseProfile);
    expect(seed.executablePath).toBeUndefined();
  });

  it('omits steamAppId when non-numeric', () => {
    const nonNumericSummary = makeLibraryCardData({ steamAppId: 'not-a-number' });
    const seed = buildHeroCreateSeed(nonNumericSummary, [card1], 'card1', baseProfile);
    expect(seed.steamAppId).toBeUndefined();
  });

  it('omits gameName when blank', () => {
    const blankNameSummary = makeLibraryCardData({ gameName: '' });
    const seed = buildHeroCreateSeed(blankNameSummary, [card1], 'card1', baseProfile);
    expect(seed.gameName).toBeUndefined();
  });

  it('omits coverArtPath when undefined', () => {
    const noCoverSummary = makeLibraryCardData({ customCoverArtPath: undefined });
    const seed = buildHeroCreateSeed(noCoverSummary, [card1], 'card1', baseProfile);
    expect(seed.coverArtPath).toBeUndefined();
  });

  it('omits portraitArtPath when undefined', () => {
    const noPortraitSummary = makeLibraryCardData({ customPortraitArtPath: undefined });
    const seed = buildHeroCreateSeed(noPortraitSummary, [card1], 'card1', baseProfile);
    expect(seed.portraitArtPath).toBeUndefined();
  });

  it('omits coverArtPath when empty string', () => {
    const noCoverSummary = makeLibraryCardData({ customCoverArtPath: '' });
    const seed = buildHeroCreateSeed(noCoverSummary, [card1], 'card1', baseProfile);
    expect(seed.coverArtPath).toBeUndefined();
  });

  it('never sets suggestedName', () => {
    const seed = buildHeroCreateSeed(summary, [card1], 'card1', baseProfile);
    expect(seed.suggestedName).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// HeroProfileCardList integration tests
// ---------------------------------------------------------------------------

describe('HeroProfileCardList', () => {
  it('renders card list and "+ New" button when cards are present', () => {
    renderList();
    expect(screen.getByRole('list', { name: 'Profile cards' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '+ New' })).toBeInTheDocument();
  });

  it('opens wizard with mode="create" and correct seed when "+ New" is clicked', async () => {
    const user = userEvent.setup();
    renderList({}, { executablePath: '/games/game.exe' });

    await user.click(screen.getByRole('button', { name: '+ New' }));

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(lastWizardProps['mode']).toBe('create');
    expect(lastWizardProps['open']).toBe(true);

    const seed = lastWizardProps['createSeed'] as Record<string, unknown>;
    expect(seed.gameName).toBe('Synthetic Quest');
    expect(seed.steamAppId).toBe('9999001');
    expect(seed.coverArtPath).toBe('/media/cover.png');
    expect(seed.portraitArtPath).toBe('/media/portrait.png');
  });

  it("seeds executablePath when selectedTrimmed is one of this game's cards", async () => {
    const user = userEvent.setup();
    renderList({ selectedTrimmed: 'card1' }, { executablePath: '/games/game.exe' });

    await user.click(screen.getByRole('button', { name: '+ New' }));

    const seed = lastWizardProps['createSeed'] as Record<string, unknown>;
    expect(seed.executablePath).toBe('/games/game.exe');
  });

  it("does not seed executablePath when selectedTrimmed is not one of this game's cards", async () => {
    const user = userEvent.setup();
    renderList({ selectedTrimmed: 'other-profile' }, { executablePath: '/games/game.exe' });

    await user.click(screen.getByRole('button', { name: '+ New' }));

    const seed = lastWizardProps['createSeed'] as Record<string, unknown>;
    expect(seed.executablePath).toBeUndefined();
  });

  it('omits steamAppId from seed when non-numeric', async () => {
    const user = userEvent.setup();
    const nonNumericSummary = makeLibraryCardData({ ...summary, steamAppId: 'non-numeric' });
    renderList({ summary: nonNumericSummary });

    await user.click(screen.getByRole('button', { name: '+ New' }));

    const seed = lastWizardProps['createSeed'] as Record<string, unknown>;
    expect(seed.steamAppId).toBeUndefined();
  });

  it('closes wizard and calls selectProfile when onComplete fires with a name', async () => {
    const user = userEvent.setup();
    renderList();

    await user.click(screen.getByRole('button', { name: '+ New' }));
    expect(screen.getByRole('dialog')).toBeInTheDocument();

    const onComplete = lastWizardProps['onComplete'] as (name?: string) => void;
    act(() => {
      onComplete('NewProfile');
    });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(selectProfileSpy).toHaveBeenCalledWith('NewProfile');
  });

  it('closes wizard but does not call selectProfile when onComplete fires without a name', async () => {
    const user = userEvent.setup();
    renderList();

    await user.click(screen.getByRole('button', { name: '+ New' }));

    const onComplete = lastWizardProps['onComplete'] as (name?: string) => void;
    act(() => {
      onComplete(undefined);
    });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(selectProfileSpy).not.toHaveBeenCalled();
  });

  it('closes wizard and restores prior selection when onDismiss fires with selectedTrimmed set', async () => {
    const user = userEvent.setup();
    renderList({ selectedTrimmed: 'card2' });

    await user.click(screen.getByRole('button', { name: '+ New' }));

    const onDismiss = lastWizardProps['onDismiss'] as () => void;
    act(() => {
      onDismiss();
    });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(selectProfileSpy).toHaveBeenCalledWith('card2');
  });

  it('closes wizard but does NOT call selectProfile when onDismiss fires with empty selectedTrimmed', async () => {
    const user = userEvent.setup();
    renderList({ selectedTrimmed: '' });

    await user.click(screen.getByRole('button', { name: '+ New' }));

    const onDismiss = lastWizardProps['onDismiss'] as () => void;
    act(() => {
      onDismiss();
    });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(selectProfileSpy).not.toHaveBeenCalled();
  });

  // -------------------------------------------------------------------------
  // Empty-state CTA
  // -------------------------------------------------------------------------

  it('renders empty-state panel with "Create profile" CTA when no cards exist', () => {
    renderList({ cards: [] });

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.getByText('No profiles found for this game.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create profile' })).toBeInTheDocument();
  });

  it('opening wizard via empty-state "Create profile" CTA shows the wizard', async () => {
    const user = userEvent.setup();
    renderList({ cards: [] });

    await user.click(screen.getByRole('button', { name: 'Create profile' }));

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(lastWizardProps['mode']).toBe('create');
  });

  it('does not render "Create profile" CTA when cards are present', () => {
    renderList();
    expect(screen.queryByRole('button', { name: 'Create profile' })).not.toBeInTheDocument();
  });
});
