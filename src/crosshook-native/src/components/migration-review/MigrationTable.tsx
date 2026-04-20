import { Fragment } from 'react';
import type { MigrationSuggestion } from '../../types';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { FIELD_LABELS, getConfidenceInfo, rowKey } from './utils';

interface MigrationTableProps {
  rows: MigrationSuggestion[];
  checked: Set<string>;
  onCheckRow: (key: string, isChecked: boolean) => void;
  showWarnings?: boolean;
}

export function MigrationTable({ rows, checked, onCheckRow, showWarnings = false }: MigrationTableProps) {
  return (
    <table className="crosshook-health-dashboard-table" style={{ width: '100%', fontSize: '0.875em' }}>
      <thead>
        <tr>
          <th scope="col" style={{ width: '32px' }}></th>
          <th scope="col">Profile</th>
          <th scope="col">Field</th>
          <th scope="col">Current</th>
          <th scope="col">Suggested</th>
          <th scope="col">Confidence</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((s) => {
          const key = rowKey(s);
          const isChecked = checked.has(key);
          const badge = getConfidenceInfo(s);
          return (
            <Fragment key={key}>
              <tr>
                <td>
                  <input
                    type="checkbox"
                    checked={isChecked}
                    onChange={(e) => onCheckRow(key, e.target.checked)}
                    className="crosshook-focus-ring crosshook-nav-target crosshook-focus-target"
                    aria-label={`Select ${s.profile_name} (${FIELD_LABELS[s.field] ?? s.field}) for migration`}
                    style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                  />
                </td>
                <td>{s.profile_name}</td>
                <td className="crosshook-muted">{FIELD_LABELS[s.field] ?? s.field}</td>
                <td>
                  <span style={{ color: 'var(--crosshook-color-danger)' }} title={s.old_path}>
                    {s.old_proton_name}
                  </span>
                </td>
                <td>
                  <span style={{ color: 'var(--crosshook-color-success)' }} title={s.new_path}>
                    {s.new_proton_name}
                  </span>
                </td>
                <td>
                  <span style={{ color: badge.color, fontWeight: 600 }}>{badge.text}</span>
                </td>
              </tr>
              <tr>
                <td colSpan={6} style={{ padding: '0 0 4px 32px' }}>
                  <CollapsibleSection title="Show full path" defaultOpen={false}>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                      <code
                        style={{
                          color: 'var(--crosshook-color-danger)',
                          wordBreak: 'break-all',
                          fontSize: '0.85em',
                        }}
                      >
                        {s.old_path}
                      </code>
                      <span aria-hidden="true" style={{ color: 'var(--crosshook-color-text-muted)' }}>
                        &darr;
                      </span>
                      <code
                        style={{
                          color: 'var(--crosshook-color-success)',
                          wordBreak: 'break-all',
                          fontSize: '0.85em',
                        }}
                      >
                        {s.new_path}
                      </code>
                    </div>
                  </CollapsibleSection>
                  {showWarnings && s.crosses_major_version && (
                    <div
                      role="alert"
                      style={{
                        color: 'var(--crosshook-color-warning)',
                        fontSize: '0.8em',
                        marginTop: '4px',
                      }}
                    >
                      &#9888; Major version change — your WINE prefix may need recreation
                    </div>
                  )}
                  {showWarnings && s.confidence < 0.75 && (
                    <div
                      role="alert"
                      style={{
                        color: 'var(--crosshook-color-warning)',
                        fontSize: '0.8em',
                        marginTop: '4px',
                      }}
                    >
                      &#9888; Different Proton family — verify compatibility before applying
                    </div>
                  )}
                </td>
              </tr>
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}
