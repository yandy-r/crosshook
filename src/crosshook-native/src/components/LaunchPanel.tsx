import { useEffect, useId, useState } from 'react';
import { useLaunchStateContext } from '../context/LaunchStateContext';
import { useProfileHealthContext } from '../context/ProfileHealthContext';
import {
  presentAcknowledgeVersionChangeOutcome,
  useAcknowledgeVersionChange,
} from '../hooks/useAcknowledgeVersionChange';
import { usePreviewState } from '../hooks/usePreviewState';
import { LaunchPhase } from '../types';
import { copyToClipboard } from '../utils/clipboard';
import { LaunchPipeline } from './LaunchPipeline';
import { LaunchPanelControls } from './launch-panel/LaunchPanelControls';
import { LaunchPanelFeedback } from './launch-panel/LaunchPanelFeedback';
import { LaunchPanelVersionStatus } from './launch-panel/LaunchPanelVersionStatus';
import { PreviewModal } from './launch-panel/PreviewModal';
import type { LaunchPanelProps } from './launch-panel/types';
import '../styles/preview.css';

export type { LaunchPanelProps } from './launch-panel/types';

export function LaunchPanel({
  profileId,
  method,
  request,
  profile,
  profileSelectSlot,
  beforeActions,
  infoSlot,
  tabsSlot,
  onBeforeLaunch,
}: LaunchPanelProps) {
  const profileSelect = profileSelectSlot ?? beforeActions;
  const {
    canLaunchGame,
    canLaunchTrainer,
    feedback,
    helperLogPath,
    hintText,
    isBusy,
    isGameRunning,
    launchGame,
    launchTrainer,
    phase,
    reset,
    statusText,
  } = useLaunchStateContext();

  const { loading, preview, error: previewError, requestPreview, clearPreview, previewTarget } = usePreviewState();
  const { healthByName, revalidateSingle } = useProfileHealthContext();
  const { acknowledgeVersionChange, busy: verifyBusy } = useAcknowledgeVersionChange();
  const [showPreview, setShowPreview] = useState(false);
  const [diagnosticExpanded, setDiagnosticExpanded] = useState(false);
  const [diagnosticCopyLabel, setDiagnosticCopyLabel] = useState('Copy Report');
  const launchGuidanceId = useId();

  const metadata = healthByName[profileId]?.metadata ?? null;
  const versionStatus = metadata?.version_status ?? null;

  async function handleMarkAsVerified() {
    const outcome = await acknowledgeVersionChange(profileId, revalidateSingle);
    presentAcknowledgeVersionChangeOutcome(outcome);
  }

  useEffect(() => {
    if (preview) {
      setShowPreview(true);
    }
  }, [preview]);

  const isIdle = phase === LaunchPhase.Idle;
  const diagnosticFeedback = feedback?.kind === 'diagnostic' ? feedback.report : null;
  const launchGuidanceText = [statusText, hintText].filter(Boolean).join(' — ');

  useEffect(() => {
    setDiagnosticExpanded(false);
    setDiagnosticCopyLabel('Copy Report');
  }, []);

  function handleClosePreview() {
    setShowPreview(false);
    clearPreview();
  }

  function handleLaunchFromPreview() {
    const fromTrainerPreview = previewTarget === 'trainer';
    if (isGameRunning && !fromTrainerPreview) return;
    setShowPreview(false);
    clearPreview();
    void (async () => {
      if (fromTrainerPreview) {
        if (onBeforeLaunch) {
          const proceed = await onBeforeLaunch('trainer');
          if (!proceed) return;
        }
        launchTrainer();
      } else {
        if (onBeforeLaunch) {
          const proceed = await onBeforeLaunch('game');
          if (!proceed) return;
        }
        launchGame();
      }
    })();
  }

  async function handleCopyDiagnosticReport() {
    if (!diagnosticFeedback) {
      return;
    }

    try {
      await copyToClipboard(JSON.stringify(diagnosticFeedback, null, 2));
      setDiagnosticCopyLabel('Copied!');
      window.setTimeout(() => {
        setDiagnosticCopyLabel('Copy Report');
      }, 2000);
    } catch {
      setDiagnosticCopyLabel('Copy Failed');
      window.setTimeout(() => {
        setDiagnosticCopyLabel('Copy Report');
      }, 2000);
    }
  }

  return (
    <div className="crosshook-route-stack crosshook-launch-panel-stack">
      {/* ── Launch controls (route identity lives in RouteBanner on LaunchPage) ── */}
      <div className="crosshook-panel">
        <section className="crosshook-launch-panel crosshook-route-hero-launch-panel">
          {infoSlot}

          {feedback ? (
            <LaunchPanelFeedback
              feedback={feedback}
              diagnosticExpanded={diagnosticExpanded}
              setDiagnosticExpanded={setDiagnosticExpanded}
              diagnosticCopyLabel={diagnosticCopyLabel}
              onCopyDiagnosticReport={handleCopyDiagnosticReport}
            />
          ) : null}

          <LaunchPanelControls
            profileSelect={profileSelect}
            launchGuidanceId={launchGuidanceId}
            launchGuidanceText={launchGuidanceText}
            canLaunchGame={canLaunchGame}
            canLaunchTrainer={canLaunchTrainer}
            isBusy={isBusy}
            isGameRunning={isGameRunning}
            phase={phase}
            isIdle={isIdle}
            loading={loading}
            request={request}
            onBeforeLaunch={onBeforeLaunch}
            launchGame={launchGame}
            launchTrainer={launchTrainer}
            requestPreview={requestPreview}
            reset={reset}
          />

          <div className="crosshook-launch-panel__runner-stack">
            <LaunchPipeline method={method} profile={profile} preview={preview} phase={phase} />
            {helperLogPath ? (
              <span className="crosshook-launch-panel__indicator-copy">Log: {helperLogPath}</span>
            ) : null}
            {launchGuidanceText ? (
              <p id={launchGuidanceId} className="crosshook-launch-panel__indicator-guidance">
                {launchGuidanceText}
              </p>
            ) : null}
          </div>

          <LaunchPanelVersionStatus
            versionStatus={versionStatus ?? null}
            onAcknowledge={handleMarkAsVerified}
            busy={verifyBusy}
          />

          {previewError ? (
            <p className="crosshook-preview-modal__preview-error" role="alert">
              Preview failed: {previewError}
            </p>
          ) : null}
        </section>
      </div>

      {/* ── Tabs card (passed from parent) ── */}
      {tabsSlot}

      {/* PreviewModal — portal to document.body, stays outside cards */}
      {showPreview && preview ? (
        <PreviewModal
          preview={preview}
          profileId={profileId}
          onClose={handleClosePreview}
          onLaunch={handleLaunchFromPreview}
        />
      ) : null}
    </div>
  );
}

export default LaunchPanel;
