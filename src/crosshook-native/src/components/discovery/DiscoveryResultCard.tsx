import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { open as shellOpen } from '@/lib/plugin-stubs/shell';
import type { CommunityCompatibilityRating } from '../../hooks/useCommunityProfiles';
import { LOADING_MODE_LABELS } from '../../lib/loadingModes';
import type { CatalogEntry, CatalogSource } from '../../types/discovery';
import { copyToClipboard } from '../../utils/clipboard';
import { CompatibilityBadge, ratingOrder } from '../community/CompatibilityBadge';

export function catalogEntryBand(entry: CatalogEntry): CommunityCompatibilityRating {
  const rating = entry.compatibilityRating;
  return rating && (ratingOrder as string[]).includes(rating) ? (rating as CommunityCompatibilityRating) : 'unknown';
}

export function catalogEntryLoadingModeLabel(entry: CatalogEntry): string {
  return LOADING_MODE_LABELS[entry.trainerLoadingMode ?? 'unknown'] ?? LOADING_MODE_LABELS.unknown;
}

interface SourceRowProps {
  source: CatalogSource;
  gameLabel: string;
}

type CopyStatus = 'idle' | 'copied' | 'failed';

const COPY_STATUS_LABELS: Record<CopyStatus, string> = {
  idle: 'Copy',
  copied: 'Copied',
  failed: 'Copy failed',
};

function SourceRow({ source, gameLabel }: SourceRowProps) {
  const [copyStatus, setCopyStatus] = useState<CopyStatus>('idle');
  const copyTimeoutRef = useRef<number | null>(null);
  const isMountedRef = useRef(true);

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
    const sha256 = source.sha256;
    if (!sha256) return;
    void (async () => {
      let status: CopyStatus;
      try {
        await copyToClipboard(sha256);
        status = 'copied';
      } catch {
        status = 'failed';
      }
      if (!isMountedRef.current) {
        return;
      }
      if (copyTimeoutRef.current !== null) {
        clearTimeout(copyTimeoutRef.current);
      }
      setCopyStatus(status);
      copyTimeoutRef.current = window.setTimeout(() => {
        if (!isMountedRef.current) {
          return;
        }
        setCopyStatus('idle');
        copyTimeoutRef.current = null;
      }, 2000);
    })();
  }, [source.sha256]);

  const handleOpenSource = useCallback(() => {
    void shellOpen(source.sourceUrl);
  }, [source.sourceUrl]);

  const sha256Display = source.sha256
    ? source.sha256.length > 16
      ? `${source.sha256.slice(0, 8)}…${source.sha256.slice(-8)}`
      : source.sha256
    : null;

  return (
    <div className="crosshook-discovery-card__source-row">
      <span className="crosshook-discovery-card__source-name">{source.sourceName}</span>
      {source.trainerVersion ? <span className="crosshook-muted">v{source.trainerVersion}</span> : null}
      {sha256Display ? (
        <span className="crosshook-discovery-card__sha-row">
          <span className="crosshook-muted">SHA-256:</span>{' '}
          <code className="crosshook-discovery-card__sha">{sha256Display}</code>
          <button
            type="button"
            className="crosshook-button crosshook-button--compact crosshook-button--secondary"
            onClick={handleCopySha}
            title="Copy full SHA-256"
            aria-label={`Copy SHA-256 checksum for ${gameLabel}`}
          >
            {COPY_STATUS_LABELS[copyStatus]}
          </button>
        </span>
      ) : null}
      <button
        type="button"
        className="crosshook-button crosshook-button--compact crosshook-button--secondary"
        onClick={handleOpenSource}
      >
        Get Trainer
      </button>
    </div>
  );
}

export interface DiscoveryResultCardProps {
  entry: CatalogEntry;
  onImport: (entry: CatalogEntry) => void;
  importing: boolean;
}

export function DiscoveryResultCard({ entry, onImport, importing }: DiscoveryResultCardProps) {
  const [expanded, setExpanded] = useState(false);
  const detailsId = useId();

  const handleToggleExpand = useCallback(() => {
    setExpanded((prev) => !prev);
  }, []);

  const gameLabel = entry.gameName ?? 'Untitled profile';
  const platformTags = entry.platformTags?.split(' ').filter(Boolean) ?? [];

  return (
    <article className="crosshook-discovery-card" data-roving-item>
      <div className="crosshook-discovery-card__header">
        <div className="crosshook-discovery-card__title-row">
          <h3 className="crosshook-discovery-card__game-name">{gameLabel}</h3>
          <CompatibilityBadge rating={catalogEntryBand(entry)} />
        </div>
        <button
          type="button"
          className="crosshook-discovery-card__expand-toggle"
          aria-expanded={expanded}
          aria-controls={detailsId}
          aria-label={expanded ? 'Collapse details' : 'Expand details'}
          onClick={handleToggleExpand}
        >
          {expanded ? '▲' : '▼'}
        </button>
      </div>

      {expanded && (
        <div id={detailsId} className="crosshook-discovery-card__details">
          {entry.trainerName && (
            <div className="crosshook-discovery-card__meta-line">
              <span className="crosshook-muted">Trainer:</span> {entry.trainerName}
              {entry.trainerVersion ? ` (${entry.trainerVersion})` : ''}
            </div>
          )}
          {entry.gameVersion && (
            <div className="crosshook-discovery-card__meta-line">
              <span className="crosshook-muted">Game version:</span> {entry.gameVersion}
            </div>
          )}
          {entry.protonVersion && (
            <div className="crosshook-discovery-card__meta-line">
              <span className="crosshook-muted">Proton:</span> {entry.protonVersion}
            </div>
          )}
          {entry.author && (
            <div className="crosshook-discovery-card__meta-line">
              <span className="crosshook-muted">Author:</span> {entry.author}
            </div>
          )}
          {entry.description && (
            <div className="crosshook-discovery-card__meta-line">
              <span className="crosshook-muted">Description:</span> {entry.description}
            </div>
          )}
          {platformTags.length > 0 && (
            <div className="crosshook-discovery-card__meta-line crosshook-community-browser__chip-row">
              {platformTags.map((tag) => (
                <span key={tag} className="crosshook-community-browser__platform-tag">
                  {tag}
                </span>
              ))}
            </div>
          )}
          <div className="crosshook-discovery-card__meta-line">
            <span className="crosshook-muted crosshook-discovery-card__loading-mode">
              Loading mode: {catalogEntryLoadingModeLabel(entry)}
            </span>
          </div>
        </div>
      )}

      {entry.sources.length > 0 && (
        <div className="crosshook-discovery-card__sources">
          {entry.sources.map((source) => (
            <SourceRow key={`${source.sourceName}::${source.sourceUrl}`} source={source} gameLabel={gameLabel} />
          ))}
        </div>
      )}

      {entry.manifestPath.length > 0 && (
        <div className="crosshook-discovery-card__actions">
          <button type="button" className="crosshook-button" onClick={() => onImport(entry)} disabled={importing}>
            {importing ? 'Importing…' : 'Import Profile'}
          </button>
        </div>
      )}
    </article>
  );
}

export default DiscoveryResultCard;
