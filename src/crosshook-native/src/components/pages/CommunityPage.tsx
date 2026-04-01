import CommunityBrowser from '../CommunityBrowser';
import { useCommunityProfiles } from '../../hooks/useCommunityProfiles';

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

export function CommunityPage() {
  const communityState = useCommunityProfiles({
    profilesDirectoryPath: DEFAULT_PROFILES_DIRECTORY,
  });

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--community">
      <div className="crosshook-route-stack crosshook-community-page">
        <div className="crosshook-route-stack__body--fill crosshook-community-page__body">
          <div className="crosshook-route-card-host">
            <div className="crosshook-route-card-scroll">
              <CommunityBrowser profilesDirectoryPath={DEFAULT_PROFILES_DIRECTORY} state={communityState} />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default CommunityPage;
