import { useState } from 'react';
import type { HookStage, LaunchHook } from '@/types/profile';
import { SettingsIcon } from '../icons/SidebarIcons';

export interface HookListPanelProps {
  hooks: LaunchHook[];
  stage: HookStage;
  onUpdate: (hooks: LaunchHook[]) => void;
}

function newHookId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `hook-${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

function stageLabel(stage: HookStage): string {
  return stage === 'pre-launch' ? 'Pre-launch hooks' : 'Post-exit hooks';
}

function defaultHookName(stage: HookStage): string {
  return stage === 'pre-launch' ? 'Pre-launch hook' : 'Post-exit hook';
}

function coerceStage(hook: LaunchHook, stage: HookStage): LaunchHook {
  return { ...hook, stage };
}

function containsNul(value: string): boolean {
  return value.includes('\0');
}

function isInvalidHook(hook: LaunchHook, stage: HookStage): boolean {
  return hook.id.trim().length === 0 || hook.stage !== stage || containsNul(hook.name) || containsNul(hook.path);
}

export function HookListPanel({ hooks, stage, onUpdate }: HookListPanelProps) {
  const [openHookId, setOpenHookId] = useState<string | null>(null);
  const resolvedHooks = hooks.map((hook) => coerceStage(hook, stage));

  function emit(nextHooks: LaunchHook[]) {
    onUpdate(nextHooks.map((hook) => coerceStage(hook, stage)));
  }

  function addHook() {
    emit([
      ...resolvedHooks,
      {
        id: newHookId(),
        name: defaultHookName(stage),
        path: '',
        stage,
        enabled: true,
      },
    ]);
  }

  function updateHook(hookId: string, patch: Partial<LaunchHook>) {
    emit(resolvedHooks.map((hook) => (hook.id === hookId ? coerceStage({ ...hook, ...patch }, stage) : hook)));
  }

  function removeHookAtIndex(indexToRemove: number) {
    const hook = resolvedHooks[indexToRemove];
    if (hook && openHookId === hook.id) {
      setOpenHookId(null);
    }
    emit(resolvedHooks.filter((_, index) => index !== indexToRemove));
  }

  return (
    <div className="crosshook-hero-detail__hook-list" data-stage={stage}>
      <div className="crosshook-hero-detail__hook-list-header">
        <h4 className="crosshook-hero-detail__hook-list-title">{stageLabel(stage)}</h4>
        <span className="crosshook-hero-detail__hook-count">{resolvedHooks.length}</span>
      </div>

      {resolvedHooks.length === 0 ? (
        <p className="crosshook-hero-detail__hook-empty">
          No {stage === 'pre-launch' ? 'pre-launch' : 'post-exit'} hooks declared.
        </p>
      ) : (
        <ul className="crosshook-hero-detail__hook-rows">
          {resolvedHooks.map((hook, index) => {
            const invalid = isInvalidHook(hook, stage);
            const settingsOpen = openHookId === hook.id;
            const rowKey = hook.id.trim() || `${stage}-invalid-${index}`;

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
                      <span className="crosshook-hero-detail__hook-name">Invalid hook</span>
                      <span className="crosshook-hero-detail__hook-path crosshook-hero-detail__mono">
                        Remove this row and attach the hook again.
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
                        {hook.path.trim() ? hook.path : 'No path set'}
                      </span>
                    </div>
                    <span className="crosshook-hero-detail__hook-stage">{hook.stage}</span>
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
                              aria-invalid={hook.name.trim().length === 0 || containsNul(hook.name)}
                              onChange={(event) => updateHook(hook.id, { name: event.currentTarget.value })}
                            />
                          </label>
                          <label className="crosshook-label">
                            Path
                            <input
                              type="text"
                              className="crosshook-input"
                              value={hook.path}
                              aria-invalid={containsNul(hook.path)}
                              placeholder="/path/to/script-or-dll"
                              onChange={(event) => updateHook(hook.id, { path: event.currentTarget.value })}
                            />
                          </label>
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
        + Attach script or DLL
      </button>
    </div>
  );
}

export default HookListPanel;
