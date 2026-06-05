import { useCallback, useEffect, useMemo, useState } from 'react';
import { usePreferencesContext } from '@/context/PreferencesContext';
import { useProfileContext } from '@/context/ProfileContext';
import { useGameCoverArt } from '@/hooks/useGameCoverArt';
import { useGameDetailsProfile } from '@/hooks/useGameDetailsProfile';
import { useGameMetadata } from '@/hooks/useGameMetadata';
import { usePreviewState } from '@/hooks/usePreviewState';
import { useProfileSummaries } from '@/hooks/useProfileSummaries';
import type { OfflineReadinessReport } from '@/types';
import type { EnrichedProfileHealthReport } from '@/types/health';
import type { LibraryCardData } from '@/types/library';
import type { GameProfile } from '@/types/profile';
import { buildProfileLaunchRequest, resolveLaunchMethod } from '@/utils/launch';
import { effectiveGameArtPath } from '@/utils/profile-art';
import { HeroDetailHeader } from './HeroDetailHeader';
import { HeroDetailTabs } from './HeroDetailTabs';
import type { HeroDetailProfilesScrollTarget, HeroDetailTabId, HeroDetailTabRequestOptions } from './hero-detail-model';
import { resolveGameDetailsHero } from './hero-detail-model';

export interface GameDetailProps {
  summary: LibraryCardData;
  onBack: () => void;
  healthByName: Partial<Record<string, EnrichedProfileHealthReport>>;
  healthLoading: boolean;
  offlineReportFor: (profileName: string) => OfflineReadinessReport | undefined;
  offlineError: string | null;
  onLaunch: (name: string) => void | Promise<void>;
  onEdit: (name: string) => void | Promise<void>;
  onToggleFavorite: (name: string, current: boolean) => void;
  launchingName?: string;
}

export function GameDetail({
  summary,
  onBack,
  healthByName,
  healthLoading,
  offlineReportFor,
  offlineError,
  onLaunch,
  onEdit,
  onToggleFavorite,
  launchingName,
}: GameDetailProps) {
  const [activeTab, setActiveTab] = useState<HeroDetailTabId>('overview');
  const [profilesScrollTarget, setProfilesScrollTarget] = useState<HeroDetailProfilesScrollTarget | null>(null);
  const fallbackDetailsProfile = useGameDetailsProfile(summary.name, true);
  const { settings, defaultSteamClientInstallPath } = usePreferencesContext();
  const ctx = useProfileContext();
  const { summaries } = useProfileSummaries(ctx.profiles);

  const profileList = useMemo(
    () => summaries.filter((s) => s.gameName === summary.gameName),
    [summaries, summary.gameName]
  );
  const gameProfileNames = useMemo(() => new Set(profileList.map((s) => s.name)), [profileList]);
  const selectedProfileName = ctx.selectedProfile.trim();
  const singletonOwnsGame = gameProfileNames.has(selectedProfileName);

  const displayProfileState = useMemo<{
    profile: GameProfile | null;
    loadState: typeof fallbackDetailsProfile.loadState;
    errorMessage: string | null;
    name: string;
  }>(() => {
    if (singletonOwnsGame && ctx.profile) {
      return {
        profile: ctx.profile,
        loadState: 'ready',
        errorMessage: null,
        name: selectedProfileName,
      };
    }

    return {
      profile: fallbackDetailsProfile.profile,
      loadState: fallbackDetailsProfile.loadState,
      errorMessage: fallbackDetailsProfile.errorMessage,
      name: summary.name,
    };
  }, [
    ctx.profile,
    fallbackDetailsProfile.errorMessage,
    fallbackDetailsProfile.loadState,
    fallbackDetailsProfile.profile,
    selectedProfileName,
    singletonOwnsGame,
    summary.name,
  ]);

  const { profile, loadState, errorMessage, name: displayProfileName } = displayProfileState;

  const steamAppIdForHooks = summary.steamAppId?.trim() ?? '';
  const hasNumericAppId = /^\d+$/.test(steamAppIdForHooks);
  const appIdForArt = hasNumericAppId ? steamAppIdForHooks : undefined;

  const customBgPath = effectiveGameArtPath(profile, 'custom_background_art_path');
  const customPortraitPath =
    loadState === 'ready' && profile
      ? (effectiveGameArtPath(profile, 'custom_portrait_art_path') ?? summary.customPortraitArtPath)
      : summary.customPortraitArtPath;

  const meta = useGameMetadata(appIdForArt);
  const backgroundArt = useGameCoverArt(appIdForArt, customBgPath, 'background');
  const heroGridArt = useGameCoverArt(appIdForArt, undefined, 'hero');
  const portraitArt = useGameCoverArt(appIdForArt, customPortraitPath, 'portrait');

  const headerImage = meta.appDetails?.header_image?.trim() || null;
  const metaLoading = Boolean(hasNumericAppId) && (meta.loading || meta.state === 'idle' || meta.state === 'loading');

  const heroResolved = useMemo(
    () =>
      resolveGameDetailsHero({
        customBgPath,
        bg: { url: backgroundArt.coverArtUrl, loading: backgroundArt.loading },
        hero: { url: heroGridArt.coverArtUrl, loading: heroGridArt.loading },
        headerImage,
        metaLoading,
      }),
    [
      customBgPath,
      backgroundArt.coverArtUrl,
      backgroundArt.loading,
      heroGridArt.coverArtUrl,
      heroGridArt.loading,
      headerImage,
      metaLoading,
    ]
  );

  const [heroImgBroken, setHeroImgBroken] = useState(false);
  const [portraitImgBroken, setPortraitImgBroken] = useState(false);

  const displayName = summary.gameName || summary.name;
  const methodLabel = profile && loadState === 'ready' ? resolveLaunchMethod(profile) : null;

  const updateProfile = useMemo(
    () => async (draft: GameProfile) => {
      const result = await ctx.persistProfileDraft(displayProfileName, draft);
      if (!result.ok) {
        throw new Error(result.error);
      }
    },
    [ctx.persistProfileDraft, displayProfileName]
  );

  const launchRequest = useMemo(() => {
    if (!profile || loadState !== 'ready') {
      return null;
    }
    const method = resolveLaunchMethod(profile);
    return buildProfileLaunchRequest(
      profile,
      method,
      defaultSteamClientInstallPath || '',
      summary.name,
      settings.umu_preference
    );
  }, [profile, loadState, defaultSteamClientInstallPath, summary.name, settings.umu_preference]);

  const { loading: previewLoading, preview, error: previewError, requestPreview, clearPreview } = usePreviewState();

  useEffect(() => {
    if (!launchRequest) {
      clearPreview();
      return;
    }
    void requestPreview(launchRequest);
  }, [launchRequest, requestPreview, clearPreview]);

  const healthReport = healthByName[summary.name];
  const offlineReport = offlineReportFor(summary.name);

  const handleSetActiveTab = useCallback((tab: HeroDetailTabId, options?: HeroDetailTabRequestOptions) => {
    setProfilesScrollTarget(tab === 'profiles' ? (options?.profilesScrollTarget ?? null) : null);
    setActiveTab(tab);
  }, []);

  const handleActiveTabChange = useCallback((tab: HeroDetailTabId) => {
    if (tab !== 'profiles') {
      setProfilesScrollTarget(null);
    }
    setActiveTab(tab);
  }, []);

  const handleProfilesScrollTargetConsumed = useCallback(() => {
    setProfilesScrollTarget(null);
  }, []);

  const panelProps = useMemo(
    () => ({
      summary,
      steamAppId: steamAppIdForHooks,
      meta,
      profile,
      loadState,
      profileError: errorMessage,
      healthReport,
      healthLoading,
      offlineReport,
      offlineError,
      launchRequest,
      previewLoading,
      preview,
      previewError,
      updateProfile,
      profileList,
      onPreviewLaunch: requestPreview,
      onLaunch,
      launchingName,
      displayProfileName,
      onSetActiveTab: handleSetActiveTab,
      profilesScrollTarget,
      onProfilesScrollTargetConsumed: handleProfilesScrollTargetConsumed,
    }),
    [
      summary,
      steamAppIdForHooks,
      meta,
      profile,
      loadState,
      errorMessage,
      healthReport,
      healthLoading,
      offlineReport,
      offlineError,
      launchRequest,
      previewLoading,
      preview,
      previewError,
      updateProfile,
      profileList,
      requestPreview,
      onLaunch,
      launchingName,
      displayProfileName,
      handleSetActiveTab,
      profilesScrollTarget,
      handleProfilesScrollTargetConsumed,
    ]
  );

  return (
    <div className="crosshook-hero-detail" data-testid="game-detail">
      <HeroDetailHeader
        summary={summary}
        displayName={displayName}
        profile={profile}
        loadState={loadState}
        profileError={errorMessage}
        methodLabel={methodLabel}
        heroResolved={heroResolved}
        portraitArt={{ coverArtUrl: portraitArt.coverArtUrl, loading: portraitArt.loading }}
        heroImgBroken={heroImgBroken}
        setHeroImgBroken={setHeroImgBroken}
        portraitImgBroken={portraitImgBroken}
        setPortraitImgBroken={setPortraitImgBroken}
        launchingName={launchingName}
        onBack={onBack}
        onLaunch={onLaunch}
        onEdit={onEdit}
        onToggleFavorite={onToggleFavorite}
      />
      <div className="crosshook-hero-detail__body">
        <HeroDetailTabs activeTab={activeTab} onActiveTabChange={handleActiveTabChange} panelProps={panelProps} />
      </div>
    </div>
  );
}

export default GameDetail;
