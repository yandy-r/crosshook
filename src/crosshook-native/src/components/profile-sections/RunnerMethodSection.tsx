import { useId } from 'react';

import type { GameProfile } from '../../types';
import { ThemedSelect } from '../ui/ThemedSelect';

export interface RunnerMethodSectionProps {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  reviewMode?: boolean;
  /** When true, omits Native Linux — used where Proton installer flow applies only. */
  hideNative?: boolean;
}

/**
 * Renders the "Runner Method" section: the dropdown to select Steam AppLaunch,
 * Proton Run, or Native launch method.
 */
export function RunnerMethodSection({ profile, onUpdateProfile, hideNative }: RunnerMethodSectionProps) {
  const sectionId = useId();

  const options = [
    { value: 'steam_applaunch', label: 'Steam app launch' },
    { value: 'proton_run', label: 'Proton runtime launch' },
    ...(hideNative ? [] : [{ value: 'native' as const, label: 'Native Linux launch' }]),
  ];

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
          options={options}
        />
        <p className="crosshook-help-text">
          Choose the runner explicitly so CrossHook saves the correct launch method and only shows the relevant fields.
        </p>
      </div>
    </>
  );
}

export default RunnerMethodSection;
