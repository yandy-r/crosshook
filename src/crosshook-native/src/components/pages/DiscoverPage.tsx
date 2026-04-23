import { RouteBanner } from '../layout/RouteBanner';
import TrainerDiscoveryPanel from '../TrainerDiscoveryPanel';

export function DiscoverPage() {
  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--discover">
      <div className="crosshook-route-stack" data-crosshook-focus-zone="content">
        <RouteBanner route="discover" />
        <div className="crosshook-dashboard-route-body crosshook-dashboard-route-section-stack">
          <TrainerDiscoveryPanel />
        </div>
      </div>
    </div>
  );
}

export default DiscoverPage;
