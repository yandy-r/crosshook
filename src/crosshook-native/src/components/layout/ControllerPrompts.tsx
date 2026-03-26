export function ControllerPrompts() {
  return (
    <div className="crosshook-controller-prompts" aria-label="Controller shortcuts">
      <div className="crosshook-controller-prompts__surface" role="status" aria-live="polite">
        <div className="crosshook-controller-prompts__item">
          <span className="crosshook-controller-prompts__glyph" aria-hidden="true">
            A
          </span>
          <span className="crosshook-controller-prompts__label">Select</span>
        </div>
        <div className="crosshook-controller-prompts__item">
          <span className="crosshook-controller-prompts__glyph" aria-hidden="true">
            B
          </span>
          <span className="crosshook-controller-prompts__label">Back</span>
        </div>
        <div className="crosshook-controller-prompts__item">
          <span className="crosshook-controller-prompts__glyph" aria-hidden="true">
            LB / RB
          </span>
          <span className="crosshook-controller-prompts__label">Switch View</span>
        </div>
      </div>
    </div>
  );
}

export default ControllerPrompts;
