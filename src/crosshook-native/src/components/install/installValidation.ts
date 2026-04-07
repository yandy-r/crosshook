import type { GameProfile } from '../../types';
import type { ResolvedLaunchMethod } from '../../utils/launch';
import {
  evaluateWizardRequiredFields,
  type WizardValidationResult,
} from '../wizard/wizardValidation';

export interface EvaluateInstallRequiredFieldsArgs {
  profileName: string;
  profile: GameProfile;
  launchMethod: ResolvedLaunchMethod;
  installerPath: string;
}

/**
 * Wizard required-field checks plus install-only `installer_path`.
 * Game executable is not required before running the installer (post-install confirmation).
 */
export function evaluateInstallRequiredFields(
  args: EvaluateInstallRequiredFieldsArgs
): WizardValidationResult {
  const wizardResult = evaluateWizardRequiredFields({
    profileName: args.profileName,
    profile: args.profile,
    launchMethod: args.launchMethod,
  });

  const fields = wizardResult.fields.map((field) =>
    field.id === 'game-executable-path' ? { ...field, isSatisfied: true } : field
  );

  const installerSatisfied = args.installerPath.trim().length > 0;
  const withInstaller: WizardValidationResult['fields'] = [
    ...fields,
    {
      id: 'installer-path',
      label: 'Installer EXE',
      isSatisfied: installerSatisfied,
    },
  ];

  const firstMissing = withInstaller.find((field) => !field.isSatisfied);

  return {
    fields: withInstaller,
    isReady: withInstaller.every((field) => field.isSatisfied),
    firstMissingId: firstMissing ? firstMissing.id : null,
  };
}
