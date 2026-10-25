import { useCallback, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { ApplyMigrationRequest, BatchMigrationResult, MigrationApplyResult, MigrationScanResult } from '../types';

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Proton path migration scan/apply state. Applied migrations publish
 * `profiles-changed` (`migrated`), which drives the health revalidation in
 * `useProfileHealth`; this hook does not revalidate directly.
 */
export function useProtonMigration() {
  const [scanResult, setScanResult] = useState<MigrationScanResult | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [applyResult, setApplyResult] = useState<MigrationApplyResult | null>(null);
  const [isApplying, setIsApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isBatchApplying, setIsBatchApplying] = useState(false);
  const [batchResult, setBatchResult] = useState<BatchMigrationResult | null>(null);
  const [batchError, setBatchError] = useState<string | null>(null);

  const clearMigrationError = useCallback(() => {
    setError(null);
  }, []);

  const scanMigrations = useCallback(async (steamClientInstallPath?: string): Promise<MigrationScanResult | null> => {
    setIsScanning(true);
    setError(null);
    try {
      const result = await callCommand<MigrationScanResult>('check_proton_migrations', {
        steamClientInstallPath: steamClientInstallPath ?? null,
      });
      setScanResult(result);
      return result;
    } catch (err) {
      setError(normalizeError(err));
      return null;
    } finally {
      setIsScanning(false);
    }
  }, []);

  const applySingleMigration = useCallback(async (request: ApplyMigrationRequest) => {
    setIsApplying(true);
    setError(null);
    try {
      const result = await callCommand<MigrationApplyResult>('apply_proton_migration', { request });
      setApplyResult(result);
    } catch (err) {
      const message = normalizeError(err);
      setError(`${message} Re-scan to see current state.`);
    } finally {
      setIsApplying(false);
    }
  }, []);

  const applyBatchMigration = useCallback(async (requests: ApplyMigrationRequest[]) => {
    setIsBatchApplying(true);
    setBatchResult(null);
    setBatchError(null);
    try {
      const result = await callCommand<BatchMigrationResult>('apply_batch_migration', {
        request: { migrations: requests },
      });
      setBatchResult(result);
    } catch (err) {
      setBatchError(`${normalizeError(err)} Re-scan to see current state.`);
    } finally {
      setIsBatchApplying(false);
    }
  }, []);

  return {
    scanResult,
    isScanning,
    applyResult,
    isApplying,
    error,
    isBatchApplying,
    batchResult,
    batchError,
    clearMigrationError,
    scanMigrations,
    applySingleMigration,
    applyBatchMigration,
  };
}
