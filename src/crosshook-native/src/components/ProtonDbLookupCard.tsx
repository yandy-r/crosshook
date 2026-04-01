import { Fragment, useEffect, useId, useMemo, useRef, useState } from 'react';

import { open as openUrl } from '@tauri-apps/plugin-shell';

import { useProtonDbLookup } from '../hooks/useProtonDbLookup';
import type {
  ProtonDbLookupState,
  ProtonDbRecommendationGroup,
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
  if (['platinum', 'gold', 'silver', 'bronze', 'borked'].includes(normalized)) {
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
}: ProtonDbLookupCardProps) {
  const titleId = useId();
  const [copyLabels, setCopyLabels] = useState<Record<string, string>>({});
  const copyTimeouts = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const lookup = useProtonDbLookup(appId);

  useEffect(() => {
    return () => {
      copyTimeouts.current.forEach((id) => clearTimeout(id));
    };
  }, []);
  const { snapshot, cache, recommendationGroups } = lookup;
  const actionableGroups = recommendationGroups.filter((g) => g.group_id !== 'report-feed-unavailable');

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

  const stateClass = `crosshook-protondb-card__state crosshook-protondb-card__state--${stateTone(
    lookup.state
  )}`;

  const freshnessLabel =
    cache?.fetched_at || snapshot?.fetched_at
      ? formatRelativeTime(cache?.fetched_at || snapshot?.fetched_at || '')
      : null;

  const metaItems = [
    { label: 'Tier', value: snapshot ? formatTierLabel(snapshot.tier) : null },
    { label: 'Reports', value: snapshot?.total_reports != null ? String(snapshot.total_reports) : null },
    { label: 'Confidence', value: snapshot?.confidence ?? null },
    { label: 'Score', value: snapshot?.score != null ? snapshot.score.toFixed(2) : null },
    { label: 'Best reported', value: snapshot?.best_reported_tier ? formatTierLabel(snapshot.best_reported_tier) : null },
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
    if (existing) clearTimeout(existing);
    const id = window.setTimeout(() => {
      setCopyLabels((current) => ({ ...current, [copyKey]: 'Copy' }));
      copyTimeouts.current.delete(copyKey);
    }, 2000);
    copyTimeouts.current.set(copyKey, id);
  }

  function renderBanner() {
    if (
      versionContext?.version_status === 'game_updated' ||
      versionContext?.version_status === 'both_changed'
    ) {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--stale">
          <p className="crosshook-protondb-card__banner-copy">
            The installed game build changed since the last successful launch. ProtonDB guidance may
            be stale until newer reports catch up with the updated build.
          </p>
        </div>
      );
    }

    if (versionContext?.version_status === 'update_in_progress') {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--stale">
          <p className="crosshook-protondb-card__banner-copy">
            Steam is currently updating this game. ProtonDB guidance may not match the in-progress
            build yet.
          </p>
        </div>
      );
    }

    if (!lookup.appId) {
      return (
        <div className="crosshook-protondb-card__banner crosshook-protondb-card__banner--neutral">
          <p className="crosshook-protondb-card__banner-copy crosshook-protondb-card__banner-copy--muted">
            Add a Steam App ID to this profile to enable ProtonDB compatibility guidance.
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

  function renderRecommendationGroup(group: ProtonDbRecommendationGroup) {
    const envVars = group.env_vars ?? [];
    const launchOptions = group.launch_options ?? [];
    const notes = group.notes ?? [];
    const canApplyEnvVars = envVars.length > 0 && onApplyEnvVars != null;

    return (
      <section key={group.group_id} className="crosshook-protondb-card__recommendation-group">
        <div className="crosshook-protondb-card__meta">
          <h3 className="crosshook-protondb-card__recommendation-group-title">{group.title}</h3>
          {group.summary ? (
            <p className="crosshook-protondb-card__recommendation-group-copy">{group.summary}</p>
          ) : null}
        </div>

        {envVars.length > 0 ? (
          <div className="crosshook-protondb-card__recommendation-list">
            {envVars.map((envVar) => {
              const text = `${envVar.key}=${envVar.value}`;
              const copyKey = `${group.group_id}:${envVar.key}`;
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
                disabled={!canApplyEnvVars || applyingGroupId === group.group_id}
                onClick={() => onApplyEnvVars?.(group)}
              >
                {applyingGroupId === group.group_id ? 'Applying…' : 'Apply Suggested Env Vars'}
              </button>
            </div>
          </div>
        ) : null}

        {launchOptions.length > 0 ? (
          <div className="crosshook-protondb-card__recommendation-list">
            {launchOptions.map((launchOption, index) => {
              const copyKey = `${group.group_id}:launch:${index}`;
              return (
                <div key={copyKey} className="crosshook-protondb-card__recommendation-item">
                  <p className="crosshook-protondb-card__recommendation-label">
                    <code>{launchOption.text}</code>
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
                      onClick={() => void handleCopy(copyKey, launchOption.text)}
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
            {notes.map((note, index) => (
              <div key={`${group.group_id}:note:${index}`} className="crosshook-protondb-card__recommendation-item">
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
            <span className={`crosshook-protondb-tier-badge crosshook-protondb-tier-badge--${tierClassName(snapshot.tier)}`}>
              {formatTierLabel(snapshot.tier)}
            </span>
          ) : (
            <span className={stateClass}>
              {lookup.state === 'ready' ? 'Ready' : STATE_LABELS[lookup.state]}
            </span>
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
          {snapshot?.source_url ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--outline"
              onClick={() => void openUrl(snapshot.source_url)}
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
              {actionableGroups.map(renderRecommendationGroup)}
            </div>
          ) : (
            <p className="crosshook-protondb-card__community-empty">
              No community data available for this game yet.
            </p>
          )}
        </div>
      ) : null}
    </section>
  );
}

export default ProtonDbLookupCard;
