import { useCallback, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type {
  PrefixCleanupResult,
  PrefixCleanupTarget,
  PrefixStorageCleanupAuditRow,
  PrefixStorageHistoryResponse,
  PrefixStorageScanResult,
  PrefixStorageSnapshotRow,
} from '../types/prefix-storage';

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export type ScanSource = 'live' | 'cached' | null;

export interface PrefixStorageManagementState {
  scanResult: PrefixStorageScanResult | null;
  scanLoading: boolean;
  cleanupLoading: boolean;
  error: string | null;
  scanSource: ScanSource;
  persistenceAvailable: boolean;
  snapshots: PrefixStorageSnapshotRow[];
  auditEntries: PrefixStorageCleanupAuditRow[];
  historyLoading: boolean;
  scanStorage: () => Promise<void>;
  cleanupStorage: (targets: PrefixCleanupTarget[]) => Promise<PrefixCleanupResult>;
  loadHistory: () => Promise<void>;
}

export function usePrefixStorageManagement(): PrefixStorageManagementState {
  const [scanResult, setScanResult] = useState<PrefixStorageScanResult | null>(null);
  const [scanLoading, setScanLoading] = useState(false);
  const [cleanupLoading, setCleanupLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scanSource, setScanSource] = useState<ScanSource>(null);
  const [persistenceAvailable, setPersistenceAvailable] = useState(true);
  const [snapshots, setSnapshots] = useState<PrefixStorageSnapshotRow[]>([]);
  const [auditEntries, setAuditEntries] = useState<PrefixStorageCleanupAuditRow[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);

  const loadHistory = useCallback(async () => {
    setHistoryLoading(true);
    try {
      const response = await callCommand<PrefixStorageHistoryResponse>('get_prefix_storage_history');
      setPersistenceAvailable(response.available);
      setSnapshots(response.snapshots);
      setAuditEntries(response.audit);
    } catch {
      setPersistenceAvailable(false);
      setSnapshots([]);
      setAuditEntries([]);
    } finally {
      setHistoryLoading(false);
    }
  }, []);

  const scanStorage = useCallback(async () => {
    setScanLoading(true);
    setError(null);
    try {
      const result = await callCommand<PrefixStorageScanResult>('scan_prefix_storage');
      setScanResult(result);
      setScanSource('live');
      // Refresh history after scan to pick up newly persisted snapshots
      loadHistory().catch(() => {});
    } catch (scanError) {
      setError(formatError(scanError));
      throw scanError;
    } finally {
      setScanLoading(false);
    }
  }, [loadHistory]);

  const cleanupStorage = useCallback(async (targets: PrefixCleanupTarget[]): Promise<PrefixCleanupResult> => {
    setCleanupLoading(true);
    setError(null);
    try {
      const result = await callCommand<PrefixCleanupResult>('cleanup_prefix_storage', { targets });
      // Refresh history after cleanup to pick up newly persisted audit rows
      loadHistory().catch(() => {});
      return result;
    } catch (cleanupError) {
      setError(formatError(cleanupError));
      throw cleanupError;
    } finally {
      setCleanupLoading(false);
    }
  }, [loadHistory]);

  return {
    scanResult,
    scanLoading,
    cleanupLoading,
    error,
    scanSource,
    persistenceAvailable,
    snapshots,
    auditEntries,
    historyLoading,
    scanStorage,
    cleanupStorage,
    loadHistory,
  };
}
