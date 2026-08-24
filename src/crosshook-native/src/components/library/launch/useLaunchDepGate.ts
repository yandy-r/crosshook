import { useCallback, useEffect, useRef, useState } from 'react';
import { useLaunchStateContext } from '@/context/LaunchStateContext';
import { useLaunchPrefixDependencyGate } from '@/hooks/useLaunchPrefixDependencyGate';
import { subscribeEvent } from '@/lib/events';
import type { PrefixDepCompletePayload, PrefixVersionRepairStatus } from '@/types/prefix-deps';
import type { GameProfile } from '@/types/profile';
import { resolveEffectivePrefixPath } from '@/utils/prefixPath';

type LaunchAction = 'game' | 'trainer';

interface GateOperationToken {
  generation: number;
  scopeKey: string;
  prefixPath: string;
  action: LaunchAction;
}

interface UseLaunchDepGateOptions {
  profile: GameProfile;
  selectedName: string;
  autoInstallPrefixDeps: boolean;
}

export interface DepGateState {
  depGatePackages: string[] | null;
  depGateRepair: PrefixVersionRepairStatus | null;
  depGatePendingAction: LaunchAction | null;
  depGateInstalling: boolean;
  depGateVerifying: boolean;
  depGateRepairing: boolean;
  isGamescopeRunning: boolean;
  setDepGatePackages: (packages: string[] | null) => void;
  setDepGateRepair: (repair: PrefixVersionRepairStatus | null) => void;
  setDepGatePendingAction: (action: LaunchAction | null) => void;
  setDepGateInstalling: (installing: boolean) => void;
  setDepGateVerifying: (verifying: boolean) => void;
  setDepGateRepairing: (repairing: boolean) => void;
  handleBeforeLaunch: (action: LaunchAction) => Promise<boolean>;
  repairPrefixVersion: () => Promise<void>;
  installPrefixDependency: ReturnType<typeof useLaunchPrefixDependencyGate>['installPrefixDependency'];
}

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useLaunchDepGate({
  profile,
  selectedName,
  autoInstallPrefixDeps,
}: UseLaunchDepGateOptions): DepGateState {
  const { launchGame, launchTrainer } = useLaunchStateContext();
  const {
    getDependencyStatus,
    getPrefixVersionRepairStatus,
    installPrefixDependency,
    repairPrefixWindowsVersion,
    isGamescopeRunning,
  } = useLaunchPrefixDependencyGate();
  const resolvedPrefixPath = resolveEffectivePrefixPath(profile);
  const scopeKey = `${selectedName}\0${resolvedPrefixPath}`;

  const [depGatePackages, setDepGatePackages] = useState<string[] | null>(null);
  const [depGateRepair, setDepGateRepair] = useState<PrefixVersionRepairStatus | null>(null);
  const [depGatePendingAction, setDepGatePendingActionState] = useState<LaunchAction | null>(null);
  const [depGateInstalling, setDepGateInstalling] = useState(false);
  const [depGateVerifying, setDepGateVerifying] = useState(false);
  const [depGateRepairing, setDepGateRepairing] = useState(false);
  const pendingActionRef = useRef<LaunchAction | null>(depGatePendingAction);
  const knownRepairsRef = useRef(new Map<string, PrefixVersionRepairStatus>());
  const unlistenPrefixDepRef = useRef<(() => void) | null>(null);
  const mountedRef = useRef(true);
  const generationRef = useRef(0);
  const scopeKeyRef = useRef(scopeKey);
  const activeOperationRef = useRef<GateOperationToken | null>(null);

  if (scopeKeyRef.current !== scopeKey) {
    scopeKeyRef.current = scopeKey;
    generationRef.current += 1;
    activeOperationRef.current = null;
    pendingActionRef.current = null;
  }

  const setDepGatePendingAction = useCallback((action: LaunchAction | null) => {
    pendingActionRef.current = action;
    setDepGatePendingActionState(action);
  }, []);

  const launchPendingAction = useCallback(
    (action: LaunchAction | null) => {
      if (action === 'game') {
        launchGame();
      } else if (action === 'trainer') {
        launchTrainer();
      }
    },
    [launchGame, launchTrainer]
  );

  const clearPrefixDepListener = useCallback(() => {
    unlistenPrefixDepRef.current?.();
    unlistenPrefixDepRef.current = null;
  }, []);

  const blockForKnownRepair = useCallback((prefixPath: string, status: PrefixVersionRepairStatus) => {
    knownRepairsRef.current.set(prefixPath, status);
    setDepGateRepair(status);
  }, []);

  const clearKnownRepair = useCallback((prefixPath: string) => {
    knownRepairsRef.current.delete(prefixPath);
    setDepGateRepair(null);
  }, []);

  const verificationFailure = useCallback((error: unknown): PrefixVersionRepairStatus => {
    return {
      required: true,
      state: 'failed',
      last_error: `Unable to verify prefix repair status: ${normalizeError(error)}`,
    };
  }, []);

  const beginOperation = useCallback((action: LaunchAction, prefixPath: string): GateOperationToken => {
    const token = {
      generation: generationRef.current + 1,
      scopeKey: scopeKeyRef.current,
      prefixPath,
      action,
    };
    generationRef.current = token.generation;
    activeOperationRef.current = token;
    return token;
  }, []);

  const isOperationCurrent = useCallback((token: GateOperationToken): boolean => {
    const active = activeOperationRef.current;
    return (
      mountedRef.current &&
      active !== null &&
      active.generation === token.generation &&
      active.scopeKey === token.scopeKey &&
      scopeKeyRef.current === token.scopeKey
    );
  }, []);

  const installPrefixDependencyWithListener = useCallback(
    async (
      profileName: string,
      prefixPath: string,
      packages: string[],
      operation: GateOperationToken | null = activeOperationRef.current
    ) => {
      clearPrefixDepListener();
      const unlisten = await subscribeEvent<PrefixDepCompletePayload>('prefix-dep-complete', (event) => {
        if (event.payload.profile_name !== profileName || event.payload.prefix_path !== prefixPath) {
          return;
        }
        if (!operation || !isOperationCurrent(operation)) return;

        clearPrefixDepListener();
        if (event.payload.restore_state === 'failed') {
          setDepGateInstalling(false);
          setDepGateVerifying(false);
          setDepGatePackages(null);
          blockForKnownRepair(prefixPath, {
            required: true,
            state: 'failed',
            last_error: event.payload.restore_error,
          });
          return;
        }
        if (event.payload.restore_state === 'succeeded') {
          setDepGateVerifying(true);
        }
        void (async () => {
          if (event.payload.restore_state === 'succeeded') {
            try {
              const status = await getPrefixVersionRepairStatus(profileName, prefixPath);
              if (!isOperationCurrent(operation)) return;
              if (status.required) {
                setDepGateInstalling(false);
                setDepGateVerifying(false);
                setDepGatePackages(null);
                blockForKnownRepair(prefixPath, status);
                return;
              }
              clearKnownRepair(prefixPath);
            } catch (error) {
              if (!isOperationCurrent(operation)) return;
              setDepGateInstalling(false);
              setDepGateVerifying(false);
              setDepGatePackages(null);
              blockForKnownRepair(prefixPath, verificationFailure(error));
              return;
            }
          }

          if (!isOperationCurrent(operation)) return;
          if (!event.payload.install_succeeded) {
            setDepGateInstalling(false);
            setDepGateVerifying(false);
            setDepGatePackages(null);
            setDepGatePendingAction(null);
            activeOperationRef.current = null;
            return;
          }

          const action = pendingActionRef.current;
          if (!isOperationCurrent(operation)) return;
          setDepGateInstalling(false);
          setDepGateVerifying(false);
          setDepGatePackages(null);
          setDepGatePendingAction(null);
          activeOperationRef.current = null;
          launchPendingAction(action);
        })();
      });
      if (!operation || !isOperationCurrent(operation)) {
        unlisten();
        return;
      }
      unlistenPrefixDepRef.current = unlisten;
      await installPrefixDependency(profileName, prefixPath, packages);
    },
    [
      blockForKnownRepair,
      clearKnownRepair,
      clearPrefixDepListener,
      getPrefixVersionRepairStatus,
      installPrefixDependency,
      isOperationCurrent,
      launchPendingAction,
      setDepGatePendingAction,
      verificationFailure,
    ]
  );

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      generationRef.current += 1;
      activeOperationRef.current = null;
      clearPrefixDepListener();
    };
  }, [clearPrefixDepListener]);

  useEffect(() => {
    scopeKeyRef.current = scopeKey;
    clearPrefixDepListener();
    setDepGatePackages(null);
    setDepGateRepair(null);
    setDepGatePendingAction(null);
    setDepGateInstalling(false);
    setDepGateVerifying(false);
    setDepGateRepairing(false);
  }, [clearPrefixDepListener, scopeKey, setDepGatePendingAction]);

  const evaluatePackageGate = useCallback(
    async (action: LaunchAction, existingOperation?: GateOperationToken): Promise<boolean> => {
      const requiredPackages = profile.trainer?.required_protontricks;
      if (!requiredPackages || requiredPackages.length === 0) return true;

      const prefixPath = resolvedPrefixPath;
      if (!prefixPath) return true;
      const operation = existingOperation ?? beginOperation(action, prefixPath);

      try {
        const statuses = await getDependencyStatus(selectedName, prefixPath);
        if (!isOperationCurrent(operation)) return false;

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
            await installPrefixDependencyWithListener(selectedName, prefixPath, missing, operation);
            if (!isOperationCurrent(operation)) return false;
          } catch {
            if (!isOperationCurrent(operation)) return false;
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
        if (!isOperationCurrent(operation)) return false;
        // Cannot check — allow launch
        return true;
      }
    },
    [
      profile,
      resolvedPrefixPath,
      selectedName,
      autoInstallPrefixDeps,
      clearPrefixDepListener,
      getDependencyStatus,
      installPrefixDependencyWithListener,
      beginOperation,
      isOperationCurrent,
      setDepGatePendingAction,
    ]
  );

  const repairPrefixVersion = useCallback(async () => {
    const action = pendingActionRef.current;
    const prefixPath = resolvedPrefixPath;
    if (!action || !prefixPath) return;
    const operation = beginOperation(action, prefixPath);

    setDepGateRepairing(true);
    try {
      const status = await repairPrefixWindowsVersion(selectedName, prefixPath);
      if (!isOperationCurrent(operation)) return;
      if (status.required) {
        blockForKnownRepair(prefixPath, status);
        return;
      }

      try {
        const verifiedStatus = await getPrefixVersionRepairStatus(selectedName, prefixPath);
        if (!isOperationCurrent(operation)) return;
        if (verifiedStatus.required) {
          blockForKnownRepair(prefixPath, verifiedStatus);
          return;
        }
      } catch (error) {
        if (!isOperationCurrent(operation)) return;
        blockForKnownRepair(prefixPath, verificationFailure(error));
        return;
      }
      clearKnownRepair(prefixPath);

      const mayLaunch = await evaluatePackageGate(action, operation);
      if (!isOperationCurrent(operation)) return;
      if (mayLaunch) {
        setDepGatePendingAction(null);
        launchPendingAction(action);
      }
    } catch (error) {
      if (!isOperationCurrent(operation)) return;
      blockForKnownRepair(prefixPath, {
        required: true,
        state: 'failed',
        last_error: normalizeError(error),
      });
    } finally {
      if (isOperationCurrent(operation)) setDepGateRepairing(false);
    }
  }, [
    blockForKnownRepair,
    beginOperation,
    clearKnownRepair,
    evaluatePackageGate,
    getPrefixVersionRepairStatus,
    isOperationCurrent,
    launchPendingAction,
    resolvedPrefixPath,
    repairPrefixWindowsVersion,
    selectedName,
    setDepGatePendingAction,
    verificationFailure,
  ]);

  const handleBeforeLaunch = useCallback(
    async (action: LaunchAction): Promise<boolean> => {
      const prefixPath = resolvedPrefixPath;
      if (!prefixPath) return true;
      const operation = beginOperation(action, prefixPath);

      try {
        const repairStatus = await getPrefixVersionRepairStatus(selectedName, prefixPath);
        if (!isOperationCurrent(operation)) return false;
        if (repairStatus.required) {
          setDepGatePackages(null);
          blockForKnownRepair(prefixPath, repairStatus);
          setDepGatePendingAction(action);
          return false;
        }
        const knownRepair = knownRepairsRef.current.get(prefixPath);
        if (knownRepair) {
          setDepGatePackages(null);
          setDepGateRepair(knownRepair);
          setDepGatePendingAction(action);
          return false;
        }
        setDepGateRepair(null);
      } catch {
        if (!isOperationCurrent(operation)) return false;
        const knownRepair = knownRepairsRef.current.get(prefixPath);
        if (knownRepair) {
          setDepGatePackages(null);
          setDepGateRepair(knownRepair);
          setDepGatePendingAction(action);
          return false;
        }
        // Metadata lookup is fail-soft only when this session has no known repair.
      }

      return evaluatePackageGate(action, operation);
    },
    [
      blockForKnownRepair,
      beginOperation,
      evaluatePackageGate,
      getPrefixVersionRepairStatus,
      isOperationCurrent,
      resolvedPrefixPath,
      selectedName,
      setDepGatePendingAction,
    ]
  );

  return {
    depGatePackages,
    depGateRepair,
    depGatePendingAction,
    depGateInstalling,
    depGateVerifying,
    depGateRepairing,
    isGamescopeRunning,
    setDepGatePackages,
    setDepGateRepair,
    setDepGatePendingAction,
    setDepGateInstalling,
    setDepGateVerifying,
    setDepGateRepairing,
    handleBeforeLaunch,
    repairPrefixVersion,
    installPrefixDependency: installPrefixDependencyWithListener,
  };
}
