import type { GameProfile } from '../../types/profile';

export function parentDirectory(path: string): string {
  const normalized = path.trim().replace(/\\/g, '/');
  const separatorIndex = normalized.lastIndexOf('/');

  if (separatorIndex <= 0) {
    return '';
  }

  return normalized.slice(0, separatorIndex);
}

export function updateGameExecutablePath(current: GameProfile, nextExecutablePath: string): GameProfile {
  const previousExecutableParent = parentDirectory(current.game.executable_path);
  const currentWorkingDirectory = current.runtime.working_directory.trim();
  const shouldDeriveWorkingDirectory =
    currentWorkingDirectory.length === 0 || currentWorkingDirectory === previousExecutableParent;

  return {
    ...current,
    game: {
      ...current.game,
      executable_path: nextExecutablePath,
    },
    runtime: {
      ...current.runtime,
      working_directory: shouldDeriveWorkingDirectory
        ? parentDirectory(nextExecutablePath)
        : current.runtime.working_directory,
    },
  };
}
