import { useMemo } from 'react';
import type { LibraryCardData } from '../types/library';

export function useLibraryProfiles(profiles: LibraryCardData[], searchQuery: string): LibraryCardData[] {
  return useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return profiles;
    return profiles.filter((p) => p.name.toLowerCase().includes(query) || p.gameName.toLowerCase().includes(query));
  }, [profiles, searchQuery]);
}
