import type { CollectionDefaults } from '@/types/profile';

/**
 * Mirror of Rust `CollectionRow` in
 * crates/crosshook-core/src/metadata/models.rs.
 *
 * Field names use snake_case to match serde's default serialization, matching
 * the convention in `types/launcher.ts` and others.
 */
export interface CollectionRow {
  collection_id: string;
  name: string;
  description: string | null;
  profile_count: number;
  created_at: string;
  updated_at: string;
}

/** Phase 4: wire-format profile descriptor inside `*.crosshook-collection.toml`. */
export interface CollectionPresetProfileDescriptor {
  steam_app_id: string;
  game_name: string;
  trainer_community_trainer_sha256: string;
}

/** Phase 4: parsed preset manifest (Tauri / preview). */
export interface CollectionPresetManifest {
  schema_version: string;
  name: string;
  description?: string | null;
  defaults?: CollectionDefaults | null;
  profiles: CollectionPresetProfileDescriptor[];
}

export interface CollectionPresetMatchedEntry {
  descriptor: CollectionPresetProfileDescriptor;
  local_profile_name: string;
}

export interface CollectionPresetMatchCandidate {
  profile_name: string;
  game_name: string;
  steam_app_id: string;
}

export interface CollectionPresetAmbiguousEntry {
  descriptor: CollectionPresetProfileDescriptor;
  candidates: CollectionPresetMatchCandidate[];
}

/** Phase 4: read-only import preview from `collection_import_from_toml`. */
export interface CollectionImportPreview {
  source_path: string;
  manifest: CollectionPresetManifest;
  matched: CollectionPresetMatchedEntry[];
  ambiguous: CollectionPresetAmbiguousEntry[];
  unmatched: CollectionPresetProfileDescriptor[];
}

export interface CollectionExportResult {
  collection_id: string;
  output_path: string;
  manifest: CollectionPresetManifest;
}
