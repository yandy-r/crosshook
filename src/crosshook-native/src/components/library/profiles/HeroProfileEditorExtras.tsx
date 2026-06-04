/**
 * Supporting UI components for the Hero Detail profile editor that mirror
 * ProfilesPage.tsx affordances:
 *
 * - HeroProfileEditorHealthSection: health issues list + per-card badge scroll
 *   target + stale-check note + trainer-type / version-status / network-isolation
 *   badges (mirrors ProfilesPage.tsx:28-153).
 *
 * - HeroProfileEditorSuggestionBanner: community-recommended Proton runtime
 *   suggestion with install CTA (mirrors ProfilesPage.tsx:173-217).
 */
import type { RefObject } from 'react';
import type { TrendDirection } from '@/hooks/useProfileHealth';
import type { CachedHealthSnapshot, EnrichedProfileHealthReport } from '@/types/health';
import type { ProtonUpSuggestion } from '@/types/protonup';
import type { VersionCorrelationStatus } from '@/types/version';
import { HealthBadge } from '../../HealthBadge';
import {
  NETWORK_ISOLATION_BADGE,
  NETWORK_ISOLATION_BADGE_TITLE,
  VERSION_STATUS_LABELS,
} from '../../pages/profiles/constants';
import { CollapsibleSection } from '../../ui/CollapsibleSection';

// ── Health section ────────────────────────────────────────────────────────────

export interface HeroProfileEditorHealthSectionProps {
  /** Populated when the selected profile has a live health report. */
  selectedReport?: EnrichedProfileHealthReport;
  /** Populated when only a cached snapshot is available (no live report). */
  selectedCachedSnapshot?: CachedHealthSnapshot;
  /** Trend direction derived from healthByName + cachedSnapshots. */
  selectedTrend?: TrendDirection | null;
  /** Stale info for the selected profile (from useProfileHealthContext). */
  staleInfo?: { isStale: boolean; daysAgo: number };
  /** Display name for the trainer type catalog entry. */
  trainerTypeDisplayName?: string;
  /** Whether the trainer has a non-empty path — determines if type chip is shown. */
  hasTrainerPath?: boolean;
  /** Whether this profile uses a non-native launch method. */
  isNonNativeLaunch?: boolean;
  /** Whether the system network-isolation badge should be shown. */
  showNetworkIsolationBadge?: boolean;
  /** Version-status value from the live health report metadata. */
  versionStatus?: VersionCorrelationStatus | null;
  /**
   * Ref passed to the health-issues container so that the per-card badge
   * `onClick` can scroll here — mirrors ProfilesPage.tsx:82.
   */
  healthIssuesRef?: RefObject<HTMLDivElement>;
}

/**
 * Health badges row + health issues collapsible list for the Hero Detail
 * profile editor. Mirrors the status-badge and issues sections of
 * ProfilesPage.tsx (lines ~28-99, 149-162).
 */
export function HeroProfileEditorHealthSection({
  selectedReport,
  selectedCachedSnapshot,
  selectedTrend,
  staleInfo,
  trainerTypeDisplayName,
  hasTrainerPath = false,
  isNonNativeLaunch = false,
  showNetworkIsolationBadge = false,
  versionStatus,
  healthIssuesRef,
}: HeroProfileEditorHealthSectionProps) {
  const hasAnyHealthData = Boolean(selectedReport ?? selectedCachedSnapshot);

  // Version-status badge — mirrors ProfilesPage.tsx:28-46
  const renderVersionStatusBadge = () => {
    if (!versionStatus || versionStatus === 'untracked' || versionStatus === 'unknown' || versionStatus === 'matched') {
      return null;
    }
    const isWarning =
      versionStatus === 'game_updated' || versionStatus === 'trainer_changed' || versionStatus === 'both_changed';
    return (
      <span
        className={`crosshook-status-chip crosshook-version-badge crosshook-version-badge--${isWarning ? 'warning' : 'info'}`}
        title={
          isWarning ? 'Version mismatch detected since last successful launch' : 'Steam is currently updating this game'
        }
      >
        {VERSION_STATUS_LABELS[versionStatus] ?? versionStatus}
      </span>
    );
  };

  // Health badge with issue-scroll affordance — mirrors ProfilesPage.tsx:55-98
  const renderHealthBadge = () => {
    if (!hasAnyHealthData) {
      return null;
    }

    if (selectedReport) {
      const issueCount = selectedReport.issues.length;
      const issueTooltip =
        issueCount > 0
          ? `${issueCount} issue${issueCount !== 1 ? 's' : ''}: ${selectedReport.issues
              .slice(0, 3)
              .map((issue) => `${issue.field} — ${issue.message}`)
              .join('; ')}${issueCount > 3 ? ` (+${issueCount - 3} more)` : ''}`
          : null;

      return (
        <HealthBadge
          report={selectedReport}
          metadata={selectedReport.metadata}
          trend={selectedTrend}
          tooltip={issueTooltip}
          onClick={
            issueCount > 0
              ? () => healthIssuesRef?.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
              : undefined
          }
        />
      );
    }

    const badgeStatus = selectedCachedSnapshot?.status;
    if (!selectedCachedSnapshot || !badgeStatus) {
      return null;
    }
    const issueCount = selectedCachedSnapshot.issue_count;
    const issueTooltip = issueCount > 0 ? `${issueCount} issue${issueCount !== 1 ? 's' : ''} in cached snapshot` : null;
    return <HealthBadge status={badgeStatus} trend={selectedTrend} tooltip={issueTooltip} />;
  };

  // Stale-check note — mirrors ProfilesPage.tsx:149-153
  const renderStaleNote = () => {
    if (!selectedReport && staleInfo?.isStale) {
      return (
        <span className="crosshook-status-chip crosshook-status-chip--muted" role="note">
          Checked {staleInfo.daysAgo}d ago
        </span>
      );
    }
    return null;
  };

  const hasBadges =
    hasAnyHealthData ||
    Boolean(renderVersionStatusBadge()) ||
    (isNonNativeLaunch && hasTrainerPath) ||
    showNetworkIsolationBadge ||
    Boolean(renderStaleNote());

  if (!hasBadges && !selectedReport) {
    return null;
  }

  return (
    <div className="crosshook-hero-detail__health-block">
      {/* Status-badges row — mirrors ProfilesPage.tsx:125-154 */}
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, alignItems: 'center' }}>
        {renderHealthBadge()}
        {isNonNativeLaunch && hasTrainerPath ? (
          <span className="crosshook-status-chip" title="Trainer type catalog id for offline scoring">
            Trainer type: {trainerTypeDisplayName}
          </span>
        ) : null}
        {renderVersionStatusBadge()}
        {showNetworkIsolationBadge ? (
          <span
            className="crosshook-status-chip crosshook-version-badge crosshook-version-badge--warning"
            title={NETWORK_ISOLATION_BADGE_TITLE}
          >
            {NETWORK_ISOLATION_BADGE}
          </span>
        ) : null}
        {renderStaleNote()}
      </div>

      {/* Health issues list — mirrors ProfilesPage.tsx:158-161 (ProfilesHealthIssues) */}
      {selectedReport &&
      (selectedReport.status === 'broken' || selectedReport.status === 'stale') &&
      selectedReport.issues.length > 0 ? (
        <div ref={healthIssuesRef}>
          <CollapsibleSection title="Health Issues" className="crosshook-panel">
            <ul style={{ margin: 0, padding: 0, listStyle: 'none', display: 'grid', gap: 8 }}>
              {selectedReport.issues.map((issue) => (
                <li
                  key={`${selectedReport.name}-${issue.field}-${issue.path}-${issue.message}-${issue.severity}`}
                  style={{ borderLeft: '3px solid var(--crosshook-danger, #ef4444)', paddingLeft: 10 }}
                >
                  <strong>{issue.field}</strong>
                  {issue.path ? <span className="crosshook-muted"> — {issue.path}</span> : null}
                  <p style={{ margin: '2px 0' }}>{issue.message}</p>
                  {issue.remediation ? (
                    <p className="crosshook-help-text" style={{ margin: '2px 0' }}>
                      {issue.remediation}
                    </p>
                  ) : null}
                </li>
              ))}
            </ul>
          </CollapsibleSection>
        </div>
      ) : null}
    </div>
  );
}

// ── Runtime suggestion banner ─────────────────────────────────────────────────

export interface HeroProfileEditorSuggestionBannerProps {
  /** Community Proton version suggestion (from useProfilesPageProton). */
  suggestion: ProtonUpSuggestion | null;
  suggestionDismissed: boolean;
  suggestionInstallError: string | null;
  protonUpInstalling: boolean;
  /** Whether a steam client install path is known (enables install button). */
  hasEffectiveSteamClientInstallPath: boolean;
  onInstallSuggestedVersion: () => void;
  onDismissSuggestion: () => void;
}

/**
 * Runtime suggestion banner — community-recommended Proton version CTA.
 * Mirrors ProfilesPage.tsx:173-217.
 */
export function HeroProfileEditorSuggestionBanner({
  suggestion,
  suggestionDismissed,
  suggestionInstallError,
  protonUpInstalling,
  hasEffectiveSteamClientInstallPath,
  onInstallSuggestedVersion,
  onDismissSuggestion,
}: HeroProfileEditorSuggestionBannerProps) {
  if (!suggestion || suggestion.status !== 'missing' || suggestionDismissed) {
    return null;
  }

  const communityVersion = suggestion.community_version ?? suggestion.recommended_version ?? '';

  return (
    <div className="crosshook-panel crosshook-protonup-recommendation" role="status">
      <div className="crosshook-protonup-recommendation__content">
        <span className="crosshook-protonup-recommendation__icon" aria-hidden="true">
          &#9888;
        </span>
        <div className="crosshook-protonup-recommendation__text">
          <strong>Runtime suggestion</strong>
          <p className="crosshook-help-text" style={{ margin: '4px 0 0' }}>
            This community profile recommends
            {communityVersion ? (
              <>
                {' '}
                <strong>{communityVersion}</strong>,
              </>
            ) : null}{' '}
            which is not currently installed. You can still launch with your current runtime.
          </p>
        </div>
      </div>
      <div
        className="crosshook-protonup-recommendation__actions"
        style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginTop: 10 }}
      >
        <button
          type="button"
          className="crosshook-button crosshook-button--small crosshook-button--primary"
          onClick={onInstallSuggestedVersion}
          disabled={protonUpInstalling || !suggestion.recommended_version || !hasEffectiveSteamClientInstallPath}
        >
          {protonUpInstalling ? 'Installing…' : 'Install recommended'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--small crosshook-button--ghost"
          onClick={onDismissSuggestion}
        >
          Dismiss
        </button>
      </div>
      {suggestionInstallError ? (
        <p className="crosshook-danger" role="alert" style={{ margin: '8px 0 0' }}>
          {suggestionInstallError}
        </p>
      ) : null}
    </div>
  );
}
