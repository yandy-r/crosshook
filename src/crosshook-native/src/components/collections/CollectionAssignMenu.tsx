import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { CSSProperties } from 'react';

import { useCollections } from '@/hooks/useCollections';

export interface CollectionAssignMenuProps {
  open: boolean;
  profileName: string | null;
  anchorPosition: { x: number; y: number } | null;
  onClose: () => void;
  onCreateNew: () => void;
}

export function CollectionAssignMenu({
  open,
  profileName,
  anchorPosition,
  onClose,
  onCreateNew,
}: CollectionAssignMenuProps) {
  const { collections, addProfile, removeProfile, collectionsForProfile } = useCollections();
  const [memberOf, setMemberOf] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const popoverRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open || profileName === null) {
      return;
    }
    let active = true;
    void (async () => {
      const result = await collectionsForProfile(profileName);
      if (active) {
        setMemberOf(new Set(result.map((c) => c.collection_id)));
      }
    })();
    return () => {
      active = false;
    };
  }, [open, profileName, collectionsForProfile]);

  useEffect(() => {
    if (!open) {
      return;
    }
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    }
    function onPointerDown(e: PointerEvent) {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener('keydown', onKeyDown);
    document.addEventListener('pointerdown', onPointerDown, true);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('pointerdown', onPointerDown, true);
    };
  }, [open, onClose]);

  const handleToggle = useCallback(
    async (collectionId: string, currentlyMember: boolean) => {
      if (profileName === null) {
        return;
      }
      setBusy(true);
      const ok = currentlyMember
        ? await removeProfile(collectionId, profileName)
        : await addProfile(collectionId, profileName);
      if (ok) {
        setMemberOf((prev) => {
          const next = new Set(prev);
          if (currentlyMember) {
            next.delete(collectionId);
          } else {
            next.add(collectionId);
          }
          return next;
        });
      }
      setBusy(false);
    },
    [profileName, addProfile, removeProfile]
  );

  if (!open || anchorPosition === null || profileName === null) {
    return null;
  }

  const style: CSSProperties = {
    position: 'fixed',
    left: Math.min(anchorPosition.x, window.innerWidth - 280),
    top: Math.min(anchorPosition.y, window.innerHeight - 320),
    zIndex: 1300,
  };

  return createPortal(
    <div
      ref={popoverRef}
      className="crosshook-collection-assign-menu"
      role="menu"
      aria-label={`Add ${profileName} to collection`}
      style={style}
    >
      <div className="crosshook-collection-assign-menu__header">Add to collection</div>
      {collections.length === 0 ? (
        <p className="crosshook-collection-assign-menu__empty">No collections yet.</p>
      ) : (
        <div className="crosshook-collection-assign-menu__list" role="group">
          {collections.map((c) => {
            const isMember = memberOf.has(c.collection_id);
            return (
              <label key={c.collection_id} className="crosshook-collection-assign-menu__option">
                <input
                  type="checkbox"
                  checked={isMember}
                  disabled={busy}
                  onChange={() => void handleToggle(c.collection_id, isMember)}
                />
                <span className="crosshook-collection-assign-menu__option-name">{c.name}</span>
              </label>
            );
          })}
        </div>
      )}
      <div className="crosshook-collection-assign-menu__divider" />
      <button
        type="button"
        className="crosshook-collection-assign-menu__create"
        onClick={() => {
          onClose();
          onCreateNew();
        }}
      >
        + New collection…
      </button>
    </div>,
    document.body
  );
}
