import { formatBytes, formatReleaseDate } from '../../lib/protonup/format';

interface VersionRowProps {
  version: string;
  provider: string;
  installed: boolean;
  installing: boolean;
  canInstall: boolean;
  onInstall: () => void;
  onUninstall: () => void;
  /** ISO-8601 release date (available rows only). */
  publishedAt?: string | null;
  /** Asset size in bytes (available rows only). */
  assetSize?: number | null;
}

export function VersionRow({
  version,
  provider,
  installed,
  installing,
  canInstall,
  onInstall,
  onUninstall,
  publishedAt = null,
  assetSize = null,
}: VersionRowProps) {
  const rowClass = `crosshook-version-row${installed ? ' crosshook-version-row--installed' : ''}`;
  const dateLabel = formatReleaseDate(publishedAt);
  const sizeLabel = formatBytes(assetSize);
  const showMeta = dateLabel !== null || sizeLabel !== null;

  return (
    <div className={rowClass}>
      <div className="crosshook-version-row__left">
        <div className="crosshook-version-row__headline">
          <span className="crosshook-version-row__tag">{version}</span>
          <span className="crosshook-version-row__provider">{provider}</span>
          <span
            className={`crosshook-version-row__status-pill crosshook-version-row__status-pill--${installed ? 'installed' : 'available'}`}
          >
            {installed ? 'Installed' : 'Available'}
          </span>
        </div>
        {showMeta ? (
          <div className="crosshook-version-row__meta">
            {dateLabel ? <span>Released {dateLabel}</span> : null}
            {dateLabel && sizeLabel ? <span className="crosshook-version-row__meta-sep">·</span> : null}
            {sizeLabel ? <span>{sizeLabel}</span> : null}
          </div>
        ) : null}
      </div>

      <div className="crosshook-version-row__right">
        {installed ? (
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small"
            disabled={installing}
            onClick={onUninstall}
          >
            Uninstall
          </button>
        ) : (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            disabled={installing || !canInstall}
            onClick={onInstall}
            title={!canInstall ? 'No writable install root available' : undefined}
          >
            {installing ? 'Installing…' : 'Install'}
          </button>
        )}
      </div>
    </div>
  );
}
