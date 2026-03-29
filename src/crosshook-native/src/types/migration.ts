export type ProtonPathField = 'steam_proton_path' | 'runtime_proton_path';
export type MigrationOutcome = 'applied' | 'already_valid' | 'failed';

export interface MigrationSuggestion {
  profile_name: string;
  field: ProtonPathField;
  old_path: string;
  new_path: string;
  old_proton_name: string;
  new_proton_name: string;
  confidence: number;
  proton_family: string;
  crosses_major_version: boolean;
}

export interface UnmatchedProfile {
  profile_name: string;
  field: ProtonPathField;
  stale_path: string;
  stale_proton_name: string;
}

export interface ProtonInstallInfo {
  name: string;
  path: string;
  is_official: boolean;
}

export interface MigrationScanResult {
  suggestions: MigrationSuggestion[];
  unmatched: UnmatchedProfile[];
  profiles_scanned: number;
  affected_count: number;
  installed_proton_versions: ProtonInstallInfo[];
  diagnostics: string[];
}

export interface MigrationApplyResult {
  profile_name: string;
  field: ProtonPathField;
  old_path: string;
  new_path: string;
  outcome: MigrationOutcome;
  error: string | null;
}

export interface ApplyMigrationRequest {
  profile_name: string;
  field: ProtonPathField;
  new_path: string;
}

export interface BatchMigrationRequest {
  migrations: ApplyMigrationRequest[];
}

export interface BatchMigrationResult {
  results: MigrationApplyResult[];
  applied_count: number;
  failed_count: number;
  skipped_count: number;
}
