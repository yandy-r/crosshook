import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { open as openUrl } from '@/lib/plugin-stubs/shell';

import type { HostToolCheckResult } from '../../types/onboarding';
import { copyToClipboard } from '../../utils/clipboard';
import { getHostToolTooltipContent } from '../../utils/hostReadinessTooltips';
import { InfoTooltip } from '../ui/InfoTooltip';

type CopyTarget = 'command' | 'path' | null;

export interface HostToolCardProps {
  tool: HostToolCheckResult;
  className?: string;
  isProbingDetails?: boolean;
  onProbeDetails?: (toolId: string) => Promise<void> | void;
  onDismissReadinessNag?: (toolId: string) => void;
}

interface AvailabilityPresentation {
  tone: 'success' | 'warning' | 'danger';
  stateClass: 'found' | 'ambiguous' | 'not-found';
  label: string;
  icon: string;
}

function formatCategoryLabel(category: string): string {
  const normalized = category.trim().toLowerCase();

  switch (normalized) {
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
      if (normalized.length === 0) {
        return 'Other';
      }
      return normalized
        .split('_')
        .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
        .join(' ');
  }
}

function getAvailabilityPresentation(tool: HostToolCheckResult): AvailabilityPresentation {
  if (tool.is_available) {
    return {
      tone: 'success',
      stateClass: 'found',
      label: 'Available',
      icon: '✓',
    };
  }

  if (tool.is_required) {
    return {
      tone: 'danger',
      stateClass: 'not-found',
      label: 'Missing',
      icon: '✕',
    };
  }

  return {
    tone: 'warning',
    stateClass: 'ambiguous',
    label: 'Optional missing',
    icon: '⚠',
  };
}

function hasGuidance(tool: HostToolCheckResult): boolean {
  const docsUrl = (tool.docs_url ?? '').trim();
  const command = tool.install_guidance?.command.trim() ?? '';
  const alternatives = tool.install_guidance?.alternatives.trim() ?? '';
  return docsUrl.length > 0 || command.length > 0 || alternatives.length > 0;
}

function hasDetailValues(tool: HostToolCheckResult): boolean {
  return (tool.tool_version ?? '').trim().length > 0 || (tool.resolved_path ?? '').trim().length > 0;
}

export function HostToolCard({
  tool,
  className,
  isProbingDetails = false,
  onProbeDetails,
  onDismissReadinessNag,
}: HostToolCardProps) {
  const [guidanceOpen, setGuidanceOpen] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [copiedTarget, setCopiedTarget] = useState<CopyTarget>(null);
  const copyResetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyResetTimerRef.current !== null) {
        clearTimeout(copyResetTimerRef.current);
      }
    };
  }, []);

  const categoryLabel = useMemo(() => formatCategoryLabel(tool.category), [tool.category]);
  const availability = useMemo(() => getAvailabilityPresentation(tool), [tool]);
  const toolVersion = (tool.tool_version ?? '').trim();
  const resolvedPath = (tool.resolved_path ?? '').trim();
  const docsUrl = (tool.docs_url ?? '').trim();
  const installCommand = tool.install_guidance?.command.trim() ?? '';
  const alternativeGuidance = tool.install_guidance?.alternatives.trim() ?? '';
  const distroFamily = tool.install_guidance?.distro_family.trim() ?? '';
  const guidanceAvailable = hasGuidance(tool);
  const detailValuesAvailable = hasDetailValues(tool);

  const handleCopy = useCallback(async (target: Exclude<CopyTarget, null>, value: string) => {
    await copyToClipboard(value);
    setCopiedTarget(target);

    if (copyResetTimerRef.current !== null) {
      clearTimeout(copyResetTimerRef.current);
    }

    copyResetTimerRef.current = setTimeout(() => {
      copyResetTimerRef.current = null;
      setCopiedTarget(null);
    }, 2000);
  }, []);

  const handleToggleDetails = useCallback(async () => {
    if (detailsOpen) {
      setDetailsOpen(false);
      return;
    }

    if (!detailValuesAvailable && onProbeDetails) {
      await onProbeDetails(tool.tool_id);
    }

    setDetailsOpen(true);
  }, [detailValuesAvailable, detailsOpen, onProbeDetails, tool.tool_id]);

  const handleOpenDocs = useCallback(() => {
    if (docsUrl.length === 0) {
      return;
    }

    void openUrl(docsUrl).catch((error) => {
      console.error(`Failed to open host tool docs for ${tool.tool_id}`, error);
    });
  }, [docsUrl, tool.tool_id]);

  const rootClassName = ['crosshook-panel', 'crosshook-host-tool-card', className].filter(Boolean).join(' ');

  return (
    <article className={rootClassName} aria-labelledby={`host-tool-card-${tool.tool_id}`}>
      <header className="crosshook-host-tool-card__header">
        <div className="crosshook-host-tool-card__header-row">
          <div className="crosshook-host-tool-card__title-group">
            <div className="crosshook-host-tool-card__title-row">
              <span
                aria-hidden="true"
                className={`crosshook-auto-populate__field-state--${availability.stateClass} crosshook-host-tool-card__state-icon`}
              >
                {availability.icon}
              </span>
              <h3 id={`host-tool-card-${tool.tool_id}`} className="crosshook-host-tool-card__title">
                {tool.display_name}
              </h3>
              <InfoTooltip content={getHostToolTooltipContent(tool.tool_id)} size={14} />
            </div>
            <p className="crosshook-help-text crosshook-host-tool-card__tool-id">
              Tool ID: <code>{tool.tool_id}</code>
            </p>
          </div>

          <div className="crosshook-host-tool-card__chips">
            <span className="crosshook-status-chip crosshook-status-chip--muted">{categoryLabel}</span>
            <span className="crosshook-status-chip crosshook-status-chip--muted">
              {tool.is_required ? 'Required' : 'Optional'}
            </span>
            <span className={`crosshook-status-chip crosshook-status-chip--${availability.tone}`}>
              {availability.label}
            </span>
          </div>
        </div>

        {(toolVersion.length > 0 || resolvedPath.length > 0) && (
          <dl className="crosshook-host-tool-card__meta-list">
            {toolVersion.length > 0 ? (
              <div className="crosshook-host-tool-card__meta-item">
                <dt className="crosshook-help-text crosshook-host-tool-card__meta-term">Version</dt>
                <dd className="crosshook-host-tool-card__meta-desc">
                  <code>{toolVersion}</code>
                </dd>
              </div>
            ) : null}

            {resolvedPath.length > 0 ? (
              <div className="crosshook-host-tool-card__meta-item">
                <dt className="crosshook-help-text crosshook-host-tool-card__meta-term">Resolved path</dt>
                <dd className="crosshook-host-tool-card__meta-desc crosshook-host-tool-card__meta-desc--break-all">
                  <code>{resolvedPath}</code>
                </dd>
              </div>
            ) : null}
          </dl>
        )}
      </header>

      <div className="crosshook-host-tool-card__actions">
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary crosshook-button--small"
          aria-expanded={guidanceOpen}
          disabled={!guidanceAvailable}
          onClick={() => setGuidanceOpen((current) => !current)}
        >
          {guidanceOpen ? 'Hide guidance' : 'Guidance'}
        </button>

        <button
          type="button"
          className="crosshook-button crosshook-button--ghost crosshook-button--small"
          aria-expanded={detailsOpen}
          aria-busy={isProbingDetails}
          onClick={() => {
            void handleToggleDetails().catch((error) => {
              console.error(`Failed to probe host tool details for ${tool.tool_id}`, error);
            });
          }}
        >
          {detailsOpen ? 'Hide details' : isProbingDetails ? 'Loading details…' : 'Details'}
        </button>
      </div>

      {guidanceOpen ? (
        <section aria-label={`${tool.display_name} guidance`} className="crosshook-host-tool-card__section">
          {installCommand.length > 0 ? (
            <div className="crosshook-host-tool-card__section-row">
              <div className="crosshook-help-text crosshook-host-tool-card__section-label">
                {distroFamily.length > 0 ? `${distroFamily} install command` : 'Install command'}
              </div>
              <code className="crosshook-host-tool-card__section-code">{installCommand}</code>
            </div>
          ) : null}

          {alternativeGuidance.length > 0 ? <p className="crosshook-help-text">{alternativeGuidance}</p> : null}

          <div className="crosshook-host-tool-card__section-actions">
            {installCommand.length > 0 ? (
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary crosshook-button--small"
                onClick={() => {
                  void handleCopy('command', installCommand).catch((error) => {
                    console.error(`Failed to copy install command for ${tool.tool_id}`, error);
                  });
                }}
                title={installCommand}
              >
                {copiedTarget === 'command' ? 'Copied!' : 'Copy command'}
              </button>
            ) : null}

            {docsUrl.length > 0 ? (
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary crosshook-button--small"
                onClick={handleOpenDocs}
              >
                Open docs
              </button>
            ) : null}

            {!tool.is_available && onDismissReadinessNag ? (
              <button
                type="button"
                className="crosshook-button crosshook-button--ghost crosshook-button--small"
                onClick={() => onDismissReadinessNag(tool.tool_id)}
              >
                Dismiss reminder
              </button>
            ) : null}
          </div>
        </section>
      ) : null}

      {detailsOpen ? (
        <section aria-label={`${tool.display_name} details`} className="crosshook-host-tool-card__section">
          {detailValuesAvailable ? (
            <>
              {toolVersion.length > 0 ? (
                <div className="crosshook-host-tool-card__section-row">
                  <div className="crosshook-help-text crosshook-host-tool-card__section-label">Detected version</div>
                  <code>{toolVersion}</code>
                </div>
              ) : null}

              {resolvedPath.length > 0 ? (
                <div className="crosshook-host-tool-card__section-path">
                  <div className="crosshook-host-tool-card__section-row">
                    <div className="crosshook-help-text crosshook-host-tool-card__section-label">Detected path</div>
                    <code className="crosshook-host-tool-card__section-code">{resolvedPath}</code>
                  </div>
                  <div>
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary crosshook-button--small"
                      onClick={() => {
                        void handleCopy('path', resolvedPath).catch((error) => {
                          console.error(`Failed to copy resolved path for ${tool.tool_id}`, error);
                        });
                      }}
                      title={resolvedPath}
                    >
                      {copiedTarget === 'path' ? 'Copied!' : 'Copy path'}
                    </button>
                  </div>
                </div>
              ) : null}
            </>
          ) : (
            <p className="crosshook-help-text">
              {isProbingDetails
                ? 'Probing the host tool for version and path details…'
                : 'No version or path details have been captured yet.'}
            </p>
          )}
        </section>
      ) : null}
    </article>
  );
}

export default HostToolCard;
