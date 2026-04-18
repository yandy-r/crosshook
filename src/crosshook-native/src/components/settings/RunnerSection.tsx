import { useUmuDatabaseRefresh } from '../../hooks/useUmuDatabaseRefresh';
import type { AppSettingsData, UmuPreference } from '../../types';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { ThemedSelect } from '../ui/ThemedSelect';

interface RunnerSectionProps {
  settings: AppSettingsData;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
}

/** Collapsible section for the global runner preference and umu protonfix database refresh. */
export function RunnerSection({ settings, onPersistSettings }: RunnerSectionProps) {
  const { isRefreshing, lastRefreshStatus, refresh: onRefreshUmuDatabase } = useUmuDatabaseRefresh();

  return (
    <CollapsibleSection
      title="Runner"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">settings.toml</span>}
    >
      <p className="crosshook-muted crosshook-settings-help">
        Global runner applied to every launch. Individual profiles can override this in their Runtime section.
      </p>
      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="umu-preference" id="umu-preference-label">
          Runner (global default)
        </label>
        <ThemedSelect
          id="umu-preference"
          ariaLabelledby="umu-preference-label"
          value={settings.umu_preference}
          onValueChange={(value) => void onPersistSettings({ umu_preference: value as UmuPreference })}
          options={[
            { value: 'auto', label: 'Auto (umu when available, else Proton)' },
            { value: 'umu', label: 'Umu (umu-launcher)' },
            { value: 'proton', label: 'Proton (direct)' },
          ]}
        />
      </div>
      <div className="crosshook-settings-field-row">
        <span className="crosshook-label">umu protonfix database</span>
        <div>
          <button
            type="button"
            className="crosshook-button"
            onClick={() => void onRefreshUmuDatabase()}
            disabled={isRefreshing}
          >
            {isRefreshing ? 'Refreshing…' : 'Refresh umu protonfix database'}
          </button>
          <div className="crosshook-muted" style={{ fontSize: '0.85rem', marginTop: 4 }}>
            {lastRefreshStatus?.cached_at
              ? `Last refreshed: ${new Date(lastRefreshStatus.cached_at).toLocaleString()}`
              : lastRefreshStatus
                ? `Status: ${lastRefreshStatus.reason}`
                : 'Not refreshed this session'}
          </div>
        </div>
      </div>
    </CollapsibleSection>
  );
}
