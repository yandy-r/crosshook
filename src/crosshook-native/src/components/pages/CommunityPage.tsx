import CommunityBrowser from '../CommunityBrowser';
import { PageBanner, CommunityArt } from '../layout/PageBanner';
import { useCommunityProfiles } from '../../hooks/useCommunityProfiles';

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

export function CommunityPage() {
  const communityState = useCommunityProfiles({
    profilesDirectoryPath: DEFAULT_PROFILES_DIRECTORY,
  });

  return (
    <>
      <PageBanner
        eyebrow="Community"
        title="Browse shared profiles"
        copy="Review community taps, sync the latest index, and import shared profiles without leaving the page shell."
        illustration={<CommunityArt />}
      />
      <CommunityBrowser profilesDirectoryPath={DEFAULT_PROFILES_DIRECTORY} state={communityState} />
    </>
  );
}

export default CommunityPage;
