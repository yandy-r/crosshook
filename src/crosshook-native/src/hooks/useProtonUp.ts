import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type {
  ProtonUpAvailableVersion,
  ProtonUpCacheMeta,
  ProtonUpCatalogResponse,
  ProtonUpInstallRequest,
  ProtonUpInstallResult,
  ProtonUpProvider,
  ProtonUpSuggestion,
} from '../types/protonup';

export interface UseProtonUpOptions {
  /** Auto-fetch catalog on mount. Defaults to false. */
  autoFetchCatalog?: boolean;
  /** Steam client install path override. */
  steamClientInstallPath?: string;
  /** Which provider catalog to list (GE-Proton vs Proton-CachyOS). Defaults to `ge-proton`. */
  catalogProvider?: ProtonUpProvider;
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

const CATALOG_PROVIDERS: ProtonUpProvider[] = ['ge-proton', 'proton-cachyos'];

/**
 * Find which catalog contains `version` (checks GE first, then Proton-CachyOS).
 * Falls back to `ge-proton` when the version is absent from both (legacy install attempt).
 */
export async function resolveProtonUpProviderForVersion(version: string): Promise<ProtonUpProvider> {
  const trimmed = version.trim();
  if (trimmed.length === 0) {
    return 'ge-proton';
  }

  for (const provider of CATALOG_PROVIDERS) {
    try {
      const response = await callCommand<ProtonUpCatalogResponse>('protonup_list_available_versions', {
        provider,
        forceRefresh: false,
      });
      if (response.versions.some((v) => v.version === trimmed)) {
        return provider;
      }
    } catch {
      // Advisory path: try next provider
    }
  }

  return 'ge-proton';
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
  const catalogProvider = options.catalogProvider ?? 'ge-proton';

  useEffect(() => {
    if (!autoFetchCatalog && fetchVersion === 0) {
      return;
    }

    let active = true;
    setCatalogLoading(true);

    async function fetchCatalog() {
      try {
        const response = await callCommand<ProtonUpCatalogResponse>('protonup_list_available_versions', {
          provider: catalogProvider,
          forceRefresh: fetchVersion > 0,
        });

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
  }, [autoFetchCatalog, catalogProvider, fetchVersion]);

  const refreshCatalog = useCallback(() => {
    setFetchVersion((current) => current + 1);
  }, []);

  const installVersion = useCallback(async (request: ProtonUpInstallRequest): Promise<ProtonUpInstallResult> => {
    setInstalling(true);
    try {
      const result = await callCommand<ProtonUpInstallResult>('protonup_install_version', {
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
  }, []);

  const getSuggestion = useCallback(
    async (communityVersion: string): Promise<ProtonUpSuggestion> => {
      try {
        return await callCommand<ProtonUpSuggestion>('protonup_get_suggestion', {
          communityVersion,
          steamClientInstallPath: steamClientInstallPath.length > 0 ? steamClientInstallPath : undefined,
        });
      } catch (error) {
        return {
          status: 'unknown',
          community_version: communityVersion.trim().length > 0 ? communityVersion : undefined,
        };
      }
    },
    [steamClientInstallPath]
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
