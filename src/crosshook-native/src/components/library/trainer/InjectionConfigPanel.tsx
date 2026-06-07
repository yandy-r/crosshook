import type { InjectionFallback, InjectionMethod, InjectionSection, InjectionStage } from '@/types/profile';
import { ThemedSelect } from '../../ui/ThemedSelect';

export interface InjectionConfigPanelProps {
  injection: InjectionSection;
  onUpdate: (injection: InjectionSection) => void;
}

const methodOptions: Array<{ value: InjectionMethod; label: string }> = [
  { value: 'disabled', label: 'Disabled' },
  { value: 'load_library', label: 'LoadLibrary' },
  { value: 'manual_map', label: 'Manual map' },
];

const stageOptions: Array<{ value: InjectionStage; label: string }> = [
  { value: 'trainer_launch', label: 'Trainer launch' },
  { value: 'game_process_ready', label: 'Game process ready' },
  { value: 'manual', label: 'Manual' },
];

const fallbackOptions: Array<{ value: InjectionFallback; label: string }> = [
  { value: 'warn_and_continue', label: 'Warn and continue' },
  { value: 'disable_hook', label: 'Disable hook' },
  { value: 'abort_launch', label: 'Abort launch' },
];

function clampTimeout(value: string): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return 0;
  }
  return parsed;
}

export function InjectionConfigPanel({ injection, onUpdate }: InjectionConfigPanelProps) {
  function patchInjection(patch: Partial<InjectionSection>) {
    onUpdate({ ...injection, ...patch });
  }

  return (
    <div className="crosshook-hero-detail__trainer-config">
      <div className="crosshook-hero-detail__hook-banner">
        <p>DLL injection settings are stored on the profile. No DLL injection engine runs from this editor yet.</p>
        <span className="crosshook-hero-detail__trainer-status">Stored only</span>
      </div>

      <div className="crosshook-hero-detail__trainer-config-grid">
        <div className="crosshook-field">
          <label className="crosshook-label" htmlFor="crosshook-trainer-injection-method">
            Method
          </label>
          <ThemedSelect
            id="crosshook-trainer-injection-method"
            value={injection.method}
            onValueChange={(value) => patchInjection({ method: value as InjectionMethod })}
            options={methodOptions}
          />
          <p className="crosshook-help-text">Stored preference for a future trainer injection runtime.</p>
        </div>

        <div className="crosshook-field">
          <label className="crosshook-label" htmlFor="crosshook-trainer-injection-stage">
            Stage
          </label>
          <ThemedSelect
            id="crosshook-trainer-injection-stage"
            value={injection.stage}
            onValueChange={(value) => patchInjection({ stage: value as InjectionStage })}
            options={stageOptions}
          />
          <p className="crosshook-help-text">Defines when a stored DLL hook would be considered eligible.</p>
        </div>

        <div className="crosshook-field">
          <label className="crosshook-label" htmlFor="crosshook-trainer-injection-timeout">
            Timeout (ms)
          </label>
          <input
            id="crosshook-trainer-injection-timeout"
            type="number"
            min="0"
            step="100"
            className="crosshook-input"
            value={injection.timeout_ms}
            onChange={(event) => patchInjection({ timeout_ms: clampTimeout(event.currentTarget.value) })}
          />
          <p className="crosshook-help-text">Stored wait budget for future injection support. Use 0 for no wait.</p>
        </div>

        <div className="crosshook-field">
          <label className="crosshook-label" htmlFor="crosshook-trainer-injection-fallback">
            Fallback
          </label>
          <ThemedSelect
            id="crosshook-trainer-injection-fallback"
            value={injection.fallback}
            onValueChange={(value) => patchInjection({ fallback: value as InjectionFallback })}
            options={fallbackOptions}
          />
          <p className="crosshook-help-text">Stored behavior for future runtime failures.</p>
        </div>
      </div>
    </div>
  );
}

export default InjectionConfigPanel;
