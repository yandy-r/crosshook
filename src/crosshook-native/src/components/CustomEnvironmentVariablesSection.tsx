import { useEffect, useMemo, useState, type ChangeEvent } from 'react';

import type { GameProfile } from '../types';

/** Mirrors `RESERVED_CUSTOM_ENV_KEYS` in crosshook-core `launch/request.rs`. */
const RESERVED_CUSTOM_ENV_KEYS = new Set([
  'WINEPREFIX',
  'STEAM_COMPAT_DATA_PATH',
  'STEAM_COMPAT_CLIENT_INSTALL_PATH',
]);

type CustomEnvVarRow = { id: string; key: string; value: string };

function recordToCustomEnvRows(record: Record<string, string>): CustomEnvVarRow[] {
  return Object.entries(record).map(([key, value]) => ({
    id: crypto.randomUUID(),
    key,
    value,
  }));
}

function customEnvRowsToRecord(rows: CustomEnvVarRow[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const row of rows) {
    if (row.key.trim().length === 0) {
      continue;
    }
    out[row.key] = row.value;
  }
  return out;
}

function customEnvRecordSignature(record: Record<string, string>): string {
  const sortedEntries = Object.entries(record).sort(([a], [b]) => a.localeCompare(b));
  return JSON.stringify(sortedEntries);
}

function customEnvKeyFieldError(key: string): string | null {
  const trimmed = key.trim();
  if (trimmed.length === 0) {
    return null;
  }
  if (key.includes('=')) {
    return 'Use a non-empty key without "=" characters.';
  }
  if (key.includes('\0')) {
    return 'Key cannot contain NUL characters.';
  }
  if (RESERVED_CUSTOM_ENV_KEYS.has(trimmed)) {
    return 'This key is managed by CrossHook runtime and cannot be overridden.';
  }
  return null;
}

function customEnvDuplicateRowIds(rows: CustomEnvVarRow[]): Set<string> {
  const counts = new Map<string, number>();
  for (const row of rows) {
    const trimmed = row.key.trim();
    if (!trimmed) {
      continue;
    }
    counts.set(trimmed, (counts.get(trimmed) ?? 0) + 1);
  }
  const dup = new Set<string>();
  for (const row of rows) {
    const trimmed = row.key.trim();
    if (trimmed && (counts.get(trimmed) ?? 0) > 1) {
      dup.add(row.id);
    }
  }
  return dup;
}

function customEnvRowError(row: CustomEnvVarRow, duplicateIds: Set<string>): string | null {
  if (duplicateIds.has(row.id)) {
    return 'This key already exists in the table.';
  }
  const keyErr = customEnvKeyFieldError(row.key);
  if (keyErr) {
    return keyErr;
  }
  if (row.value.includes('\0')) {
    return 'Value cannot contain NUL characters.';
  }
  if (row.key.trim().length === 0 && row.value.trim().length > 0) {
    return 'Enter a variable name for this row.';
  }
  return null;
}

export interface CustomEnvironmentVariablesSectionProps {
  profileName: string;
  customEnvVars: Record<string, string>;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  idPrefix: string;
}

export function CustomEnvironmentVariablesSection(props: CustomEnvironmentVariablesSectionProps) {
  const { profileName, customEnvVars, onUpdateProfile, idPrefix } = props;
  const [rows, setRows] = useState<CustomEnvVarRow[]>(() => recordToCustomEnvRows(customEnvVars));
  const customEnvVarsSignature = useMemo(
    () => JSON.stringify([profileName, customEnvRecordSignature(customEnvVars)]),
    [profileName, customEnvVars],
  );

  useEffect(() => {
    setRows((currentRows) => {
      const currentSignature = JSON.stringify([
        profileName,
        customEnvRecordSignature(customEnvRowsToRecord(currentRows)),
      ]);
      if (currentSignature === customEnvVarsSignature) {
        return currentRows;
      }
      return recordToCustomEnvRows(customEnvVars);
    });
  }, [profileName, customEnvVars, customEnvVarsSignature]);

  const duplicateIds = useMemo(() => customEnvDuplicateRowIds(rows), [rows]);

  const applyRows = (next: CustomEnvVarRow[]) => {
    setRows(next);
    onUpdateProfile((current) => ({
      ...current,
      launch: {
        ...current.launch,
        custom_env_vars: customEnvRowsToRecord(next),
      },
    }));
  };

  const precedenceId = `${idPrefix}-custom-env-precedence`;

  return (
    <div className="crosshook-install-section">
      <div className="crosshook-install-section-title">Custom Environment Variables</div>
      <p className="crosshook-help-text" id={precedenceId}>
        Custom variables override built-in launch optimization variables when keys conflict.
      </p>
      {rows.length === 0 ? (
        <p className="crosshook-help-text">No custom variables configured for this profile.</p>
      ) : null}

      <div className="crosshook-custom-env-rows">
        {rows.map((row) => {
          const rowErr = customEnvRowError(row, duplicateIds);
          const rowErrorId = `${idPrefix}-custom-env-err-${row.id}`;
          const keyInputId = `${idPrefix}-custom-env-key-${row.id}`;
          const valueInputId = `${idPrefix}-custom-env-val-${row.id}`;
          const valueInvalid = row.value.includes('\0');
          const keyInvalid = Boolean(rowErr) && !valueInvalid;
          const describeKey =
            [keyInvalid ? rowErrorId : '', precedenceId].filter(Boolean).join(' ') || undefined;
          const describeValue =
            [valueInvalid ? rowErrorId : '', precedenceId].filter(Boolean).join(' ') || undefined;

          return (
            <div key={row.id} className="crosshook-custom-env-row">
              <div className="crosshook-custom-env-fields">
                <div className="crosshook-field">
                  <label className="crosshook-label" htmlFor={keyInputId}>
                    Key
                  </label>
                  <input
                    id={keyInputId}
                    className="crosshook-input"
                    value={row.key}
                    placeholder="DXVK_HUD"
                    aria-invalid={keyInvalid}
                    aria-describedby={describeKey}
                    onChange={(event: ChangeEvent<HTMLInputElement>) => {
                      const nextKey = event.target.value;
                      applyRows(rows.map((r) => (r.id === row.id ? { ...r, key: nextKey } : r)));
                    }}
                  />
                </div>
                <div className="crosshook-field">
                  <label className="crosshook-label" htmlFor={valueInputId}>
                    Value
                  </label>
                  <input
                    id={valueInputId}
                    className="crosshook-input"
                    value={row.value}
                    placeholder="1"
                    aria-invalid={valueInvalid}
                    aria-describedby={describeValue}
                    onChange={(event: ChangeEvent<HTMLInputElement>) => {
                      const nextVal = event.target.value;
                      applyRows(rows.map((r) => (r.id === row.id ? { ...r, value: nextVal } : r)));
                    }}
                  />
                </div>
                <div className="crosshook-field">
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--secondary"
                    aria-label="Remove this environment variable row"
                    onClick={() => applyRows(rows.filter((r) => r.id !== row.id))}
                  >
                    Remove
                  </button>
                </div>
              </div>
              {rowErr ? (
                <p id={rowErrorId} className="crosshook-danger" role="alert">
                  {rowErr}
                </p>
              ) : null}
            </div>
          );
        })}
      </div>

      <button
        type="button"
        className="crosshook-button crosshook-button--secondary crosshook-custom-env-add"
        onClick={() => applyRows([...rows, { id: crypto.randomUUID(), key: '', value: '' }])}
      >
        Add variable
      </button>
    </div>
  );
}
