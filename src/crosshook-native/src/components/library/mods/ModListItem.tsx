import { useEffect, useMemo, useRef, useState } from 'react';
import type { ProfileModRecord } from '@/types/mods';
import { isHttpUrl, MOD_CATEGORY_LABELS, summarizeModPaths } from './mods-model';

export interface ModListItemProps {
  mod: ProfileModRecord;
  editing: boolean;
  onToggle: (mod: ProfileModRecord) => void;
  onEdit: (modId: string) => void;
  onRemove: (modId: string) => void;
}

const CONFIRM_RESET_MS = 3000;

function sourceHostname(url: string): string | null {
  try {
    return new URL(url).hostname;
  } catch {
    return null;
  }
}

export function ModListItem({ mod, editing, onToggle, onEdit, onRemove }: ModListItemProps) {
  const [confirmingRemove, setConfirmingRemove] = useState(false);
  const confirmTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (confirmTimerRef.current !== null) {
        window.clearTimeout(confirmTimerRef.current);
      }
    };
  }, []);

  function handleRemoveClick() {
    if (confirmingRemove) {
      if (confirmTimerRef.current !== null) {
        window.clearTimeout(confirmTimerRef.current);
        confirmTimerRef.current = null;
      }
      setConfirmingRemove(false);
      onRemove(mod.mod_id);
      return;
    }
    setConfirmingRemove(true);
    confirmTimerRef.current = window.setTimeout(() => {
      confirmTimerRef.current = null;
      setConfirmingRemove(false);
    }, CONFIRM_RESET_MS);
  }

  const pathsSummary = useMemo(() => summarizeModPaths(mod.paths), [mod.paths]);
  const hostname = useMemo(
    () => (mod.source_url && isHttpUrl(mod.source_url) ? sourceHostname(mod.source_url) : null),
    [mod.source_url]
  );

  return (
    <li className={`crosshook-mods-list__item${mod.enabled ? '' : ' crosshook-mods-list__item--disabled'}`}>
      <div className="crosshook-mods-list__main">
        <button
          type="button"
          role="switch"
          aria-checked={mod.enabled}
          aria-label={`Enable ${mod.name}`}
          className="crosshook-mods-list__toggle"
          data-roving-item
          onClick={() => onToggle(mod)}
        >
          <span aria-hidden="true" className="crosshook-mods-list__toggle-track" />
        </button>
        <span className="crosshook-mods-list__name" title={mod.name}>
          {mod.name}
        </span>
        <span className="crosshook-mods-list__chip">{MOD_CATEGORY_LABELS[mod.category]}</span>
        <span className="crosshook-mods-list__badge" data-provenance={mod.provenance}>
          {mod.provenance === 'detected' ? 'Detected' : 'Manual'}
        </span>
      </div>
      <div className="crosshook-mods-list__meta">
        {pathsSummary.summary ? (
          <span className="crosshook-mods-list__paths" title={pathsSummary.full}>
            {pathsSummary.summary}
          </span>
        ) : null}
        {hostname && mod.source_url ? (
          <a className="crosshook-mods-list__link" href={mod.source_url} target="_blank" rel="noreferrer">
            {hostname}
          </a>
        ) : null}
        <span className="crosshook-mods-list__actions">
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost"
            aria-expanded={editing}
            onClick={() => onEdit(mod.mod_id)}
          >
            {editing ? 'Close' : 'Edit'}
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-mods-list__remove"
            onClick={handleRemoveClick}
          >
            {confirmingRemove ? 'Confirm remove?' : 'Remove'}
          </button>
        </span>
      </div>
    </li>
  );
}
