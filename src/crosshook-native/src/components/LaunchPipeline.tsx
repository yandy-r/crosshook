import type { GameProfile } from '../types/profile';
import type { LaunchPhase, LaunchPreview, PipelineNodeStatus } from '../types/launch';
import type { ResolvedLaunchMethod } from '../utils/launch';
import { useMemo } from 'react';
import * as Tooltip from '@radix-ui/react-tooltip';
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
  const nodes = useMemo(() => derivePipelineNodes(method, profile, preview, phase), [method, profile, preview, phase]);
  const liveActiveIdx = nodes.findIndex((n) => n.status === 'active');
  const firstIssueIdx = nodes.findIndex(
    (n) => n.id !== 'launch' && (n.status === 'not-configured' || n.status === 'error')
  );
  const launchIndex = nodes.findIndex((n) => n.id === 'launch');
  // Prefer the live active step, then the first blocking issue, then the launch summary node.
  const currentStepIndex =
    liveActiveIdx >= 0 ? liveActiveIdx : firstIssueIdx >= 0 ? firstIssueIdx : launchIndex >= 0 ? launchIndex : 0;

  const announcement = useMemo(() => {
    const issues = nodes.filter(
      (n) => n.status === 'error' || n.status === 'not-configured' || n.status === 'active'
    );
    if (issues.length === 0) return 'All pipeline steps configured.';
    return (
      issues
        .map((n) => {
          const text = n.detail || STATUS_LABEL[n.status];
          return `${n.label}: ${text}`;
        })
        .join('. ') + '.'
    );
  }, [nodes]);

  return (
    <nav className="crosshook-launch-pipeline" aria-label="Launch pipeline">
      <ol className="crosshook-launch-pipeline__steps">
        {nodes.map((node, index) => {
          const statusText = node.detail || STATUS_LABEL[node.status];
          const hasDetail = Boolean(node.detail);
          const nodeContent = (
            <>
              <span className="crosshook-launch-pipeline__node-indicator" aria-hidden="true">
                {STATUS_ICON[node.status]}
              </span>
              <span className="crosshook-launch-pipeline__node-label">{node.label}</span>
              <span className="crosshook-launch-pipeline__node-status">{statusText}</span>
            </>
          );

          return (
            <li
              key={node.id}
              className="crosshook-launch-pipeline__node"
              data-status={node.status}
              data-tone={node.tone}
              aria-current={index === currentStepIndex ? 'step' : undefined}
              aria-label={`${node.label}: ${statusText}`}
            >
              {hasDetail ? (
                <Tooltip.Root>
                  <Tooltip.Trigger asChild>
                    <span className="crosshook-launch-pipeline__node-trigger" tabIndex={0}>
                      {nodeContent}
                    </span>
                  </Tooltip.Trigger>
                  <Tooltip.Portal>
                    <Tooltip.Content
                      side="top"
                      sideOffset={6}
                      style={{
                        maxWidth: 280,
                        padding: '6px 10px',
                        borderRadius: 8,
                        fontSize: '0.8rem',
                        lineHeight: 1.4,
                        color: 'var(--crosshook-color-text)',
                        background: 'var(--crosshook-color-surface-raised, #2a2a2e)',
                        border: '1px solid var(--crosshook-color-border-strong)',
                        boxShadow: '0 4px 12px rgba(0,0,0,0.35)',
                        zIndex: 9999,
                      }}
                    >
                      {node.detail}
                      <Tooltip.Arrow style={{ fill: 'var(--crosshook-color-surface-raised, #2a2a2e)' }} />
                    </Tooltip.Content>
                  </Tooltip.Portal>
                </Tooltip.Root>
              ) : (
                nodeContent
              )}
            </li>
          );
        })}
      </ol>
      <div className="crosshook-visually-hidden" aria-live="polite" aria-atomic="true">
        {announcement}
      </div>
    </nav>
  );
}

export default LaunchPipeline;
