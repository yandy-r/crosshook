import type { GameProfile } from '../../types/profile';

export type SteamFieldState = 'Idle' | 'Saved' | 'NotFound' | 'Found' | 'Ambiguous';

export interface SteamAutoPopulateRequest {
  game_path: string;
  steam_client_install_path: string;
}

export interface SteamAutoPopulateResult {
  app_id_state: SteamFieldState;
  app_id: string;
  compatdata_state: SteamFieldState;
  compatdata_path: string;
  proton_state: SteamFieldState;
  proton_path: string;
  diagnostics: string[];
  manual_hints: string[];
}

export interface CommunityImportResolutionSummary {
  autoResolvedCount: number;
  unresolvedCount: number;
}

export type ProfileUpdateHandler = (updater: (current: GameProfile) => GameProfile) => void;
