import { type ReactNode, useId } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import type { GameDetailsProfileLoadState } from '@/hooks/useGameDetailsProfile';
import type { UseGameMetadataResult } from '@/hooks/useGameMetadata';
import { useLaunchHistoryForProfile } from '@/hooks/useLaunchHistoryForProfile';
import type { OfflineReadinessReport } from '@/types';
import type { EnrichedProfileHealthReport } from '@/types/health';
import type { LaunchPreview, LaunchRequest, PreviewEnvVar } from '@/types/launch';
import type { LibraryCardData, ProfileSummary } from '@/types/library';
import type { GameProfile } from '@/types/profile';
import { groupPreviewEnvBySource, launchMethodLabel } from '@/utils/launchPreviewPresentation';
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
  /** Phase 1 channel: intentionally async-draft shape, differs from `useProfile.ts#updateProfile` (sync updater). Consumed by Phase 4/5. */
  updateProfile?: (draft: GameProfile) => Promise<void>;
  /** Phase 1 channel: left-list cards source for Phase 4 Profiles tab. */
  profileList?: ProfileSummary[];
  /** Phase 1 channel: panel-body → shell request, distinct from `HeroDetailTabs#onActiveTabChange`. Consumed by Phase 7 Overview deep-links. */
  onSetActiveTab?: (tab: HeroDetailTabId) => void;
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

function groupEnvBySource(vars: PreviewEnvVar[]): Array<{ label: string; vars: PreviewEnvVar[] }> {
  return groupPreviewEnvBySource(vars).map(([label, entries]) => ({
    label,
    vars: [...entries].sort((a, b) => a.key.localeCompare(b.key)),
  }));
}

function formatGeneratedAt(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return date.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'medium' });
}

function issueTone(severity: 'fatal' | 'warning' | 'info'): string {
  if (severity === 'fatal') return 'Error';
  if (severity === 'warning') return 'Warning';
  return 'Info';
}

function formatEnvDump(vars: PreviewEnvVar[]): string {
  return vars.map((envVar) => `${envVar.key} = ${JSON.stringify(envVar.value)}`).join('\n');
}

function LaunchPreviewSection({
  title,
  ariaLabel,
  children,
}: {
  title: string;
  ariaLabel: string;
  children: ReactNode;
}) {
  return (
    <section className="crosshook-hero-detail__section crosshook-hero-detail__section--card" aria-label={ariaLabel}>
      <h3 className="crosshook-hero-detail__section-title">{title}</h3>
      {children}
    </section>
  );
}

function LaunchPreviewSummarySection({ preview }: { preview: LaunchPreview }) {
  return (
    <LaunchPreviewSection title="Summary" ariaLabel="Summary">
      <div className="crosshook-hero-detail__kv-list">
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Method</span>
          <span className="crosshook-hero-detail__kv-value">{launchMethodLabel(preview.resolved_method)}</span>
        </p>
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Game executable</span>
          <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
            {preview.game_executable_name || preview.game_executable}
          </span>
        </p>
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Working directory</span>
          <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
            {preview.working_directory || 'Not set'}
          </span>
        </p>
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Generated</span>
          <span className="crosshook-hero-detail__kv-value">{formatGeneratedAt(preview.generated_at)}</span>
        </p>
      </div>
    </LaunchPreviewSection>
  );
}

function LaunchPreviewValidationSection({ preview }: { preview: LaunchPreview }) {
  return (
    <LaunchPreviewSection title="Validation" ariaLabel="Validation">
      {preview.validation.issues.length === 0 ? (
        <p className="crosshook-hero-detail__text">All checks passed.</p>
      ) : (
        <ul className="crosshook-hero-detail__issue-list" aria-label="Launch validation issues">
          {preview.validation.issues.map((issue) => (
            <li
              key={`${issue.severity}-${issue.code ?? 'none'}-${issue.message}-${issue.help}`}
              className="crosshook-hero-detail__issue"
            >
              <span className="crosshook-hero-detail__issue-severity">{issueTone(issue.severity)}:</span>{' '}
              {issue.message}
              {issue.help ? <span className="crosshook-hero-detail__text--small"> — {issue.help}</span> : null}
            </li>
          ))}
        </ul>
      )}
    </LaunchPreviewSection>
  );
}

function LaunchPreviewCommandChainSection({ preview }: { preview: LaunchPreview }) {
  return (
    <LaunchPreviewSection title="Command chain" ariaLabel="Command chain">
      {preview.wrappers && preview.wrappers.length > 0 ? (
        <p className="crosshook-hero-detail__text">
          <span className="crosshook-hero-detail__label">Wrappers: </span>
          <span className="crosshook-hero-detail__mono">{preview.wrappers.join(' -> ')}</span>
        </p>
      ) : null}
      {preview.effective_command ? (
        <pre className="crosshook-hero-detail__command-block">{preview.effective_command}</pre>
      ) : (
        <p className="crosshook-hero-detail__muted">No effective command resolved.</p>
      )}
      {preview.steam_launch_options ? (
        <>
          <p className="crosshook-hero-detail__label">Steam launch options</p>
          <pre className="crosshook-hero-detail__command-block">{preview.steam_launch_options}</pre>
        </>
      ) : null}
      {preview.directives_error ? <p className="crosshook-hero-detail__warn">{preview.directives_error}</p> : null}
    </LaunchPreviewSection>
  );
}

function LaunchPreviewProtonSetupSection({ preview }: { preview: LaunchPreview }) {
  if (!preview.proton_setup) {
    return null;
  }

  return (
    <LaunchPreviewSection title="Proton setup" ariaLabel="Proton setup">
      <div className="crosshook-hero-detail__kv-list">
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Proton executable</span>
          <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
            {preview.proton_setup.proton_executable}
          </span>
        </p>
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Wine prefix</span>
          <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
            {preview.proton_setup.wine_prefix_path}
          </span>
        </p>
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Compat data</span>
          <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
            {preview.proton_setup.compat_data_path}
          </span>
        </p>
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Steam client</span>
          <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
            {preview.proton_setup.steam_client_install_path}
          </span>
        </p>
      </div>
    </LaunchPreviewSection>
  );
}

function LaunchPreviewTrainerSection({ preview }: { preview: LaunchPreview }) {
  if (!preview.trainer) {
    return null;
  }

  return (
    <LaunchPreviewSection title="Trainer" ariaLabel="Trainer">
      <div className="crosshook-hero-detail__kv-list">
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Path</span>
          <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">{preview.trainer.path}</span>
        </p>
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Host path</span>
          <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
            {preview.trainer.host_path}
          </span>
        </p>
        <p className="crosshook-hero-detail__kv-item">
          <span className="crosshook-hero-detail__kv-key">Loading mode</span>
          <span className="crosshook-hero-detail__kv-value">{preview.trainer.loading_mode}</span>
        </p>
        {preview.trainer.staged_path ? (
          <p className="crosshook-hero-detail__kv-item">
            <span className="crosshook-hero-detail__kv-key">Staged path</span>
            <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
              {preview.trainer.staged_path}
            </span>
          </p>
        ) : null}
      </div>
    </LaunchPreviewSection>
  );
}

function LaunchPreviewEnvironmentSection({ preview }: { preview: LaunchPreview }) {
  const groupedEnv = preview.environment ? groupEnvBySource(preview.environment) : [];

  return (
    <LaunchPreviewSection title="Environment" ariaLabel="Environment">
      {groupedEnv.length > 0 ? (
        <div className="crosshook-hero-detail__env-groups">
          {groupedEnv.map((group) => (
            <details key={group.label} className="crosshook-hero-detail__env-group">
              <summary className="crosshook-hero-detail__env-group-summary">
                {group.label} ({group.vars.length})
              </summary>
              <pre className="crosshook-hero-detail__command-block">{formatEnvDump(group.vars)}</pre>
            </details>
          ))}
        </div>
      ) : (
        <p className="crosshook-hero-detail__muted">No environment variables resolved.</p>
      )}
      {preview.cleared_variables.length > 0 ? (
        <details className="crosshook-hero-detail__env-group">
          <summary className="crosshook-hero-detail__env-group-summary">
            Cleared variables ({preview.cleared_variables.length})
          </summary>
          <pre className="crosshook-hero-detail__command-block">{preview.cleared_variables.join('\n')}</pre>
        </details>
      ) : null}
    </LaunchPreviewSection>
  );
}

function LaunchPreviewRawDumpSection({ preview }: { preview: LaunchPreview }) {
  if (!preview.display_text) {
    return null;
  }

  return (
    <LaunchPreviewSection title="Raw preview" ariaLabel="Raw preview">
      <details className="crosshook-hero-detail__env-group">
        <summary className="crosshook-hero-detail__env-group-summary">Raw preview dump</summary>
        <pre className="crosshook-hero-detail__command-block">{preview.display_text}</pre>
      </details>
    </LaunchPreviewSection>
  );
}

function LaunchPreviewStructuredView({ preview }: { preview: LaunchPreview }) {
  return (
    <>
      <LaunchPreviewSummarySection preview={preview} />
      <LaunchPreviewValidationSection preview={preview} />
      <LaunchPreviewCommandChainSection preview={preview} />
      <LaunchPreviewProtonSetupSection preview={preview} />
      <LaunchPreviewTrainerSection preview={preview} />
      <LaunchPreviewEnvironmentSection preview={preview} />
      <LaunchPreviewRawDumpSection preview={preview} />
    </>
  );
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
          <div className="crosshook-hero-detail__subsection">
            <h4 className="crosshook-hero-detail__subsection-title">Runtime summary</h4>
            <div className="crosshook-hero-detail__kv-list">
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Name</span>
                <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">{activeName}</span>
              </p>
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Prefix</span>
                <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
                  {profile.runtime.prefix_path || 'Not set'}
                </span>
              </p>
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Proton</span>
                <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__mono">
                  {profile.runtime.proton_path || profile.steam.proton_path || 'Not set'}
                </span>
              </p>
            </div>
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
  updateProfile,
  profileList,
  onSetActiveTab,
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
          {preview && launchRequest ? <LaunchPreviewStructuredView preview={preview} /> : null}
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
