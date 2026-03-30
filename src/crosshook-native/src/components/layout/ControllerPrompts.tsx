interface ControllerPromptsProps {
  confirmLabel?: string;
  backLabel?: string;
  showBumpers?: boolean;
}

export function ControllerPrompts({
  confirmLabel = 'Select',
  backLabel = 'Back',
  showBumpers = true,
}: ControllerPromptsProps) {
  return (
    <div className="crosshook-controller-prompts" aria-label="Controller shortcuts">
      <div className="crosshook-controller-prompts__surface" role="status" aria-live="polite">
        <div className="crosshook-controller-prompts__item">
          <span className="crosshook-controller-prompts__glyph" aria-hidden="true">
            A
          </span>
          <span className="crosshook-controller-prompts__label">{confirmLabel}</span>
        </div>
        <div className="crosshook-controller-prompts__item">
          <span className="crosshook-controller-prompts__glyph" aria-hidden="true">
            B
          </span>
          <span className="crosshook-controller-prompts__label">{backLabel}</span>
        </div>
        {showBumpers && (
          <div className="crosshook-controller-prompts__item">
            <span className="crosshook-controller-prompts__glyph" aria-hidden="true">
              LB / RB
            </span>
            <span className="crosshook-controller-prompts__label">Switch View</span>
          </div>
        )}
      </div>
    </div>
  );
}

export default ControllerPrompts;
