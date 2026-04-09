import { useEffect, useMemo, useState } from 'react';
import type { GameProfile, GamescopeConfig, LaunchMethod, TrainerLoadingMode } from '../types';
import { useLauncherExport, type SteamExternalLauncherExportRequest } from '../hooks/useLauncherExport';
import { LauncherPreviewModal } from './LauncherPreviewModal';

interface LauncherExportProps {
  profile: GameProfile;
  profileName: string;
  method: Exclude<LaunchMethod, ''>;
  steamClientInstallPath: string;
  targetHomePath: string;
  pendingReExport?: boolean;
  onReExportHandled?: () => void;
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

function deriveLauncherName(profile: GameProfile): string {
  const explicitName = stripAutomaticLauncherSuffix(safeTrim(profile.steam.launcher.display_name));
  if (explicitName) {
    return explicitName;
  }

  const gameName = safeTrim(profile.game.name);
  if (gameName) {
    return gameName;
  }

  const trainerStem = stripAutomaticLauncherSuffix(
    safeTrim(profile.trainer.path)
      .split(/[\\/]/)
      .pop()
      ?.replace(/\.[^.]+$/, '')
      .trim() ?? ''
  );
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
  profileName: string,
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
    proton_path: method === 'steam_applaunch' ? profile.steam.proton_path.trim() : profile.runtime.proton_path.trim(),
    steam_app_id: profile.steam.app_id.trim(),
    steam_client_install_path: steamClientInstallPath.trim(),
    target_home_path: targetHomePath.trim(),
    profile_name: profileName.trim() || undefined,
    network_isolation: profile.launch.network_isolation ?? true,
    gamescope: profile.launch?.trainer_gamescope,
  };
}

export function LauncherExport({
  profile,
  profileName,
  method,
  steamClientInstallPath,
  targetHomePath,
  pendingReExport,
  onReExportHandled,
}: LauncherExportProps) {
  const [launcherName, setLauncherName] = useState(() => deriveLauncherName(profile));

  const request = useMemo(
    () =>
      buildExportRequest(
        profile,
        profileName,
        method,
        launcherName,
        safeTrim(profile.steam.launcher.icon_path),
        steamClientInstallPath,
        targetHomePath
      ),
    [profile, profileName, method, launcherName, steamClientInstallPath, targetHomePath, profile.launch]
  );

  const {
    launcherStatus,
    errorMessage,
    setErrorMessage,
    statusMessage,
    result,
    isExporting,
    previewLoading,
    previewScriptContent,
    previewDesktopContent,
    showLauncherPreview,
    setShowLauncherPreview,
    deleteConfirming,
    exportLauncher,
    previewLauncher,
    handleDeleteClick,
    handleDeleteBlur,
    clearExportFeedback,
  } = useLauncherExport({
    request,
    profile,
    steamClientInstallPath,
    targetHomePath,
    pendingReExport,
    onReExportHandled,
  });

  useEffect(() => {
    setLauncherName(deriveLauncherName(profile));
  }, [profile]);

  const metadataRows = useMemo(
    () =>
      method === 'steam_applaunch'
        ? [
            { label: 'Trainer Path', value: safeTrim(profile.trainer.path) || 'Not set' },
            {
              label: 'Trainer Loading Mode',
              value:
                profile.trainer.loading_mode === 'copy_to_prefix' ? 'Copy into prefix' : 'Run from current directory',
            },
            { label: 'Steam App ID', value: safeTrim(profile.steam.app_id) || 'Not set' },
            { label: 'Compatdata Path', value: safeTrim(profile.steam.compatdata_path) || 'Not set' },
            { label: 'Proton Path', value: safeTrim(profile.steam.proton_path) || 'Not set' },
            { label: 'Network Isolation', value: (profile.launch.network_isolation ?? true) ? 'Enabled' : 'Disabled' },
            { label: 'Trainer Gamescope', value: profile.launch?.trainer_gamescope?.enabled ? 'Enabled' : 'Disabled' },
          ]
        : [
            { label: 'Trainer Path', value: safeTrim(profile.trainer.path) || 'Not set' },
            {
              label: 'Trainer Loading Mode',
              value:
                profile.trainer.loading_mode === 'copy_to_prefix' ? 'Copy into prefix' : 'Run from current directory',
            },
            { label: 'Prefix Path', value: safeTrim(profile.runtime.prefix_path) || 'Not set' },
            { label: 'Proton Path', value: safeTrim(profile.runtime.proton_path) || 'Not set' },
            { label: 'Network Isolation', value: (profile.launch.network_isolation ?? true) ? 'Enabled' : 'Disabled' },
            { label: 'Trainer Gamescope', value: profile.launch?.trainer_gamescope?.enabled ? 'Enabled' : 'Disabled' },
            { label: 'Working Directory', value: safeTrim(profile.runtime.working_directory) || 'Not set' },
          ],
    [method, profile]
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
            <div className="crosshook-input crosshook-export-readonly">{row.value}</div>
          </div>
        ))}
      </div>

      <div className="crosshook-export-actions">
        <button type="button" className="crosshook-button" disabled={!canExport} onClick={() => void exportLauncher()}>
          {isExporting ? 'Exporting...' : 'Export Launcher'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          disabled={!canExport || previewLoading}
          onClick={() => void previewLauncher()}
        >
          {previewLoading ? 'Loading...' : 'Preview Launcher'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => {
            setLauncherName(deriveLauncherName(profile));
            clearExportFeedback();
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
          <p className="crosshook-export-callout__title">Launcher files are out of date with the current profile.</p>
          <p className="crosshook-export-callout__copy">
            Current slug: <code>{launcherStatus.launcher_slug}</code>
          </p>
          <button
            type="button"
            className="crosshook-button crosshook-button--warning"
            onClick={() => void exportLauncher()}
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
