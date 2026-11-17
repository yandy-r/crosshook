import type { HeroDetailTabId } from '@/components/library/hero-detail-model';
import type { LibraryFilterKey } from './library';

/** Mirrors the LibraryFilterIntent token pattern for reopening a game's Hero Detail. */
export interface OpenGameDetailIntent {
  profileName: string;
  heroDetailTab?: HeroDetailTabId;
  token: number;
}

export interface AppNavigateOptions {
  libraryFilter?: LibraryFilterKey;
  heroDetailTab?: HeroDetailTabId;
  profileName?: string;
  /** Profile name whose Hero Detail should reopen in Library. */
  openGameDetail?: string;
}

export interface LibraryFilterIntent {
  filterKey: LibraryFilterKey;
  token: number;
}
