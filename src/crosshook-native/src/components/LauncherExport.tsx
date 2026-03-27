import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type {
  GameProfile,
  LaunchMethod,
  LauncherDeleteResult,
  LauncherInfo,
  TrainerLoadingMode,
} from '../types';
import { LauncherPreviewModal } from './LauncherPreviewModal';

interface LauncherExportProps {
  profile: GameProfile;
  method: Exclude<LaunchMethod, ''>;
  steamClientInstallPath: string;
  targetHomePath: string;
  pendingReExport?: boolean;
  onReExportHandled?: () => void;
}

interface SteamExternalLauncherExportRequest {
  method: string;
  launcher_name: string;
  trainer_path: string;
  trainer_loading_mode: TrainerLoadingMode;
  launcher_icon_path: string;
  prefix_path: string;
  proton_path: string;
  steam_app_id: string;
  steam_client_install_path: string;
  target_home_path: string;
}

interface SteamExternalLauncherExportResult {
  display_name: string;
  launcher_slug: string;
  script_path: string;
  desktop_entry_path: string;
}

const automaticLauncherSuffix = ' - Trainer';
const launcherNameHelperText =
  'CrossHook appends " - Trainer" to the exported launcher title. Enter only the base launcher name here.';

function safeTrim(value: string | undefined | null): string {
  return value?.trim() ?? '';
}

function stripAutomaticLauncherSuffix(value: string): string {
  const trimmed = value.trim();
  return trimmed.endsWith(automaticLauncherSuffix)
    ? trimmed.slice(0, -automaticLauncherSuffix.length).trimEnd()
    : trimmed;
}

function collectDeleteWarnings(result: LauncherDeleteResult): string[] {
  return [result.script_skipped_reason, result.desktop_entry_skipped_reason].filter(
    (value): value is string => typeof value === 'string' && value.trim().length > 0
  );
}

function deriveLauncherName(profile: GameProfile): string {
  const explicitName = stripAutomaticLauncherSuffix(safeTrim(profile.steam.launcher.display_name));
  if (explicitName) {
    return explicitName;
  }

  const gameName = safeTrim(profile.game.name);
  if (gameName) {
    return gameName;
  }

  const trainerStem = stripAutomaticLauncherSuffix(safeTrim(profile.trainer.path)
    .split(/[\\/]/)
    .pop()
    ?.replace(/\.[^.]+$/, '')
    .trim() ?? '');
  if (trainerStem) {
    return trainerStem;
  }

  const steamAppId = safeTrim(profile.steam.app_id);
  if (steamAppId) {
    return `steam-${steamAppId}-trainer`;
  }

  return 'crosshook-trainer';
}

function buildExportRequest(
  profile: GameProfile,
  method: Exclude<LaunchMethod, ''>,
  launcherName: string,
  launcherIconPath: string,
  steamClientInstallPath: string,
  targetHomePath: string
): SteamExternalLauncherExportRequest {
  return {
    method,
    launcher_name: launcherName.trim(),
    trainer_path: profile.trainer.path.trim(),
    trainer_loading_mode: profile.trainer.loading_mode,
    launcher_icon_path: launcherIconPath.trim(),
    prefix_path:
      method === 'steam_applaunch' ? profile.steam.compatdata_path.trim() : profile.runtime.prefix_path.trim(),
    proton_path:
      method === 'steam_applaunch' ? profile.steam.proton_path.trim() : profile.runtime.proton_path.trim(),
    steam_app_id: profile.steam.app_id.trim(),
    steam_client_install_path: steamClientInstallPath.trim(),
    target_home_path: targetHomePath.trim(),
  };
}

export function LauncherExport({
  profile,
  method,
  steamClientInstallPath,
  targetHomePath,
  pendingReExport,
  onReExportHandled,
}: LauncherExportProps) {
  const [launcherName, setLauncherName] = useState(() => deriveLauncherName(profile));
  const [isExporting, setIsExporting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [result, setResult] = useState<SteamExternalLauncherExportResult | null>(null);
  const [launcherStatus, setLauncherStatus] = useState<LauncherInfo | null>(null);
  const [deleteConfirming, setDeleteConfirming] = useState(false);
  const deleteTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [showLauncherPreview, setShowLauncherPreview] = useState(false);
  const [previewScriptContent, setPreviewScriptContent] = useState('');
  const [previewDesktopContent, setPreviewDesktopContent] = useState('');
  const [previewLoading, setPreviewLoading] = useState(false);

  const request = useMemo(
    () =>
      buildExportRequest(
        profile,
        method,
        launcherName,
        safeTrim(profile.steam.launcher.icon_path),
        steamClientInstallPath,
        targetHomePath
      ),
    [profile, method, launcherName, steamClientInstallPath, targetHomePath]
  );

  const refreshLauncherStatus = useCallback(async () => {
    try {
      const info = await invoke<LauncherInfo>('check_launcher_exists', { request });
      setLauncherStatus(info);
    } catch (error) {
      console.error('Failed to refresh launcher status.', error);
      setErrorMessage(`Failed to check launcher status: ${error instanceof Error ? error.message : String(error)}`);
      setLauncherStatus(null);
    }
  }, [request]);

  useEffect(() => {
    setLauncherName(deriveLauncherName(profile));
  }, [profile]);

  useEffect(() => {
    void refreshLauncherStatus();
  }, [refreshLauncherStatus]);

  useEffect(() => {
    return () => {
      if (deleteTimeoutRef.current !== null) {
        clearTimeout(deleteTimeoutRef.current);
      }
    };
  }, []);

  // Auto re-export after profile rename: wait briefly for request to settle, then export.
  useEffect(() => {
    if (!pendingReExport) return;

    const timer = setTimeout(() => {
      void (async () => {
        try {
          await invoke<void>('validate_launcher_export', { request });
          await invoke<SteamExternalLauncherExportResult>('export_launchers', { request });
          void refreshLauncherStatus();
        } catch {
          // Silent — user can manually re-export if auto-export fails
        } finally {
          onReExportHandled?.();
        }
      })();
    }, 150);

    return () => clearTimeout(timer);
  }, [pendingReExport, request, refreshLauncherStatus, onReExportHandled]);

  const metadataRows = useMemo(
    () =>
      method === 'steam_applaunch'
        ? [
            { label: 'Trainer Path', value: safeTrim(profile.trainer.path) || 'Not set' },
            {
              label: 'Trainer Loading Mode',
              value: profile.trainer.loading_mode === 'copy_to_prefix' ? 'Copy into prefix' : 'Run from current directory',
            },
            { label: 'Steam App ID', value: safeTrim(profile.steam.app_id) || 'Not set' },
            { label: 'Compatdata Path', value: safeTrim(profile.steam.compatdata_path) || 'Not set' },
            { label: 'Proton Path', value: safeTrim(profile.steam.proton_path) || 'Not set' },
          ]
        : [
            { label: 'Trainer Path', value: safeTrim(profile.trainer.path) || 'Not set' },
            {
              label: 'Trainer Loading Mode',
              value: profile.trainer.loading_mode === 'copy_to_prefix' ? 'Copy into prefix' : 'Run from current directory',
            },
            { label: 'Prefix Path', value: safeTrim(profile.runtime.prefix_path) || 'Not set' },
            { label: 'Proton Path', value: safeTrim(profile.runtime.proton_path) || 'Not set' },
            { label: 'Working Directory', value: safeTrim(profile.runtime.working_directory) || 'Not set' },
          ],
    [method, profile],
  );

  const canExport =
    request.trainer_path.length > 0 &&
    request.prefix_path.length > 0 &&
    request.proton_path.length > 0 &&
    (method !== 'steam_applaunch' || request.steam_app_id.length > 0) &&
    !isExporting;

  const showDeleteButton = launcherStatus?.script_exists || launcherStatus?.desktop_entry_exists;
  const launcherStatusTone = launcherStatus
    ? launcherStatus.is_stale
      ? 'warning'
      : launcherStatus.script_exists && launcherStatus.desktop_entry_exists
      ? 'success'
      : !launcherStatus.script_exists && !launcherStatus.desktop_entry_exists
      ? 'neutral'
      : 'warning'
    : null;
  const launcherStatusLabel = launcherStatus
    ? launcherStatus.is_stale
      ? 'Stale'
      : launcherStatus.script_exists && launcherStatus.desktop_entry_exists
      ? 'Exported'
      : !launcherStatus.script_exists && !launcherStatus.desktop_entry_exists
      ? 'Not Exported'
      : 'Partial'
    : '';
  const launcherStatusMessage = launcherStatus?.is_stale
    ? 'Launcher files are out of date with the current profile.'
    : launcherStatus?.script_exists && launcherStatus.desktop_entry_exists
    ? 'Launcher files are exported and up to date.'
    : !launcherStatus?.script_exists && !launcherStatus?.desktop_entry_exists
    ? 'No launcher files are currently exported.'
    : 'Only one launcher file exists for this profile.';

  async function handleExport() {
    setIsExporting(true);
    setErrorMessage(null);
    setStatusMessage(null);
    setResult(null);

    try {
      await invoke<void>('validate_launcher_export', { request });
      const exported = await invoke<SteamExternalLauncherExportResult>('export_launchers', { request });
      setResult(exported);
      setStatusMessage('Launcher export completed.');
      void refreshLauncherStatus();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsExporting(false);
    }
  }

  function handleDeleteClick() {
    if (deleteConfirming) {
      if (deleteTimeoutRef.current !== null) {
        clearTimeout(deleteTimeoutRef.current);
        deleteTimeoutRef.current = null;
      }
      setDeleteConfirming(false);
      void handleDeleteLauncher();
    } else {
      setDeleteConfirming(true);
      deleteTimeoutRef.current = setTimeout(() => {
        setDeleteConfirming(false);
        deleteTimeoutRef.current = null;
      }, 3000);
    }
  }

  function handleDeleteBlur() {
    if (deleteConfirming) {
      if (deleteTimeoutRef.current !== null) {
        clearTimeout(deleteTimeoutRef.current);
        deleteTimeoutRef.current = null;
      }
      setDeleteConfirming(false);
    }
  }

  async function handlePreviewLauncher() {
    setPreviewLoading(true);
    setErrorMessage(null);
    try {
      const [script, desktop] = await Promise.all([
        invoke<string>('preview_launcher_script', { request }),
        invoke<string>('preview_launcher_desktop', { request }),
      ]);
      setPreviewScriptContent(script);
      setPreviewDesktopContent(desktop);
      setShowLauncherPreview(true);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setPreviewLoading(false);
    }
  }

  async function handleDeleteLauncher() {
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const result = await invoke<LauncherDeleteResult>('delete_launcher', {
        displayName: profile.steam?.launcher?.display_name || '',
        steamAppId: profile.steam?.app_id || '',
        trainerPath: profile.trainer?.path || '',
        targetHomePath: targetHomePath || '',
        steamClientInstallPath: steamClientInstallPath || '',
      });
      const warnings = collectDeleteWarnings(result);
      const deletedAny = result.script_deleted || result.desktop_entry_deleted;

      if (deletedAny && warnings.length === 0) {
        setStatusMessage('Launcher deleted.');
      } else if (deletedAny) {
        setStatusMessage(`Launcher deleted with warnings: ${warnings.join(' ')}`);
      } else if (warnings.length > 0) {
        setErrorMessage(`Launcher was not deleted: ${warnings.join(' ')}`);
      } else {
        setStatusMessage('Launcher files were already absent.');
      }

      void refreshLauncherStatus();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <section className="crosshook-export-panel" aria-label="Launcher export">
      <div className="crosshook-export-panel__body">
        <div className="crosshook-export-section">
          <label className="crosshook-export-label" htmlFor="launcher-name">
            Launcher Name
          </label>
          <input
            id="launcher-name"
            className="crosshook-input"
            value={launcherName}
            onChange={(event) => setLauncherName(event.target.value)}
            placeholder="Elden Ring"
          />
        </div>

        <div className="crosshook-export-callout">{launcherNameHelperText}</div>

        <div className="crosshook-export-section">
          <label className="crosshook-export-label">Launcher Icon</label>
          <div
            className={`crosshook-input crosshook-export-readonly${safeTrim(profile.steam.launcher.icon_path) ? '' : ' crosshook-export-readonly--empty'}`}
          >
            {safeTrim(profile.steam.launcher.icon_path) || 'Use the launcher icon field from the current profile'}
          </div>
        </div>
      </div>

      <div className="crosshook-export-grid">
        {metadataRows.map((row) => (
          <div key={row.label} className="crosshook-export-section">
            <label className="crosshook-export-label">{row.label}</label>
            <div className="crosshook-input crosshook-export-readonly">
              {row.value}
            </div>
          </div>
        ))}
      </div>

      <div className="crosshook-export-actions">
        <button type="button" className="crosshook-button" disabled={!canExport} onClick={() => void handleExport()}>
          {isExporting ? 'Exporting...' : 'Export Launcher'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          disabled={!canExport || previewLoading}
          onClick={() => void handlePreviewLauncher()}
        >
          {previewLoading ? 'Loading...' : 'Preview Launcher'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => {
            setLauncherName(deriveLauncherName(profile));
            setErrorMessage(null);
            setStatusMessage(null);
            setResult(null);
          }}
        >
          Reset
        </button>
        {showDeleteButton ? (
          <button
            type="button"
            className={`crosshook-button crosshook-button--danger${deleteConfirming ? ' crosshook-button--danger-confirming' : ''}`}
            onClick={handleDeleteClick}
            onBlur={handleDeleteBlur}
            data-state={deleteConfirming ? 'confirming' : 'idle'}
          >
            {deleteConfirming ? 'Click again to confirm' : 'Delete Launcher'}
          </button>
        ) : null}
      </div>

      {statusMessage ? (
        <div className="crosshook-export-status" data-state="success" role="status">
          {statusMessage}
        </div>
      ) : null}

      {errorMessage ? (
        <div className="crosshook-export-status" data-state="error" role="alert">
          {errorMessage}
        </div>
      ) : null}

      {result ? (
        <div className="crosshook-export-result">
          <div className="crosshook-export-result__title">Exported {result.display_name}</div>
          <div className="crosshook-export-result__meta">
            Script: <span className="crosshook-export-result__value">{result.script_path}</span>
          </div>
          <div className="crosshook-export-result__meta">
            Desktop entry: <span className="crosshook-export-result__value">{result.desktop_entry_path}</span>
          </div>
          <div className="crosshook-export-result__meta">
            Slug: <span className="crosshook-export-result__value">{result.launcher_slug}</span>
          </div>
        </div>
      ) : null}

      {launcherStatus ? (
        <div className="crosshook-export-status" data-state={launcherStatusTone ?? 'neutral'}>
          <span className="crosshook-export-status__dot" aria-hidden="true" />
          <span className="crosshook-export-status__label">{launcherStatusLabel}</span>
          <span className="crosshook-export-status__copy">{launcherStatusMessage}</span>
        </div>
      ) : null}
      {launcherStatus?.is_stale ? (
        <div className="crosshook-export-callout" data-tone="warning">
          <p className="crosshook-export-callout__title">
            Launcher files are out of date with the current profile.
          </p>
          <p className="crosshook-export-callout__copy">
            Current slug: <code>{launcherStatus.launcher_slug}</code>
          </p>
          <button
            type="button"
            className="crosshook-button crosshook-button--warning"
            onClick={handleExport}
          >
            Re-export Launcher
          </button>
        </div>
      ) : null}
      {showLauncherPreview ? (
        <LauncherPreviewModal
          scriptContent={previewScriptContent}
          desktopContent={previewDesktopContent}
          displayName={`${launcherName}${automaticLauncherSuffix}`}
          onClose={() => setShowLauncherPreview(false)}
        />
      ) : null}
    </section>
  );
}

export default LauncherExport;
