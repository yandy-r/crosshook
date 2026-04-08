import type { Handler } from '../index';
import type {
  LauncherDeleteResult,
  LauncherInfo,
  LauncherRenameResult,
} from '../../../types/launcher';

// ---- Synthetic data (BR-10 / W-3: fictional names, /home/devuser paths) ----

const SYNTHETIC_LAUNCHERS: LauncherInfo[] = [
  {
    display_name: 'Test Game Alpha - Trainer',
    launcher_slug: 'test-game-alpha-trainer',
    script_path: '/home/devuser/.local/share/crosshook/launchers/test-game-alpha-trainer.sh',
    desktop_entry_path:
      '/home/devuser/.local/share/applications/test-game-alpha-trainer.desktop',
    script_exists: true,
    desktop_entry_exists: true,
    is_stale: false,
  },
];

const SYNTHETIC_EXPORT_RESULT = {
  display_name: 'Test Game Alpha - Trainer',
  launcher_slug: 'test-game-alpha-trainer',
  script_path: '/home/devuser/.local/share/crosshook/launchers/test-game-alpha-trainer.sh',
  desktop_entry_path:
    '/home/devuser/.local/share/applications/test-game-alpha-trainer.desktop',
};

const SYNTHETIC_DELETE_RESULT: LauncherDeleteResult = {
  script_deleted: false,
  desktop_entry_deleted: false,
  script_path: '/home/devuser/.local/share/crosshook/launchers/test-game-alpha-trainer.sh',
  desktop_entry_path:
    '/home/devuser/.local/share/applications/test-game-alpha-trainer.desktop',
  script_skipped_reason: null,
  desktop_entry_skipped_reason: null,
};

const SYNTHETIC_LAUNCHER_STATUS: LauncherInfo = {
  display_name: 'Test Game Alpha - Trainer',
  launcher_slug: 'test-game-alpha-trainer',
  script_path: '/home/devuser/.local/share/crosshook/launchers/test-game-alpha-trainer.sh',
  desktop_entry_path:
    '/home/devuser/.local/share/applications/test-game-alpha-trainer.desktop',
  script_exists: true,
  desktop_entry_exists: true,
  is_stale: false,
};

const MOCK_SCRIPT_CONTENT = `#!/usr/bin/env bash
# [dev-mock] Synthetic launcher script — not written to disk
set -euo pipefail
echo "[dev-mock] Launching Test Game Alpha trainer..."
`;

const MOCK_DESKTOP_CONTENT = `[Desktop Entry]
# [dev-mock] Synthetic desktop entry — not written to disk
Type=Application
Name=Test Game Alpha - Trainer
Exec=/home/devuser/.local/share/crosshook/launchers/test-game-alpha-trainer.sh
Icon=
Categories=Game;
`;

// ---- Handler registration ----

export function registerLauncher(map: Map<string, Handler>): void {
  map.set('list_launchers', async (_args): Promise<LauncherInfo[]> => {
    return structuredClone(SYNTHETIC_LAUNCHERS);
  });

  map.set('check_launcher_exists', async (_args): Promise<LauncherInfo> => {
    return structuredClone(SYNTHETIC_LAUNCHER_STATUS);
  });

  map.set('check_launcher_for_profile', async (_args): Promise<LauncherInfo> => {
    return structuredClone(SYNTHETIC_LAUNCHER_STATUS);
  });

  map.set('validate_launcher_export', async (_args): Promise<void> => {
    // No-op: validation passes in dev-mock mode
  });

  map.set('export_launchers', async (_args) => {
    console.warn('[dev-mock] export_launchers suppressed — no files written to disk');
    return structuredClone(SYNTHETIC_EXPORT_RESULT);
  });

  map.set('preview_launcher_script', async (_args): Promise<string> => {
    return MOCK_SCRIPT_CONTENT;
  });

  map.set('preview_launcher_desktop', async (_args): Promise<string> => {
    return MOCK_DESKTOP_CONTENT;
  });

  map.set('delete_launcher', async (_args): Promise<LauncherDeleteResult> => {
    console.warn('[dev-mock] delete_launcher suppressed — no files deleted');
    return structuredClone(SYNTHETIC_DELETE_RESULT);
  });

  map.set('delete_launcher_by_slug', async (_args): Promise<LauncherDeleteResult> => {
    console.warn('[dev-mock] delete_launcher_by_slug suppressed — no files deleted');
    return structuredClone(SYNTHETIC_DELETE_RESULT);
  });

  map.set('rename_launcher', async (_args): Promise<LauncherRenameResult> => {
    console.warn('[dev-mock] rename_launcher suppressed — no files renamed');
    const result: LauncherRenameResult = {
      old_slug: 'test-game-alpha-trainer',
      new_slug: 'test-game-alpha-trainer',
      new_script_path:
        '/home/devuser/.local/share/crosshook/launchers/test-game-alpha-trainer.sh',
      new_desktop_entry_path:
        '/home/devuser/.local/share/applications/test-game-alpha-trainer.desktop',
      script_renamed: false,
      desktop_entry_renamed: false,
      old_script_cleanup_warning: null,
      old_desktop_entry_cleanup_warning: null,
    };
    return result;
  });

  map.set('reexport_launcher_by_slug', async (_args) => {
    console.warn('[dev-mock] reexport_launcher_by_slug suppressed — no files written to disk');
    return structuredClone(SYNTHETIC_EXPORT_RESULT);
  });

  map.set('find_orphaned_launchers', async (_args): Promise<LauncherInfo[]> => {
    return [];
  });
}
