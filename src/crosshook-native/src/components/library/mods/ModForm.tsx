import { type FormEvent, useId, useState } from 'react';
import { ThemedSelectField } from '@/components/ui/ThemedSelectField';
import type { ModCategory, ProfileModInput, ProfileModRecord } from '@/types/mods';
import { isHttpUrl, MOD_CATEGORY_OPTIONS } from './mods-model';

export interface ModFormProps {
  initial?: ProfileModRecord;
  gameExecutablePath: string;
  onSubmit: (input: ProfileModInput) => Promise<void>;
  onCancel: () => void;
}

export function ModForm({ initial, gameExecutablePath, onSubmit, onCancel }: ModFormProps) {
  const nameId = useId();
  const urlId = useId();
  const enabledId = useId();

  const [name, setName] = useState(initial?.name ?? '');
  const [category, setCategory] = useState<ModCategory>(initial?.category ?? 'other');
  const [paths, setPaths] = useState<string[]>(initial?.paths.length ? [...initial.paths] : ['']);
  const [sourceUrl, setSourceUrl] = useState(initial?.source_url ?? '');
  const [enabled, setEnabled] = useState(initial?.enabled ?? true);
  const [formError, setFormError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const hasRelativePath = paths.some((path) => {
    const trimmed = path.trim();
    return trimmed.length > 0 && !trimmed.startsWith('/');
  });
  const showRelativeHelp = hasRelativePath && gameExecutablePath.trim().length === 0;

  function updatePath(index: number, value: string) {
    setPaths((current) => current.map((path, i) => (i === index ? value : path)));
  }

  function removePath(index: number) {
    setPaths((current) => (current.length > 1 ? current.filter((_, i) => i !== index) : ['']));
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const trimmedName = name.trim();
    if (!trimmedName) {
      setFormError('Mod name is required.');
      return;
    }
    const cleanedPaths = paths.map((path) => path.trim()).filter((path) => path.length > 0);
    if (cleanedPaths.some((path) => path.includes('\0'))) {
      setFormError('Paths may not contain NUL characters.');
      return;
    }
    const trimmedUrl = sourceUrl.trim();
    if (trimmedUrl.length > 0 && !isHttpUrl(trimmedUrl)) {
      setFormError('Source URL must start with http:// or https://.');
      return;
    }

    const input: ProfileModInput = {
      name: trimmedName,
      category,
      paths: cleanedPaths,
      enabled,
      source_url: trimmedUrl.length > 0 ? trimmedUrl : undefined,
      provenance: initial ? initial.provenance : 'manual',
    };

    setBusy(true);
    setFormError(null);
    try {
      await onSubmit(input);
    } catch (e) {
      setFormError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="crosshook-mods-form" onSubmit={handleSubmit}>
      <div className="crosshook-mods-form__row">
        <label className="crosshook-label" htmlFor={nameId}>
          Name
        </label>
        <input
          id={nameId}
          className="crosshook-input"
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
        />
      </div>

      <div className="crosshook-mods-form__row">
        <ThemedSelectField
          label="Category"
          value={category}
          onValueChange={(value) => setCategory(value as ModCategory)}
          options={MOD_CATEGORY_OPTIONS}
        />
      </div>

      <fieldset className="crosshook-mods-form__paths">
        <legend className="crosshook-label">Paths</legend>
        {paths.map((path, index) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: rows are positional editable slots
          <div className="crosshook-mods-form__path-row" key={index}>
            <input
              className="crosshook-input"
              type="text"
              value={path}
              aria-label={`Path ${index + 1}`}
              onChange={(e) => updatePath(index, e.target.value)}
            />
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={() => removePath(index)}
            >
              Remove
            </button>
          </div>
        ))}
        <button
          type="button"
          className="crosshook-button crosshook-button--ghost"
          onClick={() => setPaths((current) => [...current, ''])}
        >
          Add path
        </button>
        {showRelativeHelp ? (
          <p className="crosshook-mods-form__help" role="status">
            Relative paths are resolved against the game directory.
          </p>
        ) : null}
      </fieldset>

      <div className="crosshook-mods-form__row">
        <label className="crosshook-label" htmlFor={urlId}>
          Source URL (optional)
        </label>
        <input
          id={urlId}
          className="crosshook-input"
          type="text"
          value={sourceUrl}
          onChange={(e) => setSourceUrl(e.target.value)}
          placeholder="https://…"
        />
      </div>

      <div className="crosshook-mods-form__row crosshook-mods-form__row--inline">
        <input id={enabledId} type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
        <label className="crosshook-label" htmlFor={enabledId}>
          Enabled
        </label>
      </div>

      {formError ? (
        <p className="crosshook-mods-form__error" role="alert">
          {formError}
        </p>
      ) : null}

      <div className="crosshook-mods-form__actions">
        <button type="submit" className="crosshook-button" disabled={busy}>
          {initial ? 'Save mod' : 'Add mod'}
        </button>
        <button type="button" className="crosshook-button crosshook-button--ghost" onClick={onCancel} disabled={busy}>
          Cancel
        </button>
      </div>
    </form>
  );
}
