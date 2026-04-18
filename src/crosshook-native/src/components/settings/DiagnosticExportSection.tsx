import { useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { DiagnosticBundleResult } from '../../types';
import { chooseDirectory } from '../../utils/dialog';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { truncatePath } from './format';

/** Collapsible section for exporting a diagnostic bundle for bug reports. */
export function DiagnosticExportSection() {
  const [isExporting, setIsExporting] = useState(false);
  const [redactPaths, setRedactPaths] = useState(true);
  const [useDefaultLocation, setUseDefaultLocation] = useState(true);
  const [customDir, setCustomDir] = useState<string | null>(null);
  const [result, setResult] = useState<DiagnosticBundleResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleBrowse() {
    const selected = await chooseDirectory('Choose export location');
    if (selected) {
      setCustomDir(selected);
    }
  }

  async function handleExport() {
    const outputDir = useDefaultLocation ? null : customDir;
    if (!useDefaultLocation && !outputDir) {
      setError('Choose a directory first or use the default location.');
      return;
    }

    setIsExporting(true);
    setError(null);
    setResult(null);
    try {
      const bundleResult = await callCommand<DiagnosticBundleResult>('export_diagnostics', {
        redactPaths,
        outputDir,
      });
      setResult(bundleResult);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsExporting(false);
    }
  }

  return (
    <CollapsibleSection
      title="Diagnostic Export"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">Bug reports and troubleshooting</span>}
    >
      <p className="crosshook-muted crosshook-settings-help">
        Export a diagnostic bundle containing system info, profiles, logs, and Steam diagnostics as a single .tar.gz
        archive. Attach this to GitHub issues for faster troubleshooting.
      </p>

      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={redactPaths}
          onChange={(event) => setRedactPaths(event.target.checked)}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Redact home directory paths</span>
          <p className="crosshook-muted crosshook-settings-note">
            Replaces your home directory with ~ in profile configs and settings before bundling.
          </p>
        </span>
      </label>

      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={useDefaultLocation}
          onChange={(event) => setUseDefaultLocation(event.target.checked)}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Use default location</span>
          <p className="crosshook-muted crosshook-settings-note">Save the bundle to the system temp directory.</p>
        </span>
      </label>

      {!useDefaultLocation ? (
        <div className="crosshook-settings-field-row">
          <span className="crosshook-label" id="settings-export-directory-label">
            Export directory
          </span>
          <div className="crosshook-settings-input-row">
            <input
              className="crosshook-input"
              value={customDir ?? ''}
              readOnly
              placeholder="No directory selected"
              aria-labelledby="settings-export-directory-label"
            />
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              onClick={() => void handleBrowse()}
            >
              Browse
            </button>
          </div>
        </div>
      ) : null}

      <div className="crosshook-settings-clear-row">
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          disabled={isExporting || (!useDefaultLocation && !customDir)}
          onClick={() => void handleExport()}
        >
          {isExporting ? 'Exporting...' : 'Export Diagnostic Bundle'}
        </button>
      </div>

      {result ? (
        <div className="crosshook-settings-help" style={{ marginTop: 8 }}>
          <p>
            <strong>Bundle exported:</strong>{' '}
            <span className="crosshook-muted" title={result.archive_path}>
              {truncatePath(result.archive_path)}
            </span>
          </p>
          <p className="crosshook-muted">
            {result.summary.profile_count} profile{result.summary.profile_count !== 1 ? 's' : ''},{' '}
            {result.summary.log_file_count} log file{result.summary.log_file_count !== 1 ? 's' : ''},{' '}
            {result.summary.proton_install_count} Proton version
            {result.summary.proton_install_count !== 1 ? 's' : ''}
          </p>
        </div>
      ) : null}

      {error ? (
        <p className="crosshook-danger crosshook-settings-error" style={{ marginTop: 8 }}>
          {error}
        </p>
      ) : null}
    </CollapsibleSection>
  );
}
