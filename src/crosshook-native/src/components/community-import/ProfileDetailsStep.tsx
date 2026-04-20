import type { CommunityImportPreview } from '../../hooks/useCommunityProfiles';
import type { GameProfile, LaunchMethod } from '../../types/profile';

interface ProfileDetailsStepProps {
  draft: CommunityImportPreview;
  profile: GameProfile;
  profileName: string;
  launchMethod: Exclude<LaunchMethod, ''>;
  onProfileNameChange: (value: string) => void;
}

export function ProfileDetailsStep({
  draft,
  profile,
  profileName,
  launchMethod,
  onProfileNameChange,
}: ProfileDetailsStepProps) {
  return (
    <div className="crosshook-community-import-wizard__stack">
      <div className="crosshook-community-import-wizard__card">
        <div className="crosshook-community-import-wizard__label">Source</div>
        <div className="crosshook-community-import-wizard__mono">{draft.source_path}</div>
      </div>
      <div className="crosshook-community-import-wizard__meta-grid">
        <div className="crosshook-community-import-wizard__card">
          <div className="crosshook-community-import-wizard__label">Game</div>
          <div>{draft.manifest.metadata.game_name || profile.game.name || 'Unknown'}</div>
        </div>
        <div className="crosshook-community-import-wizard__card">
          <div className="crosshook-community-import-wizard__label">Trainer Type</div>
          <div>{profile.trainer.type || 'Unknown'}</div>
        </div>
        <div className="crosshook-community-import-wizard__card">
          <div className="crosshook-community-import-wizard__label">Launch Method</div>
          <div>{launchMethod}</div>
        </div>
        <div className="crosshook-community-import-wizard__card">
          <div className="crosshook-community-import-wizard__label">Optimizations</div>
          <div>{profile.launch.optimizations.enabled_option_ids.length}</div>
        </div>
      </div>
      <label className="crosshook-community-import-wizard__field">
        <span className="crosshook-label">Local Profile Name</span>
        <input
          className="crosshook-input"
          value={profileName}
          onChange={(event) => onProfileNameChange(event.target.value)}
          placeholder="community-profile"
        />
      </label>
      {draft.required_prefix_deps.length > 0 ? (
        <div className="crosshook-prefix-deps-trust" role="alert">
          <h4 className="crosshook-prefix-deps-trust__title">Required Prefix Dependencies</h4>
          <p className="crosshook-help-text">
            This community profile requires the following packages to be installed into your WINE prefix. Only import
            profiles from sources you trust.
          </p>
          <ul className="crosshook-prefix-deps-trust__list">
            {draft.required_prefix_deps.map((dep) => (
              <li key={dep}>
                <code>{dep}</code>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

export default ProfileDetailsStep;
