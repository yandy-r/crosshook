import type { HeroDetailTabId } from '@/components/library/hero-detail-model';
import type { LibraryFilterKey } from './library';

// NOTE(hero-detail-consolidation): delete with Phase 10 route removal.
export interface GameDetailOrigin {
  profileName: string;
  displayName: string;
}

/** Mirrors the LibraryFilterIntent token pattern for reopening a game's Hero Detail. */
export interface OpenGameDetailIntent {
  profileName: string;
  token: number;
}

export interface AppNavigateOptions {
  libraryFilter?: LibraryFilterKey;
  heroDetailTab?: HeroDetailTabId;
  profileName?: string;
  gameDetailOrigin?: GameDetailOrigin; // NOTE(hero-detail-consolidation): delete with Phase 10 route removal.
  /** Profile name whose Hero Detail should reopen in Library. */
  openGameDetail?: string;
}

export interface LibraryFilterIntent {
  filterKey: LibraryFilterKey;
  token: number;
}
