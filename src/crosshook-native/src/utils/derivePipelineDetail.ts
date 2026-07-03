import type { EnvVarSource, LaunchPreview, PreviewWrapperSource, UmuDecisionPreview } from '../types/launch';
import { lastPathSegment } from './derivePipelineNodes';
import { envSourceLabel, umuGameIdResolutionSourceLabel } from './launchPreviewPresentation';

export interface EnvGroupVar {
  key: string;
  /** null for the cleared group. */
  value: string | null;
}

export interface EnvGroup {
  id: EnvVarSource | 'cleared';
  label: string;
  vars: EnvGroupVar[];
  cleared: boolean;
}

export type WrapperChainItemKind = 'isolation' | 'optimization' | 'gamescope' | 'runtime';

export interface WrapperChainItem {
  /** Stable key: `${kind}-${index}` for detail rows, 'runtime-umu' / 'runtime-proton'. */
  id: string;
  kind: WrapperChainItemKind;
  /** Display token, e.g. 'unshare --net', 'gamescope --', 'umu-run', 'proton'. */
  token: string;
  active: boolean;
  reason: string;
  /** Secondary line (umu csv coverage / gameid summary). */
  detail?: string;
  /** Set when the wrapper is absorbed into another invocation (e.g. 'gamescope --mangoapp'). */
  foldedInto?: string;
}

const ENV_GROUP_SOURCE_ORDER = [
  'host',
  'steam_proton',
  'proton_runtime',
  'launch_optimization',
  'profile_custom',
] as const satisfies readonly EnvVarSource[];

export function deriveEnvGroups(preview: LaunchPreview): EnvGroup[] {
  const environment = preview.environment;
  if (environment === null) {
    return [];
  }

  const varsBySource = new Map<EnvVarSource, EnvGroupVar[]>();
  for (const envVar of environment) {
    const entry: EnvGroupVar = { key: envVar.key, value: envVar.value };
    const bucket = varsBySource.get(envVar.source);
    if (bucket) {
      bucket.push(entry);
    } else {
      varsBySource.set(envVar.source, [entry]);
    }
  }

  const groups: EnvGroup[] = [];
  for (const source of ENV_GROUP_SOURCE_ORDER) {
    const vars = varsBySource.get(source);
    if (vars) {
      groups.push({ id: source, label: envSourceLabel(source), vars, cleared: false });
    }
  }

  if (preview.cleared_variables.length > 0) {
    groups.push({
      id: 'cleared',
      label: 'Cleared before launch',
      vars: preview.cleared_variables.map((key) => ({ key, value: null })),
      cleared: true,
    });
  }

  return groups;
}

function kindFor(source: PreviewWrapperSource): Exclude<WrapperChainItemKind, 'runtime'> {
  switch (source) {
    case 'optimization':
      return 'optimization';
    case 'network_isolation':
      return 'isolation';
    case 'gamescope':
      return 'gamescope';
  }
}

export function deriveWrapperChain(preview: LaunchPreview): WrapperChainItem[] | null {
  const details = preview.wrapper_details;
  if (details === null || details === undefined) {
    return null;
  }

  const chain: WrapperChainItem[] = details.map((detail, index) => ({
    id: `${kindFor(detail.source)}-${index}`,
    kind: kindFor(detail.source),
    token: detail.command.join(' '),
    active: detail.active,
    reason: detail.reason,
    foldedInto: detail.folded_into ?? undefined,
  }));

  const protonToken = lastPathSegment(preview.proton_setup?.proton_executable ?? '') || 'proton';
  const umuDecision = preview.umu_decision;
  if (umuDecision) {
    const detailParts = [
      `csv coverage: ${umuDecision.csv_coverage}`,
      umuDecision.gameid_resolution
        ? `GAMEID ${umuDecision.gameid_resolution.game_id} (${umuGameIdResolutionSourceLabel(umuDecision.gameid_resolution.source)})`
        : null,
    ];
    chain.push({
      id: 'runtime-umu',
      kind: 'runtime',
      token: 'umu-run',
      active: umuDecision.will_use_umu,
      reason: umuDecision.reason,
      detail: detailParts.filter(Boolean).join(' · '),
    });
    if (!umuDecision.will_use_umu) {
      chain.push({
        id: 'runtime-proton',
        kind: 'runtime',
        token: protonToken,
        active: true,
        reason: 'Direct Proton launch',
      });
    }
  } else if (preview.proton_setup) {
    chain.push({
      id: 'runtime-proton',
      kind: 'runtime',
      token: protonToken,
      active: true,
      reason: preview.resolved_method === 'steam_applaunch' ? 'Launched through Steam' : 'Direct Proton launch',
    });
  }

  return chain;
}

export function formatEnvGroupClipboard(group: EnvGroup): string {
  if (group.cleared) {
    return group.vars.map((envVar) => `unset ${envVar.key}`).join('\n');
  }
  return group.vars.map((envVar) => `${envVar.key}=${envVar.value ?? ''}`).join('\n');
}

export function detailSummaryMeta(
  groups: EnvGroup[],
  chain: WrapperChainItem[] | null,
  umuDecision: UmuDecisionPreview | null | undefined
): string {
  const envCount = groups.filter((group) => !group.cleared).reduce((total, group) => total + group.vars.length, 0);
  const parts = [`${envCount} env var${envCount === 1 ? '' : 's'}`];
  if (chain !== null) {
    const wrapperCount = chain.filter((item) => item.kind !== 'runtime' && item.active && !item.foldedInto).length;
    parts.push(`${wrapperCount} wrapper${wrapperCount === 1 ? '' : 's'}`);
  }
  if (umuDecision) {
    parts.push(umuDecision.will_use_umu ? 'umu: umu-run' : 'umu: direct Proton');
  }
  return parts.join(' · ');
}
