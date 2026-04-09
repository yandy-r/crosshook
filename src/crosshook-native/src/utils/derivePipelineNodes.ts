import type { GameProfile } from '../types/profile';
import type { LaunchPhase, LaunchPreview, PipelineNode, PipelineNodeStatus } from '../types/launch';
import type { ResolvedLaunchMethod } from './launch';

/**
 * Derives Tier-1 (config-only) pipeline nodes for the launch method. `preview` and `phase` are
 * reserved for later phases (preview-driven paths, active-step animation) and ignored in Phase 1.
 *
 * Example shapes (Tier 1, `preview === null`):
 * - `proton_run`: game → wine-prefix → proton → trainer → optimizations → launch
 * - `steam_applaunch`: game → steam → trainer → optimizations → launch
 * - `native`: game → trainer → launch
 */
export function derivePipelineNodes(
  method: ResolvedLaunchMethod,
  profile: GameProfile,
  _preview: LaunchPreview | null,
  _phase: LaunchPhase
): PipelineNode[] {
  const ids = METHOD_NODE_IDS[method];
  const nodes: PipelineNode[] = [];

  for (let i = 0; i < ids.length; i += 1) {
    const id = ids[i];
    const label = NODE_DEFS[id]?.label ?? id;
    let status: PipelineNodeStatus;

    if (id === 'launch') {
      const prior = ids.slice(0, i);
      const allPriorConfigured = prior.every((pid) => tier1Status(pid, profile, method) === 'configured');
      status = allPriorConfigured ? 'configured' : 'not-configured';
    } else {
      status = tier1Status(id, profile, method);
    }

    nodes.push({ id, label, status });
  }

  return nodes;
}

const NODE_DEFS: Record<string, { label: string }> = {
  game: { label: 'Game' },
  'wine-prefix': { label: 'Wine Prefix' },
  proton: { label: 'Proton' },
  steam: { label: 'Steam' },
  trainer: { label: 'Trainer' },
  optimizations: { label: 'Optimizations' },
  launch: { label: 'Launch' },
};

const METHOD_NODE_IDS: Record<ResolvedLaunchMethod, readonly string[]> = {
  proton_run: ['game', 'wine-prefix', 'proton', 'trainer', 'optimizations', 'launch'],
  steam_applaunch: ['game', 'steam', 'trainer', 'optimizations', 'launch'],
  native: ['game', 'trainer', 'launch'],
};

function tier1Status(
  nodeId: string,
  profile: GameProfile,
  method: ResolvedLaunchMethod
): Extract<PipelineNodeStatus, 'configured' | 'not-configured'> {
  switch (nodeId) {
    case 'game':
      return profile.game.executable_path.trim() !== '' ? 'configured' : 'not-configured';
    case 'wine-prefix':
      return profile.runtime.prefix_path.trim() !== '' ? 'configured' : 'not-configured';
    case 'proton':
      return profile.runtime.proton_path.trim() !== '' ? 'configured' : 'not-configured';
    case 'steam':
      return method === 'steam_applaunch' ? 'configured' : 'not-configured';
    case 'trainer':
      return profile.trainer.path.trim() !== '' ? 'configured' : 'not-configured';
    case 'optimizations':
      return profile.launch.optimizations.enabled_option_ids.length > 0 ? 'configured' : 'not-configured';
    default:
      return 'not-configured';
  }
}
