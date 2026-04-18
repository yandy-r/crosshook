import { toDisplayList, truncatePath } from './format';

interface RecentFilesSectionProps {
  label: string;
  paths: string[];
  limit?: number;
}

/** Displays a labelled list of recently used file paths. */
export function RecentFilesSection({ label, paths, limit }: RecentFilesSectionProps) {
  const visiblePaths = toDisplayList(paths, limit);
  const countSuffix =
    typeof limit === 'number' && limit > 0 && paths.length > limit
      ? ` showing ${limit} of ${paths.length}`
      : ` (${paths.length})`;

  return (
    <section className="crosshook-panel crosshook-settings-section">
      <div className="crosshook-settings-section-header">
        <div className="crosshook-heading-eyebrow">{label}</div>
        <div className="crosshook-muted crosshook-settings-meta">
          {paths.length === 0 ? 'No entries yet' : `Recent paths${countSuffix}`}
        </div>
      </div>

      {visiblePaths.length === 0 ? (
        <p className="crosshook-muted crosshook-settings-help">
          CrossHook will remember recently used {label.toLowerCase()} here once they are saved or loaded.
        </p>
      ) : (
        <ul className="crosshook-recent-list">
          {visiblePaths.map((path) => (
            <li key={path} className="crosshook-recent-item" title={path}>
              <div className="crosshook-recent-item__label">{truncatePath(path)}</div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
