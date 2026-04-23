import { useLaunchStateContext } from '../../../context/LaunchStateContext';
import type { DepGateState } from './useLaunchDepGate';
import type { GameProfile } from '../../../types/profile';

interface LaunchDepGateModalProps {
  depGate: DepGateState;
  profile: GameProfile;
  selectedName: string;
}

export function LaunchDepGateModal({ depGate, profile, selectedName }: LaunchDepGateModalProps) {
  const { launchGame, launchTrainer } = useLaunchStateContext();

  if (depGate.depGatePackages === null) {
    return null;
  }

  return (
    <div className="crosshook-modal-overlay" role="dialog" aria-modal="true" aria-labelledby="dep-gate-title">
      <div className="crosshook-modal crosshook-prefix-deps__confirm">
        <h3 id="dep-gate-title">Missing Prefix Dependencies</h3>
        <p>
          This profile requires WINE prefix dependencies that are not installed. You can install them now or skip
          and launch anyway.
        </p>
        <ul>
          {depGate.depGatePackages.map((pkg) => (
            <li key={pkg}>
              <code>{pkg}</code>
            </li>
          ))}
        </ul>
        {depGate.depGateInstalling ? <p className="crosshook-muted">Installing dependencies...</p> : null}
        <div className="crosshook-modal__actions">
          <button
            type="button"
            className="crosshook-button"
            disabled={depGate.depGateInstalling}
            onClick={() => {
              void (async () => {
                const prefixPath = profile.runtime?.prefix_path ?? profile.steam?.compatdata_path ?? '';
                depGate.setDepGateInstalling(true);
                try {
                  await depGate.installPrefixDependency(selectedName, prefixPath, depGate.depGatePackages!);
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
