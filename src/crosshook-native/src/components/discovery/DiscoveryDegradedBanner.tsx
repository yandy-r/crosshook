export const DEGRADED_CATALOG_MESSAGE =
  'Trainer catalog is running in reduced mode (metadata database unavailable). Results are served from in-memory tap data.';

/** Shared degraded-mode banner for catalog surfaces (Discover + Browse). */
export function DiscoveryDegradedBanner() {
  return (
    <div className="crosshook-warning-banner crosshook-warning-banner--section" role="status">
      {DEGRADED_CATALOG_MESSAGE}
    </div>
  );
}

export default DiscoveryDegradedBanner;
