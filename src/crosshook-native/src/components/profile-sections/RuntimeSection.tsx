import type { ReactNode } from 'react';

import AutoPopulate from '../AutoPopulate';
import { FieldRow, LauncherMetadataFields, OptionalSection, ProtonPathField } from '../ProfileFormSections';
import type { ProtonInstallOption } from '../ProfileFormSections';
import { chooseDirectory, chooseFile } from '../../utils/dialog';
import { deriveSteamClientInstallPath } from '../../utils/steam';
import { validateSteamAppId } from '../../utils/art';
import type { GameProfile, LaunchMethod } from '../../types';

export interface RuntimeSectionProps {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  reviewMode?: boolean;
  launchMethod: LaunchMethod;
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
  /**
   * Optional ProtonDB panel node rendered at the end of steam_applaunch and proton_run sections.
   * Kept in ProfileFormSections and passed here to avoid lifting ProtonDB state into RuntimeSection.
   */
  protonDbPanel?: ReactNode;
}

/**
 * Renders the runner-method-conditional runtime fields.
 * - steam_applaunch: Steam App ID, Prefix Path, Proton Path, AutoPopulate, ProtonDB panel
 * - proton_run: Prefix Path, Steam App ID, Working Dir, Proton Path, ProtonDB panel
 * - native: Working Directory override
 */
export function RuntimeSection({
  profile,
  onUpdateProfile,
  reviewMode = false,
  launchMethod,
  protonInstalls,
  protonInstallsError,
  protonDbPanel,
}: RuntimeSectionProps) {
  const supportsTrainerLaunch = launchMethod !== 'native';
  const showLauncherMetadata = supportsTrainerLaunch && !reviewMode;
  const workingDirectoryCollapsed = reviewMode && profile.runtime.working_directory.trim().length === 0;
  const steamClientInstallPath = deriveSteamClientInstallPath(profile.steam.compatdata_path);

  const runtimeTitle =
    launchMethod === 'steam_applaunch'
      ? 'Steam Runtime'
      : launchMethod === 'proton_run'
        ? 'Proton Runtime'
        : 'Native Runtime';

  return (
    <div className="crosshook-install-section">
      <div className="crosshook-install-section-title">{runtimeTitle}</div>

      {launchMethod === 'steam_applaunch' ? (
        <>
          <div className="crosshook-install-grid">
            <FieldRow
              label="Steam App ID"
              value={profile.steam.app_id}
              onChange={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, app_id: value },
                }))
              }
              placeholder="1245620"
            />

            <FieldRow
              label="Prefix Path"
              value={profile.steam.compatdata_path}
              onChange={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, compatdata_path: value },
                }))
              }
              placeholder="/home/user/.local/share/Steam/steamapps/compatdata/1245620"
              browseLabel="Browse"
              onBrowse={async () => {
                const path = await chooseDirectory('Select Steam Prefix Directory');

                if (path) {
                  onUpdateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, compatdata_path: path },
                  }));
                }
              }}
            />

            {showLauncherMetadata ? (
              <LauncherMetadataFields profile={profile} onUpdateProfile={onUpdateProfile} />
            ) : null}
          </div>

          <ProtonPathField
            label="Proton Path"
            value={profile.steam.proton_path}
            onChange={(value) =>
              onUpdateProfile((current) => ({
                ...current,
                steam: { ...current.steam, proton_path: value },
              }))
            }
            placeholder="/home/user/.steam/root/steamapps/common/Proton - Experimental/proton"
            installs={protonInstalls}
            error={null}
            installsError={protonInstallsError}
            onBrowse={async () => {
              const path = await chooseFile('Select Proton Executable');

              if (path) {
                onUpdateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, proton_path: path },
                }));
              }
            }}
          />

          <div style={{ display: 'grid', gap: 16, marginTop: 18 }}>
            <AutoPopulate
              gamePath={profile.game.executable_path}
              steamClientInstallPath={steamClientInstallPath}
              currentAppId={profile.steam.app_id}
              currentCompatdataPath={profile.steam.compatdata_path}
              currentProtonPath={profile.steam.proton_path}
              onApplyAppId={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, app_id: value },
                }))
              }
              onApplyCompatdataPath={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, compatdata_path: value },
                }))
              }
              onApplyProtonPath={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, proton_path: value },
                }))
              }
            />

            {protonDbPanel}
          </div>
        </>
      ) : null}

      {launchMethod === 'proton_run' ? (
        <>
          <div className="crosshook-install-grid">
            <FieldRow
              label="Prefix Path"
              value={profile.runtime.prefix_path}
              onChange={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  runtime: { ...current.runtime, prefix_path: value },
                }))
              }
              placeholder="/path/to/prefix"
              browseLabel="Browse"
              onBrowse={async () => {
                const path = await chooseDirectory('Select Proton Prefix Directory');

                if (path) {
                  onUpdateProfile((current) => ({
                    ...current,
                    runtime: { ...current.runtime, prefix_path: path },
                  }));
                }
              }}
            />

            <FieldRow
              label="Steam App ID"
              value={profile.runtime?.steam_app_id ?? ''}
              onChange={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  runtime: { ...current.runtime, steam_app_id: value },
                }))
              }
              placeholder="Optional — used for art and metadata lookup"
              error={
                !validateSteamAppId(profile.runtime?.steam_app_id ?? '')
                  ? 'App ID must contain digits only'
                  : null
              }
            />

            {showLauncherMetadata ? (
              <LauncherMetadataFields profile={profile} onUpdateProfile={onUpdateProfile} />
            ) : null}

            <OptionalSection summary="Working directory override" collapsed={workingDirectoryCollapsed}>
              <FieldRow
                label="Working Directory"
                value={profile.runtime.working_directory}
                onChange={(value) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    runtime: { ...current.runtime, working_directory: value },
                  }))
                }
                placeholder="Optional override"
                browseLabel="Browse"
                onBrowse={async () => {
                  const path = await chooseDirectory('Select Working Directory');

                  if (path) {
                    onUpdateProfile((current) => ({
                      ...current,
                      runtime: { ...current.runtime, working_directory: path },
                    }));
                  }
                }}
              />
            </OptionalSection>
          </div>

          <ProtonPathField
            label="Proton Path"
            value={profile.runtime.proton_path}
            onChange={(value) =>
              onUpdateProfile((current) => ({
                ...current,
                runtime: { ...current.runtime, proton_path: value },
              }))
            }
            placeholder="/path/to/proton"
            installs={protonInstalls}
            error={null}
            installsError={protonInstallsError}
            onBrowse={async () => {
              const path = await chooseFile('Select Proton Executable');

              if (path) {
                onUpdateProfile((current) => ({
                  ...current,
                  runtime: { ...current.runtime, proton_path: path },
                }));
              }
            }}
          />

          {protonDbPanel}
        </>
      ) : null}

      {launchMethod === 'native' ? (
        <OptionalSection summary="Working directory override" collapsed={workingDirectoryCollapsed}>
          <div className="crosshook-install-grid" style={{ marginTop: 16 }}>
            <FieldRow
              label="Working Directory"
              value={profile.runtime.working_directory}
              onChange={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  runtime: { ...current.runtime, working_directory: value },
                }))
              }
              placeholder="Optional override"
              browseLabel="Browse"
              onBrowse={async () => {
                const path = await chooseDirectory('Select Working Directory');

                if (path) {
                  onUpdateProfile((current) => ({
                    ...current,
                    runtime: { ...current.runtime, working_directory: path },
                  }));
                }
              }}
            />
          </div>
        </OptionalSection>
      ) : null}
    </div>
  );
}

export default RuntimeSection;
