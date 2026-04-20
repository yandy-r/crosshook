import type {
  Capability,
  HostToolCheckResult,
  HostToolDetails,
  HostToolInstallCommand,
  ReadinessCheckResult,
} from '../../../types/onboarding';
import { getActiveToggles } from '../../toggles';
import { getStore } from '../store';
import {
  type CachedHostReadinessSnapshot,
  type DismissReadinessNagArgs,
  MOCK_CAPABILITY_DEFINITIONS,
  MOCK_DETECTED_DISTRO_FAMILY,
  MOCK_HOST_TOOL_DEFINITIONS,
  type MockCapabilityDefinition,
  type ProbeHostToolDetailsArgs,
} from './onboarding-constants';
import { setCachedHostReadinessSnapshot } from './onboarding-state';

export function isDismissReadinessNagArgs(value: unknown): value is DismissReadinessNagArgs {
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

export function getHostToolDetails(toolId: string): HostToolDetails {
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

export function buildBaseReadinessPayload(): ReadinessCheckResult {
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

export function applyReadinessOverlays(raw: ReadinessCheckResult): ReadinessCheckResult {
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

export function persistReadinessSnapshot(raw: ReadinessCheckResult): void {
  const snapshot: CachedHostReadinessSnapshot = {
    checked_at: nowIso(),
    detected_distro_family: raw.detected_distro_family ?? '',
    tool_checks: cloneToolChecks(raw.tool_checks ?? []),
    all_passed: raw.all_passed,
    critical_failures: raw.critical_failures,
    warnings: raw.warnings,
  };
  setCachedHostReadinessSnapshot(snapshot);
}

export function buildMockReadinessResult(): ReadinessCheckResult {
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
      docs_url: '',
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

export function deriveMockCapabilities(toolChecks: HostToolCheckResult[]): Capability[] {
  return MOCK_CAPABILITY_DEFINITIONS.map((definition: MockCapabilityDefinition) => {
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

export function requireProbeToolId(value: unknown): string {
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

/** Strip install guidance for dismissed tools (matches real IPC `get_cached_host_readiness_snapshot`). */
export function sanitizeCachedSnapshot(
  snapshot: CachedHostReadinessSnapshot | null
): CachedHostReadinessSnapshot | null {
  if (snapshot == null) {
    return null;
  }

  const cloned = structuredClone(snapshot);
  const dismissedToolIds = getStore().dismissedReadinessToolIds;
  cloned.tool_checks = cloned.tool_checks.map((tool) =>
    dismissedToolIds.has(tool.tool_id)
      ? {
          ...tool,
          install_guidance: null,
        }
      : tool
  );
  return cloned;
}
