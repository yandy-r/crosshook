import type { GameProfile } from './profile';

/** Write-source labels stored with each config revision. */
export type ConfigRevisionSource =
  | 'manual_save'
  | 'rollback_apply'
  | 'import'
  | 'launch_optimization_save'
  | 'preset_apply'
  | 'migration';

/**
 * IPC DTO from `profile_config_history`.
 *
 * One entry per revision in the profile's history timeline, ordered
 * newest-first. Mirrors the Rust `ConfigRevisionSummary` struct.
 */
export interface ConfigRevisionSummary {
  id: number;
  profile_name_at_write: string;
  source: ConfigRevisionSource;
  content_hash: string;
  /** ID of the revision this row was derived from (rollback lineage). */
  source_revision_id: number | null;
  is_last_known_working: boolean;
  /** RFC3339 timestamp. */
  created_at: string;
}

/**
 * IPC DTO from `profile_config_diff`.
 *
 * Contains unified diff text comparing a stored revision against the
 * current profile state or a second specified revision.
 */
export interface ConfigDiffResult {
  revision_id: number;
  revision_source: ConfigRevisionSource;
  /** RFC3339 timestamp of the left-hand revision. */
  revision_created_at: string;
  /** Unified diff text; empty string when the two sides are identical. */
  diff_text: string;
  added_lines: number;
  removed_lines: number;
}

/**
 * IPC DTO from `profile_config_rollback`.
 *
 * Returned after a successful rollback apply. `new_revision_id` is the
 * freshly appended lineage row; `null` when the restored content was
 * identical to the current head and no new row was written.
 */
export interface ConfigRollbackResult {
  restored_revision_id: number;
  new_revision_id: number | null;
  /** Full restored profile state after the rollback write. */
  profile: GameProfile;
}
