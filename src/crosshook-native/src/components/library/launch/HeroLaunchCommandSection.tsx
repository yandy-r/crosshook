import { createContext, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { usePreferencesContext } from '@/context/PreferencesContext';
import { useProfileContext } from '@/context/ProfileContext';
import { type SteamExternalLauncherExportRequest, useLauncherExport } from '@/hooks/useLauncherExport';
import type { LaunchPhase, LaunchPreview, LaunchRequest } from '@/types/launch';
import type { GameProfile } from '@/types/profile';
import { copyToClipboard } from '@/utils/clipboard';
import { resolveLaunchMethod } from '@/utils/launch';
import { buildLauncherExportRequest, deriveLauncherName, safeTrim } from '@/utils/launcherExport';
import { umuGameIdResolutionSourceLabel } from '@/utils/launchPreviewPresentation';
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

type ExportDesktopState = ReturnType<typeof useLauncherExport>;

const ExportDesktopContext = createContext<ExportDesktopState | null>(null);

function ExportDesktopProvider({
  request,
  profile,
  steamClientInstallPath,
  targetHomePath,
  children,
}: {
  request: SteamExternalLauncherExportRequest;
  profile: GameProfile;
  steamClientInstallPath: string;
  targetHomePath: string;
  children: React.ReactNode;
}) {
  const exportState = useLauncherExport({
    request,
    profile,
    steamClientInstallPath,
    targetHomePath,
  });

  return <ExportDesktopContext.Provider value={exportState}>{children}</ExportDesktopContext.Provider>;
}

function ExportDesktopButton({ className }: { className: string }) {
  const exportState = useContext(ExportDesktopContext);
  if (!exportState) {
    return null;
  }

  const { isExporting, exportLauncher } = exportState;

  return (
    <button
      type="button"
      className={className}
      disabled={isExporting}
      onClick={() => void exportLauncher()}
      title="Export .desktop launcher"
    >
      {isExporting ? 'Exporting…' : '.desktop'}
    </button>
  );
}

function ExportDesktopFeedback() {
  const exportState = useContext(ExportDesktopContext);
  if (!exportState) {
    return null;
  }

  const { errorMessage, statusMessage, result } = exportState;

  return (
    <>
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
  /** Whether export may act on the context-selected profile. */
  canExportDesktop?: boolean;
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
  isIdle,
  onBeforeLaunch,
  onLaunchGame,
  onLaunchTrainer,
  notSelectableHint,
  canExportDesktop = true,
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
    if (!canExportDesktop) {
      return null;
    }
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
  }, [canExportDesktop, globalUmuPreference, profile, resolvedProfileName, steamClientInstallPath, targetHomePath]);

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

  const secondaryActionClass = 'crosshook-button crosshook-button--secondary crosshook-button--small';
  const primaryActionClass = 'crosshook-button crosshook-button--small';

  const launchToolbar = (
    <div className="crosshook-hero-detail__launch-actions">
      <div
        className="crosshook-hero-detail__profile-actions crosshook-hero-detail__launch-actions-toolbar"
        role="toolbar"
        aria-label="Launch command actions"
      >
        <div className="crosshook-hero-detail__profile-actions-group" role="group" aria-label="Preview and export">
          <button
            type="button"
            className={secondaryActionClass}
            disabled={!canPreview}
            onClick={() => {
              if (launchRequest) {
                void onPreviewLaunch?.(launchRequest);
              }
            }}
          >
            {previewLoading ? 'Building...' : 'Dry-run'}
          </button>
          <button type="button" className={secondaryActionClass} disabled={!canCopy} onClick={() => void handleCopy()}>
            Copy
          </button>
          {exportRequest ? (
            <ExportDesktopButton className={secondaryActionClass} />
          ) : (
            <button type="button" className={secondaryActionClass} disabled title="Export .desktop launcher">
              .desktop
            </button>
          )}
        </div>

        <div className="crosshook-hero-detail__profile-actions-divider" role="presentation" aria-hidden="true" />

        <div
          className="crosshook-hero-detail__profile-actions-group crosshook-hero-detail__profile-actions-group--trailing"
          role="group"
          aria-label="Launch profile"
        >
          {isInPlaceMode ? (
            <>
              <button
                type="button"
                className={primaryActionClass}
                disabled={!canLaunchGame}
                aria-label={isGameRunning ? 'Game Running' : isBusy && !isIdle ? 'Launching…' : 'Launch Game'}
                onClick={() => handleInPlaceLaunch('game')}
              >
                {isGameRunning ? 'Game Running' : isBusy && !isIdle ? 'Launching…' : 'Launch Game'}
              </button>
              <button
                type="button"
                className={secondaryActionClass}
                disabled={!canLaunchTrainer}
                onClick={() => handleInPlaceLaunch('trainer')}
              >
                {isBusy && !isIdle ? 'Launching…' : 'Launch Trainer'}
              </button>
            </>
          ) : (
            <button
              type="button"
              className={primaryActionClass}
              disabled={!legacyCanLaunch}
              onClick={() => {
                void onLaunch?.(resolvedProfileName);
              }}
            >
              {isLaunching ? 'Launching...' : 'Launch'}
            </button>
          )}
        </div>
      </div>

      <div className="crosshook-hero-detail__launch-actions-messages">
        {exportRequest ? <ExportDesktopFeedback /> : null}
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
      </div>
    </div>
  );

  return (
    <DashboardPanelSection title="Launch command" titleAs="h3" className="crosshook-hero-detail__section">
      {exportRequest ? (
        <ExportDesktopProvider
          request={exportRequest}
          profile={profile}
          steamClientInstallPath={steamClientInstallPath}
          targetHomePath={targetHomePath}
        >
          {launchToolbar}
        </ExportDesktopProvider>
      ) : (
        launchToolbar
      )}
      {!launchRequest ? (
        <p className="crosshook-hero-detail__muted">
          Launch preview is unavailable until the game executable is set on this profile.
        </p>
      ) : null}
      {previewLoading ? <p className="crosshook-hero-detail__muted">Building launch preview...</p> : null}
      {previewError ? <p className="crosshook-hero-detail__warn">{previewError}</p> : null}
      {preview?.umu_decision?.gameid_resolution ? (
        <div className="crosshook-hero-detail__meta-grid" role="group" aria-label="umu GAMEID resolution">
          <span>
            <strong>umu GAMEID:</strong> {preview.umu_decision.gameid_resolution.game_id}
          </span>
          <span>
            <strong>Source:</strong> {umuGameIdResolutionSourceLabel(preview.umu_decision.gameid_resolution.source)}
          </span>
          {preview.umu_decision.gameid_resolution.lookup_key ? (
            <span>
              <strong>Key:</strong> {preview.umu_decision.gameid_resolution.lookup_key.store}/
              {preview.umu_decision.gameid_resolution.lookup_key.codename}
            </span>
          ) : null}
          {preview.umu_decision.gameid_resolution.expires_at ? (
            <span>
              <strong>Expires:</strong>{' '}
              <time dateTime={preview.umu_decision.gameid_resolution.expires_at}>
                {new Date(preview.umu_decision.gameid_resolution.expires_at).toLocaleString()}
              </time>
            </span>
          ) : null}
        </div>
      ) : null}
      {preview ? <HighlightedCommandBlock preview={preview} profileName={resolvedProfileName} /> : null}
      {notSelectableHint ? (
        <p className="crosshook-hero-detail__muted" role="note">
          {notSelectableHint}
        </p>
      ) : null}
    </DashboardPanelSection>
  );
}

export default HeroLaunchCommandSection;
