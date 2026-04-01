export type LibraryViewMode = 'grid' | 'list';

export interface LibraryCardData {
  name: string;
  gameName: string;
  steamAppId: string;
  customCoverArtPath?: string;
  isFavorite: boolean;
}
