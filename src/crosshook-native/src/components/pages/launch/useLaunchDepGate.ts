import { useCallback, useEffect, useRef, useState } from 'react';
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
  const pendingActionRef = useRef<'game' | 'trainer' | null>(depGatePendingAction);
  const unlistenPrefixDepRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    pendingActionRef.current = depGatePendingAction;
  }, [depGatePendingAction]);

  const clearPrefixDepListener = useCallback(() => {
    unlistenPrefixDepRef.current?.();
    unlistenPrefixDepRef.current = null;
  }, []);

  const installPrefixDependencyWithListener = useCallback(
    async (profileName: string, prefixPath: string, packages: string[]) => {
      clearPrefixDepListener();
      const unlisten = await subscribeEvent<{
        profile_name: string;
        prefix_path: string;
        succeeded: boolean;
      }>('prefix-dep-complete', (event) => {
        if (event.payload.profile_name !== profileName || event.payload.prefix_path !== prefixPath) {
          return;
        }

        clearPrefixDepListener();
        setDepGateInstalling(false);
        setDepGatePackages(null);
        const action = pendingActionRef.current;
        setDepGatePendingAction(null);
        if (!event.payload.succeeded) {
          return;
        }
        if (action === 'game') {
          launchGame();
        } else if (action === 'trainer') {
          launchTrainer();
        }
      });
      unlistenPrefixDepRef.current = unlisten;
      await installPrefixDependency(profileName, prefixPath, packages);
    },
    [clearPrefixDepListener, installPrefixDependency, launchGame, launchTrainer]
  );

  useEffect(() => clearPrefixDepListener, [clearPrefixDepListener]);

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
            await installPrefixDependencyWithListener(selectedName, prefixPath, missing);
          } catch {
            clearPrefixDepListener();
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
    [
      profile,
      selectedName,
      autoInstallPrefixDeps,
      clearPrefixDepListener,
      getDependencyStatus,
      installPrefixDependencyWithListener,
    ]
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
    installPrefixDependency: installPrefixDependencyWithListener,
  };
}
