import type { ReactNode } from 'react';
import type { LaunchPreview, LaunchRequest, LaunchValidationSeverity, PatternMatch, PreviewEnvVar } from '../../types';
import { groupPreviewEnvBySource, launchMethodLabel } from '../../utils/launchPreviewPresentation';

export function severityIcon(severity: LaunchValidationSeverity): string {
  switch (severity) {
    case 'fatal':
      return '\u2717';
    case 'warning':
      return '!';
    default:
      return '\u2713';
  }
}

export function sortPatternMatchesBySeverity(matches: PatternMatch[]): PatternMatch[] {
  const order: Record<LaunchValidationSeverity, number> = { fatal: 0, warning: 1, info: 2 };
  return [...matches].sort((a, b) => order[a.severity] - order[b.severity]);
}

export function methodLabel(method: string): string {
  return launchMethodLabel(method);
}

export function groupEnvBySource(vars: PreviewEnvVar[]): [string, PreviewEnvVar[]][] {
  return groupPreviewEnvBySource(vars);
}

export function isStale(generatedAt: string): boolean {
  const generatedTime = new Date(generatedAt).getTime();
  if (!Number.isFinite(generatedTime)) {
    return true;
  }

  return Date.now() - generatedTime > 60_000;
}

export function buildSummaryParts(preview: LaunchPreview): ReactNode[] {
  const envCount = preview.environment?.length ?? 0;
  const wrapperCount = preview.wrappers?.length ?? 0;
  const fatalCount = preview.validation.issues.filter((i) => i.severity === 'fatal').length;
  const warningCount = preview.validation.issues.filter((i) => i.severity === 'warning').length;
  const passedCount = preview.validation.issues.filter((i) => i.severity === 'info').length;

  const parts: ReactNode[] = [
    <span key="env" className="crosshook-preview-modal__count--success">
      {envCount} env vars
    </span>,
    <span key="wrap" className="crosshook-preview-modal__count--success">
      {wrapperCount} {wrapperCount === 1 ? 'wrapper' : 'wrappers'}
    </span>,
  ];

  if (passedCount > 0) {
    parts.push(
      <span key="pass" className="crosshook-preview-modal__count--success">
        {passedCount} passed
      </span>
    );
  }
  if (warningCount > 0) {
    parts.push(
      <span key="warn" className="crosshook-preview-modal__count--warning">
        {warningCount} {warningCount === 1 ? 'warning' : 'warnings'}
      </span>
    );
  }
  if (fatalCount > 0) {
    parts.push(
      <span key="err" className="crosshook-preview-modal__count--danger">
        {fatalCount} {fatalCount === 1 ? 'error' : 'errors'}
      </span>
    );
  }
  if (preview.validation.issues.length === 0) {
    parts.push(
      <span key="ok" className="crosshook-preview-modal__count--success">
        all checks passed
      </span>
    );
  }

  return parts;
}

export function buildGameOnlyRequest(request: LaunchRequest): LaunchRequest {
  return {
    ...request,
    launch_game_only: true,
    launch_trainer_only: false,
    preview_target: 'game',
  };
}

export function buildTrainerOnlyRequest(request: LaunchRequest): LaunchRequest {
  return {
    ...request,
    launch_game_only: false,
    launch_trainer_only: true,
    preview_target: 'trainer',
  };
}
