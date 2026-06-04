import { useState } from 'react';
import { useProfileCardMeta } from '@/hooks/useProfileCardMeta';
import type { EnrichedProfileHealthReport } from '@/types/health';
import type { LibraryCardData, ProfileSummary } from '@/types/library';
import { HealthBadge } from '../../HealthBadge';
import { OnboardingWizard } from '../../OnboardingWizard';

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

  return (
    <>
      <aside className="crosshook-hero-detail__profiles-cards" aria-label="Profiles for this game">
        {cards.length === 0 ? (
          <p className="crosshook-hero-detail__muted" role="status">
            No profiles found for this game.
          </p>
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
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => setShowWizard(true)}
          >
            + New
          </button>
        </div>
      </aside>

      {showWizard ? (
        <OnboardingWizard
          open
          mode="create"
          onComplete={() => setShowWizard(false)}
          onDismiss={() => setShowWizard(false)}
        />
      ) : null}
    </>
  );
}

export default HeroProfileCardList;
