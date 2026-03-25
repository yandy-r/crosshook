import { useCallback, useEffect, useRef, useState, type ChangeEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

import { useInstallGame } from '../hooks/useInstallGame';
import type {
  InstallGameExecutableCandidate,
  InstallGamePrefixPathState,
  InstallGameStage,
  InstallProfileReviewPayload,
  ProfileReviewSource,
} from '../types/install';

export interface InstallGamePanelProps {
  onOpenProfileReview: (payload: InstallProfileReviewPayload) => void | Promise<boolean>;
  onRequestInstallAction?: (action: 'retry' | 'reset') => boolean | Promise<boolean>;
}

type ProtonInstallOption = {
  name: string;
  path: string;
  is_official: boolean;
};

const detectedProtonInstalls: ProtonInstallOption[] = [];
function formatProtonInstallLabel(install: ProtonInstallOption, duplicateNameCounts: Record<string, number>): string {
  const baseLabel = install.name.trim() || 'Unnamed Proton install';
  if ((duplicateNameCounts[baseLabel] ?? 0) <= 1) {
    return baseLabel;
  }

  return `${baseLabel} (${install.is_official ? 'Steam' : 'Custom'})`;
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

async function chooseFile(title: string, filters?: { name: string; extensions: string[] }[]) {
  const result = await open({
    directory: false,
    multiple: false,
    title,
    filters,
  });

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return result ?? null;
}

async function chooseDirectory(title: string) {
  const result = await open({
    directory: true,
    multiple: false,
    title,
  });

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return result ?? null;
}

function InstallField(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  browseLabel?: string;
  browseTitle?: string;
  browseMode?: 'file' | 'directory';
  browseFilters?: { name: string; extensions: string[] }[];
  helpText?: string;
  error?: string | null;
  className?: string;
}) {
  return (
    <div className={props.className ? `crosshook-field ${props.className}` : 'crosshook-field'}>
      <label className="crosshook-label">{props.label}</label>
      <div className="crosshook-install-field-control">
        <input
          className="crosshook-input"
          style={{ flex: 1, minWidth: 0 }}
          value={props.value}
          placeholder={props.placeholder}
          onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
        />
        {props.browseLabel ? (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={async () => {
              const path =
                props.browseMode === 'directory'
                  ? await chooseDirectory(props.browseTitle ?? `Select ${props.label}`)
                  : await chooseFile(props.browseTitle ?? `Select ${props.label}`, props.browseFilters);

              if (path) {
                props.onChange(path);
              }
            }}
          >
            {props.browseLabel}
          </button>
        ) : null}
      </div>
      {props.helpText ? <p className="crosshook-help-text">{props.helpText}</p> : null}
      {props.error ? <p className="crosshook-danger">{props.error}</p> : null}
    </div>
  );
}

function ProtonPathField(props: {
  value: string;
  onChange: (value: string) => void;
  error?: string | null;
  installs: ProtonInstallOption[];
  installsError: string | null;
}) {
  const duplicateNameCounts = props.installs.reduce<Record<string, number>>((counts, install) => {
    const key = install.name.trim() || 'Unnamed Proton install';
    counts[key] = (counts[key] ?? 0) + 1;
    return counts;
  }, {});
  const selectedPath = props.installs.find((install) => install.path.trim() === props.value.trim())?.path ?? '';

  return (
    <div className="crosshook-field crosshook-install-proton-field">
      <label className="crosshook-label" htmlFor="install-detected-proton">
        Proton Path
      </label>
      <div style={{ display: 'grid', gap: 10 }}>
        <select
          id="install-detected-proton"
          className="crosshook-select"
          value={selectedPath}
          onChange={(event) => {
            if (event.target.value.trim().length > 0) {
              props.onChange(event.target.value);
            }
          }}
        >
          <option value="">Detected Proton install</option>
          {props.installs.map((install) => (
            <option key={install.path} value={install.path}>
              {formatProtonInstallLabel(install, duplicateNameCounts)}
            </option>
          ))}
        </select>

        <div className="crosshook-install-field-control">
          <input
            id="install-proton-path"
            className="crosshook-input"
            style={{ flex: 1, minWidth: 0 }}
            value={props.value}
            onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
            placeholder="/home/user/.steam/root/steamapps/common/Proton - Experimental/proton"
          />
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={async () => {
              const path = await chooseFile('Select Proton Executable');
              if (path) {
                props.onChange(path);
              }
            }}
          >
            Browse
          </button>
        </div>
      </div>

      <p className="crosshook-help-text">
        Pick a detected Proton install to fill this field automatically, or edit the path manually.
      </p>
      {props.error ? <p className="crosshook-danger">{props.error}</p> : null}
      {props.installsError ? <p className="crosshook-danger">{props.installsError}</p> : null}
    </div>
  );
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

  const candidateCount = candidateOptions.length;
  const logPath = result?.helper_log_path ?? '';
  const reviewableInstallResult = result?.succeeded === true && reviewProfile !== null ? result : null;
  const canReviewGeneratedProfile = reviewableInstallResult !== null && reviewProfile !== null;
  const lastAutoOpenReviewKeyRef = useRef<string | null>(null);
  const [protonInstalls, setProtonInstalls] = useState<ProtonInstallOption[]>(detectedProtonInstalls);
  const [protonInstallsError, setProtonInstallsError] = useState<string | null>(null);

  const openReviewPayload = useCallback(
    (source: ProfileReviewSource) => {
      if (reviewableInstallResult === null || reviewProfile === null) {
        return;
      }

      void onOpenProfileReview({
        source,
        profileName: reviewableInstallResult.profile_name.trim() || request.profile_name.trim(),
        generatedProfile: reviewProfile,
        candidateOptions,
        helperLogPath: logPath,
        message: reviewableInstallResult.message,
      });
    },
    [candidateOptions, logPath, onOpenProfileReview, request.profile_name, reviewProfile, reviewableInstallResult],
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

  useEffect(() => {
    let active = true;

    async function loadProtonInstalls() {
      try {
        const installs = await invoke<ProtonInstallOption[]>('list_proton_installs');
        const sortedInstalls = [...installs].sort((left, right) => {
          if (left.is_official !== right.is_official) {
            return left.is_official ? -1 : 1;
          }

          return left.name.localeCompare(right.name) || left.path.localeCompare(right.path);
        });

        if (!active) {
          return;
        }

        setProtonInstalls(sortedInstalls);
        setProtonInstallsError(null);
      } catch (loadError) {
        if (!active) {
          return;
        }

        setProtonInstalls([]);
        setProtonInstallsError(loadError instanceof Error ? loadError.message : String(loadError));
      }
    }

    void loadProtonInstalls();

    return () => {
      active = false;
    };
  }, []);

  return (
    <section className="crosshook-install-shell" aria-labelledby="install-game-heading">
      <div className="crosshook-install-intro">
        <div className="crosshook-heading-eyebrow">Install Game</div>
        <h3 id="install-game-heading" className="crosshook-heading-title" style={{ fontSize: '1.5rem' }}>
          Guided install shell
        </h3>
        <p className="crosshook-heading-copy">
          This tab resolves a default prefix, runs the installer through Proton, and hands back a reviewable profile
          without saving it yet.
        </p>
      </div>

      <div className="crosshook-install-section">
        <div className="crosshook-install-section-title">Profile identity</div>
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
        </div>
      </div>

      <div className="crosshook-install-section">
        <div className="crosshook-install-section-title">Install media</div>
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

      <div className="crosshook-install-section">
        <div className="crosshook-install-section-title">Runtime</div>
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
              <span>{reviewProfile?.game.name || request.display_name || request.profile_name || 'Unnamed profile'}</span>
            </div>
            <div className="crosshook-install-candidate" style={{ cursor: 'default' }}>
              <span>Runtime target</span>
              <span>{reviewProfile?.game.executable_path || request.installed_game_executable_path || 'Awaiting executable confirmation'}</span>
            </div>
            <div className="crosshook-install-candidate" style={{ cursor: 'default' }}>
              <span>Prefix</span>
              <span>{reviewProfile?.runtime.prefix_path || request.prefix_path || 'Awaiting prefix resolution'}</span>
            </div>
            <div className="crosshook-install-candidate" style={{ cursor: 'default' }}>
              <span>Working directory</span>
              <span>{reviewProfile?.runtime.working_directory || 'Will be derived from the selected executable'}</span>
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
          {isResolvingDefaultPrefixPath ? 'Resolving the suggested prefix path before install.' : 'The generated profile stays editable until the modal save step.'}
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
