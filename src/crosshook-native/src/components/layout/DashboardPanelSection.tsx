import { type ComponentPropsWithoutRef, type ReactNode, useId } from 'react';

function joinClasses(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(' ');
}

type DashboardPanelSectionHeadingTag = 'h2' | 'h3' | 'h4';

export interface DashboardPanelSectionProps
  extends Omit<ComponentPropsWithoutRef<'section'>, 'aria-label' | 'aria-labelledby' | 'children' | 'title'> {
  eyebrow?: ReactNode;
  title: ReactNode;
  summary?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
  titleAs?: DashboardPanelSectionHeadingTag;
  titleClassName?: string;
  titleId?: string;
  headerClassName?: string;
  contentClassName?: string;
  bodyClassName?: string;
  'aria-label'?: string;
  'aria-labelledby'?: string;
}

export function DashboardPanelSection({
  eyebrow,
  title,
  summary,
  description,
  actions,
  children,
  className,
  titleAs = 'h2',
  titleClassName,
  titleId,
  headerClassName,
  contentClassName,
  bodyClassName,
  'aria-label': ariaLabel,
  'aria-labelledby': ariaLabelledBy,
  ...sectionProps
}: DashboardPanelSectionProps) {
  const generatedTitleId = useId();
  const resolvedTitleId = titleId ?? `crosshook-dashboard-panel-section-title-${generatedTitleId.replace(/:/g, '')}`;
  const resolvedSummary = summary ?? description;
  const HeadingTag = titleAs;

  return (
    <section
      {...sectionProps}
      aria-label={ariaLabel}
      aria-labelledby={ariaLabel ? ariaLabelledBy : (ariaLabelledBy ?? resolvedTitleId)}
      className={joinClasses('crosshook-panel', 'crosshook-dashboard-panel-section', className)}
    >
      <div className={joinClasses('crosshook-dashboard-panel-section__header', headerClassName)}>
        <div className="crosshook-dashboard-panel-section__heading-group">
          {eyebrow ? (
            <p className="crosshook-dashboard-panel-section__eyebrow crosshook-heading-eyebrow">{eyebrow}</p>
          ) : null}
          <div className="crosshook-dashboard-panel-section__title-group">
            <HeadingTag
              id={resolvedTitleId}
              className={joinClasses(
                'crosshook-heading-title',
                'crosshook-heading-title--card',
                'crosshook-dashboard-panel-section__title',
                titleClassName
              )}
            >
              {title}
            </HeadingTag>
            {resolvedSummary ? (
              <p className="crosshook-dashboard-panel-section__summary crosshook-heading-copy">{resolvedSummary}</p>
            ) : null}
          </div>
        </div>
        {actions ? <div className="crosshook-dashboard-panel-section__actions">{actions}</div> : null}
      </div>
      {children ? (
        <div className={joinClasses('crosshook-dashboard-panel-section__content', contentClassName, bodyClassName)}>
          {children}
        </div>
      ) : null}
    </section>
  );
}

export default DashboardPanelSection;
