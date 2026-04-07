import { useCallback } from 'react';
import { open as shellOpen } from '@/lib/plugin-stubs/shell';
import type { ExternalTrainerResult, ExternalTrainerSearchResponse } from '../types/discovery';

interface ExternalResultCardProps {
  result: ExternalTrainerResult;
}

function ExternalResultCard({ result }: ExternalResultCardProps) {
  const handleOpenSource = useCallback(() => {
    void shellOpen(result.sourceUrl);
  }, [result.sourceUrl]);

  return (
    <article className="crosshook-discovery-card">
      <div className="crosshook-discovery-card__header">
        <div className="crosshook-discovery-card__title-row">
          <h3 className="crosshook-discovery-card__game-name">{result.gameName}</h3>
          <span className="crosshook-discovery-badge crosshook-discovery-badge--external">
            {result.sourceName} (External)
          </span>
        </div>
      </div>

      {result.pubDate && (
        <div className="crosshook-discovery-card__details" style={{ borderTop: 'none', paddingTop: 0 }}>
          <div className="crosshook-discovery-card__meta-line">
            <span className="crosshook-muted">Published:</span> {result.pubDate}
          </div>
        </div>
      )}

      <div className="crosshook-discovery-card__actions">
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={handleOpenSource}
        >
          View on {result.sourceName}
        </button>
      </div>
    </article>
  );
}

interface ExternalResultsSectionProps {
  data: ExternalTrainerSearchResponse | null;
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}

export function ExternalResultsSection({ data, loading, error, onRetry }: ExternalResultsSectionProps) {
  if (!loading && !data && !error) {
    return null;
  }

  const cacheAgeMinutes = data?.cacheAgeSecs != null
    ? Math.round(data.cacheAgeSecs / 60)
    : null;

  return (
    <div className="crosshook-discovery-external-section">
      <div className="crosshook-discovery-external-section__header">
        <span className="crosshook-discovery-external-section__title">
          Online Sources
        </span>
        {data?.cached && cacheAgeMinutes != null && (
          <span className="crosshook-discovery-cache-indicator">
            Results from cache, {cacheAgeMinutes} min ago
          </span>
        )}
        {data?.isStale && (
          <span className="crosshook-discovery-cache-indicator">
            Results may be outdated
          </span>
        )}
      </div>

      {data?.offline && (
        <div className="crosshook-discovery-offline-banner">
          <span>Online search unavailable. Showing local results only.</span>
          <button
            type="button"
            className="crosshook-button crosshook-button--compact crosshook-button--secondary"
            onClick={onRetry}
          >
            Retry
          </button>
        </div>
      )}

      {loading && (
        <div role="status" aria-live="polite">
          <span className="crosshook-muted">Searching online sources…</span>
        </div>
      )}

      {!loading && error && (
        <p className="crosshook-discovery-panel__error">{error}</p>
      )}

      {!loading && !error && data && data.results.length > 0 && (
        <div className="crosshook-discovery-results" role="list">
          {data.results.map((result, index) => (
            <div key={`${result.source}-${result.sourceUrl}-${index}`} role="listitem">
              <ExternalResultCard result={result} />
            </div>
          ))}
        </div>
      )}

      {!loading && !error && data && !data.offline && data.results.length === 0 && (
        <span className="crosshook-muted">No external trainers found.</span>
      )}
    </div>
  );
}
