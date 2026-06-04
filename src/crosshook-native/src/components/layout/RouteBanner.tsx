import type { BreadcrumbSegment } from './Breadcrumb';
import { Breadcrumb } from './Breadcrumb';
import { ROUTE_METADATA } from './routeMetadata';
import type { AppRoute } from './Sidebar';

export interface RouteBannerProps {
  route: AppRoute;
  trail?: BreadcrumbSegment[];
}

/** Shared top-of-route identity banner — mirrors sidebar brand row (text + visible icon); no nested scroll. */
export function RouteBanner({ route, trail }: RouteBannerProps) {
  const meta = ROUTE_METADATA[route];
  const titleId = `crosshook-route-banner-title-${route}`;
  const Art = meta.Art;

  return (
    <section className="crosshook-route-banner crosshook-panel" aria-labelledby={titleId}>
      <div className="crosshook-route-banner__inner">
        <div className="crosshook-route-banner__body">
          {trail && trail.length > 0 ? (
            <Breadcrumb segments={trail} className="crosshook-route-banner__eyebrow" />
          ) : (
            <p className="crosshook-route-banner__eyebrow crosshook-heading-eyebrow">{meta.sectionEyebrow}</p>
          )}
          <h1 id={titleId} className="crosshook-route-banner__title">
            {meta.bannerTitle}
          </h1>
          {meta.bannerSummary.trim().length > 0 ? (
            <p className="crosshook-route-banner__summary crosshook-heading-copy">{meta.bannerSummary}</p>
          ) : null}
        </div>
        <div className="crosshook-route-banner__icon" aria-hidden="true">
          <Art />
        </div>
      </div>
    </section>
  );
}
