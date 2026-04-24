import type { HeroDetailTabId } from '@/components/library/hero-detail-model';
import type { LibraryFilterKey } from './library';

export interface AppNavigateOptions {
  libraryFilter?: LibraryFilterKey;
  heroDetailTab?: HeroDetailTabId;
  profileName?: string;
}

export interface LibraryFilterIntent {
  filterKey: LibraryFilterKey;
  token: number;
}
