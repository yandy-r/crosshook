import {
  LaunchPhase,
  type LaunchPreview,
  type LaunchValidationIssue,
  type PipelineNode,
  type PipelineNodeStatus,
} from '../types/launch';
import type { GameProfile } from '../types/profile';
import type { ResolvedLaunchMethod } from './launch';
import { groupIssuesByNode, type PipelineNodeId } from './mapValidationToNode';

/**
 * Derives pipeline nodes for the launch method. Tier 1 (config-only) when `preview` is null; Tier 2
 * (preview-derived validation and resolved paths) when `preview` is set. When `phase` is not `Idle`,
 * a live launch overlay updates active/complete nodes without mutating the base Tier 1/2 derivation.
 */
export function derivePipelineNodes(
  method: ResolvedLaunchMethod,
  profile: GameProfile,
  preview: LaunchPreview | null,
  phase: LaunchPhase
): PipelineNode[] {
  const ids = METHOD_NODE_IDS[method];
  const issuesByNode = preview
    ? groupIssuesByNode(preview.validation.issues)
    : new Map<PipelineNodeId, LaunchValidationIssue[]>();
  const baseNodes: PipelineNode[] = [];

  for (let i = 0; i < ids.length; i += 1) {
    const id = ids[i];
    const label = NODE_DEFS[id]?.label ?? id;

    if (preview && id === 'launch') {
      baseNodes.push(buildLaunchNode(label, baseNodes, preview, issuesByNode));
    } else if (preview && id !== 'launch') {
      baseNodes.push(buildTier2Node(id, label, preview, issuesByNode));
    } else if (id === 'launch') {
      const prior = ids.slice(0, i);
      const allPriorConfigured = prior.every((pid) => tier1Status(pid, profile, method) === 'configured');
      const status: PipelineNodeStatus = allPriorConfigured ? 'configured' : 'not-configured';
      baseNodes.push({ id, label, status });
    } else {
      baseNodes.push({ id, label, status: tier1Status(id, profile, method) });
    }
  }

  return applyPhaseOverlay(method, phase, baseNodes);
}

function applyPhaseOverlay(method: ResolvedLaunchMethod, phase: LaunchPhase, base: PipelineNode[]): PipelineNode[] {
  if (phase === LaunchPhase.Idle) {
    return base;
  }

  const nodes = base.map((n) => ({ ...n }));
  const ids = METHOD_NODE_IDS[method];
  const patch = (id: PipelineNodeId, partial: Partial<PipelineNode>): void => {
    const idx = nodes.findIndex((n) => n.id === id);
    if (idx < 0) {
      return;
    }
    nodes[idx] = { ...nodes[idx], ...partial };
  };

  // Display label for the waiting state
  const WAITING_DETAIL = 'Waiting' as const;

  switch (phase) {
    case LaunchPhase.GameLaunching:
      patch('game', { status: 'active', tone: undefined });
      break;

    case LaunchPhase.WaitingForTrainer:
      patch('game', { status: 'complete', tone: undefined });
      if (ids.includes('trainer')) {
        patch('trainer', {
          status: 'active',
          tone: 'waiting',
          detail: WAITING_DETAIL,
        });
      }
      break;

    case LaunchPhase.TrainerLaunching:
      patch('game', { status: 'complete', tone: undefined });
      if (ids.includes('trainer')) {
        patch('trainer', { status: 'active', tone: undefined });
      }
      break;

    case LaunchPhase.SessionActive:
      for (let i = 0; i < nodes.length; i += 1) {
        const n = nodes[i];
        if (n.id === 'launch') {
          nodes[i] = { ...n, status: 'active', tone: undefined };
        } else {
          nodes[i] = { ...n, status: 'complete', tone: undefined };
        }
      }
      break;

    default:
      phase satisfies never;
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
      return profile.launch.optimizations.enabled_option_ids.length > 0 ? 'configured' : 'not-configured';
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
    case 'game': {
      const name = preview.game_executable_name?.trim();
      if (name) {
        return name;
      }
      const exe = preview.game_executable?.trim();
      return exe ? lastPathSegment(exe) : undefined;
    }
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
      return p ? lastPathSegment(p) : undefined;
    }
    case 'optimizations': {
      if (preview.environment === null && preview.wrappers === null) {
        return undefined;
      }
      const envCount = preview.environment?.length ?? 0;
      return envCount > 0 ? `${envCount} env vars` : 'No optimizations';
    }
  }
}

function isTier2Resolved(id: Exclude<PipelineNodeId, 'launch'>, preview: LaunchPreview): boolean {
  switch (id) {
    case 'game':
      return Boolean(preview.game_executable_name?.trim() || preview.game_executable?.trim());
    case 'wine-prefix':
      return Boolean(preview.proton_setup?.wine_prefix_path?.trim());
    case 'proton':
      return Boolean(preview.proton_setup?.proton_executable?.trim());
    case 'steam':
      return preview.resolved_method === 'steam_applaunch';
    case 'trainer':
      return preview.trainer !== null;
    case 'optimizations':
      return preview.environment !== null || preview.wrappers !== null;
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
  // NOTE: Only `fatal` severity issues promote a node to `error` status.
  // `warning`-severity issues are intentionally not surfaced in the pipeline
  // visualization yet — the mock fixture `__MOCK_VALIDATION_WARNING__` exists
  // but the display path is a no-op. See #74 for future warning indicator work.
  if (id === 'optimizations' && preview.directives_error) {
    return { id, label, status: 'error', detail: preview.directives_error };
  }
  const detail = tier2DetailForNode(id, preview);
  if (!isTier2Resolved(id, preview)) {
    return {
      id,
      label,
      status: 'not-configured',
      detail: detail ?? 'Not configured',
    };
  }
  return {
    id,
    label,
    status: 'configured',
    detail,
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
  const hasNotConfigured = priorNodes.some((n) => n.status === 'not-configured');
  if (hasNotConfigured) {
    return { id: 'launch', label, status: 'not-configured', detail: 'Complete steps above' };
  }
  if (!preview.effective_command?.trim()) {
    return { id: 'launch', label, status: 'not-configured', detail: 'Not ready' };
  }
  return {
    id: 'launch',
    label,
    status: 'configured',
    detail: 'Command ready',
  };
}
