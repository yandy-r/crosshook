import { useEffect } from 'react';
import { useProtonInstallProgress } from '../../hooks/useProtonInstallProgress';
import type { ProtonInstallPhase } from '../../types/protonup';

interface InstallProgressBarProps {
  opId: string;
  onCancel: (opId: string) => void;
  onDismiss: (opId: string) => void;
}

/** How long to keep a successful install visible before auto-dismissing. */
const AUTO_DISMISS_DONE_MS = 4000;

const TERMINAL_PHASES: Set<ProtonInstallPhase> = new Set(['done', 'failed', 'cancelled']);

function phaseLabel(phase: ProtonInstallPhase, percent: number | null): string {
  switch (phase) {
    case 'resolving':
      return 'Resolving…';
    case 'downloading':
      return percent !== null ? `Downloading ${percent}%` : 'Downloading…';
    case 'verifying':
      return 'Verifying…';
    case 'extracting':
      return 'Extracting…';
    case 'finalizing':
      return 'Finalizing…';
    case 'done':
      return 'Completed';
    case 'failed':
      return 'Failed';
    case 'cancelled':
      return 'Cancelled';
    default: {
      const _exhaustive: never = phase;
      return String(_exhaustive);
    }
  }
}

function phaseModifier(phase: ProtonInstallPhase): string {
  if (phase === 'done') return ' crosshook-install-progress__phase--done';
  if (phase === 'failed') return ' crosshook-install-progress__phase--failed';
  if (phase === 'cancelled') return ' crosshook-install-progress__phase--cancelled';
  return '';
}

function fillModifier(phase: ProtonInstallPhase): string {
  if (phase === 'done') return ' crosshook-install-progress__fill--done';
  if (phase === 'failed') return ' crosshook-install-progress__fill--failed';
  return '';
}

export function InstallProgressBar({ opId, onCancel, onDismiss }: InstallProgressBarProps) {
  const { progress, percent } = useProtonInstallProgress(opId);

  // While no progress event has arrived yet show an indeterminate state.
  const phase = progress?.phase ?? 'resolving';
  const displayPercent = percent ?? (TERMINAL_PHASES.has(phase) ? 100 : 0);
  const isTerminal = TERMINAL_PHASES.has(phase);

  // Auto-dismiss successful installs after a short grace period so the
  // status container doesn't linger. Failures and cancellations stay until
  // the user dismisses them explicitly.
  useEffect(() => {
    if (phase !== 'done') return;
    const timeout = window.setTimeout(() => onDismiss(opId), AUTO_DISMISS_DONE_MS);
    return () => window.clearTimeout(timeout);
  }, [phase, opId, onDismiss]);

  const installLabel = progress?.message ?? `Installing ${opId.slice(0, 8)}…`;

  return (
    <div className="crosshook-install-progress" role="status" aria-live="polite">
      <div className="crosshook-install-progress__top">
        <span className="crosshook-install-progress__label">{installLabel}</span>
        <span className={`crosshook-install-progress__phase${phaseModifier(phase)}`}>{phaseLabel(phase, percent)}</span>
        {isTerminal ? (
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small"
            onClick={() => onDismiss(opId)}
            aria-label="Dismiss install status"
          >
            Dismiss
          </button>
        ) : (
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small"
            onClick={() => onCancel(opId)}
            aria-label={`Cancel install ${opId.slice(0, 8)}`}
          >
            Cancel
          </button>
        )}
      </div>

      <div className="crosshook-install-progress__track" aria-hidden="true">
        <div
          className={`crosshook-install-progress__fill${fillModifier(phase)}`}
          style={{ width: `${displayPercent}%` }}
        />
      </div>
    </div>
  );
}
