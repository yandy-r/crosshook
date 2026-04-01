import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type SteamFieldState = 'Idle' | 'Saved' | 'NotFound' | 'Found' | 'Ambiguous';

interface SteamAutoPopulateRequest {
  game_path: string;
  steam_client_install_path: string;
}

interface SteamAutoPopulateResult {
  app_id_state: SteamFieldState;
  app_id: string;
  compatdata_state: SteamFieldState;
  compatdata_path: string;
  proton_state: SteamFieldState;
  proton_path: string;
  diagnostics: string[];
  manual_hints: string[];
}

interface AutoPopulateProps {
  gamePath: string;
  steamClientInstallPath: string;
  currentAppId: string;
  currentCompatdataPath: string;
  currentProtonPath: string;
  onApplyAppId: (value: string) => void;
  onApplyCompatdataPath: (value: string) => void;
  onApplyProtonPath: (value: string) => void;
}

interface FieldCardProps {
  label: string;
  state: SteamFieldState;
  currentValue: string;
  proposedValue: string;
  onApply: (() => void) | null;
}

const stateStyles: Record<SteamFieldState, { label: string }> = {
  Idle: {
    label: 'Not Scanned',
  },
  Saved: {
    label: 'Saved',
  },
  Found: {
    label: 'Found',
  },
  Ambiguous: {
    label: 'Ambiguous',
  },
  NotFound: {
    label: 'Not Found',
  },
};

function getStateVariant(state: SteamFieldState): string {
  switch (state) {
    case 'Idle':
      return 'idle';
    case 'Saved':
      return 'saved';
    case 'Found':
      return 'found';
    case 'Ambiguous':
      return 'ambiguous';
    case 'NotFound':
      return 'not-found';
  }

  throw new Error(`Unsupported Steam field state: ${state}`);
}

function FieldCard({ label, state, currentValue, proposedValue, onApply }: FieldCardProps) {
  const styles = stateStyles[state];
  const stateVariant = getStateVariant(state);
  const hasProposedValue = proposedValue.trim().length > 0;
  const showApply =
    state === 'Found' && onApply !== null && hasProposedValue && proposedValue.trim() !== currentValue.trim();

  return (
    <div className={`crosshook-auto-populate__field-card crosshook-auto-populate__field-card--${stateVariant}`}>
      <div className="crosshook-auto-populate__field-header">
        <div className="crosshook-auto-populate__field-heading">
          <div className="crosshook-auto-populate__field-label">{label}</div>
          <div className={`crosshook-auto-populate__field-state crosshook-auto-populate__field-state--${stateVariant}`}>
            {styles.label}
          </div>
        </div>
        {showApply ? (
          <button
            type="button"
            className="crosshook-auto-populate__button crosshook-auto-populate__button--subtle"
            onClick={onApply}
          >
            Apply
          </button>
        ) : null}
      </div>

      <div className="crosshook-auto-populate__field-values">
        <div className="crosshook-auto-populate__field-value">
          <strong className="crosshook-auto-populate__field-value-label">Current:</strong>{' '}
          {currentValue.trim().length > 0 ? currentValue : 'unset'}
        </div>
        <div className="crosshook-auto-populate__field-value">
          <strong className="crosshook-auto-populate__field-value-label">Proposed:</strong>{' '}
          {hasProposedValue ? proposedValue : 'none'}
        </div>
      </div>
    </div>
  );
}

export function AutoPopulate({
  gamePath,
  steamClientInstallPath,
  currentAppId,
  currentCompatdataPath,
  currentProtonPath,
  onApplyAppId,
  onApplyCompatdataPath,
  onApplyProtonPath,
}: AutoPopulateProps) {
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<SteamAutoPopulateResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function runAutoPopulate() {
    setLoading(true);
    setError(null);

    try {
      const response = await invoke<SteamAutoPopulateResult>('auto_populate_steam', {
        request: {
          game_path: gamePath,
          steam_client_install_path: steamClientInstallPath,
        } satisfies SteamAutoPopulateRequest,
      });

      setResult(response);
    } catch (invokeError) {
      setError(invokeError instanceof Error ? invokeError.message : String(invokeError));
      setResult(null);
    } finally {
      setLoading(false);
    }
  }

  const appIdState = result?.app_id_state ?? (currentAppId.trim().length > 0 ? 'Saved' : 'Idle');
  const compatdataState = result?.compatdata_state ?? (currentCompatdataPath.trim().length > 0 ? 'Saved' : 'Idle');
  const protonState = result?.proton_state ?? (currentProtonPath.trim().length > 0 ? 'Saved' : 'Idle');

  return (
    <section className="crosshook-auto-populate" aria-label="Auto-populate Steam values">
      <div className="crosshook-auto-populate__header">
        <div className="crosshook-auto-populate__heading">
          <h2 className="crosshook-auto-populate__title">Auto-Populate Steam</h2>
          <p className="crosshook-auto-populate__copy">
            Scan the selected game and Steam install to fill App ID, prefix path, and Proton values.
          </p>
        </div>

        <button
          type="button"
          className="crosshook-auto-populate__button"
          onClick={() => void runAutoPopulate()}
          disabled={loading || gamePath.trim().length === 0}
        >
          {loading ? 'Scanning...' : 'Auto-Populate'}
        </button>
      </div>

      <div className="crosshook-auto-populate__field-grid">
        <FieldCard
          label="Steam App ID"
          state={appIdState}
          currentValue={currentAppId}
          proposedValue={result?.app_id ?? ''}
          onApply={
            result?.app_id_state === 'Found' && result.app_id.trim().length > 0
              ? () => onApplyAppId(result.app_id)
              : null
          }
        />
        <FieldCard
          label="Prefix Path"
          state={compatdataState}
          currentValue={currentCompatdataPath}
          proposedValue={result?.compatdata_path ?? ''}
          onApply={
            result?.compatdata_state === 'Found' && result.compatdata_path.trim().length > 0
              ? () => onApplyCompatdataPath(result.compatdata_path)
              : null
          }
        />
        <FieldCard
          label="Proton Path"
          state={protonState}
          currentValue={currentProtonPath}
          proposedValue={result?.proton_path ?? ''}
          onApply={
            result?.proton_state === 'Found' && result.proton_path.trim().length > 0
              ? () => onApplyProtonPath(result.proton_path)
              : null
          }
        />
      </div>

      {error ? (
        <div className="crosshook-auto-populate__error" role="alert">
          {error}
        </div>
      ) : null}

      <div className="crosshook-auto-populate__info-grid">
        <section className="crosshook-auto-populate__info-card">
          <h3 className="crosshook-auto-populate__info-title">Diagnostics</h3>
          <div className="crosshook-auto-populate__info-body">
            {loading ? (
              <div className="crosshook-auto-populate__info-empty">Waiting for Steam discovery output...</div>
            ) : result?.diagnostics?.length ? (
              result.diagnostics.map((entry, index) => (
                <div key={`${index}-${entry}`} className="crosshook-auto-populate__info-entry">
                  {entry}
                </div>
              ))
            ) : (
              <div className="crosshook-auto-populate__info-empty">Run auto-populate to see discovery steps.</div>
            )}
          </div>
        </section>

        <section className="crosshook-auto-populate__info-card">
          <h3 className="crosshook-auto-populate__info-title">Manual Hints</h3>
          <div className="crosshook-auto-populate__info-body">
            {result?.manual_hints?.length ? (
              result.manual_hints.map((entry, index) => (
                <div key={`${index}-${entry}`} className="crosshook-auto-populate__info-entry">
                  {entry}
                </div>
              ))
            ) : (
              <div className="crosshook-auto-populate__info-empty">
                Hints appear here when discovery is incomplete or ambiguous.
              </div>
            )}
          </div>
        </section>
      </div>
    </section>
  );
}

export default AutoPopulate;
