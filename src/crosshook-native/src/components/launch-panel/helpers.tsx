import type { LaunchValidationSeverity, PatternMatch } from '../../types';

export function sortPatternMatchesBySeverity(matches: PatternMatch[]): PatternMatch[] {
  const order: Record<LaunchValidationSeverity, number> = { fatal: 0, warning: 1, info: 2 };
  return [...matches].sort((a, b) => order[a.severity] - order[b.severity]);
}
