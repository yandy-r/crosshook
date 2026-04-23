import { useCallback, useEffect, useMemo, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import { usePreferencesContext } from '../../../context/PreferencesContext';
import { useProfileContext } from '../../../context/ProfileContext';
import { useCollectionMembers } from '../../../hooks/useCollectionMembers';
import { useCollections } from '../../../hooks/useCollections';
import { useLaunchPlatformStatus } from '../../../hooks/useLaunchPlatformStatus';
import type { ProfileSummary } from '../../../types/library';
import { resolveArtAppId } from '../../../utils/art';
import { buildProfileLaunchRequest } from '../../../utils/launch';

export function useLaunchPageState() {
  const profileState = useProfileContext();
  const launchPlatform = useLaunchPlatformStatus();
  const [profileNetworkIsolation, setProfileNetworkIsolation] = useState<Record<string, boolean>>({});
  const { activeCollectionId, setActiveCollectionId } = profileState;
  const { collections } = useCollections();
  const { memberNames, membersForCollectionId, loading: membersLoading } = useCollectionMembers(activeCollectionId);

  const activeCollection = useMemo(
    () =>
      activeCollectionId === null ? null : (collections.find((c) => c.collection_id === activeCollectionId) ?? null),
    [collections, activeCollectionId]
  );

  const filteredProfiles = useMemo(() => {
    if (activeCollectionId === null) {
      return profileState.profiles;
    }
    if (membersForCollectionId !== activeCollectionId || membersLoading) {
      return [];
    }
    if (memberNames.length === 0) {
      return [];
    }
    const set = new Set(memberNames);
    return profileState.profiles.filter((name) => set.has(name));
  }, [profileState.profiles, activeCollectionId, memberNames, membersForCollectionId, membersLoading]);

  // Auto-deselect profile when it falls outside the active collection filter
  useEffect(() => {
    if (activeCollectionId === null) {
      return;
    }
    if (membersLoading || membersForCollectionId !== activeCollectionId) {
      return;
    }
    const sel = profileState.selectedProfile.trim();
    if (filteredProfiles.length === 0) {
      if (sel !== '') {
        void profileState.selectProfile('');
      }
      return;
    }
    if (sel !== '' && !filteredProfiles.includes(sel)) {
      void profileState.selectProfile(filteredProfiles[0], {
        collectionId: activeCollectionId ?? undefined,
      });
    }
  }, [
    activeCollectionId,
    membersLoading,
    membersForCollectionId,
    filteredProfiles,
    profileState.selectedProfile,
    profileState.selectProfile,
  ]);

  // Fetch network-isolation summaries when the collection filter changes
  useEffect(() => {
    let active = true;
    const collectionId = activeCollectionId?.trim() || undefined;
    void callCommand<ProfileSummary[]>('profile_list_summaries', { collectionId })
      .then((rows) => {
        if (!active) {
          return;
        }
        const next: Record<string, boolean> = {};
        for (const row of rows) {
          next[row.name] = row.networkIsolation;
        }
        setProfileNetworkIsolation(next);
      })
      .catch(() => {
        if (active) {
          setProfileNetworkIsolation({});
        }
      });
    return () => {
      active = false;
    };
  }, [activeCollectionId]);

  const showNetworkIsolationBadge = useCallback(
    (profileName: string) => {
      if (!launchPlatform || launchPlatform.unshareNetAvailable || !profileName.trim()) {
        return false;
      }
      return profileNetworkIsolation[profileName] === true;
    },
    [launchPlatform, profileNetworkIsolation]
  );

  const { settings, defaultSteamClientInstallPath } = usePreferencesContext();

  const profile = profileState.profile;
  const selectedName = profileState.selectedProfile || '';

  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || profileState.steamClientInstallPath,
    [defaultSteamClientInstallPath, profileState.steamClientInstallPath]
  );

  const launchRequest = buildProfileLaunchRequest(
    profile,
    profileState.launchMethod,
    effectiveSteamClientInstallPath,
    selectedName,
    settings.umu_preference
  );

  const profileId = profileState.profileName.trim() || selectedName || 'new-profile';

  const pinnedSet = useMemo(() => new Set(profileState.favoriteProfiles), [profileState.favoriteProfiles]);

  const handleTogglePin = useCallback(
    (value: string) => {
      void profileState.toggleFavorite(value, !pinnedSet.has(value));
    },
    [pinnedSet, profileState.toggleFavorite]
  );

  const optimizationPresetNames = useMemo(
    () => Object.keys(profile.launch.presets ?? {}).sort((a, b) => a.localeCompare(b)),
    [profile.launch.presets]
  );

  const hasSavedSelectedProfile = useMemo(() => {
    const trimmedProfileName = profileState.profileName.trim();
    const trimmedSelectedProfile = profileState.selectedProfile.trim();
    return (
      trimmedProfileName.length > 0 &&
      trimmedSelectedProfile.length > 0 &&
      trimmedProfileName === trimmedSelectedProfile &&
      profileState.profiles.includes(trimmedProfileName)
    );
  }, [profileState.profileName, profileState.profiles, profileState.selectedProfile]);

  const resolvedSteamAppId = resolveArtAppId(profile);

  return {
    activeCollection,
    activeCollectionId,
    effectiveSteamClientInstallPath,
    filteredProfiles,
    hasSavedSelectedProfile,
    handleTogglePin,
    launchRequest,
    optimizationPresetNames,
    pinnedSet,
    profile,
    profileId,
    profileState,
    resolvedSteamAppId,
    selectedName,
    setActiveCollectionId,
    settings,
    showNetworkIsolationBadge,
  };
}
