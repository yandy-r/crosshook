import type { HealthIssue } from './health';

/** Actionable installation guidance for umu-launcher, mirroring the Rust `UmuInstallGuidance` struct.
 * Present only when running inside a Flatpak sandbox and `umu-run` cannot be resolved on the host.
 */
export interface UmuInstallGuidance {
  /** Host shell command the user can run to install umu-launcher. */
  install_command: string;
  /** URL pointing to official umu-launcher install documentation. */
  docs_url: string;
  /** Human-readable description for the guidance row. */
  description: string;
}

/** Caveats shown when running on a Steam Deck, mirroring the Rust `SteamDeckCaveats` struct. */
export interface SteamDeckCaveats {
  /** Human-readable description for the caveats row. */
  description: string;
  /** List of individual caveat items to display. */
  items: string[];
  /** URL pointing to relevant documentation. */
  docs_url: string;
}

/** One distro-specific install hint row from the host readiness catalog. */
export interface HostToolInstallCommand {
  distro_family: string;
  command: string;
  alternatives: string;
}

/** Result for a single host tool probe (generalized readiness). */
export interface HostToolCheckResult {
  tool_id: string;
  display_name: string;
  is_available: boolean;
  is_required: boolean;
  category: string;
  /** Upstream / project documentation URL from the catalog. */
  docs_url?: string;
  /** Parsed tool version from an optional detail probe. */
  tool_version?: string | null;
  /** Resolved host path from an optional detail probe. */
  resolved_path?: string | null;
  /** Present when the tool is missing and Flatpak host guidance applies. */
  install_guidance: HostToolInstallCommand | null;
}

export type CapabilityState = 'available' | 'degraded' | 'unavailable';

/** Derived capability status backed by one or more host tool probes. */
export interface Capability {
  id: string;
  label: string;
  category: string;
  state: CapabilityState;
  rationale: string | null;
  required_tool_ids: string[];
  optional_tool_ids: string[];
  missing_required: HostToolCheckResult[];
  missing_optional: HostToolCheckResult[];
  install_hints: HostToolInstallCommand[];
}

/** Optional per-tool detail probe used by the host tool dashboard. */
export interface HostToolDetails {
  tool_id: string;
  tool_version: string | null;
  resolved_path: string | null;
}

export interface ReadinessCheckResult {
  checks: HealthIssue[];
  all_passed: boolean;
  critical_failures: number;
  warnings: number;
  /** Actionable umu install guidance; non-null only for Flatpak + missing umu-run. */
  umu_install_guidance: UmuInstallGuidance | null;
  /** Steam Deck-specific caveats; non-null only when running on a Steam Deck. */
  steam_deck_caveats: SteamDeckCaveats | null;
  /** Host tool rows from `check_generalized_readiness` (empty for legacy `check_readiness` unless backend merges). */
  tool_checks?: HostToolCheckResult[];
  /** Detected host distro key from `/etc/os-release` (e.g. `Arch`, `SteamOS`). */
  detected_distro_family?: string;
}

export type OnboardingWizardStage = 'identity_game' | 'runtime' | 'trainer' | 'media' | 'review' | 'completed';

export interface TrainerGuidanceEntry {
  id: string;
  title: string;
  description: string;
  when_to_use: string;
  examples: string[];
}

export interface TrainerGuidanceContent {
  loading_modes: TrainerGuidanceEntry[];
  trainer_sources: TrainerGuidanceEntry[];
  verification_steps: string[];
}

/** Startup event payload for the `onboarding-check` Tauri event. */
export interface OnboardingCheckPayload {
  show: boolean;
  has_profiles: boolean;
}
