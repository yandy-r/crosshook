import { useEffect, useMemo, useRef, useState } from 'react';
import { usePreferencesContext } from '@/context/PreferencesContext';
import { useProfileContext } from '@/context/ProfileContext';
import { useLaunchEnvironmentAutosave } from '@/hooks/profile/useLaunchEnvironmentAutosave';
import { type SteamExternalLauncherExportRequest, useLauncherExport } from '@/hooks/useLauncherExport';
import type { LaunchPreview, LaunchRequest } from '@/types/launch';
import type { LibraryCardData } from '@/types/library';
import type { GameProfile } from '@/types/profile';
import { copyToClipboard } from '@/utils/clipboard';
import { resolveLaunchMethod } from '@/utils/launch';
import { buildLauncherExportRequest, deriveLauncherName, safeTrim } from '@/utils/launcherExport';
import { CustomEnvironmentVariablesSection } from '../CustomEnvironmentVariablesSection';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { HighlightedCommandBlock } from './HighlightedCommandBlock';

export interface HeroDetailLaunchTabProps {
  summary: LibraryCardData;
  launchRequest: LaunchRequest | null;
  previewLoading: boolean;
  preview: LaunchPreview | null;
  previewError: string | null;
  onPreviewLaunch?: (request: LaunchRequest) => void | Promise<void>;
  onLaunch?: (name: string) => void | Promise<void>;
  launchingName?: string;
  displayProfileName?: string;
}

function profileCanExport(profile: GameProfile): boolean {
  const method = resolveLaunchMethod(profile);
  if (!safeTrim(profile.trainer.path)) {
    return false;
  }
  if (method === 'steam_applaunch') {
    return Boolean(
      safeTrim(profile.steam.app_id) && safeTrim(profile.steam.compatdata_path) && safeTrim(profile.steam.proton_path)
    );
  }
  return Boolean(safeTrim(profile.runtime.prefix_path) && safeTrim(profile.runtime.proton_path));
}

function ExportDesktopButton({
  request,
  profile,
  steamClientInstallPath,
  targetHomePath,
}: {
  request: SteamExternalLauncherExportRequest;
  profile: GameProfile;
  steamClientInstallPath: string;
  targetHomePath: string;
}) {
  const { errorMessage, statusMessage, result, isExporting, exportLauncher } = useLauncherExport({
    request,
    profile,
    steamClientInstallPath,
    targetHomePath,
  });

  return (
    <>
      <button
        type="button"
        className="crosshook-button crosshook-button--secondary"
        disabled={isExporting}
        onClick={() => void exportLauncher()}
      >
        {isExporting ? 'Exporting...' : '.desktop'}
      </button>
      {statusMessage ? (
        <p className="crosshook-hero-detail__launch-status" role="status">
          {statusMessage}
        </p>
      ) : null}
      {result ? (
        <p className="crosshook-hero-detail__launch-status" role="status">
          Exported {result.display_name}
        </p>
      ) : null}
      {errorMessage ? (
        <p className="crosshook-hero-detail__warn" role="alert">
          {errorMessage}
        </p>
      ) : null}
    </>
  );
}

export function HeroDetailLaunchTab({
  summary,
  launchRequest,
  previewLoading,
  preview,
  previewError,
  onPreviewLaunch,
  onLaunch,
  launchingName,
  displayProfileName,
}: HeroDetailLaunchTabProps) {
  const {
    profile,
    profileName,
    selectedProfile,
    profiles,
    updateProfile,
    persistProfileDraft,
    steamClientInstallPath,
    targetHomePath,
  } = useProfileContext();
  const {
    settings: { umu_preference: globalUmuPreference },
  } = usePreferencesContext();
  const [copyStatus, setCopyStatus] = useState<'idle' | 'copied' | 'failed'>('idle');
  const copyStatusResetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyStatusResetTimerRef.current) {
        clearTimeout(copyStatusResetTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    setCopyStatus('idle');
  }, [preview]);

  const selectedTrimmed = selectedProfile.trim();
  const profileNameTrimmed = profileName.trim();
  const resolvedProfileName = displayProfileName?.trim() || selectedTrimmed || profileNameTrimmed || summary.name;
  const hasSavedSelectedProfile =
    selectedTrimmed.length > 0 && profiles.includes(selectedTrimmed) && profileNameTrimmed === selectedTrimmed;
  // Badge reflects persisted profile vars, not in-progress row edits in CustomEnvironmentVariablesSection.
  const customEnvCount = Object.keys(profile.launch.custom_env_vars).length;
  const isLaunching = launchingName === resolvedProfileName;

  const { handleEnvironmentBlurAutoSave } = useLaunchEnvironmentAutosave({
    hasSavedSelectedProfile,
    profile,
    profileName: resolvedProfileName,
    persistProfileDraft,
  });

  const exportRequest = useMemo(() => {
    if (!profileCanExport(profile)) {
      return null;
    }
    const method = resolveLaunchMethod(profile);
    const launcherName = deriveLauncherName(profile);
    return buildLauncherExportRequest(
      profile,
      resolvedProfileName,
      method,
      launcherName,
      safeTrim(profile.steam.launcher.icon_path),
      steamClientInstallPath,
      targetHomePath,
      globalUmuPreference
    );
  }, [globalUmuPreference, profile, resolvedProfileName, steamClientInstallPath, targetHomePath]);

  const canPreview = Boolean(launchRequest && onPreviewLaunch && !previewLoading);
  const canCopy = Boolean(preview?.effective_command);
  const canLaunch = Boolean(onLaunch && launchRequest && !isLaunching);

  const handleCopy = async () => {
    setCopyStatus('idle');
    if (copyStatusResetTimerRef.current) {
      clearTimeout(copyStatusResetTimerRef.current);
      copyStatusResetTimerRef.current = null;
    }
    if (!preview?.effective_command) {
      return;
    }
    try {
      await copyToClipboard(preview.effective_command);
      setCopyStatus('copied');
    } catch {
      setCopyStatus('failed');
    }
    copyStatusResetTimerRef.current = setTimeout(() => {
      setCopyStatus('idle');
      copyStatusResetTimerRef.current = null;
    }, 2500);
  };

  return (
    <div className="crosshook-hero-detail__launch-tab">
      <DashboardPanelSection
        title="Launch command"
        titleAs="h3"
        className="crosshook-hero-detail__section"
        actions={
          <div className="crosshook-hero-detail__launch-actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              disabled={!canPreview}
              onClick={() => {
                if (launchRequest) {
                  void onPreviewLaunch?.(launchRequest);
                }
              }}
            >
              {previewLoading ? 'Building...' : 'Dry-run'}
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              disabled={!canCopy}
              onClick={() => void handleCopy()}
            >
              Copy
            </button>
            {exportRequest ? (
              <ExportDesktopButton
                request={exportRequest}
                profile={profile}
                steamClientInstallPath={steamClientInstallPath}
                targetHomePath={targetHomePath}
              />
            ) : (
              <button type="button" className="crosshook-button crosshook-button--secondary" disabled>
                .desktop
              </button>
            )}
            <button
              type="button"
              className="crosshook-button"
              disabled={!canLaunch}
              onClick={() => {
                void onLaunch?.(resolvedProfileName);
              }}
            >
              {isLaunching ? 'Launching...' : 'Launch'}
            </button>
          </div>
        }
      >
        {!launchRequest ? (
          <p className="crosshook-hero-detail__muted">
            Launch preview is unavailable until the game executable is set on this profile.
          </p>
        ) : null}
        {previewLoading ? <p className="crosshook-hero-detail__muted">Building launch preview...</p> : null}
        {previewError ? <p className="crosshook-hero-detail__warn">{previewError}</p> : null}
        {preview ? <HighlightedCommandBlock preview={preview} profileName={resolvedProfileName} /> : null}
        {copyStatus === 'copied' ? (
          <p className="crosshook-hero-detail__launch-status" role="status">
            Command copied.
          </p>
        ) : null}
        {copyStatus === 'failed' ? (
          <p className="crosshook-hero-detail__warn" role="alert">
            Failed to copy command.
          </p>
        ) : null}
      </DashboardPanelSection>

      <DashboardPanelSection
        title="Environment"
        titleAs="h3"
        className="crosshook-hero-detail__section"
        headingAfter={
          customEnvCount > 0 ? <span className="crosshook-hero-detail__pill">{customEnvCount} ON</span> : null
        }
      >
        <CustomEnvironmentVariablesSection
          profileName={resolvedProfileName}
          customEnvVars={profile.launch.custom_env_vars}
          onUpdateProfile={updateProfile}
          idPrefix="hero-detail-launch"
          onAutoSaveBlur={handleEnvironmentBlurAutoSave}
        />
      </DashboardPanelSection>

      <DashboardPanelSection title="Pre/post hooks" titleAs="h3" className="crosshook-hero-detail__section">
        <div className="crosshook-hero-detail__hook-placeholder">
          <p className="crosshook-hero-detail__muted">No pre/post hooks configured yet</p>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            disabled
            aria-label="Add hook (not yet available)"
            title="Add hook (not yet available)"
          >
            Add hook
          </button>
        </div>
      </DashboardPanelSection>
    </div>
  );
}

export default HeroDetailLaunchTab;
