import { useCallback, useMemo, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { ConfigDiffResult, ConfigRevisionSummary, ConfigRollbackResult } from '../../types';
import { formatInvokeError } from './formatInvokeError';

export interface UseProfileHistoryOptions {
  loadProfile: (
    name: string,
    loadOptions?: {
      collectionId?: string;
      loadErrorContext?: string;
      throwOnFailure?: boolean;
    }
  ) => Promise<void>;
  onAfterRollback?: (profileName: string) => void;
}

export function useProfileHistory({ loadProfile, onAfterRollback }: UseProfileHistoryOptions) {
  const [historyInFlightCount, setHistoryInFlightCount] = useState(0);
  const [historyError, setHistoryError] = useState<string | null>(null);

  const beginHistoryRequest = useCallback(() => {
    setHistoryInFlightCount((c) => c + 1);
  }, []);

  const endHistoryRequest = useCallback(() => {
    setHistoryInFlightCount((c) => Math.max(0, c - 1));
  }, []);

  const historyLoading = useMemo(() => historyInFlightCount > 0, [historyInFlightCount]);

  const onAfterRollbackRef = useRef(onAfterRollback);
  onAfterRollbackRef.current = onAfterRollback;

  const fetchConfigHistory = useCallback(
    async (name: string, limit?: number): Promise<ConfigRevisionSummary[]> => {
      beginHistoryRequest();
      setHistoryError(null);
      try {
        return await callCommand<ConfigRevisionSummary[]>('profile_config_history', {
          name,
          ...(limit !== undefined ? { limit } : {}),
        });
      } catch (err) {
        const message = formatInvokeError(err);
        setHistoryError(message);
        throw message;
      } finally {
        endHistoryRequest();
      }
    },
    [beginHistoryRequest, endHistoryRequest]
  );

  const fetchConfigDiff = useCallback(
    async (name: string, revisionId: number, rightRevisionId?: number): Promise<ConfigDiffResult> => {
      beginHistoryRequest();
      setHistoryError(null);
      try {
        return await callCommand<ConfigDiffResult>('profile_config_diff', {
          name,
          revisionId,
          ...(rightRevisionId !== undefined ? { rightRevisionId } : {}),
        });
      } catch (err) {
        const message = formatInvokeError(err);
        setHistoryError(message);
        throw message;
      } finally {
        endHistoryRequest();
      }
    },
    [beginHistoryRequest, endHistoryRequest]
  );

  const rollbackConfig = useCallback(
    async (name: string, revisionId: number): Promise<ConfigRollbackResult> => {
      beginHistoryRequest();
      setHistoryError(null);
      try {
        const result = await callCommand<ConfigRollbackResult>('profile_config_rollback', {
          name,
          revisionId,
        });
        await loadProfile(name, {
          loadErrorContext: 'Rollback applied, but reloading the profile failed',
          throwOnFailure: true,
        });
        onAfterRollbackRef.current?.(name);
        return result;
      } catch (err) {
        const message = formatInvokeError(err);
        setHistoryError(message);
        throw message;
      } finally {
        endHistoryRequest();
      }
    },
    [beginHistoryRequest, endHistoryRequest, loadProfile]
  );

  const markKnownGood = useCallback(
    async (name: string, revisionId: number): Promise<void> => {
      beginHistoryRequest();
      setHistoryError(null);
      try {
        await callCommand('profile_mark_known_good', { name, revisionId });
      } catch (err) {
        const message = formatInvokeError(err);
        setHistoryError(message);
        throw message;
      } finally {
        endHistoryRequest();
      }
    },
    [beginHistoryRequest, endHistoryRequest]
  );

  return {
    historyLoading,
    historyError,
    fetchConfigHistory,
    fetchConfigDiff,
    rollbackConfig,
    markKnownGood,
  };
}
