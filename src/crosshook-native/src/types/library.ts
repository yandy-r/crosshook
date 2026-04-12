export type LibraryViewMode = 'grid' | 'list';

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

