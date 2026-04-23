import type { ComponentType, ErrorInfo, ReactNode } from 'react';
import React from 'react';
import { type InspectorBodyProps, ROUTE_METADATA } from './routeMetadata';
import type { AppRoute } from './Sidebar';

export type InspectorProps = InspectorBodyProps & {
  route: AppRoute;
  width: number;
};

class InspectorErrorBoundary extends React.Component<{ children: ReactNode }, { hasError: boolean }> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): { hasError: boolean } {
    return { hasError: true };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error('Inspector body error', error, info.componentStack);
  }

  override render(): ReactNode {
    if (this.state.hasError) {
      return (
        <p className="crosshook-inspector__error" role="status">
          Inspector unavailable.
        </p>
      );
    }
    return this.props.children;
  }
}

export function Inspector({ route, selection, width, onLaunch, onEditProfile, onToggleFavorite }: InspectorProps) {
  const InspectorComponent = ROUTE_METADATA[route].inspectorComponent as ComponentType<InspectorBodyProps> | undefined;

  return (
    <aside
      className="crosshook-inspector"
      data-testid="inspector"
      style={{ width: `${width}px` }}
      data-crosshook-focus-zone="inspector"
      aria-label="CrossHook inspector"
    >
      <div className="crosshook-inspector__body">
        {InspectorComponent == null ? (
          <p className="crosshook-inspector__empty-route" role="status">
            No inspector content for this route
          </p>
        ) : (
          <InspectorErrorBoundary>
            <InspectorComponent
              selection={selection}
              onLaunch={onLaunch}
              onEditProfile={onEditProfile}
              onToggleFavorite={onToggleFavorite}
            />
          </InspectorErrorBoundary>
        )}
      </div>
    </aside>
  );
}
