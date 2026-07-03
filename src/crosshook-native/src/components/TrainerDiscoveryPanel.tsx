import { useCallback, useEffect, useRef, useState } from 'react';
import { usePreferencesContext } from '../context/PreferencesContext';
import { useCommunityCatalog } from '../hooks/useCommunityCatalog';
import { useExternalTrainerSearch } from '../hooks/useExternalTrainerSearch';
import { useImportCommunityProfile } from '../hooks/useImportCommunityProfile';
import type { CatalogEntry } from '../types/discovery';
import { catalogEntryKey, DiscoveryCatalogSection } from './discovery/DiscoveryCatalogSection';
import { ExternalResultsSection } from './ExternalResultsSection';
import { DashboardPanelSection } from './layout/DashboardPanelSection';

export interface TrainerDiscoveryPanelProps {
  initialQuery?: string;
}

// ---------------------------------------------------------------------------
// ConsentDialog
// ---------------------------------------------------------------------------

interface ConsentDialogProps {
  onAccept: () => void;
  onCancel: () => void;
}

function ConsentDialog({ onAccept, onCancel }: ConsentDialogProps) {
  return (
    <div
      className="crosshook-discovery-consent"
      role="dialog"
      aria-modal="true"
      aria-labelledby="discovery-consent-title"
    >
      <h3 id="discovery-consent-title" className="crosshook-discovery-consent__title">
        Trainer Discovery
      </h3>
      <div className="crosshook-discovery-consent__body">
        <p>
          Trainer Discovery links to external community sources. CrossHook does <strong>not</strong> host, distribute,
          or endorse any trainers or third-party content.
        </p>
        <p>
          You are solely responsible for ensuring compliance with applicable laws and the terms of service for any game
          you use a trainer with. Using trainers in online games may violate those games&apos; terms of service and
          result in account bans or other penalties.
        </p>
        <p>By enabling Trainer Discovery you acknowledge that you have read and understood the above.</p>
      </div>
      <div className="crosshook-discovery-consent__actions">
        <button type="button" className="crosshook-button crosshook-button--secondary" onClick={onCancel}>
          Cancel
        </button>
        <button type="button" className="crosshook-button" onClick={onAccept}>
          I Understand
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// TrainerDiscoveryPanel
// ---------------------------------------------------------------------------

export function TrainerDiscoveryPanel({ initialQuery = '' }: TrainerDiscoveryPanelProps) {
  const { importCommunityProfile } = useImportCommunityProfile();
  const { settings, persistSettings } = usePreferencesContext();
  const [importingKey, setImportingKey] = useState<string | null>(null);
  const [importError, setImportError] = useState<string | null>(null);
  const [importNotice, setImportNotice] = useState<string | null>(null);
  const [pendingConsent, setPendingConsent] = useState(false);

  const catalog = useCommunityCatalog({ enabled: settings.discovery_enabled });
  const externalSearch = useExternalTrainerSearch(settings.discovery_enabled ? catalog.searchText : '');

  const initialQueryAppliedRef = useRef(false);
  const { setSearchText } = catalog;
  useEffect(() => {
    if (initialQueryAppliedRef.current) {
      return;
    }
    initialQueryAppliedRef.current = true;
    if (initialQuery.trim().length > 0) {
      setSearchText(initialQuery);
    }
  }, [initialQuery, setSearchText]);

  // Show consent dialog if feature is disabled
  const showConsent = !settings.discovery_enabled && pendingConsent;

  const handleEnableClick = useCallback(() => {
    setPendingConsent(true);
  }, []);

  const handleConsentAccept = useCallback(() => {
    void persistSettings({ discovery_enabled: true }).then(() => {
      setPendingConsent(false);
    });
  }, [persistSettings]);

  const handleConsentCancel = useCallback(() => {
    setPendingConsent(false);
  }, []);

  const handleImport = useCallback(
    async (entry: CatalogEntry) => {
      setImportingKey(catalogEntryKey(entry));
      setImportError(null);
      setImportNotice(null);

      try {
        await importCommunityProfile(entry.manifestPath);
        setImportNotice(`Imported profile for ${entry.gameName ?? entry.relativePath}.`);
      } catch (err) {
        setImportError(err instanceof Error ? err.message : String(err));
      } finally {
        setImportingKey(null);
      }
    },
    [importCommunityProfile]
  );

  return (
    <div className="crosshook-discovery-panel">
      <DashboardPanelSection
        eyebrow="Community"
        title="Trainer Discovery"
        summary="Browse community trainer sources. CrossHook does not host, distribute, or endorse any trainers or third-party content."
        titleAs="h2"
      >
        {!settings.discovery_enabled && !pendingConsent && (
          <div className="crosshook-discovery-panel__gate">
            <p className="crosshook-muted">
              Trainer Discovery is disabled. Enable it to search community trainer sources.
            </p>
            <button type="button" className="crosshook-button" onClick={handleEnableClick}>
              Enable Trainer Discovery
            </button>
          </div>
        )}

        {showConsent && <ConsentDialog onAccept={handleConsentAccept} onCancel={handleConsentCancel} />}
      </DashboardPanelSection>

      {settings.discovery_enabled && (
        <>
          {importNotice && (
            <div className="crosshook-warning-banner crosshook-warning-banner--section" role="status">
              {importNotice}
            </div>
          )}
          {importError && (
            <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
              {importError}
            </div>
          )}

          <DiscoveryCatalogSection
            catalog={catalog}
            importingKey={importingKey}
            onImport={(entry) => {
              void handleImport(entry);
            }}
          />

          <DashboardPanelSection
            eyebrow="Online"
            title="External results"
            summary="Online trainer results for your query from configured external sources."
            titleAs="h2"
          >
            <ExternalResultsSection
              data={externalSearch.data}
              loading={externalSearch.loading}
              error={externalSearch.error}
              onRetry={() => {
                void externalSearch.search(true);
              }}
            />
          </DashboardPanelSection>
        </>
      )}
    </div>
  );
}

export default TrainerDiscoveryPanel;
