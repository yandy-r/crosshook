import type { AppSettingsData } from '../../types';
import { CollapsibleSection } from '../ui/CollapsibleSection';

interface NewProfileDefaultsSectionProps {
  settings: AppSettingsData;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
}

/** Collapsible section for default values applied when creating a new profile. */
export function NewProfileDefaultsSection({ settings, onPersistSettings }: NewProfileDefaultsSectionProps) {
  return (
    <CollapsibleSection
      title="New profile defaults"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">settings.toml</span>}
    >
      <p className="crosshook-muted crosshook-settings-help">
        Applied when you save a profile for the first time (new name). Empty fields keep CrossHook&apos;s built-in
        detection.
      </p>
      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="default-proton-path">
          Default Proton path
        </label>
        <input
          id="default-proton-path"
          key={`dp-${settings.default_proton_path}`}
          className="crosshook-input"
          defaultValue={settings.default_proton_path}
          placeholder="/path/to/proton"
          onBlur={(event) => {
            const v = event.target.value.trim();
            if (v !== settings.default_proton_path.trim()) {
              void onPersistSettings({ default_proton_path: v });
            }
          }}
        />
      </div>
      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="default-launch-method">
          Default launch method
        </label>
        <select
          id="default-launch-method"
          className="crosshook-input"
          value={settings.default_launch_method}
          onChange={(event) => void onPersistSettings({ default_launch_method: event.target.value })}
        >
          <option value="">Auto (from game / Steam)</option>
          <option value="proton_run">proton_run</option>
          <option value="steam_applaunch">steam_applaunch</option>
          <option value="native">native</option>
        </select>
      </div>
      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="default-trainer-mode">
          Default trainer loading mode
        </label>
        <select
          id="default-trainer-mode"
          className="crosshook-input"
          value={settings.default_trainer_loading_mode}
          onChange={(event) => void onPersistSettings({ default_trainer_loading_mode: event.target.value })}
        >
          <option value="source_directory">source_directory</option>
          <option value="copy_to_prefix">copy_to_prefix</option>
        </select>
      </div>
      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="default-bundled-preset">
          Default bundled optimization preset id
        </label>
        <input
          id="default-bundled-preset"
          key={`dbp-${settings.default_bundled_optimization_preset_id}`}
          className="crosshook-input"
          defaultValue={settings.default_bundled_optimization_preset_id}
          placeholder="e.g. preset id from metadata catalog"
          onBlur={(event) => {
            const v = event.target.value.trim();
            if (v !== settings.default_bundled_optimization_preset_id.trim()) {
              void onPersistSettings({ default_bundled_optimization_preset_id: v });
            }
          }}
        />
      </div>
    </CollapsibleSection>
  );
}
