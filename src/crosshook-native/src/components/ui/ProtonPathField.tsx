import type { ChangeEvent } from 'react';
import { chooseFile } from '../../utils/dialog';
import { formatProtonInstallLabel } from '../../utils/proton';
import type { ProtonInstallOption } from '../ProfileFormSections';
import { ThemedSelect } from './ThemedSelect';

export function ProtonPathField(props: {
  value: string;
  onChange: (value: string) => void;
  error?: string | null;
  installs: ProtonInstallOption[];
  installsError: string | null;
  idPrefix?: string;
}) {
  const prefix = props.idPrefix ?? 'install';
  const duplicateNameCounts = props.installs.reduce<Record<string, number>>((counts, install) => {
    const key = install.name.trim() || 'Unnamed Proton install';
    counts[key] = (counts[key] ?? 0) + 1;
    return counts;
  }, {});
  const selectedPath = props.installs.find((install) => install.path.trim() === props.value.trim())?.path ?? '';

  return (
    <div className="crosshook-field crosshook-install-proton-field">
      <label className="crosshook-label" htmlFor={`${prefix}-detected-proton`}>
        Proton Path
      </label>
      <div style={{ display: 'grid', gap: 10 }}>
        <ThemedSelect
          id={`${prefix}-detected-proton`}
          value={selectedPath}
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
            id={`${prefix}-proton-path`}
            className="crosshook-input"
            style={{ flex: 1, minWidth: 0 }}
            value={props.value}
            onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
            placeholder="/home/user/.steam/root/steamapps/common/Proton - Experimental/proton"
          />
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={async () => {
              const path = await chooseFile('Select Proton Executable');
              if (path) {
                props.onChange(path);
              }
            }}
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

export default ProtonPathField;
