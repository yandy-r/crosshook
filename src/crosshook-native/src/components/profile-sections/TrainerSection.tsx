import { useId, useState } from 'react';
import { useTrainerTypeCatalog } from '../../hooks/useTrainerTypeCatalog';
import type { GameProfile, LaunchMethod } from '../../types';
import { chooseFile } from '../../utils/dialog';
import { OfflineTrainerInfoModal, type TrainerInfoModalKey } from '../OfflineTrainerInfoModal';
import { FieldRow, OptionalSection, TrainerVersionSetField } from '../ProfileFormSections';
import { InfoTooltip } from '../ui/InfoTooltip';
import { ThemedSelect } from '../ui/ThemedSelect';

function isSupportedTrainerInfoModal(value: string | undefined | null): value is TrainerInfoModalKey {
  return value === 'aurora_offline_setup' || value === 'wemod_offline_info';
}

export interface TrainerSectionProps {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  reviewMode?: boolean;
  launchMethod: LaunchMethod;
  profileName: string;
  profileExists?: boolean;
  trainerVersion?: string | null;
  onVersionSet?: () => void;
}

/**
 * Renders the "Trainer" section with trainer path, type, loading mode, and version fields.
 * Hidden when launchMethod is 'native' (native launch does not support trainer injection).
 * Collapsed when reviewMode is true and no trainer path is set.
 */
export function TrainerSection({
  profile,
  onUpdateProfile,
  reviewMode = false,
  launchMethod,
  profileName,
  profileExists = false,
  trainerVersion = null,
  onVersionSet,
}: TrainerSectionProps) {
  const sectionId = useId();
  const {
    catalog: trainerTypeCatalog,
    error: trainerTypeCatalogError,
    selectOptions: trainerTypeSelectOptions,
  } = useTrainerTypeCatalog();
  const [trainerInfoModal, setTrainerInfoModal] = useState<TrainerInfoModalKey | null>(null);

  const supportsTrainerLaunch = launchMethod !== 'native';

  if (!supportsTrainerLaunch) {
    return null;
  }

  const currentTrainerTypeId = profile.trainer.trainer_type?.trim() || 'unknown';
  const selectedTrainerTypeEntry = trainerTypeCatalog.find((e) => e.id === currentTrainerTypeId);
  const trainerRequiresNetwork = selectedTrainerTypeEntry?.requires_network === true;
  const networkIsolation = profile.launch.network_isolation ?? true;
  const trainerCollapsed = reviewMode && profile.trainer.path.trim().length === 0;

  return (
    <div className="crosshook-install-section">
      <OfflineTrainerInfoModal
        open={trainerInfoModal !== null}
        modalKey={trainerInfoModal}
        onClose={() => setTrainerInfoModal(null)}
      />

      <div className="crosshook-install-section-title">Trainer</div>
      <OptionalSection summary="Trainer details" collapsed={trainerCollapsed}>
        <div className="crosshook-install-grid">
          <FieldRow
            label="Trainer Path"
            value={profile.trainer.path}
            onChange={(value) =>
              onUpdateProfile((current) => ({
                ...current,
                trainer: { ...current.trainer, path: value },
              }))
            }
            placeholder="/path/to/trainer.exe"
            browseLabel="Browse"
            onBrowse={async () => {
              const path = await chooseFile('Select Trainer Executable', [
                { name: 'Windows Executable', extensions: ['exe'] },
              ]);

              if (path) {
                onUpdateProfile((current) => ({
                  ...current,
                  trainer: { ...current.trainer, path },
                }));
              }
            }}
          />

          <div className="crosshook-field">
            <label className="crosshook-label" htmlFor={`${sectionId}-trainer-type`}>
              Trainer type (offline scoring)
            </label>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 10, alignItems: 'center' }}>
              <div style={{ flex: '1 1 200px', minWidth: 0 }}>
                <ThemedSelect
                  id={`${sectionId}-trainer-type`}
                  value={currentTrainerTypeId}
                  onValueChange={(value) =>
                    onUpdateProfile((current) => ({
                      ...current,
                      trainer: { ...current.trainer, trainer_type: value },
                    }))
                  }
                  options={trainerTypeSelectOptions}
                />
              </div>
              {selectedTrainerTypeEntry && isSupportedTrainerInfoModal(selectedTrainerTypeEntry.info_modal) ? (
                <button
                  type="button"
                  className="crosshook-button crosshook-button--secondary"
                  onClick={() => {
                    const k = selectedTrainerTypeEntry.info_modal;
                    if (isSupportedTrainerInfoModal(k)) {
                      setTrainerInfoModal(k);
                    }
                  }}
                >
                  Offline help
                </button>
              ) : null}
            </div>
            {trainerTypeCatalogError ? (
              <p className="crosshook-help-text" role="status">
                {trainerTypeCatalogError} Showing &quot;Unknown&quot; only until the catalog loads.
              </p>
            ) : (
              <p className="crosshook-help-text">
                Used for offline readiness scoring. Separate from the display &quot;type&quot; label in exported
                metadata.
              </p>
            )}
          </div>

          <div className="crosshook-field">
            <label className="crosshook-label" htmlFor={`${sectionId}-trainer-loading-mode`}>
              Trainer Loading Mode
            </label>
            <ThemedSelect
              id={`${sectionId}-trainer-loading-mode`}
              value={profile.trainer.loading_mode}
              onValueChange={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  trainer: {
                    ...current.trainer,
                    loading_mode: value as typeof current.trainer.loading_mode,
                  },
                }))
              }
              options={[
                { value: 'source_directory', label: 'Run from current directory' },
                { value: 'copy_to_prefix', label: 'Copy into prefix' },
              ]}
            />
            <p className="crosshook-help-text">
              Use the original trainer location by default so stateful bundles like Aurora keep one shared install.
              Switch to copy mode only when a trainer requires prefix-local files.
            </p>
          </div>

          <div className="crosshook-field">
            <label className="crosshook-settings-checkbox-row">
              <input
                id={`${sectionId}-network-isolation`}
                type="checkbox"
                checked={trainerRequiresNetwork ? false : networkIsolation}
                disabled={trainerRequiresNetwork}
                onChange={(event) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    launch: { ...current.launch, network_isolation: event.target.checked },
                  }))
                }
                className="crosshook-settings-checkbox"
              />
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
                Network isolation
                <InfoTooltip
                  content={
                    trainerRequiresNetwork
                      ? `Disabled \u2014 ${selectedTrainerTypeEntry?.display_name ?? 'This trainer type'} requires network access.`
                      : 'Isolates the trainer in a network namespace via unshare --net, blocking outbound connections like telemetry and update checks. This function may not work on all platforms.'
                  }
                />
              </span>
            </label>
          </div>

          {trainerVersion ? (
            <div className="crosshook-field">
              <label className="crosshook-label">Trainer Version</label>
              <input
                className="crosshook-input"
                value={trainerVersion}
                readOnly
                aria-readonly="true"
                style={{ opacity: 0.7 }}
              />
              <p className="crosshook-help-text">Version recorded at last successful launch.</p>
            </div>
          ) : null}

          {profileExists && !reviewMode ? (
            <TrainerVersionSetField profileName={profileName} onVersionSet={onVersionSet} />
          ) : null}
        </div>
      </OptionalSection>
    </div>
  );
}

export default TrainerSection;
