export type LibraryViewMode = 'grid' | 'list';

export type LibrarySortKey = 'recent' | 'name' | 'lastPlayed' | 'playtime';

export type LibraryFilterKey = 'all' | 'favorites' | 'installed' | 'recentlyLaunched';

export interface ProfileSummary {
  name: string;
  gameName: string;
  steamAppId: string;
  customCoverArtPath?: string;
  customPortraitArtPath?: string;
  /** Effective launch network isolation (for Flatpak capability UI). */
  networkIsolation: boolean;
}

export interface LibraryCardData extends ProfileSummary {
  isFavorite: boolean;
}

/** Value equality for every field rendered by the library inspector / cards. */
export function libraryCardDataEqual(a: LibraryCardData, b: LibraryCardData): boolean {
  return (
    a.name === b.name &&
    a.gameName === b.gameName &&
    a.steamAppId === b.steamAppId &&
    a.isFavorite === b.isFavorite &&
    a.networkIsolation === b.networkIsolation &&
    (a.customCoverArtPath ?? '') === (b.customCoverArtPath ?? '') &&
    (a.customPortraitArtPath ?? '') === (b.customPortraitArtPath ?? '')
  );
}

/** Subset of `launch_operations` from `list_launch_history_for_profile` (no `diagnostic_json`). */
export interface LaunchHistoryEntry {
  operation_id: string;
  launch_method: string;
  status: string;
  started_at: string;
  finished_at: string | null;
  exit_code: number | null;
  signal: number | null;
  severity: string | null;
  failure_mode: string | null;
}
