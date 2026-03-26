import { useCallback, useEffect, useRef, type ReactNode } from 'react';

export interface CollapsibleSectionProps {
  title: string;
  defaultOpen?: boolean;
  open?: boolean;
  onToggle?: (nextOpen: boolean) => void;
  meta?: ReactNode;
  className?: string;
  children: ReactNode;
}

export function CollapsibleSection({
  title,
  defaultOpen = true,
  open,
  onToggle,
  meta,
  className,
  children,
}: CollapsibleSectionProps) {
  const detailsRef = useRef<HTMLDetailsElement>(null);
  const isControlled = open !== undefined;

  const handleToggle = useCallback(() => {
    const element = detailsRef.current;
    if (!element) {
      return;
    }

    onToggle?.(element.open);
  }, [onToggle]);

  useEffect(() => {
    if (!isControlled) {
      return;
    }

    const element = detailsRef.current;
    if (element && element.open !== open) {
      element.open = open;
    }
  }, [isControlled, open]);

  const rootClass = ['crosshook-collapsible', className].filter(Boolean).join(' ');

  return (
    <details
      ref={detailsRef}
      className={rootClass}
      open={isControlled ? undefined : defaultOpen}
      onToggle={handleToggle}
    >
      <summary className="crosshook-collapsible__summary">
        <span className="crosshook-collapsible__chevron" aria-hidden="true" />
        <span className="crosshook-collapsible__title">{title}</span>
        {meta ? <span className="crosshook-collapsible__meta">{meta}</span> : null}
      </summary>
      <div className="crosshook-collapsible__body">{children}</div>
    </details>
  );
}

export default CollapsibleSection;
