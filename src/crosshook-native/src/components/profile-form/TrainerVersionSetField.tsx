import { type ChangeEvent, useId, useState } from 'react';
import { useSetTrainerVersion } from '../../hooks/useSetTrainerVersion';

export function TrainerVersionSetField({
  profileName,
  onVersionSet,
}: {
  profileName: string;
  onVersionSet?: () => void;
}) {
  const [pendingVersion, setPendingVersion] = useState('');
  const inputId = useId();
  const { setting, error, success, setVersion, clearSuccess } = useSetTrainerVersion(profileName, onVersionSet);

  const handleSet = async () => {
    if (setting || pendingVersion.trim().length === 0) {
      return;
    }
    const saved = await setVersion(pendingVersion);
    if (saved) {
      setPendingVersion('');
    }
  };

  return (
    <div className="crosshook-field">
      <label className="crosshook-label" htmlFor={inputId}>
        Set Trainer Version
      </label>
      <div className="crosshook-install-field-control">
        <input
          id={inputId}
          className="crosshook-input"
          style={{ flex: 1, minWidth: 0 }}
          value={pendingVersion}
          placeholder="e.g. v1.0.2 or 2024.01.15"
          onChange={(event: ChangeEvent<HTMLInputElement>) => {
            setPendingVersion(event.target.value);
            clearSuccess();
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') void handleSet();
          }}
        />
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => void handleSet()}
          disabled={setting || !pendingVersion.trim()}
        >
          {setting ? 'Saving...' : 'Set'}
        </button>
      </div>
      <p className="crosshook-help-text">Manually record the trainer version when it cannot be auto-detected.</p>
      {error ? <p className="crosshook-danger">{error}</p> : null}
      {success ? (
        <p className="crosshook-help-text" role="status">
          Trainer version saved.
        </p>
      ) : null}
    </div>
  );
}
