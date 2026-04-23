import { useCallback, useEffect, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { useLaunchStateContext } from '../../../context/LaunchStateContext';
import { useLaunchPrefixDependencyGate } from '../../../hooks/useLaunchPrefixDependencyGate';
import type { GameProfile } from '../../../types/profile';

interface UseLaunchDepGateOptions {
  profile: GameProfile;
  selectedName: string;
  autoInstallPrefixDeps: boolean;
}

export interface DepGateState {
  depGatePackages: string[] | null;
  depGatePendingAction: 'game' | 'trainer' | null;
  depGateInstalling: boolean;
  isGamescopeRunning: boolean;
  setDepGatePackages: (packages: string[] | null) => void;
  setDepGatePendingAction: (action: 'game' | 'trainer' | null) => void;
  setDepGateInstalling: (installing: boolean) => void;
  handleBeforeLaunch: (action: 'game' | 'trainer') => Promise<boolean>;
  installPrefixDependency: ReturnType<typeof useLaunchPrefixDependencyGate>['installPrefixDependency'];
}

export function useLaunchDepGate({
  profile,
  selectedName,
  autoInstallPrefixDeps,
}: UseLaunchDepGateOptions): DepGateState {
  const { launchGame, launchTrainer } = useLaunchStateContext();
  const { getDependencyStatus, installPrefixDependency, isGamescopeRunning } = useLaunchPrefixDependencyGate();

  const [depGatePackages, setDepGatePackages] = useState<string[] | null>(null);
  const [depGatePendingAction, setDepGatePendingAction] = useState<'game' | 'trainer' | null>(null);
  const [depGateInstalling, setDepGateInstalling] = useState(false);

  // Listen for prefix-dep-complete events while installing
  useEffect(() => {
    if (!depGateInstalling) return;

    const prefixPath = profile.runtime?.prefix_path ?? profile.steam?.compatdata_path ?? '';

    const unlistenPromise = subscribeEvent<{
      profile_name: string;
      prefix_path: string;
      succeeded: boolean;
    }>('prefix-dep-complete', (event) => {
      if (event.payload.profile_name !== selectedName || event.payload.prefix_path !== prefixPath) return;

      setDepGateInstalling(false);
      if (event.payload.succeeded) {
        const action = depGatePendingAction;
        setDepGatePackages(null);
        setDepGatePendingAction(null);
        if (action === 'game') {
          launchGame();
        } else if (action === 'trainer') {
          launchTrainer();
        }
      } else {
        setDepGatePackages(null);
        setDepGatePendingAction(null);
      }
    });

    return () => {
      void unlistenPromise.then((fn) => fn());
    };
  }, [depGateInstalling, selectedName, profile, depGatePendingAction, launchGame, launchTrainer]);

  const handleBeforeLaunch = useCallback(
    async (action: 'game' | 'trainer'): Promise<boolean> => {
      const requiredPackages = profile.trainer?.required_protontricks;
      if (!requiredPackages || requiredPackages.length === 0) return true;

      const prefixPath = profile.runtime?.prefix_path ?? profile.steam?.compatdata_path ?? '';
      if (!prefixPath) return true;

      try {
        const statuses = await getDependencyStatus(selectedName, prefixPath);

        const missing = requiredPackages.filter((pkg) => {
          const status = statuses.find((s) => s.package_name === pkg);
          return (
            !status || status.state === 'missing' || status.state === 'install_failed' || status.state === 'unknown'
          );
        });

        if (missing.length === 0) return true;

        if (autoInstallPrefixDeps) {
          // Auto-install: invoke and wait for the prefix-dep-complete event
          setDepGatePackages(missing);
          setDepGatePendingAction(action);
          setDepGateInstalling(true);
          try {
            await installPrefixDependency(selectedName, prefixPath, missing);
          } catch {
            setDepGateInstalling(false);
            setDepGatePackages(null);
            setDepGatePendingAction(null);
          }
          return false;
        }

        // Show gate modal
        setDepGatePackages(missing);
        setDepGatePendingAction(action);
        return false;
      } catch {
        // Cannot check — allow launch
        return true;
      }
    },
    [profile, selectedName, autoInstallPrefixDeps, getDependencyStatus, installPrefixDependency]
  );

  return {
    depGatePackages,
    depGatePendingAction,
    depGateInstalling,
    isGamescopeRunning,
    setDepGatePackages,
    setDepGatePendingAction,
    setDepGateInstalling,
    handleBeforeLaunch,
    installPrefixDependency,
  };
}
