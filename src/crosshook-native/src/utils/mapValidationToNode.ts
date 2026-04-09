import type { LaunchValidationIssue } from '../types/launch';

export type PipelineNodeId =
  | 'game'
  | 'wine-prefix'
  | 'proton'
  | 'steam'
  | 'trainer'
  | 'optimizations'
  | 'launch';

const SEVERITY_RANK: Record<LaunchValidationIssue['severity'], number> = {
  fatal: 0,
  warning: 1,
  info: 2,
};

function sortIssuesBySeverity(issues: LaunchValidationIssue[]): LaunchValidationIssue[] {
  return [...issues].sort((a, b) => SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity]);
}

/**
 * Maps a validation issue to the pipeline node it belongs to.
 * Returns 'launch' (summary node) for unmapped or code-less issues.
 */
export function mapValidationToNode(issue: LaunchValidationIssue): PipelineNodeId {
  const code = issue.code?.trim();
  if (!code) {
    return 'launch';
  }
  if (code.startsWith('game_path') || code.startsWith('native_windows_executable')) {
    return 'game';
  }
  if (code.startsWith('runtime_prefix') || code.startsWith('low_disk_space')) {
    return 'wine-prefix';
  }
  if (code.startsWith('runtime_proton')) {
    return 'proton';
  }
  if (code.startsWith('steam_')) {
    return 'steam';
  }
  if (code.startsWith('trainer_') || code.startsWith('native_trainer') || code.startsWith('unshare_net')) {
    return 'trainer';
  }
  if (
    code.startsWith('unknown_launch_optimization') ||
    code.startsWith('duplicate_launch_optimization') ||
    code.startsWith('incompatible_launch_optimization') ||
    code.startsWith('launch_optimization') ||
    code.startsWith('gamescope_') ||
    code.startsWith('custom_env_var')
  ) {
    return 'optimizations';
  }
  return 'launch';
}

/**
 * Groups validation issues by pipeline node, returning a Map sorted by severity (fatal first)
 * within each group.
 */
export function groupIssuesByNode(issues: LaunchValidationIssue[]): Map<PipelineNodeId, LaunchValidationIssue[]> {
  const map = new Map<PipelineNodeId, LaunchValidationIssue[]>();
  for (const issue of issues) {
    const nodeId = mapValidationToNode(issue);
    const next = map.get(nodeId) ?? [];
    next.push(issue);
    map.set(nodeId, next);
  }
  for (const [nodeId, nodeIssues] of map) {
    map.set(nodeId, sortIssuesBySeverity(nodeIssues));
  }
  return map;
}
