// Mirrors Rust `crosshook_core::mods` Serde DTOs (snake_case wire fields).

export type ModCategory = 'overlay_injection' | 'script_extender' | 'file_replacement' | 'other';
export type ModProvenance = 'manual' | 'detected';

export interface ProfileModRecord {
  mod_id: string;
  profile_id: string;
  name: string;
  category: ModCategory;
  paths: string[];
  enabled: boolean;
  provenance: ModProvenance;
  source_url?: string;
  created_at: string;
  updated_at: string;
}

export interface ProfileModInput {
  name: string;
  category: ModCategory;
  paths: string[];
  enabled: boolean;
  source_url?: string;
  provenance: ModProvenance;
}

export interface DetectedModCandidate {
  detector_id: string;
  suggested_name: string;
  category: ModCategory;
  matched_paths: string[];
  already_registered: boolean;
}

export interface DetectionScanReport {
  scanned_root: string;
  candidates: DetectedModCandidate[];
  entries_scanned: number;
  truncated: boolean;
}

export interface ProfileModsResponse {
  available: boolean;
  mods: ProfileModRecord[];
}
