/**
 * `profiles-changed` event payload. Mirrors `crosshook_app::ProfileChangeReason`
 * (`#[serde(tag = "kind", rename_all = "snake_case")]`) and the Tauri bridge's
 * `ProfilesChangedPayload`.
 */
export type ProfileChangeReason =
  | { kind: 'saved' }
  | { kind: 'deleted' }
  | { kind: 'renamed'; from: string }
  | { kind: 'duplicated' }
  | { kind: 'favorite_toggled' }
  | { kind: 'rolled_back' }
  | { kind: 'preset_applied' }
  | { kind: 'imported' }
  | { kind: 'migrated' };

export type ProfileChangeKind = ProfileChangeReason['kind'];

/**
 * Rename: `names` = [new], `reason.from` = old. Delete: [removed].
 * Duplicate: [copy]. `names: []` is reserved for conservative resyncs.
 */
export interface ProfilesChangedPayload {
  reason: ProfileChangeReason;
  names: string[];
}
