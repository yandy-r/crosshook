import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';

import type { DepState, PrefixDependencyStatus } from '../types/prefix-deps';
import { usePrefixDeps } from '../hooks/usePrefixDeps';

interface PrefixDepsPanelProps {
  profileName: string;
  prefixPath: string;
  requiredPackages: string[];
}

function stateLabel(state: DepState): string {
  switch (state) {
    case 'installed': return 'Installed';
    case 'missing': return 'Missing';
    case 'install_failed': return 'Failed';
    case 'check_failed': return 'Check Failed';
    case 'user_skipped': return 'Skipped';
    default: return 'Unknown';
  }
}

function stateModifier(state: DepState): string {
  switch (state) {
    case 'installed': return 'success';
    case 'missing': return 'warning';
    case 'install_failed': return 'danger';
    case 'user_skipped': return 'muted';
    default: return 'muted';
  }
}

/** Renders a status chip for a single dependency package. */
function DependencyStatusBadge({ dep }: { dep: PrefixDependencyStatus }) {
  return (
    <span
      className={`crosshook-status-chip crosshook-status-chip--${stateModifier(dep.state)}`}
      title={dep.last_error ?? undefined}
    >
      {dep.package_name}: {stateLabel(dep.state)}
    </span>
  );
}

export function PrefixDepsPanel({
  profileName,
  prefixPath,
  requiredPackages,
}: PrefixDepsPanelProps) {
  const { deps, loading, error, checkDeps, installDep, reload } = usePrefixDeps(
    profileName,
    prefixPath,
  );
  const [installing, setInstalling] = useState(false);
  const [confirmInstall, setConfirmInstall] = useState<string[] | null>(null);
  const [logLines, setLogLines] = useState<string[]>([]);

  // Merge required packages with cached status
  const packageStatuses: PrefixDependencyStatus[] = requiredPackages.map((pkg) => {
    const cached = deps.find((d) => d.package_name === pkg);
    return cached ?? {
      package_name: pkg,
      state: 'unknown' as DepState,
      checked_at: null,
      installed_at: null,
      last_error: null,
    };
  });

  const missingPackages = packageStatuses
    .filter((d) => d.state === 'missing' || d.state === 'install_failed')
    .map((d) => d.package_name);

  // Listen for install events
  useEffect(() => {
    const unlistenLog = listen<{ line: string }>('prefix-dep-log', (event) => {
      setLogLines((prev) => [...prev.slice(-200), event.payload.line]);
    });

    const unlistenComplete = listen<{ succeeded: boolean; exit_code: number | null }>(
      'prefix-dep-complete',
      (event) => {
        setInstalling(false);
        if (event.payload.succeeded) {
          reload();
        }
      },
    );

    return () => {
      void unlistenLog.then((fn) => fn());
      void unlistenComplete.then((fn) => fn());
    };
  }, [reload]);

  const handleCheck = useCallback(() => {
    void checkDeps(requiredPackages);
  }, [checkDeps, requiredPackages]);

  const handleInstallConfirm = useCallback(() => {
    if (!confirmInstall) return;
    setInstalling(true);
    setLogLines([]);
    void installDep(confirmInstall);
    setConfirmInstall(null);
  }, [confirmInstall, installDep]);

  const handleInstallAll = useCallback(() => {
    if (missingPackages.length === 0) return;
    setConfirmInstall(missingPackages);
  }, [missingPackages]);

  const handleInstallSingle = useCallback(
    (pkg: string) => {
      setConfirmInstall([pkg]);
    },
    [],
  );

  if (requiredPackages.length === 0) return null;

  return (
    <div className="crosshook-prefix-deps">
      {/* Package list */}
      <div className="crosshook-prefix-deps__list">
        {packageStatuses.map((dep) => (
          <div key={dep.package_name} className="crosshook-prefix-deps__item">
            <DependencyStatusBadge dep={dep} />
            {(dep.state === 'missing' || dep.state === 'install_failed') && !installing ? (
              <button
                type="button"
                className="crosshook-button crosshook-button--small"
                onClick={() => handleInstallSingle(dep.package_name)}
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
          disabled={loading || installing}
        >
          {loading ? 'Checking...' : 'Check Now'}
        </button>
        {missingPackages.length > 0 ? (
          <button
            type="button"
            className="crosshook-button"
            onClick={handleInstallAll}
            disabled={installing}
          >
            Install All Missing ({missingPackages.length})
          </button>
        ) : null}
      </div>

      {/* Error display */}
      {error ? (
        <p className="crosshook-danger" style={{ margin: '8px 0 0' }}>
          {error}
        </p>
      ) : null}

      {/* Install log output */}
      {(installing || logLines.length > 0) ? (
        <div className="crosshook-prefix-deps__log">
          <div className="crosshook-prefix-deps__log-header">
            <strong>{installing ? 'Installing...' : 'Install Log'}</strong>
          </div>
          <pre className="crosshook-prefix-deps__log-output">
            {logLines.join('\n') || (installing ? 'Waiting for output...' : '')}
          </pre>
        </div>
      ) : null}

      {/* Confirmation modal */}
      {confirmInstall !== null ? (
        <div className="crosshook-modal-overlay" role="dialog" aria-modal="true">
          <div className="crosshook-modal crosshook-prefix-deps__confirm">
            <h3>Install Prefix Dependencies</h3>
            <p>The following packages will be installed:</p>
            <ul>
              {confirmInstall.map((pkg) => (
                <li key={pkg}>{pkg}</li>
              ))}
            </ul>
            <p className="crosshook-help-text">
              Installation may take several minutes and requires internet access.
              Do not close CrossHook during installation.
            </p>
            <div className="crosshook-modal__actions">
              <button
                type="button"
                className="crosshook-button"
                onClick={handleInstallConfirm}
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
    </div>
  );
}

export default PrefixDepsPanel;
