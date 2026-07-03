export type VersionMatchStatus = 'exact' | 'compatible' | 'newer_available' | 'outdated' | 'unknown';

export interface CatalogQuery {
  query?: string;
  gameTitles?: string[];
  loadingModes?: string[];
  compatibilityBands?: string[];
  tapUrls?: string[];
  limit?: number;
  offset?: number;
}

export interface CatalogSource {
  sourceName: string;
  sourceUrl: string;
  sha256?: string | null;
  trainerVersion?: string | null;
  gameVersion?: string | null;
  notes?: string | null;
}

export interface CatalogEntry {
  id: number | null;
  tapUrl: string;
  tapLocalPath: string;
  relativePath: string;
  /** Empty for source-only entries (no community profile to import). */
  manifestPath: string;
  gameName?: string | null;
  gameVersion?: string | null;
  trainerName?: string | null;
  trainerVersion?: string | null;
  protonVersion?: string | null;
  compatibilityRating?: string | null;
  author?: string | null;
  description?: string | null;
  platformTags?: string | null;
  trainerLoadingMode?: string | null;
  schemaVersion: number;
  sources: CatalogSource[];
}

export interface CatalogFacetValue {
  value: string;
  count: number;
}

export interface CatalogFacets {
  gameTitles: CatalogFacetValue[];
  loadingModes: CatalogFacetValue[];
  compatibilityBands: CatalogFacetValue[];
  taps: CatalogFacetValue[];
}

export interface CatalogPage {
  entries: CatalogEntry[];
  facets: CatalogFacets;
  totalCount: number;
  tapCount: number;
  degraded: boolean;
}

export interface VersionMatchResult {
  status: VersionMatchStatus;
  trainerGameVersion?: string;
  installedGameVersion?: string;
  detail?: string;
}

// Phase B: External trainer search types

export interface ExternalTrainerSearchQuery {
  gameName: string;
  steamAppId?: string;
  forceRefresh?: boolean;
}

export interface ExternalTrainerResult {
  gameName: string;
  sourceName: string;
  sourceUrl: string;
  pubDate?: string;
  source: string;
  relevanceScore: number;
}

export interface ExternalTrainerSearchResponse {
  results: ExternalTrainerResult[];
  source: string;
  cached: boolean;
  cacheAgeSecs?: number;
  isStale: boolean;
  offline: boolean;
}

export interface ExternalTrainerSourceSubscription {
  sourceId: string;
  displayName: string;
  baseUrl: string;
  sourceType: string;
  enabled: boolean;
}
