import { useCallback, useEffect, useRef, useState } from 'react';
import { open as shellOpen } from '@/lib/plugin-stubs/shell';
import { usePreferencesContext } from '../context/PreferencesContext';
import { useExternalTrainerSearch } from '../hooks/useExternalTrainerSearch';
import { useImportCommunityProfile } from '../hooks/useImportCommunityProfile';
import { useTrainerDiscovery } from '../hooks/useTrainerDiscovery';
import type { TrainerSearchResult } from '../types/discovery';
import { ExternalResultsSection } from './ExternalResultsSection';

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
// TrainerResultCard
// ---------------------------------------------------------------------------

interface TrainerResultCardProps {
  result: TrainerSearchResult;
  onImport: (result: TrainerSearchResult) => void;
  importing: boolean;
}

function TrainerResultCard({ result, onImport, importing }: TrainerResultCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const copyTimeoutRef = useRef<number | null>(null);
  const isMountedRef = useRef(true);

  const handleOpenSource = useCallback(() => {
    void shellOpen(result.sourceUrl);
  }, [result.sourceUrl]);

  const handleToggleExpand = useCallback(() => {
    setExpanded((prev) => !prev);
  }, []);

  useEffect(
    () => () => {
      isMountedRef.current = false;
      if (copyTimeoutRef.current !== null) {
        clearTimeout(copyTimeoutRef.current);
      }
    },
    []
  );

  const handleCopySha = useCallback(() => {
    if (!result.sha256) return;
    void navigator.clipboard.writeText(result.sha256).then(() => {
      if (!isMountedRef.current) {
        return;
      }
      if (copyTimeoutRef.current !== null) {
        clearTimeout(copyTimeoutRef.current);
      }
      setCopied(true);
      copyTimeoutRef.current = window.setTimeout(() => {
        if (!isMountedRef.current) {
          return;
        }
        setCopied(false);
        copyTimeoutRef.current = null;
      }, 2000);
    });
  }, [result.sha256]);

  const sha256Display = result.sha256
    ? result.sha256.length > 16
      ? `${result.sha256.slice(0, 8)}…${result.sha256.slice(-8)}`
      : result.sha256
    : null;

  return (
    <article className="crosshook-discovery-card">
      <div className="crosshook-discovery-card__header">
        <div className="crosshook-discovery-card__title-row">
          <h3 className="crosshook-discovery-card__game-name">{result.gameName}</h3>
          <span className="crosshook-discovery-badge crosshook-discovery-badge--community">
            {result.sourceName} · Community
          </span>
        </div>
        <button
          type="button"
          className="crosshook-discovery-card__expand-toggle"
          aria-expanded={expanded}
          aria-label={expanded ? 'Collapse details' : 'Expand details'}
          onClick={handleToggleExpand}
        >
          {expanded ? '▲' : '▼'}
        </button>
      </div>

      {expanded && (
        <div className="crosshook-discovery-card__details">
          {result.trainerVersion && (
            <div className="crosshook-discovery-card__meta-line">
              <span className="crosshook-muted">Trainer version:</span> {result.trainerVersion}
            </div>
          )}
          {result.gameVersion && (
            <div className="crosshook-discovery-card__meta-line">
              <span className="crosshook-muted">Game version:</span> {result.gameVersion}
            </div>
          )}
          {result.notes && (
            <div className="crosshook-discovery-card__meta-line">
              <span className="crosshook-muted">Notes:</span> {result.notes}
            </div>
          )}
          {sha256Display && (
            <div className="crosshook-discovery-card__meta-line crosshook-discovery-card__sha-row">
              <span className="crosshook-muted">SHA-256:</span>{' '}
              <code className="crosshook-discovery-card__sha">{sha256Display}</code>
              <button
                type="button"
                className="crosshook-button crosshook-button--compact crosshook-button--secondary"
                onClick={handleCopySha}
                title="Copy full SHA-256"
              >
                {copied ? 'Copied' : 'Copy'}
              </button>
            </div>
          )}
        </div>
      )}

      <div className="crosshook-discovery-card__actions">
        <button type="button" className="crosshook-button crosshook-button--secondary" onClick={handleOpenSource}>
          Get Trainer
        </button>
        <button type="button" className="crosshook-button" onClick={() => onImport(result)} disabled={importing}>
          {importing ? 'Importing…' : 'Import Profile'}
        </button>
      </div>
    </article>
  );
}

// ---------------------------------------------------------------------------
// TrainerDiscoveryPanel
// ---------------------------------------------------------------------------

export function TrainerDiscoveryPanel({ initialQuery = '' }: TrainerDiscoveryPanelProps) {
  const { importCommunityProfile } = useImportCommunityProfile();
  const { settings, persistSettings } = usePreferencesContext();
  const [query, setQuery] = useState(initialQuery);
  const [importingId, setImportingId] = useState<number | null>(null);
  const [importError, setImportError] = useState<string | null>(null);
  const [importNotice, setImportNotice] = useState<string | null>(null);
  const [pendingConsent, setPendingConsent] = useState(false);

  const { data, loading, error } = useTrainerDiscovery(settings.discovery_enabled ? query : '');
  const externalSearch = useExternalTrainerSearch(settings.discovery_enabled ? query : '');

  const results = data?.results ?? [];
  const totalCount = data?.totalCount ?? 0;

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

  const handleClearQuery = useCallback(() => {
    setQuery('');
  }, []);

  const handleImport = useCallback(
    async (result: TrainerSearchResult) => {
      setImportingId(result.id);
      setImportError(null);
      setImportNotice(null);

      const profilePath = `${result.tapLocalPath}/${result.relativePath}/community-profile.json`;
      try {
        await importCommunityProfile(profilePath);
        setImportNotice(`Imported profile for ${result.gameName}.`);
      } catch (err) {
        setImportError(err instanceof Error ? err.message : String(err));
      } finally {
        setImportingId(null);
      }
    },
    [importCommunityProfile]
  );

  return (
    <section className="crosshook-card crosshook-discovery-panel" aria-label="Trainer Discovery">
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

      {settings.discovery_enabled && (
        <>
          <div className="crosshook-discovery-panel__search-row">
            <div className="crosshook-discovery-search crosshook-discovery-panel__search-field">
              <input
                id="discovery-search"
                className="crosshook-input"
                type="search"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search games or trainers..."
                aria-label="Search games or trainers"
              />
              {query.length > 0 && (
                <button
                  type="button"
                  className="crosshook-discovery-search__clear"
                  aria-label="Clear search"
                  onClick={handleClearQuery}
                >
                  ×
                </button>
              )}
            </div>
          </div>

          {importNotice && <p className="crosshook-success crosshook-discovery-panel__notice">{importNotice}</p>}
          {importError && <p className="crosshook-discovery-panel__error">{importError}</p>}

          <div className="crosshook-discovery-panel__results-meta" role="status" aria-live="polite" aria-atomic="true">
            {loading && <span className="crosshook-muted">Searching…</span>}
            {!loading && query.trim() && error && <span className="crosshook-discovery-panel__error">{error}</span>}
            {!loading && !error && query.trim() && totalCount > 0 && (
              <span className="crosshook-muted">{`${totalCount} result${totalCount !== 1 ? 's' : ''}`}</span>
            )}
            {!loading && !error && query.trim() && totalCount === 0 && (
              <div className="crosshook-discovery-panel__empty">
                <p className="crosshook-muted">No local trainers found for &ldquo;{query.trim()}&rdquo;</p>
                <p className="crosshook-muted crosshook-discovery-panel__empty-hint">
                  Check the online results below, or add community taps with <code>trainer-sources.json</code> manifests
                  for local discovery.
                </p>
              </div>
            )}
            {!query.trim() && !loading && (
              <span className="crosshook-muted">Enter a search query above to discover trainers.</span>
            )}
          </div>

          {!loading && !error && results.length > 0 && (
            <div className="crosshook-discovery-results" role="list">
              {results.map((result) => (
                <div key={result.id} role="listitem">
                  <TrainerResultCard
                    result={result}
                    onImport={(r) => {
                      void handleImport(r);
                    }}
                    importing={importingId === result.id}
                  />
                </div>
              ))}
            </div>
          )}

          <ExternalResultsSection
            data={externalSearch.data}
            loading={externalSearch.loading}
            error={externalSearch.error}
            onRetry={() => {
              void externalSearch.search(true);
            }}
          />
        </>
      )}
    </section>
  );
}

export default TrainerDiscoveryPanel;
