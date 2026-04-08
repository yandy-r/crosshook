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
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  // Re-anchor the draft when fresh defaults arrive (collection switch or
  // post-save reload). Effect (not useMemo) so the state mutation runs after
  // the render commits.
  useEffect(() => {
    setDraft(defaults ?? {});
    setSaveError(null);
  }, [defaults]);

  const draftIsEmpty = isCollectionDefaultsEmpty(draft);

  async function handleSave() {
    setSaving(true);
    setSaveError(null);
    try {
      await saveDefaults(draftIsEmpty ? null : draft);
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  function handleResetDraft() {
    setDraft(defaults ?? {});
    setSaveError(null);
  }

  function handleClearAll() {
    setDraft({});
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
    const nextVars = { ...(draft.custom_env_vars ?? {}) };
    let i = 1;
    let key = `NEW_VAR_${i}`;
    while (key in nextVars) {
      i += 1;
      key = `NEW_VAR_${i}`;
    }
    nextVars[key] = '';
    setDraft({ ...draft, custom_env_vars: nextVars });
  }

  function updateEnvVarKey(oldKey: string, newKey: string) {
    const trimmed = newKey;
    const nextVars: Record<string, string> = {};
    for (const [k, v] of Object.entries(draft.custom_env_vars ?? {})) {
      if (k === oldKey) {
        if (trimmed.trim() !== '') nextVars[trimmed] = v;
      } else {
        nextVars[k] = v;
      }
    }
    setDraft({ ...draft, custom_env_vars: nextVars });
  }

  function updateEnvVarValue(key: string, value: string) {
    const nextVars = { ...(draft.custom_env_vars ?? {}) };
    nextVars[key] = value;
    setDraft({ ...draft, custom_env_vars: nextVars });
  }

  function removeEnvVar(key: string) {
    const nextVars = { ...(draft.custom_env_vars ?? {}) };
    delete nextVars[key];
    if (Object.keys(nextVars).length === 0) {
      const { custom_env_vars: _omit, ...rest } = draft;
      void _omit;
      setDraft(rest);
      return;
    }
    setDraft({ ...draft, custom_env_vars: nextVars });
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
            {Object.entries(draft.custom_env_vars ?? {}).length === 0 && (
              <p className="crosshook-collection-launch-defaults-editor__hint">
                No collection env vars set. Profile env vars still apply.
              </p>
            )}
            {Object.entries(draft.custom_env_vars ?? {}).map(([k, v]) => (
              <div
                key={k}
                className="crosshook-collection-launch-defaults-editor__env-row"
              >
                <input
                  type="text"
                  className="crosshook-input"
                  value={k}
                  onChange={(e) => updateEnvVarKey(k, e.target.value)}
                  placeholder="KEY"
                  aria-label={`env var key for ${k}`}
                />
                <input
                  type="text"
                  className="crosshook-input"
                  value={v}
                  onChange={(e) => updateEnvVarValue(k, e.target.value)}
                  placeholder="value"
                  aria-label={`env var value for ${k}`}
                />
                <button
                  type="button"
                  className="crosshook-button crosshook-button--ghost"
                  onClick={() => removeEnvVar(k)}
                  aria-label={`Remove ${k}`}
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
