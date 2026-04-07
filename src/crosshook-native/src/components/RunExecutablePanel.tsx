import { useEffect, useRef, useState } from 'react';

import type { ProtonInstallOption } from './ProfileFormSections';
import { InstallField } from './ui/InstallField';
import { ProtonPathField } from './ui/ProtonPathField';
import { useRunExecutable } from '../hooks/useRunExecutable';
import type { RunExecutableStage } from '../types/run-executable';

const RUNNING_WARNING_ID = 'run-executable-running-warning';

export interface RunExecutablePanelProps {
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
}

function fileNameFromPath(path: string): string {
  const normalized = path.trim().replace(/\\/g, '/');
  return normalized.split('/').pop() || normalized || 'executable';
}

function stageLabel(stage: RunExecutableStage): string {
  switch (stage) {
    case 'preparing':
      return 'Preparing';
    case 'running':
      return 'Running';
    case 'complete':
      return 'Complete';
    case 'failed':
      return 'Failed';
    case 'idle':
    default:
      return 'Idle';
  }
}

export function RunExecutablePanel({ protonInstalls, protonInstallsError }: RunExecutablePanelProps) {
  const {
    request,
    validation,
    stage,
    result,
    error,
    updateField,
    statusText,
    hintText,
    actionLabel,
    canStart,
    isRunning,
    startRun,
    cancelRun,
    stopRun,
    reset,
  } = useRunExecutable();

  const [showConfirmation, setShowConfirmation] = useState(false);
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  const triggerButtonRef = useRef<HTMLButtonElement | null>(null);

  const logPath = result?.helper_log_path ?? '';
  const resolvedPrefixPath = result?.resolved_prefix_path ?? '';
  const heroCopy =
    'Run a one-off .exe or .msi through Proton without saving a profile. Useful for trying trainers, running installers, or one-off maintenance binaries.';
  const prefixHint =
    request.prefix_path.trim().length > 0 ? request.prefix_path.trim() : 'a new throwaway prefix under _run-adhoc/';

  // Modal a11y: Escape closes, focus moves to the destructive-default
  // button (Cancel) on open, and focus returns to the original trigger
  // on close. Cancel is the destructive default so a misfired Enter or
  // Escape never accidentally launches the executable.
  useEffect(() => {
    if (!showConfirmation) {
      return;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        setShowConfirmation(false);
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    cancelButtonRef.current?.focus();
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [showConfirmation]);

  function closeConfirmation() {
    setShowConfirmation(false);
    // Restore focus to the element that opened the dialog so keyboard
    // users do not get dumped at the top of the document.
    triggerButtonRef.current?.focus();
  }

  return (
    <section className="crosshook-install-shell" aria-labelledby="run-executable-heading" data-crosshook-focus-zone>
      <div className="crosshook-install-shell__content">
        <div className="crosshook-install-intro">
          <div className="crosshook-heading-eyebrow">Run EXE/MSI</div>
          <p className="crosshook-heading-copy">{heroCopy}</p>
        </div>

        <div className="crosshook-install-section">
          <div className="crosshook-install-section-title">Executable</div>
          <div className="crosshook-install-grid">
            <InstallField
              label="Executable"
              value={request.executable_path}
              onChange={(value) => updateField('executable_path', value)}
              placeholder="/mnt/media/setup.exe or /mnt/media/installer.msi"
              browseLabel="Browse"
              browseTitle="Select Executable"
              browseFilters={[{ name: 'Windows Executable', extensions: ['exe', 'msi'] }]}
              helpText="Choose any .exe or .msi to run inside the Proton prefix."
              error={validation.fieldErrors.executable_path}
            />
          </div>
        </div>

        <div className="crosshook-install-section">
          <div className="crosshook-install-section-title">Runtime</div>
          <div className="crosshook-install-runtime-stack">
            <ProtonPathField
              value={request.proton_path}
              onChange={(value) => updateField('proton_path', value)}
              error={validation.fieldErrors.proton_path}
              installs={protonInstalls}
              installsError={protonInstallsError}
              idPrefix="run-exec"
            />

            <InstallField
              label="Prefix Path (optional)"
              value={request.prefix_path}
              onChange={(value) => updateField('prefix_path', value)}
              placeholder="Auto: ~/.local/share/crosshook/prefixes/_run-adhoc/<slug>"
              browseLabel="Browse"
              browseMode="directory"
              browseTitle="Select Prefix Directory"
              helpText="Leave empty to auto-create a throwaway prefix under _run-adhoc/."
              error={validation.fieldErrors.prefix_path}
            />

            <InstallField
              label="Working Directory (optional)"
              value={request.working_directory}
              onChange={(value) => updateField('working_directory', value)}
              placeholder="Defaults to the executable's parent directory"
              browseLabel="Browse"
              browseMode="directory"
              browseTitle="Select Working Directory"
              helpText="Optional override for the process current directory."
            />
          </div>
        </div>

        <div className="crosshook-install-review">
          {error ? <p className="crosshook-danger">{error}</p> : null}
          {validation.generalError ? <p className="crosshook-danger">{validation.generalError}</p> : null}
          {statusText ? (
            <p className="crosshook-heading-copy crosshook-install-review__status-copy">{statusText}</p>
          ) : null}
          {hintText ? <p className="crosshook-help-text">{hintText}</p> : null}

          {logPath ? (
            <div className="crosshook-install-candidate crosshook-install-candidate--readonly">
              <span>Run log path</span>
              <span className="crosshook-install-candidate__path-value">{logPath}</span>
            </div>
          ) : null}

          {resolvedPrefixPath ? (
            <div className="crosshook-install-candidate crosshook-install-candidate--readonly">
              <span>Resolved prefix path</span>
              <span className="crosshook-install-candidate__path-value">{resolvedPrefixPath}</span>
            </div>
          ) : null}

          {isRunning ? (
            <p
              id={RUNNING_WARNING_ID}
              className="crosshook-install-shell__running-warning"
              role="status"
              aria-live="polite"
            >
              <strong>Cancel</strong> sends SIGTERM so the executable can clean up. <strong>Stop</strong> force-kills
              the wrapper PID and sweeps every Wine helper that references the active prefix — use it only if the
              executable is wedged.
            </p>
          ) : null}
        </div>
      </div>

      <div className="crosshook-install-shell__footer crosshook-route-footer">
        <div className="crosshook-install-shell__actions crosshook-install-shell__actions--run-executable">
          <button
            ref={triggerButtonRef}
            type="button"
            className="crosshook-button"
            onClick={() => setShowConfirmation(true)}
            disabled={isRunning || !canStart}
          >
            {actionLabel}
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void cancelRun()}
            disabled={!isRunning}
            aria-describedby={isRunning ? RUNNING_WARNING_ID : undefined}
          >
            Cancel
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void stopRun()}
            disabled={!isRunning}
            aria-describedby={isRunning ? RUNNING_WARNING_ID : undefined}
          >
            Stop
          </button>
          <button type="button" className="crosshook-button crosshook-button--secondary" onClick={() => reset()}>
            Reset
          </button>
          <div className="crosshook-install-shell__status-badge">
            <div className="crosshook-install-stage">{stageLabel(stage)}</div>
          </div>
        </div>
      </div>

      {showConfirmation && (
        <div
          className="crosshook-modal-overlay"
          role="dialog"
          aria-modal="true"
          aria-labelledby="run-executable-confirm-title"
          aria-describedby="run-executable-confirm-body"
          onClick={closeConfirmation}
        >
          <div className="crosshook-modal-dialog" onClick={(event) => event.stopPropagation()}>
            <h4 id="run-executable-confirm-title">Run {fileNameFromPath(request.executable_path)} through Proton?</h4>
            <p id="run-executable-confirm-body">
              This will spawn the executable inside {prefixHint} and stream its output to the console drawer.
            </p>
            <div className="crosshook-modal-actions">
              <button
                ref={cancelButtonRef}
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={closeConfirmation}
              >
                Cancel
              </button>
              <button
                type="button"
                className="crosshook-button"
                onClick={() => {
                  closeConfirmation();
                  void startRun();
                }}
              >
                Run
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

export default RunExecutablePanel;
