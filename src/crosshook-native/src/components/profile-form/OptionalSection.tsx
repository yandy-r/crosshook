import type { ReactNode } from 'react';

export function OptionalSection(props: { summary: string; children: ReactNode; collapsed: boolean }) {
  if (!props.collapsed) {
    return <>{props.children}</>;
  }

  return (
    <details className="crosshook-optional-section">
      <summary className="crosshook-optional-section__summary">{props.summary}</summary>
      <div style={{ marginTop: 4 }}>{props.children}</div>
    </details>
  );
}
