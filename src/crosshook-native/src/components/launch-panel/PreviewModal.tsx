import { type KeyboardEvent, type MouseEvent, useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { LaunchPreview } from '../../types';
import { copyToClipboard } from '../../utils/clipboard';
import { sortIssuesBySeverity } from '../../utils/mapValidationToNode';
import { InfoCircleIcon } from '../icons/SidebarIcons';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { focusElement, getFocusableElements } from './focusTrap';
import { buildSummaryParts, groupEnvBySource, isStale, methodLabel, severityIcon } from './helpers';

const UMU_DATABASE_MISSING_HINT =
  'umu has no known entry for this app id in the current umu database. The database only tracks titles needing protonfixes — most titles work fine without an entry.';

interface PreviewModalProps {
  preview: LaunchPreview;
  profileId: string;
  onClose: () => void;
  onLaunch: () => void;
}

export function PreviewModal({ preview, profileId, onClose, onLaunch }: PreviewModalProps) {
  const portalHostRef = useRef<HTMLElement | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const bodyStyleRef = useRef('');
  const hiddenNodesRef = useRef<Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>>([]);
  const titleId = useId();
  const [isMounted, setIsMounted] = useState(false);
  const [copyLabel, setCopyLabel] = useState('Copy Preview');

  useEffect(() => {
    const host = document.createElement('div');
    host.className = 'crosshook-modal-portal';
    portalHostRef.current = host;
    document.body.appendChild(host);
    setIsMounted(true);

    return () => {
      host.remove();
      portalHostRef.current = null;
      setIsMounted(false);
    };
  }, []);

  useEffect(() => {
    if (!isMounted) return;

    const { body } = document;
    const portalHost = portalHostRef.current;
    if (!portalHost) return;

    previouslyFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    bodyStyleRef.current = body.style.overflow;
    body.style.overflow = 'hidden';
    body.classList.add('crosshook-modal-open');

    hiddenNodesRef.current = Array.from(body.children)
      .filter((child): child is HTMLElement => child instanceof HTMLElement && child !== portalHost)
      .map((element) => {
        const inertState = (element as HTMLElement & { inert?: boolean }).inert ?? false;
        const ariaHidden = element.getAttribute('aria-hidden');
        (element as HTMLElement & { inert?: boolean }).inert = true;
        element.setAttribute('aria-hidden', 'true');
        return { element, inert: inertState, ariaHidden };
      });

    const frame = window.requestAnimationFrame(() => {
      if (focusElement(headingRef.current)) return;
      const focusable = surfaceRef.current ? getFocusableElements(surfaceRef.current) : [];
      if (focusable.length > 0) focusElement(focusable[0]);
    });

    return () => {
      window.cancelAnimationFrame(frame);
      body.style.overflow = bodyStyleRef.current;
      body.classList.remove('crosshook-modal-open');

      for (const { element, inert, ariaHidden } of hiddenNodesRef.current) {
        (element as HTMLElement & { inert?: boolean }).inert = inert;
        if (ariaHidden === null) {
          element.removeAttribute('aria-hidden');
        } else {
          element.setAttribute('aria-hidden', ariaHidden);
        }
      }
      hiddenNodesRef.current = [];

      const restoreTarget = previouslyFocusedRef.current;
      if (restoreTarget?.isConnected) {
        focusElement(restoreTarget);
      }
      previouslyFocusedRef.current = null;
    };
  }, [isMounted]);

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      event.preventDefault();
      onClose();
      return;
    }

    if (event.key !== 'Tab') return;

    const container = surfaceRef.current;
    if (!container) return;

    const focusable = getFocusableElements(container);
    if (focusable.length === 0) {
      event.preventDefault();
      return;
    }

    const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
    const lastIndex = focusable.length - 1;

    if (event.shiftKey) {
      if (currentIndex <= 0) {
        event.preventDefault();
        focusElement(focusable[lastIndex]);
      }
      return;
    }

    if (currentIndex === -1 || currentIndex === lastIndex) {
      event.preventDefault();
      focusElement(focusable[0]);
    }
  }

  function handleBackdropMouseDown(event: MouseEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget) return;
    onClose();
  }

  async function handleCopy() {
    try {
      await copyToClipboard(preview.display_text);
      setCopyLabel('Copied');
      window.setTimeout(() => setCopyLabel('Copy Preview'), 2000);
    } catch {
      setCopyLabel('Copy failed');
      window.setTimeout(() => setCopyLabel('Copy Preview'), 2000);
    }
  }

  if (!isMounted || !portalHostRef.current) return null;

  const sortedIssues = sortIssuesBySeverity(preview.validation.issues);
  const summaryParts = buildSummaryParts(preview);
  const stale = isStale(preview.generated_at);
  const isNative = preview.resolved_method === 'native';
  const isSteam = preview.resolved_method === 'steam_applaunch';
  const envCount = preview.environment?.length ?? 0;
  const previewIsReady = preview.validation.issues.length === 0;
  const generatedTime = new Date(preview.generated_at);
  const generatedTimeLabel = Number.isFinite(generatedTime.getTime())
    ? generatedTime.toLocaleTimeString()
    : 'time unavailable';

  return createPortal(
    <div className="crosshook-modal" role="presentation">
      <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
      <div
        ref={surfaceRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
        {/* Header */}
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <div className="crosshook-heading-eyebrow">Launch preview</div>
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              {profileId}
            </h2>
          </div>
          <div className="crosshook-modal__header-actions">
            <span
              className={[
                'crosshook-modal__status-chip',
                previewIsReady ? 'crosshook-modal__status-chip--success' : 'crosshook-modal__status-chip--danger',
              ].join(' ')}
            >
              {previewIsReady ? 'Ready to launch' : 'Cannot launch'}
            </span>
          </div>
        </header>

        {/* Summary Banner */}
        <div className="crosshook-preview-modal__summary-banner">
          <div className="crosshook-preview-modal__summary-fields">
            <div className="crosshook-preview-modal__summary-field">
              <div className="crosshook-preview-modal__summary-label">Method</div>
              <div className="crosshook-preview-modal__summary-value">{methodLabel(preview.resolved_method)}</div>
            </div>
            <div className="crosshook-preview-modal__summary-field">
              <div className="crosshook-preview-modal__summary-label">Game Executable</div>
              <div className="crosshook-preview-modal__summary-value crosshook-preview-modal__summary-value--mono">
                {preview.game_executable_name || preview.game_executable}
              </div>
            </div>
            <div className="crosshook-preview-modal__summary-field">
              <div className="crosshook-preview-modal__summary-label">Working Directory</div>
              <div className="crosshook-preview-modal__summary-value crosshook-preview-modal__summary-value--mono">
                {preview.working_directory || 'Not set'}
              </div>
            </div>
          </div>
          <p className="crosshook-preview-modal__plan-line">
            Preview:{' '}
            {summaryParts.reduce<import('react').ReactNode[]>((acc, part, i) => {
              if (i > 0) acc.push(', ');
              acc.push(part);
              return acc;
            }, [])}
          </p>
        </div>

        {/* Body with collapsible sections */}
        <div className="crosshook-modal__body">
          <div className="crosshook-preview-modal__sections">
            {/* Validation Results */}
            <CollapsibleSection
              title="Validation Results"
              defaultOpen
              meta={
                <span style={{ fontSize: '0.82rem' }}>
                  {preview.validation.issues.length} {preview.validation.issues.length === 1 ? 'check' : 'checks'}
                </span>
              }
            >
              {sortedIssues.length > 0 ? (
                <ul className="crosshook-preview-modal__validation-list">
                  {sortedIssues.map((issue) => (
                    <li
                      key={`${issue.severity}-${issue.code ?? 'nocode'}-${issue.message}-${issue.help}-${issue.trainer_hash_stored ?? ''}-${issue.trainer_hash_current ?? ''}-${issue.trainer_sha256_community ?? ''}`}
                      className="crosshook-preview-modal__validation-item"
                      data-severity={issue.severity}
                    >
                      <span
                        className="crosshook-preview-modal__validation-icon"
                        data-severity={issue.severity}
                        aria-hidden="true"
                      >
                        {severityIcon(issue.severity)}
                      </span>
                      <div className="crosshook-preview-modal__validation-content">
                        <div className="crosshook-preview-modal__validation-message">{issue.message}</div>
                        {issue.help ? (
                          <div className="crosshook-preview-modal__validation-help">{issue.help}</div>
                        ) : null}
                      </div>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="crosshook-preview-modal__empty">All validation checks passed.</p>
              )}
            </CollapsibleSection>

            {/* Command Chain */}
            <CollapsibleSection title="Command Chain" defaultOpen>
              {preview.effective_command ? (
                <pre className="crosshook-preview-modal__command-block">{preview.effective_command}</pre>
              ) : (
                <p className="crosshook-preview-modal__empty">No command resolved.</p>
              )}
              {preview.umu_decision
                ? (() => {
                    const umuChipModifier =
                      preview.umu_decision.will_use_umu && preview.umu_decision.csv_coverage === 'missing'
                        ? 'crosshook-preview-modal__umu-decision--info'
                        : preview.umu_decision.will_use_umu
                          ? 'crosshook-preview-modal__umu-decision--umu'
                          : 'crosshook-preview-modal__umu-decision--proton';
                    return (
                      <div className={['crosshook-preview-modal__umu-decision', umuChipModifier].join(' ')}>
                        <div>
                          <strong>umu decision:</strong>{' '}
                          {preview.umu_decision.will_use_umu ? 'using umu-run' : 'using direct Proton'}
                        </div>
                        <div className="crosshook-muted">
                          requested preference: <code>{preview.umu_decision.requested_preference}</code>
                          {' · '}
                          umu-run on PATH:{' '}
                          <code>{preview.umu_decision.umu_run_path_on_backend_path ?? 'not found'}</code>
                        </div>
                        <div className="crosshook-muted crosshook-preview-modal__umu-decision-reason">
                          {preview.umu_decision.reason}
                        </div>
                        <div className="crosshook-muted" style={{ marginTop: 4 }}>
                          umu protonfix coverage: <code>{preview.umu_decision.csv_coverage}</code>
                        </div>
                        {preview.umu_decision.will_use_umu && preview.umu_decision.csv_coverage === 'missing' ? (
                          <div className="crosshook-preview-modal__umu-decision-info">
                            <InfoCircleIcon
                              className="crosshook-preview-modal__umu-decision-info-icon"
                              width={14}
                              height={14}
                              aria-hidden
                            />
                            <span>{UMU_DATABASE_MISSING_HINT}</span>
                          </div>
                        ) : null}
                      </div>
                    );
                  })()
                : null}
              {isSteam && preview.steam_launch_options ? (
                <>
                  <div className="crosshook-preview-modal__sub-label">Steam Launch Options</div>
                  <pre className="crosshook-preview-modal__command-block">{preview.steam_launch_options}</pre>
                </>
              ) : null}
              {preview.directives_error ? (
                <div className="crosshook-preview-modal__directives-error">{preview.directives_error}</div>
              ) : null}
            </CollapsibleSection>

            {/* Environment Variables */}
            <CollapsibleSection
              title="Environment Variables"
              defaultOpen={false}
              meta={<span style={{ fontSize: '0.82rem' }}>{envCount} vars</span>}
            >
              {preview.environment && preview.environment.length > 0 ? (
                groupEnvBySource(preview.environment).map(([group, vars]) => (
                  <div key={group} className="crosshook-preview-modal__env-group">
                    <div className="crosshook-preview-modal__env-group-title">{group}</div>
                    <table className="crosshook-preview-modal__env-table">
                      <tbody>
                        {vars.map((v) => (
                          <tr key={v.key}>
                            <td>{v.key}</td>
                            <td>{v.value}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                ))
              ) : (
                <p className="crosshook-preview-modal__empty">No environment variables resolved.</p>
              )}
              {preview.cleared_variables.length > 0 ? (
                <div className="crosshook-preview-modal__cleared-vars">
                  <div className="crosshook-preview-modal__sub-label">Cleared Variables</div>
                  <ul className="crosshook-preview-modal__cleared-list">
                    {preview.cleared_variables.map((v) => (
                      <li key={v} className="crosshook-preview-modal__cleared-item">
                        {v}
                      </li>
                    ))}
                  </ul>
                </div>
              ) : null}
            </CollapsibleSection>

            {/* Proton / Runtime Setup — hidden for native method (BR-8) */}
            {!isNative ? (
              <CollapsibleSection title="Proton / Runtime Setup" defaultOpen={false}>
                {preview.proton_setup ? (
                  <div className="crosshook-preview-modal__field-list">
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Wine Prefix</div>
                      <div className="crosshook-preview-modal__field-value">
                        {preview.proton_setup.wine_prefix_path}
                      </div>
                    </div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Compat Data</div>
                      <div className="crosshook-preview-modal__field-value">
                        {preview.proton_setup.compat_data_path}
                      </div>
                    </div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Steam Client Install</div>
                      <div className="crosshook-preview-modal__field-value">
                        {preview.proton_setup.steam_client_install_path}
                      </div>
                    </div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Proton Executable</div>
                      <div className="crosshook-preview-modal__field-value">
                        {preview.proton_setup.proton_executable}
                      </div>
                    </div>
                  </div>
                ) : (
                  <p className="crosshook-preview-modal__empty">No Proton setup data available.</p>
                )}
                {preview.trainer ? (
                  <div className="crosshook-preview-modal__field-list" style={{ marginTop: 16 }}>
                    <div className="crosshook-preview-modal__sub-label">Trainer Info</div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Trainer Path</div>
                      <div className="crosshook-preview-modal__field-value">{preview.trainer.path}</div>
                    </div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Host Path</div>
                      <div className="crosshook-preview-modal__field-value">{preview.trainer.host_path}</div>
                    </div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Loading Mode</div>
                      <div className="crosshook-preview-modal__field-value">{preview.trainer.loading_mode}</div>
                    </div>
                    {preview.trainer.staged_path ? (
                      <div className="crosshook-preview-modal__field">
                        <div className="crosshook-preview-modal__field-label">Staged Path</div>
                        <div className="crosshook-preview-modal__field-value">{preview.trainer.staged_path}</div>
                      </div>
                    ) : null}
                  </div>
                ) : null}
              </CollapsibleSection>
            ) : null}
          </div>
        </div>

        {/* Footer */}
        <footer className="crosshook-modal__footer">
          <span
            className={['crosshook-preview-modal__timestamp', stale ? 'crosshook-preview-modal__timestamp--stale' : '']
              .filter(Boolean)
              .join(' ')}
          >
            Generated {generatedTimeLabel}
            {stale ? ' (stale)' : ''}
          </span>
          <div className="crosshook-modal__footer-actions">
            <button type="button" className="crosshook-button crosshook-button--ghost" onClick={handleCopy}>
              {copyLabel}
            </button>
            <button type="button" className="crosshook-button" onClick={onLaunch} disabled={!previewIsReady}>
              Launch Now
            </button>
            <button type="button" className="crosshook-button crosshook-button--ghost" onClick={onClose}>
              Close
            </button>
          </div>
        </footer>
      </div>
    </div>,
    portalHostRef.current
  );
}

export default PreviewModal;
