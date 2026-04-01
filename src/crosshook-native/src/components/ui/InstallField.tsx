import type { ChangeEvent } from 'react';

import { chooseDirectory, chooseFile } from '../../utils/dialog';

export function InstallField(props: {
  id?: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  browseLabel?: string;
  browseTitle?: string;
  browseMode?: 'file' | 'directory';
  browseFilters?: { name: string; extensions: string[] }[];
  helpText?: string;
  error?: string | null;
  className?: string;
}) {
  return (
    <div className={props.className ? `crosshook-field ${props.className}` : 'crosshook-field'}>
      <label htmlFor={props.id} className="crosshook-label">
        {props.label}
      </label>
      <div className="crosshook-install-field-control">
        <input
          id={props.id}
          className="crosshook-input"
          style={{ flex: 1, minWidth: 0 }}
          value={props.value}
          placeholder={props.placeholder}
          onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
        />
        {props.browseLabel ? (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={async () => {
              const path =
                props.browseMode === 'directory'
                  ? await chooseDirectory(props.browseTitle ?? `Select ${props.label}`)
                  : await chooseFile(props.browseTitle ?? `Select ${props.label}`, props.browseFilters);

              if (path) {
                props.onChange(path);
              }
            }}
          >
            {props.browseLabel}
          </button>
        ) : null}
      </div>
      {props.helpText ? <p className="crosshook-help-text">{props.helpText}</p> : null}
      {props.error ? <p className="crosshook-danger">{props.error}</p> : null}
    </div>
  );
}

export default InstallField;
