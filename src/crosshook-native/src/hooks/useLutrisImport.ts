import { useCallback, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { GameProfile } from '../types/profile';

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export type LutrisImportOutcome = 'imported' | 'skipped' | 'failed';

export interface LutrisImportPreview {
  entries: LutrisImportEntry[];
  lutris_root: string | null;
  diagnostics: string[];
}

export interface LutrisImportEntry {
  source_path: string;
  suggested_name: string;
  game_name: string;
  runner: string;
  mapped: GameProfile;
  warnings: string[];
  importable: boolean;
}

export interface LutrisImportEntryResult {
  entry: LutrisImportEntry;
  outcome: LutrisImportOutcome;
  profile_name: string | null;
  profile_path: string | null;
  error: string | null;
}

export interface LutrisImportResult {
  results: LutrisImportEntryResult[];
  imported_count: number;
  skipped_count: number;
  failed_count: number;
}

export function useLutrisImport() {
  const [isPreparing, setIsPreparing] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [importResult, setImportResult] = useState<LutrisImportResult | null>(null);
  const [importError, setImportError] = useState<string | null>(null);

  const clearImportState = useCallback(() => {
    setImportResult(null);
    setImportError(null);
  }, []);

  const prepare = useCallback(async (directory?: string): Promise<LutrisImportPreview> => {
    setIsPreparing(true);
    try {
      const result = await callCommand<LutrisImportPreview>('lutris_prepare_import', {
        directory: directory ?? null,
      });
      return result;
    } catch (err) {
      const message = normalizeError(err);
      throw new Error(message);
    } finally {
      setIsPreparing(false);
    }
  }, []);

  const importProfiles = useCallback(async (entries: LutrisImportEntry[]): Promise<LutrisImportResult | null> => {
    setIsImporting(true);
    setImportError(null);
    setImportResult(null);
    try {
      const result = await callCommand<LutrisImportResult>('lutris_import_profiles', { entries });
      setImportResult(result);
      return result;
    } catch (err) {
      setImportError(normalizeError(err));
      return null;
    } finally {
      setIsImporting(false);
    }
  }, []);

  return {
    isPreparing,
    isImporting,
    importResult,
    importError,
    clearImportState,
    prepare,
    importProfiles,
  };
}
