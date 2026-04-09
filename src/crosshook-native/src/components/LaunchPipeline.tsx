import type { GameProfile } from '../types/profile';
import type { LaunchPhase, LaunchPreview, PipelineNodeStatus } from '../types/launch';
import type { ResolvedLaunchMethod } from '../utils/launch';
import { useMemo } from 'react';
import { derivePipelineNodes } from '../utils/derivePipelineNodes';
import '../styles/launch-pipeline.css';

interface LaunchPipelineProps {
  method: ResolvedLaunchMethod;
  profile: GameProfile;
  preview: LaunchPreview | null;
  phase: LaunchPhase;
}

const STATUS_ICON: Record<PipelineNodeStatus, string> = {
  configured: '\u2713',
  'not-configured': '\u2014',
  error: '\u2717',
  active: '\u25CF',
  complete: '\u2713',
};

const STATUS_LABEL: Record<PipelineNodeStatus, string> = {
  configured: 'Ready',
  'not-configured': 'Not configured',
  error: 'Error',
  active: 'Running',
  complete: 'Done',
};

export function LaunchPipeline({ method, profile, preview, phase }: LaunchPipelineProps) {
  const nodes = useMemo(
    () => derivePipelineNodes(method, profile, preview, phase),
    [method, profile, preview, phase]
  );
  const liveActiveIdx = nodes.findIndex((n) => n.status === 'active');
  const firstIssueIdx = nodes.findIndex(
    (n) => n.id !== 'launch' && (n.status === 'not-configured' || n.status === 'error')
  );
  const launchIndex = nodes.findIndex((n) => n.id === 'launch');
  // Prefer the live active step, then the first blocking issue, then the launch summary node.
  const currentStepIndex =
    liveActiveIdx >= 0
      ? liveActiveIdx
      : firstIssueIdx >= 0
        ? firstIssueIdx
        : launchIndex >= 0
          ? launchIndex
          : 0;

  return (
    <nav className="crosshook-launch-pipeline" aria-label="Launch pipeline">
      <ol className="crosshook-launch-pipeline__steps">
        {nodes.map((node, index) => {
          const statusText = node.detail || STATUS_LABEL[node.status];
          return (
            <li
              key={node.id}
              className="crosshook-launch-pipeline__node"
              data-status={node.status}
              data-tone={node.tone === 'waiting' ? 'waiting' : undefined}
              aria-current={index === currentStepIndex ? 'step' : undefined}
              aria-label={`${node.label}: ${statusText}`}
              title={node.detail}
            >
              <span className="crosshook-launch-pipeline__node-indicator" aria-hidden="true">
                {STATUS_ICON[node.status]}
              </span>
              <span className="crosshook-launch-pipeline__node-label">{node.label}</span>
              <span className="crosshook-launch-pipeline__node-status">{statusText}</span>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

export default LaunchPipeline;
