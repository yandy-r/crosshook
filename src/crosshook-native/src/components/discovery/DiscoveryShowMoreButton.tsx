export interface DiscoveryShowMoreButtonProps {
  shownCount: number;
  totalCount: number;
  onLoadMore: () => void;
}

/** Shared "Show more (n of total)" pagination button; renders nothing when
 * every entry is already visible. */
export function DiscoveryShowMoreButton({ shownCount, totalCount, onLoadMore }: DiscoveryShowMoreButtonProps) {
  if (shownCount >= totalCount) {
    return null;
  }

  return (
    <button
      type="button"
      className="crosshook-button crosshook-button--secondary crosshook-discovery-catalog__show-more"
      onClick={onLoadMore}
    >
      {`Show more (${shownCount} of ${totalCount})`}
    </button>
  );
}

export default DiscoveryShowMoreButton;
