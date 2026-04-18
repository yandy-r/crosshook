import type { AppSettingsData } from '../../types';
import { CollapsibleSection } from '../ui/CollapsibleSection';

interface StartupSectionProps {
  settings: AppSettingsData;
  onAutoLoadLastProfileChange: (enabled: boolean) => void;
}

/** Collapsible section for startup behaviour (auto-load last profile). */
export function StartupSection({ settings, onAutoLoadLastProfileChange }: StartupSectionProps) {
  return (
    <CollapsibleSection
      title="Startup"
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">Controlled by settings.toml</span>}
    >
      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={settings.auto_load_last_profile}
          onChange={(event) => onAutoLoadLastProfileChange(event.target.checked)}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Auto-load last profile</span>
          <p className="crosshook-muted crosshook-settings-note">
            When enabled, CrossHook should reopen the most recently used profile on startup if it still exists.
          </p>
        </span>
      </label>

      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="last-used-profile">
          Last used profile
        </label>
        <input
          id="last-used-profile"
          className="crosshook-input"
          value={settings.last_used_profile}
          readOnly
          placeholder="No profile selected"
        />
      </div>
    </CollapsibleSection>
  );
}
