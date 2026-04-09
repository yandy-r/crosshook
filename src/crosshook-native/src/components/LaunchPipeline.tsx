import type { GameProfile } from '../types/profile';
import type { LaunchPhase, LaunchPreview, PipelineNode, PipelineNodeStatus } from '../types/launch';
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
  const firstIncompleteIdx = nodes.findIndex((n) => n.id !== 'launch' && n.status === 'not-configured');
  const launchIndex = nodes.findIndex((n) => n.id === 'launch');
  // Fallback to 0 is unreachable: every pipeline ends with a 'launch' node (launchIndex always >= 0)
  const currentStepIndex =
    firstIncompleteIdx >= 0 ? firstIncompleteIdx : launchIndex >= 0 ? launchIndex : 0;

  return (
    <nav className="crosshook-launch-pipeline" aria-label="Launch pipeline">
      <ol className="crosshook-launch-pipeline__steps">
        {nodes.map((node, index) => (
          <li
            key={node.id}
            className="crosshook-launch-pipeline__node"
            data-status={node.status}
            aria-current={index === currentStepIndex ? 'step' : undefined}
            aria-label={`${node.label}: ${STATUS_LABEL[node.status]}`}
          >
            <span className="crosshook-launch-pipeline__node-indicator" aria-hidden="true">
              {STATUS_ICON[node.status]}
            </span>
            <span className="crosshook-launch-pipeline__node-label">{node.label}</span>
            <span className="crosshook-launch-pipeline__node-status">{STATUS_LABEL[node.status]}</span>
          </li>
        ))}
      </ol>
    </nav>
  );
}

export default LaunchPipeline;
