import { type ChangeEvent, useId } from 'react';

export function FieldRow(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  helperText?: string;
  error?: string | null;
  browseLabel?: string;
  onBrowse?: () => Promise<void>;
}) {
  const inputId = useId();
  const helperId = props.helperText ? `${inputId}-helper` : undefined;
  const errorId = props.error ? `${inputId}-error` : undefined;
  const describedBy = [helperId, errorId].filter(Boolean).join(' ') || undefined;

  return (
    <div className="crosshook-field">
      <label className="crosshook-label" htmlFor={inputId}>
        {props.label}
      </label>
      <div className="crosshook-install-field-control">
        <input
          id={inputId}
          className="crosshook-input"
          style={{ flex: 1, minWidth: 0 }}
          value={props.value}
          placeholder={props.placeholder}
          aria-invalid={Boolean(props.error)}
          aria-describedby={describedBy}
          onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
        />
        {props.onBrowse ? (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void props.onBrowse?.()}
          >
            {props.browseLabel ?? 'Browse'}
          </button>
        ) : null}
      </div>
      {props.helperText ? (
        <p id={helperId} className="crosshook-help-text">
          {props.helperText}
        </p>
      ) : null}
      {props.error ? (
        <p id={errorId} className="crosshook-danger">
          {props.error}
        </p>
      ) : null}
    </div>
  );
}
