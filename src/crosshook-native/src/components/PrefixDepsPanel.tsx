import { useCallback, useEffect, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { usePrefixDeps } from '../hooks/usePrefixDeps';
import type { DepState, PrefixDepCompletePayload, PrefixDependencyStatus } from '../types/prefix-deps';

interface PrefixDepsPanelProps {
  profileName: string;
  prefixPath: string;
  requiredPackages: string[];
}

function stateLabel(state: DepState): string {
  switch (state) {
    case 'installed':
      return 'Installed';
    case 'missing':
      return 'Missing';
    case 'install_failed':
      return 'Failed';
    case 'check_failed':
      return 'Check Failed';
    case 'user_skipped':
      return 'Skipped';
    default:
      return 'Unknown';
  }
}

function stateModifier(state: DepState): string {
  switch (state) {
    case 'installed':
      return 'success';
    case 'missing':
      return 'warning';
    case 'install_failed':
      return 'danger';
    case 'user_skipped':
      return 'muted';
    default:
      return 'muted';
  }
}

/** Renders a status chip for a single dependency package. */
function DependencyStatusBadge({ dep }: { dep: PrefixDependencyStatus }) {
  return (
    <span
      className={`crosshook-status-chip crosshook-status-chip--${stateModifier(dep.state)}`}
      title={dep.last_error ?? undefined}
      role="status"
      aria-label={`${dep.package_name}: ${stateLabel(dep.state)}`}
    >
      {dep.package_name}: {stateLabel(dep.state)}
    </span>
  );
}

export function PrefixDepsPanel({ profileName, prefixPath, requiredPackages }: PrefixDepsPanelProps) {
  const {
    deps,
    repairStatus,
    repairStatusKnown,
    repairStatusError,
    loading,
    repairing,
    error,
    checkDeps,
    installDep,
    repairPrefixVersion,
    reload,
  } = usePrefixDeps(profileName, prefixPath);
  const [installing, setInstalling] = useState(false);
  const [confirmInstall, setConfirmInstall] = useState<string[] | null>(null);
  const [logLines, setLogLines] = useState<string[]>([]);

  // Merge required packages with cached status
  const packageStatuses: PrefixDependencyStatus[] = requiredPackages.map((pkg) => {
    const cached = deps.find((d) => d.package_name === pkg);
    return (
      cached ?? {
        package_name: pkg,
        state: 'unknown' as DepState,
        checked_at: null,
        installed_at: null,
        last_error: null,
      }
    );
  });

  const missingPackages = packageStatuses
    .filter((d) => d.state === 'missing' || d.state === 'install_failed')
    .map((d) => d.package_name);
  const dependencyActionsBlocked =
    repairStatus.required || repairing || !repairStatusKnown || repairStatusError !== null;

  // Listen for install events
  useEffect(() => {
    const unlistenLog = subscribeEvent<{ profile_name: string; prefix_path: string; line: string }>(
      'prefix-dep-log',
      (event) => {
        if (event.payload.profile_name !== profileName || event.payload.prefix_path !== prefixPath) {
          return;
        }
        setLogLines((prev) => [...prev.slice(-200), event.payload.line]);
      }
    );

    const unlistenComplete = subscribeEvent<PrefixDepCompletePayload>('prefix-dep-complete', (event) => {
      if (event.payload.profile_name !== profileName || event.payload.prefix_path !== prefixPath) {
        return;
      }
      setInstalling(false);
      void reload();
    });

    return () => {
      void unlistenLog.then((fn) => fn());
      void unlistenComplete.then((fn) => fn());
    };
  }, [profileName, prefixPath, reload]);

  const handleCheck = useCallback(() => {
    void (async () => {
      await checkDeps(requiredPackages);
      await reload();
    })();
  }, [checkDeps, reload, requiredPackages]);

  const handleInstallConfirm = useCallback(async () => {
    if (!confirmInstall || dependencyActionsBlocked) return;
    setInstalling(true);
    setLogLines([]);
    let installStarted = false;
    try {
      await installDep(confirmInstall);
      installStarted = true;
    } catch (_error) {
      // usePrefixDeps already stores a user-facing error message.
    } finally {
      if (!installStarted) {
        setInstalling(false);
      }
      setConfirmInstall(null);
    }
  }, [confirmInstall, dependencyActionsBlocked, installDep]);

  const handleInstallAll = useCallback(() => {
    if (missingPackages.length === 0) return;
    setConfirmInstall(missingPackages);
  }, [missingPackages]);

  const handleInstallSingle = useCallback((pkg: string) => {
    setConfirmInstall([pkg]);
  }, []);

  const handleRepair = useCallback(async () => {
    try {
      await repairPrefixVersion();
    } catch {
      // usePrefixDeps stores the user-facing error and keeps repair required.
    }
  }, [repairPrefixVersion]);

  if (requiredPackages.length === 0 && !repairStatus.required) {
    return (
      <section aria-label="Prefix dependencies" className="crosshook-prefix-deps">
        <p className="crosshook-help-text" role="status">
          No prefix dependencies are declared for this profile.
        </p>
        {error ? (
          <p className="crosshook-danger" role="alert" aria-live="assertive">
            {error}
          </p>
        ) : null}
      </section>
    );
  }

  return (
    <section aria-label="Prefix dependencies" className="crosshook-prefix-deps">
      {repairStatus.required ? (
        <div className="crosshook-error-banner" role="alert" aria-live="assertive">
          <strong>Prefix repair required</strong>
          <p>
            CrossHook must restore this prefix&apos;s Windows compatibility version before the game or trainer can
            launch.
          </p>
          {repairStatus.last_error ? <p>Last repair attempt: {repairStatus.last_error}</p> : null}
          <button
            type="button"
            className="crosshook-button crosshook-button--danger"
            onClick={() => {
              void handleRepair();
            }}
            disabled={repairing}
          >
            {repairing ? 'Repairing...' : 'Repair Prefix'}
          </button>
        </div>
      ) : null}

      {/* Package list */}
      <div className="crosshook-prefix-deps__list" aria-live="polite">
        {packageStatuses.map((dep) => (
          <div key={dep.package_name} className="crosshook-prefix-deps__item">
            <DependencyStatusBadge dep={dep} />
            {(dep.state === 'missing' || dep.state === 'install_failed') && !installing ? (
              <button
                type="button"
                className="crosshook-button crosshook-button--small"
                onClick={() => handleInstallSingle(dep.package_name)}
                disabled={dependencyActionsBlocked}
              >
                {dep.state === 'install_failed' ? 'Retry' : 'Install'}
              </button>
            ) : null}
          </div>
        ))}
      </div>

      {/* Action buttons */}
      <div className="crosshook-prefix-deps__actions">
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={handleCheck}
          disabled={loading || installing || repairing || repairStatus.required}
          aria-disabled={loading || installing || repairing || repairStatus.required}
        >
          {loading ? 'Checking...' : 'Check Now'}
        </button>
        {missingPackages.length > 0 ? (
          <button
            type="button"
            className="crosshook-button"
            onClick={handleInstallAll}
            disabled={installing || dependencyActionsBlocked}
            aria-disabled={installing || dependencyActionsBlocked}
          >
            Install All Missing ({missingPackages.length})
          </button>
        ) : null}
      </div>

      {/* Error display */}
      {error ? (
        <p className="crosshook-danger" role="alert" aria-live="assertive" style={{ margin: '8px 0 0' }}>
          {error}
        </p>
      ) : null}

      {/* Install progress indicator */}
      {installing ? <progress aria-label="Dependency installation in progress" /> : null}

      {/* Install log output */}
      {installing || logLines.length > 0 ? (
        <div className="crosshook-prefix-deps__log">
          <div className="crosshook-prefix-deps__log-header">
            <strong>{installing ? 'Installing...' : 'Install Log'}</strong>
          </div>
          <pre className="crosshook-prefix-deps__log-output" aria-live="polite" aria-busy={installing}>
            {logLines.join('\n') || (installing ? 'Waiting for output...' : '')}
          </pre>
        </div>
      ) : null}

      {/* Confirmation modal */}
      {confirmInstall !== null ? (
        <div
          className="crosshook-modal-overlay"
          role="dialog"
          aria-modal="true"
          aria-labelledby="prefix-deps-confirm-title"
        >
          <div className="crosshook-modal crosshook-prefix-deps__confirm">
            <h3 id="prefix-deps-confirm-title">Install Prefix Dependencies</h3>
            <p>The following packages will be installed:</p>
            <ul>
              {confirmInstall.map((pkg) => (
                <li key={pkg}>{pkg}</li>
              ))}
            </ul>
            <p className="crosshook-help-text">
              Installation may take several minutes and requires internet access. Do not close CrossHook during
              installation.
            </p>
            <div className="crosshook-modal__actions">
              <button
                type="button"
                className="crosshook-button"
                onClick={() => {
                  void handleInstallConfirm();
                }}
                disabled={dependencyActionsBlocked}
              >
                Install
              </button>
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={() => setConfirmInstall(null)}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </section>
  );
}

export default PrefixDepsPanel;
