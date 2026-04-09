import { useCallback, useState } from 'react';

import { BROWSER_DEV_IMPORT_PRESET_PATH } from '@/constants/browserDevPresetPaths';
import { useCollections } from '@/hooks/useCollections';
import { chooseFile } from '@/utils/dialog';
import type { CollectionImportPreview } from '@/types/collections';

import { BrowserDevPresetExplainerModal } from './BrowserDevPresetExplainerModal';
import { CollectionEditModal } from './CollectionEditModal';
import { CollectionImportReviewModal } from './CollectionImportReviewModal';

export interface CollectionsSidebarProps {
  onOpenCollection: (id: string) => void;
}

export function CollectionsSidebar({ onOpenCollection }: CollectionsSidebarProps) {
  const { collections, createCollection, error, prepareCollectionImportPreview, applyImportedCollection } =
    useCollections();
  const [createOpen, setCreateOpen] = useState(false);
  const [createSessionError, setCreateSessionError] = useState<string | null>(null);
  const [importPreview, setImportPreview] = useState<CollectionImportPreview | null>(null);
  const [importReviewOpen, setImportReviewOpen] = useState(false);
  const [importExplainerOpen, setImportExplainerOpen] = useState(false);
  const [importApplying, setImportApplying] = useState(false);
  const [importSessionError, setImportSessionError] = useState<string | null>(null);

  const handleCreate = useCallback(
    async (name: string, description: string | null): Promise<boolean> => {
      setCreateSessionError(null);
      const result = await createCollection(name, description);
      if (!result.ok) {
        setCreateSessionError(result.error);
        return false;
      }
      if (result.descriptionFailed) {
        setCreateSessionError(`Collection created, but description could not be saved: ${result.descriptionFailed}`);
        return false;
      }
      return true;
    },
    [createCollection]
  );

  const handleClickCollection = useCallback(
    (id: string) => {
      onOpenCollection(id);
    },
    [onOpenCollection]
  );

  const handleImportPreset = useCallback(async () => {
    setImportSessionError(null);
    if (__WEB_DEV_MODE__) {
      setImportExplainerOpen(true);
      return;
    }
    const path = await chooseFile('Import collection preset', [
      { name: 'CrossHook collection preset', extensions: ['toml'] },
    ]);
    if (path === null) {
      return;
    }
    try {
      const preview = await prepareCollectionImportPreview(path);
      setImportPreview(preview);
      setImportReviewOpen(true);
    } catch (err) {
      setImportSessionError(err instanceof Error ? err.message : String(err));
    }
  }, [prepareCollectionImportPreview]);

  const handleImportExplainerContinue = useCallback(async () => {
    setImportExplainerOpen(false);
    try {
      const preview = await prepareCollectionImportPreview(BROWSER_DEV_IMPORT_PRESET_PATH);
      setImportPreview(preview);
      setImportReviewOpen(true);
    } catch (err) {
      setImportSessionError(err instanceof Error ? err.message : String(err));
    }
  }, [prepareCollectionImportPreview]);

  const handleImportConfirm = useCallback(
    async (input: { name: string; description: string | null; ambiguousResolutions: (string | null)[] }) => {
      if (importPreview === null) {
        return;
      }
      setImportApplying(true);
      setImportSessionError(null);
      try {
        const result = await applyImportedCollection({
          preview: importPreview,
          name: input.name,
          description: input.description,
          ambiguousResolutions: input.ambiguousResolutions,
        });
        if (!result.ok) {
          setImportSessionError(result.error);
          return;
        }
        setImportReviewOpen(false);
        setImportPreview(null);
      } finally {
        setImportApplying(false);
      }
    },
    [applyImportedCollection, importPreview]
  );

  return (
    <>
      <nav className="crosshook-sidebar__section crosshook-collections-sidebar" aria-label="Collections">
        <h2 className="crosshook-sidebar__section-label">Collections</h2>
        {collections.length > 0 ? (
          <ul className="crosshook-sidebar__section-items crosshook-collections-sidebar__list">
            {collections.map((c) => (
              <li key={c.collection_id}>
                <button
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
              </li>
            ))}
          </ul>
        ) : (
          <p className="crosshook-collections-sidebar__empty-copy">
            No collections yet. Create one or import a preset to group your profiles.
          </p>
        )}

        <button
          type="button"
          className="crosshook-sidebar__item crosshook-collections-sidebar__cta"
          onClick={() => {
            setCreateSessionError(null);
            setCreateOpen(true);
          }}
        >
          <span className="crosshook-sidebar__item-icon" aria-hidden="true">
            +
          </span>
          <span className="crosshook-sidebar__item-label">New Collection</span>
        </button>

        <button
          type="button"
          className="crosshook-sidebar__item crosshook-collections-sidebar__cta"
          onClick={() => void handleImportPreset()}
        >
          <span className="crosshook-sidebar__item-icon" aria-hidden="true">
            &gt;
          </span>
          <span className="crosshook-sidebar__item-label">Import Preset</span>
        </button>

        {(createSessionError ?? importSessionError ?? error) !== null && (
          <p className="crosshook-collections-sidebar__error" role="alert">
            {createSessionError ?? importSessionError ?? error}
          </p>
        )}
      </nav>

      <CollectionEditModal
        open={createOpen}
        mode="create"
        onClose={() => {
          setCreateSessionError(null);
          setCreateOpen(false);
        }}
        onSubmitCreate={handleCreate}
        // mode="create" — onSubmitEdit is never called here
        onSubmitEdit={async () => false}
        externalError={createSessionError}
      />

      <CollectionImportReviewModal
        open={importReviewOpen}
        preview={importPreview}
        applying={importApplying}
        importSessionError={importSessionError}
        onClose={() => {
          if (!importApplying) {
            setImportReviewOpen(false);
            setImportPreview(null);
          }
        }}
        onConfirm={(input) => void handleImportConfirm(input)}
      />

      {__WEB_DEV_MODE__ && (
        <BrowserDevPresetExplainerModal
          mode="import"
          open={importExplainerOpen}
          onClose={() => setImportExplainerOpen(false)}
          onContinue={() => void handleImportExplainerContinue()}
        />
      )}
    </>
  );
}
