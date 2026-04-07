import { useCallback, useId, useMemo } from 'react';

import type { BundledOptimizationPreset } from '../../types';
import {
  BUNDLED_PRESET_KEY_PREFIX,
  bundledOptimizationTomlKey,
} from '../../utils/launchOptimizationPresets';
import { ThemedSelect, type SelectOptionGroup } from '../ui/ThemedSelect';

export interface WizardPresetPickerProps {
  bundledPresets: readonly BundledOptimizationPreset[];
  savedPresetNames: readonly string[];
  activePresetKey: string;
  busy: boolean;
  /** When true, the picker is disabled with an explanatory hint (e.g. create
   * mode, where the profile is not yet persisted so preset apply IPCs no-op). */
  unavailableReason?: string;
  onApplyBundled: (presetId: string) => Promise<void>;
  onSelectSaved: (presetName: string) => Promise<void>;
}

/**
 * Slim launch preset picker for the Review step of the onboarding wizard.
 *
 * Exposes a single `ThemedSelect` grouped into `Built-in` and `Saved` sections,
 * dispatching the corresponding IPC through the caller-supplied handlers. This
 * mirrors the preset row pattern in `LaunchOptimizationsPanel.tsx` without any
 * of the per-option toggles — "choose/apply preset", not the full panel.
 */
export function WizardPresetPicker({
  bundledPresets,
  savedPresetNames,
  activePresetKey,
  busy,
  unavailableReason,
  onApplyBundled,
  onSelectSaved,
}: WizardPresetPickerProps) {
  const selectId = useId();
  const helpId = useId();

  const savedOptions = useMemo(
    () =>
      savedPresetNames
        .filter((name) => !name.startsWith(BUNDLED_PRESET_KEY_PREFIX))
        .map((name) => ({ value: name, label: name })),
    [savedPresetNames]
  );

  const groups = useMemo((): SelectOptionGroup[] => {
    const next: SelectOptionGroup[] = [];
    if (bundledPresets.length > 0) {
      next.push({
        label: 'Built-in',
        options: bundledPresets.map((preset) => ({
          value: bundledOptimizationTomlKey(preset.preset_id),
          label: preset.display_name,
          badge: 'Built-in',
        })),
      });
    }
    if (savedOptions.length > 0) {
      next.push({ label: 'Saved', options: savedOptions });
    }
    return next;
  }, [bundledPresets, savedOptions]);

  const handleChange = useCallback(
    (value: string) => {
      if (busy || unavailableReason) return;

      if (value.startsWith(BUNDLED_PRESET_KEY_PREFIX)) {
        const presetId = value.slice(BUNDLED_PRESET_KEY_PREFIX.length);
        const isCatalogBundled = bundledPresets.some((preset) => preset.preset_id === presetId);
        if (isCatalogBundled) {
          void onApplyBundled(presetId);
          return;
        }
      }
      void onSelectSaved(value);
    },
    [busy, unavailableReason, bundledPresets, onApplyBundled, onSelectSaved]
  );

  const hasAnyPreset = groups.length > 0;
  const disabled = busy || !hasAnyPreset || Boolean(unavailableReason);

  const activeValue = useMemo(() => {
    const active = activePresetKey.trim();
    if (!active) return '';
    const inBundled = bundledPresets.some(
      (preset) => bundledOptimizationTomlKey(preset.preset_id) === active
    );
    const inSaved = savedOptions.some((opt) => opt.value === active);
    return inBundled || inSaved ? active : '';
  }, [activePresetKey, bundledPresets, savedOptions]);

  const helpText = unavailableReason
    ? unavailableReason
    : hasAnyPreset
      ? 'Optional. Apply a built-in or saved launch optimization preset before saving the profile. You can change this later from the Launch page.'
      : 'No launch optimization presets are available. You can configure presets later from the Launch page.';

  return (
    <div className="crosshook-install-section">
      <div className="crosshook-install-section-title">Launch Preset</div>
      <div className="crosshook-field">
        <label className="crosshook-label" htmlFor={selectId}>
          Preset
        </label>
        {disabled ? (
          <ThemedSelect
            id={selectId}
            value={activeValue}
            onValueChange={() => {}}
            options={[]}
            ariaLabelledby={helpId}
          />
        ) : (
          <ThemedSelect
            id={selectId}
            value={activeValue}
            onValueChange={handleChange}
            groups={groups}
            ariaLabelledby={helpId}
          />
        )}
        <p id={helpId} className="crosshook-help-text">
          {helpText}
        </p>
      </div>
    </div>
  );
}

export default WizardPresetPicker;
