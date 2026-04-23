import * as Tabs from '@radix-ui/react-tabs';
import * as Tooltip from '@radix-ui/react-tooltip';
import { type RefObject, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Group, Panel, type PanelImperativeHandle, Separator } from 'react-resizable-panels';
import { CollectionEditModal } from '@/components/collections/CollectionEditModal';
import { CollectionViewModal } from '@/components/collections/CollectionViewModal';
import { useCollectionViewModalState } from '@/components/collections/useCollectionViewModalState';
import ConsoleDrawer from '@/components/layout/ConsoleDrawer';
import ContentArea from '@/components/layout/ContentArea';
import ControllerPrompts from '@/components/layout/ControllerPrompts';
import Sidebar, { type AppRoute } from '@/components/layout/Sidebar';
import { OnboardingWizard } from '@/components/OnboardingWizard';
import { LaunchStateProvider } from '@/context/LaunchStateContext';
import { PreferencesProvider, usePreferencesContext } from '@/context/PreferencesContext';
import { useProfileContext } from '@/context/ProfileContext';
import { useHighContrastTheme } from '@/hooks/useAccessibilityEnhancements';
import { useBreakpoint } from '@/hooks/useBreakpoint';
import { useCollections } from '@/hooks/useCollections';
import { useFlatpakMigrationToast } from '@/hooks/useFlatpakMigrationToast';
import { subscribeEvent } from '@/lib/events';
import { isAppRoute } from '@/lib/validAppRoutes';
import type { OnboardingCheckPayload } from '@/types/onboarding';
import { sidebarVariantFromBreakpoint, sidebarWidthForVariant } from './sidebarVariants';

function ConsoleDock({ panelRef }: { panelRef: RefObject<PanelImperativeHandle | null> }) {
  const { settings } = usePreferencesContext();
  const defaultCollapsed = settings.console_drawer_collapsed_default;

  useEffect(() => {
    if (defaultCollapsed) {
      panelRef.current?.collapse();
    } else {
      panelRef.current?.expand();
    }
  }, [defaultCollapsed, panelRef]);

  return <ConsoleDrawer panelRef={panelRef} defaultCollapsed={defaultCollapsed} />;
}

function AccessibilityThemeSync() {
  const { settings } = usePreferencesContext();
  useHighContrastTheme(settings.high_contrast);
  return null;
}

export function AppShell({ controllerMode }: { controllerMode: boolean }) {
  const { profileName, selectedProfile, selectProfile, activeCollectionId, setActiveCollectionId } =
    useProfileContext();
  const [route, setRoute] = useState<AppRoute>('library');
  const lastProfile = profileName.trim() || selectedProfile;
  const shellRef = useRef<HTMLDivElement>(null);
  const consolePanelRef = useRef<PanelImperativeHandle>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const breakpoint = useBreakpoint(shellRef);
  const sidebarVariant = sidebarVariantFromBreakpoint(breakpoint.size, breakpoint.height);
  const sidebarWidth = sidebarWidthForVariant(sidebarVariant);

  const {
    open: collectionModalOpen,
    collectionId: openCollectionId,
    openForCollection,
    close: closeCollectionModal,
  } = useCollectionViewModalState();
  const { renameCollection, updateDescription, collections } = useCollections();
  const [editingCollectionId, setEditingCollectionId] = useState<string | null>(null);
  const [editSessionError, setEditSessionError] = useState<string | null>(null);
  const [collectionDescriptionToast, setCollectionDescriptionToast] = useState<{
    collectionId: string;
    description: string | null;
  } | null>(null);
  const editingCollection = useMemo(
    () =>
      editingCollectionId === null ? null : (collections.find((c) => c.collection_id === editingCollectionId) ?? null),
    [collections, editingCollectionId]
  );

  const handleOpenCollection = useCallback(
    (id: string) => {
      setActiveCollectionId(id);
      openForCollection(id);
    },
    [openForCollection, setActiveCollectionId]
  );

  const handleLaunchFromCollection = useCallback(
    async (name: string) => {
      // The user clicked Launch on a card inside CollectionViewModal, so `name`
      // is guaranteed to be a member of `activeCollectionId`. Thread the
      // collection context so `profile_load` applies the collection's launch
      // defaults via `effective_profile_with` (Phase 3 merge layer).
      await selectProfile(name, { collectionId: activeCollectionId ?? undefined });
      setRoute('launch');
    },
    [selectProfile, activeCollectionId]
  );

  const handleEditFromCollection = useCallback(
    async (name: string) => {
      await selectProfile(name);
      setRoute('profiles');
    },
    [selectProfile]
  );

  const handleRequestEditMetadata = useCallback((id: string) => {
    setEditingCollectionId(id);
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: reset errors when edit session opens/closes
  useEffect(() => {
    setEditSessionError(null);
  }, [editingCollectionId]);

  const handleSubmitEditCollection = useCallback(
    async (name: string, description: string | null): Promise<boolean> => {
      if (editingCollectionId === null) {
        return false;
      }
      const id = editingCollectionId;
      setEditSessionError(null);
      const renamed = await renameCollection(id, name);
      if (!renamed.ok) {
        setEditSessionError(renamed.error);
        return false;
      }
      const descResult = await updateDescription(id, description);
      if (!descResult.ok) {
        setCollectionDescriptionToast({ collectionId: id, description });
      }
      return true;
    },
    [editingCollectionId, renameCollection, updateDescription]
  );

  const { importCount: flatpakImportCount, dismiss: dismissFlatpakToast } = useFlatpakMigrationToast();

  const retryCollectionDescription = useCallback(async () => {
    if (collectionDescriptionToast === null) {
      return;
    }
    const result = await updateDescription(
      collectionDescriptionToast.collectionId,
      collectionDescriptionToast.description
    );
    if (result.ok) {
      setCollectionDescriptionToast(null);
    }
  }, [collectionDescriptionToast, updateDescription]);

  useEffect(() => {
    const p = subscribeEvent<OnboardingCheckPayload>('onboarding-check', (event) => {
      if (event.payload.show && !event.payload.has_profiles) setShowOnboarding(true);
    });
    return () => {
      p.then((f) => f());
    };
  }, []);

  const handleOpenOnboardingHostToolDashboard = useCallback(() => {
    setRoute('host-tools');
    setShowOnboarding(false);
  }, []);

  return (
    <Tooltip.Provider delayDuration={200}>
      <PreferencesProvider activeProfileName={lastProfile}>
        <AccessibilityThemeSync />
        <LaunchStateProvider>
          <Tabs.Root
            orientation="vertical"
            value={route}
            onValueChange={(value) => {
              if (isAppRoute(value)) setRoute(value);
            }}
          >
            <div className="crosshook-app-layout" ref={shellRef}>
              <Group
                className="crosshook-shell-group"
                orientation="horizontal"
                resizeTargetMinimumSize={{ coarse: 36, fine: 12 }}
              >
                <Panel
                  className="crosshook-shell-panel"
                  defaultSize={sidebarWidth}
                  minSize={sidebarWidth}
                  maxSize={sidebarWidth}
                >
                  <Sidebar
                    activeRoute={route}
                    onNavigate={setRoute}
                    controllerMode={controllerMode}
                    lastProfile={lastProfile}
                    onOpenCollection={handleOpenCollection}
                    variant={sidebarVariant}
                  />
                </Panel>
                <Panel className="crosshook-shell-panel" minSize="28%">
                  <Group
                    className="crosshook-shell-group"
                    orientation="vertical"
                    resizeTargetMinimumSize={{ coarse: 36, fine: 12 }}
                  >
                    <Panel className="crosshook-shell-panel" defaultSize="80%" minSize="28%">
                      <ContentArea route={route} onNavigate={setRoute} />
                    </Panel>
                    <Separator className="crosshook-resize-handle crosshook-resize-handle--horizontal" />
                    <Panel
                      className="crosshook-shell-panel"
                      panelRef={consolePanelRef}
                      collapsible
                      collapsedSize="40px"
                      defaultSize="60%"
                      minSize="25%"
                      maxSize="75%"
                    >
                      <ConsoleDock panelRef={consolePanelRef} />
                    </Panel>
                  </Group>
                </Panel>
              </Group>
            </div>
            {controllerMode ? <ControllerPrompts /> : null}
          </Tabs.Root>
          {showOnboarding && (
            <OnboardingWizard
              open={showOnboarding}
              onComplete={() => setShowOnboarding(false)}
              onDismiss={() => setShowOnboarding(false)}
              onOpenHostToolDashboard={handleOpenOnboardingHostToolDashboard}
            />
          )}
          <CollectionViewModal
            open={collectionModalOpen}
            collectionId={openCollectionId}
            onClose={closeCollectionModal}
            onLaunch={handleLaunchFromCollection}
            onEdit={handleEditFromCollection}
            onRequestEditMetadata={handleRequestEditMetadata}
            onCollectionDeleted={(id) => {
              if (activeCollectionId === id) {
                setActiveCollectionId(null);
              }
            }}
            onOpenInProfilesPage={() => {
              // `activeCollectionId` is already set by the open-collection flow,
              // so the Profiles page filter is preserved across the navigation.
              closeCollectionModal();
              setRoute('profiles');
            }}
          />
          <CollectionEditModal
            open={editingCollection !== null}
            mode="edit"
            initialName={editingCollection?.name ?? ''}
            initialDescription={editingCollection?.description ?? null}
            onClose={() => setEditingCollectionId(null)}
            onSubmitCreate={async () => false}
            onSubmitEdit={handleSubmitEditCollection}
            externalError={editSessionError}
          />
          {collectionDescriptionToast !== null ? (
            <div className="crosshook-status-toast crosshook-rename-toast" role="status" aria-live="polite">
              <span>Name saved, but the description could not be saved.</span>
              <button
                type="button"
                className="crosshook-button crosshook-button--ghost"
                onClick={() => void retryCollectionDescription()}
              >
                Retry
              </button>
              <button
                type="button"
                className="crosshook-rename-toast-dismiss"
                aria-label="Dismiss"
                onClick={() => setCollectionDescriptionToast(null)}
              >
                ×
              </button>
            </div>
          ) : null}
          {flatpakImportCount !== null ? (
            <div className="crosshook-status-toast crosshook-toast--flatpak-migration" role="status" aria-live="polite">
              <span>
                Imported your existing CrossHook data ({flatpakImportCount} item
                {flatpakImportCount !== 1 ? 's' : ''}). Your settings and game library are ready.
              </span>
              <button
                type="button"
                className="crosshook-rename-toast-dismiss"
                aria-label="Dismiss"
                onClick={dismissFlatpakToast}
              >
                ×
              </button>
            </div>
          ) : null}
        </LaunchStateProvider>
      </PreferencesProvider>
    </Tooltip.Provider>
  );
}
