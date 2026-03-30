import { invoke } from '@tauri-apps/api/core';
import { useEffect, useMemo, useState } from 'react';
import type { CommunityImportPreview } from '../hooks/useCommunityProfiles';
import { isLaunchValidationIssue, type LaunchPreview, type LaunchRequest, type LaunchValidationIssue } from '../types';
import type { GameProfile, LaunchMethod } from '../types/profile';
import ProfileReviewModal from './ProfileReviewModal';

type SteamFieldState = 'Idle' | 'Saved' | 'NotFound' | 'Found' | 'Ambiguous';

interface SteamAutoPopulateRequest {
  game_path: string;
  steam_client_install_path: string;
}

interface SteamAutoPopulateResult {
  app_id_state: SteamFieldState;
  app_id: string;
  compatdata_state: SteamFieldState;
  compatdata_path: string;
  proton_state: SteamFieldState;
  proton_path: string;
  diagnostics: string[];
  manual_hints: string[];
}

interface CommunityImportResolutionSummary {
  autoResolvedCount: number;
  unresolvedCount: number;
}

interface CommunityImportWizardModalProps {
  open: boolean;
  draft: CommunityImportPreview | null;
  saving: boolean;
  onClose: () => void;
  onSave: (
    profileName: string,
    profile: GameProfile,
    summary: CommunityImportResolutionSummary
  ) => Promise<void>;
}

const STEP_LABELS = ['Profile Details', 'Auto-Resolve', 'Manual Adjustment', 'Validate & Save'] as const;

function normalizeProfile(profile: GameProfile): GameProfile {
  return {
    ...profile,
    trainer: {
      ...profile.trainer,
      loading_mode: profile.trainer?.loading_mode ?? 'source_directory',
    },
    steam: {
      ...profile.steam,
      launcher: {
        icon_path: profile.steam?.launcher?.icon_path ?? '',
        display_name: profile.steam?.launcher?.display_name ?? '',
      },
    },
    runtime: {
      prefix_path: profile.runtime?.prefix_path ?? '',
      proton_path: profile.runtime?.proton_path ?? '',
      working_directory: profile.runtime?.working_directory ?? '',
    },
    launch: {
      ...profile.launch,
      method: profile.launch?.method ?? 'proton_run',
      optimizations: {
        enabled_option_ids: profile.launch?.optimizations?.enabled_option_ids ?? [],
      },
    },
    local_override: profile.local_override ?? {
      game: { executable_path: '' },
      trainer: { path: '' },
      steam: {
        compatdata_path: '',
        proton_path: '',
      },
      runtime: {
        prefix_path: '',
        proton_path: '',
      },
    },
  };
}

function resolveLaunchMethod(profile: GameProfile): Exclude<LaunchMethod, ''> {
  if (profile.launch?.method === 'steam_applaunch' || profile.launch?.method === 'proton_run' || profile.launch?.method === 'native') {
    return profile.launch.method;
  }

  if (profile.steam.enabled) {
    return 'steam_applaunch';
  }

  if (profile.game.executable_path.trim().toLowerCase().endsWith('.exe')) {
    return 'proton_run';
  }

  return 'native';
}

function buildLaunchRequest(profile: GameProfile, steamClientInstallPath: string): LaunchRequest {
  const method = resolveLaunchMethod(profile);
  return {
    method,
    game_path: profile.game.executable_path,
    trainer_path: profile.trainer.path,
    trainer_host_path: profile.trainer.path,
    trainer_loading_mode: profile.trainer.loading_mode,
    steam: {
      app_id: profile.steam.app_id,
      compatdata_path: profile.steam.compatdata_path,
      proton_path: profile.steam.proton_path,
      steam_client_install_path: steamClientInstallPath,
    },
    runtime: {
      prefix_path: profile.runtime.prefix_path,
      proton_path: profile.runtime.proton_path,
      working_directory: profile.runtime.working_directory,
    },
    optimizations: {
      enabled_option_ids: method === 'proton_run' ? profile.launch.optimizations.enabled_option_ids : [],
    },
    launch_game_only: false,
    launch_trainer_only: false,
  };
}

function toStatusClass(state: SteamFieldState): string {
  switch (state) {
    case 'Found':
      return 'found';
    case 'NotFound':
      return 'not-found';
    case 'Ambiguous':
      return 'ambiguous';
    case 'Saved':
      return 'saved';
    case 'Idle':
    default:
      return 'idle';
  }
}

export function CommunityImportWizardModal({
  open,
  draft,
  saving,
  onClose,
  onSave,
}: CommunityImportWizardModalProps) {
  const [step, setStep] = useState(0);
  const [profileName, setProfileName] = useState('');
  const [profile, setProfile] = useState<GameProfile | null>(null);
  const [steamClientInstallPath, setSteamClientInstallPath] = useState('');
  const [autoPopulateResult, setAutoPopulateResult] = useState<SteamAutoPopulateResult | null>(null);
  const [autoPopulateError, setAutoPopulateError] = useState<string | null>(null);
  const [autoPopulating, setAutoPopulating] = useState(false);
  const [autoResolvedFields, setAutoResolvedFields] = useState<Set<string>>(new Set());
  const [validationIssues, setValidationIssues] = useState<LaunchValidationIssue[]>([]);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [validating, setValidating] = useState(false);
  const [initialPathValues, setInitialPathValues] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!open || !draft) {
      return;
    }

    setStep(0);
    const normalizedProfile = normalizeProfile(draft.profile);
    setProfileName(draft.profile_name);
    setProfile(normalizedProfile);
    setAutoPopulateResult(null);
    setAutoPopulateError(null);
    setAutoResolvedFields(new Set());
    setValidationIssues([]);
    setValidationError(null);
    setInitialPathValues({
      game_path: normalizedProfile.game.executable_path,
      app_id: normalizedProfile.steam.app_id,
      compatdata_path: normalizedProfile.steam.compatdata_path,
      proton_path: normalizedProfile.steam.proton_path,
      prefix_path: normalizedProfile.runtime.prefix_path,
    });
  }, [draft, open]);

  useEffect(() => {
    if (!open) {
      return;
    }

    let active = true;
    void invoke<string>('default_steam_client_install_path')
      .then((value) => {
        if (active) {
          setSteamClientInstallPath(value);
        }
      })
      .catch(() => {
        if (active) {
          setSteamClientInstallPath('');
        }
      });

    return () => {
      active = false;
    };
  }, [open]);

  const runAutoPopulate = async () => {
    if (!profile) {
      return;
    }

    setAutoPopulating(true);
    setAutoPopulateError(null);
    setValidationIssues([]);
    setValidationError(null);

    try {
      const response = await invoke<SteamAutoPopulateResult>('auto_populate_steam', {
        request: {
          game_path: profile.game.executable_path,
          steam_client_install_path: steamClientInstallPath,
        } satisfies SteamAutoPopulateRequest,
      });
      setAutoPopulateResult(response);

      setProfile((currentProfile) => {
        if (!currentProfile) {
          return currentProfile;
        }
        const nextProfile = normalizeProfile({ ...currentProfile });
        const nextResolved = new Set(autoResolvedFields);

        if (
          response.app_id_state === 'Found' &&
          response.app_id.trim().length > 0 &&
          nextProfile.steam.app_id.trim().length === 0
        ) {
          nextProfile.steam = { ...nextProfile.steam, app_id: response.app_id };
          nextResolved.add('app_id');
        }
        if (
          response.compatdata_state === 'Found' &&
          response.compatdata_path.trim().length > 0 &&
          nextProfile.steam.compatdata_path.trim().length === 0
        ) {
          nextProfile.steam = { ...nextProfile.steam, compatdata_path: response.compatdata_path };
          if (nextProfile.runtime.prefix_path.trim().length === 0) {
            nextProfile.runtime = { ...nextProfile.runtime, prefix_path: response.compatdata_path };
            nextResolved.add('prefix_path');
          }
          nextResolved.add('compatdata_path');
        }
        if (
          response.proton_state === 'Found' &&
          response.proton_path.trim().length > 0 &&
          nextProfile.steam.proton_path.trim().length === 0
        ) {
          nextProfile.steam = { ...nextProfile.steam, proton_path: response.proton_path };
          nextResolved.add('proton_path');
        }

        setAutoResolvedFields(nextResolved);
        return nextProfile;
      });
    } catch (error) {
      setAutoPopulateError(error instanceof Error ? error.message : String(error));
      setAutoPopulateResult(null);
    } finally {
      setAutoPopulating(false);
    }
  };

  useEffect(() => {
    if (!open || !profile || step !== 1) {
      return;
    }
    void runAutoPopulate();
    // Auto-run once when entering step 2.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, step]);

  const validateDraft = async (): Promise<LaunchValidationIssue[]> => {
    if (!profile) {
      return [];
    }

    setValidating(true);
    setValidationError(null);
    try {
      const request = buildLaunchRequest(profile, steamClientInstallPath);
      const preview = await invoke<LaunchPreview>('preview_launch', { request });
      setValidationIssues(preview.validation.issues);
      return preview.validation.issues;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setValidationError(message);
      setValidationIssues([]);
      return [];
    } finally {
      setValidating(false);
    }
  };

  useEffect(() => {
    if (!open || step !== 3 || !profile) {
      return;
    }
    void validateDraft();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, step, profile?.game.executable_path, profile?.steam.app_id, profile?.steam.compatdata_path, profile?.steam.proton_path, profile?.runtime.prefix_path]);

  const fatalCount = validationIssues.filter((issue) => issue.severity === 'fatal').length;
  const warningCount = validationIssues.filter((issue) => issue.severity === 'warning').length;

  const unresolvedCount = useMemo(() => {
    if (!profile) {
      return 0;
    }
    const requiredPaths = [
      profile.game.executable_path,
      profile.steam.app_id,
      profile.steam.compatdata_path,
      profile.steam.proton_path,
      profile.runtime.prefix_path,
    ];
    return requiredPaths.filter((value) => value.trim().length === 0).length;
  }, [profile]);

  const hasManualAdjustments = useMemo(() => {
    if (!profile) {
      return false;
    }
    return (
      profile.game.executable_path !== initialPathValues.game_path ||
      profile.steam.app_id !== initialPathValues.app_id ||
      profile.steam.compatdata_path !== initialPathValues.compatdata_path ||
      profile.steam.proton_path !== initialPathValues.proton_path ||
      profile.runtime.prefix_path !== initialPathValues.prefix_path
    );
  }, [initialPathValues, profile]);

  const canMoveForward = profile !== null && profileName.trim().length > 0;
  const canSave = profile !== null && profileName.trim().length > 0 && fatalCount === 0 && !saving && !validating;

  const applyProfileUpdate = (updater: (current: GameProfile) => GameProfile) => {
    setProfile((current) => (current ? updater(current) : current));
    setValidationIssues([]);
    setValidationError(null);
  };

  const renderStepBody = () => {
    if (!profile || !draft) {
      return null;
    }

    if (step === 0) {
      return (
        <div className="crosshook-community-import-wizard__stack">
          <div className="crosshook-community-import-wizard__card">
            <div className="crosshook-community-import-wizard__label">Source</div>
            <div className="crosshook-community-import-wizard__mono">{draft.source_path}</div>
          </div>
          <div className="crosshook-community-import-wizard__meta-grid">
            <div className="crosshook-community-import-wizard__card">
              <div className="crosshook-community-import-wizard__label">Game</div>
              <div>{draft.manifest.metadata.game_name || profile.game.name || 'Unknown'}</div>
            </div>
            <div className="crosshook-community-import-wizard__card">
              <div className="crosshook-community-import-wizard__label">Trainer Type</div>
              <div>{profile.trainer.type || 'Unknown'}</div>
            </div>
            <div className="crosshook-community-import-wizard__card">
              <div className="crosshook-community-import-wizard__label">Launch Method</div>
              <div>{resolveLaunchMethod(profile)}</div>
            </div>
            <div className="crosshook-community-import-wizard__card">
              <div className="crosshook-community-import-wizard__label">Optimizations</div>
              <div>{profile.launch.optimizations.enabled_option_ids.length}</div>
            </div>
          </div>
          <label className="crosshook-community-import-wizard__field">
            <span className="crosshook-label">Local Profile Name</span>
            <input
              className="crosshook-input"
              value={profileName}
              onChange={(event) => setProfileName(event.target.value)}
              placeholder="community-profile"
            />
          </label>
        </div>
      );
    }

    if (step === 1) {
      return (
        <div className="crosshook-community-import-wizard__stack">
          <div className="crosshook-community-import-wizard__button-row">
            <button
              type="button"
              className="crosshook-button"
              onClick={() => void runAutoPopulate()}
              disabled={autoPopulating || profile.game.executable_path.trim().length === 0}
            >
              {autoPopulating ? 'Resolving...' : 'Re-run Auto-Resolve'}
            </button>
            <span className="crosshook-muted">
              Auto-resolved fields: {autoResolvedFields.size}
            </span>
          </div>
          {autoPopulateError ? <p className="crosshook-community-browser__error">{autoPopulateError}</p> : null}
          {autoPopulateResult ? (
            <div className="crosshook-community-import-wizard__status-grid">
              <div className={`crosshook-community-import-wizard__status crosshook-community-import-wizard__status--${toStatusClass(autoPopulateResult.app_id_state)}`}>
                App ID: {autoPopulateResult.app_id_state}
              </div>
              <div className={`crosshook-community-import-wizard__status crosshook-community-import-wizard__status--${toStatusClass(autoPopulateResult.compatdata_state)}`}>
                Compatdata: {autoPopulateResult.compatdata_state}
              </div>
              <div className={`crosshook-community-import-wizard__status crosshook-community-import-wizard__status--${toStatusClass(autoPopulateResult.proton_state)}`}>
                Proton: {autoPopulateResult.proton_state}
              </div>
            </div>
          ) : (
            <p className="crosshook-muted">Run auto-resolve to detect Steam metadata from the game executable.</p>
          )}
          {autoPopulateResult?.manual_hints?.length ? (
            <div className="crosshook-community-import-wizard__card">
              <div className="crosshook-community-import-wizard__label">Manual hints</div>
              {autoPopulateResult.manual_hints.map((hint) => (
                <div key={hint} className="crosshook-muted">{hint}</div>
              ))}
            </div>
          ) : null}
        </div>
      );
    }

    if (step === 2) {
      return (
        <div className="crosshook-community-import-wizard__stack">
          <div className="crosshook-community-import-wizard__meta-grid">
            <label className="crosshook-community-import-wizard__field">
              <span className="crosshook-label">Game Executable</span>
              <input
                className="crosshook-input"
                value={profile.game.executable_path}
                onChange={(event) =>
                  applyProfileUpdate((current) => ({
                    ...current,
                    game: { ...current.game, executable_path: event.target.value },
                  }))
                }
              />
            </label>
            <label className="crosshook-community-import-wizard__field">
              <span className="crosshook-label">Trainer Path</span>
              <input
                className="crosshook-input"
                value={profile.trainer.path}
                onChange={(event) =>
                  applyProfileUpdate((current) => ({
                    ...current,
                    trainer: { ...current.trainer, path: event.target.value },
                  }))
                }
              />
            </label>
            <label className="crosshook-community-import-wizard__field">
              <span className="crosshook-label">Steam App ID</span>
              <input
                className="crosshook-input"
                value={profile.steam.app_id}
                onChange={(event) =>
                  applyProfileUpdate((current) => ({
                    ...current,
                    steam: { ...current.steam, app_id: event.target.value },
                  }))
                }
              />
            </label>
            <label className="crosshook-community-import-wizard__field">
              <span className="crosshook-label">Compatdata Path</span>
              <input
                className="crosshook-input"
                value={profile.steam.compatdata_path}
                onChange={(event) =>
                  applyProfileUpdate((current) => ({
                    ...current,
                    steam: { ...current.steam, compatdata_path: event.target.value },
                  }))
                }
              />
            </label>
            <label className="crosshook-community-import-wizard__field">
              <span className="crosshook-label">Steam Proton Path</span>
              <input
                className="crosshook-input"
                value={profile.steam.proton_path}
                onChange={(event) =>
                  applyProfileUpdate((current) => ({
                    ...current,
                    steam: { ...current.steam, proton_path: event.target.value },
                  }))
                }
              />
            </label>
            <label className="crosshook-community-import-wizard__field">
              <span className="crosshook-label">Runtime Prefix Path</span>
              <input
                className="crosshook-input"
                value={profile.runtime.prefix_path}
                onChange={(event) =>
                  applyProfileUpdate((current) => ({
                    ...current,
                    runtime: { ...current.runtime, prefix_path: event.target.value },
                  }))
                }
              />
            </label>
          </div>
        </div>
      );
    }

    return (
      <div className="crosshook-community-import-wizard__stack">
        <div className="crosshook-community-import-wizard__button-row">
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void validateDraft()}
            disabled={validating}
          >
            {validating ? 'Validating...' : 'Re-run Validation'}
          </button>
          <span className="crosshook-muted">Fatal: {fatalCount} | Warnings: {warningCount}</span>
        </div>
        {validationError ? <p className="crosshook-community-browser__error">{validationError}</p> : null}
        {validationIssues.length > 0 ? (
          <ul className="crosshook-community-import-wizard__validation-list">
            {validationIssues.map((issue, index) => (
              <li key={`${issue.severity}-${index}`} className="crosshook-community-import-wizard__validation-item">
                <strong>[{issue.severity}]</strong> {issue.message}
                {issue.help ? <div className="crosshook-muted">{issue.help}</div> : null}
              </li>
            ))}
          </ul>
        ) : (
          <p className="crosshook-success">No validation issues reported for this draft.</p>
        )}
      </div>
    );
  };

  const footer = (
    <div className="crosshook-community-import-wizard__footer-actions">
      <button
        type="button"
        className="crosshook-button crosshook-button--secondary"
        onClick={onClose}
        disabled={saving || autoPopulating || validating}
      >
        Cancel
      </button>
      {step > 0 ? (
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => setStep((current) => Math.max(0, current - 1))}
          disabled={saving}
        >
          Back
        </button>
      ) : null}
      {step < STEP_LABELS.length - 1 ? (
        <button
          type="button"
          className="crosshook-button"
          onClick={() => setStep((current) => Math.min(STEP_LABELS.length - 1, current + 1))}
          disabled={!canMoveForward || saving}
        >
          Next
        </button>
      ) : (
        <button
          type="button"
          className="crosshook-button"
          onClick={() => {
            if (!profile) {
              return;
            }
            void (async () => {
              const request = buildLaunchRequest(profile, steamClientInstallPath);
              setValidationError(null);
              try {
                await invoke<void>('validate_launch', { request });
              } catch (error) {
                if (isLaunchValidationIssue(error)) {
                  setValidationIssues([error]);
                  setStep(3);
                  return;
                }
                setValidationError(error instanceof Error ? error.message : String(error));
                setStep(3);
                return;
              }

              await onSave(profileName, profile, {
                autoResolvedCount: autoResolvedFields.size,
                unresolvedCount,
              });
            })();
          }}
          disabled={!canSave}
        >
          {saving ? 'Saving...' : 'Save Imported Profile'}
        </button>
      )}
    </div>
  );

  const summaryProfile = profile;

  return (
    <ProfileReviewModal
      open={open && draft !== null && summaryProfile !== null}
      title="Community Import Wizard"
      statusLabel={`Step ${step + 1} of ${STEP_LABELS.length}`}
      statusTone={fatalCount > 0 ? 'warning' : 'neutral'}
      profileName={profileName || draft?.profile_name || ''}
      executablePath={summaryProfile?.game?.executable_path ?? ''}
      prefixPath={summaryProfile?.runtime?.prefix_path ?? ''}
      helperLogPath={draft?.source_path ?? ''}
      description="Review profile metadata, auto-resolve local paths, adjust unresolved values, validate, then save."
      onClose={onClose}
      footer={footer}
    >
      <div className="crosshook-community-import-wizard">
        <div className="crosshook-community-import-wizard__stepper">
          {STEP_LABELS.map((label, index) => (
            <div
              key={label}
              className={[
                'crosshook-community-import-wizard__step',
                index === step ? 'crosshook-community-import-wizard__step--active' : '',
              ].join(' ')}
            >
              <span className="crosshook-community-import-wizard__step-index">{index + 1}</span>
              <span>{label}</span>
            </div>
          ))}
        </div>
        {renderStepBody()}
        <div className="crosshook-community-import-wizard__summary">
          <span>Auto-resolved: {autoResolvedFields.size}</span>
          <span>Manual edits: {hasManualAdjustments ? 'Yes' : 'No'}</span>
          <span>Unresolved required fields: {unresolvedCount}</span>
        </div>
      </div>
    </ProfileReviewModal>
  );
}

export default CommunityImportWizardModal;
