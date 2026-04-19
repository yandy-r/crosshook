import { useCallback, useEffect, useMemo } from 'react';

interface CollectionSummary {
  collection_id: string;
  name: string;
}

interface UseProfilesCollectionStateArgs {
  activeCollectionId: string | null;
  collections: CollectionSummary[];
  memberNames: string[];
  membersForCollectionId: string | null;
  membersLoading: boolean;
  profileNetworkIsolation: Record<string, boolean>;
  profileUsesNetworkIsolation: boolean;
  profiles: string[];
  selectedProfile: string;
  selectProfile: (name: string) => Promise<void>;
  systemCanUnshareNet: boolean | null;
}

export function useProfilesCollectionState({
  activeCollectionId,
  collections,
  memberNames,
  membersForCollectionId,
  membersLoading,
  profileNetworkIsolation,
  profileUsesNetworkIsolation,
  profiles,
  selectedProfile,
  selectProfile,
  systemCanUnshareNet,
}: UseProfilesCollectionStateArgs) {
  const activeCollection = useMemo(
    () =>
      activeCollectionId === null
        ? null
        : (collections.find((entry) => entry.collection_id === activeCollectionId) ?? null),
    [collections, activeCollectionId]
  );

  const filteredProfiles = useMemo(() => {
    if (activeCollectionId === null) {
      return profiles;
    }
    if (membersLoading || membersForCollectionId !== activeCollectionId) {
      return [];
    }
    if (memberNames.length === 0) {
      return [];
    }
    const set = new Set(memberNames);
    return profiles.filter((name) => set.has(name));
  }, [profiles, activeCollectionId, memberNames, membersLoading, membersForCollectionId]);

  useEffect(() => {
    if (activeCollectionId === null) {
      return;
    }
    if (membersLoading || membersForCollectionId !== activeCollectionId) {
      return;
    }
    const selected = selectedProfile.trim();
    if (filteredProfiles.length === 0) {
      if (selected !== '') {
        void selectProfile('');
      }
      return;
    }
    if (selected !== '' && !filteredProfiles.includes(selected)) {
      void selectProfile(filteredProfiles[0]);
    }
  }, [activeCollectionId, membersLoading, membersForCollectionId, filteredProfiles, selectedProfile, selectProfile]);

  const showNetworkIsolationBadge = useCallback(
    (candidateProfileName: string) => {
      if (systemCanUnshareNet !== false || !candidateProfileName.trim()) {
        return false;
      }

      if (candidateProfileName.trim() === selectedProfile.trim()) {
        return profileUsesNetworkIsolation;
      }

      return profileNetworkIsolation[candidateProfileName] === true;
    },
    [profileNetworkIsolation, profileUsesNetworkIsolation, selectedProfile, systemCanUnshareNet]
  );

  return {
    activeCollection,
    filteredProfiles,
    showNetworkIsolationBadge,
  };
}
