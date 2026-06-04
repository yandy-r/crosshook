import { useMemo, useRef, useState } from 'react';

import { usePreferencesContext } from '../../../context/PreferencesContext';
import { useProfileContext } from '../../../context/ProfileContext';
import { useProfileHealthContext } from '../../../context/ProfileHealthContext';
import { useProfileActions } from '../../../hooks/profile/useProfileActions';
import { useCollectionMembers } from '../../../hooks/useCollectionMembers';
import { useCollections } from '../../../hooks/useCollections';
import { useLaunchPlatformStatus } from '../../../hooks/useLaunchPlatformStatus';
import { useOfflineReadiness } from '../../../hooks/useOfflineReadiness';
import { useProfileSummaries } from '../../../hooks/useProfileSummaries';
import { useTrainerTypeCatalog } from '../../../hooks/useTrainerTypeCatalog';
import { deriveTargetHomePath } from '../../../utils/steam';
import { useProfilesCollectionState } from './useProfilesCollectionState';
import { useProfilesPageProton } from './useProfilesPageProton';

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

  const actions = useProfileActions({ setPendingLauncherReExport });

  const {
    batchValidate: _batchValidate,
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

  const healthIssuesRef = useRef<HTMLDivElement>(null);

  const selectedReport = selectedProfile ? healthByName[selectedProfile] : undefined;
  const selectedCachedSnapshot = selectedProfile ? cachedSnapshots[selectedProfile] : undefined;
  const selectedStaleInfo = selectedProfile ? staleInfoByName[selectedProfile] : undefined;
  const selectedTrend = selectedProfile ? (trendByName[selectedProfile] ?? null) : null;
  const selectedVersionStatus = selectedReport?.metadata?.version_status ?? null;
  const selectedTrainerVersion = selectedReport?.metadata?.trainer_version ?? null;
  const hasSelectedProfile = selectedProfile.trim().length > 0;
  const selectedOfflineReport = selectedProfile ? offlineReadiness.reportForProfile(selectedProfile) : undefined;

  const trainerTypeDisplayName = useMemo(() => {
    const id = profile.trainer?.trainer_type?.trim() || 'unknown';
    return trainerTypeLabels[id] ?? id;
  }, [profile.trainer?.trainer_type, trainerTypeLabels]);

  return {
    ...collectionState,
    ...actions,
    ...protonState,
    cancelDelete,
    confirmDelete,
    deleting,
    dirty,
    duplicateProfile,
    duplicating,
    effectiveSteamClientInstallPath,
    error,
    executeDelete,
    fetchConfigDiff,
    fetchConfigHistory,
    hasSelectedProfile,
    healthIssuesRef,
    healthLoading,
    launchMethod,
    loading,
    markKnownGood,
    pendingDelete,
    pendingLauncherReExport,
    profile,
    profileExists,
    profileName,
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
    summary,
    targetHomePath,
    trainerTypeDisplayName,
    updateProfile,
  };
}
