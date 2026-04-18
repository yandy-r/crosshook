import { useEffect, useState } from 'react';
import { useLauncherManagement } from '../../hooks/useLauncherManagement';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { truncatePath } from './format';

interface ManageLaunchersSectionProps {
  targetHomePath: string;
  steamClientInstallPath: string;
}

/** Collapsible section for listing, re-exporting, and deleting launcher desktop entries. */
export function ManageLaunchersSection({ targetHomePath, steamClientInstallPath }: ManageLaunchersSectionProps) {
  const [confirmSlug, setConfirmSlug] = useState<string | null>(null);
  const {
    launchers,
    error,
    isListing,
    deletingSlug,
    reexportingSlug,
    listLaunchers,
    deleteLauncher,
    reexportLauncher,
  } = useLauncherManagement({
    targetHomePath,
    steamClientInstallPath,
  });

  useEffect(() => {
    void listLaunchers();
  }, [listLaunchers]);

  async function handleDelete(slug: string) {
    const deleted = await deleteLauncher(slug);
    if (deleted) {
      setConfirmSlug(null);
    }
  }

  async function handleReexport(slug: string) {
    await reexportLauncher(slug);
  }

  if (launchers.length === 0 && !error) {
    return null;
  }

  return (
    <CollapsibleSection
      title="Manage Launchers"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={
        <>
          <span className="crosshook-muted">
            {launchers.length} launcher{launchers.length !== 1 ? 's' : ''} on disk
          </span>
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small crosshook-settings-small-button"
            onClick={(event) => {
              event.preventDefault();
              void listLaunchers();
            }}
          >
            {isListing ? 'Refreshing...' : 'Refresh'}
          </button>
        </>
      }
    >
      {error ? <p className="crosshook-danger crosshook-settings-error">{error}</p> : null}

      <ul className="crosshook-recent-list">
        {launchers.map((launcher) => (
          <li key={launcher.launcher_slug} className="crosshook-recent-item">
            <div className="crosshook-settings-launcher-row">
              <div>
                <div className="crosshook-recent-item__label crosshook-settings-launcher-label">
                  {launcher.launcher_slug}
                  {launcher.is_stale ? (
                    <span className="crosshook-health-chip crosshook-health-chip--warning" style={{ marginLeft: 8 }}>
                      Stale
                    </span>
                  ) : null}
                </div>
                <div className="crosshook-recent-item__label crosshook-settings-launcher-path">
                  {launcher.script_exists ? truncatePath(launcher.script_path) : null}
                  {launcher.script_exists && launcher.desktop_entry_exists ? ' | ' : null}
                  {launcher.desktop_entry_exists ? truncatePath(launcher.desktop_entry_path) : null}
                </div>
              </div>
              <div className="crosshook-settings-launcher-actions">
                {launcher.is_stale ? (
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--warning crosshook-settings-small-button"
                    disabled={reexportingSlug === launcher.launcher_slug}
                    onClick={() => void handleReexport(launcher.launcher_slug)}
                  >
                    {reexportingSlug === launcher.launcher_slug ? 'Re-exporting...' : 'Re-export'}
                  </button>
                ) : null}
                {confirmSlug === launcher.launcher_slug ? (
                  <>
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--danger crosshook-settings-small-button"
                      disabled={deletingSlug === launcher.launcher_slug || reexportingSlug === launcher.launcher_slug}
                      onClick={() => void handleDelete(launcher.launcher_slug)}
                    >
                      {deletingSlug === launcher.launcher_slug ? 'Deleting...' : 'Confirm'}
                    </button>
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small crosshook-settings-small-button"
                      onClick={() => setConfirmSlug(null)}
                    >
                      Cancel
                    </button>
                  </>
                ) : (
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small crosshook-settings-small-button"
                    onClick={() => setConfirmSlug(launcher.launcher_slug)}
                  >
                    Delete
                  </button>
                )}
              </div>
            </div>
          </li>
        ))}
      </ul>
    </CollapsibleSection>
  );
}
