/** Mirrors the backend `MetadataStatus` serde shape (`#[serde(tag = "state", rename_all = "snake_case")]`). */
export type MetadataStoreStatus =
  | { state: 'ok' }
  | { state: 'newer_schema'; found: number; supported: number }
  | { state: 'disabled'; reason: string };
