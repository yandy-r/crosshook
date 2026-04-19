import { useCallback, useMemo, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import { usePreferencesContext } from '../../../context/PreferencesContext';
import { useProfileContext } from '../../../context/ProfileContext';
import { useProfileHealthContext } from '../../../context/ProfileHealthContext';
import { useCollectionMembers } from '../../../hooks/useCollectionMembers';
import { useCollections } from '../../../hooks/useCollections';
import type { CommunityExportResult } from '../../../hooks/useCommunityProfiles';
import { useLaunchPlatformStatus } from '../../../hooks/useLaunchPlatformStatus';
import { useOfflineReadiness } from '../../../hooks/useOfflineReadiness';
import { useProfileSummaries } from '../../../hooks/useProfileSummaries';
import { useTrainerTypeCatalog } from '../../../hooks/useTrainerTypeCatalog';
import { chooseSaveFile } from '../../../utils/dialog';
import { deriveTargetHomePath } from '../../../utils/steam';
import { useProfilesCollectionState } from './useProfilesCollectionState';
import { useProfilesPageNotifications } from './useProfilesPageNotifications';
import { useProfilesPageProton } from './useProfilesPageProton';
import { suggestedCommunityExportFilename } from './utils';

export function useProfilesPageState() {
  const { defaultSteamClientInstallPath } = usePreferencesContext();
  const {
    deleting,
    dirty,
    duplicateProfile,
    duplicating,
    error,
    executeDelete,
    loading,
    pendingDelete,
    profile,
    profileExists,
    profileName,
    profiles,
    refreshProfiles,
    renameProfile,
    renaming,
    saveProfile,
    saving,
    selectProfile,
    selectedProfile,
    setProfileName,
    cancelDelete,
    confirmDelete,
    updateProfile,
    launchMethod,
    steamClientInstallPath,
    fetchConfigHistory,
    fetchConfigDiff,
    rollbackConfig,
    markKnownGood,
    activeCollectionId,
    setActiveCollectionId,
  } = useProfileContext();
  const { collections } = useCollections();
  const { memberNames, membersForCollectionId, loading: membersLoading } = useCollectionMembers(activeCollectionId);

  const launchPlatform = useLaunchPlatformStatus();
  const { summaries: profileSummaries } = useProfileSummaries(profiles, activeCollectionId);
  const profileNetworkIsolation = useMemo(() => {
    const next: Record<string, boolean> = {};
    for (const row of profileSummaries) {
      next[row.name] = row.networkIsolation;
    }
    return next;
  }, [profileSummaries]);

  const collectionState = useProfilesCollectionState({
    activeCollectionId,
    collections,
    memberNames,
    membersForCollectionId,
    membersLoading,
    profileNetworkIsolation,
    profileUsesNetworkIsolation: profile.launch.network_isolation ?? true,
    profiles,
    selectedProfile,
    selectProfile,
    systemCanUnshareNet: launchPlatform?.unshareNetAvailable ?? null,
  });

  const [pendingLauncherReExport, setPendingLauncherReExport] = useState(false);
  const notifications = useProfilesPageNotifications({
    canRename: profileExists && !saving && !deleting && !loading && !duplicating && !renaming,
    hasPendingDelete: pendingDelete !== null,
    profiles,
    renaming,
    renameProfile,
    selectedProfile,
    setPendingLauncherReExport,
  });

  const {
    batchValidate,
    revalidateSingle,
    healthByName,
    summary,
    loading: healthLoading,
    staleInfoByName,
    cachedSnapshots,
    trendByName,
  } = useProfileHealthContext();
  const offlineReadiness = useOfflineReadiness();
  const { labels: trainerTypeLabels } = useTrainerTypeCatalog();

  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || steamClientInstallPath,
    [defaultSteamClientInstallPath, steamClientInstallPath]
  );
  const targetHomePath = useMemo(
    () => deriveTargetHomePath(effectiveSteamClientInstallPath),
    [effectiveSteamClientInstallPath]
  );
  const protonState = useProfilesPageProton({
    effectiveSteamClientInstallPath,
    gameName: profile.game.name,
    selectedProfile,
  });

  const canSave =
    profileName.trim().length > 0 && profile.game.executable_path.trim().length > 0 && !saving && !deleting && !loading;
  const canDelete = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canDuplicate = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canRename = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canPreview = profileName.trim().length > 0 && !loading;
  const [previewing, setPreviewing] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [showProfilePreview, setShowProfilePreview] = useState(false);
  const [profilePreviewContent, setProfilePreviewContent] = useState('');
  const [exportingCommunity, setExportingCommunity] = useState(false);
  const [communityExportError, setCommunityExportError] = useState<string | null>(null);
  const [communityExportSuccess, setCommunityExportSuccess] = useState<string | null>(null);
  const [showHistoryPanel, setShowHistoryPanel] = useState(false);
  const [showWizard, setShowWizard] = useState(false);
  const [wizardMode, setWizardMode] = useState<'create' | 'edit'>('create');
  const healthIssuesRef = useRef<HTMLDivElement>(null);

  const canExportCommunity =
    profileExists && !saving && !deleting && !loading && !duplicating && !renaming && !exportingCommunity;
  const canViewHistory =
    Boolean(selectedProfile.trim()) &&
    profiles.includes(selectedProfile.trim()) &&
    !saving &&
    !deleting &&
    !loading &&
    !duplicating &&
    !renaming &&
    !exportingCommunity;

  const handleSave = useCallback(async () => {
    await saveProfile();
    if (profileName.trim()) {
      void revalidateSingle(profileName.trim());
    }
  }, [profileName, revalidateSingle, saveProfile]);

  const handleAfterRollback = useCallback(
    (name: string) => {
      void revalidateSingle(name);
    },
    [revalidateSingle]
  );

  const handleExportCommunityProfile = useCallback(async () => {
    const nameOnDisk = selectedProfile.trim();
    if (!nameOnDisk || !profiles.includes(nameOnDisk)) {
      setCommunityExportError('Save the profile before exporting as a community manifest.');
      setCommunityExportSuccess(null);
      return;
    }

    setCommunityExportError(null);
    setCommunityExportSuccess(null);

    const outputPath = await chooseSaveFile('Export community profile', {
      defaultPath: suggestedCommunityExportFilename(nameOnDisk),
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });

    if (outputPath === null) {
      return;
    }

    setExportingCommunity(true);
    try {
      const result = await callCommand<CommunityExportResult>('community_export_profile', {
        profile_name: nameOnDisk,
        output_path: outputPath,
      });
      setCommunityExportSuccess(`Community profile saved to ${result.output_path}`);
      setCommunityExportError(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('Community profile export failed:', err);
      setCommunityExportError(message);
      setCommunityExportSuccess(null);
    } finally {
      setExportingCommunity(false);
    }
  }, [profiles, selectedProfile]);

  const handlePreviewProfile = useCallback(async () => {
    setPreviewing(true);
    setPreviewError(null);
    try {
      const toml = await callCommand<string>('profile_export_toml', {
        name: profileName,
        data: profile,
      });
      setProfilePreviewContent(toml);
      setPreviewError(null);
      setShowProfilePreview(true);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('Profile preview failed:', err);
      setPreviewError(message);
    } finally {
      setPreviewing(false);
    }
  }, [profile, profileName]);

  const handleCloseProfilePreview = useCallback(() => {
    setShowProfilePreview(false);
    setPreviewError(null);
  }, []);

  const handleRefreshStatus = useCallback(async () => {
    await refreshProfiles();
    await batchValidate();
  }, [batchValidate, refreshProfiles]);

  const openWizard = useCallback((mode: 'create' | 'edit') => {
    setWizardMode(mode);
    setShowWizard(true);
  }, []);

  const selectedReport = selectedProfile ? healthByName[selectedProfile] : undefined;
  const selectedCachedSnapshot = selectedProfile ? cachedSnapshots[selectedProfile] : undefined;
  const selectedStaleInfo = selectedProfile ? staleInfoByName[selectedProfile] : undefined;
  const selectedTrend = selectedProfile ? (trendByName[selectedProfile] ?? null) : null;
  const selectedVersionStatus = selectedReport?.metadata?.version_status ?? null;
  const selectedTrainerVersion = selectedReport?.metadata?.trainer_version ?? null;
  const hasSelectedProfile = selectedProfile.trim().length > 0;
  const selectedOfflineReport = selectedProfile ? offlineReadiness.reportForProfile(selectedProfile) : undefined;

  const trainerTypeDisplayName = useMemo(() => {
    const id = profile.trainer.trainer_type?.trim() || 'unknown';
    return trainerTypeLabels[id] ?? id;
  }, [profile.trainer.trainer_type, trainerTypeLabels]);

  return {
    ...collectionState,
    ...notifications,
    ...protonState,
    canDelete,
    canDuplicate,
    canExportCommunity,
    canPreview,
    canRename,
    canSave,
    canViewHistory,
    cancelDelete,
    communityExportError,
    communityExportSuccess,
    confirmDelete,
    deleting,
    dirty,
    duplicateProfile,
    duplicating,
    effectiveSteamClientInstallPath,
    error,
    executeDelete,
    exportingCommunity,
    fetchConfigDiff,
    fetchConfigHistory,
    handleAfterRollback,
    handleCloseProfilePreview,
    handleExportCommunityProfile,
    handlePreviewProfile,
    handleRefreshStatus,
    handleSave,
    hasSelectedProfile,
    healthIssuesRef,
    healthLoading,
    launchMethod,
    loading,
    markKnownGood,
    pendingDelete,
    pendingLauncherReExport,
    previewError,
    previewing,
    profile,
    profileExists,
    profileName,
    profilePreviewContent,
    profiles,
    renaming,
    revalidateSingle,
    rollbackConfig,
    saving,
    selectProfile,
    selectedCachedSnapshot,
    selectedOfflineReport,
    selectedProfile,
    selectedReport,
    selectedStaleInfo,
    selectedTrainerVersion,
    selectedTrend,
    selectedVersionStatus,
    setActiveCollectionId,
    setPendingLauncherReExport,
    setProfileName,
    setShowHistoryPanel,
    setShowWizard,
    showHistoryPanel,
    showProfilePreview,
    showWizard,
    summary,
    targetHomePath,
    trainerTypeDisplayName,
    updateProfile,
    wizardMode,
    openWizard,
  };
}
