import { useEffect, useState } from 'react';

import { useCollectionDefaults } from '@/hooks/useCollectionDefaults';
import {
  isCollectionDefaultsEmpty,
  type CollectionDefaults,
  type LaunchMethod,
} from '@/types/profile';

import './CollectionLaunchDefaultsEditor.css';

const LAUNCH_METHOD_OPTIONS: ReadonlyArray<{ value: LaunchMethod; label: string }> = [
  { value: '', label: '(inherit)' },
  { value: 'native', label: 'native' },
  { value: 'proton_run', label: 'proton_run' },
  { value: 'steam_applaunch', label: 'steam_applaunch' },
];

interface EnvVarRow {
  id: string;
  key: string;
  value: string;
}

function omitCustomEnvVars(d: CollectionDefaults): CollectionDefaults {
  const { custom_env_vars: _omit, ...rest } = d;
  void _omit;
  return rest;
}

function recordToEnvRows(record: Record<string, string> | undefined): EnvVarRow[] {
  if (!record) return [];
  return Object.entries(record).map(([key, value]) => ({
    id: crypto.randomUUID(),
    key,
    value,
  }));
}

function envRowsToRecord(rows: EnvVarRow[]): Record<string, string> | undefined {
  const o: Record<string, string> = {};
  for (const r of rows) {
    const trimmedKey = r.key.trim();
    if (trimmedKey !== '') o[trimmedKey] = r.value;
  }
  return Object.keys(o).length ? o : undefined;
}

interface Props {
  collectionId: string;
  /**
   * Called when the user clicks "Open in Profiles page →". The host wires this
   * to a route change while preserving `activeCollectionId` so the Profiles page
   * opens inside the collection filter.
   */
  onOpenInProfilesPage: () => void;
}

/**
 * Inline editor for per-collection launch defaults. Renders inside
 * `<CollectionViewModal>` as a collapsible `<details>` block above the search
 * input. Users can set/clear `method`, `network_isolation`, and the additive
 * `custom_env_vars` map; saving writes the defaults via `collection_set_defaults`.
 *
 * Excluded from the inline editor (per PRD): `optimizations`, `gamescope`,
 * `trainer_gamescope`, `mangohud`, `presets`, `active_preset`. Users wanting
 * those overrides use the "Open in Profiles page →" link-out and edit at the
 * profile level.
 */
export function CollectionLaunchDefaultsEditor({ collectionId, onOpenInProfilesPage }: Props) {
  const { defaults, loading, error, saveDefaults } = useCollectionDefaults(collectionId);
  const [draft, setDraft] = useState<CollectionDefaults>({});
  const [envRows, setEnvRows] = useState<EnvVarRow[]>([]);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  function mergedCollectionDefaults(): CollectionDefaults {
    const env = envRowsToRecord(envRows);
    if (!env) return draft;
    return { ...draft, custom_env_vars: env };
  }

  // Re-anchor the draft when fresh defaults arrive (collection switch or
  // post-save reload). Effect (not useMemo) so the state mutation runs after
  // the render commits.
  useEffect(() => {
    setDraft(omitCustomEnvVars(defaults ?? {}));
    setEnvRows(recordToEnvRows(defaults?.custom_env_vars));
    setSaveError(null);
  }, [defaults]);

  const draftIsEmpty = isCollectionDefaultsEmpty(mergedCollectionDefaults());

  async function handleSave() {
    setSaving(true);
    setSaveError(null);
    try {
      await saveDefaults(draftIsEmpty ? null : mergedCollectionDefaults());
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  function handleResetDraft() {
    setDraft(omitCustomEnvVars(defaults ?? {}));
    setEnvRows(recordToEnvRows(defaults?.custom_env_vars));
    setSaveError(null);
  }

  function handleClearAll() {
    setDraft({});
    setEnvRows([]);
  }

  function setMethod(value: string) {
    if (value === '') {
      const { method: _omit, ...rest } = draft;
      void _omit;
      setDraft(rest);
      return;
    }
    setDraft({ ...draft, method: value as LaunchMethod });
  }

  function setNetworkIsolation(value: string) {
    if (value === '') {
      const { network_isolation: _omit, ...rest } = draft;
      void _omit;
      setDraft(rest);
      return;
    }
    setDraft({ ...draft, network_isolation: value === 'on' });
  }

  function addEnvVar() {
    setEnvRows((rows) => {
      const keys = new Set(rows.map((r) => r.key));
      let i = 1;
      let key = `NEW_VAR_${i}`;
      while (keys.has(key)) {
        i += 1;
        key = `NEW_VAR_${i}`;
      }
      return [...rows, { id: crypto.randomUUID(), key, value: '' }];
    });
  }

  function updateEnvVarKey(rowId: string, newKeyRaw: string) {
    setEnvRows((rows) =>
      rows.map((r) => (r.id === rowId ? { ...r, key: newKeyRaw } : r))
    );
  }

  function updateEnvVarValue(rowId: string, value: string) {
    setEnvRows((rows) => rows.map((r) => (r.id === rowId ? { ...r, value } : r)));
  }

  function removeEnvVar(rowId: string) {
    setEnvRows((rows) => rows.filter((r) => r.id !== rowId));
  }

  const persistedActive = !isCollectionDefaultsEmpty(defaults);

  return (
    <details className="crosshook-collection-launch-defaults-editor">
      <summary className="crosshook-collection-launch-defaults-editor__summary">
        <span>Collection launch defaults</span>
        {persistedActive && (
          <span className="crosshook-collection-launch-defaults-editor__badge">Active</span>
        )}
      </summary>

      {loading && (
        <p className="crosshook-collection-launch-defaults-editor__status">Loading defaults…</p>
      )}
      {error && (
        <p className="crosshook-collection-launch-defaults-editor__error" role="alert">
          {error}
        </p>
      )}

      {!loading && (
        <div className="crosshook-collection-launch-defaults-editor__body">
          <p className="crosshook-collection-launch-defaults-editor__hint">
            Collection defaults override the profile's launch settings, but local
            machine paths always win on top. Empty fields inherit the profile.
          </p>

          <div className="crosshook-collection-launch-defaults-editor__row">
            <label className="crosshook-label">
              Method
              <select
                className="crosshook-input"
                value={draft.method ?? ''}
                onChange={(e) => setMethod(e.target.value)}
              >
                {LAUNCH_METHOD_OPTIONS.map((opt) => (
                  <option key={opt.value || 'inherit'} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="crosshook-label">
              Network isolation
              <select
                className="crosshook-input"
                value={
                  draft.network_isolation === undefined
                    ? ''
                    : draft.network_isolation
                      ? 'on'
                      : 'off'
                }
                onChange={(e) => setNetworkIsolation(e.target.value)}
              >
                <option value="">(inherit)</option>
                <option value="on">on</option>
                <option value="off">off</option>
              </select>
            </label>
          </div>

          <fieldset className="crosshook-collection-launch-defaults-editor__env">
            <legend>Custom env vars (additive)</legend>
            {envRows.length === 0 && (
              <p className="crosshook-collection-launch-defaults-editor__hint">
                No collection env vars set. Profile env vars still apply.
              </p>
            )}
            {envRows.map((row, index) => (
              <div
                key={row.id}
                className="crosshook-collection-launch-defaults-editor__env-row"
              >
                <input
                  type="text"
                  className="crosshook-input"
                  value={row.key}
                  onChange={(e) => updateEnvVarKey(row.id, e.target.value)}
                  placeholder="KEY"
                  aria-label={
                    row.key.trim()
                      ? `Environment variable name: ${row.key}`
                      : `Environment variable name (row ${index + 1})`
                  }
                />
                <input
                  type="text"
                  className="crosshook-input"
                  value={row.value}
                  onChange={(e) => updateEnvVarValue(row.id, e.target.value)}
                  placeholder="value"
                  aria-label={`env var value for ${row.key || 'new row'}`}
                />
                <button
                  type="button"
                  className="crosshook-button crosshook-button--ghost"
                  onClick={() => removeEnvVar(row.id)}
                  aria-label={`Remove env var row`}
                >
                  ×
                </button>
              </div>
            ))}
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={addEnvVar}
            >
              + Add env var
            </button>
          </fieldset>

          <p className="crosshook-collection-launch-defaults-editor__hint">
            Optimizations, gamescope, and MangoHUD overrides are managed from the
            Profiles page.
          </p>

          <div className="crosshook-collection-launch-defaults-editor__actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={onOpenInProfilesPage}
            >
              Open in Profiles page →
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={handleClearAll}
            >
              Clear all
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={handleResetDraft}
            >
              Reset draft
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--primary"
              onClick={handleSave}
              disabled={saving}
            >
              {saving ? 'Saving…' : 'Save'}
            </button>
          </div>

          {saveError && (
            <p className="crosshook-collection-launch-defaults-editor__error" role="alert">
              {saveError}
            </p>
          )}
        </div>
      )}
    </details>
  );
}
