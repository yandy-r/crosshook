import { useEffect, useMemo, useRef, useState } from 'react';
import { usePreferencesContext } from '@/context/PreferencesContext';
import { useProfileContext } from '@/context/ProfileContext';
import { type SteamExternalLauncherExportRequest, useLauncherExport } from '@/hooks/useLauncherExport';
import type { LaunchPhase, LaunchPreview, LaunchRequest } from '@/types/launch';
import type { GameProfile } from '@/types/profile';
import { copyToClipboard } from '@/utils/clipboard';
import { resolveLaunchMethod } from '@/utils/launch';
import { buildLauncherExportRequest, deriveLauncherName, safeTrim } from '@/utils/launcherExport';
import { DashboardPanelSection } from '../../layout/DashboardPanelSection';
import { HighlightedCommandBlock } from '../HighlightedCommandBlock';

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

export interface HeroLaunchCommandSectionProps {
  launchRequest: LaunchRequest | null;
  previewLoading: boolean;
  preview: LaunchPreview | null;
  previewError: string | null;
  resolvedProfileName: string;
  isLaunching: boolean;
  onPreviewLaunch?: (request: LaunchRequest) => void | Promise<void>;
  /** Legacy navigation-based launch (used by the outer shell / tests). */
  onLaunch?: (name: string) => void | Promise<void>;
  // ── In-place launch props (wired by HeroLaunchGate) ───────────────────────
  /** Whether the Launch Game button should be enabled. */
  canLaunchGame?: boolean;
  /** Whether the Launch Trainer button should be enabled. */
  canLaunchTrainer?: boolean;
  /** Whether a launch is currently in progress. */
  isBusy?: boolean;
  /** Whether the game process is already running. */
  isGameRunning?: boolean;
  /** Current launch phase for button label derivation. */
  phase?: LaunchPhase;
  /** Whether the launch phase is idle. */
  isIdle?: boolean;
  /** Pre-launch hook — dep gate + selectProfile-first. Returns false to abort. */
  onBeforeLaunch?: (action: 'game' | 'trainer') => Promise<boolean>;
  /** In-place game launch (from LaunchStateContext). */
  onLaunchGame?: () => void;
  /** In-place trainer launch (from LaunchStateContext). */
  onLaunchTrainer?: () => void;
  /** Hint shown when the profile is not selectable (fallback profile). */
  notSelectableHint?: string | null;
}

export function HeroLaunchCommandSection({
  launchRequest,
  previewLoading,
  preview,
  previewError,
  resolvedProfileName,
  isLaunching,
  onPreviewLaunch,
  onLaunch,
  canLaunchGame,
  canLaunchTrainer,
  isBusy = false,
  isGameRunning = false,
  phase,
  isIdle,
  onBeforeLaunch,
  onLaunchGame,
  onLaunchTrainer,
  notSelectableHint,
}: HeroLaunchCommandSectionProps) {
  const { profile, steamClientInstallPath, targetHomePath } = useProfileContext();
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

  // biome-ignore lint/correctness/useExhaustiveDependencies: trigger-only dep — reset copy status whenever the preview changes
  useEffect(() => {
    setCopyStatus('idle');
  }, [preview]);

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

  // In-place mode: canLaunchGame / canLaunchTrainer are provided by HeroLaunchGate.
  // Legacy mode: canLaunch derived from onLaunch prop.
  const isInPlaceMode = onLaunchGame !== undefined || onLaunchTrainer !== undefined;
  const legacyCanLaunch = Boolean(onLaunch && launchRequest && !isLaunching);

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

  function handleInPlaceLaunch(action: 'game' | 'trainer') {
    void (async () => {
      if (onBeforeLaunch) {
        const proceed = await onBeforeLaunch(action);
        if (!proceed) return;
      }
      if (action === 'game') {
        onLaunchGame?.();
      } else {
        onLaunchTrainer?.();
      }
    })();
  }

  return (
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
          {isInPlaceMode ? (
            <>
              <button
                type="button"
                className="crosshook-button"
                disabled={!canLaunchGame}
                aria-label={isGameRunning ? 'Game Running' : isBusy && phase ? 'Launching…' : 'Launch Game'}
                onClick={() => handleInPlaceLaunch('game')}
              >
                {isGameRunning ? 'Game Running' : isBusy && isIdle === false ? 'Launching…' : 'Launch Game'}
              </button>
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                disabled={!canLaunchTrainer}
                onClick={() => handleInPlaceLaunch('trainer')}
              >
                {isBusy && phase !== undefined ? 'Launching…' : 'Launch Trainer'}
              </button>
            </>
          ) : (
            <button
              type="button"
              className="crosshook-button"
              disabled={!legacyCanLaunch}
              onClick={() => {
                void onLaunch?.(resolvedProfileName);
              }}
            >
              {isLaunching ? 'Launching...' : 'Launch'}
            </button>
          )}
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
      {notSelectableHint ? (
        <p className="crosshook-hero-detail__muted" role="note">
          {notSelectableHint}
        </p>
      ) : null}
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
  );
}

export default HeroLaunchCommandSection;
