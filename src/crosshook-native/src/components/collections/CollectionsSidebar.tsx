import { useCallback, useState } from 'react';

import { useCollections } from '@/hooks/useCollections';

import { CollectionEditModal } from './CollectionEditModal';

export interface CollectionsSidebarProps {
  onOpenCollection: (id: string) => void;
}

export function CollectionsSidebar({ onOpenCollection }: CollectionsSidebarProps) {
  const { collections, createCollection, error } = useCollections();
  const [createOpen, setCreateOpen] = useState(false);

  const handleCreate = useCallback(
    async (name: string, description: string | null): Promise<boolean> => {
      const id = await createCollection(name, description);
      return id !== null;
    },
    [createCollection]
  );

  const handleClickCollection = useCallback(
    (id: string) => {
      onOpenCollection(id);
    },
    [onOpenCollection]
  );

  return (
    <>
      <div className="crosshook-sidebar__section crosshook-collections-sidebar">
        <div className="crosshook-sidebar__section-label">Collections</div>
        {collections.length > 0 ? (
          <div className="crosshook-sidebar__section-items crosshook-collections-sidebar__list" role="list">
            {collections.map((c) => (
              <button
                key={c.collection_id}
                type="button"
                className="crosshook-sidebar__item crosshook-collections-sidebar__item"
                onClick={() => handleClickCollection(c.collection_id)}
                title={c.name}
              >
                <span className="crosshook-collections-sidebar__item-name">{c.name}</span>
                <span
                  className="crosshook-collections-sidebar__item-count"
                  aria-label={`${c.profile_count} profiles`}
                >
                  {c.profile_count}
                </span>
              </button>
            ))}
          </div>
        ) : null}

        <button
          type="button"
          className="crosshook-sidebar__item crosshook-collections-sidebar__cta"
          onClick={() => setCreateOpen(true)}
        >
          <span className="crosshook-sidebar__item-icon" aria-hidden="true">
            +
          </span>
          <span className="crosshook-sidebar__item-label">New Collection</span>
        </button>

        {error !== null && (
          <p className="crosshook-collections-sidebar__error" role="alert">
            {error}
          </p>
        )}
      </div>

      <CollectionEditModal
        open={createOpen}
        mode="create"
        onClose={() => setCreateOpen(false)}
        onSubmitCreate={handleCreate}
        onSubmitEdit={async () => false}
        externalError={null}
      />
    </>
  );
}
