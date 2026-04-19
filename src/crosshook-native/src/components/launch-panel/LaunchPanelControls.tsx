import type { ReactNode } from 'react';
import type { LaunchRequest } from '../../types';
import { LaunchPhase } from '../../types';
import { LAUNCH_PANEL_ACTION_BUTTON_STYLE } from '../../utils/launchPanelActionButtonStyle';
import { buildGameOnlyRequest, buildTrainerOnlyRequest } from './helpers';

interface LaunchPanelControlsProps {
  profileSelect: ReactNode;
  launchGuidanceId: string;
  launchGuidanceText: string;
  canLaunchGame: boolean;
  canLaunchTrainer: boolean;
  isBusy: boolean;
  isGameRunning: boolean;
  phase: LaunchPhase;
  isIdle: boolean;
  loading: boolean;
  request: LaunchRequest | null;
  onBeforeLaunch: ((action: 'game' | 'trainer') => Promise<boolean>) | undefined;
  launchGame: () => void;
  launchTrainer: () => void;
  requestPreview: (request: LaunchRequest) => void;
  reset: () => void;
}

export function LaunchPanelControls({
  profileSelect,
  launchGuidanceId,
  launchGuidanceText,
  canLaunchGame,
  canLaunchTrainer,
  isBusy,
  isGameRunning,
  phase,
  isIdle,
  loading,
  request,
  onBeforeLaunch,
  launchGame,
  launchTrainer,
  requestPreview,
  reset,
}: LaunchPanelControlsProps) {
  const previewDisabled = !request || !isIdle || loading;

  return (
    <div className="crosshook-launch-panel__profile-row crosshook-launch-panel__profile-row--wrap-centered">
      <label
        id="launch-active-profile-label"
        className="crosshook-label"
        htmlFor="launch-profile-selector"
        style={{ margin: 0, whiteSpace: 'nowrap' }}
      >
        Active Profile
      </label>
      <div className="crosshook-launch-panel__profile-row-select">{profileSelect}</div>
      <div className="crosshook-launch-panel__profile-row-actions">
        <button
          type="button"
          className="crosshook-button crosshook-launch-panel__action"
          style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
          onClick={() => {
            void (async () => {
              if (onBeforeLaunch) {
                const proceed = await onBeforeLaunch('game');
                if (!proceed) return;
              }
              launchGame();
            })();
          }}
          disabled={!canLaunchGame}
          aria-describedby={launchGuidanceText ? launchGuidanceId : undefined}
        >
          {isGameRunning
            ? 'Game Running'
            : isBusy && phase === LaunchPhase.GameLaunching
              ? 'Launching\u2026'
              : 'Launch Game'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-launch-panel__action"
          style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
          onClick={() => {
            void (async () => {
              if (onBeforeLaunch) {
                const proceed = await onBeforeLaunch('trainer');
                if (!proceed) return;
              }
              launchTrainer();
            })();
          }}
          disabled={!canLaunchTrainer}
          aria-describedby={launchGuidanceText ? launchGuidanceId : undefined}
        >
          {isBusy && phase === LaunchPhase.TrainerLaunching ? 'Launching\u2026' : 'Launch Trainer'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
          style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
          onClick={() => request && requestPreview(buildGameOnlyRequest(request))}
          disabled={previewDisabled}
        >
          {loading ? 'Loading Preview\u2026' : 'Preview Game'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
          style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
          onClick={() => request && requestPreview(buildTrainerOnlyRequest(request))}
          disabled={previewDisabled}
        >
          {loading ? 'Loading Preview\u2026' : 'Preview Trainer'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
          style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
          onClick={reset}
        >
          Reset
        </button>
      </div>
    </div>
  );
}
