import type { PendingProtonDbOverwrite } from '../utils/protondb';

export interface ProtonDbOverwriteConfirmationProps {
  pendingProtonDbOverwrite: PendingProtonDbOverwrite;
  onUpdateProtonDbResolution: (key: string, choice: 'keep_current' | 'use_suggestion') => void;
  onCancelProtonDbOverwrite: () => void;
  onConfirmProtonDbOverwrite: (selectedKeys: readonly string[]) => void;
}

export function ProtonDbOverwriteConfirmation({
  pendingProtonDbOverwrite,
  onUpdateProtonDbResolution,
  onCancelProtonDbOverwrite,
  onConfirmProtonDbOverwrite,
}: ProtonDbOverwriteConfirmationProps) {
  return (
    <fieldset className="crosshook-protondb-card__recommendation-group crosshook-fieldset-reset">
      <legend className="crosshook-visually-hidden">ProtonDB overwrite confirmation</legend>
      <div className="crosshook-protondb-card__meta">
        <h3 className="crosshook-protondb-card__recommendation-group-title">
          Confirm conflicting environment-variable updates
        </h3>
        <p className="crosshook-protondb-card__recommendation-group-copy">
          Choose per key whether CrossHook should keep the current profile value or use the ProtonDB suggestion.
        </p>
      </div>

      <div className="crosshook-protondb-card__recommendation-list">
        {pendingProtonDbOverwrite.conflicts.map((conflict) => {
          const resolution = pendingProtonDbOverwrite.resolutions[conflict.key] ?? 'keep_current';
          return (
            <div key={conflict.key} className="crosshook-protondb-card__recommendation-item">
              <p className="crosshook-protondb-card__recommendation-label">
                <code>{conflict.key}</code>
              </p>
              <p className="crosshook-protondb-card__recommendation-note">
                Current: <code>{conflict.currentValue}</code>
              </p>
              <p className="crosshook-protondb-card__recommendation-note">
                Suggested: <code>{conflict.suggestedValue}</code>
              </p>
              <div className="crosshook-protondb-card__actions">
                <button
                  type="button"
                  className="crosshook-button crosshook-button--secondary"
                  onClick={() => onUpdateProtonDbResolution(conflict.key, 'keep_current')}
                >
                  {resolution === 'keep_current' ? 'Keeping current value' : 'Keep current'}
                </button>
                <button
                  type="button"
                  className="crosshook-button"
                  onClick={() => onUpdateProtonDbResolution(conflict.key, 'use_suggestion')}
                >
                  {resolution === 'use_suggestion' ? 'Using suggestion' : 'Use suggestion'}
                </button>
              </div>
            </div>
          );
        })}
      </div>

      <div className="crosshook-protondb-card__actions">
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={onCancelProtonDbOverwrite}
        >
          Cancel
        </button>
        <button
          type="button"
          className="crosshook-button"
          onClick={() =>
            onConfirmProtonDbOverwrite(
              Object.entries(pendingProtonDbOverwrite.resolutions)
                .filter(([, resolution]) => resolution === 'use_suggestion')
                .map(([key]) => key)
            )
          }
        >
          Apply selected changes
        </button>
      </div>
    </fieldset>
  );
}

export default ProtonDbOverwriteConfirmation;
