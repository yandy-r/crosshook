export type VersionMatchStatus = 'exact' | 'compatible' | 'newer_available' | 'outdated' | 'unknown';

export interface TrainerSearchQuery {
  query: string;
  compatibilityFilter?: string;
  platformFilter?: string;
  limit?: number;
  offset?: number;
}

export interface TrainerSearchResult {
  id: number;
  gameName: string;
  steamAppId?: number;
  sourceName: string;
  sourceUrl: string;
  trainerVersion?: string;
  gameVersion?: string;
  notes?: string;
  sha256?: string;
  relativePath: string;
  tapUrl: string;
  tapLocalPath: string;
  relevanceScore: number;
}

export interface TrainerSearchResponse {
  results: TrainerSearchResult[];
  totalCount: number;
}

export interface VersionMatchResult {
  status: VersionMatchStatus;
  trainerGameVersion?: string;
  installedGameVersion?: string;
  detail?: string;
}
