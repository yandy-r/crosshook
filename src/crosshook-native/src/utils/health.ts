import type { HealthCheckSummary, ProfileHealthReport } from '../types';

export function countProfileStatuses(
  profiles: ProfileHealthReport[],
): Pick<
  HealthCheckSummary,
  'healthy_count' | 'stale_count' | 'broken_count' | 'total_count'
> {
  let healthy_count = 0;
  let stale_count = 0;
  let broken_count = 0;

  for (const profile of profiles) {
    if (profile.status === 'healthy') {
      healthy_count++;
    } else if (profile.status === 'stale') {
      stale_count++;
    } else if (profile.status === 'broken') {
      broken_count++;
    }
  }

  return { healthy_count, stale_count, broken_count, total_count: profiles.length };
}
