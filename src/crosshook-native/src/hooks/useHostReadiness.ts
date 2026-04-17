import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { Capability, HostToolCheckResult, HostToolDetails, ReadinessCheckResult } from '../types/onboarding';

const HOST_READINESS_STALE_MS = 24 * 60 * 60 * 1000;

export interface CachedHostReadinessSnapshot {
  checked_at: string;
  detected_distro_family: string;
  tool_checks: HostToolCheckResult[];
  all_passed: boolean;
  critical_failures: number;
  warnings: number;
}

export interface UseHostReadinessResult {
  snapshot: CachedHostReadinessSnapshot | null;
  capabilities: Capability[];
  isStale: boolean;
  lastCheckedAt: string | null;
  isRefreshing: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  probeTool: (toolId: string) => Promise<HostToolDetails>;
}

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isMissingMockCommandError(error: unknown): boolean {
  return error instanceof Error && error.message.startsWith('[dev-mock] Unhandled command:');
}

function isSnapshotStale(checkedAt: string | null): boolean {
  if (checkedAt == null) {
    return false;
  }

  const timestamp = Date.parse(checkedAt);
  if (Number.isNaN(timestamp)) {
    return true;
  }

  return Date.now() - timestamp > HOST_READINESS_STALE_MS;
}

function snapshotFromReadinessResult(
  result: ReadinessCheckResult,
  checkedAt: string = new Date().toISOString()
): CachedHostReadinessSnapshot {
  return {
    checked_at: checkedAt,
    detected_distro_family: result.detected_distro_family ?? '',
    tool_checks: result.tool_checks ?? [],
    all_passed: result.all_passed,
    critical_failures: result.critical_failures,
    warnings: result.warnings,
  };
}

function mergeToolCheckDetails(toolCheck: HostToolCheckResult, details: HostToolDetails): HostToolCheckResult {
  if (toolCheck.tool_id !== details.tool_id) {
    return toolCheck;
  }

  return {
    ...toolCheck,
    tool_version: details.tool_version,
    resolved_path: details.resolved_path,
  };
}

function mergeSnapshotDetails(
  snapshot: CachedHostReadinessSnapshot | null,
  details: HostToolDetails
): CachedHostReadinessSnapshot | null {
  if (snapshot == null) {
    return null;
  }

  return {
    ...snapshot,
    tool_checks: snapshot.tool_checks.map((toolCheck) => mergeToolCheckDetails(toolCheck, details)),
  };
}

function mergeCapabilityDetails(capabilities: Capability[], details: HostToolDetails): Capability[] {
  return capabilities.map((capability) => ({
    ...capability,
    missing_required: capability.missing_required.map((toolCheck) => mergeToolCheckDetails(toolCheck, details)),
    missing_optional: capability.missing_optional.map((toolCheck) => mergeToolCheckDetails(toolCheck, details)),
  }));
}

export function useHostReadiness(): UseHostReadinessResult {
  const [snapshot, setSnapshot] = useState<CachedHostReadinessSnapshot | null>(null);
  const [capabilities, setCapabilities] = useState<Capability[]>([]);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(false);
  const requestIdRef = useRef(0);
  const hasBootstrappedLiveRefreshRef = useRef(false);

  const refresh = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    setIsRefreshing(true);
    setError(null);

    try {
      const result = await callCommand<ReadinessCheckResult>('check_generalized_readiness');
      if (!mountedRef.current || requestId !== requestIdRef.current) {
        return;
      }

      let nextSnapshot = snapshotFromReadinessResult(result);
      setSnapshot(nextSnapshot);

      try {
        const cachedSnapshot = await callCommand<CachedHostReadinessSnapshot | null>(
          'get_cached_host_readiness_snapshot'
        );
        if (!mountedRef.current || requestId !== requestIdRef.current) {
          return;
        }
        if (cachedSnapshot != null) {
          nextSnapshot = cachedSnapshot;
          setSnapshot(nextSnapshot);
        }
      } catch {
        // Cache fetch is ancillary; live result already committed above
      }

      try {
        const nextCapabilities = await callCommand<Capability[]>('get_capabilities');
        if (!mountedRef.current || requestId !== requestIdRef.current) {
          return;
        }
        setCapabilities(nextCapabilities);
      } catch (capabilitiesError) {
        if (!isMissingMockCommandError(capabilitiesError)) {
          throw capabilitiesError;
        }
      }
    } catch (refreshError) {
      if (mountedRef.current && requestId === requestIdRef.current) {
        setError(normalizeError(refreshError));
      }
      throw refreshError;
    } finally {
      if (mountedRef.current && requestId === requestIdRef.current) {
        setIsRefreshing(false);
      }
    }
  }, []);

  const probeTool = useCallback(async (toolId: string) => {
    const normalizedToolId = toolId.trim();
    if (normalizedToolId.length === 0) {
      throw new Error('probeTool requires a non-empty toolId.');
    }

    setError(null);

    try {
      const details = await callCommand<HostToolDetails>('probe_host_tool_details', { toolId: normalizedToolId });
      if (!mountedRef.current) {
        return details;
      }

      setSnapshot((current) => mergeSnapshotDetails(current, details));
      setCapabilities((current) => mergeCapabilityDetails(current, details));

      return details;
    } catch (probeError) {
      if (mountedRef.current) {
        setError(normalizeError(probeError));
      }
      throw probeError;
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;

    const requestId = ++requestIdRef.current;
    setIsRefreshing(true);
    setError(null);

    const run = async () => {
      const [snapshotResult, capabilitiesResult] = await Promise.allSettled([
        callCommand<CachedHostReadinessSnapshot | null>('get_cached_host_readiness_snapshot'),
        callCommand<Capability[]>('get_capabilities'),
      ]);

      if (!mountedRef.current || requestId !== requestIdRef.current) {
        return;
      }

      const loadedSnapshot = snapshotResult.status === 'fulfilled' ? snapshotResult.value : null;
      const loadedCapabilities = capabilitiesResult.status === 'fulfilled' ? capabilitiesResult.value : null;

      if (loadedSnapshot != null) {
        setSnapshot(loadedSnapshot);
      }

      if (loadedCapabilities != null) {
        setCapabilities(loadedCapabilities);
      }

      if (
        loadedSnapshot == null ||
        isSnapshotStale(loadedSnapshot.checked_at) ||
        !hasBootstrappedLiveRefreshRef.current
      ) {
        hasBootstrappedLiveRefreshRef.current = true;
        try {
          await refresh();
        } catch {
          // refresh() already records the user-facing error state
        }
        return;
      }

      let nextError: string | null = null;
      if (snapshotResult.status === 'rejected' && !isMissingMockCommandError(snapshotResult.reason)) {
        nextError = normalizeError(snapshotResult.reason);
      } else if (capabilitiesResult.status === 'rejected' && !isMissingMockCommandError(capabilitiesResult.reason)) {
        nextError = normalizeError(capabilitiesResult.reason);
      }

      setError(nextError);
      setIsRefreshing(false);
    };

    void run();

    return () => {
      mountedRef.current = false;
      requestIdRef.current += 1;
    };
  }, [refresh]);

  const lastCheckedAt = snapshot?.checked_at ?? null;
  const isStale = useMemo(() => isSnapshotStale(lastCheckedAt), [lastCheckedAt]);

  return useMemo(
    () => ({
      snapshot,
      capabilities,
      isStale,
      lastCheckedAt,
      isRefreshing,
      error,
      refresh,
      probeTool,
    }),
    [snapshot, capabilities, isStale, lastCheckedAt, isRefreshing, error, refresh, probeTool]
  );
}
