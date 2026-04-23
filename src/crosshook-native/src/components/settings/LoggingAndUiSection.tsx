import type { AppSettingsData } from '../../types';
import { CollapsibleSection } from '../ui/CollapsibleSection';

interface LoggingAndUiSectionProps {
  settings: AppSettingsData;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
}

/** Collapsible section for log level, console drawer default, and recent files limit. */
export function LoggingAndUiSection({ settings, onPersistSettings }: LoggingAndUiSectionProps) {
  return (
    <CollapsibleSection
      title="Logging and UI"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">settings.toml</span>}
    >
      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="log-filter">
          Log detail level
        </label>
        <select
          id="log-filter"
          className="crosshook-input"
          value={settings.log_filter}
          onChange={(event) => {
            const v = event.target.value;
            if (v !== settings.log_filter) {
              void onPersistSettings({ log_filter: v });
            }
          }}
        >
          <option value="error">Error — critical issues only</option>
          <option value="warn">Warning — errors and warnings</option>
          <option value="info">Info — general activity (default)</option>
          <option value="debug">Debug — detailed diagnostics</option>
          <option value="trace">Trace — everything (verbose)</option>
        </select>
      </div>
      <p className="crosshook-muted crosshook-settings-note">
        Controls how much detail appears in the backend logs. Higher levels include more output and may affect
        performance. Restart the app after changing.
      </p>
      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={!settings.console_drawer_collapsed_default}
          onChange={(event) => void onPersistSettings({ console_drawer_collapsed_default: !event.target.checked })}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Start with console drawer expanded</span>
          <p className="crosshook-muted crosshook-settings-note">
            Applies on desktop and ultrawide layouts when the shell is tall enough. Narrow, deck, or short-height shells
            (window height ≤ 720px) use the compact status bar instead—AppShell switches to status mode below that
            threshold.
          </p>
        </span>
      </label>
      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={settings.high_contrast}
          onChange={(event) => void onPersistSettings({ high_contrast: event.target.checked })}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Enable high-contrast theme</span>
          <p className="crosshook-muted crosshook-settings-note">
            Increases foreground/background contrast, focus rings, and outlines for better visibility and screen reader
            cues.
          </p>
        </span>
      </label>
      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="recent-files-limit">
          Recent files limit (per list)
        </label>
        <input
          id="recent-files-limit"
          type="number"
          min={1}
          max={100}
          className="crosshook-input"
          style={{ maxWidth: 120 }}
          defaultValue={settings.recent_files_limit}
          key={`rfl-${settings.recent_files_limit}`}
          onBlur={(event) => {
            const raw = parseInt(event.target.value, 10);
            if (!Number.isFinite(raw)) return;
            const v = Math.min(100, Math.max(1, raw));
            if (v !== settings.recent_files_limit) {
              void onPersistSettings({ recent_files_limit: v });
            }
          }}
        />
      </div>
    </CollapsibleSection>
  );
}
