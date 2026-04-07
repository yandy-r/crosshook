import {
  INSTALL_GAME_VALIDATION_FIELD,
  INSTALL_GAME_VALIDATION_MESSAGES,
  type InstallGameRequest,
  type InstallGameValidationError,
} from '../../types/install';

export function mapValidationErrorToField(message: string): keyof InstallGameRequest | null {
  const variants = Object.keys(INSTALL_GAME_VALIDATION_MESSAGES) as InstallGameValidationError[];
  for (const variant of variants) {
    if (message === INSTALL_GAME_VALIDATION_MESSAGES[variant]) {
      return INSTALL_GAME_VALIDATION_FIELD[variant];
    }
  }

  const normalized = message.toLowerCase();

  if (
    normalized.includes('profile name') ||
    normalized.includes('invalid profile name') ||
    normalized.includes('invalid characters')
  ) {
    return 'profile_name';
  }

  if (
    normalized.includes('installer path') ||
    normalized.includes('windows .exe') ||
    normalized.includes('installer media')
  ) {
    return 'installer_path';
  }

  if (normalized.includes('trainer path')) {
    return 'trainer_path';
  }

  if (normalized.includes('custom cover art path')) {
    return 'custom_cover_art_path';
  }

  if (normalized.includes('custom portrait art path')) {
    return 'custom_portrait_art_path';
  }

  if (normalized.includes('custom background art path')) {
    return 'custom_background_art_path';
  }

  if (normalized.includes('proton path')) {
    return 'proton_path';
  }

  if (normalized.includes('prefix path')) {
    return 'prefix_path';
  }

  if (normalized.includes('final game executable path')) {
    return 'installed_game_executable_path';
  }

  return null;
}
