import { useUmuDatabaseRefresh } from '../../hooks/useUmuDatabaseRefresh';
import type { AppSettingsData, UmuDatabaseLookupPreference, UmuPreference } from '../../types';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { ThemedSelectField } from '../ui/ThemedSelectField';
import { formatTimestamp } from './format';

interface RunnerSectionProps {
  settings: AppSettingsData;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
}

/** Collapsible section for the global runner preference and umu protonfix database refresh. */
export function RunnerSection({ settings, onPersistSettings }: RunnerSectionProps) {
  const { isRefreshing, lastRefreshStatus, refresh: onRefreshUmuDatabase, refreshStatusId } = useUmuDatabaseRefresh();

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
        <ThemedSelectField
          label="Runner (global default)"
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
        <ThemedSelectField
          label="umu GAMEID lookup"
          value={settings.umu_database_lookup}
          onValueChange={(value) =>
            void onPersistSettings({ umu_database_lookup: value as UmuDatabaseLookupPreference })
          }
          options={[
            { value: 'disabled', label: 'Disabled' },
            { value: 'enabled', label: 'Enabled' },
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
            aria-describedby={refreshStatusId}
          >
            {isRefreshing ? 'Refreshing…' : 'Refresh umu protonfix database'}
          </button>
          <div
            id={refreshStatusId}
            className="crosshook-muted"
            style={{ fontSize: '0.85rem', marginTop: 4 }}
            role="status"
            aria-live="polite"
            aria-atomic="true"
          >
            {lastRefreshStatus?.cached_at
              ? `Last refreshed: ${formatTimestamp(lastRefreshStatus.cached_at)}`
              : lastRefreshStatus
                ? `Status: ${lastRefreshStatus.reason}`
                : 'Not refreshed this session'}
          </div>
        </div>
      </div>
    </CollapsibleSection>
  );
}
