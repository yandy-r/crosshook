import { Fragment, useEffect, useId, useMemo, useRef, useState } from 'react';

import { open as openUrl } from '@/lib/plugin-stubs/shell';

import { useProtonDbLookup } from '../hooks/useProtonDbLookup';
import type {
  AcceptSuggestionRequest,
  CatalogSuggestionItem,
  EnvVarSuggestionItem,
  ProtonDbLookupState,
  ProtonDbRecommendationGroup,
  ProtonDbSuggestionSet,
  ProtonDbTier,
  ProtonDbVersionContext,
} from '../types/protondb';
import { copyToClipboard } from '../utils/clipboard';
import { formatRelativeTime } from '../utils/format';

export interface ProtonDbLookupCardProps {
  appId: string;
  className?: string;
  trainerVersion?: string | null;
  versionContext?: ProtonDbVersionContext | null;
  onApplyEnvVars?: (group: ProtonDbRecommendationGroup) => void;
  applyingGroupId?: string | null;
  suggestionSet?: ProtonDbSuggestionSet | null;
  onAcceptSuggestion?: (request: AcceptSuggestionRequest) => Promise<void>;
  onDismissSuggestion?: (suggestionKey: string) => void;
}

const STATE_LABELS: Record<Exclude<ProtonDbLookupState, 'ready'>, string> = {
  idle: 'Awaiting Steam App ID',
  loading: 'Loading ProtonDB',
  stale: 'Cached result',
  unavailable: 'Unavailable',
};

function formatTierLabel(tier: ProtonDbTier): string {
  return tier
    .split(/[_-]/g)
    .filter(Boolean)
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(' ');
}

function tierClassName(tier: ProtonDbTier): string {
  const normalized = tier.toLowerCase();
  if (['platinum', 'gold', 'silver', 'bronze', 'borked', 'native'].includes(normalized)) {
    return normalized;
  }
  return 'unknown';
}

function stateTone(state: ProtonDbLookupState): 'loading' | 'stale' | 'unavailable' | 'neutral' {
  switch (state) {
    case 'loading':
      return 'loading';
    case 'stale':
      return 'stale';
    case 'unavailable':
      return 'unavailable';
    default:
      return 'neutral';
  }
}

export function ProtonDbLookupCard({
  appId,
  className,
  trainerVersion = null,
  versionContext = null,
  onApplyEnvVars,
  applyingGroupId = null,
  suggestionSet = null,
  onAcceptSuggestion,
  onDismissSuggestion,
}: ProtonDbLookupCardProps) {
  const titleId = useId();
  const [copyLabels, setCopyLabels] = useState<Record<string, string>>({});
  const copyTimeouts = useRef<Map<string, number>>(new Map());
  const lookup = useProtonDbLookup(appId);

  useEffect(() => {
    return () => {
      copyTimeouts.current.forEach((id) => {
        window.clearTimeout(id);
      });
    };
  }, []);
  const { snapshot, cache, recommendationGroups } = lookup;
  const actionableGroups = recommendationGroups;

  const cardClasses = useMemo(() => {
    const classes = ['crosshook-protondb-card'];
    if (snapshot) {
      classes.push(`crosshook-protondb-card--${tierClassName(snapshot.tier)}`);
    } else if (lookup.state !== 'ready') {
      classes.push(`crosshook-protondb-card--${stateTone(lookup.state)}`);
    }
    if (className) {
      classes.push(className);
    }
    return classes.join(' ');
  }, [className, lookup.state, snapshot]);

  const stateClass = `crosshook-protondb-card__state crosshook-protondb-card__state--${stateTone(lookup.state)}`;
  const sourceUrl = snapshot?.source_url?.trim() ?? '';

  const freshnessLabel =
    cache?.fetched_at || snapshot?.fetched_at
      ? formatRelativeTime(cache?.fetched_at || snapshot?.fetched_at || '')
      : null;

  const metaItems = [
    { label: 'Tier', value: snapshot ? formatTierLabel(snapshot.tier) : null },
    { label: 'Reports', value: snapshot?.total_reports != null ? String(snapshot.total_reports) : null },
    { label: 'Confidence', value: snapshot?.confidence ?? null },
    { label: 'Score', value: snapshot?.score != null ? snapshot.score.toFixed(2) : null },
    {
      label: 'Best reported',
      value: snapshot?.best_reported_tier ? formatTierLabel(snapshot.best_reported_tier) : null,
    },
    { label: 'Trending', value: snapshot?.trending_tier ? formatTierLabel(snapshot.trending_tier) : null },
    { label: 'Trainer version', value: trainerVersion ?? null },
    { label: 'Last updated', value: freshnessLabel },
  ].filter((item): item is { label: string; value: string } => item.value !== null);

  async function handleCopy(copyKey: string, text: string) {
    try {
      await copyToClipboard(text);
      setCopyLabels((current) => ({ ...current, [copyKey]: 'Copied' }));
    } catch {
      setCopyLabels((current) => ({ ...current, [copyKey]: 'Copy failed' }));
    }

    const existing = copyTimeouts.current.get(copyKey);
    if (existing) window.clearTimeout(existing);
    const id = window.setTimeout(() => {
      setCopyLabels((current) => ({ ...current, [copyKey]: 'Copy' }));
      copyTimeouts.current.delete(copyKey);
    }, 2000);
    copyTimeouts.current.set(copyKey, id);
  }

  function renderBanner() {
    if (!lookup.appId) {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--neutral">
          <p className="crosshook-protondb-card__banner-copy crosshook-protondb-card__banner-copy--muted">
            Add a Steam App ID to this profile to enable ProtonDB compatibility guidance.
          </p>
        </div>
      );
    }

    if (versionContext?.version_status === 'game_updated' || versionContext?.version_status === 'both_changed') {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--stale">
          <p className="crosshook-protondb-card__banner-copy">
            The installed game build changed since the last successful launch. ProtonDB guidance may be stale until
            newer reports catch up with the updated build.
          </p>
        </div>
      );
    }

    if (versionContext?.version_status === 'update_in_progress') {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--stale">
          <p className="crosshook-protondb-card__banner-copy">
            Steam is currently updating this game. ProtonDB guidance may not match the in-progress build yet.
          </p>
        </div>
      );
    }

    if (lookup.state === 'loading' && !snapshot) {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--loading">
          <p className="crosshook-protondb-card__banner-copy">
            Loading the latest ProtonDB tier and community recommendations.
          </p>
        </div>
      );
    }

    if (lookup.state === 'stale') {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--stale">
          <p className="crosshook-protondb-card__banner-copy">
            Showing cached ProtonDB guidance because the live lookup failed.
          </p>
        </div>
      );
    }

    if (lookup.state === 'unavailable') {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--unavailable">
          <p className="crosshook-protondb-card__banner-copy">
            ProtonDB is unavailable right now. The rest of the profile editor remains fully usable.
          </p>
        </div>
      );
    }

    if (cache?.from_cache) {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--neutral">
          <p className="crosshook-protondb-card__banner-copy crosshook-protondb-card__banner-copy--muted">
            Loaded from the local metadata cache.
          </p>
        </div>
      );
    }

    return null;
  }

  function renderRecommendationGroup(group: ProtonDbRecommendationGroup, index: number) {
    const envVars = group.env_vars ?? [];
    const launchOptions = group.launch_options ?? [];
    const notes = group.notes ?? [];
    const canApplyEnvVars = envVars.length > 0 && onApplyEnvVars != null;
    const groupId = group.group_id?.trim() || group.title?.trim() || `group-${index}`;
    const isApplyingGroup = applyingGroupId != null && applyingGroupId === groupId;

    return (
      <section key={groupId} className="crosshook-protondb-card__recommendation-group">
        <div className="crosshook-protondb-card__meta">
          <h3 className="crosshook-protondb-card__recommendation-group-title">{group.title}</h3>
          {group.summary ? <p className="crosshook-protondb-card__recommendation-group-copy">{group.summary}</p> : null}
        </div>

        {envVars.length > 0 ? (
          <div className="crosshook-protondb-card__recommendation-list">
            {envVars.map((envVar) => {
              const text = `${envVar.key}=${envVar.value}`;
              const copyKey = `${groupId}:${envVar.key}`;
              return (
                <div key={copyKey} className="crosshook-protondb-card__recommendation-item">
                  <p className="crosshook-protondb-card__recommendation-label">
                    <code>{text}</code>
                  </p>
                  <p className="crosshook-protondb-card__recommendation-note">
                    {envVar.source_label}
                    {envVar.supporting_report_count != null
                      ? ` • ${envVar.supporting_report_count} supporting report${
                          envVar.supporting_report_count === 1 ? '' : 's'
                        }`
                      : ''}
                  </p>
                  <div className="crosshook-protondb-card__actions">
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary"
                      onClick={() => void handleCopy(copyKey, text)}
                    >
                      {copyLabels[copyKey] ?? 'Copy'}
                    </button>
                  </div>
                </div>
              );
            })}
            <div className="crosshook-protondb-card__actions">
              <button
                type="button"
                className="crosshook-button"
                disabled={!canApplyEnvVars || isApplyingGroup}
                onClick={() => onApplyEnvVars?.(group)}
              >
                {isApplyingGroup ? 'Applying…' : 'Apply Suggested Env Vars'}
              </button>
            </div>
          </div>
        ) : null}

        {launchOptions.length > 0 ? (
          <div className="crosshook-protondb-card__recommendation-list">
            {launchOptions.map((launchOption, index) => {
              const text = launchOption.text?.trim() ?? '';
              if (!text) {
                return null;
              }
              const copyKey = `${groupId}:launch:${index}`;
              return (
                <div key={copyKey} className="crosshook-protondb-card__recommendation-item">
                  <p className="crosshook-protondb-card__recommendation-label">
                    <code>{text}</code>
                  </p>
                  <p className="crosshook-protondb-card__recommendation-note">
                    {launchOption.source_label}
                    {launchOption.supporting_report_count != null
                      ? ` • ${launchOption.supporting_report_count} supporting report${
                          launchOption.supporting_report_count === 1 ? '' : 's'
                        }`
                      : ''}
                  </p>
                  <div className="crosshook-protondb-card__actions">
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary"
                      onClick={() => void handleCopy(copyKey, text)}
                    >
                      {copyLabels[copyKey] ?? 'Copy Launch Options'}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        ) : null}

        {notes.length > 0 ? (
          <div className="crosshook-protondb-card__recommendation-list">
            {notes.map((note) => (
              <div
                key={`${groupId}:note:${note.kind}:${note.source_label ?? ''}:${note.text ?? ''}`}
                className="crosshook-protondb-card__recommendation-item"
              >
                <p className="crosshook-protondb-card__recommendation-note">{note.text}</p>
                {note.source_label ? (
                  <p className="crosshook-protondb-card__recommendation-note">{note.source_label}</p>
                ) : null}
              </div>
            ))}
          </div>
        ) : null}
      </section>
    );
  }

  return (
    <section className={cardClasses} aria-labelledby={titleId}>
      <div className="crosshook-protondb-card__header">
        <div className="crosshook-protondb-card__title-row">
          <div className="crosshook-protondb-card__meta">
            <h2 id={titleId} className="crosshook-protondb-card__title">
              ProtonDB Guidance
            </h2>
            <p className="crosshook-protondb-card__subtitle">
              Advisory Linux compatibility data for the current Steam App ID.
            </p>
          </div>
          {snapshot ? (
            <span
              className={`crosshook-protondb-tier-badge crosshook-protondb-tier-badge--${tierClassName(snapshot.tier)}`}
            >
              {formatTierLabel(snapshot.tier)}
            </span>
          ) : (
            <span className={stateClass}>{lookup.state === 'ready' ? 'Ready' : STATE_LABELS[lookup.state]}</span>
          )}
        </div>

        {snapshot ? (
          <dl className="crosshook-protondb-card__summary">
            {metaItems.map(({ label, value }) => (
              <Fragment key={label}>
                <dt className="crosshook-protondb-card__summary-key">{label}</dt>
                <dd className="crosshook-protondb-card__summary-value">{value}</dd>
              </Fragment>
            ))}
          </dl>
        ) : null}

        {renderBanner()}

        <div className="crosshook-protondb-card__source-row">
          {sourceUrl ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--outline"
              onClick={() => void openUrl(sourceUrl)}
            >
              Open in ProtonDB ↗
            </button>
          ) : null}
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            disabled={!lookup.appId || lookup.loading}
            onClick={() => void lookup.refresh()}
          >
            {lookup.loading ? 'Refreshing…' : 'Refresh'}
          </button>
        </div>
      </div>

      {snapshot ? (
        <div className="crosshook-protondb-card__community">
          <h3 className="crosshook-protondb-card__community-title">Community Recommendations</h3>
          {actionableGroups.length > 0 ? (
            <div className="crosshook-protondb-card__recommendations">
              {actionableGroups.map((group, index) => renderRecommendationGroup(group, index))}
            </div>
          ) : (
            <p className="crosshook-protondb-card__community-empty">No community data available for this game yet.</p>
          )}
        </div>
      ) : null}

      {(() => {
        if (!snapshot || !suggestionSet) return null;
        const visibleCatalog = suggestionSet.catalogSuggestions.filter((s) => s.status !== 'dismissed');
        const visibleEnvVar = suggestionSet.envVarSuggestions.filter((s) => s.status !== 'dismissed');
        if (visibleCatalog.length === 0 && visibleEnvVar.length === 0) return null;

        return (
          <div className="crosshook-protondb-card__suggestions">
            <h3 className="crosshook-protondb-card__community-title">Smart Suggestions</h3>

            {suggestionSet.isStale ? (
              <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--stale">
                <p className="crosshook-protondb-card__banner-copy">
                  Suggestions are based on cached ProtonDB data and may be outdated.
                </p>
              </div>
            ) : null}

            {visibleCatalog.map((item: CatalogSuggestionItem) => (
              <div key={item.catalogEntryId} className="crosshook-protondb-card__recommendation-item">
                <div className="crosshook-protondb-card__recommendation-label">
                  <strong>{item.label}</strong>
                  {item.status === 'already_applied' ? (
                    <span className="crosshook-protondb-card__status-badge crosshook-protondb-card__status-badge--applied">
                      &#10003; Applied
                    </span>
                  ) : null}
                </div>
                <p className="crosshook-protondb-card__recommendation-note">
                  {item.description} &bull; {item.supportingReportCount} report
                  {item.supportingReportCount === 1 ? '' : 's'}
                </p>
                <div className="crosshook-protondb-card__actions">
                  {item.status === 'new' && onAcceptSuggestion ? (
                    <button
                      type="button"
                      className="crosshook-button"
                      onClick={() =>
                        void onAcceptSuggestion({
                          kind: 'catalog',
                          profileName: '',
                          catalogEntryId: item.catalogEntryId,
                        })
                      }
                    >
                      Enable
                    </button>
                  ) : null}
                  {onDismissSuggestion ? (
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary"
                      onClick={() => onDismissSuggestion(item.catalogEntryId)}
                    >
                      Dismiss
                    </button>
                  ) : null}
                </div>
              </div>
            ))}

            {visibleEnvVar.map((item: EnvVarSuggestionItem) => (
              <div key={item.key} className="crosshook-protondb-card__recommendation-item">
                <p className="crosshook-protondb-card__recommendation-label">
                  <code>
                    {item.key}={item.value}
                  </code>
                  {item.status === 'already_applied' ? (
                    <span className="crosshook-protondb-card__status-badge crosshook-protondb-card__status-badge--applied">
                      &#10003; Applied
                    </span>
                  ) : item.status === 'conflict' ? (
                    <span className="crosshook-protondb-card__status-badge crosshook-protondb-card__status-badge--conflict">
                      &#9888; Conflict
                    </span>
                  ) : null}
                </p>
                <p className="crosshook-protondb-card__recommendation-note">
                  {item.supportingReportCount} report{item.supportingReportCount === 1 ? '' : 's'}
                </p>
                <div className="crosshook-protondb-card__actions">
                  {(item.status === 'new' || item.status === 'conflict') && onAcceptSuggestion ? (
                    <button
                      type="button"
                      className="crosshook-button"
                      onClick={() =>
                        void onAcceptSuggestion({
                          kind: 'env_var',
                          profileName: '',
                          envKey: item.key,
                          envValue: item.value,
                        })
                      }
                    >
                      {item.status === 'conflict' ? 'Overwrite' : 'Apply'}
                    </button>
                  ) : null}
                  {onDismissSuggestion ? (
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary"
                      onClick={() => onDismissSuggestion(item.key)}
                    >
                      Dismiss
                    </button>
                  ) : null}
                </div>
              </div>
            ))}
          </div>
        );
      })()}
    </section>
  );
}

export default ProtonDbLookupCard;
