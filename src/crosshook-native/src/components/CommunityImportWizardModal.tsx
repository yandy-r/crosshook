import { useCallback, useEffect, useMemo, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { CommunityImportPreview } from '../hooks/useCommunityProfiles';
import type { LaunchPreview, LaunchValidationIssue } from '../types';
import type { GameProfile } from '../types/profile';
import AutoResolveStep from './community-import/AutoResolveStep';
import ManualAdjustmentStep from './community-import/ManualAdjustmentStep';
import ProfileDetailsStep from './community-import/ProfileDetailsStep';
import SummaryBar from './community-import/SummaryBar';
import type {
  CommunityImportResolutionSummary,
  ProfileUpdateHandler,
  SteamAutoPopulateRequest,
  SteamAutoPopulateResult,
} from './community-import/types';
import {
  buildLaunchRequest,
  isStrictLaunchValidationIssue,
  normalizeProfile,
  resolveLaunchMethod,
} from './community-import/utils';
import ValidationStep from './community-import/ValidationStep';
import WizardStepper from './community-import/WizardStepper';
import ProfileReviewModal from './ProfileReviewModal';

const STEP_LABELS = ['Profile Details', 'Auto-Resolve', 'Manual Adjustment', 'Validate & Save'] as const;

interface CommunityImportWizardModalProps {
  open: boolean;
  draft: CommunityImportPreview | null;
  saving: boolean;
  onClose: () => void;
  onSave: (profileName: string, profile: GameProfile, summary: CommunityImportResolutionSummary) => Promise<void>;
}

export function CommunityImportWizardModal({ open, draft, saving, onClose, onSave }: CommunityImportWizardModalProps) {
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
    void callCommand<string>('default_steam_client_install_path')
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

  const runAutoPopulate = useCallback(async () => {
    if (!profile) {
      return;
    }

    setAutoPopulating(true);
    setAutoPopulateError(null);
    setValidationIssues([]);
    setValidationError(null);

    try {
      const response = await callCommand<SteamAutoPopulateResult>('auto_populate_steam', {
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
        const resolvedAdds: string[] = [];

        if (
          response.app_id_state === 'Found' &&
          response.app_id.trim().length > 0 &&
          nextProfile.steam.app_id.trim().length === 0
        ) {
          nextProfile.steam = { ...nextProfile.steam, app_id: response.app_id };
          resolvedAdds.push('app_id');
        }
        if (
          response.compatdata_state === 'Found' &&
          response.compatdata_path.trim().length > 0 &&
          nextProfile.steam.compatdata_path.trim().length === 0
        ) {
          nextProfile.steam = { ...nextProfile.steam, compatdata_path: response.compatdata_path };
          if (nextProfile.runtime.prefix_path.trim().length === 0) {
            nextProfile.runtime = { ...nextProfile.runtime, prefix_path: response.compatdata_path };
            resolvedAdds.push('prefix_path');
          }
          resolvedAdds.push('compatdata_path');
        }
        if (
          response.proton_state === 'Found' &&
          response.proton_path.trim().length > 0 &&
          nextProfile.steam.proton_path.trim().length === 0
        ) {
          nextProfile.steam = { ...nextProfile.steam, proton_path: response.proton_path };
          resolvedAdds.push('proton_path');
        }

        if (resolvedAdds.length > 0) {
          setAutoResolvedFields((prev) => {
            const nextResolved = new Set(prev);
            for (const key of resolvedAdds) {
              nextResolved.add(key);
            }
            return nextResolved;
          });
        }
        return nextProfile;
      });
    } catch (error) {
      setAutoPopulateError(error instanceof Error ? error.message : String(error));
      setAutoPopulateResult(null);
    } finally {
      setAutoPopulating(false);
    }
  }, [profile, steamClientInstallPath]);

  useEffect(() => {
    if (!open || !profile || step !== 1) {
      return;
    }
    void runAutoPopulate();
    // Auto-run once when entering step 2.
  }, [open, step, profile, runAutoPopulate]);

  const validateDraft = useCallback(async (): Promise<LaunchValidationIssue[]> => {
    if (!profile) {
      return [];
    }

    setValidating(true);
    setValidationError(null);
    try {
      const request = buildLaunchRequest(profile, steamClientInstallPath);
      const preview = await callCommand<LaunchPreview>('preview_launch', { request });
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
  }, [profile, steamClientInstallPath]);

  useEffect(() => {
    if (!open || step !== 3 || !profile) {
      return;
    }
    void validateDraft();
  }, [
    open,
    step,
    profile?.game.executable_path,
    profile?.steam.app_id,
    profile?.steam.compatdata_path,
    profile?.steam.proton_path,
    profile?.runtime.prefix_path,
    validateDraft,
    profile,
  ]);

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

  const applyProfileUpdate: ProfileUpdateHandler = (updater) => {
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
        <ProfileDetailsStep
          draft={draft}
          profile={profile}
          profileName={profileName}
          launchMethod={resolveLaunchMethod(profile)}
          onProfileNameChange={setProfileName}
        />
      );
    }

    if (step === 1) {
      return (
        <AutoResolveStep
          autoPopulating={autoPopulating}
          autoPopulateError={autoPopulateError}
          autoPopulateResult={autoPopulateResult}
          autoResolvedCount={autoResolvedFields.size}
          canRun={profile.game.executable_path.trim().length > 0}
          onRunAutoPopulate={() => void runAutoPopulate()}
        />
      );
    }

    if (step === 2) {
      return <ManualAdjustmentStep profile={profile} onProfileChange={applyProfileUpdate} />;
    }

    return (
      <ValidationStep
        fatalCount={fatalCount}
        warningCount={warningCount}
        validationIssues={validationIssues}
        validationError={validationError}
        validating={validating}
        onValidate={() => void validateDraft()}
      />
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
                await callCommand<void>('validate_launch', { request });
              } catch (error) {
                if (isStrictLaunchValidationIssue(error)) {
                  setValidationIssues([error]);
                  setStep(3);
                  return;
                }
                setValidationError(error instanceof Error ? error.message : String(error));
                setStep(3);
                return;
              }

              await onSave(profileName.trim(), profile, {
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
        <WizardStepper labels={STEP_LABELS} currentStep={step} />
        {renderStepBody()}
        <SummaryBar
          autoResolvedCount={autoResolvedFields.size}
          hasManualAdjustments={hasManualAdjustments}
          unresolvedCount={unresolvedCount}
        />
      </div>
    </ProfileReviewModal>
  );
}

export default CommunityImportWizardModal;
