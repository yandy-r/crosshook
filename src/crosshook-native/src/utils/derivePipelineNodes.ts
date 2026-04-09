import type { GameProfile } from '../types/profile';
import type {
  LaunchPhase,
  LaunchPreview,
  LaunchValidationIssue,
  PipelineNode,
  PipelineNodeStatus,
} from '../types/launch';
import type { ResolvedLaunchMethod } from './launch';
import {
  groupIssuesByNode,
  type PipelineNodeId,
} from './mapValidationToNode';

/**
 * Derives pipeline nodes for the launch method. Tier 1 (config-only) when `preview` is null; Tier 2
 * (preview-derived validation and resolved paths) when `preview` is set.
 */
export function derivePipelineNodes(
  method: ResolvedLaunchMethod,
  profile: GameProfile,
  preview: LaunchPreview | null,
  _phase: LaunchPhase
): PipelineNode[] {
  const ids = METHOD_NODE_IDS[method];
  const issuesByNode = preview ? groupIssuesByNode(preview.validation.issues) : null;
  const nodes: PipelineNode[] = [];

  for (let i = 0; i < ids.length; i += 1) {
    const id = ids[i];
    const label = NODE_DEFS[id]?.label ?? id;

    if (preview && id === 'launch') {
      nodes.push(buildLaunchNode(label, nodes, preview, issuesByNode!));
    } else if (preview && id !== 'launch') {
      nodes.push(buildTier2Node(id, label, preview, issuesByNode!));
    } else if (id === 'launch') {
      const prior = ids.slice(0, i);
      const allPriorConfigured = prior.every((pid) => tier1Status(pid, profile, method) === 'configured');
      const status: PipelineNodeStatus = allPriorConfigured ? 'configured' : 'not-configured';
      nodes.push({ id, label, status });
    } else {
      nodes.push({ id, label, status: tier1Status(id, profile, method) });
    }
  }

  return nodes;
}

const NODE_DEFS: Record<PipelineNodeId, { label: string }> = {
  game: { label: 'Game' },
  'wine-prefix': { label: 'Wine Prefix' },
  proton: { label: 'Proton' },
  steam: { label: 'Steam' },
  trainer: { label: 'Trainer' },
  optimizations: { label: 'Optimizations' },
  launch: { label: 'Launch' },
};

const METHOD_NODE_IDS: Record<ResolvedLaunchMethod, readonly PipelineNodeId[]> = {
  proton_run: ['game', 'wine-prefix', 'proton', 'trainer', 'optimizations', 'launch'],
  steam_applaunch: ['game', 'steam', 'trainer', 'optimizations', 'launch'],
  native: ['game', 'trainer', 'launch'],
};

function tier1Status(
  nodeId: PipelineNodeId,
  profile: GameProfile,
  _method: ResolvedLaunchMethod
): Extract<PipelineNodeStatus, 'configured' | 'not-configured'> {
  switch (nodeId) {
    case 'game':
      return profile.game.executable_path.trim() !== '' ? 'configured' : 'not-configured';
    case 'wine-prefix':
      return profile.runtime.prefix_path.trim() !== '' ? 'configured' : 'not-configured';
    case 'proton':
      return profile.runtime.proton_path.trim() !== '' ? 'configured' : 'not-configured';
    case 'steam':
      return profile.steam.app_id.trim() !== '' ? 'configured' : 'not-configured';
    case 'trainer':
      return profile.trainer.path.trim() !== '' ? 'configured' : 'not-configured';
    case 'optimizations':
      return 'configured';
    case 'launch':
      return 'not-configured';
  }
}

function lastPathSegment(path: string): string {
  const trimmed = path.trim();
  if (!trimmed) {
    return '';
  }
  const parts = trimmed.split(/[/\\]/);
  return parts[parts.length - 1] ?? trimmed;
}

function tier2DetailForNode(id: Exclude<PipelineNodeId, 'launch'>, preview: LaunchPreview): string | undefined {
  switch (id) {
    case 'game':
      return preview.game_executable_name || undefined;
    case 'wine-prefix': {
      const p = preview.proton_setup?.wine_prefix_path;
      return p ? lastPathSegment(p) : undefined;
    }
    case 'proton': {
      const p = preview.proton_setup?.proton_executable;
      return p ? lastPathSegment(p) : undefined;
    }
    case 'steam':
      return preview.steam_launch_options ? 'Launch options set' : 'Ready';
    case 'trainer': {
      const p = preview.trainer?.path;
      return p ? lastPathSegment(p) : 'Not configured';
    }
    case 'optimizations': {
      const envCount = preview.environment?.length ?? 0;
      return envCount > 0 ? `${envCount} env vars` : 'No optimizations';
    }
  }
}

function buildTier2Node(
  id: Exclude<PipelineNodeId, 'launch'>,
  label: string,
  preview: LaunchPreview,
  issuesByNode: Map<PipelineNodeId, LaunchValidationIssue[]>
): PipelineNode {
  const nodeIssues = issuesByNode.get(id) ?? [];
  const fatalIssue = nodeIssues.find((issue) => issue.severity === 'fatal');
  if (fatalIssue) {
    return { id, label, status: 'error', detail: fatalIssue.message };
  }
  if (id === 'optimizations' && preview.directives_error) {
    return { id, label, status: 'error', detail: preview.directives_error };
  }
  return {
    id,
    label,
    status: 'configured',
    detail: tier2DetailForNode(id, preview),
  };
}

function buildLaunchNode(
  label: string,
  priorNodes: PipelineNode[],
  preview: LaunchPreview,
  issuesByNode: Map<PipelineNodeId, LaunchValidationIssue[]>
): PipelineNode {
  const launchIssues = issuesByNode.get('launch') ?? [];
  const fatalLaunch = launchIssues.find((issue) => issue.severity === 'fatal');
  if (fatalLaunch) {
    return { id: 'launch', label, status: 'error', detail: fatalLaunch.message };
  }
  const hasError = priorNodes.some((n) => n.status === 'error');
  if (hasError) {
    return { id: 'launch', label, status: 'error', detail: 'Resolve errors above' };
  }
  return {
    id: 'launch',
    label,
    status: 'configured',
    detail: preview.effective_command ? 'Command ready' : 'Not ready',
  };
}
