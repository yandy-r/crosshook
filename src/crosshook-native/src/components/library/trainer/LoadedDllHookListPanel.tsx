import { useState } from 'react';
import type { LoadedDllHook } from '@/types/profile';
import { SettingsIcon } from '../../icons/SidebarIcons';

export interface LoadedDllHookListPanelProps {
  hooks: LoadedDllHook[];
  onUpdate: (hooks: LoadedDllHook[]) => void;
}

function newHookId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `loaded-dll-hook-${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

function containsNul(value: string): boolean {
  return value.includes('\0');
}

function hasDllExtensionHint(path: string): boolean {
  const trimmed = path.trim();
  return trimmed.length > 0 && !trimmed.toLowerCase().endsWith('.dll');
}

function isInvalidHook(hook: LoadedDllHook): boolean {
  return hook.id.trim().length === 0 || containsNul(hook.name) || containsNul(hook.path);
}

export function LoadedDllHookListPanel({ hooks, onUpdate }: LoadedDllHookListPanelProps) {
  const [openHookId, setOpenHookId] = useState<string | null>(null);

  function emit(nextHooks: LoadedDllHook[]) {
    onUpdate(nextHooks.map((hook) => ({ ...hook })));
  }

  function addHook() {
    emit([
      ...hooks,
      {
        id: newHookId(),
        name: 'Loaded DLL hook',
        path: '',
        enabled: true,
      },
    ]);
  }

  function updateHook(hookId: string, patch: Partial<LoadedDllHook>) {
    emit(hooks.map((hook) => (hook.id === hookId ? { ...hook, ...patch } : hook)));
  }

  function removeHookAtIndex(indexToRemove: number) {
    const hook = hooks[indexToRemove];
    if (hook && openHookId === hook.id) {
      setOpenHookId(null);
    }
    emit(hooks.filter((_, index) => index !== indexToRemove));
  }

  return (
    <div className="crosshook-hero-detail__hook-list" data-hook-kind="loaded-dll">
      <div className="crosshook-hero-detail__hook-list-header">
        <h4 className="crosshook-hero-detail__hook-list-title">Loaded DLL hooks</h4>
        <span className="crosshook-hero-detail__hook-count">{hooks.length}</span>
      </div>

      {hooks.length === 0 ? (
        <p className="crosshook-hero-detail__hook-empty">No loaded DLL hooks declared.</p>
      ) : (
        <ul className="crosshook-hero-detail__hook-rows">
          {hooks.map((hook, index) => {
            const invalid = isInvalidHook(hook);
            const settingsOpen = openHookId === hook.id;
            const rowKey = hook.id.trim() || `loaded-dll-invalid-${index}`;
            const showDllHint = hasDllExtensionHint(hook.path);

            return (
              <li
                key={rowKey}
                className={[
                  'crosshook-hero-detail__hook-row',
                  invalid ? 'crosshook-hero-detail__hook-row--invalid' : '',
                ]
                  .filter(Boolean)
                  .join(' ')}
              >
                {invalid ? (
                  <>
                    <div className="crosshook-hero-detail__hook-row-main">
                      <span className="crosshook-hero-detail__hook-name">Invalid DLL hook</span>
                      <span className="crosshook-hero-detail__hook-path crosshook-hero-detail__mono">
                        Remove this row and attach the DLL hook again.
                      </span>
                    </div>
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary"
                      onClick={() => removeHookAtIndex(index)}
                    >
                      Remove
                    </button>
                  </>
                ) : (
                  <>
                    <label className="crosshook-hero-detail__hook-toggle">
                      <input
                        type="checkbox"
                        checked={hook.enabled}
                        onChange={(event) => updateHook(hook.id, { enabled: event.currentTarget.checked })}
                      />
                      <span>{hook.enabled ? 'Enabled' : 'Disabled'}</span>
                    </label>
                    <div className="crosshook-hero-detail__hook-row-main">
                      <span className="crosshook-hero-detail__hook-name">{hook.name}</span>
                      <span className="crosshook-hero-detail__hook-path crosshook-hero-detail__mono">
                        {hook.path.trim() ? hook.path : 'No DLL path set'}
                      </span>
                      {showDllHint ? (
                        <span className="crosshook-hero-detail__hook-path">Expected a DLL path ending in .dll.</span>
                      ) : null}
                    </div>
                    <div className="crosshook-hero-detail__hook-settings">
                      <button
                        type="button"
                        className="crosshook-button crosshook-button--secondary crosshook-hero-detail__hook-settings-button"
                        aria-label={`Edit ${hook.name}`}
                        aria-expanded={settingsOpen}
                        onClick={() => setOpenHookId(settingsOpen ? null : hook.id)}
                      >
                        <SettingsIcon width={16} height={16} />
                      </button>
                      {settingsOpen ? (
                        <div className="crosshook-hero-detail__hook-popover">
                          <label className="crosshook-label">
                            Name
                            <input
                              type="text"
                              className="crosshook-input"
                              value={hook.name}
                              aria-invalid={containsNul(hook.name)}
                              onChange={(event) => updateHook(hook.id, { name: event.currentTarget.value })}
                            />
                          </label>
                          <label className="crosshook-label">
                            DLL path
                            <input
                              type="text"
                              className="crosshook-input"
                              value={hook.path}
                              aria-invalid={containsNul(hook.path)}
                              placeholder="/path/to/hook.dll"
                              onChange={(event) => updateHook(hook.id, { path: event.currentTarget.value })}
                            />
                          </label>
                          {showDllHint ? (
                            <p className="crosshook-hero-detail__hook-empty">DLL hook paths usually end in .dll.</p>
                          ) : null}
                          <button
                            type="button"
                            className="crosshook-button crosshook-button--secondary"
                            onClick={() => removeHookAtIndex(index)}
                          >
                            Remove
                          </button>
                        </div>
                      ) : null}
                    </div>
                  </>
                )}
              </li>
            );
          })}
        </ul>
      )}

      <button type="button" className="crosshook-button crosshook-button--secondary" onClick={addHook}>
        + Attach DLL
      </button>
    </div>
  );
}

export default LoadedDllHookListPanel;
