import type { ReactNode } from 'react';

export interface PanelRouteDecorProps {
  illustration: ReactNode;
}

/** Non-interactive illustration layer for a primary panel or card (absolute, no layout space). */
export function PanelRouteDecor({ illustration }: PanelRouteDecorProps) {
  return (
    <div className="crosshook-panel-route-decor" aria-hidden="true">
      <div className="crosshook-panel-route-decor__glow" />
      <div className="crosshook-panel-route-decor__art">{illustration}</div>
    </div>
  );
}
