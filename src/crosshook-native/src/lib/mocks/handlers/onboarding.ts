import type {
  Capability,
  HostToolCheckResult,
  HostToolDetails,
  HostToolInstallCommand,
  OnboardingCheckPayload,
  ReadinessCheckResult,
  TrainerGuidanceContent,
} from '../../../types/onboarding';
import { getActiveToggles } from '../../toggles';
import { emitMockEvent } from '../eventBus';
import { getStore } from '../store';
import type { Handler } from './types';

let onboardingDismissed = false;

// Synthesize the `onboarding-check` event ONCE per session when
// `?onboarding=show` is present in the URL. The guard prevents HMR or
// re-imports of this module from re-firing the event. The 500ms delay
// ensures App.tsx has already mounted and called `subscribeEvent()` before
// the emit fans out — without it, the event would race the subscription and
// the listener would miss the payload.
let onboardingEventSynthesized = false;

/** Prevents duplicate retry loops from module init + registerOnboarding(). */
let onboardingSynthesisScheduled = false;

const ONBOARDING_EMIT_INITIAL_MS = 500;
const ONBOARDING_EMIT_RETRY_MS = 200;
const ONBOARDING_EMIT_MAX_ATTEMPTS = 25;
const MOCK_DETECTED_DISTRO_FAMILY = 'Arch';

interface CachedHostReadinessSnapshot {
  checked_at: string;
  detected_distro_family: string;
  tool_checks: HostToolCheckResult[];
  all_passed: boolean;
  critical_failures: number;
  warnings: number;
}

interface DismissReadinessNagArgs {
  toolId: string;
}

interface ProbeHostToolDetailsArgs {
  toolId?: string;
  tool_id?: string;
}

interface MockCapabilityDefinition {
  id: string;
  label: string;
  category: string;
  requiredToolIds: string[];
  optionalToolIds: string[];
}

interface MockHostToolDefinition {
  check: HostToolCheckResult;
  details: HostToolDetails;
}

let cachedHostReadinessSnapshot: CachedHostReadinessSnapshot | null = null;

const MOCK_CAPABILITY_DEFINITIONS: readonly MockCapabilityDefinition[] = [
  {
    id: 'gamescope',
    label: 'Gamescope',
    category: 'performance',
    requiredToolIds: ['gamescope'],
    optionalToolIds: [],
  },
  {
    id: 'mangohud',
    label: 'MangoHud',
    category: 'overlay',
    requiredToolIds: ['mangohud'],
    optionalToolIds: [],
  },
  {
    id: 'gamemode',
    label: 'GameMode',
    category: 'performance',
    requiredToolIds: ['gamemode'],
    optionalToolIds: [],
  },
  {
    id: 'prefix_tools',
    label: 'Prefix tools',
    category: 'prefix_tools',
    requiredToolIds: [],
    optionalToolIds: ['winetricks', 'protontricks'],
  },
  {
    id: 'non_steam_launch',
    label: 'Non-Steam launch',
    category: 'runtime',
    requiredToolIds: ['umu_run'],
    optionalToolIds: [],
  },
];

const MOCK_HOST_TOOL_DEFINITIONS: readonly MockHostToolDefinition[] = [
  {
    check: {
      tool_id: 'umu_run',
      display_name: 'umu-launcher',
      is_available: false,
      is_required: false,
      category: 'runtime',
      docs_url: 'https://github.com/Open-Wine-Components/umu-launcher',
      tool_version: null,
      resolved_path: null,
      install_guidance: {
        distro_family: MOCK_DETECTED_DISTRO_FAMILY,
        command: 'sudo pacman -S umu-launcher',
        alternatives: 'If unavailable in mirrors, use upstream docs for source or user-level install.',
      },
    },
    details: {
      tool_id: 'umu_run',
      tool_version: null,
      resolved_path: null,
    },
  },
  {
    check: {
      tool_id: 'gamescope',
      display_name: 'Gamescope',
      is_available: false,
      is_required: false,
      category: 'performance',
      docs_url: 'https://github.com/ValveSoftware/gamescope',
      tool_version: null,
      resolved_path: null,
      install_guidance: {
        distro_family: MOCK_DETECTED_DISTRO_FAMILY,
        command: 'sudo pacman -S gamescope',
        alternatives: 'Use mesa-git or distro gaming repos if the stock package is old.',
      },
    },
    details: {
      tool_id: 'gamescope',
      tool_version: null,
      resolved_path: null,
    },
  },
  {
    check: {
      tool_id: 'mangohud',
      display_name: 'MangoHud',
      is_available: true,
      is_required: false,
      category: 'overlay',
      docs_url: 'https://github.com/flightlessmango/MangoHud',
      tool_version: '0.7.2',
      resolved_path: '/usr/bin/mangohud',
      install_guidance: null,
    },
    details: {
      tool_id: 'mangohud',
      tool_version: '0.7.2',
      resolved_path: '/usr/bin/mangohud',
    },
  },
  {
    check: {
      tool_id: 'game_performance',
      display_name: 'game-performance',
      is_available: false,
      is_required: false,
      category: 'performance',
      docs_url: 'https://wiki.cachyos.org/',
      tool_version: null,
      resolved_path: null,
      install_guidance: {
        distro_family: MOCK_DETECTED_DISTRO_FAMILY,
        command: 'sudo pacman -S game-performance',
        alternatives:
          'Package is provided on CachyOS. On vanilla Arch, use CachyOS repos or an AUR helper if available.',
      },
    },
    details: {
      tool_id: 'game_performance',
      tool_version: null,
      resolved_path: null,
    },
  },
  {
    check: {
      tool_id: 'gamemode',
      display_name: 'GameMode',
      is_available: true,
      is_required: false,
      category: 'performance',
      docs_url: 'https://github.com/FeralInteractive/gamemode',
      tool_version: '1.8.1',
      resolved_path: '/usr/bin/gamemoderun',
      install_guidance: null,
    },
    details: {
      tool_id: 'gamemode',
      tool_version: '1.8.1',
      resolved_path: '/usr/bin/gamemoderun',
    },
  },
  {
    check: {
      tool_id: 'winetricks',
      display_name: 'Winetricks',
      is_available: true,
      is_required: false,
      category: 'prefix_tools',
      docs_url: 'https://github.com/Winetricks/winetricks',
      tool_version: '20250102-next',
      resolved_path: '/usr/bin/winetricks',
      install_guidance: null,
    },
    details: {
      tool_id: 'winetricks',
      tool_version: '20250102-next',
      resolved_path: '/usr/bin/winetricks',
    },
  },
  {
    check: {
      tool_id: 'protontricks',
      display_name: 'Protontricks',
      is_available: false,
      is_required: false,
      category: 'prefix_tools',
      docs_url: 'https://github.com/Matoking/protontricks',
      tool_version: null,
      resolved_path: null,
      install_guidance: {
        distro_family: MOCK_DETECTED_DISTRO_FAMILY,
        command: 'sudo pacman -S protontricks',
        alternatives: 'Or pip: pip install --user protontricks',
      },
    },
    details: {
      tool_id: 'protontricks',
      tool_version: null,
      resolved_path: null,
    },
  },
];

function isDismissReadinessNagArgs(value: unknown): value is DismissReadinessNagArgs {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const candidate = value as { toolId?: unknown };
  return typeof candidate.toolId === 'string' && candidate.toolId.trim() !== '';
}

function nowIso(): string {
  return new Date().toISOString();
}

function cloneToolCheck(toolCheck: HostToolCheckResult): HostToolCheckResult {
  return structuredClone(toolCheck);
}

function cloneToolChecks(toolChecks: HostToolCheckResult[]): HostToolCheckResult[] {
  return toolChecks.map((toolCheck) => cloneToolCheck(toolCheck));
}

function cloneInstallHint(hint: HostToolInstallCommand): HostToolInstallCommand {
  return structuredClone(hint);
}

function buildBaseToolChecks(): HostToolCheckResult[] {
  return MOCK_HOST_TOOL_DEFINITIONS.map(({ check }) => cloneToolCheck(check));
}

function getHostToolDetails(toolId: string): HostToolDetails {
  const definition = MOCK_HOST_TOOL_DEFINITIONS.find(({ check }) => check.tool_id === toolId);
  if (!definition) {
    return {
      tool_id: toolId,
      tool_version: null,
      resolved_path: null,
    };
  }
  return structuredClone(definition.details);
}

function buildReadinessChecks(toolChecks: HostToolCheckResult[]): ReadinessCheckResult['checks'] {
  return toolChecks.map((tool) => {
    if (tool.is_available) {
      const resolvedPath = (tool.resolved_path ?? '').trim();
      return {
        field: tool.tool_id,
        path: resolvedPath,
        message:
          resolvedPath === ''
            ? `${tool.display_name} is available.`
            : `${tool.display_name} is available at ${resolvedPath}.`,
        remediation: '',
        severity: 'info',
      };
    }
    const guidance = tool.install_guidance;
    const remediationParts = [(guidance?.command ?? '').trim(), (guidance?.alternatives ?? '').trim()].filter(
      (part) => part !== ''
    );
    return {
      field: tool.tool_id,
      path: '',
      message: `${tool.display_name} is missing.`,
      remediation: remediationParts.join(' ').trim(),
      severity: tool.is_required ? 'error' : 'warning',
    };
  });
}

function buildBaseReadinessPayload(): ReadinessCheckResult {
  const toolChecks = buildBaseToolChecks();
  const warnings = toolChecks.filter((tool) => !tool.is_available && !tool.is_required).length;
  const criticalFailures = toolChecks.filter((tool) => !tool.is_available && tool.is_required).length;
  return {
    checks: buildReadinessChecks(toolChecks),
    all_passed: criticalFailures === 0 && warnings === 0,
    critical_failures: criticalFailures,
    warnings,
    umu_install_guidance: {
      install_command: 'sudo pacman -S umu-launcher',
      docs_url: 'https://github.com/Open-Wine-Components/umu-launcher',
      description:
        'Install umu-launcher on your host to enable improved Proton runtime bootstrapping for non-Steam launches.',
    },
    steam_deck_caveats: {
      description:
        'CrossHook works on Steam Deck desktop mode today. In gaming mode you may hit these documented upstream issues on SteamOS 3.7+:',
      items: [
        'Black screen until Shader Pre-Caching completes — enable it in Steam Settings → Downloads → Shader Pre-Caching',
        'Steam overlay can render below the game under gamescope + Flatpak',
        'HDR + gamescope + Flatpak regression on SteamOS 3.7.13 (toggle HDR off if the screen tints or flickers)',
      ],
      docs_url: 'https://github.com/ValveSoftware/gamescope/issues',
    },
    tool_checks: toolChecks,
    detected_distro_family: MOCK_DETECTED_DISTRO_FAMILY,
  };
}

function applyReadinessOverlays(raw: ReadinessCheckResult): ReadinessCheckResult {
  const store = getStore();
  const toggles = getActiveToggles();
  const dismissedToolIds = store.dismissedReadinessToolIds;
  const toolChecks = (raw.tool_checks ?? []).map((tool) =>
    dismissedToolIds.has(tool.tool_id)
      ? {
          ...tool,
          install_guidance: null,
        }
      : cloneToolCheck(tool)
  );

  const umuInstallGuidance =
    store.settings.install_nag_dismissed_at != null || dismissedToolIds.has('umu_run')
      ? null
      : raw.umu_install_guidance;
  const steamDeckCaveats =
    !toggles.showSteamDeckCaveats ||
    store.settings.steam_deck_caveats_dismissed_at != null ||
    dismissedToolIds.has('steam_deck_caveats')
      ? null
      : raw.steam_deck_caveats;

  return {
    ...raw,
    checks: buildReadinessChecks(toolChecks),
    umu_install_guidance: umuInstallGuidance,
    steam_deck_caveats: steamDeckCaveats,
    tool_checks: toolChecks,
  };
}

function persistReadinessSnapshot(raw: ReadinessCheckResult): void {
  cachedHostReadinessSnapshot = {
    checked_at: nowIso(),
    detected_distro_family: raw.detected_distro_family ?? '',
    tool_checks: cloneToolChecks(raw.tool_checks ?? []),
    all_passed: raw.all_passed,
    critical_failures: raw.critical_failures,
    warnings: raw.warnings,
  };
}

function buildMockReadinessResult(): ReadinessCheckResult {
  const raw = buildBaseReadinessPayload();
  return applyReadinessOverlays(raw);
}

function buildCapabilityRationale(
  label: string,
  state: Capability['state'],
  missingRequired: HostToolCheckResult[],
  missingOptional: HostToolCheckResult[]
): string | null {
  if (state === 'available') {
    return null;
  }
  if (state === 'unavailable') {
    const names = missingRequired.map((tool) => tool.display_name).join(', ');
    return `${label} is unavailable because ${names} ${missingRequired.length === 1 ? 'is' : 'are'} missing.`;
  }
  const names = missingOptional.map((tool) => tool.display_name).join(', ');
  return `${label} is degraded because optional tooling is missing: ${names}.`;
}

function collectInstallHints(toolChecks: HostToolCheckResult[]): HostToolInstallCommand[] {
  const seen = new Set<string>();
  const hints: HostToolInstallCommand[] = [];

  for (const toolCheck of toolChecks) {
    const hint = toolCheck.install_guidance;
    if (!hint) {
      continue;
    }
    const key = `${hint.distro_family}\u0000${hint.command}\u0000${hint.alternatives}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    hints.push(cloneInstallHint(hint));
  }

  return hints;
}

function resolveCapabilityToolCheck(
  toolChecks: HostToolCheckResult[],
  toolId: string,
  isRequired: boolean
): HostToolCheckResult {
  const toolCheck = toolChecks.find((candidate) => candidate.tool_id === toolId);
  if (!toolCheck) {
    return {
      tool_id: toolId,
      display_name: toolId,
      is_available: false,
      is_required: isRequired,
      category: 'runtime',
      docs_url: undefined,
      tool_version: null,
      resolved_path: null,
      install_guidance: null,
    };
  }

  return {
    ...cloneToolCheck(toolCheck),
    is_required: isRequired,
  };
}

function deriveMockCapabilities(toolChecks: HostToolCheckResult[]): Capability[] {
  return MOCK_CAPABILITY_DEFINITIONS.map((definition) => {
    const missingRequired = definition.requiredToolIds
      .map((toolId) => resolveCapabilityToolCheck(toolChecks, toolId, true))
      .filter((toolCheck) => !toolCheck.is_available);
    const missingOptional = definition.optionalToolIds
      .map((toolId) => resolveCapabilityToolCheck(toolChecks, toolId, false))
      .filter((toolCheck) => !toolCheck.is_available);

    const state: Capability['state'] =
      missingRequired.length > 0 ? 'unavailable' : missingOptional.length > 0 ? 'degraded' : 'available';

    return {
      id: definition.id,
      label: definition.label,
      category: definition.category,
      state,
      rationale: buildCapabilityRationale(definition.label, state, missingRequired, missingOptional),
      required_tool_ids: [...definition.requiredToolIds],
      optional_tool_ids: [...definition.optionalToolIds],
      missing_required: missingRequired,
      missing_optional: missingOptional,
      install_hints: collectInstallHints([...missingRequired, ...missingOptional]),
    };
  });
}

function requireProbeToolId(value: unknown): string {
  if (typeof value === 'string' && value.trim() !== '') {
    return value;
  }
  if (typeof value === 'object' && value !== null) {
    const candidate = value as ProbeHostToolDetailsArgs;
    const toolId = candidate.toolId ?? candidate.tool_id;
    if (typeof toolId === 'string' && toolId.trim() !== '') {
      return toolId;
    }
  }
  throw new Error('probe_host_tool_details requires a non-empty toolId');
}

function maybeSynthesizeOnboardingEvent(): void {
  if (onboardingEventSynthesized) return;
  if (!getActiveToggles().showOnboarding) return;
  if (onboardingSynthesisScheduled) return;
  onboardingSynthesisScheduled = true;

  let attempts = 0;

  const tryEmit = (): void => {
    if (onboardingEventSynthesized) return;
    const store = getStore();
    const payload: OnboardingCheckPayload = {
      show: true,
      has_profiles: store.profiles.size > 0,
    };
    if (emitMockEvent('onboarding-check', payload)) {
      onboardingEventSynthesized = true;
      return;
    }
    attempts += 1;
    if (attempts >= ONBOARDING_EMIT_MAX_ATTEMPTS) {
      return;
    }
    setTimeout(tryEmit, ONBOARDING_EMIT_RETRY_MS);
  };

  setTimeout(tryEmit, ONBOARDING_EMIT_INITIAL_MS);
}

// Eagerly schedule the synthesized event at module init so it fires even if
// nothing else triggers `registerOnboarding()` later. The guard above makes
// the second call from `registerOnboarding()` a no-op.
maybeSynthesizeOnboardingEvent();

export function registerOnboarding(map: Map<string, Handler>): void {
  maybeSynthesizeOnboardingEvent();
  map.set('check_readiness', async (): Promise<ReadinessCheckResult> => {
    return buildMockReadinessResult();
  });

  map.set('check_generalized_readiness', async (): Promise<ReadinessCheckResult> => {
    const raw = buildBaseReadinessPayload();
    persistReadinessSnapshot(raw);
    return applyReadinessOverlays(raw);
  });

  map.set('probe_host_tool_details', async (args: unknown): Promise<HostToolDetails> => {
    return getHostToolDetails(requireProbeToolId(args));
  });

  map.set('get_cached_host_readiness_snapshot', async (): Promise<CachedHostReadinessSnapshot | null> => {
    if (cachedHostReadinessSnapshot == null) {
      return null;
    }

    const snapshot = structuredClone(cachedHostReadinessSnapshot);
    const dismissedToolIds = getStore().dismissedReadinessToolIds;
    snapshot.tool_checks = snapshot.tool_checks.map((tool) =>
      dismissedToolIds.has(tool.tool_id)
        ? {
            ...tool,
            install_guidance: null,
          }
        : tool
    );
    return snapshot;
  });

  map.set('get_capabilities', async (): Promise<Capability[]> => {
    if (cachedHostReadinessSnapshot == null) {
      persistReadinessSnapshot(buildBaseReadinessPayload());
    }
    const snapshot = cachedHostReadinessSnapshot;
    if (snapshot == null) {
      return [];
    }
    return structuredClone(deriveMockCapabilities(snapshot.tool_checks));
  });

  map.set('dismiss_onboarding', async (): Promise<null> => {
    onboardingDismissed = true;
    const store = getStore();
    store.settings.onboarding_completed = true;
    return null;
  });

  map.set('dismiss_umu_install_nag', async (): Promise<null> => {
    const store = getStore();
    store.settings.install_nag_dismissed_at = new Date().toISOString();
    return null;
  });

  map.set('dismiss_steam_deck_caveats', async (): Promise<null> => {
    getStore().settings.steam_deck_caveats_dismissed_at = new Date().toISOString();
    return null;
  });

  map.set('dismiss_readiness_nag', async (args: unknown): Promise<null> => {
    if (!isDismissReadinessNagArgs(args)) {
      throw new Error('dismiss_readiness_nag requires a non-empty toolId');
    }
    getStore().dismissedReadinessToolIds.add(args.toolId);
    return null;
  });

  map.set('get_trainer_guidance', async (): Promise<TrainerGuidanceContent> => {
    return {
      loading_modes: [
        {
          id: 'source_directory',
          title: 'Source Directory',
          description: 'Proton reads the trainer directly from its downloaded location. The trainer stays in place.',
          when_to_use: 'Use when the trainer runs standalone without extra DLLs or support files.',
          examples: ['FLiNG single-file .exe trainers'],
        },
        {
          id: 'copy_to_prefix',
          title: 'Copy to Prefix',
          description:
            "CrossHook copies the trainer and support files into the WINE prefix's C:\\ drive before launch.",
          when_to_use: 'Use when the trainer bundles DLLs or support files that must be present in the WINE prefix.',
          examples: ['FLiNG trainers that bundle DLLs', 'Trainers with companion .ini or .dat files'],
        },
      ],
      trainer_sources: [
        {
          id: 'fling',
          title: 'FLiNG Trainers',
          description: 'FLiNG standalone .exe trainers — free, no account required. Primary recommendation.',
          when_to_use: 'Primary recommendation — no account needed, direct .exe download.',
          examples: ['flingtrainer.com standalone executables'],
        },
        {
          id: 'wemod',
          title: 'WeMod',
          description:
            'WeMod extracted trainers — requires a WeMod account and the WeMod desktop app installed under WINE.',
          when_to_use: 'Use only if WeMod is already set up under WINE. See wemod-launcher for setup instructions.',
          examples: ['WeMod extracted trainer DLLs'],
        },
      ],
      verification_steps: [
        'Verify the trainer .exe file exists at the configured path.',
        "Confirm the game version matches the trainer's target version.",
        'For Copy to Prefix mode: ensure companion DLLs and support files are in the same directory.',
        'Launch the game at least once to initialize the WINE prefix before using trainers.',
      ],
    };
  });
}

export { onboardingDismissed };
