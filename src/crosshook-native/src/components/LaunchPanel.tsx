import type { LaunchMethod, LaunchRequest } from '../types';
import { LaunchPhase } from '../types';
import { useLaunchState } from '../hooks/useLaunchState';

interface LaunchPanelProps {
  profileId: string;
  method: Exclude<LaunchMethod, ''>;
  request: LaunchRequest | null;
}

export function LaunchPanel({ profileId, method, request }: LaunchPanelProps) {
  const {
    actionLabel,
    canLaunchGame,
    canLaunchTrainer,
    errorMessage,
    helperLogPath,
    hintText,
    isBusy,
    launchGame,
    launchTrainer,
    phase,
    reset,
    statusText,
  } = useLaunchState({
    profileId,
    method,
    request,
  });

  const isWaitingForTrainer = phase === LaunchPhase.WaitingForTrainer;
  const isSessionActive = phase === LaunchPhase.SessionActive;
  const canLaunch = isWaitingForTrainer ? canLaunchTrainer : canLaunchGame;
  const primaryAction = isWaitingForTrainer ? launchTrainer : launchGame;

  return (
    <section className="crosshook-launch-panel">
      <div className="crosshook-launch-panel__header">
        <div>
          <p className="crosshook-launch-panel__eyebrow">
            {method === 'steam_applaunch' ? 'Steam Launch' : method === 'proton_run' ? 'Proton Launch' : 'Native Launch'}
          </p>
          <h1 className="crosshook-launch-panel__title">CrossHook Native</h1>
          <p className="crosshook-launch-panel__copy">
            {method === 'native'
              ? 'Direct launch flow for Linux-native executables, driven by the native Tauri backend.'
              : `Two-step launch flow for ${method === 'steam_applaunch' ? 'Steam' : 'Proton'} games and trainers, driven by the native Tauri backend.`}
          </p>
        </div>

        <div className="crosshook-launch-panel__status" data-phase={phase}>
          {phase}
        </div>
      </div>

      <div className="crosshook-launch-panel__info">
        <p className="crosshook-launch-panel__status-text">{statusText}</p>
        <p className="crosshook-launch-panel__hint">{hintText}</p>
        {helperLogPath ? (
          <p className="crosshook-launch-panel__helper-log">Helper log: {helperLogPath}</p>
        ) : null}
        {errorMessage ? <p className="crosshook-launch-panel__error">{errorMessage}</p> : null}
      </div>

      <div className="crosshook-launch-panel__actions">
        <button
          type="button"
          className="crosshook-button crosshook-launch-panel__action"
          onClick={primaryAction}
          disabled={!canLaunch || isBusy}
        >
          {actionLabel}
        </button>

        <button
          type="button"
          className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
          onClick={reset}
        >
          Reset
        </button>
      </div>

      <div
        className="crosshook-launch-panel__indicator"
        data-state={isSessionActive ? 'active' : isWaitingForTrainer ? 'waiting' : 'idle'}
      >
        <div className="crosshook-launch-panel__indicator-row">
          <span className="crosshook-launch-panel__indicator-dot" aria-hidden="true" />
          <span className="crosshook-launch-panel__indicator-label">
            {method === 'steam_applaunch'
              ? 'Steam runner selected'
              : method === 'proton_run'
                ? 'Proton runner selected'
                : 'Native runner selected'}
          </span>
        </div>
        <div className="crosshook-launch-panel__indicator-copy">
          {request ? 'Profile request is loaded.' : 'No profile request is loaded yet.'}
        </div>
      </div>
    </section>
  );
}

export default LaunchPanel;
