import type { CommunityTapSubscription } from '../../hooks/useCommunityProfiles';

/** Stable React key / row id for a tap subscription (same repo URL can appear on multiple branches or pins). */
export function tapSubscriptionStableKey(sub: CommunityTapSubscription): string {
  return `${sub.url}::${sub.branch ?? ''}::${sub.pinned_commit ?? ''}`;
}
