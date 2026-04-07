import TrainerDiscoveryPanel from '../TrainerDiscoveryPanel';
import { RouteBanner } from '../layout/RouteBanner';

export function DiscoverPage() {
  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--discover">
      <div className="crosshook-route-stack crosshook-discover-page">
        <div className="crosshook-route-stack__body--fill crosshook-discover-page__body">
          <RouteBanner route="discover" />
          <div className="crosshook-route-card-host">
            <div className="crosshook-route-card-scroll">
              <TrainerDiscoveryPanel />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default DiscoverPage;
