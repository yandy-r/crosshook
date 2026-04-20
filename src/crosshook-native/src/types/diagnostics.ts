import type { LaunchValidationSeverity } from './launch';

const FAILURE_MODES = [
  'clean_exit',
  'non_zero_exit',
  'segfault',
  'abort',
  'kill',
  'bus_error',
  'illegal_instruction',
  'floating_point_exception',
  'broken_pipe',
  'terminated',
  'command_not_found',
  'permission_denied',
  'unknown_signal',
  'indeterminate',
  'unknown',
] as const;

export type FailureMode = (typeof FAILURE_MODES)[number];

export interface ExitCodeInfo {
  code: number | null;
  signal: number | null;
  signal_name: string | null;
  core_dumped: boolean;
  failure_mode: FailureMode;
  description: string;
  severity: LaunchValidationSeverity;
}

export interface PatternMatch {
  pattern_id: string;
  summary: string;
  severity: LaunchValidationSeverity;
  matched_line: string | null;
  suggestion: string;
}

export interface ActionableSuggestion {
  title: string;
  description: string;
  severity: LaunchValidationSeverity;
}

export const TEARDOWN_REASONS = [
  'natural_exit',
  'watchdog_natural_exit',
  'linked_session_exit',
  'user_request',
  'receiver_closed',
] as const;

export type TeardownReason = (typeof TEARDOWN_REASONS)[number];

export interface DiagnosticReport {
  severity: LaunchValidationSeverity;
  summary: string;
  exit_info: ExitCodeInfo;
  pattern_matches: PatternMatch[];
  suggestions: ActionableSuggestion[];
  launch_method: string;
  log_tail_path: string | null;
  analyzed_at: string;
  /**
   * Populated by the Rust stream finalizer to record why this launch was
   * torn down — set by the gamescope watchdog when it fires, or by the
   * cancel-drain path for launches that have no gamescope tree (e.g. a
   * non-gamescope trainer cascaded by its parent game). Absent from
   * pre-#230 `launch_operations.diagnostic_json` rows.
   */
  teardown_reason?: TeardownReason;
}

function isSeverity(value: unknown): value is LaunchValidationSeverity {
  return value === 'fatal' || value === 'warning' || value === 'info';
}

function isPatternMatch(value: unknown): value is PatternMatch {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Partial<PatternMatch>;

  return (
    typeof candidate.pattern_id === 'string' &&
    typeof candidate.summary === 'string' &&
    isSeverity(candidate.severity) &&
    (candidate.matched_line === null || typeof candidate.matched_line === 'string') &&
    typeof candidate.suggestion === 'string'
  );
}

function isActionableSuggestion(value: unknown): value is ActionableSuggestion {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Partial<ActionableSuggestion>;

  return (
    typeof candidate.title === 'string' && typeof candidate.description === 'string' && isSeverity(candidate.severity)
  );
}

function isExitCodeInfo(value: unknown): value is ExitCodeInfo {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Partial<ExitCodeInfo>;

  return (
    (candidate.code === null || typeof candidate.code === 'number') &&
    (candidate.signal === null || typeof candidate.signal === 'number') &&
    (candidate.signal_name === null || typeof candidate.signal_name === 'string') &&
    typeof candidate.core_dumped === 'boolean' &&
    typeof candidate.failure_mode === 'string' &&
    FAILURE_MODES.includes(candidate.failure_mode as FailureMode) &&
    typeof candidate.description === 'string' &&
    isSeverity(candidate.severity)
  );
}

export function isDiagnosticReport(value: unknown): value is DiagnosticReport {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Partial<DiagnosticReport>;

  return (
    isSeverity(candidate.severity) &&
    typeof candidate.summary === 'string' &&
    isExitCodeInfo(candidate.exit_info) &&
    Array.isArray(candidate.pattern_matches) &&
    candidate.pattern_matches.every(isPatternMatch) &&
    Array.isArray(candidate.suggestions) &&
    candidate.suggestions.every(isActionableSuggestion) &&
    typeof candidate.launch_method === 'string' &&
    (candidate.log_tail_path === null || typeof candidate.log_tail_path === 'string') &&
    typeof candidate.analyzed_at === 'string' &&
    (candidate.teardown_reason === undefined || TEARDOWN_REASONS.includes(candidate.teardown_reason as TeardownReason))
  );
}

// -- Diagnostic bundle export types --

export interface DiagnosticBundleSummary {
  crosshook_version: string;
  profile_count: number;
  log_file_count: number;
  proton_install_count: number;
  generated_at: string;
}

export interface DiagnosticBundleResult {
  archive_path: string;
  summary: DiagnosticBundleSummary;
}
