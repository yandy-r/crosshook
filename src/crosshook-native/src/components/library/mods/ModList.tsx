import { Fragment, type ReactNode, useMemo } from 'react';
import { useRovingTabindex } from '@/hooks/useRovingTabindex';
import type { ProfileModRecord } from '@/types/mods';
import { ModListItem } from './ModListItem';
import { sortMods } from './mods-model';

export interface ModListProps {
  mods: ProfileModRecord[];
  editingId: string | null;
  onToggle: (mod: ProfileModRecord) => void;
  onEdit: (modId: string) => void;
  onRemove: (modId: string) => void;
  /** Inline editor rendered under the row whose `mod_id === editingId`. */
  renderEditor: (mod: ProfileModRecord) => ReactNode;
}

export function ModList({ mods, editingId, onToggle, onEdit, onRemove, renderEditor }: ModListProps) {
  const rovingRef = useRovingTabindex({ itemSelector: '[data-roving-item]' });
  const sorted = useMemo(() => sortMods(mods), [mods]);

  return (
    <ul className="crosshook-mods-list" ref={rovingRef}>
      {sorted.map((mod) => (
        <Fragment key={mod.mod_id}>
          <ModListItem
            mod={mod}
            editing={editingId === mod.mod_id}
            onToggle={onToggle}
            onEdit={onEdit}
            onRemove={onRemove}
          />
          {editingId === mod.mod_id ? <li className="crosshook-mods-list__editor">{renderEditor(mod)}</li> : null}
        </Fragment>
      ))}
    </ul>
  );
}
