import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

export type MetadataStoreStatus =
  | { state: 'ok' }
  | { state: 'newer_schema'; found: number; supported: number }
  | { state: 'disabled'; reason: string };

// sessionStorage: resets on window restart so the banner re-surfaces until dismissed again.
// Keyed by state (plus the found schema version for newer_schema) so a different degraded
// state or a different schema version is not hidden by an earlier dismissal.
export const METADATA_STATUS_DISMISS_KEY_PREFIX = 'crosshook.metadata.status.dismissed.';

type DegradedStatus = Exclude<MetadataStoreStatus, { state: 'ok' }>;

function dismissKey(status: DegradedStatus): string {
  return status.state === 'newer_schema' ? `newer_schema:${status.found}` : status.state;
}

function readDismissed(key: string): boolean {
  try {
    return sessionStorage.getItem(`${METADATA_STATUS_DISMISS_KEY_PREFIX}${key}`) === '1';
  } catch {
    return false;
  }
}

/** Runtime guard: the IPC payload is untrusted; anything off-contract is a failure. */
function isValidSchemaVersion(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && Number.isInteger(value) && value >= 0;
}

function parseStatus(payload: unknown): MetadataStoreStatus | null {
  if (typeof payload !== 'object' || payload === null) return null;
  const { state } = payload as { state?: unknown };
  if (state === 'ok') return { state: 'ok' };
  if (state === 'newer_schema') {
    const { found, supported } = payload as { found?: unknown; supported?: unknown };
    if (isValidSchemaVersion(found) && isValidSchemaVersion(supported)) {
      return { state, found, supported };
    }
    return null;
  }
  if (state === 'disabled') {
    const { reason } = payload as { reason?: unknown };
    if (typeof reason === 'string' && reason.length > 0) return { state, reason };
    return null;
  }
  return null;
}

export interface UseMetadataStoreStatusResult {
  /** Degraded status to show; null means render nothing (ok, failed, loading, or dismissed). */
  status: Exclude<MetadataStoreStatus, { state: 'ok' }> | null;
  dismiss: () => void;
}

export function useMetadataStoreStatus(): UseMetadataStoreStatusResult {
  const [status, setStatus] = useState<UseMetadataStoreStatusResult['status']>(null);

  useEffect(() => {
    let active = true;
    callCommand<unknown>('metadata_store_status')
      .then((result) => {
        if (!active) return;
        const parsed = parseStatus(result);
        if (parsed === null) {
          console.error('metadata_store_status returned an invalid payload', result);
          return;
        }
        if (parsed.state === 'ok' || readDismissed(dismissKey(parsed))) return;
        setStatus(parsed);
      })
      .catch((error: unknown) => {
        console.error('metadata_store_status failed', error);
      });
    return () => {
      active = false;
    };
  }, []);

  const dismiss = useCallback(() => {
    setStatus((current) => {
      if (current) {
        try {
          sessionStorage.setItem(`${METADATA_STATUS_DISMISS_KEY_PREFIX}${dismissKey(current)}`, '1');
        } catch {
          // Ignore storage errors in restricted environments.
        }
      }
      return null;
    });
  }, []);

  return { status, dismiss };
}
