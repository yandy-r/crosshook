import { useCallback, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type { PrefixCleanupResult, PrefixCleanupTarget, PrefixStorageScanResult } from '../types/prefix-storage';

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export interface PrefixStorageManagementState {
  scanResult: PrefixStorageScanResult | null;
  scanLoading: boolean;
  cleanupLoading: boolean;
  error: string | null;
  scanStorage: () => Promise<void>;
  cleanupStorage: (targets: PrefixCleanupTarget[]) => Promise<PrefixCleanupResult>;
}

export function usePrefixStorageManagement(): PrefixStorageManagementState {
  const [scanResult, setScanResult] = useState<PrefixStorageScanResult | null>(null);
  const [scanLoading, setScanLoading] = useState(false);
  const [cleanupLoading, setCleanupLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scanStorage = useCallback(async () => {
    setScanLoading(true);
    setError(null);
    try {
      const result = await invoke<PrefixStorageScanResult>('scan_prefix_storage');
      setScanResult(result);
    } catch (scanError) {
      setError(formatError(scanError));
      throw scanError;
    } finally {
      setScanLoading(false);
    }
  }, []);

  const cleanupStorage = useCallback(async (targets: PrefixCleanupTarget[]): Promise<PrefixCleanupResult> => {
    setCleanupLoading(true);
    setError(null);
    try {
      return await invoke<PrefixCleanupResult>('cleanup_prefix_storage', { targets });
    } catch (cleanupError) {
      setError(formatError(cleanupError));
      throw cleanupError;
    } finally {
      setCleanupLoading(false);
    }
  }, []);

  return {
    scanResult,
    scanLoading,
    cleanupLoading,
    error,
    scanStorage,
    cleanupStorage,
  };
}

