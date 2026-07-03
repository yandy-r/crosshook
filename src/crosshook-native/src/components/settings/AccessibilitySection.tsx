import { useReducedMotion, useResolvedHighContrast } from '@/hooks/useAccessibilityPreferences';
import type { AppSettingsData, HighContrastPreference, ReducedMotionPreference } from '../../types';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { ThemedSelectField } from '../ui/ThemedSelectField';

interface AccessibilitySectionProps {
  settings: AppSettingsData;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
}

interface PreferenceRowProps {
  label: string;
  value: string;
  options: { value: string; label: string }[];
  note: string;
  onChange: (value: string) => void;
}

function PreferenceRow({ label, value, options, note, onChange }: PreferenceRowProps) {
  return (
    <>
      <div className="crosshook-settings-field-row">
        <ThemedSelectField
          label={label}
          value={value}
          onValueChange={(next) => {
            if (next !== value) {
              onChange(next);
            }
          }}
          options={options}
        />
      </div>
      <p className="crosshook-muted crosshook-settings-note">{note}</p>
    </>
  );
}

/** Collapsible section for the tri-state high-contrast and reduced-motion preferences. */
export function AccessibilitySection({ settings, onPersistSettings }: AccessibilitySectionProps) {
  const contrastOn = useResolvedHighContrast(settings.high_contrast);
  const motion = useReducedMotion(settings.reduced_motion);

  return (
    <CollapsibleSection
      title="Accessibility"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">settings.toml</span>}
    >
      <PreferenceRow
        label="High contrast"
        value={settings.high_contrast}
        options={[
          { value: 'auto', label: 'Auto — follow system contrast preference' },
          { value: 'on', label: 'On — always high contrast' },
          { value: 'off', label: 'Off — never high contrast' },
        ]}
        note={`Currently: high contrast ${contrastOn ? 'on' : 'off'}${
          settings.high_contrast === 'auto' ? ' (from system preference)' : ''
        }`}
        onChange={(value) => void onPersistSettings({ high_contrast: value as HighContrastPreference })}
      />
      <PreferenceRow
        label="Motion"
        value={settings.reduced_motion}
        options={[
          { value: 'auto', label: 'Auto — follow system reduced-motion preference' },
          { value: 'reduced', label: 'Reduced — minimize animation' },
          { value: 'full', label: 'Full — always animate' },
        ]}
        note={`Currently: ${motion} motion${settings.reduced_motion === 'auto' ? ' (from system preference)' : ''}`}
        onChange={(value) => void onPersistSettings({ reduced_motion: value as ReducedMotionPreference })}
      />
    </CollapsibleSection>
  );
}
