import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { CSSProperties, KeyboardEvent } from 'react';

import { useCollections } from '@/hooks/useCollections';
import { getFocusableElements } from '@/lib/focus-utils';

/**
 * CollectionAssignMenu intentionally manages its own focus trap instead of
 * adopting useFocusTrap from src/hooks/useFocusTrap.ts. Reasons:
 * - Popover context: body-lock and sibling `inert` are inappropriate
 * - ArrowUp/ArrowDown roving navigation over the checkbox list is unique to this component
 * See useFocusTrap for the shared modal focus-trap implementation.
 */

export interface CollectionAssignMenuProps {
  open: boolean;
  profileName: string | null;
  anchorPosition: { x: number; y: number } | null;
  /** Prefer this element when restoring focus after close (e.g. library card root). */
  restoreFocusTo?: HTMLElement | null;
  onClose: () => void;
  onCreateNew: () => void;
}

export function CollectionAssignMenu({
  open,
  profileName,
  anchorPosition,
  restoreFocusTo = null,
  onClose,
  onCreateNew,
}: CollectionAssignMenuProps) {
  const { collections, addProfile, removeProfile, collectionsForProfile } = useCollections();
  const [memberOf, setMemberOf] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [inlineError, setInlineError] = useState<string | null>(null);
  const [, setViewportTick] = useState(0);
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) {
      setInlineError(null);
    }
  }, [open]);

  useLayoutEffect(() => {
    if (!open) return;
    let frame = 0;
    function onResize() {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => setViewportTick((t) => t + 1));
    }
    window.addEventListener('resize', onResize);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener('resize', onResize);
    };
  }, [open]);

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

  const handleClose = useCallback(() => {
    const restoreTarget = restoreFocusTo && restoreFocusTo.isConnected ? restoreFocusTo : previouslyFocusedRef.current;
    onClose();
    if (restoreTarget && restoreTarget.isConnected) {
      restoreTarget.focus();
    }
    previouslyFocusedRef.current = null;
  }, [onClose, restoreFocusTo]);

  useEffect(() => {
    if (!open) return;
    previouslyFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const frame = requestAnimationFrame(() => {
      const popover = popoverRef.current;
      if (!popover) return;
      const focusable = getFocusableElements(popover);
      if (focusable.length > 0) {
        focusable[0].focus();
      }
    });
    return () => cancelAnimationFrame(frame);
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    function onPointerDown(e: PointerEvent) {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        handleClose();
      }
    }
    document.addEventListener('pointerdown', onPointerDown, true);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown, true);
    };
  }, [open, handleClose]);

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      event.preventDefault();
      handleClose();
      return;
    }

    if (event.key === 'Tab') {
      const popover = popoverRef.current;
      if (!popover) return;
      const focusable = getFocusableElements(popover);
      if (focusable.length === 0) {
        event.preventDefault();
        return;
      }
      const idx = focusable.indexOf(document.activeElement as HTMLElement);
      const last = focusable.length - 1;
      if (event.shiftKey) {
        if (idx <= 0) {
          event.preventDefault();
          focusable[last].focus();
        }
      } else {
        if (idx === -1 || idx === last) {
          event.preventDefault();
          focusable[0].focus();
        }
      }
      return;
    }

    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const popover = popoverRef.current;
      if (!popover) return;
      const focusable = getFocusableElements(popover);
      if (focusable.length === 0) return;
      const idx = focusable.indexOf(document.activeElement as HTMLElement);
      let next: number;
      if (event.key === 'ArrowDown') {
        next = idx < focusable.length - 1 ? idx + 1 : 0;
      } else {
        next = idx > 0 ? idx - 1 : focusable.length - 1;
      }
      focusable[next].focus();
    }
  }

  const handleToggle = useCallback(
    async (collectionId: string, currentlyMember: boolean) => {
      if (profileName === null) {
        return;
      }
      setBusy(true);
      setInlineError(null);
      const result = currentlyMember
        ? await removeProfile(collectionId, profileName)
        : await addProfile(collectionId, profileName);
      if (result.ok) {
        setMemberOf((prev) => {
          const next = new Set(prev);
          if (currentlyMember) {
            next.delete(collectionId);
          } else {
            next.add(collectionId);
          }
          return next;
        });
      } else {
        setInlineError(result.error);
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
      role="dialog"
      aria-modal="true"
      aria-label={`Add ${profileName} to collection`}
      data-crosshook-focus-root="modal"
      style={style}
      onKeyDown={handleKeyDown}
    >
      <div className="crosshook-collection-assign-menu__header">Add to collection</div>
      {inlineError !== null ? (
        <p className="crosshook-collection-assign-menu__error" role="alert">
          {inlineError}
        </p>
      ) : null}
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
          handleClose();
          onCreateNew();
        }}
      >
        + New collection…
      </button>
    </div>,
    document.body
  );
}
