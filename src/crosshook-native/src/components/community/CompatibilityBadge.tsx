import type { CommunityCompatibilityRating } from '../../hooks/useCommunityProfiles';

export const ratingOrder: CommunityCompatibilityRating[] = ['platinum', 'working', 'partial', 'broken', 'unknown'];

export const ratingLabel: Record<CommunityCompatibilityRating, string> = {
  unknown: 'Unknown',
  broken: 'Broken',
  partial: 'Partial',
  working: 'Working',
  platinum: 'Platinum',
};

export interface CompatibilityBadgeProps {
  rating: CommunityCompatibilityRating;
}

export function CompatibilityBadge({ rating }: CompatibilityBadgeProps) {
  return (
    <span className={`crosshook-community-rating-badge crosshook-community-rating-badge--${rating}`}>
      {ratingLabel[rating]}
    </span>
  );
}

export default CompatibilityBadge;
