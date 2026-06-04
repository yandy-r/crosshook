import { useEffect, useMemo } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import type { GameDetailsProfileLoadState } from '@/hooks/useGameDetailsProfile';
import { useProtonInstalls } from '@/hooks/useProtonInstalls';
import type { EnrichedProfileHealthReport } from '@/types/health';
import type { LibraryCardData, ProfileSummary } from '@/types/library';
import { resolveLaunchMethod } from '@/utils/launch';
import { HeroProfileCardList } from './profiles/HeroProfileCardList';
import { HeroProfileEditorSections } from './profiles/HeroProfileEditorSections';
import { useHeroProfilesAutosave } from './profiles/useHeroProfilesAutosave';

export interface HeroDetailProfilesTabProps {
  summary: LibraryCardData;
  profileList: ProfileSummary[] | undefined;
  loadState: GameDetailsProfileLoadState;
  profileError: string | null;
  healthByName?: Partial<Record<string, EnrichedProfileHealthReport>>;
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

  const cards = profileList ?? [];
  const cardNames = useMemo(() => cards.map((card) => card.name), [cards]);
  const profileNames = useMemo(() => new Set(cardNames), [cardNames]);
  const singletonOwnsGame = ownsProfile(profileNames, selectedProfile);
  const selectedTrimmed = selectedProfile.trim();
  const profileExists = selectedTrimmed.length > 0 && profiles.includes(selectedTrimmed);
  const launchMethod = resolveLaunchMethod(profile);
  const { installs: protonInstalls, error: protonInstallsError } = useProtonInstalls({
    steamClientInstallPath,
  });

  useEffect(() => {
    if (cards.length === 0 || singletonOwnsGame) {
      return;
    }

    void selectProfile(summary.name);
  }, [cards.length, selectProfile, singletonOwnsGame, summary.name]);

  const { autoSaveStatus, selectCard } = useHeroProfilesAutosave({
    profile,
    profileName,
    selectedProfile,
    profiles,
    dirty,
    saving,
    error,
    persistProfileDraft,
    selectProfile,
  });

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
      <HeroProfileCardList
        cards={cards}
        summary={summary}
        selectedTrimmed={selectedTrimmed}
        healthByName={healthByName}
        onSelectCard={(cardName) => {
          void selectCard(cardName);
        }}
      />

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
            <HeroProfileEditorSections
              profile={profile}
              profileName={profileName}
              profileExists={profileExists}
              profiles={profiles}
              launchMethod={launchMethod}
              protonInstalls={protonInstalls}
              protonInstallsError={protonInstallsError}
              onUpdateProfile={updateProfile}
              onProfileNameChange={setProfileName}
            />
          </div>
        ) : null}
      </section>
    </div>
  );
}

export default HeroDetailProfilesTab;
