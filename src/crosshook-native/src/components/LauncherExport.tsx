import { useState, type CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { GameProfile } from '../types';

interface LauncherExportProps {
  profile: GameProfile;
  steamClientInstallPath: string;
  targetHomePath: string;
}

interface SteamExternalLauncherExportRequest {
  launcher_name: string;
  trainer_path: string;
  launcher_icon_path: string;
  steam_app_id: string;
  steam_compat_data_path: string;
  steam_proton_path: string;
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
  gap: 16,
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
  launcherName: string,
  launcherIconPath: string,
  steamClientInstallPath: string,
  targetHomePath: string
): SteamExternalLauncherExportRequest {
  return {
    launcher_name: launcherName.trim(),
    trainer_path: profile.trainer.path.trim(),
    launcher_icon_path: launcherIconPath.trim(),
    steam_app_id: profile.steam.app_id.trim(),
    steam_compat_data_path: profile.steam.compatdata_path.trim(),
    steam_proton_path: profile.steam.proton_path.trim(),
    steam_client_install_path: steamClientInstallPath.trim(),
    target_home_path: targetHomePath.trim(),
  };
}

export function LauncherExport({ profile, steamClientInstallPath, targetHomePath }: LauncherExportProps) {
  const [launcherName, setLauncherName] = useState(() => deriveLauncherName(profile));
  const [isExporting, setIsExporting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [result, setResult] = useState<SteamExternalLauncherExportResult | null>(null);

  const request = buildExportRequest(
    profile,
    launcherName,
    safeTrim(profile.steam.launcher.icon_path),
    steamClientInstallPath,
    targetHomePath
  );

  const canExport =
    request.trainer_path.length > 0 &&
    request.steam_app_id.length > 0 &&
    request.steam_compat_data_path.length > 0 &&
    request.steam_proton_path.length > 0 &&
    request.steam_client_install_path.length > 0 &&
    request.target_home_path.length > 0 &&
    !isExporting;

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
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsExporting(false);
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
          Generate a shell script and matching desktop entry from the current profile and Steam settings.
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
            {safeTrim(profile.steam.launcher.icon_path) || 'Use the Steam Mode launcher icon field above'}
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
        <div style={sectionStyle}>
          <label style={labelStyle}>Trainer Path</label>
          <div
            style={{
              ...inputStyle,
              display: 'flex',
              alignItems: 'flex-start',
              padding: '10px 14px',
              wordBreak: 'break-word',
            }}
          >
            {safeTrim(profile.trainer.path) || 'Not set'}
          </div>
        </div>
        <div style={sectionStyle}>
          <label style={labelStyle}>Steam App ID</label>
          <div
            style={{
              ...inputStyle,
              display: 'flex',
              alignItems: 'flex-start',
              padding: '10px 14px',
              wordBreak: 'break-word',
            }}
          >
            {safeTrim(profile.steam.app_id) || 'Not set'}
          </div>
        </div>
        <div style={sectionStyle}>
          <label style={labelStyle}>Compatdata Path</label>
          <div
            style={{
              ...inputStyle,
              display: 'flex',
              alignItems: 'flex-start',
              padding: '10px 14px',
              wordBreak: 'break-word',
            }}
          >
            {safeTrim(profile.steam.compatdata_path) || 'Not set'}
          </div>
        </div>
        <div style={sectionStyle}>
          <label style={labelStyle}>Proton Path</label>
          <div
            style={{
              ...inputStyle,
              display: 'flex',
              alignItems: 'flex-start',
              padding: '10px 14px',
              wordBreak: 'break-word',
            }}
          >
            {safeTrim(profile.steam.proton_path) || 'Not set'}
          </div>
        </div>
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
    </section>
  );
}

export default LauncherExport;
