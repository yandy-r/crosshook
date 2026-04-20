import type { GameProfile } from '../../types/profile';
import type { ProfileUpdateHandler } from './types';

interface ManualAdjustmentStepProps {
  profile: GameProfile;
  onProfileChange: ProfileUpdateHandler;
}

export function ManualAdjustmentStep({ profile, onProfileChange }: ManualAdjustmentStepProps) {
  return (
    <div className="crosshook-community-import-wizard__stack">
      <div className="crosshook-community-import-wizard__meta-grid">
        <label className="crosshook-community-import-wizard__field">
          <span className="crosshook-label">Game Executable</span>
          <input
            className="crosshook-input"
            value={profile.game.executable_path}
            onChange={(event) =>
              onProfileChange((current) => ({
                ...current,
                game: { ...current.game, executable_path: event.target.value },
              }))
            }
          />
        </label>
        <label className="crosshook-community-import-wizard__field">
          <span className="crosshook-label">Custom Cover Art</span>
          <input
            className="crosshook-input"
            value={profile.game.custom_cover_art_path ?? ''}
            placeholder="/path/to/cover.png (optional; landscape, backdrop crop)"
            onChange={(event) =>
              onProfileChange((current) => ({
                ...current,
                game: { ...current.game, custom_cover_art_path: event.target.value },
              }))
            }
          />
        </label>
        <label className="crosshook-community-import-wizard__field">
          <span className="crosshook-label">Trainer Path</span>
          <input
            className="crosshook-input"
            value={profile.trainer.path}
            onChange={(event) =>
              onProfileChange((current) => ({
                ...current,
                trainer: { ...current.trainer, path: event.target.value },
              }))
            }
          />
        </label>
        <label className="crosshook-community-import-wizard__field">
          <span className="crosshook-label">Steam App ID</span>
          <input
            className="crosshook-input"
            value={profile.steam.app_id}
            onChange={(event) =>
              onProfileChange((current) => ({
                ...current,
                steam: { ...current.steam, app_id: event.target.value },
              }))
            }
          />
        </label>
        <label className="crosshook-community-import-wizard__field">
          <span className="crosshook-label">Compatdata Path</span>
          <input
            className="crosshook-input"
            value={profile.steam.compatdata_path}
            onChange={(event) =>
              onProfileChange((current) => ({
                ...current,
                steam: { ...current.steam, compatdata_path: event.target.value },
              }))
            }
          />
        </label>
        <label className="crosshook-community-import-wizard__field">
          <span className="crosshook-label">Steam Proton Path</span>
          <input
            className="crosshook-input"
            value={profile.steam.proton_path}
            onChange={(event) =>
              onProfileChange((current) => ({
                ...current,
                steam: { ...current.steam, proton_path: event.target.value },
              }))
            }
          />
        </label>
        <label className="crosshook-community-import-wizard__field">
          <span className="crosshook-label">Runtime Prefix Path</span>
          <input
            className="crosshook-input"
            value={profile.runtime.prefix_path}
            onChange={(event) =>
              onProfileChange((current) => ({
                ...current,
                runtime: { ...current.runtime, prefix_path: event.target.value },
              }))
            }
          />
        </label>
      </div>
    </div>
  );
}

export default ManualAdjustmentStep;
