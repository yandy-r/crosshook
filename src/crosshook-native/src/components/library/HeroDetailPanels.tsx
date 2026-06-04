import type { GameDetailsProfileLoadState } from '@/hooks/useGameDetailsProfile';
import type { UseGameMetadataResult } from '@/hooks/useGameMetadata';
import { useLaunchHistoryForProfile } from '@/hooks/useLaunchHistoryForProfile';
import type { OfflineReadinessReport } from '@/types';
import type { EnrichedProfileHealthReport } from '@/types/health';
import type { LaunchPreview, LaunchRequest } from '@/types/launch';
import type { LibraryCardData, ProfileSummary } from '@/types/library';
import type { GameProfile } from '@/types/profile';
import { GameDetailsCompatibilitySection } from './GameDetailsCompatibilitySection';
import { GameDetailsHealthSection } from './GameDetailsHealthSection';
import { GameDetailsMetadataSection } from './GameDetailsMetadataSection';
import { HeroDetailLaunchTab } from './HeroDetailLaunchTab';
import { HeroDetailProfilesTab } from './HeroDetailProfilesTab';
import type { HeroDetailTabId } from './hero-detail-model';
import { displayPath } from './hero-detail-model';

export interface HeroDetailPanelsProps {
  mode: HeroDetailTabId;
  summary: LibraryCardData;
  steamAppId: string;
  meta: UseGameMetadataResult;
  profile: GameProfile | null;
  loadState: GameDetailsProfileLoadState;
  profileError: string | null;
  healthReport: EnrichedProfileHealthReport | undefined;
  healthLoading: boolean;
  offlineReport: OfflineReadinessReport | undefined;
  offlineError: string | null;
  launchRequest: LaunchRequest | null;
  previewLoading: boolean;
  preview: LaunchPreview | null;
  previewError: string | null;
  /** Phase 1 channel: intentionally async-draft shape, differs from `useProfile.ts#updateProfile` (sync updater). Consumed by Phase 4/5. */
  updateProfile?: (draft: GameProfile) => Promise<void>;
  /** Phase 1 channel: left-list cards source for Phase 4 Profiles tab. */
  profileList?: ProfileSummary[];
  /** Phase 1 channel: panel-body → shell request, distinct from `HeroDetailTabs#onActiveTabChange`. Consumed by Phase 7 Overview deep-links. */
  onSetActiveTab?: (tab: HeroDetailTabId) => void;
  onPreviewLaunch?: (request: LaunchRequest) => void | Promise<void>;
  onLaunch?: (name: string) => void | Promise<void>;
  launchingName?: string;
  displayProfileName?: string;
}

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

function HistoryPanel({ profileName }: { profileName: string }) {
  const { rows, error } = useLaunchHistoryForProfile(profileName, 20);

  return (
    <div className="crosshook-hero-detail__panel-grid">
      <section
        className="crosshook-hero-detail__section crosshook-hero-detail__section--card"
        aria-label="Recent launches"
      >
        <h3 className="crosshook-hero-detail__section-title">Recent launches</h3>
        {error ? (
          <p className="crosshook-hero-detail__warn" role="status">
            {error}
          </p>
        ) : rows === null ? (
          <p className="crosshook-hero-detail__muted" role="status">
            Loading recent launches…
          </p>
        ) : rows.length === 0 ? (
          <p className="crosshook-hero-detail__muted" role="status">
            No recent launches recorded for this profile.
          </p>
        ) : (
          <ul className="crosshook-hero-detail__launch-list" aria-label="Recent launches">
            {rows.map((row) => (
              <li key={row.operation_id} className="crosshook-hero-detail__launch-item">
                <div className="crosshook-hero-detail__launch-line">
                  <span className="crosshook-hero-detail__launch-time">{formatLaunchTime(row.started_at)}</span>
                  <span className="crosshook-hero-detail__launch-status">{launchStatusLabel(row.status)}</span>
                </div>
                <div className="crosshook-hero-detail__launch-meta">
                  {row.launch_method}
                  {row.finished_at ? ` · finished ${formatLaunchTime(row.finished_at)}` : null}
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

export function HeroDetailPanels({
  mode,
  summary,
  steamAppId,
  meta,
  profile,
  loadState,
  profileError,
  healthReport,
  healthLoading,
  offlineReport,
  offlineError,
  launchRequest,
  previewLoading,
  preview,
  previewError,
  profileList,
  onPreviewLaunch,
  onLaunch,
  launchingName,
  displayProfileName,
}: HeroDetailPanelsProps) {
  switch (mode) {
    case 'overview':
      return (
        <div className="crosshook-hero-detail__panel-grid">
          {loadState === 'loading' ? <p className="crosshook-hero-detail__muted">Loading profile details…</p> : null}
          {loadState === 'error' ? (
            <p className="crosshook-hero-detail__warn">{profileError ?? 'Failed to load profile.'}</p>
          ) : null}
          <GameDetailsMetadataSection steamAppId={steamAppId} meta={meta} />
          <GameDetailsHealthSection
            profileName={summary.name}
            healthReport={healthReport}
            healthLoading={healthLoading}
            offlineReport={offlineReport}
            offlineError={offlineError}
          />
        </div>
      );
    case 'profiles':
      return (
        <HeroDetailProfilesTab
          summary={summary}
          profileList={profileList}
          loadState={loadState}
          profileError={profileError}
        />
      );
    case 'launch-options':
      // Deliberately does not forward `updateProfile`: HeroDetailLaunchTab reads it from
      // useProfileContext() (GameDetail wraps the hero detail in ProfileProvider). If this panel
      // is reused outside that tree, extend HeroDetailLaunchTab to accept an optional override.
      return (
        <HeroDetailLaunchTab
          summary={summary}
          launchRequest={launchRequest}
          previewLoading={previewLoading}
          preview={preview}
          previewError={previewError}
          onPreviewLaunch={onPreviewLaunch}
          onLaunch={onLaunch}
          launchingName={launchingName}
          displayProfileName={displayProfileName}
        />
      );
    case 'trainer':
      return (
        <div className="crosshook-hero-detail__panel-grid">
          {loadState === 'loading' ? (
            <p className="crosshook-hero-detail__muted">Loading trainer configuration…</p>
          ) : null}
          {loadState === 'error' ? (
            <p className="crosshook-hero-detail__warn">{profileError ?? 'Failed to load profile.'}</p>
          ) : null}
          {profile && loadState === 'ready' ? (
            <section
              className="crosshook-hero-detail__section crosshook-hero-detail__section--card"
              aria-label="Trainer"
            >
              <h3 className="crosshook-hero-detail__section-title">Trainer</h3>
              <div className="crosshook-hero-detail__subsection">
                <h4 className="crosshook-hero-detail__subsection-title">Configuration</h4>
                <div className="crosshook-hero-detail__kv-list">
                  <p className="crosshook-hero-detail__kv-item">
                    <span className="crosshook-hero-detail__kv-key">Path</span>
                    <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
                      {displayPath(profile.trainer.path)}
                    </span>
                  </p>
                  <p className="crosshook-hero-detail__kv-item">
                    <span className="crosshook-hero-detail__kv-key">Loading mode</span>
                    <span className="crosshook-hero-detail__kv-value">{profile.trainer.loading_mode || 'Not set'}</span>
                  </p>
                </div>
              </div>
            </section>
          ) : null}
        </div>
      );
    case 'history':
      return <HistoryPanel profileName={summary.name} />;
    case 'compatibility':
      return (
        <div className="crosshook-hero-detail__panel-grid">
          <GameDetailsCompatibilitySection steamAppId={steamAppId} />
        </div>
      );
    default:
      return null;
  }
}
