import type { DiagnosticReport } from '../../../types/diagnostics';
import type { InjectionLogEvent } from '../../../types/injection';
import type { LaunchPreview, LaunchRequest, LaunchResult, LaunchValidationIssue } from '../../../types/launch';
import type { LaunchHistoryEntry } from '../../../types/library';
import { getActiveFixture } from '../../fixture';
import { emitMockEvent } from '../eventBus';
import { getStore } from '../store';
import { mockCommandArgumentCatalogEntries } from './system';
import type { Handler } from './types';

// ---------------------------------------------------------------------------
// Module-scope state (NOT on MockStore — avoids cross-file write conflicts)
// ---------------------------------------------------------------------------

let lastLaunchHelperLogPath = '/mock/logs/game-launch-9999001.log';
let lastTrainerHelperLogPath = '/mock/logs/trainer-launch-9999001.log';
const runningGames: Set<string> = new Set();
const runningProfiles: Set<string> = new Set();
const pendingLaunchTimers = new Set<number>();

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

function escapeSteamToken(value: string): string {
  const needsQuotes =
    value.length === 0 ||
    [...value].some((ch) => {
      if (/\s/.test(ch)) return true;
      return (
        ch === '$' ||
        ch === ';' ||
        ch === '"' ||
        ch === "'" ||
        ch === '\\' ||
        ch === '`' ||
        ch === '\n' ||
        ch === '\r' ||
        ch === '|' ||
        ch === '&' ||
        ch === '<' ||
        ch === '>'
      );
    });

  if (!needsQuotes) {
    return value;
  }

  let out = '"';
  for (const ch of value) {
    switch (ch) {
      case '\\':
        out += '\\\\';
        break;
      case '"':
        out += '\\"';
        break;
      case '$':
        out += '\\$';
        break;
      case '`':
        out += '\\`';
        break;
      case '\n':
        out += '\\n';
        break;
      case '\r':
        out += '\\r';
        break;
      default:
        out += ch;
    }
  }
  out += '"';
  return out;
}

function readStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((entry): entry is string => typeof entry === 'string');
}

function resolveMockCommandArgumentTokens(
  enabledArgumentIds: readonly string[],
  customArgs: readonly string[],
  resolvedMethod: 'proton_run' | 'steam_applaunch'
): string[] {
  if (enabledArgumentIds.length === 0 && customArgs.length === 0) {
    return [];
  }

  const selectedIds = new Set(enabledArgumentIds);
  const tokens: string[] = [];

  for (const entry of mockCommandArgumentCatalogEntries) {
    if (!selectedIds.has(entry.id)) {
      continue;
    }
    if (!entry.applicable_methods.includes(resolvedMethod)) {
      continue;
    }
    tokens.push(...entry.tokens);
  }

  tokens.push(...customArgs);
  return tokens;
}

function appendSteamCommandArguments(command: string, tokens: readonly string[]): string {
  if (tokens.length === 0) {
    return command;
  }
  let out = command;
  for (const token of tokens) {
    out += ` ${escapeSteamToken(token)}`;
  }
  return out;
}

function buildMockSteamLaunchOptionsPrefix(
  enabledOptionIds: readonly string[],
  customEnvVars: Readonly<Record<string, string>>
): string {
  const parts: string[] = [];
  for (const [key, value] of Object.entries(customEnvVars)) {
    const trimmedKey = key.trim();
    if (trimmedKey.length === 0) {
      continue;
    }
    parts.push(`${trimmedKey}=${escapeSteamToken(value)}`);
  }
  if (enabledOptionIds.includes('esync')) parts.push('WINEESYNC=1');
  if (enabledOptionIds.includes('fsync')) parts.push('WINEFSYNC=1');
  if (enabledOptionIds.includes('proton_log')) parts.push('PROTON_LOG=1');
  parts.push('%command%');
  return parts.join(' ');
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

function scheduleLaunchTimeout(callback: () => void, delayMs: number): void {
  const id = window.setTimeout(() => {
    pendingLaunchTimers.delete(id);
    callback();
  }, delayMs);
  pendingLaunchTimers.add(id);
}

// ---------------------------------------------------------------------------
// Event scheduling after launch_game / launch_trainer
// ---------------------------------------------------------------------------

function scheduleLaunchLogSequence(
  logLines: string[],
  helperLogPath: string,
  delayBetweenMs: number,
  afterLogsDelayMs: number,
  profileName: string | undefined
): void {
  logLines.forEach((line, index) => {
    scheduleLaunchTimeout(
      () => {
        emitMockEvent('launch-log', line);
      },
      delayBetweenMs * (index + 1)
    );
  });

  const diagnosticDelay = delayBetweenMs * (logLines.length + 1) + afterLogsDelayMs;
  scheduleLaunchTimeout(() => {
    const report = makeDiagnosticReport(helperLogPath);
    emitMockEvent('launch-diagnostic', report);
  }, diagnosticDelay);

  const completeDelay = diagnosticDelay + 200;
  scheduleLaunchTimeout(() => {
    const trimmed = profileName?.trim();
    if (trimmed) {
      runningProfiles.delete(trimmed);
    }
    emitMockEvent('launch-complete', { code: 0, signal: null });
  }, completeDelay);
}

function makeInjectionLogEvent(
  request: LaunchRequest,
  sessionId: string,
  level: InjectionLogEvent['level'],
  source: InjectionLogEvent['source'],
  message: string,
  unsupportedRuntime = false
): InjectionLogEvent {
  const profileName = request.profile_name?.trim() || 'Unknown profile';
  return {
    timestamp: new Date().toISOString(),
    profile_name: profileName,
    session_id: sessionId,
    session_kind: 'trainer',
    level,
    source,
    message,
    unsupported_runtime: unsupportedRuntime,
  };
}

function scheduleTrainerInjectionLogSequence(request: LaunchRequest, steamAppId: string): void {
  const sessionId = `mock-trainer-session-${steamAppId}`;
  const rows: Array<{
    delayMs: number;
    level: InjectionLogEvent['level'];
    source: InjectionLogEvent['source'];
    message: string;
    unsupportedRuntime?: boolean;
  }> = [
    {
      delayMs: 50,
      level: 'info',
      source: 'trainer',
      message: 'Trainer launch requested.',
    },
    {
      delayMs: 350,
      level: 'info',
      source: 'trainer',
      message: 'Trainer process started.',
    },
    {
      delayMs: 550,
      level: 'warning',
      source: 'injection',
      message: 'DLL injection engine is not available; stored hook configuration was not applied.',
      unsupportedRuntime: true,
    },
    {
      delayMs: 1500,
      level: 'info',
      source: 'trainer',
      message: 'Trainer launch completed.',
    },
  ];

  rows.forEach((row) => {
    scheduleLaunchTimeout(() => {
      emitMockEvent(
        'injection-log',
        makeInjectionLogEvent(request, sessionId, row.level, row.source, row.message, row.unsupportedRuntime ?? false)
      );
    }, row.delayMs);
  });
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

export function registerLaunch(map: Map<string, Handler>): void {
  map.set('launch_platform_status', async () => ({
    isFlatpak: false,
    unshareNetAvailable: true,
  }));

  map.set('list_launch_history_for_profile', async (args): Promise<LaunchHistoryEntry[]> => {
    const fixture = getActiveFixture();
    if (fixture === 'error') throw forcedError('list_launch_history_for_profile');
    if (fixture === 'loading') return neverResolving<LaunchHistoryEntry[]>();
    if (fixture === 'empty') return [];
    const { profileName } = args as { profileName?: string; limit?: number };
    const trimmed = profileName?.trim() ?? '';
    if (trimmed.length === 0) {
      throw new Error('[dev-mock] list_launch_history_for_profile: profileName is required');
    }
    const suffix = trimmed.replace(/\s+/g, '-');
    return [
      {
        operation_id: `mock-launch-op-${suffix}`,
        launch_method: 'proton_run',
        status: 'succeeded',
        started_at: '2026-01-15T12:00:00.000Z',
        finished_at: '2026-01-15T12:00:20.000Z',
        exit_code: 0,
        signal: null,
        severity: 'info',
        failure_mode: 'clean_exit',
      },
    ];
  });

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

    scheduleLaunchLogSequence(gameLogLines, helperLogPath, 150, 300, request.profile_name);

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

    scheduleTrainerInjectionLogSequence(request, steamAppId);
    scheduleLaunchLogSequence(trainerLogLines, helperLogPath, 200, 250, request.profile_name);

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
              proton_executable: '/home/devuser/.steam/steam/compatibilitytools.d/proton-ge/proton',
              umu_run_path: null,
            },
        working_directory: '/home/devuser/Games/TestGameAlpha',
        game_executable: '',
        game_executable_name: '',
        trainer: null,
        generated_at: new Date().toISOString(),
        display_text: 'Mock preview with validation issues for pipeline Tier 2.',
        umu_decision: null,
      };
      return previewWithIssues;
    }

    const mockProtonExe = '/home/devuser/.steam/steam/compatibilitytools.d/proton-ge/proton';
    const mockGameExe = '/home/devuser/Games/TestGameAlpha/game.exe';
    const mockUmuRun = '/usr/bin/umu-run';
    const mockUsesUmu = method === 'proton_run';

    const commandArgumentTokens = request.launch_trainer_only
      ? []
      : resolveMockCommandArgumentTokens(
          request.command_arguments?.enabled_argument_ids ?? [],
          request.command_arguments?.custom_args ?? [],
          method === 'steam_applaunch' ? 'steam_applaunch' : 'proton_run'
        );

    const protonEffectiveCommand = mockUsesUmu
      ? `gamescope mangohud -- ${mockUmuRun} ${mockGameExe}${commandArgumentTokens.length > 0 ? ` ${commandArgumentTokens.join(' ')}` : ''}`
      : `${mockProtonExe} run ${mockGameExe}${commandArgumentTokens.length > 0 ? ` ${commandArgumentTokens.join(' ')}` : ''}`;

    const steamLaunchOptionsBase =
      method === 'steam_applaunch'
        ? buildMockSteamLaunchOptionsPrefix(
            request.optimizations?.enabled_option_ids ?? [],
            request.custom_env_vars ?? {}
          )
        : null;
    const steamLaunchOptions =
      steamLaunchOptionsBase === null
        ? null
        : appendSteamCommandArguments(steamLaunchOptionsBase, commandArgumentTokens);

    // Build populated preview (shared base for both populated and warning paths)
    const preview: LaunchPreview = {
      resolved_method: method,
      validation: { issues: [] },
      environment: makePreviewEnvVars(),
      cleared_variables: ['LD_PRELOAD'],
      wrappers: ['gamescope', 'mangohud'],
      effective_command:
        method === 'steam_applaunch' ? steamLaunchOptions : method === 'native' ? mockGameExe : protonEffectiveCommand,
      directives_error: null,
      steam_launch_options: steamLaunchOptions,
      proton_setup:
        method !== 'native'
          ? {
              wine_prefix_path: '/home/devuser/.local/share/mock-prefix',
              compat_data_path: '/home/devuser/.steam/steam/steamapps/compatdata/9999001',
              steam_client_install_path: getStore().defaultSteamClientInstallPath,
              proton_executable: mockProtonExe,
              umu_run_path: mockUsesUmu ? mockUmuRun : null,
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
      display_text: 'Mock preview: game will be launched via Proton with PROTON_LOG=1 and esync enabled.',
      umu_decision:
        method === 'proton_run'
          ? (() => {
              // Keep the csv_coverage heuristic aligned with the `check_umu_coverage`
              // mock so Preview and the Runner-dropdown badge agree in dev mode.
              const mockFoundAppIds = new Set(['546590', '2050650']);
              const appId = (
                request?.runtime?.umu_game_id ??
                request?.steam?.app_id ??
                request?.runtime?.steam_app_id ??
                ''
              ).trim();
              const lookupEnabled = getStore().settings.umu_database_lookup === 'enabled';
              const lookupStore = request?.runtime?.umu_store?.trim() ?? '';
              const lookupCodename = request?.runtime?.umu_codename?.trim() ?? '';
              const hasLookupHints = lookupStore.length > 0 && lookupCodename.length > 0;
              const resolvedGameId = appId || (lookupEnabled && hasLookupHints ? 'UMU-MOCK-LOOKUP' : 'umu-0');
              const resolutionSource = appId
                ? 'steam_app_id'
                : lookupEnabled
                  ? hasLookupHints
                    ? 'fresh_lookup'
                    : 'missing_hints'
                  : 'lookup_disabled';
              const csvCoverage: 'found' | 'missing' | 'unknown' =
                appId === '' ? 'unknown' : mockFoundAppIds.has(appId) ? 'found' : 'missing';
              return {
                requested_preference: 'umu',
                umu_run_path_on_backend_path: '/usr/bin/umu-run',
                will_use_umu: true,
                reason: 'using umu-run at /usr/bin/umu-run',
                csv_coverage: csvCoverage,
                gameid_resolution: {
                  game_id: resolvedGameId,
                  store: lookupStore || null,
                  source: resolutionSource,
                  lookup_key:
                    lookupStore && lookupCodename
                      ? {
                          store: lookupStore,
                          codename: lookupCodename,
                        }
                      : null,
                  fetched_at: null,
                  expires_at: null,
                  error_category: null,
                },
              };
            })()
          : null,
    };

    if (gamePath === '__MOCK_VALIDATION_WARNING__') {
      return {
        ...preview,
        validation: {
          issues: [
            {
              message: 'Trainer binary hash does not match the community checksum.',
              help: 'Re-download or verify the trainer file integrity.',
              severity: 'warning' as const,
              code: 'trainer_hash_mismatch',
            },
          ],
        },
        wrappers: ['gamescope'],
        display_text: 'Mock preview with warning-severity validation.',
      };
    }

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
  // list_running_profiles — returns profile names marked running in dev tests
  // -------------------------------------------------------------------------
  map.set('list_running_profiles', async (): Promise<string[]> => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return [];
    if (fixture === 'loading') return neverResolving<string[]>();
    // `error` is allowed to resolve here — this is a polled status read and
    // throwing on every poll would flood the console without meaningful UX.
    return [...runningProfiles].sort();
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

    const raw = args as Record<string, unknown>;
    const enabled_option_ids = readStringArray(raw.enabled_option_ids ?? raw.enabledOptionIds);
    const custom_env_vars = (raw.custom_env_vars ?? raw.customEnvVars ?? {}) as Record<string, string>;
    const enabled_argument_ids = readStringArray(raw.enabled_argument_ids ?? raw.enabledArgumentIds);
    const custom_command_args = readStringArray(raw.custom_command_args ?? raw.customCommandArgs);

    const base = buildMockSteamLaunchOptionsPrefix(enabled_option_ids, custom_env_vars);
    const argumentTokens = resolveMockCommandArgumentTokens(
      enabled_argument_ids,
      custom_command_args,
      'steam_applaunch'
    );
    return appendSteamCommandArguments(base, argumentTokens);
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

  map.set('_mock_set_profile_running', async (args): Promise<null> => {
    const { profileName, running } = args as { profileName: string; running: boolean };
    if (running) {
      runningProfiles.add(profileName.trim());
    } else {
      runningProfiles.delete(profileName.trim());
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

export function resetLaunchMockState(): void {
  lastLaunchHelperLogPath = '/mock/logs/game-launch-9999001.log';
  lastTrainerHelperLogPath = '/mock/logs/trainer-launch-9999001.log';
  runningGames.clear();
  runningProfiles.clear();
  for (const timerId of pendingLaunchTimers) {
    window.clearTimeout(timerId);
  }
  pendingLaunchTimers.clear();
}
