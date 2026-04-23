import { RouteBanner } from '../layout/RouteBanner';
import TrainerDiscoveryPanel from '../TrainerDiscoveryPanel';

export function DiscoverPage() {
  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--discover">
      <div className="crosshook-route-stack" data-crosshook-focus-zone="content">
        <RouteBanner route="discover" />
        <div className="crosshook-route-stack__body--scroll">
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
