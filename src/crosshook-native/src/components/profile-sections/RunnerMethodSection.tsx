import { useId } from 'react';

import type { GameProfile } from '../../types';
import { ThemedSelect } from '../ui/ThemedSelect';

export interface RunnerMethodSectionProps {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  reviewMode?: boolean;
}

/**
 * Renders the "Runner Method" section: the dropdown to select Steam AppLaunch,
 * Proton Run, or Native launch method.
 */
export function RunnerMethodSection({ profile, onUpdateProfile }: RunnerMethodSectionProps) {
  const sectionId = useId();

  return (
    <>
      <div className="crosshook-install-section-title">Runner Method</div>
      <div className="crosshook-field">
        <label className="crosshook-label" htmlFor={`${sectionId}-launch-method`}>
          Runner Method
        </label>
        <ThemedSelect
          id={`${sectionId}-launch-method`}
          value={profile.launch.method}
          onValueChange={(val) =>
            onUpdateProfile((current) => ({
              ...current,
              steam: { ...current.steam, enabled: val === 'steam_applaunch' },
              launch: {
                ...current.launch,
                method: val as typeof current.launch.method,
              },
            }))
          }
          options={[
            { value: 'steam_applaunch', label: 'Steam app launch' },
            { value: 'proton_run', label: 'Proton runtime launch' },
            { value: 'native', label: 'Native Linux launch' },
          ]}
        />
        <p className="crosshook-help-text">
          Choose the runner explicitly so CrossHook saves the correct launch method and only shows the relevant fields.
        </p>
      </div>
    </>
  );
}

export default RunnerMethodSection;
