import { useMetadataStoreStatus } from '@/hooks/useMetadataStoreStatus';
import type { MetadataStoreStatus } from '@/types/metadata';

type DegradedStatus = Exclude<MetadataStoreStatus, { state: 'ok' }>;

const MAX_REASON_LENGTH = 200;

function clampReason(reason: string): string {
  return reason.length > MAX_REASON_LENGTH ? `${reason.slice(0, MAX_REASON_LENGTH - 1)}…` : reason;
}

/** D6: TOML saves still succeed while metadata is degraded; only advisory snapshots are skipped. */
const SAVE_NOTICE = 'Profile files still save, but configuration snapshots are not recorded.';

function StatusMessage({ status }: { status: DegradedStatus }) {
  if (status.state === 'newer_schema') {
    return (
      <span>
        This data was written by a newer CrossHook (schema v{status.found}). History, health and catalogs are read-only
        until you update CrossHook. {SAVE_NOTICE}
      </span>
    );
  }
  return (
    <span>
      Metadata storage is unavailable, so history, health and catalogs are limited. {SAVE_NOTICE} Reason:{' '}
      {clampReason(status.reason)}
    </span>
  );
}

/** App-level, session-dismissible banner shown when the metadata database is degraded. */
export function MetadataStatusBanner() {
  const { status, dismiss } = useMetadataStoreStatus();
  if (status === null) return null;

  return (
    <div
      className="crosshook-warning-banner crosshook-metadata-status-banner"
      role="status"
      aria-label="Metadata storage warning"
    >
      <StatusMessage status={status} />
      <button
        type="button"
        className="crosshook-rename-toast-dismiss"
        aria-label="Dismiss metadata warning"
        onClick={dismiss}
      >
        ×
      </button>
    </div>
  );
}

export default MetadataStatusBanner;
