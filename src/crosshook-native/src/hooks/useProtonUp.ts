import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type {
  ProtonUpAvailableVersion,
  ProtonUpCacheMeta,
  ProtonUpCatalogResponse,
  ProtonUpInstallRequest,
  ProtonUpInstallResult,
  ProtonUpSuggestion,
} from '../types/protonup';

export interface UseProtonUpOptions {
  /** Auto-fetch catalog on mount. Defaults to false. */
  autoFetchCatalog?: boolean;
  /** Steam client install path override. */
  steamClientInstallPath?: string;
}

export interface UseProtonUpResult {
  /** Available versions from catalog. */
  versions: ProtonUpAvailableVersion[];
  /** Cache metadata (stale/offline/timestamps). */
  cacheMeta: ProtonUpCacheMeta | null;
  /** Whether catalog is currently loading. */
  catalogLoading: boolean;
  /** Catalog fetch error message. */
  catalogError: string | null;
  /** Refresh the catalog (force fetch from network). */
  refreshCatalog: () => void;
  /** Install a specific version. Returns the install result. */
  installVersion: (request: ProtonUpInstallRequest) => Promise<ProtonUpInstallResult>;
  /** Whether an install is currently in progress. */
  installing: boolean;
  /** Get suggestion for a community version string. */
  getSuggestion: (communityVersion: string) => Promise<ProtonUpSuggestion>;
}

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useProtonUp(options: UseProtonUpOptions = {}): UseProtonUpResult {
  const [versions, setVersions] = useState<ProtonUpAvailableVersion[]>([]);
  const [cacheMeta, setCacheMeta] = useState<ProtonUpCacheMeta | null>(null);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [fetchVersion, setFetchVersion] = useState(0);

  const steamClientInstallPath = options.steamClientInstallPath?.trim() ?? '';
  const autoFetchCatalog = options.autoFetchCatalog ?? false;

  useEffect(() => {
    if (!autoFetchCatalog && fetchVersion === 0) {
      return;
    }

    let active = true;
    setCatalogLoading(true);

    async function fetchCatalog() {
      try {
        const response = await invoke<ProtonUpCatalogResponse>(
          'protonup_list_available_versions',
          {
            provider: null,
            forceRefresh: fetchVersion > 0,
          },
        );

        if (!active) {
          return;
        }

        setVersions(response.versions);
        setCacheMeta(response.cache);
        setCatalogError(null);
      } catch (error) {
        if (!active) {
          return;
        }

        setVersions([]);
        setCacheMeta(null);
        setCatalogError(normalizeError(error));
      } finally {
        if (active) {
          setCatalogLoading(false);
        }
      }
    }

    void fetchCatalog();

    return () => {
      active = false;
    };
  }, [autoFetchCatalog, fetchVersion]);

  const refreshCatalog = useCallback(() => {
    setFetchVersion((current) => current + 1);
  }, []);

  const installVersion = useCallback(
    async (request: ProtonUpInstallRequest): Promise<ProtonUpInstallResult> => {
      setInstalling(true);
      try {
        const result = await invoke<ProtonUpInstallResult>('protonup_install_version', {
          request,
        });
        return result;
      } catch (error) {
        return {
          success: false,
          error_kind: 'unknown',
          error_message: normalizeError(error),
        };
      } finally {
        setInstalling(false);
      }
    },
    [],
  );

  const getSuggestion = useCallback(
    async (communityVersion: string): Promise<ProtonUpSuggestion> => {
      try {
        return await invoke<ProtonUpSuggestion>('protonup_get_suggestion', {
          communityVersion,
          steamClientInstallPath:
            steamClientInstallPath.length > 0 ? steamClientInstallPath : undefined,
        });
      } catch (error) {
        return {
          status: 'unknown',
          community_version: communityVersion.trim().length > 0 ? communityVersion : undefined,
        };
      }
    },
    [steamClientInstallPath],
  );

  return {
    versions,
    cacheMeta,
    catalogLoading,
    catalogError,
    refreshCatalog,
    installVersion,
    installing,
    getSuggestion,
  };
}

export default useProtonUp;
