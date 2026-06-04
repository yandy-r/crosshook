import { useEffect, useMemo, useRef } from 'react';
import { usePreferencesContext } from '@/context/PreferencesContext';
import { useProfileContext } from '@/context/ProfileContext';
import { useProfileHealthContext } from '@/context/ProfileHealthContext';
import type { GameDetailsProfileLoadState } from '@/hooks/useGameDetailsProfile';
import { useProtonInstalls } from '@/hooks/useProtonInstalls';
import { useTrainerTypeCatalog } from '@/hooks/useTrainerTypeCatalog';
import type { EnrichedProfileHealthReport } from '@/types/health';
import type { LibraryCardData, ProfileSummary } from '@/types/library';
import { resolveArtAppId } from '@/utils/art';
import { resolveLaunchMethod } from '@/utils/launch';
import { useProfilesPageProton } from '../pages/profiles/useProfilesPageProton';
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

  const { defaultSteamClientInstallPath } = usePreferencesContext();

  const { healthByName: healthByNameCtx, staleInfoByName, cachedSnapshots, trendByName } = useProfileHealthContext();

  const { labels: trainerTypeLabels } = useTrainerTypeCatalog();

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

  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || steamClientInstallPath,
    [defaultSteamClientInstallPath, steamClientInstallPath]
  );

  // Proton suggestion banner (community-recommended version)
  const protonState = useProfilesPageProton({
    effectiveSteamClientInstallPath,
    gameName: profile.game.name,
    selectedProfile,
  });

  // Health data for the selected profile
  // Prefer live data from healthByName prop (passed from parent), fall back to context
  const selectedReport = selectedTrimmed
    ? (healthByName?.[selectedTrimmed] ?? healthByNameCtx[selectedTrimmed])
    : undefined;
  const selectedCachedSnapshot = selectedTrimmed ? cachedSnapshots[selectedTrimmed] : undefined;
  const selectedStaleInfo = selectedTrimmed ? staleInfoByName[selectedTrimmed] : undefined;
  const selectedTrend = selectedTrimmed ? (trendByName[selectedTrimmed] ?? null) : null;
  const versionStatus = selectedReport?.metadata?.version_status ?? null;
  const trainerVersion = selectedReport?.metadata?.trainer_version ?? null;

  // Trainer type display name (for health-badge chip)
  const trainerTypeDisplayName = useMemo(() => {
    const id = profile.trainer?.trainer_type?.trim() || 'unknown';
    return trainerTypeLabels[id] ?? id;
  }, [profile.trainer?.trainer_type, trainerTypeLabels]);

  // Network isolation badge: show when the selected card reports no isolation
  const selectedCard = cards.find((c) => c.name === selectedTrimmed);
  const showNetworkIsolationBadge = selectedCard?.networkIsolation === false;

  // Ref for health-issues scroll target (per-card badge click)
  const healthIssuesRef = useRef<HTMLDivElement>(null);

  // Steam App ID for GameMetadataBar
  const steamAppId = resolveArtAppId(profile) || summary.steamAppId || undefined;

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
              steamAppId={steamAppId}
              trainerVersion={trainerVersion}
              selectedReport={selectedReport}
              selectedCachedSnapshot={selectedCachedSnapshot}
              selectedTrend={selectedTrend}
              staleInfo={selectedStaleInfo}
              trainerTypeDisplayName={trainerTypeDisplayName}
              showNetworkIsolationBadge={showNetworkIsolationBadge}
              versionStatus={versionStatus ?? undefined}
              healthIssuesRef={healthIssuesRef}
              suggestion={protonState.suggestion}
              suggestionDismissed={protonState.suggestionDismissed}
              suggestionInstallError={protonState.suggestionInstallError}
              protonUpInstalling={protonState.protonUp.installing}
              effectiveSteamClientInstallPath={effectiveSteamClientInstallPath}
              onInstallSuggestedVersion={() => void protonState.handleInstallSuggestedVersion()}
              onDismissSuggestion={() => protonState.setSuggestionDismissed(true)}
            />
          </div>
        ) : null}
      </section>
    </div>
  );
}

export default HeroDetailProfilesTab;
