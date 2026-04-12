import type { InstallGameRequest, InstallGameResult } from '../../../types/install';
import { createDefaultProfile } from '../../../types/profile';
import { emitMockEvent } from '../eventBus';
import type { Handler } from './types';

/** Tracks the profile name of the currently in-flight install, if any. */
let installInFlight: string | null = null;

function buildMockInstallResult(request: InstallGameRequest): InstallGameResult {
  const base = createDefaultProfile();

  const profile = {
    ...base,
    game: {
      ...base.game,
      name: request.profile_name || 'Mock Installed Game',
      executable_path: request.installed_game_executable_path || '/mock/games/mock-game/game.exe',
      custom_cover_art_path: request.custom_cover_art_path || '',
      custom_portrait_art_path: request.custom_portrait_art_path || '',
      custom_background_art_path: request.custom_background_art_path || '',
    },
    trainer: {
      ...base.trainer,
      path: request.trainer_path || '/mock/trainers/mock-trainer.exe',
    },
    steam: {
      ...base.steam,
      app_id: request.steam_app_id || '9999001',
      proton_path: request.proton_path || '/home/devuser/.steam/steam/compatibilitytools.d/mock-proton/proton',
      launcher: {
        icon_path: request.launcher_icon_path || '',
        display_name: request.display_name || request.profile_name || 'Mock Game',
      },
    },
    runtime: {
      ...base.runtime,
      prefix_path: request.prefix_path || '/home/devuser/.steam/steam/steamapps/compatdata/9999001/pfx',
      proton_path: request.proton_path || '/home/devuser/.steam/steam/compatibilitytools.d/mock-proton/proton',
      working_directory: request.working_directory || '/mock/games/mock-game',
    },
    launch: {
      ...base.launch,
      method: (request.runner_method as '' | 'proton_run' | 'steam_applaunch' | 'native') || 'proton_run',
    },
  };

  const candidates = ['/mock/games/mock-game/game.exe', '/mock/games/mock-game/launcher.exe'];

  return {
    succeeded: true,
    message: '[dev-mock] Install completed successfully.',
    helper_log_path: `/mock/logs/${request.profile_name || 'mock'}-install.log`,
    profile_name: request.profile_name || 'mock-game',
    needs_executable_confirmation: true,
    discovered_game_executable_candidates: candidates,
    profile,
  };
}

function scheduleInstallEvents(profileName: string): void {
  const started = { profileName, phase: 'started', progress: 0, message: 'Starting installer…' };
  emitMockEvent('install-started', started);

  window.setTimeout(() => {
    emitMockEvent('install-progress', {
      profileName,
      phase: 'running_installer',
      progress: 10,
      message: 'Extracting installer resources…',
    });
  }, 200);

  window.setTimeout(() => {
    emitMockEvent('install-progress', {
      profileName,
      phase: 'running_installer',
      progress: 30,
      message: 'Running installer under Proton…',
    });
  }, 500);

  window.setTimeout(() => {
    emitMockEvent('install-progress', {
      profileName,
      phase: 'running_installer',
      progress: 60,
      message: 'Writing game files…',
    });
  }, 900);

  window.setTimeout(() => {
    emitMockEvent('install-progress', {
      profileName,
      phase: 'running_installer',
      progress: 90,
      message: 'Finalising installation…',
    });
  }, 1300);
}

export function registerInstall(map: Map<string, Handler>): void {
  map.set('install_default_prefix_path', async (args) => {
    const { profileName } = args as { profileName: string };
    const trimmed = (profileName ?? '').trim();
    if (!trimmed) {
      throw new Error('[dev-mock] install_default_prefix_path: profile_name is required');
    }
    const slug = trimmed.toLowerCase().replace(/[^a-z0-9_-]/g, '_');
    return `/home/devuser/.local/share/crosshook/prefixes/${slug}`;
  });

  map.set('validate_install_request', async (args) => {
    const { request } = args as { request: InstallGameRequest };
    if (!request.profile_name?.trim()) {
      throw new Error('[dev-mock] ProfileNameRequired');
    }
    if (!request.installer_path?.trim()) {
      throw new Error('[dev-mock] InstallerPathRequired');
    }
    if (!request.proton_path?.trim()) {
      throw new Error('[dev-mock] ProtonPathRequired');
    }
    if (!request.prefix_path?.trim()) {
      throw new Error('[dev-mock] PrefixPathRequired');
    }
    return null;
  });

  map.set('install_game', async (args) => {
    const { request } = args as { request: InstallGameRequest };
    const profileName = request.profile_name?.trim() || 'mock-game';

    if (installInFlight !== null) {
      throw new Error(`[dev-mock] install_game: install already in progress for "${installInFlight}"`);
    }

    const runToken = profileName;
    installInFlight = runToken;

    try {
      scheduleInstallEvents(profileName);

      // Simulate install duration (~1.5 s total)
      await new Promise<void>((resolve) => window.setTimeout(resolve, 1500));

      const result = buildMockInstallResult(request);

      emitMockEvent('install-complete', {
        profileName,
        phase: 'complete',
        progress: 100,
        message: result.message,
        result,
      });

      return result;
    } finally {
      if (installInFlight === runToken) {
        installInFlight = null;
      }
    }
  });
}
