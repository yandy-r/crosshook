export interface BreadcrumbSegment {
  label: string;
  /** Absent = current page: rendered as plain text with aria-current="page". */
  onNavigate?: () => void;
}

export interface BreadcrumbProps {
  segments: BreadcrumbSegment[];
  /** Extra class on the <nav> root for slot-specific sizing (e.g. hero detail). */
  className?: string;
}

export function Breadcrumb({ segments, className }: BreadcrumbProps) {
  return (
    <nav className={className ? `crosshook-breadcrumb ${className}` : 'crosshook-breadcrumb'} aria-label="Breadcrumb">
      <ol className="crosshook-breadcrumb__list crosshook-list-reset">
        {segments.map(({ label, onNavigate }, index) => (
          <li key={label || index} className="crosshook-breadcrumb__item">
            {index > 0 && (
              <span className="crosshook-breadcrumb__separator" aria-hidden="true">
                ›
              </span>
            )}
            {onNavigate ? (
              <button type="button" className="crosshook-breadcrumb__crumb" onClick={onNavigate}>
                {label}
              </button>
            ) : (
              <span className="crosshook-breadcrumb__current" aria-current="page">
                {label}
              </span>
            )}
          </li>
        ))}
      </ol>
    </nav>
  );
}
