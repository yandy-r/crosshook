export type LibraryViewMode = 'grid' | 'list';

export interface ProfileSummary {
  name: string;
  gameName: string;
  steamAppId: string;
  customCoverArtPath?: string;
  customPortraitArtPath?: string;
}

export interface LibraryCardData {
  name: string;
  gameName: string;
  steamAppId: string;
  customCoverArtPath?: string;
  customPortraitArtPath?: string;
  isFavorite: boolean;
}
