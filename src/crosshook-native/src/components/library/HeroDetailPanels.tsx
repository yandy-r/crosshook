import { useId } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import type { GameDetailsProfileLoadState } from '@/hooks/useGameDetailsProfile';
import type { UseGameMetadataResult } from '@/hooks/useGameMetadata';
import { useLaunchHistoryForProfile } from '@/hooks/useLaunchHistoryForProfile';
import type { OfflineReadinessReport } from '@/types';
import type { EnrichedProfileHealthReport } from '@/types/health';
import type { LaunchPreview, LaunchRequest } from '@/types/launch';
import type { LibraryCardData } from '@/types/library';
import type { GameProfile } from '@/types/profile';
import { GameDetailsCompatibilitySection } from './GameDetailsCompatibilitySection';
import { GameDetailsHealthSection } from './GameDetailsHealthSection';
import { GameDetailsMetadataSection } from './GameDetailsMetadataSection';
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

function ProfilesPanel({ profileName }: { profileName: string }) {
  const activeProfileHeadingId = useId();
  const { profileName: activeName, profile } = useProfileContext();
  const isActive = activeName === profileName;

  return (
    <div className="crosshook-hero-detail__panel-grid">
      <section
        className="crosshook-hero-detail__section crosshook-hero-detail__section--card"
        aria-labelledby={activeProfileHeadingId}
      >
        <h3 id={activeProfileHeadingId} className="crosshook-hero-detail__section-title">
          Active profile
        </h3>
        {!isActive ? (
          <p className="crosshook-hero-detail__muted" role="status">
            No active profile loaded in the editor for this game.
          </p>
        ) : (
          <div className="crosshook-hero-detail__health-block">
            <p className="crosshook-hero-detail__text">
              <span className="crosshook-hero-detail__label">Name: </span>
              <span className="crosshook-hero-detail__mono">{activeName}</span>
            </p>
            <p className="crosshook-hero-detail__text">
              <span className="crosshook-hero-detail__label">Prefix: </span>
              {profile.runtime.prefix_path || '—'}
            </p>
            <p className="crosshook-hero-detail__text">
              <span className="crosshook-hero-detail__label">Proton: </span>
              {profile.runtime.proton_path || profile.steam.proton_path || '—'}
            </p>
          </div>
        )}
      </section>
    </div>
  );
}

function HistoryPanel({ profileName }: { profileName: string }) {
  const { rows, error } = useLaunchHistoryForProfile(profileName, 20);

  return (
    <div className="crosshook-hero-detail__panel-grid">
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
      return <ProfilesPanel profileName={summary.name} />;
    case 'launch-options':
      return (
        <div className="crosshook-hero-detail__panel-grid">
          {!launchRequest ? (
            <p className="crosshook-hero-detail__muted">
              Launch preview is unavailable until the game executable is set on this profile.
            </p>
          ) : null}
          {launchRequest && previewLoading ? (
            <p className="crosshook-hero-detail__muted">Building launch preview…</p>
          ) : null}
          {previewError ? <p className="crosshook-hero-detail__warn">{previewError}</p> : null}
          {preview && launchRequest ? (
            <>
              {preview.display_text ? (
                <p className="crosshook-hero-detail__text crosshook-hero-detail__text--desc">{preview.display_text}</p>
              ) : null}
              {preview.effective_command ? (
                <pre className="crosshook-hero-detail__command-block">{preview.effective_command}</pre>
              ) : null}
            </>
          ) : null}
        </div>
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
              <p className="crosshook-hero-detail__text">
                <span className="crosshook-hero-detail__label">Path: </span>
                <span className="crosshook-hero-detail__mono">{displayPath(profile.trainer.path)}</span>
              </p>
              <p className="crosshook-hero-detail__text">
                <span className="crosshook-hero-detail__label">Loading mode: </span>
                {profile.trainer.loading_mode || '—'}
              </p>
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
