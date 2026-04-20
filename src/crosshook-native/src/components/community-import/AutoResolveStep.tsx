import type { SteamAutoPopulateResult } from './types';
import { toStatusClass } from './utils';

interface AutoResolveStepProps {
  autoPopulating: boolean;
  autoPopulateError: string | null;
  autoPopulateResult: SteamAutoPopulateResult | null;
  autoResolvedCount: number;
  canRun: boolean;
  onRunAutoPopulate: () => void;
}

export function AutoResolveStep({
  autoPopulating,
  autoPopulateError,
  autoPopulateResult,
  autoResolvedCount,
  canRun,
  onRunAutoPopulate,
}: AutoResolveStepProps) {
  return (
    <div className="crosshook-community-import-wizard__stack">
      <div className="crosshook-community-import-wizard__button-row">
        <button
          type="button"
          className="crosshook-button"
          onClick={() => onRunAutoPopulate()}
          disabled={autoPopulating || !canRun}
        >
          {autoPopulating ? 'Resolving...' : 'Re-run Auto-Resolve'}
        </button>
        <span className="crosshook-muted">Auto-resolved fields: {autoResolvedCount}</span>
      </div>
      {autoPopulateError ? <p className="crosshook-community-browser__error">{autoPopulateError}</p> : null}
      {autoPopulateResult ? (
        <div className="crosshook-community-import-wizard__status-grid">
          <div
            className={`crosshook-community-import-wizard__status crosshook-community-import-wizard__status--${toStatusClass(autoPopulateResult.app_id_state)}`}
          >
            App ID: {autoPopulateResult.app_id_state}
          </div>
          <div
            className={`crosshook-community-import-wizard__status crosshook-community-import-wizard__status--${toStatusClass(autoPopulateResult.compatdata_state)}`}
          >
            Compatdata: {autoPopulateResult.compatdata_state}
          </div>
          <div
            className={`crosshook-community-import-wizard__status crosshook-community-import-wizard__status--${toStatusClass(autoPopulateResult.proton_state)}`}
          >
            Proton: {autoPopulateResult.proton_state}
          </div>
        </div>
      ) : (
        <p className="crosshook-muted">Run auto-resolve to detect Steam metadata from the game executable.</p>
      )}
      {autoPopulateResult?.manual_hints?.length ? (
        <div className="crosshook-community-import-wizard__card">
          <div className="crosshook-community-import-wizard__label">Manual hints</div>
          {autoPopulateResult.manual_hints.map((hint) => (
            <div key={hint} className="crosshook-muted">
              {hint}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}

export default AutoResolveStep;
