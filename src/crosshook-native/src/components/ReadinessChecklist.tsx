import { useCallback, useEffect, useRef, useState } from 'react';
import { open as openUrl } from '@/lib/plugin-stubs/shell';
import type { HealthIssue, HealthIssueSeverity } from '../types/health';
import type { HostToolCheckResult } from '../types/onboarding';
import { getHostToolTooltipContent } from '../utils/hostReadinessTooltips';
import { InfoTooltip } from './ui/InfoTooltip';

interface ReadinessChecklistProps {
  checks: HealthIssue[];
  isLoading: boolean;
}

function getSeverityVariant(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'info':
      return 'found';
    case 'warning':
      return 'ambiguous';
    case 'error':
      return 'not-found';
  }
}

function getSeverityIcon(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'info':
      return '✓';
    case 'warning':
      return '⚠';
    case 'error':
      return '✕';
  }
}

function getSeverityLabel(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'info':
      return 'OK';
    case 'warning':
      return 'Warning';
    case 'error':
      return 'Error';
  }
}

interface CheckCardProps {
  check: HealthIssue;
}

function CheckCard({ check }: CheckCardProps) {
  const variant = getSeverityVariant(check.severity);
  const icon = getSeverityIcon(check.severity);
  const label = getSeverityLabel(check.severity);

  return (
    <div
      className={`crosshook-auto-populate__field-card crosshook-auto-populate__field-card--${variant} crosshook-readiness-checklist__card`}
    >
      <div className="crosshook-readiness-checklist__card-header">
        <span
          className={`crosshook-readiness-checklist__icon crosshook-auto-populate__field-state--${variant}`}
          aria-hidden="true"
        >
          {icon}
        </span>
        <span className="crosshook-readiness-checklist__message">{check.message}</span>
        <span className={`crosshook-readiness-checklist__badge crosshook-auto-populate__field-state--${variant}`}>
          {label}
        </span>
      </div>
      {check.remediation ? <div className="crosshook-readiness-checklist__remediation">{check.remediation}</div> : null}
    </div>
  );
}

const HOST_TOOL_CATEGORY_ORDER = ['runtime', 'performance', 'overlay', 'compatibility', 'prefix_tools'] as const;

function formatHostToolCategoryLabel(category: string): string {
  const key = category.trim().toLowerCase();
  switch (key) {
    case 'runtime':
      return 'Runtime';
    case 'performance':
      return 'Performance';
    case 'overlay':
      return 'Overlay';
    case 'compatibility':
      return 'Compatibility';
    case 'prefix_tools':
      return 'Prefix tools';
    default:
      return category || 'Other';
  }
}

function groupHostToolsByCategory(
  toolChecks: HostToolCheckResult[]
): { label: string; tools: HostToolCheckResult[] }[] {
  const map = new Map<string, HostToolCheckResult[]>();
  for (const t of toolChecks) {
    const key = (t.category || 'other').toLowerCase();
    let bucket = map.get(key);
    if (bucket === undefined) {
      bucket = [];
      map.set(key, bucket);
    }
    bucket.push(t);
  }
  const keys = new Set(map.keys());
  const ordered: string[] = [];
  for (const k of HOST_TOOL_CATEGORY_ORDER) {
    if (keys.has(k)) ordered.push(k);
  }
  const rest = [...keys]
    .filter((k) => !HOST_TOOL_CATEGORY_ORDER.includes(k as (typeof HOST_TOOL_CATEGORY_ORDER)[number]))
    .sort((a, b) => a.localeCompare(b));
  ordered.push(...rest);
  return ordered.map((key) => ({
    label: formatHostToolCategoryLabel(key === 'other' ? 'other' : key),
    tools: map.get(key) ?? [],
  }));
}

function hasExpandableGuidance(tool: HostToolCheckResult): boolean {
  const guidance = tool.install_guidance;
  if (guidance == null) {
    return false;
  }
  if (guidance.command.trim() !== '') {
    return true;
  }
  if (guidance.alternatives.trim() !== '') {
    return true;
  }
  return (tool.docs_url ?? '').trim() !== '';
}

export interface HostToolsReadinessSectionProps {
  toolChecks: HostToolCheckResult[];
  detectedDistroFamily: string;
  /** Clears install nag for this tool in SQLite (TTL). */
  onDismissReadinessNag?: (toolId: string) => void;
}

/**
 * Host catalog tools from generalized readiness — grouped by category with
 * copy-command / docs / dismiss when Flatpak guidance is present.
 */
export function HostToolsReadinessSection({
  toolChecks,
  detectedDistroFamily,
  onDismissReadinessNag,
}: HostToolsReadinessSectionProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [copiedToolId, setCopiedToolId] = useState<string | null>(null);
  const copyResetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyResetTimerRef.current !== null) {
        clearTimeout(copyResetTimerRef.current);
      }
    };
  }, []);

  const handleCopyCommand = useCallback(async (toolId: string, command: string) => {
    try {
      await navigator.clipboard.writeText(command);
      setCopiedToolId(toolId);
      if (copyResetTimerRef.current !== null) {
        clearTimeout(copyResetTimerRef.current);
      }
      copyResetTimerRef.current = setTimeout(() => {
        copyResetTimerRef.current = null;
        setCopiedToolId(null);
      }, 2000);
    } catch {
      // Clipboard unavailable
    }
  }, []);

  if (toolChecks.length === 0) {
    return null;
  }

  const groups = groupHostToolsByCategory(toolChecks);

  return (
    <section aria-label="Host tools" style={{ marginTop: 16 }}>
      <div className="crosshook-install-section-title">Host Tools</div>
      {detectedDistroFamily ? (
        <p className="crosshook-help-text" style={{ marginBottom: 8 }}>
          Detected host: <strong>{detectedDistroFamily}</strong>
        </p>
      ) : null}
      {groups.map(({ label, tools }) => (
        <div key={label} style={{ marginBottom: 12 }}>
          <div className="crosshook-help-text" style={{ fontWeight: 600, marginBottom: 6 }}>
            {label}
          </div>
          <ul className="crosshook-onboarding-wizard__review-list">
            {tools.map((tool) => {
              const variant = tool.is_available ? 'found' : tool.is_required ? 'not-found' : 'ambiguous';
              const icon = tool.is_available ? '✓' : tool.is_required ? '✕' : '⚠';
              const badge = tool.is_available ? 'OK' : tool.is_required ? 'Missing' : 'Optional';
              const guidance = tool.install_guidance;
              const docs = (tool.docs_url ?? '').trim();
              const hasGuidance = hasExpandableGuidance(tool);
              const hasCommand = guidance != null && guidance.command.trim() !== '';
              const command = guidance?.command ?? '';
              const distroFamily = guidance?.distro_family ?? '';
              const alternatives = guidance?.alternatives.trim() ?? '';
              const expanded = expandedId === tool.tool_id;

              return (
                <li
                  key={tool.tool_id}
                  className="crosshook-onboarding-wizard__review-row"
                  style={{ flexDirection: 'column', alignItems: 'stretch' }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8, width: '100%' }}>
                    <span
                      aria-hidden="true"
                      className={`crosshook-auto-populate__field-state--${variant}`}
                      style={{ minWidth: '1.25rem' }}
                    >
                      {icon}
                    </span>
                    <span
                      className="crosshook-onboarding-wizard__review-label"
                      style={{ flex: 1, display: 'inline-flex', alignItems: 'center', gap: 6 }}
                    >
                      {tool.display_name}
                      <InfoTooltip content={getHostToolTooltipContent(tool.tool_id)} size={14} />
                    </span>
                    <span
                      className={`crosshook-readiness-checklist__badge crosshook-auto-populate__field-state--${variant}`}
                    >
                      {badge}
                    </span>
                    {hasGuidance ? (
                      <button
                        type="button"
                        className="crosshook-button crosshook-button--ghost crosshook-button--sm"
                        aria-expanded={expanded}
                        onClick={() => setExpandedId((id) => (id === tool.tool_id ? null : tool.tool_id))}
                      >
                        {expanded ? 'Hide' : 'Install help'}
                      </button>
                    ) : null}
                  </div>
                  {hasGuidance && expanded ? (
                    <div
                      className="crosshook-onboarding-wizard__umu-guidance"
                      style={{ marginTop: 8, marginLeft: '1.75rem' }}
                    >
                      {hasCommand ? (
                        <p className="crosshook-help-text" style={{ marginBottom: 6 }}>
                          <span className="crosshook-help-text" style={{ fontWeight: 600 }}>
                            {distroFamily}:{' '}
                          </span>
                          <code style={{ wordBreak: 'break-all' }}>{command}</code>
                        </p>
                      ) : null}
                      {alternatives ? (
                        <p className="crosshook-help-text" style={{ marginBottom: 8 }}>
                          {alternatives}
                        </p>
                      ) : null}
                      <div className="crosshook-onboarding-wizard__umu-guidance-actions">
                        {hasCommand ? (
                          <button
                            type="button"
                            className="crosshook-button crosshook-button--secondary crosshook-button--sm"
                            onClick={() => void handleCopyCommand(tool.tool_id, command)}
                            title={command}
                          >
                            {copiedToolId === tool.tool_id ? 'Copied!' : 'Copy command'}
                          </button>
                        ) : null}
                        {docs ? (
                          <button
                            type="button"
                            className="crosshook-button crosshook-button--secondary crosshook-button--sm"
                            onClick={() => {
                              void openUrl(docs).catch((err) => {
                                console.error('Failed to open host tool docs', err);
                              });
                            }}
                          >
                            Open docs
                          </button>
                        ) : null}
                        {onDismissReadinessNag ? (
                          <button
                            type="button"
                            className="crosshook-button crosshook-button--ghost crosshook-button--sm"
                            onClick={() => onDismissReadinessNag(tool.tool_id)}
                          >
                            Dismiss reminder
                          </button>
                        ) : null}
                      </div>
                    </div>
                  ) : null}
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </section>
  );
}

export function ReadinessChecklist({ checks, isLoading }: ReadinessChecklistProps) {
  if (isLoading) {
    return (
      <section className="crosshook-readiness-checklist" aria-label="System readiness checks" aria-busy="true">
        <div className="crosshook-readiness-checklist__loading">
          <div className="crosshook-readiness-checklist__spinner" aria-hidden="true" />
          <span>Running readiness checks...</span>
        </div>
      </section>
    );
  }

  if (checks.length === 0) {
    return (
      <section className="crosshook-readiness-checklist" aria-label="System readiness checks">
        <div className="crosshook-readiness-checklist__empty">No checks have been run yet.</div>
      </section>
    );
  }

  return (
    <section className="crosshook-readiness-checklist" aria-label="System readiness checks">
      <ul className="crosshook-readiness-checklist__list">
        {checks.map((check) => (
          <li key={`${check.field}-${check.path}-${check.message}-${check.severity}`}>
            <CheckCard check={check} />
          </li>
        ))}
      </ul>
    </section>
  );
}

export default ReadinessChecklist;
