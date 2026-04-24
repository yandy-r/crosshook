import type { GameProfile } from '../../types';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { ThemedSelectField } from '../ui/ThemedSelectField';

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
  const options = [
    { value: 'steam_applaunch', label: 'Steam app launch' },
    { value: 'proton_run', label: 'Proton runtime launch' },
    ...(hideNative ? [] : [{ value: 'native' as const, label: 'Native Linux launch' }]),
  ];

  const currentLabel = options.find((o) => o.value === profile.launch.method)?.label ?? profile.launch.method;

  return (
    <DashboardPanelSection
      titleAs="h3"
      eyebrow="Profile"
      title="Runner Method"
      actions={
        <div className="crosshook-dashboard-pill-row">
          <span className="crosshook-dashboard-pill">{currentLabel}</span>
        </div>
      }
    >
      <div className="crosshook-field">
        <ThemedSelectField
          label="Runner Method"
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
    </DashboardPanelSection>
  );
}

export default RunnerMethodSection;
