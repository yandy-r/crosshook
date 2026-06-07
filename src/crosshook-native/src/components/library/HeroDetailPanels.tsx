import type { ReactNode } from 'react';
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
import { HeroDetailTrainerTab } from './HeroDetailTrainerTab';
import type { HeroDetailProfilesScrollTarget, HeroDetailTabId, HeroDetailTabRequestOptions } from './hero-detail-model';
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
  onSetActiveTab?: (tab: HeroDetailTabId, options?: HeroDetailTabRequestOptions) => void;
  profilesScrollTarget?: HeroDetailProfilesScrollTarget | null;
  onProfilesScrollTargetConsumed?: () => void;
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

interface OverviewActionCardProps {
  title: string;
  description: string;
  buttonLabel: string;
  onClick?: () => void;
  children?: ReactNode;
}

function OverviewActionCard({ title, description, buttonLabel, onClick, children }: OverviewActionCardProps) {
  return (
    <section className="crosshook-hero-detail__section crosshook-hero-detail__section--card" aria-label={title}>
      <div className="crosshook-hero-detail__section-header-row">
        <div>
          <h3 className="crosshook-hero-detail__section-title">{title}</h3>
          <p className="crosshook-hero-detail__muted">{description}</p>
        </div>
        <div className="crosshook-hero-detail__overview-actions">
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={onClick}
            disabled={!onClick}
          >
            {buttonLabel}
          </button>
        </div>
      </div>
      {children ? <div className="crosshook-hero-detail__kv-list">{children}</div> : null}
    </section>
  );
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
  onSetActiveTab,
  profilesScrollTarget,
  onProfilesScrollTargetConsumed,
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
          <OverviewActionCard
            title="Runtime"
            description="Jump to the runtime fields in the Profiles editor."
            buttonLabel="Open runtime"
            onClick={onSetActiveTab ? () => onSetActiveTab('profiles', { profilesScrollTarget: 'runtime' }) : undefined}
          >
            <p className="crosshook-hero-detail__kv-item">
              <span className="crosshook-hero-detail__kv-key">Prefix</span>
              <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
                {displayPath(profile?.runtime?.prefix_path ?? profile?.steam?.compatdata_path)}
              </span>
            </p>
            <p className="crosshook-hero-detail__kv-item">
              <span className="crosshook-hero-detail__kv-key">Proton</span>
              <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
                {displayPath(profile?.runtime?.proton_path ?? profile?.steam?.proton_path)}
              </span>
            </p>
          </OverviewActionCard>
          <OverviewActionCard
            title="Active profile"
            description="Open the selected profile editor for this game."
            buttonLabel="Open profile"
            onClick={onSetActiveTab ? () => onSetActiveTab('profiles') : undefined}
          >
            <p className="crosshook-hero-detail__kv-item">
              <span className="crosshook-hero-detail__kv-key">Profile</span>
              <span className="crosshook-hero-detail__kv-value">{displayProfileName ?? summary.name}</span>
            </p>
            <p className="crosshook-hero-detail__kv-item">
              <span className="crosshook-hero-detail__kv-key">Game</span>
              <span className="crosshook-hero-detail__kv-value">{summary.gameName || summary.name}</span>
            </p>
          </OverviewActionCard>
          <OverviewActionCard
            title="Launch command"
            description="Open the launch configuration and command preview."
            buttonLabel="Edit launch config"
            onClick={onSetActiveTab ? () => onSetActiveTab('launch-options') : undefined}
          >
            <p className="crosshook-hero-detail__kv-item">
              <span className="crosshook-hero-detail__kv-key">Method</span>
              <span className="crosshook-hero-detail__kv-value">{launchRequest?.method ?? 'Not available'}</span>
            </p>
            <p className="crosshook-hero-detail__kv-item">
              <span className="crosshook-hero-detail__kv-key">Command</span>
              <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
                {previewLoading ? 'Building preview…' : (preview?.effective_command ?? previewError ?? 'Not available')}
              </span>
            </p>
          </OverviewActionCard>
          <OverviewActionCard
            title="Trainer hook"
            description="Open the Pre/post hooks surface in Launch options."
            buttonLabel="Manage hooks"
            onClick={onSetActiveTab ? () => onSetActiveTab('launch-options') : undefined}
          >
            <p className="crosshook-hero-detail__kv-item">
              <span className="crosshook-hero-detail__kv-key">Pre-launch</span>
              <span className="crosshook-hero-detail__kv-value">
                {profile?.pre_launch_hooks?.length ?? 0} configured
              </span>
            </p>
            <p className="crosshook-hero-detail__kv-item">
              <span className="crosshook-hero-detail__kv-key">Post-exit</span>
              <span className="crosshook-hero-detail__kv-value">
                {profile?.post_exit_hooks?.length ?? 0} configured
              </span>
            </p>
          </OverviewActionCard>
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
          scrollTarget={profilesScrollTarget}
          onScrollTargetConsumed={onProfilesScrollTargetConsumed}
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
            <HeroDetailTrainerTab summary={summary} displayProfileName={displayProfileName} />
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
