import { type ChangeEvent, useId } from 'react';
import type { ProtonInstallOption } from '../../types/proton';
import { formatProtonInstallLabel } from '../../utils/proton';
import { ThemedSelect } from '../ui/ThemedSelect';

export function ProtonPathField(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  installs: ProtonInstallOption[];
  error: string | null;
  installsError: string | null;
  onBrowse: () => Promise<void>;
}) {
  const duplicateNameCounts = props.installs.reduce<Record<string, number>>((counts, install) => {
    const key = install.name.trim() || 'Unnamed Proton install';
    counts[key] = (counts[key] ?? 0) + 1;
    return counts;
  }, {});
  const selectId = useId();
  const inputId = useId();
  const selectedInstallPath = props.installs.find((install) => install.path.trim() === props.value.trim())?.path ?? '';

  return (
    <div className="crosshook-field crosshook-install-proton-field">
      <label className="crosshook-label" htmlFor={selectId}>
        {props.label}
      </label>
      <div style={{ display: 'grid', gap: 10 }}>
        <ThemedSelect
          id={selectId}
          value={selectedInstallPath}
          onValueChange={(val) => {
            if (val.trim().length > 0) {
              props.onChange(val);
            }
          }}
          placeholder="Detected Proton install"
          options={props.installs.map((install) => ({
            value: install.path,
            label: formatProtonInstallLabel(install, duplicateNameCounts),
          }))}
        />

        <div className="crosshook-install-field-control">
          <input
            id={inputId}
            className="crosshook-input"
            style={{ flex: 1, minWidth: 0 }}
            value={props.value}
            onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
            placeholder={props.placeholder}
          />
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void props.onBrowse()}
          >
            Browse
          </button>
        </div>
      </div>

      <p className="crosshook-help-text">
        Pick a detected Proton install to fill this field automatically, or edit the path manually.
      </p>
      {props.error ? <p className="crosshook-danger">{props.error}</p> : null}
      {props.installsError ? <p className="crosshook-danger">{props.installsError}</p> : null}
    </div>
  );
}
