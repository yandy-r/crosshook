import { useMemo, useState } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import { useProfileCardMeta } from '@/hooks/useProfileCardMeta';
import type { EnrichedProfileHealthReport } from '@/types/health';
import type { LibraryCardData, ProfileSummary } from '@/types/library';
import type { GameProfile } from '@/types/profile';
import { HealthBadge } from '../../HealthBadge';
import { OnboardingWizard } from '../../OnboardingWizard';
import type { ProfileCreateSeed } from '../../wizard/profileCreateSeed';

const NUMERIC_APP_ID_RE = /^\d{1,12}$/;

export interface HeroProfileCardListProps {
  cards: ProfileSummary[];
  summary: LibraryCardData;
  selectedTrimmed: string;
  healthByName?: Partial<Record<string, EnrichedProfileHealthReport>>;
  onSelectCard: (cardName: string) => void;
}

function profileCardTitle(card: ProfileSummary): string {
  return card.gameName.trim() ? `${card.name} - ${card.gameName}` : card.name;
}

function profileCardMetaLabel(profileName: string, protonLabel: string | null): string {
  return [profileName ? `${profileName}.toml` : null, protonLabel].filter(Boolean).join(' · ');
}

/**
 * Builds a ProfileCreateSeed from the hero-detail context. Pure function — safe
 * to call in useMemo. The executablePath is only populated when the context
 * profile is one of the cards listed for this game (singleton ownership).
 */
export function buildHeroCreateSeed(
  summary: LibraryCardData,
  cards: ProfileSummary[],
  selectedTrimmed: string,
  contextProfile: GameProfile
): ProfileCreateSeed {
  const seed: ProfileCreateSeed = {};

  if (summary.gameName) {
    seed.gameName = summary.gameName;
  }

  if (summary.steamAppId && NUMERIC_APP_ID_RE.test(summary.steamAppId)) {
    seed.steamAppId = summary.steamAppId;
  }

  if (summary.customCoverArtPath) {
    seed.coverArtPath = summary.customCoverArtPath;
  }

  if (summary.customPortraitArtPath) {
    seed.portraitArtPath = summary.customPortraitArtPath;
  }

  // Only seed executablePath when the context profile belongs to one of this game's cards.
  const contextProfileName = contextProfile.game.name.trim();
  const isContextProfileInCards = cards.some((c) => c.name === contextProfileName);
  if (selectedTrimmed && contextProfileName === selectedTrimmed && isContextProfileInCards) {
    const execPath = contextProfile.game.executable_path;
    if (execPath) {
      seed.executablePath = execPath;
    }
  }

  return seed;
}

/**
 * Renders the profile card list sidebar for the Hero Detail profiles tab,
 * including the "+ New" button that opens the OnboardingWizard.
 */
export function HeroProfileCardList({
  cards,
  summary,
  selectedTrimmed,
  healthByName,
  onSelectCard,
}: HeroProfileCardListProps) {
  const [showWizard, setShowWizard] = useState(false);
  const cardNames = cards.map((card) => card.name);
  const { metaByProfileName } = useProfileCardMeta(cardNames);
  const { profile, selectProfile } = useProfileContext();

  const seed = useMemo(
    () => buildHeroCreateSeed(summary, cards, selectedTrimmed, profile),
    [summary, cards, selectedTrimmed, profile]
  );

  function openWizard() {
    setShowWizard(true);
  }

  return (
    <>
      <aside className="crosshook-hero-detail__profiles-cards" aria-label="Profiles for this game">
        {cards.length === 0 ? (
          <div className="crosshook-panel" role="status">
            <p className="crosshook-hero-detail__muted">No profiles found for this game.</p>
            <button type="button" className="crosshook-button crosshook-button--primary" onClick={openWizard}>
              Create profile
            </button>
          </div>
        ) : (
          <ul className="crosshook-hero-detail__profiles-list" aria-label="Profile cards">
            {cards.map((card) => {
              const isSelected = card.name === selectedTrimmed;
              const cardMeta = metaByProfileName[card.name];
              const metaLabel = profileCardMetaLabel(card.name, cardMeta?.protonLabel ?? null);
              const healthReport = healthByName?.[card.name];

              return (
                <li key={card.name}>
                  <button
                    type="button"
                    className={[
                      'crosshook-hero-detail__profiles-card',
                      isSelected ? 'crosshook-hero-detail__profiles-card--selected' : '',
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    aria-current={isSelected ? 'true' : undefined}
                    aria-label={profileCardTitle(card)}
                    onClick={() => {
                      onSelectCard(card.name);
                    }}
                  >
                    <div className="crosshook-hero-detail__profiles-card-header">
                      <strong className="crosshook-hero-detail__profiles-card-name">
                        {isSelected ? <span aria-hidden="true">✓ </span> : null}
                        {card.name}
                      </strong>
                      {isSelected ? <span className="crosshook-hero-detail__pill">Active</span> : null}
                    </div>
                    <span className="crosshook-hero-detail__text--small">{card.gameName || summary.gameName}</span>
                    {metaLabel ? <span className="crosshook-hero-detail__profiles-card-meta">{metaLabel}</span> : null}
                    {cardMeta?.lastUsedLabel ? (
                      <span className="crosshook-hero-detail__text--small">last used {cardMeta.lastUsedLabel}</span>
                    ) : null}
                    {healthReport ? <HealthBadge report={healthReport} /> : null}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
        <div className="crosshook-hero-detail__profiles-cta">
          <button type="button" className="crosshook-button crosshook-button--secondary" onClick={openWizard}>
            + New
          </button>
        </div>
      </aside>

      {showWizard ? (
        <OnboardingWizard
          open
          mode="create"
          createSeed={seed}
          onComplete={(createdName) => {
            setShowWizard(false);
            if (createdName) {
              void selectProfile(createdName);
            }
          }}
          onDismiss={() => {
            setShowWizard(false);
            if (selectedTrimmed) {
              void selectProfile(selectedTrimmed);
            }
          }}
        />
      ) : null}
    </>
  );
}

export default HeroProfileCardList;
