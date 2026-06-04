export const NETWORK_ISOLATION_BADGE = 'No network isolation';
export const NETWORK_ISOLATION_BADGE_TITLE =
  'This system cannot enforce network isolation (unshare --net). The profile still launches; traffic is not isolated.';

export const RENAME_TOAST_DURATION_MS = 6000;
export const HEALTH_BANNER_DISMISSED_SESSION_KEY = 'crosshook.healthBannerDismissed';
export const RENAME_TOAST_DISMISSED_SESSION_KEY = 'crosshook.renameToastDismissed';

export const VERSION_STATUS_LABELS: Record<string, string> = {
  game_updated: 'Game updated',
  trainer_changed: 'Trainer changed',
  both_changed: 'Both changed',
  update_in_progress: 'Update in progress',
};

/** Minimal shape of a row returned by `community_list_indexed_profiles`. */
export interface CommunityIndexedProfileRow {
  game_name: string | null;
  proton_version: string | null;
}

export interface RenameToast {
  newName: string;
  oldName: string;
}
