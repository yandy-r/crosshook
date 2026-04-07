import { type CSSProperties, useState } from 'react';

import type { ProtonInstallOption } from './ProfileFormSections';
import { GameMetadataBar } from './profile-sections/GameMetadataBar';
import { InstallField } from './ui/InstallField';
import { ProtonPathField } from './ui/ProtonPathField';
import { ThemedSelect } from './ui/ThemedSelect';
import { useGameCoverArt } from '../hooks/useGameCoverArt';
import { useImageDominantColor } from '../hooks/useImageDominantColor';
import { useUpdateGame } from '../hooks/useUpdateGame';
import type { UpdateGameStage } from '../types';

export interface UpdateGamePanelProps {
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
}

function fileNameFromPath(path: string): string {
  const normalized = path.trim().replace(/\\/g, '/');
  return normalized.split('/').pop() || normalized || 'update executable';
}

function stageLabel(stage: UpdateGameStage): string {
  switch (stage) {
    case 'preparing':
      return 'Preparing';
    case 'running_updater':
      return 'Running updater';
    case 'complete':
      return 'Complete';
    case 'failed':
      return 'Failed';
    case 'idle':
    default:
      return 'Idle';
  }
}

export function UpdateGamePanel({ protonInstalls, protonInstallsError }: UpdateGamePanelProps) {
  const {
    request,
    validation,
    stage,
    result,
    error,
    profiles,
    profilesError,
    isLoadingProfiles,
    selectedProfile,
    profileCoverSource,
    updateField,
    statusText,
    hintText,
    actionLabel,
    canStart,
    isRunning,
    populateFromProfile,
    startUpdate,
    reset,
  } = useUpdateGame();

  const [showConfirmation, setShowConfirmation] = useState(false);
  const logPath = result?.helper_log_path ?? '';

  const steamAppIdForCover = profileCoverSource?.steamAppId;
  const customCoverForHook =
    profileCoverSource && profileCoverSource.customCoverArtPath.trim().length > 0
      ? profileCoverSource.customCoverArtPath.trim()
      : undefined;
  const { coverArtUrl, loading: coverArtLoading } = useGameCoverArt(steamAppIdForCover, customCoverForHook);
  const dominantColor = useImageDominantColor(coverArtUrl);
  const gameColorStyle: CSSProperties | undefined = dominantColor
    ? ({
        '--crosshook-game-color-r': String(dominantColor[0]),
        '--crosshook-game-color-g': String(dominantColor[1]),
        '--crosshook-game-color-b': String(dominantColor[2]),
      } as CSSProperties)
    : undefined;
  const hasCoverHero = profileCoverSource !== null && (Boolean(coverArtUrl) || coverArtLoading);

  const heroCopy =
    'Run a Windows update executable against an existing Proton prefix. Select a profile to auto-fill the prefix and Proton paths.';

  return (
    <section
      className="crosshook-install-shell"
      aria-labelledby="update-game-heading"
      data-crosshook-focus-zone
      style={gameColorStyle}
    >
      <div className="crosshook-install-shell__content">
        {hasCoverHero ? (
          <div className="crosshook-profile-hero">
            {coverArtUrl ? (
              <>
                <img src={coverArtUrl} className="crosshook-profile-hero__art" alt="" aria-hidden="true" />
                <div className="crosshook-profile-hero__gradient" />
              </>
            ) : (
              <div className="crosshook-profile-hero__skeleton crosshook-skeleton" />
            )}
            <div className="crosshook-profile-hero__content">
              <GameMetadataBar steamAppId={steamAppIdForCover} />
              {!steamAppIdForCover && selectedProfile ? (
                <div className="crosshook-game-metadata-bar">
                  <span className="crosshook-game-metadata-bar__name">{selectedProfile}</span>
                </div>
              ) : null}
              <div className="crosshook-heading-eyebrow">Update Game</div>
              <p className="crosshook-heading-copy">{heroCopy}</p>
            </div>
          </div>
        ) : (
          <div className="crosshook-install-intro">
            <div className="crosshook-heading-eyebrow">Update Game</div>
            <p className="crosshook-heading-copy">{heroCopy}</p>
          </div>
        )}

        <div className="crosshook-install-section">
          <div className="crosshook-install-section-title">Profile</div>
          <div className="crosshook-field">
            <label className="crosshook-label" htmlFor="update-profile-select">
              Target Profile
            </label>
            <ThemedSelect
              id="update-profile-select"
              value={selectedProfile}
              onValueChange={(value) => {
                if (value.trim().length > 0) {
                  void populateFromProfile(value);
                }
              }}
              placeholder={
                isLoadingProfiles
                  ? 'Loading profiles...'
                  : profiles.length === 0
                    ? 'No proton_run profiles found'
                    : 'Select a profile'
              }
              options={profiles.map((name) => ({ value: name, label: name }))}
            />
            {profilesError ? <p className="crosshook-danger">{profilesError}</p> : null}
          </div>
        </div>

        <div className="crosshook-install-section">
          <div className="crosshook-install-section-title">Update media</div>
          <div className="crosshook-install-grid">
            <InstallField
              label="Update Executable"
              value={request.updater_path}
              onChange={(value) => updateField('updater_path', value)}
              placeholder="/mnt/media/update.exe"
              browseLabel="Browse"
              browseTitle="Select Update Executable"
              browseFilters={[{ name: 'Windows Executable', extensions: ['exe'] }]}
              helpText="The Windows .exe to run inside the Proton prefix."
              error={validation.fieldErrors.updater_path}
            />
          </div>
        </div>

        <div className="crosshook-install-section">
          <div className="crosshook-install-section-title">Runtime</div>
          <div className="crosshook-install-runtime-stack">
            <ProtonPathField
              value={request.proton_path}
              onChange={(value) => updateField('proton_path', value)}
              error={validation.fieldErrors.proton_path}
              installs={protonInstalls}
              installsError={protonInstallsError}
              idPrefix="update"
            />

            <InstallField
              label="Prefix Path"
              value={request.prefix_path}
              onChange={(value) => updateField('prefix_path', value)}
              placeholder="/home/user/.local/share/crosshook/prefixes/game-name"
              browseLabel="Browse"
              browseMode="directory"
              browseTitle="Select Prefix Directory"
              helpText="The existing Proton prefix directory for the selected profile."
              error={validation.fieldErrors.prefix_path}
            />
          </div>
        </div>

        <div className="crosshook-install-card">
          <div className="crosshook-install-status">
            <div>
              <div className="crosshook-install-stage">{stageLabel(stage)}</div>
              <h4 style={{ margin: '10px 0 0', fontSize: '1.05rem' }}>Status</h4>
              <p className="crosshook-heading-copy" style={{ marginTop: 8 }}>
                {statusText}
              </p>
            </div>
          </div>

          <div className="crosshook-install-review">
            {error ? <p className="crosshook-danger">{error}</p> : null}
            {validation.generalError ? <p className="crosshook-danger">{validation.generalError}</p> : null}
            <p className="crosshook-help-text">{hintText}</p>

            {logPath ? (
              <div className="crosshook-install-candidate" style={{ cursor: 'default', flexDirection: 'column' }}>
                <span>Update log path</span>
                <span style={{ wordBreak: 'break-all', color: 'var(--crosshook-color-text)' }}>{logPath}</span>
              </div>
            ) : null}
          </div>
        </div>
      </div>

      <div className="crosshook-install-shell__footer crosshook-route-footer">
        <div className="crosshook-install-shell__actions">
          <button
            type="button"
            className="crosshook-button"
            onClick={() => setShowConfirmation(true)}
            disabled={isRunning || !canStart}
          >
            {actionLabel}
          </button>
          <button type="button" className="crosshook-button crosshook-button--secondary" onClick={() => reset()}>
            Reset
          </button>
        </div>
      </div>

      {showConfirmation && (
        <div className="crosshook-modal-overlay" onClick={() => setShowConfirmation(false)}>
          <div className="crosshook-modal-dialog" onClick={(e) => e.stopPropagation()}>
            <h4>Apply update to {selectedProfile}?</h4>
            <p>
              This will run {fileNameFromPath(request.updater_path)} inside the Proton prefix. This action cannot be
              automatically undone.
            </p>
            <div style={{ display: 'flex', gap: 12, justifyContent: 'flex-end', marginTop: 16 }}>
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={() => setShowConfirmation(false)}
                autoFocus
              >
                Cancel
              </button>
              <button
                type="button"
                className="crosshook-button"
                onClick={() => {
                  setShowConfirmation(false);
                  void startUpdate();
                }}
              >
                Apply Update
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

export default UpdateGamePanel;
