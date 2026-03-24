import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { GameProfile, LaunchMethod, LauncherInfo } from '../types';

interface LauncherExportProps {
  profile: GameProfile;
  method: Exclude<LaunchMethod, ''>;
  steamClientInstallPath: string;
  targetHomePath: string;
  context?: 'default' | 'install';
}

interface SteamExternalLauncherExportRequest {
  method: string;
  launcher_name: string;
  trainer_path: string;
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

const panelStyle: CSSProperties = {
  display: 'grid',
  alignContent: 'start',
  gap: 16,
  height: '100%',
  boxSizing: 'border-box',
  padding: 20,
  borderRadius: 18,
  background: 'radial-gradient(circle at top right, rgba(59, 130, 246, 0.14), transparent 30%), rgba(10, 15, 24, 0.96)',
  border: '1px solid rgba(96, 165, 250, 0.24)',
  boxShadow: '0 24px 60px rgba(0, 0, 0, 0.35)',
};

const sectionStyle: CSSProperties = {
  display: 'grid',
  gap: 10,
};

const labelStyle: CSSProperties = {
  fontSize: 13,
  fontWeight: 700,
  letterSpacing: '0.02em',
  color: '#cbd5e1',
};

const inputStyle: CSSProperties = {
  width: '100%',
  minWidth: 0,
  minHeight: 44,
  boxSizing: 'border-box',
  borderRadius: 12,
  border: '1px solid rgba(96, 165, 250, 0.24)',
  background: 'rgba(15, 23, 42, 0.92)',
  color: '#f8fafc',
  padding: '0 14px',
};

const buttonStyle: CSSProperties = {
  minHeight: 44,
  borderRadius: 12,
  border: '1px solid rgba(96, 165, 250, 0.32)',
  background: 'linear-gradient(135deg, #2563eb 0%, #0ea5e9 100%)',
  color: '#fff',
  padding: '0 16px',
  cursor: 'pointer',
  fontWeight: 700,
};

const subtleButtonStyle: CSSProperties = {
  ...buttonStyle,
  background: 'rgba(15, 23, 42, 0.9)',
};

const deleteButtonStyle: CSSProperties = {
  ...buttonStyle,
  background: 'rgba(185, 28, 28, 0.16)',
  border: '1px solid rgba(248, 113, 113, 0.28)',
  color: '#fee2e2',
};

const deleteButtonConfirmingStyle: CSSProperties = {
  ...deleteButtonStyle,
  background: 'rgba(185, 28, 28, 0.28)',
};

const helperStyle: CSSProperties = {
  margin: 0,
  color: '#94a3b8',
  fontSize: 13,
  lineHeight: 1.5,
};

function safeTrim(value: string | undefined | null): string {
  return value?.trim() ?? '';
}

function deriveLauncherName(profile: GameProfile): string {
  const explicitName = safeTrim(profile.steam.launcher.display_name);
  if (explicitName) {
    return explicitName;
  }

  const gameName = safeTrim(profile.game.name);
  if (gameName) {
    return gameName;
  }

  const trainerStem = safeTrim(profile.trainer.path)
    .split(/[\\/]/)
    .pop()
    ?.replace(/\.[^.]+$/, '')
    .trim();
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
  context = 'default',
}: LauncherExportProps) {
  const [launcherName, setLauncherName] = useState(() => deriveLauncherName(profile));
  const [isExporting, setIsExporting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [result, setResult] = useState<SteamExternalLauncherExportResult | null>(null);
  const [launcherStatus, setLauncherStatus] = useState<LauncherInfo | null>(null);
  const [deleteConfirming, setDeleteConfirming] = useState(false);
  const deleteTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const refreshLauncherStatus = useCallback(async () => {
    if (context !== 'default' || !profile) return;
    try {
      const info = await invoke<LauncherInfo>('check_launcher_exists', {
        displayName: profile.steam?.launcher?.display_name || '',
        steamAppId: profile.steam?.app_id || '',
        trainerPath: profile.trainer?.path || '',
        targetHomePath: targetHomePath || '',
        steamClientInstallPath: steamClientInstallPath || '',
      });
      setLauncherStatus(info);
    } catch {
      setLauncherStatus(null);
    }
  }, [profile, targetHomePath, steamClientInstallPath, context]);

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

  const request = buildExportRequest(
    profile,
    method,
    launcherName,
    safeTrim(profile.steam.launcher.icon_path),
    steamClientInstallPath,
    targetHomePath
  );

  const metadataRows = useMemo(
    () =>
      method === 'steam_applaunch'
        ? [
            { label: 'Trainer Path', value: safeTrim(profile.trainer.path) || 'Not set' },
            { label: 'Steam App ID', value: safeTrim(profile.steam.app_id) || 'Not set' },
            { label: 'Compatdata Path', value: safeTrim(profile.steam.compatdata_path) || 'Not set' },
            { label: 'Proton Path', value: safeTrim(profile.steam.proton_path) || 'Not set' },
          ]
        : [
            { label: 'Trainer Path', value: safeTrim(profile.trainer.path) || 'Not set' },
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

  const showDeleteButton =
    (launcherStatus?.script_exists || launcherStatus?.desktop_entry_exists) && context === 'default';

  if (context === 'install') {
    return (
      <section style={panelStyle} aria-label="Install review">
        <header style={{ display: 'grid', gap: 8 }}>
          <div
            style={{
              color: '#60a5fa',
              fontSize: 12,
              letterSpacing: '0.18em',
              textTransform: 'uppercase',
            }}
          >
            Install Review
          </div>
          <h2 style={{ margin: 0, fontSize: 22, fontWeight: 800 }}>Review and save the generated profile</h2>
          <p style={helperStyle}>
            Install Game keeps the profile editable until you confirm the executable and save it in the Profile tab.
          </p>
        </header>

        <div style={{ display: 'grid', gap: 12 }}>
          <div style={sectionStyle}>
            <label style={labelStyle}>Runner</label>
            <div
              style={{
                ...inputStyle,
                display: 'flex',
                alignItems: 'center',
                color: '#f8fafc',
              }}
            >
              Proton (`proton_run`)
            </div>
          </div>

          <div style={sectionStyle}>
            <label style={labelStyle}>Save boundary</label>
            <div
              style={{
                ...inputStyle,
                display: 'flex',
                alignItems: 'flex-start',
                padding: '10px 14px',
                color: '#f8fafc',
              }}
            >
              Nothing is persisted until you switch back to Profile and click Save.
            </div>
          </div>

          <div style={sectionStyle}>
            <label style={labelStyle}>After save</label>
            <div
              style={{
                ...inputStyle,
                display: 'flex',
                alignItems: 'flex-start',
                padding: '10px 14px',
                color: '#f8fafc',
              }}
            >
              The normal launch and launcher export flow becomes available once the generated profile is saved.
            </div>
          </div>
        </div>
      </section>
    );
  }

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

  async function handleDeleteLauncher() {
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      await invoke('delete_launcher', {
        displayName: profile.steam?.launcher?.display_name || '',
        steamAppId: profile.steam?.app_id || '',
        trainerPath: profile.trainer?.path || '',
        targetHomePath: targetHomePath || '',
        steamClientInstallPath: steamClientInstallPath || '',
      });
      setStatusMessage('Launcher deleted.');
      void refreshLauncherStatus();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <section style={panelStyle} aria-label="Launcher export">
      <header style={{ display: 'grid', gap: 8 }}>
        <div
          style={{
            color: '#60a5fa',
            fontSize: 12,
            letterSpacing: '0.18em',
            textTransform: 'uppercase',
          }}
        >
          Launcher Export
        </div>
        <h2 style={{ margin: 0, fontSize: 22, fontWeight: 800 }}>Export a standalone trainer launcher</h2>
        <p style={helperStyle}>
          Generate a shell script and matching desktop entry from the current profile and runtime settings.
        </p>
      </header>

      <div style={{ display: 'grid', gap: 12 }}>
        <div style={sectionStyle}>
          <label style={labelStyle} htmlFor="launcher-name">
            Launcher Name
          </label>
          <input
            id="launcher-name"
            style={inputStyle}
            value={launcherName}
            onChange={(event) => setLauncherName(event.target.value)}
            placeholder="Elden Ring Trainer"
          />
        </div>

        <div style={sectionStyle}>
          <label style={labelStyle}>Launcher Icon</label>
          <div
            style={{
              ...inputStyle,
              display: 'flex',
              alignItems: 'flex-start',
              color: safeTrim(profile.steam.launcher.icon_path) ? '#f8fafc' : '#94a3b8',
              padding: '10px 14px',
              wordBreak: 'break-word',
            }}
          >
            {safeTrim(profile.steam.launcher.icon_path) || 'Use the launcher icon field from the current profile'}
          </div>
        </div>
      </div>

      <div
        style={{
          display: 'grid',
          gap: 12,
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
        }}
      >
        {metadataRows.map((row) => (
          <div key={row.label} style={sectionStyle}>
            <label style={labelStyle}>{row.label}</label>
            <div
              style={{
                ...inputStyle,
                display: 'flex',
                alignItems: 'flex-start',
                padding: '10px 14px',
                wordBreak: 'break-word',
              }}
            >
              {row.value}
            </div>
          </div>
        ))}
      </div>

      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
        <button type="button" style={buttonStyle} disabled={!canExport} onClick={() => void handleExport()}>
          {isExporting ? 'Exporting...' : 'Export Launcher'}
        </button>
        <button
          type="button"
          style={subtleButtonStyle}
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
            style={deleteConfirming ? deleteButtonConfirmingStyle : deleteButtonStyle}
            onClick={handleDeleteClick}
            onBlur={handleDeleteBlur}
          >
            {deleteConfirming ? 'Click again to confirm' : 'Delete Launcher'}
          </button>
        ) : null}
      </div>

      {statusMessage ? (
        <div
          style={{
            borderRadius: 12,
            padding: 12,
            background: 'rgba(16, 185, 129, 0.12)',
            border: '1px solid rgba(16, 185, 129, 0.28)',
            color: '#d1fae5',
          }}
        >
          {statusMessage}
        </div>
      ) : null}

      {errorMessage ? (
        <div
          style={{
            borderRadius: 12,
            padding: 12,
            background: 'rgba(185, 28, 28, 0.16)',
            border: '1px solid rgba(248, 113, 113, 0.28)',
            color: '#fee2e2',
          }}
        >
          {errorMessage}
        </div>
      ) : null}

      {result ? (
        <div
          style={{
            display: 'grid',
            gap: 10,
            borderRadius: 14,
            padding: 14,
            background: 'rgba(15, 23, 42, 0.7)',
            border: '1px solid rgba(96, 165, 250, 0.18)',
          }}
        >
          <div style={{ color: '#dbeafe', fontWeight: 700 }}>Exported {result.display_name}</div>
          <div style={helperStyle}>
            Script: <span style={{ color: '#e2e8f0' }}>{result.script_path}</span>
          </div>
          <div style={helperStyle}>
            Desktop entry: <span style={{ color: '#e2e8f0' }}>{result.desktop_entry_path}</span>
          </div>
          <div style={helperStyle}>
            Slug: <span style={{ color: '#e2e8f0' }}>{result.launcher_slug}</span>
          </div>
        </div>
      ) : null}

      {context === 'default' && launcherStatus && (
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          padding: '6px 10px',
          borderRadius: '6px',
          background: launcherStatus.is_stale
            ? 'rgba(245, 158, 11, 0.12)'
            : launcherStatus.script_exists && launcherStatus.desktop_entry_exists
            ? 'rgba(16, 185, 129, 0.12)'
            : !launcherStatus.script_exists && !launcherStatus.desktop_entry_exists
            ? 'rgba(107, 114, 128, 0.12)'
            : 'rgba(245, 158, 11, 0.12)',
          marginTop: '8px',
        }}>
          <span style={{
            width: '8px',
            height: '8px',
            borderRadius: '50%',
            background: launcherStatus.is_stale
              ? '#f59e0b'
              : launcherStatus.script_exists && launcherStatus.desktop_entry_exists
              ? '#10b981'
              : !launcherStatus.script_exists && !launcherStatus.desktop_entry_exists
              ? '#6b7280'
              : '#f59e0b',
          }} />
          <span style={{
            fontSize: '0.85rem',
            color: launcherStatus.is_stale
              ? '#fef3c7'
              : launcherStatus.script_exists && launcherStatus.desktop_entry_exists
              ? '#d1fae5'
              : !launcherStatus.script_exists && !launcherStatus.desktop_entry_exists
              ? '#d1d5db'
              : '#fef3c7',
          }}>
            {launcherStatus.is_stale
              ? 'Stale'
              : launcherStatus.script_exists && launcherStatus.desktop_entry_exists
              ? 'Exported'
              : !launcherStatus.script_exists && !launcherStatus.desktop_entry_exists
              ? 'Not Exported'
              : 'Partial'}
          </span>
        </div>
      )}
      {launcherStatus?.is_stale && context === 'default' && (
        <div style={{
          background: 'rgba(245, 158, 11, 0.08)',
          border: '1px solid rgba(245, 158, 11, 0.2)',
          borderRadius: '8px',
          padding: '12px',
          marginTop: '8px',
        }}>
          <p style={{ margin: '0 0 8px', fontSize: '0.85rem', color: '#fef3c7' }}>
            Launcher files are out of date with the current profile.
          </p>
          <p style={{ margin: '0 0 8px', fontSize: '0.8rem', color: '#d1d5db' }}>
            Current slug: <code>{launcherStatus.launcher_slug}</code>
          </p>
          <button
            onClick={handleExport}
            style={{
              padding: '6px 16px',
              minHeight: '44px',
              background: 'rgba(245, 158, 11, 0.16)',
              border: '1px solid rgba(245, 158, 11, 0.28)',
              color: '#fef3c7',
              borderRadius: '6px',
              cursor: 'pointer',
            }}
          >
            Re-export Launcher
          </button>
        </div>
      )}
    </section>
  );
}

export default LauncherExport;
