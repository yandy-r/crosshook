import { invoke } from '@tauri-apps/api/core';
import { useCallback, useState } from 'react';

import { useProfileHealthContext } from '../context/ProfileHealthContext';
import type { ApplyMigrationRequest, BatchMigrationResult, MigrationApplyResult, MigrationScanResult } from '../types';

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useProtonMigration() {
  const { revalidateSingle, batchValidate } = useProfileHealthContext();

  const [scanResult, setScanResult] = useState<MigrationScanResult | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [applyResult, setApplyResult] = useState<MigrationApplyResult | null>(null);
  const [isApplying, setIsApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isBatchApplying, setIsBatchApplying] = useState(false);
  const [batchResult, setBatchResult] = useState<BatchMigrationResult | null>(null);
  const [batchError, setBatchError] = useState<string | null>(null);

  const scanMigrations = useCallback(async (steamClientInstallPath?: string): Promise<MigrationScanResult | null> => {
    setIsScanning(true);
    setError(null);
    try {
      const result = await invoke<MigrationScanResult>('check_proton_migrations', {
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

  const applySingleMigration = useCallback(
    async (request: ApplyMigrationRequest) => {
      setIsApplying(true);
      setError(null);
      try {
        const result = await invoke<MigrationApplyResult>('apply_proton_migration', { request });
        setApplyResult(result);
        if (result.outcome === 'applied') {
          await revalidateSingle(request.profile_name);
        }
      } catch (err) {
        const message = normalizeError(err);
        setError(`${message} Re-scan to see current state.`);
      } finally {
        setIsApplying(false);
      }
    },
    [revalidateSingle]
  );

  const applyBatchMigration = useCallback(
    async (requests: ApplyMigrationRequest[]) => {
      setIsBatchApplying(true);
      setBatchResult(null);
      setBatchError(null);
      try {
        const result = await invoke<BatchMigrationResult>('apply_batch_migration', {
          request: { migrations: requests },
        });
        setBatchResult(result);
        if (result.applied_count > 0) {
          await batchValidate();
        }
      } catch (err) {
        setBatchError(`${normalizeError(err)} Re-scan to see current state.`);
      } finally {
        setIsBatchApplying(false);
      }
    },
    [batchValidate]
  );

  return {
    scanResult,
    isScanning,
    applyResult,
    isApplying,
    error,
    isBatchApplying,
    batchResult,
    batchError,
    scanMigrations,
    applySingleMigration,
    applyBatchMigration,
  };
}
