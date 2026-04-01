export type SteamMetadataLookupState = 'idle' | 'loading' | 'ready' | 'stale' | 'unavailable';

export interface SteamGenre {
  id: string;
  description: string;
}

export interface SteamAppDetails {
  name: string | null;
  short_description: string | null;
  header_image: string | null;
  genres: SteamGenre[];
}

export interface SteamMetadataLookupResult {
  app_id: string;
  state: SteamMetadataLookupState;
  app_details: SteamAppDetails | null;
  from_cache: boolean;
  is_stale: boolean;
}
