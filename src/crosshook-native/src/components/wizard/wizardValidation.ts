import type { GameProfile, ResolvedLaunchMethod } from '../../types';

/**
 * A single required-field check surfaced in the wizard review step.
 * The set is intentionally a strict superset of what `validateProfileForSave`
 * enforces in `useProfile.ts` — the wizard's UI gate does not replace the
 * server-side save gate, it complements it.
 */
export interface WizardRequiredField {
  id: string;
  label: string;
  isSatisfied: boolean;
}

export interface WizardValidationResult {
  fields: readonly WizardRequiredField[];
  isReady: boolean;
  /** Id of the first missing field, used for aria-describedby on the disabled Save button. */
  firstMissingId: string | null;
}

export interface EvaluateWizardRequiredFieldsArgs {
  profileName: string;
  profile: GameProfile;
  launchMethod: ResolvedLaunchMethod;
}

function notBlank(value: string | undefined | null): boolean {
  return typeof value === 'string' && value.trim().length > 0;
}

/**
 * Compute the required-field list for the wizard review step given the
 * current draft profile and runner method.
 *
 * Rules:
 * - Always required: profile name, game name, game executable path, runner method.
 * - `steam_applaunch`: Steam App ID, Steam compatdata path, Steam Proton path.
 * - `proton_run`:      Proton prefix path, Proton path (Steam App ID is optional
 *                      here — see `RuntimeSection.tsx` — and is used only for art
 *                      / metadata lookups).
 * - `native`:          no additional fields.
 *
 * Pure and synchronous — no IPC, no React state. Safe to call on every render.
 */
export function evaluateWizardRequiredFields(
  args: EvaluateWizardRequiredFieldsArgs
): WizardValidationResult {
  const { profileName, profile, launchMethod } = args;

  const fields: WizardRequiredField[] = [
    {
      id: 'profile-name',
      label: 'Profile name',
      isSatisfied: notBlank(profileName),
    },
    {
      id: 'game-name',
      label: 'Game name',
      isSatisfied: notBlank(profile.game.name),
    },
    {
      id: 'game-executable-path',
      label: 'Game executable path',
      isSatisfied: notBlank(profile.game.executable_path),
    },
    {
      id: 'runner-method',
      label: 'Runner method',
      isSatisfied:
        launchMethod === 'steam_applaunch' ||
        launchMethod === 'proton_run' ||
        launchMethod === 'native',
    },
  ];

  if (launchMethod === 'steam_applaunch') {
    fields.push(
      {
        id: 'steam-app-id',
        label: 'Steam App ID',
        isSatisfied: notBlank(profile.steam.app_id),
      },
      {
        id: 'steam-compatdata-path',
        label: 'Steam prefix (compatdata) path',
        isSatisfied: notBlank(profile.steam.compatdata_path),
      },
      {
        id: 'steam-proton-path',
        label: 'Proton path',
        isSatisfied: notBlank(profile.steam.proton_path),
      }
    );
  } else if (launchMethod === 'proton_run') {
    fields.push(
      {
        id: 'runtime-prefix-path',
        label: 'Proton prefix path',
        isSatisfied: notBlank(profile.runtime.prefix_path),
      },
      {
        id: 'runtime-proton-path',
        label: 'Proton path',
        isSatisfied: notBlank(profile.runtime.proton_path),
      }
    );
  }

  const firstMissing = fields.find((field) => !field.isSatisfied);

  return {
    fields,
    isReady: fields.every((field) => field.isSatisfied),
    firstMissingId: firstMissing ? firstMissing.id : null,
  };
}
