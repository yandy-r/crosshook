import { useMemo } from 'react';
import '../../styles/host-tool-dashboard.css';
import { HostToolMetricsHero } from '@/components/host-readiness/HostToolMetricsHero';
import { PinnedProfilesStrip } from '@/components/PinnedProfilesStrip';
import { useHostReadinessContext } from '@/context/HostReadinessContext';
import { useInspectorSelection } from '@/context/InspectorSelectionContext';
import { useProfileContext } from '@/context/ProfileContext';
import { useLaunchHistoryForProfile } from '@/hooks/useLaunchHistoryForProfile';
import type { LaunchHistoryEntry } from '@/types/library';

const LAUNCH_HISTORY_LIMIT = 48;

function formatLaunchTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) {
    return iso;
  }
  return d.toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' });
}

function launchStatusLabel(status: string): string {
  switch (status) {
    case 'started':
      return 'In progress';
    case 'succeeded':
      return 'Succeeded';
    case 'failed':
      return 'Failed';
    case 'abandoned':
      return 'Abandoned';
    default:
      return status;
  }
}

/** Buckets [oldest … newest] for the last 7 calendar days from `Date.now()`. */
export function bucketLaunchesLast7Days(rows: LaunchHistoryEntry[] | null, nowMs: number = Date.now()): number[] {
  const out = [0, 0, 0, 0, 0, 0, 0];
  if (!rows?.length) {
    return out;
  }
  const msPerDay = 86_400_000;
  for (const row of rows) {
    const t = Date.parse(row.started_at);
    if (Number.isNaN(t)) {
      continue;
    }
    const daysAgo = Math.floor((nowMs - t) / msPerDay);
    if (daysAgo < 0 || daysAgo > 6) {
      continue;
    }
    out[6 - daysAgo]++;
  }
  return out;
}

export function ContextRail() {
  const { snapshot, capabilities, error } = useHostReadinessContext();
  const { favoriteProfiles, selectedProfile, selectProfile, toggleFavorite } = useProfileContext();
  const { inspectorSelection } = useInspectorSelection();

  const focusProfileName = useMemo(() => {
    const pick = inspectorSelection?.name?.trim() || selectedProfile.trim();
    return pick.length > 0 ? pick : null;
  }, [inspectorSelection?.name, selectedProfile]);

  const { rows, error: historyError } = useLaunchHistoryForProfile(focusProfileName ?? undefined, LAUNCH_HISTORY_LIMIT);

  const activityBuckets = useMemo(
    () => bucketLaunchesLast7Days(focusProfileName ? rows : null),
    [focusProfileName, rows]
  );
  const maxBucket = useMemo(() => Math.max(1, ...activityBuckets), [activityBuckets]);

  const topSessions = useMemo(() => {
    if (!focusProfileName || !rows?.length) {
      return [];
    }
    const weekAgo = Date.now() - 7 * 86_400_000;
    return rows.filter((r) => r.status === 'succeeded' && Date.parse(r.started_at) >= weekAgo).slice(0, 5);
  }, [focusProfileName, rows]);

  const hostLoading = snapshot === null && error === null;

  return (
    <aside className="crosshook-context-rail" data-testid="context-rail" aria-label="Library context">
      <header className="crosshook-context-rail__header">
        <h2 className="crosshook-context-rail__title">Context</h2>
        <p className="crosshook-context-rail__subtitle">Host, pins, and recent launches</p>
      </header>
      <div className="crosshook-context-rail__body">
        <section className="crosshook-context-rail__section" aria-labelledby="crosshook-context-rail-host-title">
          <h3 id="crosshook-context-rail-host-title" className="crosshook-game-inspector__eyebrow">
            Host readiness
          </h3>
          {error ? (
            <p className="crosshook-game-inspector__feedback-help" role="status">
              {error}
            </p>
          ) : (
            <HostToolMetricsHero
              toolChecks={snapshot?.tool_checks ?? []}
              capabilities={capabilities}
              loading={hostLoading}
            />
          )}
        </section>

        <section className="crosshook-context-rail__section" aria-label="Pinned profiles">
          {favoriteProfiles.length === 0 ? (
            <>
              <h3 className="crosshook-game-inspector__eyebrow">Pinned profiles</h3>
              <p className="crosshook-game-inspector__muted" role="status">
                Pin games from the library to show them here.
              </p>
            </>
          ) : (
            <PinnedProfilesStrip
              favoriteProfiles={favoriteProfiles}
              selectedProfile={selectedProfile}
              onSelectProfile={selectProfile}
              onToggleFavorite={toggleFavorite}
            />
          )}
        </section>

        <section className="crosshook-context-rail__section" aria-labelledby="crosshook-context-rail-activity-title">
          <h3 id="crosshook-context-rail-activity-title" className="crosshook-game-inspector__eyebrow">
            7-day activity
          </h3>
          {!focusProfileName ? (
            <p className="crosshook-game-inspector__muted" role="status">
              Select or load a profile to see launch activity.
            </p>
          ) : historyError ? (
            <p className="crosshook-game-inspector__feedback-help" role="status">
              {historyError}
            </p>
          ) : rows === null ? (
            <p className="crosshook-game-inspector__muted" role="status">
              Loading activity…
            </p>
          ) : (
            <div className="crosshook-context-rail__chart">
              {activityBuckets.map((n, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: fixed seven calendar buckets; order is stable
                <div key={i} className="crosshook-context-rail__chart-cell">
                  <div
                    className="crosshook-context-rail__chart-bar"
                    style={{ height: `${Math.round((n / maxBucket) * 100)}%` }}
                  />
                  <span className="crosshook-context-rail__chart-label">{i + 1}</span>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="crosshook-context-rail__section" aria-labelledby="crosshook-context-rail-most-title">
          <h3 id="crosshook-context-rail-most-title" className="crosshook-game-inspector__eyebrow">
            Most-played sessions
          </h3>
          {!focusProfileName ? (
            <p className="crosshook-game-inspector__muted" role="status">
              Select or load a profile to see recent successful sessions.
            </p>
          ) : historyError ? (
            <p className="crosshook-game-inspector__feedback-help" role="status">
              {historyError}
            </p>
          ) : rows === null ? (
            <p className="crosshook-game-inspector__muted" role="status">
              Loading sessions…
            </p>
          ) : topSessions.length === 0 ? (
            <p className="crosshook-game-inspector__muted" role="status">
              No successful launches in the last 7 days.
            </p>
          ) : (
            <ul className="crosshook-game-inspector__launch-list" aria-label="Recent successful launches">
              {topSessions.map((row) => (
                <li key={row.operation_id} className="crosshook-game-inspector__launch-item">
                  <div className="crosshook-game-inspector__launch-line">
                    <span className="crosshook-game-inspector__launch-time">{formatLaunchTime(row.started_at)}</span>
                    <span className="crosshook-game-inspector__launch-status">{launchStatusLabel(row.status)}</span>
                  </div>
                  <div className="crosshook-game-inspector__launch-meta">{row.launch_method}</div>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </aside>
  );
}
