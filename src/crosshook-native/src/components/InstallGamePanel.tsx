import { type CSSProperties, useCallback, useEffect, useRef, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

import { InstallField } from './ui/InstallField';
import { ProtonPathField } from './ui/ProtonPathField';
import { useGameCoverArt } from '../hooks/useGameCoverArt';
import { useImageDominantColor } from '../hooks/useImageDominantColor';
import { useInstallGame } from '../hooks/useInstallGame';
import { useProtonInstalls } from '../hooks/useProtonInstalls';
import type { GameProfile } from '../types/profile';
import type {
  InstallGameExecutableCandidate,
  InstallGamePrefixPathState,
  InstallGameStage,
  InstallProfileReviewPayload,
  ProfileReviewSource,
} from '../types/install';

type InstallFlowTabId = 'identity' | 'media' | 'runtime' | 'review';

const INSTALL_FLOW_TAB_LABELS: Record<InstallFlowTabId, string> = {
  identity: 'Profile identity',
  media: 'Install media',
  runtime: 'Runtime',
  review: 'Review',
};

function isInstallFlowTabId(value: string): value is InstallFlowTabId {
  return Object.prototype.hasOwnProperty.call(INSTALL_FLOW_TAB_LABELS, value);
}

export interface InstallGamePanelProps {
  onOpenProfileReview: (payload: InstallProfileReviewPayload) => void | Promise<boolean>;
  onRequestInstallAction?: (action: 'retry' | 'reset') => boolean | Promise<boolean>;
}

function stageLabel(stage: InstallGameStage): string {
  switch (stage) {
    case 'preparing':
      return 'Preparing';
    case 'running_installer':
      return 'Running installer';
    case 'review_required':
      return 'Review required';
    case 'ready_to_save':
      return 'Ready to save';
    case 'failed':
      return 'Failed';
    case 'idle':
    default:
      return 'Idle';
  }
}

function fileNameFromPath(path: string): string {
  const normalized = path.trim().replace(/\\/g, '/');
  const lastSegment = normalized.split('/').pop() ?? '';
  return lastSegment || normalized || 'Unnamed executable';
}

function candidateLabel(candidate: InstallGameExecutableCandidate): string {
  const baseName = fileNameFromPath(candidate.path);
  return candidate.is_recommended ? `${baseName} (recommended)` : baseName;
}

function prefixStateLabel(state: InstallGamePrefixPathState): string {
  switch (state) {
    case 'loading':
      return 'Resolving default prefix...';
    case 'ready':
      return 'Default prefix resolved';
    case 'failed':
      return 'Default prefix unavailable';
    case 'idle':
    default:
      return 'Awaiting profile name';
  }
}

function CandidateRow(props: {
  candidate: InstallGameExecutableCandidate;
  currentPath: string;
  onSelect: (path: string) => void;
}) {
  const isSelected = props.candidate.path.trim() === props.currentPath.trim();

  return (
    <button
      type="button"
      className="crosshook-install-candidate"
      onClick={() => props.onSelect(props.candidate.path)}
      style={{
        width: '100%',
        appearance: 'none',
        textAlign: 'left',
        cursor: 'pointer',
        color: isSelected ? 'var(--crosshook-color-text)' : 'var(--crosshook-color-text-muted)',
        borderColor: isSelected ? 'rgba(0, 120, 212, 0.45)' : 'rgba(255, 255, 255, 0.06)',
      }}
    >
      <span>
        <strong style={{ color: 'var(--crosshook-color-text)' }}>{candidateLabel(props.candidate)}</strong>
        {props.candidate.is_recommended ? <span className="crosshook-muted"> - suggested</span> : null}
      </span>
      <span style={{ wordBreak: 'break-all' }}>{props.candidate.path}</span>
    </button>
  );
}

export function InstallGamePanel({ onOpenProfileReview, onRequestInstallAction }: InstallGamePanelProps) {
  const {
    request,
    updateRequest,
    validation,
    stage,
    result,
    reviewProfile,
    error,
    defaultPrefixPath,
    defaultPrefixPathState,
    defaultPrefixPathError,
    candidateOptions,
    isRunningInstaller,
    isResolvingDefaultPrefixPath,
    setInstalledExecutablePath,
    startInstall,
    reset,
    actionLabel,
    statusText,
    hintText,
  } = useInstallGame();

  const customCoverTrimmed = request.custom_cover_art_path.trim();
  const { coverArtUrl, loading: coverArtLoading } = useGameCoverArt(
    undefined,
    customCoverTrimmed.length > 0 ? customCoverTrimmed : undefined
  );
  const dominantColor = useImageDominantColor(coverArtUrl);
  const gameColorStyle: CSSProperties | undefined = dominantColor
    ? ({
        '--crosshook-game-color-r': String(dominantColor[0]),
        '--crosshook-game-color-g': String(dominantColor[1]),
        '--crosshook-game-color-b': String(dominantColor[2]),
      } as CSSProperties)
    : undefined;
  const hasCoverHero = Boolean(coverArtUrl) || coverArtLoading;

  const candidateCount = candidateOptions.length;
  const logPath = result?.helper_log_path ?? '';
  const reviewableInstallResult = result?.succeeded === true && reviewProfile !== null ? result : null;
  const canReviewGeneratedProfile = reviewableInstallResult !== null && reviewProfile !== null;
  const lastAutoOpenReviewKeyRef = useRef<string | null>(null);
  const [activeInstallTab, setActiveInstallTab] = useState<InstallFlowTabId>('identity');
  const { installs: protonInstalls, error: protonInstallsError } = useProtonInstalls();

  const installFlowTabs = (['identity', 'media', 'runtime', 'review'] as const).map((id) => ({
    id,
    label: INSTALL_FLOW_TAB_LABELS[id],
  }));

  const openReviewPayload = useCallback(
    (source: ProfileReviewSource) => {
      if (reviewableInstallResult === null || reviewProfile === null) {
        return;
      }

      const patchedProfile: GameProfile = {
        ...reviewProfile,
        game: {
          ...reviewProfile.game,
          custom_cover_art_path: reviewProfile.game.custom_cover_art_path,
        },
        steam: request.launcher_icon_path.trim()
          ? {
              ...reviewProfile.steam,
              launcher: { ...reviewProfile.steam.launcher, icon_path: request.launcher_icon_path.trim() },
            }
          : reviewProfile.steam,
      };

      void onOpenProfileReview({
        source,
        profileName: reviewableInstallResult.profile_name.trim() || request.profile_name.trim(),
        generatedProfile: patchedProfile,
        candidateOptions,
        helperLogPath: logPath,
        message: reviewableInstallResult.message,
      });
    },
    [
      candidateOptions,
      logPath,
      onOpenProfileReview,
      request.launcher_icon_path,
      request.profile_name,
      reviewProfile,
      reviewableInstallResult,
    ]
  );

  useEffect(() => {
    if (reviewableInstallResult === null || reviewProfile === null) {
      lastAutoOpenReviewKeyRef.current = null;
      return;
    }

    const autoOpenReviewKey = [
      reviewableInstallResult.profile_name.trim(),
      reviewableInstallResult.helper_log_path.trim(),
      reviewableInstallResult.message.trim(),
    ].join('::');

    if (lastAutoOpenReviewKeyRef.current === autoOpenReviewKey) {
      return;
    }

    lastAutoOpenReviewKeyRef.current = autoOpenReviewKey;
    openReviewPayload('install-complete');
  }, [openReviewPayload, reviewableInstallResult, reviewProfile]);

  return (
    <section className="crosshook-install-shell" aria-labelledby="install-game-heading">
      {hasCoverHero ? (
        <div className="crosshook-profile-hero">
          {coverArtUrl ? (
            <>
              <img
                src={coverArtUrl}
                className="crosshook-profile-hero__art"
                alt=""
                aria-hidden="true"
              />
              <div className="crosshook-profile-hero__gradient" />
            </>
          ) : (
            <div className="crosshook-profile-hero__skeleton crosshook-skeleton" />
          )}
          <div className="crosshook-profile-hero__content">
            <div className="crosshook-heading-eyebrow">Install Game</div>
            <h3 id="install-game-heading" className="crosshook-heading-title crosshook-heading-title--install">
              Guided install shell
            </h3>
            <p className="crosshook-heading-copy">
              This flow resolves a default prefix, runs the installer through Proton, and hands back a reviewable profile
              without saving it yet.
            </p>
          </div>
        </div>
      ) : (
        <div className="crosshook-install-intro">
          <div className="crosshook-heading-eyebrow">Install Game</div>
          <h3 id="install-game-heading" className="crosshook-heading-title crosshook-heading-title--install">
            Guided install shell
          </h3>
          <p className="crosshook-heading-copy">
            This flow resolves a default prefix, runs the installer through Proton, and hands back a reviewable profile
            without saving it yet.
          </p>
        </div>
      )}

      <Tabs.Root
        className="crosshook-install-flow-tabs"
        style={gameColorStyle}
        value={activeInstallTab}
        onValueChange={(value) => setActiveInstallTab(isInstallFlowTabId(value) ? value : 'identity')}
      >
        <Tabs.List
          className={`crosshook-subtab-row${dominantColor ? ' crosshook-subtab-row--themed' : ''}`}
          aria-label="Install flow sections"
        >
          {installFlowTabs.map(({ id, label }) => (
            <Tabs.Trigger
              key={id}
              value={id}
              className={`crosshook-subtab${activeInstallTab === id ? ' crosshook-subtab--active' : ''}`}
            >
              {label}
            </Tabs.Trigger>
          ))}
        </Tabs.List>

        <Tabs.Content
          value="identity"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeInstallTab === 'identity' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
            <div className="crosshook-install-section">
              <div className="crosshook-install-grid">
                <InstallField
                  label="Profile Name"
                  value={request.profile_name}
                  onChange={(value) => updateRequest('profile_name', value)}
                  placeholder="god-of-war-ragnarok"
                  helpText="Saved profile identifier and default prefix slug."
                  error={validation.fieldErrors.profile_name}
                />

                <InstallField
                  label="Display Name"
                  value={request.display_name}
                  onChange={(value) => updateRequest('display_name', value)}
                  placeholder="God of War Ragnarok"
                  helpText="Optional friendly name for the generated profile."
                  error={validation.fieldErrors.display_name}
                />

                <InstallField
                  label="Custom Cover Art"
                  value={request.custom_cover_art_path}
                  onChange={(value) => updateRequest('custom_cover_art_path', value)}
                  placeholder="/path/to/cover.png"
                  browseLabel="Browse"
                  browseTitle="Select Custom Cover Art"
                  browseFilters={[{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }]}
                  helpText="Overrides Steam/SteamGridDB. Shown as a full-width backdrop behind profile tabs (cropped to fill). Steam's store header is 460×215 (~2.14:1); larger landscape art (e.g. 920×430) with focal detail toward the top works well."
                  error={validation.fieldErrors.custom_cover_art_path}
                />

                <InstallField
                  label="Launcher Icon"
                  value={request.launcher_icon_path}
                  onChange={(value) => updateRequest('launcher_icon_path', value)}
                  placeholder="/path/to/icon.png"
                  browseLabel="Browse"
                  browseTitle="Select Launcher Icon"
                  browseFilters={[{ name: 'Images', extensions: ['png', 'jpg', 'jpeg'] }]}
                  helpText="Optional icon for the desktop launcher entry."
                />
              </div>
            </div>
          </div>
        </Tabs.Content>

        <Tabs.Content
          value="media"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeInstallTab === 'media' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
            <div className="crosshook-install-section">
              <div className="crosshook-install-grid">
                <InstallField
                  label="Installer EXE"
                  value={request.installer_path}
                  onChange={(value) => updateRequest('installer_path', value)}
                  placeholder="/mnt/media/setup.exe"
                  browseLabel="Browse"
                  browseTitle="Select Installer Executable"
                  browseFilters={[{ name: 'Windows Executable', extensions: ['exe'] }]}
                  helpText="Choose the installer media, not the final game executable."
                  error={validation.fieldErrors.installer_path}
                />

                <InstallField
                  label="Trainer EXE"
                  value={request.trainer_path}
                  onChange={(value) => updateRequest('trainer_path', value)}
                  placeholder="/mnt/media/trainer.exe"
                  browseLabel="Browse"
                  browseTitle="Select Optional Trainer Executable"
                  browseFilters={[{ name: 'Windows Executable', extensions: ['exe'] }]}
                  helpText="Optional. The review step keeps trainer media separate from the game executable."
                  error={validation.fieldErrors.trainer_path}
                />
              </div>
            </div>
          </div>
        </Tabs.Content>

        <Tabs.Content
          value="runtime"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeInstallTab === 'runtime' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
            <div className="crosshook-install-section">
              <div className="crosshook-install-runtime-stack">
                <ProtonPathField
                  value={request.proton_path}
                  onChange={(value) => updateRequest('proton_path', value)}
                  error={validation.fieldErrors.proton_path}
                  installs={protonInstalls}
                  installsError={protonInstallsError}
                />

                <InstallField
                  label="Prefix Path"
                  value={request.prefix_path}
                  onChange={(value) => updateRequest('prefix_path', value)}
                  placeholder="/home/user/.local/share/crosshook/prefixes/god-of-war-ragnarok"
                  browseLabel="Browse"
                  browseMode="directory"
                  browseTitle="Select Prefix Directory"
                  helpText={
                    defaultPrefixPathState === 'loading'
                      ? 'Resolving the default prefix from the entered profile name.'
                      : defaultPrefixPath.trim().length > 0
                        ? `Suggested default prefix: ${defaultPrefixPath}`
                        : 'Defaults under ~/.local/share/crosshook/prefixes/<slug> and stays editable.'
                  }
                  error={validation.fieldErrors.prefix_path || defaultPrefixPathError}
                  className="crosshook-install-prefix-field"
                />
              </div>
            </div>
          </div>
        </Tabs.Content>

        <Tabs.Content
          value="review"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeInstallTab === 'review' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
            <div className="crosshook-install-card">
              <div className="crosshook-install-status">
                <div>
                  <div className="crosshook-install-stage">{stageLabel(stage)}</div>
                  <h4 style={{ margin: '10px 0 0', fontSize: '1.05rem' }}>Status and review space</h4>
                  <p className="crosshook-heading-copy" style={{ marginTop: 8 }}>
                    {statusText}
                  </p>
                </div>
                <div style={{ display: 'grid', gap: 10, justifyItems: 'end' }}>
                  <div className="crosshook-install-pill">{prefixStateLabel(defaultPrefixPathState)}</div>
                  <div className="crosshook-install-pill">Candidates: {candidateCount}</div>
                </div>
              </div>

              <div className="crosshook-install-review">
                {error ? <p className="crosshook-danger">{error}</p> : null}
                {validation.generalError ? <p className="crosshook-danger">{validation.generalError}</p> : null}
                <p className="crosshook-help-text">{hintText}</p>

                <InstallField
                  label="Final Executable"
                  value={request.installed_game_executable_path}
                  onChange={(value) => setInstalledExecutablePath(value)}
                  placeholder="/home/user/.local/share/crosshook/prefixes/god-of-war-ragnarok/drive_c/Game/Game.exe"
                  browseLabel="Browse"
                  browseTitle="Select Installed Game Executable"
                  browseFilters={[{ name: 'Windows Executable', extensions: ['exe'] }]}
                  helpText="Selecting a candidate fills this field, but it remains editable for the final review step."
                  error={validation.fieldErrors.installed_game_executable_path}
                />

                {candidateOptions.length > 0 ? (
                  <div className="crosshook-install-candidate-list">
                    {candidateOptions.map((candidate) => (
                      <CandidateRow
                        key={`${candidate.index}:${candidate.path}`}
                        candidate={candidate}
                        currentPath={request.installed_game_executable_path}
                        onSelect={setInstalledExecutablePath}
                      />
                    ))}
                  </div>
                ) : (
                  <p className="crosshook-help-text">
                    {isRunningInstaller
                      ? 'Candidate discovery will appear after the installer exits.'
                      : 'No executable candidates have been discovered yet.'}
                  </p>
                )}

                <div className="crosshook-install-candidate-list">
                  <div className="crosshook-install-candidate" style={{ cursor: 'default' }}>
                    <span>Generated profile preview</span>
                    <span>
                      {reviewProfile?.game.name || request.display_name || request.profile_name || 'Unnamed profile'}
                    </span>
                  </div>
                  <div className="crosshook-install-candidate" style={{ cursor: 'default' }}>
                    <span>Runtime target</span>
                    <span>
                      {reviewProfile?.game.executable_path ||
                        request.installed_game_executable_path ||
                        'Awaiting executable confirmation'}
                    </span>
                  </div>
                  <div className="crosshook-install-candidate" style={{ cursor: 'default' }}>
                    <span>Prefix</span>
                    <span>
                      {reviewProfile?.runtime.prefix_path || request.prefix_path || 'Awaiting prefix resolution'}
                    </span>
                  </div>
                  <div className="crosshook-install-candidate" style={{ cursor: 'default' }}>
                    <span>Working directory</span>
                    <span>
                      {reviewProfile?.runtime.working_directory || 'Will be derived from the selected executable'}
                    </span>
                  </div>
                </div>

                {logPath ? (
                  <div className="crosshook-install-candidate" style={{ cursor: 'default', flexDirection: 'column' }}>
                    <span>Installer log path</span>
                    <span style={{ wordBreak: 'break-all', color: 'var(--crosshook-color-text)' }}>{logPath}</span>
                  </div>
                ) : (
                  <p className="crosshook-help-text">
                    Installer logs will be exposed here once the backend command returns a log path.
                  </p>
                )}
              </div>
            </div>
          </div>
        </Tabs.Content>
      </Tabs.Root>

      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
        <button
          type="button"
          className="crosshook-button"
          onClick={async () => {
            const shouldProceed = await Promise.resolve(onRequestInstallAction?.('retry') ?? true);
            if (!shouldProceed) {
              return;
            }

            await startInstall();
          }}
          disabled={isRunningInstaller || isResolvingDefaultPrefixPath}
        >
          {actionLabel}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={async () => {
            const shouldProceed = await Promise.resolve(onRequestInstallAction?.('reset') ?? true);
            if (!shouldProceed) {
              return;
            }

            reset();
          }}
        >
          Reset Form
        </button>
        <div className="crosshook-help-text" style={{ alignSelf: 'center' }}>
          {isResolvingDefaultPrefixPath
            ? 'Resolving the suggested prefix path before install.'
            : 'The generated profile stays editable until the modal save step.'}
        </div>
        {reviewableInstallResult !== null ? (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            disabled={!canReviewGeneratedProfile}
            onClick={() => openReviewPayload('manual-verify')}
          >
            Review in Modal
          </button>
        ) : null}
      </div>
    </section>
  );
}

export default InstallGamePanel;
