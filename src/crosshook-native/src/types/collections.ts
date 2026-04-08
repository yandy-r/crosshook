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
