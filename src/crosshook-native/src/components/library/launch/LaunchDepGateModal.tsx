import { useLaunchStateContext } from '@/context/LaunchStateContext';
import type { GameProfile } from '@/types/profile';
import { resolveEffectivePrefixPath } from '@/utils/prefixPath';
import type { DepGateState } from './useLaunchDepGate';

interface LaunchDepGateModalProps {
  depGate: DepGateState;
  profile: GameProfile;
  selectedName: string;
}

export function LaunchDepGateModal({ depGate, profile, selectedName }: LaunchDepGateModalProps) {
  const { launchGame, launchTrainer } = useLaunchStateContext();

  const depGatePackages = depGate.depGatePackages;
  const repair = depGate.depGateRepair;

  if (repair?.required) {
    return (
      <div
        className="crosshook-modal-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dep-gate-repair-title"
        aria-busy={depGate.depGateRepairing}
      >
        <div className="crosshook-modal crosshook-prefix-deps__confirm crosshook-panel">
          <h3 id="dep-gate-repair-title">Prefix Repair Required</h3>
          <p>
            CrossHook must restore this prefix&apos;s Windows compatibility version before the game or trainer can
            launch.
          </p>
          {repair.last_error ? <p className="crosshook-danger">Last repair attempt: {repair.last_error}</p> : null}
          {depGate.depGateRepairing ? (
            <p className="crosshook-muted" role="status" aria-live="polite">
              Repairing prefix...
            </p>
          ) : null}
          <div className="crosshook-modal__actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--danger"
              disabled={depGate.depGateRepairing}
              onClick={() => {
                void depGate.repairPrefixVersion();
              }}
            >
              {depGate.depGateRepairing ? 'Repairing...' : 'Repair + Launch'}
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              disabled={depGate.depGateRepairing}
              onClick={() => {
                depGate.setDepGateRepair(null);
                depGate.setDepGatePendingAction(null);
                depGate.setDepGateRepairing(false);
              }}
            >
              Cancel
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (depGatePackages === null) {
    return null;
  }

  return (
    <div
      className="crosshook-modal-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="dep-gate-title"
      aria-busy={depGate.depGateInstalling || depGate.depGateVerifying}
    >
      <div className="crosshook-modal crosshook-prefix-deps__confirm crosshook-panel">
        <h3 id="dep-gate-title">Missing Prefix Dependencies</h3>
        <p>
          This profile requires WINE prefix dependencies that are not installed. You can install them now or skip and
          launch anyway.
        </p>
        <ul>
          {depGatePackages.map((pkg) => (
            <li key={pkg}>
              <code>{pkg}</code>
            </li>
          ))}
        </ul>
        {depGate.depGateVerifying ? (
          <p className="crosshook-muted" role="status" aria-live="polite">
            Verifying prefix repair...
          </p>
        ) : depGate.depGateInstalling ? (
          <p className="crosshook-muted" role="status" aria-live="polite">
            Installing dependencies...
          </p>
        ) : null}
        <div className="crosshook-modal__actions">
          <button
            type="button"
            className="crosshook-button"
            disabled={depGate.depGateInstalling}
            onClick={() => {
              void (async () => {
                const prefixPath = resolveEffectivePrefixPath(profile);
                depGate.setDepGateInstalling(true);
                try {
                  await depGate.installPrefixDependency(selectedName, prefixPath, depGatePackages);
                } catch {
                  depGate.setDepGateInstalling(false);
                }
              })();
            }}
          >
            Install + Launch
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            disabled={depGate.depGateInstalling}
            onClick={() => {
              const action = depGate.depGatePendingAction;
              depGate.setDepGatePackages(null);
              depGate.setDepGatePendingAction(null);
              if (action === 'game') {
                launchGame();
              } else if (action === 'trainer') {
                launchTrainer();
              }
            }}
          >
            Skip and Launch
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            disabled={depGate.depGateInstalling}
            onClick={() => {
              depGate.setDepGatePackages(null);
              depGate.setDepGatePendingAction(null);
              depGate.setDepGateInstalling(false);
            }}
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
