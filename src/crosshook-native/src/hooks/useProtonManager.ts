import { useCallback, useEffect, useMemo, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type {
  InstallRootDescriptor,
  ProtonInstallHandle,
  ProtonUninstallResult,
  ProtonUpAvailableVersion,
  ProtonUpCacheMeta,
  ProtonUpCatalogResponse,
  ProtonUpInstallRequest,
  ProtonUpProvider,
  ProtonUpProviderDescriptor,
} from '../types/protonup';
import { useProtonInstalls } from './useProtonInstalls';
import { useProtonUp } from './useProtonUp';

export interface UseProtonManagerOptions {
  steamClientInstallPath?: string;
}

export interface UseProtonManagerResult {
  providers: ProtonUpProviderDescriptor[];
  roots: InstallRootDescriptor[];
  defaultRoot: InstallRootDescriptor | null;
  /** `null` means "All providers" (merged catalog). */
  selectedProviderId: string | null;
  setSelectedProviderId: (id: string | null) => void;
  /** Versions + cacheMeta. In All mode this is merged across providers; always sorted by release date desc. */
  catalog: {
    versions: ProtonUpAvailableVersion[];
    cacheMeta: ProtonUpCacheMeta | null;
    catalogLoading: boolean;
    catalogError: string | null;
    refreshCatalog: () => void;
  };
  /** Installed Proton tools (rescan-is-truth). */
  installs: ReturnType<typeof useProtonInstalls>;
  activeOpIds: string[];
  /** Remove a terminal op from the active list (called when the user dismisses). */
  dismissOp: (opId: string) => void;
  /**
   * Start an async install. `version` must be the full catalog DTO — the Rust
   * handler (`protonup_install_version_async`) deserializes it to
   * `ProtonUpAvailableVersion` and uses its checksum URL / asset size during
   * the download. Passing only the tag string causes Tauri to reject the
   * IPC payload as malformed.
   */
  install: (request: ProtonUpInstallRequest, version: ProtonUpAvailableVersion) => Promise<ProtonInstallHandle>;
  cancel: (opId: string) => Promise<boolean>;
  uninstall: (toolPath: string) => Promise<ProtonUninstallResult>;
  loading: boolean;
  error: string | null;
  offline: boolean;
}

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Sort by `published_at` descending (newest first). ISO-8601 sorts
 * lexicographically, so string compare is sufficient. Undated entries
 * sink to the end; tag-name desc is the final tiebreaker.
 */
function sortByPublishedDesc<T extends { published_at?: string | null; version: string }>(versions: T[]): T[] {
  return [...versions].sort((a, b) => {
    const ap = a.published_at ?? '';
    const bp = b.published_at ?? '';
    if (ap && bp && ap !== bp) return bp.localeCompare(ap);
    if (ap && !bp) return -1;
    if (!ap && bp) return 1;
    return b.version.localeCompare(a.version);
  });
}

function minIso(a: string | undefined, b: string | undefined): string | undefined {
  if (a === undefined) return b;
  if (b === undefined) return a;
  return a < b ? a : b;
}

const ALL_MODE_SENTINEL = null;

/**
 * Composite hook for the native Proton download manager UI.
 *
 * Loads available providers and install roots on mount, tracks active install
 * operation IDs, and exposes install/cancel/uninstall actions.
 *
 * When `selectedProviderId === null` ("All" mode), fans out
 * `protonup_list_available_versions` across every registered provider and
 * merges the results. Otherwise delegates to the single-provider
 * `useProtonUp` hook.
 */
export function useProtonManager(opts: UseProtonManagerOptions = {}): UseProtonManagerResult {
  const [providers, setProviders] = useState<ProtonUpProviderDescriptor[]>([]);
  const [roots, setRoots] = useState<InstallRootDescriptor[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(ALL_MODE_SENTINEL);
  const [activeOpIds, setActiveOpIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [allVersions, setAllVersions] = useState<ProtonUpAvailableVersion[]>([]);
  const [allCacheMeta, setAllCacheMeta] = useState<ProtonUpCacheMeta | null>(null);
  const [allLoading, setAllLoading] = useState(false);
  const [allError, setAllError] = useState<string | null>(null);
  const [allFetchVersion, setAllFetchVersion] = useState(0);

  const steamClientInstallPath = opts.steamClientInstallPath?.trim() ?? '';

  const inAllMode = selectedProviderId === ALL_MODE_SENTINEL;

  // Single-provider catalog (delegates to the legacy useProtonUp). Always
  // called to preserve hook-order invariants; auto-fetch is gated on a
  // concrete provider selection.
  const catalogProvider = (selectedProviderId ?? 'ge-proton') as ProtonUpProvider;
  const protonUp = useProtonUp({
    autoFetchCatalog: !inAllMode,
    catalogProvider,
    steamClientInstallPath: steamClientInstallPath.length > 0 ? steamClientInstallPath : undefined,
  });

  const installs = useProtonInstalls({
    steamClientInstallPath: steamClientInstallPath.length > 0 ? steamClientInstallPath : undefined,
  });

  // Load providers and install roots on mount (and whenever the Steam path changes).
  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);

    async function loadProviderData() {
      try {
        const [resolvedProviders, resolvedRoots] = await Promise.all([
          callCommand<ProtonUpProviderDescriptor[]>('protonup_list_providers'),
          callCommand<InstallRootDescriptor[]>('protonup_resolve_install_roots', {
            steamClientInstallPath: steamClientInstallPath.length > 0 ? steamClientInstallPath : undefined,
          }),
        ]);

        if (!active) return;

        setProviders(resolvedProviders);
        setRoots(resolvedRoots);
        setError(null);
      } catch (err) {
        if (!active) return;
        setProviders([]);
        setRoots([]);
        setError(normalizeError(err));
      } finally {
        if (active) setLoading(false);
      }
    }

    void loadProviderData();

    return () => {
      active = false;
    };
  }, [steamClientInstallPath]);

  // All-mode fan-out: fetch every provider catalog in parallel and merge.
  useEffect(() => {
    if (!inAllMode || providers.length === 0) {
      return;
    }

    let active = true;
    setAllLoading(true);

    const ids = providers.map((p) => p.id);

    (async () => {
      try {
        const responses = await Promise.all(
          ids.map((id) =>
            callCommand<ProtonUpCatalogResponse>('protonup_list_available_versions', {
              provider: id,
              forceRefresh: allFetchVersion > 0,
            }).catch((err: unknown): ProtonUpCatalogResponse => {
              console.warn(`[useProtonManager] catalog fetch failed for ${id}:`, err);
              return {
                versions: [],
                cache: { stale: false, offline: true, fetched_at: undefined, expires_at: undefined },
              };
            })
          )
        );

        if (!active) return;

        const merged = responses.flatMap((r) => r.versions);
        const cache: ProtonUpCacheMeta = responses.reduce<ProtonUpCacheMeta>(
          (acc, r) => ({
            stale: acc.stale || r.cache.stale,
            offline: acc.offline && r.cache.offline,
            fetched_at: minIso(acc.fetched_at, r.cache.fetched_at ?? undefined),
            expires_at: minIso(acc.expires_at, r.cache.expires_at ?? undefined),
          }),
          { stale: false, offline: true, fetched_at: undefined, expires_at: undefined }
        );

        setAllVersions(merged);
        setAllCacheMeta(cache);
        setAllError(null);
      } catch (err) {
        if (!active) return;
        setAllVersions([]);
        setAllCacheMeta(null);
        setAllError(normalizeError(err));
      } finally {
        if (active) setAllLoading(false);
      }
    })();

    return () => {
      active = false;
    };
  }, [inAllMode, providers, allFetchVersion]);

  const refreshAll = useCallback(() => {
    setAllFetchVersion((current) => current + 1);
  }, []);

  const install = useCallback(
    async (request: ProtonUpInstallRequest, version: ProtonUpAvailableVersion): Promise<ProtonInstallHandle> => {
      const handle = await callCommand<ProtonInstallHandle>('protonup_install_version_async', {
        request,
        version,
      });
      setActiveOpIds((ids) => [...ids, handle.op_id]);
      return handle;
    },
    []
  );

  const cancel = useCallback(async (opId: string): Promise<boolean> => {
    // Don't touch activeOpIds here — the Rust install task will emit
    // `Phase::Cancelled` when the token fires, which flips the
    // InstallProgressBar into its terminal state (Dismiss button,
    // red fill). Removing the opId now would unmount the progress
    // card before that event arrives and the user would see "nothing
    // happened". The dismiss handler removes the opId on user action
    // (or the auto-dismiss timer for successful installs).
    return await callCommand<boolean>('protonup_cancel_install', { opId });
  }, []);

  const dismissOp = useCallback((opId: string) => {
    setActiveOpIds((ids) => ids.filter((id) => id !== opId));
  }, []);

  const uninstall = useCallback(
    async (toolPath: string): Promise<ProtonUninstallResult> => {
      const result = await callCommand<ProtonUninstallResult>('protonup_uninstall_version', {
        toolPath,
        steamClientInstallPath: steamClientInstallPath.length > 0 ? steamClientInstallPath : undefined,
      });
      if (result.success) {
        installs.reload();
      }
      return result;
    },
    [steamClientInstallPath, installs]
  );

  const defaultRoot = useMemo(() => roots.find((r) => r.writable) ?? null, [roots]);

  const catalog = useMemo(() => {
    if (inAllMode) {
      return {
        versions: sortByPublishedDesc(allVersions),
        cacheMeta: allCacheMeta,
        catalogLoading: allLoading,
        catalogError: allError,
        refreshCatalog: refreshAll,
      };
    }
    return {
      versions: sortByPublishedDesc(protonUp.versions),
      cacheMeta: protonUp.cacheMeta,
      catalogLoading: protonUp.catalogLoading,
      catalogError: protonUp.catalogError,
      refreshCatalog: protonUp.refreshCatalog,
    };
  }, [
    inAllMode,
    allVersions,
    allCacheMeta,
    allLoading,
    allError,
    refreshAll,
    protonUp.versions,
    protonUp.cacheMeta,
    protonUp.catalogLoading,
    protonUp.catalogError,
    protonUp.refreshCatalog,
  ]);

  const offline = catalog.cacheMeta?.offline ?? false;

  return {
    providers,
    roots,
    defaultRoot,
    selectedProviderId,
    setSelectedProviderId,
    catalog,
    installs,
    activeOpIds,
    dismissOp,
    install,
    cancel,
    uninstall,
    loading,
    error,
    offline,
  };
}
