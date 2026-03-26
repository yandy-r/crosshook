import LaunchOptimizationsPanel from '../LaunchOptimizationsPanel';
import LaunchPanel from '../LaunchPanel';
import SteamLaunchOptionsPanel from '../SteamLaunchOptionsPanel';
import { ThemedSelect } from '../ui/ThemedSelect';
import { useProfileContext } from '../../context/ProfileContext';
import { PageBanner, LaunchArt } from '../layout/PageBanner';
import type { GameProfile, LaunchMethod, LaunchRequest } from '../../types';

function buildLaunchRequest(
  profile: GameProfile,
  launchMethod: Exclude<LaunchMethod, ''>,
  steamClientInstallPath: string
): LaunchRequest | null {
  if (!profile.game.executable_path.trim()) {
    return null;
  }

  return {
    method: launchMethod,
    game_path: profile.game.executable_path,
    trainer_path: profile.trainer.path,
    trainer_host_path: profile.trainer.path,
    steam: {
      app_id: profile.steam.app_id,
      compatdata_path: profile.steam.compatdata_path,
      proton_path: profile.steam.proton_path,
      steam_client_install_path: steamClientInstallPath,
    },
    runtime: {
      prefix_path: profile.runtime.prefix_path,
      proton_path: profile.runtime.proton_path,
      working_directory: profile.runtime.working_directory,
    },
    optimizations: {
      enabled_option_ids:
        launchMethod === 'proton_run' ? profile.launch.optimizations.enabled_option_ids : [],
    },
    launch_trainer_only: false,
    launch_game_only: false,
  };
}

export function LaunchPage() {
  const profileState = useProfileContext();
  const profile = profileState.profile;
  const launchRequest = buildLaunchRequest(
    profile,
    profileState.launchMethod,
    profileState.steamClientInstallPath
  );
  const profileId = profileState.profileName.trim() || profileState.selectedProfile || 'new-profile';
  const showsOptimizationPanels =
    profileState.launchMethod === 'proton_run' || profileState.launchMethod === 'steam_applaunch';
  const showsSteamLaunchOptions = profileState.launchMethod === 'steam_applaunch';

  return (
    <div className="crosshook-content-area">
      <PageBanner
        eyebrow="Launch"
        title="Launch controls"
        copy="Start the selected profile through its current runtime method without the install-flow override from the old shell."
        illustration={<LaunchArt />}
      />

      <div style={{ display: 'grid', gap: 24 }}>
        <LaunchPanel profileId={profileId} method={profileState.launchMethod} request={launchRequest} />

        <section className="crosshook-launch-profile-selector">
          <span className="crosshook-heading-eyebrow">Active Profile</span>
          <ThemedSelect
            value={profileState.selectedProfile}
            onValueChange={(name) => void profileState.selectProfile(name)}
            placeholder="Select a profile"
            options={profileState.profiles.map((name) => ({ value: name, label: name }))}
          />
        </section>

        {showsOptimizationPanels ? (
          <LaunchOptimizationsPanel
            method={profileState.launchMethod}
            enabledOptionIds={profile.launch.optimizations.enabled_option_ids}
            onToggleOption={profileState.toggleLaunchOptimization}
            status={profileState.launchOptimizationsStatus}
          />
        ) : null}

        {showsSteamLaunchOptions ? (
          <SteamLaunchOptionsPanel
            enabledOptionIds={profile.launch.optimizations.enabled_option_ids}
          />
        ) : null}
      </div>
    </div>
  );
}

export default LaunchPage;
