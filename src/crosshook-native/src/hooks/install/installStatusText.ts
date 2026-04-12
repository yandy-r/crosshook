import type { InstallGamePrefixPathState, InstallGameResult, InstallGameStage } from '../../types/install';

export function deriveResultStage(result: InstallGameResult | null): InstallGameStage {
  if (result === null) {
    return 'idle';
  }

  if (!result.succeeded) {
    return 'failed';
  }

  return result.needs_executable_confirmation ? 'review_required' : 'ready_to_save';
}

export function deriveStatusText(
  stage: InstallGameStage,
  defaultPrefixPathState: InstallGamePrefixPathState,
  defaultPrefixPath: string,
  result: InstallGameResult | null
): string {
  if (defaultPrefixPathState === 'loading') {
    return 'Resolving the default prefix path from the current profile name.';
  }

  switch (stage) {
    case 'preparing':
      return 'Validating install inputs and preparing to launch the installer.';
    case 'running_installer':
      return 'Installer execution is in progress through Proton.';
    case 'review_required':
      return 'Installer finished. Confirm the final executable before the profile can be handed off.';
    case 'ready_to_save':
      return 'Final executable confirmed. The profile is ready for the later save handoff.';
    case 'failed':
      return result?.message || 'Install failed. Review the errors and try again.';
    default:
      return defaultPrefixPath.trim().length > 0
        ? 'Install fields are ready. CrossHook will use the suggested default prefix unless you override it.'
        : 'Fill the install form to resolve a default prefix and launch the installer.';
  }
}

export function deriveHintText(
  stage: InstallGameStage,
  _result: InstallGameResult | null,
  defaultPrefixPath: string,
  defaultPrefixPathError: string | null
): string {
  if (defaultPrefixPathError) {
    return defaultPrefixPathError;
  }

  switch (stage) {
    case 'preparing':
      return 'The backend will validate the request, create the prefix if needed, and then launch the installer.';
    case 'running_installer':
      return 'The installer log path appears after the process completes. The resulting profile stays editable.';
    case 'review_required':
      return 'Pick a candidate or browse for the installed executable. The field stays editable after selection.';
    case 'ready_to_save':
      return 'The install result is ready to hand off to the save flow in the next task.';
    case 'failed':
      return 'The install request failed. Review the error message and adjust the inputs before retrying.';
    default:
      return defaultPrefixPath.trim().length > 0
        ? 'CrossHook keeps the suggested prefix path editable so you can override it before running the installer.'
        : 'As you type the profile name, CrossHook will resolve a default prefix under your local data directory.';
  }
}
