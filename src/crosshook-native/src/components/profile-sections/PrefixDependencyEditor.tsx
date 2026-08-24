import { useId, useState } from 'react';

const KNOWN_PREFIX_DEPENDENCIES = [
  'dotnet48',
  'dotnet40',
  'dotnet35',
  'vcrun2022',
  'vcrun2019',
  'd3dcompiler_47',
  'd3dx9',
  'corefonts',
  'allfonts',
  'dxvk',
  'xact',
  'xinput',
] as const;

function isValidPrefixDependency(value: string): boolean {
  return /^[a-z0-9][a-z0-9_-]{0,63}$/.test(value);
}

interface PrefixDependencyEditorProps {
  dependencies: string[];
  onChange: (dependencies: string[]) => void;
}

export function PrefixDependencyEditor({ dependencies, onChange }: PrefixDependencyEditorProps) {
  const id = useId();
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<string | null>(null);
  const optionsId = `${id.replace(/[^a-zA-Z0-9_-]/g, '')}-prefix-dependency-options`;
  const helpId = `${id}-prefix-dependency-help`;
  const errorId = `${id}-prefix-dependency-error`;

  const addDependency = () => {
    const normalized = draft.trim();
    if (!isValidPrefixDependency(normalized)) {
      setError('Use lowercase letters, numbers, underscores, or hyphens; do not start with a flag.');
      return;
    }
    if (dependencies.includes(normalized)) {
      setError(`${normalized} is already declared.`);
      return;
    }
    onChange([...dependencies, normalized]);
    setDraft('');
    setError(null);
  };

  return (
    <div className="crosshook-field crosshook-prefix-dependency-editor">
      <label className="crosshook-label" htmlFor={`${id}-prefix-dependency`}>
        Prefix dependencies
      </label>
      <div className="crosshook-prefix-dependency-editor__entry">
        <input
          id={`${id}-prefix-dependency`}
          className="crosshook-input"
          value={draft}
          list={optionsId}
          placeholder="dotnet48"
          aria-label="Add prefix dependency"
          aria-describedby={error ? `${helpId} ${errorId}` : helpId}
          aria-invalid={error !== null}
          onChange={(event) => {
            setDraft(event.target.value);
            setError(null);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              addDependency();
            }
          }}
        />
        <datalist id={optionsId}>
          {KNOWN_PREFIX_DEPENDENCIES.map((dependency) => (
            <option key={dependency} value={dependency} />
          ))}
        </datalist>
        <button type="button" className="crosshook-button crosshook-button--secondary" onClick={addDependency}>
          Add dependency
        </button>
      </div>
      {dependencies.length > 0 ? (
        <ul className="crosshook-prefix-dependency-editor__chips" aria-label="Declared prefix dependencies">
          {dependencies.map((dependency) => (
            <li key={dependency} className="crosshook-status-chip crosshook-prefix-dependency-editor__chip">
              {dependency}
              <button
                type="button"
                className="crosshook-prefix-dependency-editor__remove"
                aria-label={`Remove ${dependency} dependency`}
                onClick={() => onChange(dependencies.filter((entry) => entry !== dependency))}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      ) : null}
      {error ? (
        <p id={errorId} className="crosshook-danger" role="alert">
          {error}
        </p>
      ) : null}
      <p id={helpId} className="crosshook-help-text">
        Declare Winetricks or Protontricks verbs required by this trainer. CrossHook checks and installs them into the
        selected profile prefix before launch.
      </p>
    </div>
  );
}
