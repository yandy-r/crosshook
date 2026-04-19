import { useState } from 'react';
import { open as openShell } from '@/lib/plugin-stubs/shell';
import { CollapsibleSection } from '../ui/CollapsibleSection';

interface SteamGridDbSectionProps {
  hasApiKey: boolean;
  onApiKeyChange?: (key: string) => Promise<void>;
}

/** Collapsible section for managing the SteamGridDB API key used for cover art. */
export function SteamGridDbSection({ hasApiKey, onApiKeyChange }: SteamGridDbSectionProps) {
  const [localKey, setLocalKey] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [lastAction, setLastAction] = useState<'save' | 'clear' | null>(null);

  async function handleSave() {
    if (!onApiKeyChange) {
      return;
    }
    setIsSaving(true);
    setSaveError(null);
    setSaved(false);
    const trimmedKey = localKey.trim();
    try {
      await onApiKeyChange(trimmedKey);
      setLocalKey('');
      setSaved(true);
      setLastAction('save');
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSaving(false);
    }
  }

  async function handleClear() {
    if (!onApiKeyChange) {
      return;
    }
    setIsSaving(true);
    setSaveError(null);
    setSaved(false);
    try {
      await onApiKeyChange('');
      setLocalKey('');
      setSaved(true);
      setLastAction('clear');
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <CollapsibleSection
      title="SteamGridDB"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">Optional — higher-quality cover art</span>}
    >
      <p className="crosshook-muted crosshook-settings-help">
        Enter your SteamGridDB API key to fetch higher-quality cover art for your game profiles. When set, CrossHook
        will try SteamGridDB before falling back to Steam CDN images.
      </p>

      <div className="crosshook-settings-field-row">
        <span className="crosshook-label">Key status</span>
        {hasApiKey ? (
          <span
            className="crosshook-muted crosshook-settings-note"
            style={{ color: 'var(--crosshook-success, #4caf50)' }}
          >
            Key is set
          </span>
        ) : (
          <span className="crosshook-muted crosshook-settings-note">No key configured</span>
        )}
      </div>

      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="steamgriddb-api-key">
          {hasApiKey ? 'Replace API Key' : 'API Key'}
        </label>
        <div className="crosshook-settings-input-row">
          <input
            id="steamgriddb-api-key"
            type="password"
            className="crosshook-input"
            value={localKey}
            onChange={(event) => {
              setLocalKey(event.target.value);
              setSaved(false);
            }}
            placeholder={hasApiKey ? 'Enter new key to replace the existing one' : 'Enter your SteamGridDB API key'}
            autoComplete="new-password"
          />
          {onApiKeyChange ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              disabled={isSaving || localKey.trim().length === 0}
              onClick={() => void handleSave()}
            >
              {isSaving ? 'Saving...' : 'Save'}
            </button>
          ) : null}
        </div>
      </div>

      {hasApiKey && onApiKeyChange ? (
        <div className="crosshook-settings-clear-row">
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost"
            disabled={isSaving}
            onClick={() => void handleClear()}
          >
            Clear API key
          </button>
        </div>
      ) : null}

      {saved ? (
        <p className="crosshook-muted crosshook-settings-note" style={{ color: 'var(--crosshook-success, #4caf50)' }}>
          {lastAction === 'clear' ? 'API key cleared.' : 'API key saved.'}
        </p>
      ) : null}

      {saveError ? (
        <p className="crosshook-danger crosshook-settings-error" style={{ marginTop: 4 }}>
          {saveError}
        </p>
      ) : null}

      <p className="crosshook-muted crosshook-settings-note">
        The key is stored in <code>~/.config/crosshook/settings.toml</code>. Avoid syncing this file to public
        repositories.
      </p>

      <div className="crosshook-settings-clear-row">
        <button
          type="button"
          className="crosshook-button crosshook-button--outline"
          onClick={() => void openShell('https://www.steamgriddb.com/')}
        >
          Get API Key at steamgriddb.com ↗
        </button>
      </div>
    </CollapsibleSection>
  );
}
