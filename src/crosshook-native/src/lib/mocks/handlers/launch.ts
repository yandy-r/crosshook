import type { Handler } from './types';
import { getActiveFixture } from '../../fixture';
import { getStore } from '../store';
import { emitMockEvent } from '../eventBus';
import type { DiagnosticReport } from '../../../types/diagnostics';
import type {
  LaunchRequest,
  LaunchResult,
  LaunchPreview,
  LaunchValidationIssue,
} from '../../../types/launch';
import type { HashVerifyResult, OfflineReadinessReport } from '../../../types/offline';

// ---------------------------------------------------------------------------
// Module-scope state (NOT on MockStore — avoids cross-file write conflicts)
// ---------------------------------------------------------------------------

let lastLaunchHelperLogPath = '/mock/logs/game-launch-9999001.log';
let lastTrainerHelperLogPath = '/mock/logs/trainer-launch-9999001.log';
let runningGames: Set<string> = new Set();

// ---------------------------------------------------------------------------
// Fixture helpers (BR-11)
// ---------------------------------------------------------------------------
//
// Launch commands are NOT shell-critical, so they follow the standard
// fixture-dispatch pattern:
//   populated — current behavior
//   empty     — read commands return empty/false; mutating launch commands
//                fall through to populated (no meaningful "empty" mutation)
//   error     — fallible commands throw `[dev-mock] forced error for <name>`
//   loading   — non-shell-critical commands return a never-resolving promise
//
// NOTE: Task 3.2's `wrapHandler()` middleware (`lib/mocks/wrapHandler.ts`) is
// orthogonal to these helpers — it implements `?errors=true` / `?delay=<ms>`
// toggles, while these implement `?fixture=loading|error`. Both systems are
// applied to every handler; do NOT remove these helpers.

/**
 * Returns a promise that never resolves. Used by the `loading` fixture so
 * loading-state UIs stay visible. Orthogonal to `?delay=<ms>` in `wrapHandler.ts`.
 */
function neverResolving<T>(): Promise<T> {
  return new Promise<T>(() => {
    /* intentionally never resolves */
  });
}

/**
 * Synthesizes a `[dev-mock] forced error` for the named command. Used by the
 * `?fixture=error` dispatch path. Orthogonal to `?errors=true` in `wrapHandler.ts`.
 */
function forcedError(commandName: string): Error {
  return new Error(`[dev-mock] forced error for ${commandName}`);
}

// ---------------------------------------------------------------------------
// Synthetic data helpers
// ---------------------------------------------------------------------------

function makeLaunchResult(message: string, logSuffix: string): LaunchResult {
  return {
    succeeded: true,
    message,
    helper_log_path: `/mock/logs/${logSuffix}.log`,
    warnings: [],
  };
}

function makeDiagnosticReport(logPath: string): DiagnosticReport {
  return {
    severity: 'info',
    summary: 'Mock launch completed with no errors detected.',
    exit_info: {
      code: 0,
      signal: null,
      signal_name: null,
      core_dumped: false,
      failure_mode: 'clean_exit',
      description: 'Process exited cleanly (code 0).',
      severity: 'info',
    },
    pattern_matches: [],
    suggestions: [],
    launch_method: 'proton_run',
    log_tail_path: logPath,
    analyzed_at: new Date().toISOString(),
  };
}

function makePreviewEnvVars(): LaunchPreview['environment'] {
  return [
    { key: 'WINEPREFIX', value: '/home/devuser/.local/share/mock-prefix', source: 'proton_runtime' },
    { key: 'PROTON_LOG', value: '1', source: 'launch_optimization' },
    { key: 'MOCK_ENV', value: 'devuser', source: 'host' },
  ];
}

// ---------------------------------------------------------------------------
// Event scheduling after launch_game / launch_trainer
// ---------------------------------------------------------------------------

function scheduleLaunchLogSequence(
  logLines: string[],
  helperLogPath: string,
  delayBetweenMs: number,
  afterLogsDelayMs: number
): void {
  logLines.forEach((line, index) => {
    setTimeout(() => {
      emitMockEvent('launch-log', line);
    }, delayBetweenMs * (index + 1));
  });

  const diagnosticDelay = delayBetweenMs * (logLines.length + 1) + afterLogsDelayMs;
  setTimeout(() => {
    const report = makeDiagnosticReport(helperLogPath);
    emitMockEvent('launch-diagnostic', report);
  }, diagnosticDelay);

  const completeDelay = diagnosticDelay + 200;
  setTimeout(() => {
    emitMockEvent('launch-complete', { code: 0, signal: null });
  }, completeDelay);
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

export function registerLaunch(map: Map<string, Handler>): void {
  // -------------------------------------------------------------------------
  // launch_game — returns LaunchResult immediately then emits event sequence
  // -------------------------------------------------------------------------
  map.set('launch_game', async (args): Promise<LaunchResult> => {
    const fixture = getActiveFixture();
    if (fixture === 'error') throw forcedError('launch_game');
    if (fixture === 'loading') return neverResolving<LaunchResult>();
    const { request } = args as { request: LaunchRequest };
    const steamAppId = request.steam?.app_id ?? '9999001';
    const logSuffix = `game-launch-${steamAppId}`;
    const helperLogPath = `/mock/logs/${logSuffix}.log`;
    lastLaunchHelperLogPath = helperLogPath;

    const gameLogLines = [
      '[mock] Preparing launch environment...',
      '[mock] Loading Wine prefix at /home/devuser/.local/share/mock-prefix',
      '[mock] Initializing Proton runtime...',
      '[mock] Applying launch optimizations (esync, fsync)...',
      '[mock] Starting game process: /home/devuser/Games/TestGameAlpha/game.exe',
      '[mock] Game process started successfully. Waiting for window...',
      '[mock] Game window detected. Launch sequence complete.',
    ];

    scheduleLaunchLogSequence(gameLogLines, helperLogPath, 150, 300);

    return makeLaunchResult('Game launch started.', logSuffix);
  });

  // -------------------------------------------------------------------------
  // launch_trainer — returns LaunchResult immediately then emits event sequence
  // -------------------------------------------------------------------------
  map.set('launch_trainer', async (args): Promise<LaunchResult> => {
    const fixture = getActiveFixture();
    if (fixture === 'error') throw forcedError('launch_trainer');
    if (fixture === 'loading') return neverResolving<LaunchResult>();
    const { request } = args as { request: LaunchRequest };
    const steamAppId = request.steam?.app_id ?? '9999001';
    const logSuffix = `trainer-launch-${steamAppId}`;
    const helperLogPath = `/mock/logs/${logSuffix}.log`;
    lastTrainerHelperLogPath = helperLogPath;

    const trainerLogLines = [
      '[mock] Preparing trainer launch environment...',
      '[mock] Locating trainer binary: /home/devuser/Trainers/mock-trainer.exe',
      '[mock] Injecting trainer into Proton prefix...',
      '[mock] Trainer process started. Waiting for game to become responsive...',
      '[mock] Trainer attached successfully. Cheat engine active.',
    ];

    scheduleLaunchLogSequence(trainerLogLines, helperLogPath, 200, 250);

    return makeLaunchResult('Trainer launch started.', logSuffix);
  });

  // -------------------------------------------------------------------------
  // validate_launch — returns void (null) on success, throws LaunchValidationIssue on failure
  // -------------------------------------------------------------------------
  map.set('validate_launch', async (args): Promise<null> => {
    const fixture = getActiveFixture();
    if (fixture === 'error') throw forcedError('validate_launch');
    if (fixture === 'loading') return neverResolving<null>();
    if (fixture === 'empty') return null;
    const { request } = args as { request: LaunchRequest };
    const gamePath = request?.game_path?.trim() ?? '';
    if (!gamePath) {
      const issue: LaunchValidationIssue = {
        message: 'Game path is required.',
        help: 'Specify a valid game executable path before launching.',
        severity: 'fatal',
        code: 'missing_game_path',
      };
      throw issue;
    }
    return null;
  });

  // -------------------------------------------------------------------------
  // preview_launch — returns a synthetic LaunchPreview
  // -------------------------------------------------------------------------
  map.set('preview_launch', async (args): Promise<LaunchPreview> => {
    const fixture = getActiveFixture();
    if (fixture === 'error') throw forcedError('preview_launch');
    if (fixture === 'loading') return neverResolving<LaunchPreview>();
    // `empty` falls through to the populated path because LaunchPreview is a
    // non-nullable structural payload with no meaningful empty representation.
    const { request } = args as { request: LaunchRequest };
    const method = (request?.method ?? 'proton_run') as LaunchPreview['resolved_method'];
    const gamePath = request?.game_path?.trim() ?? '';

    if (gamePath === '' || gamePath === '__MOCK_VALIDATION_ERROR__') {
      const isNative = method === 'native';
      const issues: LaunchPreview['validation']['issues'] = [
        {
          message: 'A game executable path is required.',
          help: 'Set a game executable path in the profile.',
          severity: 'fatal' as const,
          code: 'game_path_required',
        },
      ];
      if (!isNative) {
        issues.push({
          message: 'The runtime prefix path does not exist.',
          help: 'Check that the Wine prefix directory exists.',
          severity: 'fatal' as const,
          code: 'runtime_prefix_path_missing',
        });
      }
      const previewWithIssues: LaunchPreview = {
        resolved_method: method,
        validation: { issues },
        environment: null,
        cleared_variables: [],
        wrappers: null,
        effective_command: null,
        directives_error: isNative ? null : 'Mock directive resolution error',
        steam_launch_options: null,
        proton_setup: isNative
          ? null
          : {
              wine_prefix_path: '/home/devuser/.local/share/mock-prefix',
              compat_data_path: '/home/devuser/.steam/steam/steamapps/compatdata/9999001',
              steam_client_install_path: getStore().defaultSteamClientInstallPath,
              proton_executable:
                '/home/devuser/.steam/steam/compatibilitytools.d/proton-ge/proton',
              umu_run_path: null,
            },
        working_directory: '/home/devuser/Games/TestGameAlpha',
        game_executable: '',
        game_executable_name: '',
        trainer: null,
        generated_at: new Date().toISOString(),
        display_text: 'Mock preview with validation issues for pipeline Tier 2.',
      };
      return previewWithIssues;
    }

    const preview: LaunchPreview = {
      resolved_method: method,
      validation: { issues: [] },
      environment: makePreviewEnvVars(),
      cleared_variables: ['LD_PRELOAD'],
      wrappers: ['gamescope', 'mangohud'],
      effective_command:
        '/home/devuser/.steam/steam/compatibilitytools.d/proton-ge/proton run /home/devuser/Games/TestGameAlpha/game.exe',
      directives_error: null,
      steam_launch_options: method === 'steam_applaunch' ? 'PROTON_LOG=1 %command%' : null,
      proton_setup:
        method !== 'native'
          ? {
              wine_prefix_path: '/home/devuser/.local/share/mock-prefix',
              compat_data_path: '/home/devuser/.steam/steam/steamapps/compatdata/9999001',
              steam_client_install_path: getStore().defaultSteamClientInstallPath,
              proton_executable:
                '/home/devuser/.steam/steam/compatibilitytools.d/proton-ge/proton',
              umu_run_path: null,
            }
          : null,
      working_directory: '/home/devuser/Games/TestGameAlpha',
      game_executable: '/home/devuser/Games/TestGameAlpha/game.exe',
      game_executable_name: 'game.exe',
      trainer:
        method !== 'native'
          ? {
              path: '/home/devuser/Trainers/mock-trainer.exe',
              host_path: '/home/devuser/Trainers/mock-trainer.exe',
              loading_mode: 'source_directory',
              staged_path: null,
            }
          : null,
      generated_at: new Date().toISOString(),
      display_text:
        'Mock preview: game will be launched via Proton with PROTON_LOG=1 and esync enabled.',
    };

    return preview;
  });

  // -------------------------------------------------------------------------
  // check_game_running — returns false in browser dev mode (no real /proc)
  // -------------------------------------------------------------------------
  map.set('check_game_running', async (args): Promise<boolean> => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return false;
    if (fixture === 'loading') return neverResolving<boolean>();
    // `error` is allowed to resolve here — this is a polled status read and
    // throwing on every poll would flood the console without meaningful UX.
    const { exeName } = args as { exeName: string };
    return runningGames.has(exeName.trim());
  });

  // -------------------------------------------------------------------------
  // check_gamescope_session — always false in browser dev mode
  // -------------------------------------------------------------------------
  map.set('check_gamescope_session', async (): Promise<boolean> => {
    const fixture = getActiveFixture();
    if (fixture === 'loading') return neverResolving<boolean>();
    // `empty` and `error` both naturally return false here — gamescope is
    // never active in browser dev mode anyway.
    return false;
  });

  // -------------------------------------------------------------------------
  // verify_trainer_hash and check_offline_readiness are owned by
  // handlers/system.ts (offline domain) — registered there, not here.
  // -------------------------------------------------------------------------

  // -------------------------------------------------------------------------
  // build_steam_launch_options_command — returns a synthetic Steam launch options string
  // -------------------------------------------------------------------------
  map.set('build_steam_launch_options_command', async (args): Promise<string> => {
    const fixture = getActiveFixture();
    if (fixture === 'error') throw forcedError('build_steam_launch_options_command');
    if (fixture === 'loading') return neverResolving<string>();
    if (fixture === 'empty') return '%command%';
    const { enabled_option_ids } = args as {
      enabled_option_ids: string[];
      custom_env_vars: Record<string, string>;
      gamescope: unknown;
    };
    const parts: string[] = [];
    if (enabled_option_ids.includes('esync')) parts.push('WINEESYNC=1');
    if (enabled_option_ids.includes('fsync')) parts.push('WINEFSYNC=1');
    if (enabled_option_ids.includes('proton_log')) parts.push('PROTON_LOG=1');
    parts.push('%command%');
    return parts.join(' ');
  });

  // -------------------------------------------------------------------------
  // Expose module-scope state accessors for test introspection
  // (these are not real Tauri commands — they are dev-only helpers)
  // -------------------------------------------------------------------------
  map.set('_mock_set_game_running', async (args): Promise<null> => {
    const { exeName, running } = args as { exeName: string; running: boolean };
    if (running) {
      runningGames.add(exeName.trim());
    } else {
      runningGames.delete(exeName.trim());
    }
    return null;
  });

  map.set('_mock_get_last_launch_log_path', async (): Promise<string> => {
    return lastLaunchHelperLogPath;
  });

  map.set('_mock_get_last_trainer_log_path', async (): Promise<string> => {
    return lastTrainerHelperLogPath;
  });
}
