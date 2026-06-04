/**
 * HeroLaunchGate — in-place launch orchestrator for the Hero Detail tab.
 *
 * Mirrors the LaunchPanel + LaunchPage approach:
 * 1. Ensures the displayed profile is selected into ProfileContext before launch.
 * 2. Routes launch actions through `useLaunchDepGate` (dependency gate modal).
 * 3. Calls `useLaunchStateContext` mutators (`launchGame` / `launchTrainer`).
 * 4. Renders `LaunchPanelFeedback`, `LaunchPipeline`, helper log, and guidance.
 * 5. Exposes `isGamescopeRunning` to `HeroLaunchSubTabsHost`.
 *
 * The component does NOT navigate — all state stays local to the Hero Detail tab.
 */
import { useCallback, useId, useState } from 'react';
import { useLaunchStateContext } from '@/context/LaunchStateContext';
import { usePreferencesContext } from '@/context/PreferencesContext';
import { useProfileContext } from '@/context/ProfileContext';
import { LaunchPhase } from '@/types';
import { copyToClipboard } from '@/utils/clipboard';
import { resolveLaunchMethod } from '@/utils/launch';
import { LaunchPipeline } from '../../LaunchPipeline';
import { LaunchPanelFeedback } from '../../launch-panel/LaunchPanelFeedback';
import { LaunchDepGateModal } from '../../pages/launch/LaunchDepGateModal';
import { useLaunchDepGate } from '../../pages/launch/useLaunchDepGate';
import type { HeroLaunchCommandSectionProps } from './HeroLaunchCommandSection';
import { HeroLaunchCommandSection } from './HeroLaunchCommandSection';
import type { HeroLaunchSubTabsHostProps } from './HeroLaunchSubTabsHost';
import { HeroLaunchSubTabsHost } from './HeroLaunchSubTabsHost';

export interface HeroLaunchGateProps
  extends Pick<
      HeroLaunchCommandSectionProps,
      'launchRequest' | 'previewLoading' | 'preview' | 'previewError' | 'onPreviewLaunch'
    >,
    Pick<HeroLaunchSubTabsHostProps, 'resolvedSteamAppId' | 'hasSavedSelectedProfile' | 'profileMismatch'> {
  /**
   * The resolved profile name (display name for the active profile).
   * Passed to both `HeroLaunchCommandSection` and `HeroLaunchSubTabsHost`.
   */
  resolvedProfileName: string;
  /**
   * The raw `displayProfileName` prop from `HeroDetailLaunchTab`.
   * Used to select the profile into ProfileContext before launching.
   */
  displayProfileName: string;
  /**
   * Whether the launch gate is in a "launching" state (from the parent).
   */
  isLaunching: boolean;
}

/**
 * Whether the displayed profile can be selected into ProfileContext.
 * A fallback profile (not in the saved profiles list) cannot be selected —
 * in that case we disable launch but still render the section.
 */
function canSelectProfile(displayProfileName: string, profiles: string[]): boolean {
  const trimmed = displayProfileName.trim();
  return trimmed.length > 0 && profiles.includes(trimmed);
}

export function HeroLaunchGate({
  launchRequest,
  previewLoading,
  preview,
  previewError,
  onPreviewLaunch,
  resolvedProfileName,
  resolvedSteamAppId,
  hasSavedSelectedProfile,
  profileMismatch,
  displayProfileName,
  isLaunching,
}: HeroLaunchGateProps) {
  const { profile, selectedProfile, profiles, selectProfile, activeCollectionId } = useProfileContext();
  const { settings } = usePreferencesContext();

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
    statusText,
  } = useLaunchStateContext();

  // ── Dep gate (mirrors LaunchPage.tsx:39-43) ────────────────────────────────
  const depGate = useLaunchDepGate({
    profile,
    selectedName: selectedProfile,
    autoInstallPrefixDeps: settings.auto_install_prefix_deps,
  });

  // ── Diagnostic report copy ─────────────────────────────────────────────────
  const [diagnosticExpanded, setDiagnosticExpanded] = useState(false);
  const [diagnosticCopyLabel, setDiagnosticCopyLabel] = useState('Copy Report');
  const launchGuidanceId = useId();

  const launchGuidanceText = [statusText, hintText].filter(Boolean).join(' — ');

  const handleCopyDiagnosticReport = useCallback(async () => {
    if (feedback?.kind !== 'diagnostic') return;
    try {
      await copyToClipboard(JSON.stringify(feedback.report, null, 2));
      setDiagnosticCopyLabel('Copied!');
      window.setTimeout(() => setDiagnosticCopyLabel('Copy Report'), 2000);
    } catch {
      setDiagnosticCopyLabel('Copy Failed');
      window.setTimeout(() => setDiagnosticCopyLabel('Copy Report'), 2000);
    }
  }, [feedback]);

  // ── selectProfile-first gate ───────────────────────────────────────────────
  // LaunchStateContext builds its LaunchRequest from ProfileContext's *selected*
  // profile. Before any launch, we ensure the displayed profile is selected.
  // Mirrors LibraryPage.tsx:170 (`await selectProfile(name, { collectionId })`).
  const profileSelectable = canSelectProfile(displayProfileName, profiles);

  const handleBeforeLaunch = useCallback(
    async (action: 'game' | 'trainer'): Promise<boolean> => {
      const displayedTrimmed = displayProfileName.trim();
      const selectedTrimmed = selectedProfile.trim();

      // Only select if the profile differs from what is already selected.
      if (displayedTrimmed.length > 0 && displayedTrimmed !== selectedTrimmed) {
        if (!profiles.includes(displayedTrimmed)) {
          // Fallback profile — cannot select, abort launch.
          return false;
        }
        await selectProfile(displayedTrimmed, {
          collectionId: activeCollectionId ?? undefined,
        });
      }

      // Delegate to the dependency gate.
      return depGate.handleBeforeLaunch(action);
    },
    [displayProfileName, selectedProfile, profiles, selectProfile, activeCollectionId, depGate.handleBeforeLaunch]
  );

  // ── Derived flags ──────────────────────────────────────────────────────────
  const isIdle = phase === LaunchPhase.Idle;
  const launchMethod = resolveLaunchMethod(profile);

  // Gate canLaunchGame/canLaunchTrainer on profileSelectable — buttons are
  // disabled when the profile is a read-only fallback that cannot be selected.
  const effectiveCanLaunchGame = canLaunchGame && profileSelectable;
  const effectiveCanLaunchTrainer = canLaunchTrainer && profileSelectable;

  // Hint shown when the profile is not in the saved profiles list.
  const notSelectableHint =
    !profileSelectable && displayProfileName.trim().length > 0
      ? 'This profile is not saved and cannot be launched from the library. Open it in the editor to save and launch.'
      : null;

  return (
    <div className="crosshook-hero-detail__launch-gate">
      {/* ── Feedback (error / diagnostic / runtime message) ── */}
      {feedback ? (
        <LaunchPanelFeedback
          feedback={feedback}
          diagnosticExpanded={diagnosticExpanded}
          setDiagnosticExpanded={setDiagnosticExpanded}
          diagnosticCopyLabel={diagnosticCopyLabel}
          onCopyDiagnosticReport={handleCopyDiagnosticReport}
        />
      ) : null}

      {/* ── Launch command block + action row ── */}
      <HeroLaunchCommandSection
        launchRequest={launchRequest}
        previewLoading={previewLoading}
        preview={preview}
        previewError={previewError}
        resolvedProfileName={resolvedProfileName}
        isLaunching={isLaunching}
        onPreviewLaunch={onPreviewLaunch}
        canLaunchGame={effectiveCanLaunchGame}
        canLaunchTrainer={effectiveCanLaunchTrainer}
        isBusy={isBusy}
        isGameRunning={isGameRunning}
        phase={phase}
        isIdle={isIdle}
        onBeforeLaunch={handleBeforeLaunch}
        onLaunchGame={launchGame}
        onLaunchTrainer={launchTrainer}
        notSelectableHint={notSelectableHint}
      />

      {/* ── Pipeline visualization + log path + guidance ── */}
      <div className="crosshook-launch-panel__runner-stack">
        <LaunchPipeline method={launchMethod} profile={profile} preview={preview} phase={phase} />
        {helperLogPath ? <span className="crosshook-launch-panel__indicator-copy">Log: {helperLogPath}</span> : null}
        {launchGuidanceText ? (
          <p id={launchGuidanceId} className="crosshook-launch-panel__indicator-guidance">
            {launchGuidanceText}
          </p>
        ) : null}
      </div>

      {/* ── Sub-tabs (Environment, Gamescope, MangoHud, etc.) ── */}
      <HeroLaunchSubTabsHost
        resolvedProfileName={resolvedProfileName}
        resolvedSteamAppId={resolvedSteamAppId}
        hasSavedSelectedProfile={hasSavedSelectedProfile}
        profileMismatch={profileMismatch}
        isGamescopeRunning={depGate.isGamescopeRunning}
      />

      {/* ── Dependency gate modal ── */}
      <LaunchDepGateModal depGate={depGate} profile={profile} selectedName={selectedProfile} />
    </div>
  );
}

export default HeroLaunchGate;
