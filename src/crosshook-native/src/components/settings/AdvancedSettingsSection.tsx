import { useUmuDatabaseRefresh } from '../../hooks/useUmuDatabaseRefresh';
import { CollapsibleSection } from '../ui/CollapsibleSection';

export function AdvancedSettingsSection() {
  const { isClearing, lastClearStatus, clearStatusId, clearGameIdLookupCache } = useUmuDatabaseRefresh();

  return (
    <CollapsibleSection
      title="Advanced"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">maintenance</span>}
    >
      <div className="crosshook-settings-field-row">
        <span className="crosshook-label">umu GAMEID lookup cache</span>
        <div>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void clearGameIdLookupCache()}
            disabled={isClearing}
            aria-describedby={clearStatusId}
          >
            {isClearing ? 'Clearing...' : 'Clear lookup cache'}
          </button>
          <div
            id={clearStatusId}
            className="crosshook-muted"
            style={{ fontSize: '0.85rem', marginTop: 4 }}
            role="status"
            aria-live="polite"
            aria-atomic="true"
          >
            {lastClearStatus ?? 'Lookup cache not cleared this session'}
          </div>
        </div>
      </div>
    </CollapsibleSection>
  );
}
