import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import { launchOptimizationsAutosaveDelayMs } from '@/hooks/profile/constants';
import type { GameDetailsProfileLoadState } from '@/hooks/useGameDetailsProfile';
import { useProfileCardMeta } from '@/hooks/useProfileCardMeta';
import { useProtonInstalls } from '@/hooks/useProtonInstalls';
import type { EnrichedProfileHealthReport } from '@/types/health';
import type { LaunchAutoSaveStatus } from '@/types/launch';
import type { LibraryCardData, ProfileSummary } from '@/types/library';
import { resolveLaunchMethod } from '@/utils/launch';
import { HealthBadge } from '../HealthBadge';
import { OnboardingWizard } from '../OnboardingWizard';
import { GameSection } from '../profile-sections/GameSection';
import { MediaSection } from '../profile-sections/MediaSection';
import { ProfileIdentitySection } from '../profile-sections/ProfileIdentitySection';
import { RuntimeSection } from '../profile-sections/RuntimeSection';

export interface HeroDetailProfilesTabProps {
  summary: LibraryCardData;
  profileList: ProfileSummary[] | undefined;
  loadState: GameDetailsProfileLoadState;
  profileError: string | null;
  healthByName?: Partial<Record<string, EnrichedProfileHealthReport>>;
}

const idleStatus: LaunchAutoSaveStatus = {
  tone: 'idle',
  label: 'Saved',
};

function profileCardTitle(card: ProfileSummary): string {
  return card.gameName.trim() ? `${card.name} - ${card.gameName}` : card.name;
}

function profileCardMetaLabel(profileName: string, protonLabel: string | null): string {
  return [profileName ? `${profileName}.toml` : null, protonLabel].filter(Boolean).join(' · ');
}

function ownsProfile(profileNames: Set<string>, selectedProfile: string): boolean {
  return selectedProfile.trim().length > 0 && profileNames.has(selectedProfile.trim());
}

export function HeroDetailProfilesTab({
  summary,
  profileList,
  loadState,
  profileError,
  healthByName,
}: HeroDetailProfilesTabProps) {
  const {
    profile,
    profileName,
    selectedProfile,
    profiles,
    dirty,
    saving,
    error,
    selectProfile,
    updateProfile,
    setProfileName,
    persistProfileDraft,
    steamClientInstallPath,
  } = useProfileContext();
  const [showWizard, setShowWizard] = useState(false);
  const [autoSaveStatus, setAutoSaveStatus] = useState<LaunchAutoSaveStatus>(idleStatus);
  const latestProfileNameRef = useRef(selectedProfile.trim());
  const latestProfileRef = useRef(profile);

  const cards = profileList ?? [];
  const cardNames = useMemo(() => cards.map((card) => card.name), [cards]);
  const profileNames = useMemo(() => new Set(cardNames), [cardNames]);
  const singletonOwnsGame = ownsProfile(profileNames, selectedProfile);
  const selectedTrimmed = selectedProfile.trim();
  const profileNameTrimmed = profileName.trim();
  const profileExists = selectedTrimmed.length > 0 && profiles.includes(selectedTrimmed);
  const hasSavedSelectedProfile = profileExists && profileNameTrimmed === selectedTrimmed;
  const launchMethod = resolveLaunchMethod(profile);
  const { installs: protonInstalls, error: protonInstallsError } = useProtonInstalls({
    steamClientInstallPath,
  });
  const { metaByProfileName } = useProfileCardMeta(cardNames);

  useEffect(() => {
    latestProfileNameRef.current = selectedTrimmed;
  }, [selectedTrimmed]);

  useEffect(() => {
    latestProfileRef.current = profile;
  }, [profile]);

  useEffect(() => {
    if (cards.length === 0 || singletonOwnsGame) {
      return;
    }

    void selectProfile(summary.name);
  }, [cards.length, selectProfile, singletonOwnsGame, summary.name]);

  useEffect(() => {
    if (saving) {
      setAutoSaveStatus({ tone: 'saving', label: 'Saving profile…' });
      return;
    }

    if (error) {
      setAutoSaveStatus({ tone: 'error', label: 'Profile save failed', detail: error });
      return;
    }

    if (!dirty) {
      setAutoSaveStatus(idleStatus);
    }
  }, [dirty, error, saving]);

  useEffect(() => {
    if (!dirty || !hasSavedSelectedProfile) {
      return;
    }

    const scheduledProfileName = selectedTrimmed;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled || latestProfileNameRef.current !== scheduledProfileName) {
        return;
      }

      setAutoSaveStatus({ tone: 'saving', label: 'Saving profile…' });
      void persistProfileDraft(scheduledProfileName, latestProfileRef.current).then((result) => {
        if (cancelled || latestProfileNameRef.current !== scheduledProfileName) {
          return;
        }

        setAutoSaveStatus(
          result.ok
            ? { tone: 'success', label: 'Profile saved' }
            : { tone: 'error', label: 'Profile save failed', detail: result.error }
        );
      });
    }, launchOptimizationsAutosaveDelayMs);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [dirty, hasSavedSelectedProfile, persistProfileDraft, selectedTrimmed]);

  const selectCard = useCallback(
    async (cardName: string) => {
      if (cardName === selectedTrimmed) {
        return;
      }

      if (dirty && hasSavedSelectedProfile) {
        const result = await persistProfileDraft(selectedTrimmed, profile);
        if (!result.ok) {
          setAutoSaveStatus({ tone: 'error', label: 'Profile save failed', detail: result.error });
          return;
        }
      }

      await selectProfile(cardName);
    },
    [dirty, hasSavedSelectedProfile, persistProfileDraft, profile, selectProfile, selectedTrimmed]
  );

  const autoSaveChip =
    autoSaveStatus.tone !== 'idle' ? (
      <span
        className={`crosshook-launch-autosave-chip crosshook-launch-autosave-chip--${autoSaveStatus.tone}`}
        aria-live="polite"
        aria-atomic="true"
        title={autoSaveStatus.detail}
      >
        {autoSaveStatus.label}
      </span>
    ) : null;

  return (
    <div className="crosshook-hero-detail__profiles">
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
                      void selectCard(card.name);
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

      <section className="crosshook-hero-detail__profiles-editor" aria-label="Profile editor">
        {loadState === 'loading' ? <p className="crosshook-hero-detail__muted">Loading profile details…</p> : null}
        {loadState === 'error' ? (
          <p className="crosshook-hero-detail__warn">{profileError ?? 'Failed to load profile.'}</p>
        ) : null}
        {!singletonOwnsGame && loadState !== 'loading' ? (
          <p className="crosshook-hero-detail__muted" role="status">
            Select a profile card to edit this game.
          </p>
        ) : null}
        {singletonOwnsGame && profile ? (
          <div className="crosshook-hero-detail__profiles-editor-stack">
            <div className="crosshook-hero-detail__profiles-editor-header">
              <h3 className="crosshook-hero-detail__section-title">{profileName || selectedTrimmed}</h3>
              {autoSaveChip}
            </div>
            <ProfileIdentitySection
              profileName={profileName}
              profile={profile}
              onProfileNameChange={setProfileName}
              onUpdateProfile={updateProfile}
              profileExists={profileExists}
              profiles={profiles}
            />
            <RuntimeSection
              profile={profile}
              onUpdateProfile={updateProfile}
              launchMethod={launchMethod}
              protonInstalls={protonInstalls}
              protonInstallsError={protonInstallsError}
            />
            <GameSection profile={profile} onUpdateProfile={updateProfile} launchMethod={launchMethod} />
            <MediaSection profile={profile} onUpdateProfile={updateProfile} launchMethod={launchMethod} />
          </div>
        ) : null}
      </section>

      {showWizard ? (
        <OnboardingWizard
          open
          mode="create"
          onComplete={() => setShowWizard(false)}
          onDismiss={() => setShowWizard(false)}
        />
      ) : null}
    </div>
  );
}

export default HeroDetailProfilesTab;
