import { createPortal } from 'react-dom';
import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from 'react';
import type {
  PatternMatch,
  EnvVarSource,
  LaunchMethod,
  LaunchPreview,
  LaunchRequest,
  LaunchValidationIssue,
  LaunchValidationSeverity,
  PreviewEnvVar,
} from '../types';
import { LaunchPhase } from '../types';
import { useLaunchState } from '../hooks/useLaunchState';
import { usePreviewState } from '../hooks/usePreviewState';
import { copyToClipboard } from '../utils/clipboard';
import { CollapsibleSection } from './ui/CollapsibleSection';
import '../styles/preview.css';

/* ───────── Focus-trap helpers (mirrors ProfileReviewModal) ───────── */

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  '[contenteditable="true"]',
].join(', ');

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) =>
      !el.hasAttribute('disabled') &&
      el.tabIndex >= 0 &&
      el.getClientRects().length > 0,
  );
}

function focusElement(element: HTMLElement | null): boolean {
  if (!element) return false;
  element.focus({ preventScroll: true });
  return document.activeElement === element;
}

/* ───────── Preview data helpers ───────── */

function severityIcon(severity: LaunchValidationSeverity): string {
  switch (severity) {
    case 'fatal':
      return '\u2717';
    case 'warning':
      return '!';
    case 'info':
    default:
      return '\u2713';
  }
}

function sortIssuesBySeverity(issues: LaunchValidationIssue[]): LaunchValidationIssue[] {
  const order: Record<LaunchValidationSeverity, number> = { fatal: 0, warning: 1, info: 2 };
  return [...issues].sort((a, b) => order[a.severity] - order[b.severity]);
}

function sortPatternMatchesBySeverity(matches: PatternMatch[]): PatternMatch[] {
  const order: Record<LaunchValidationSeverity, number> = { fatal: 0, warning: 1, info: 2 };
  return [...matches].sort((a, b) => order[a.severity] - order[b.severity]);
}

function sourceLabel(source: EnvVarSource): string {
  switch (source) {
    case 'proton_runtime':
      return 'Proton Runtime';
    case 'launch_optimization':
      return 'Launch Optimization';
    case 'host':
      return 'Host';
    case 'steam_proton':
      return 'Steam Proton';
  }
}

function groupEnvBySource(vars: PreviewEnvVar[]): [string, PreviewEnvVar[]][] {
  const groups = new Map<string, PreviewEnvVar[]>();
  for (const v of vars) {
    const label = sourceLabel(v.source);
    const list = groups.get(label);
    if (list) {
      list.push(v);
    } else {
      groups.set(label, [v]);
    }
  }
  return Array.from(groups.entries());
}

function methodLabel(method: string): string {
  switch (method) {
    case 'steam_applaunch':
      return 'Steam Launch';
    case 'proton_run':
      return 'Proton Launch';
    case 'native':
      return 'Native Launch';
    default:
      return method;
  }
}

function isStale(generatedAt: string): boolean {
  const generatedTime = new Date(generatedAt).getTime();
  if (!Number.isFinite(generatedTime)) {
    return true;
  }

  return Date.now() - generatedTime > 60_000;
}

function buildSummaryParts(preview: LaunchPreview): ReactNode[] {
  const envCount = preview.environment?.length ?? 0;
  const wrapperCount = preview.wrappers?.length ?? 0;
  const fatalCount = preview.validation.issues.filter((i) => i.severity === 'fatal').length;
  const warningCount = preview.validation.issues.filter((i) => i.severity === 'warning').length;
  const passedCount = preview.validation.issues.filter((i) => i.severity === 'info').length;

  const parts: ReactNode[] = [
    <span key="env" className="crosshook-preview-modal__count--success">
      {envCount} env vars
    </span>,
    <span key="wrap" className="crosshook-preview-modal__count--success">
      {wrapperCount} {wrapperCount === 1 ? 'wrapper' : 'wrappers'}
    </span>,
  ];

  if (passedCount > 0) {
    parts.push(
      <span key="pass" className="crosshook-preview-modal__count--success">
        {passedCount} passed
      </span>,
    );
  }
  if (warningCount > 0) {
    parts.push(
      <span key="warn" className="crosshook-preview-modal__count--warning">
        {warningCount} {warningCount === 1 ? 'warning' : 'warnings'}
      </span>,
    );
  }
  if (fatalCount > 0) {
    parts.push(
      <span key="err" className="crosshook-preview-modal__count--danger">
        {fatalCount} {fatalCount === 1 ? 'error' : 'errors'}
      </span>,
    );
  }
  if (preview.validation.issues.length === 0) {
    parts.push(
      <span key="ok" className="crosshook-preview-modal__count--success">
        all checks passed
      </span>,
    );
  }

  return parts;
}

/* ───────── PreviewModal component ───────── */

interface PreviewModalProps {
  preview: LaunchPreview;
  profileId: string;
  onClose: () => void;
  onLaunch: () => void;
}

function PreviewModal({ preview, profileId, onClose, onLaunch }: PreviewModalProps) {
  const portalHostRef = useRef<HTMLElement | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const bodyStyleRef = useRef('');
  const hiddenNodesRef = useRef<
    Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>
  >([]);
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

    previouslyFocusedRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;

    bodyStyleRef.current = body.style.overflow;
    body.style.overflow = 'hidden';
    body.classList.add('crosshook-modal-open');

    hiddenNodesRef.current = Array.from(body.children)
      .filter(
        (child): child is HTMLElement =>
          child instanceof HTMLElement && child !== portalHost,
      )
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
      if (restoreTarget && restoreTarget.isConnected) {
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
      <div
        className="crosshook-modal__backdrop"
        aria-hidden="true"
        onMouseDown={handleBackdropMouseDown}
      />
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
                previewIsReady
                  ? 'crosshook-modal__status-chip--success'
                  : 'crosshook-modal__status-chip--danger',
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
              <div className="crosshook-preview-modal__summary-value">
                {methodLabel(preview.resolved_method)}
              </div>
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
            {summaryParts.reduce<ReactNode[]>((acc, part, i) => {
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
                  {preview.validation.issues.length}{' '}
                  {preview.validation.issues.length === 1 ? 'check' : 'checks'}
                </span>
              }
            >
              {sortedIssues.length > 0 ? (
                <ul className="crosshook-preview-modal__validation-list">
                  {sortedIssues.map((issue, i) => (
                    <li
                      key={i}
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
                        <div className="crosshook-preview-modal__validation-message">
                          {issue.message}
                        </div>
                        {issue.help ? (
                          <div className="crosshook-preview-modal__validation-help">
                            {issue.help}
                          </div>
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
                <pre className="crosshook-preview-modal__command-block">
                  {preview.effective_command}
                </pre>
              ) : (
                <p className="crosshook-preview-modal__empty">No command resolved.</p>
              )}
              {isSteam && preview.steam_launch_options ? (
                <>
                  <div className="crosshook-preview-modal__sub-label">Steam Launch Options</div>
                  <pre className="crosshook-preview-modal__command-block">
                    {preview.steam_launch_options}
                  </pre>
                </>
              ) : null}
              {preview.directives_error ? (
                <div className="crosshook-preview-modal__directives-error">
                  {preview.directives_error}
                </div>
              ) : null}
            </CollapsibleSection>

            {/* Environment Variables */}
            <CollapsibleSection
              title="Environment Variables"
              defaultOpen={false}
              meta={<span style={{ fontSize: '0.82rem' }}>{envCount} vars</span>}
            >
              {preview.environment && preview.environment.length > 0 ? (
                <>
                  {groupEnvBySource(preview.environment).map(([group, vars]) => (
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
                  ))}
                </>
              ) : (
                <p className="crosshook-preview-modal__empty">
                  No environment variables resolved.
                </p>
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
                      <div className="crosshook-preview-modal__field-label">
                        Steam Client Install
                      </div>
                      <div className="crosshook-preview-modal__field-value">
                        {preview.proton_setup.steam_client_install_path}
                      </div>
                    </div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">
                        Proton Executable
                      </div>
                      <div className="crosshook-preview-modal__field-value">
                        {preview.proton_setup.proton_executable}
                      </div>
                    </div>
                  </div>
                ) : (
                  <p className="crosshook-preview-modal__empty">
                    No Proton setup data available.
                  </p>
                )}
                {preview.trainer ? (
                  <div className="crosshook-preview-modal__field-list" style={{ marginTop: 16 }}>
                    <div className="crosshook-preview-modal__sub-label">Trainer Info</div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Trainer Path</div>
                      <div className="crosshook-preview-modal__field-value">
                        {preview.trainer.path}
                      </div>
                    </div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Host Path</div>
                      <div className="crosshook-preview-modal__field-value">
                        {preview.trainer.host_path}
                      </div>
                    </div>
                    <div className="crosshook-preview-modal__field">
                      <div className="crosshook-preview-modal__field-label">Loading Mode</div>
                      <div className="crosshook-preview-modal__field-value">
                        {preview.trainer.loading_mode}
                      </div>
                    </div>
                    {preview.trainer.staged_path ? (
                      <div className="crosshook-preview-modal__field">
                        <div className="crosshook-preview-modal__field-label">Staged Path</div>
                        <div className="crosshook-preview-modal__field-value">
                          {preview.trainer.staged_path}
                        </div>
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
            className={[
              'crosshook-preview-modal__timestamp',
              stale ? 'crosshook-preview-modal__timestamp--stale' : '',
            ]
              .filter(Boolean)
              .join(' ')}
          >
            Generated {generatedTimeLabel}
            {stale ? ' (stale)' : ''}
          </span>
          <div className="crosshook-modal__footer-actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={handleCopy}
            >
              {copyLabel}
            </button>
            <button
              type="button"
              className="crosshook-button"
              onClick={onLaunch}
              disabled={!previewIsReady}
            >
              Launch Now
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={onClose}
            >
              Close
            </button>
          </div>
        </footer>
      </div>
    </div>,
    portalHostRef.current,
  );
}

/* ───────── LaunchPanel component ───────── */

interface LaunchPanelProps {
  profileId: string;
  method: Exclude<LaunchMethod, ''>;
  request: LaunchRequest | null;
}

function buildGameOnlyRequest(request: LaunchRequest): LaunchRequest {
  return {
    ...request,
    launch_game_only: true,
    launch_trainer_only: false,
  };
}

export function LaunchPanel({ profileId, method, request }: LaunchPanelProps) {
  const {
    actionLabel,
    canLaunchGame,
    canLaunchTrainer,
    feedback,
    helperLogPath,
    hintText,
    isBusy,
    launchGame,
    launchTrainer,
    phase,
    reset,
    statusText,
  } = useLaunchState({
    profileId,
    method,
    request,
  });

  const { loading, preview, error: previewError, requestPreview, clearPreview } = usePreviewState();
  const [showPreview, setShowPreview] = useState(false);
  const [diagnosticExpanded, setDiagnosticExpanded] = useState(false);
  const [diagnosticCopyLabel, setDiagnosticCopyLabel] = useState('Copy Report');

  useEffect(() => {
    if (preview) {
      setShowPreview(true);
    }
  }, [preview]);

  const isIdle = phase === LaunchPhase.Idle;
  const previewDisabled = !request || !isIdle || loading;

  const isWaitingForTrainer = phase === LaunchPhase.WaitingForTrainer;
  const isSessionActive = phase === LaunchPhase.SessionActive;
  const canLaunch = isWaitingForTrainer ? canLaunchTrainer : canLaunchGame;
  const primaryAction = isWaitingForTrainer ? launchTrainer : launchGame;
  const validationFeedback = feedback?.kind === 'validation' ? feedback.issue : null;
  const diagnosticFeedback = feedback?.kind === 'diagnostic' ? feedback.report : null;
  const runtimeFeedback = feedback?.kind === 'runtime' ? feedback.message : null;
  const diagnosticMatches = diagnosticFeedback
    ? sortPatternMatchesBySeverity(diagnosticFeedback.pattern_matches)
    : [];
  const visibleDiagnosticMatches = diagnosticExpanded
    ? diagnosticMatches
    : diagnosticMatches.slice(0, 3);
  const feedbackSeverity =
    diagnosticFeedback?.severity ?? validationFeedback?.severity ?? 'fatal';
  const feedbackLabel =
    feedbackSeverity === 'fatal'
      ? 'Fatal'
      : feedbackSeverity === 'warning'
        ? 'Warning'
        : 'Info';

  useEffect(() => {
    setDiagnosticExpanded(false);
    setDiagnosticCopyLabel('Copy Report');
  }, [diagnosticFeedback?.analyzed_at]);

  function handleClosePreview() {
    setShowPreview(false);
    clearPreview();
  }

  function handleLaunchFromPreview() {
    setShowPreview(false);
    clearPreview();
    launchGame();
  }

  async function handleCopyDiagnosticReport() {
    if (!diagnosticFeedback) {
      return;
    }

    try {
      await copyToClipboard(JSON.stringify(diagnosticFeedback, null, 2));
      setDiagnosticCopyLabel('Copied!');
      window.setTimeout(() => {
        setDiagnosticCopyLabel('Copy Report');
      }, 2000);
    } catch {
      setDiagnosticCopyLabel('Copy Failed');
      window.setTimeout(() => {
        setDiagnosticCopyLabel('Copy Report');
      }, 2000);
    }
  }

  return (
    <section className="crosshook-launch-panel">
      <div className="crosshook-launch-panel__header">
        <div>
          <p className="crosshook-launch-panel__eyebrow">
            {method === 'steam_applaunch' ? 'Steam Launch' : method === 'proton_run' ? 'Proton Launch' : 'Native Launch'}
          </p>
          <h1 className="crosshook-launch-panel__title">CrossHook Native</h1>
          <p className="crosshook-launch-panel__copy">
            {method === 'native'
              ? 'Direct launch flow for Linux-native executables, driven by the native Tauri backend.'
              : `Two-step launch flow for ${method === 'steam_applaunch' ? 'Steam' : 'Proton'} games and trainers, driven by the native Tauri backend.`}
          </p>
        </div>

        <div className="crosshook-launch-panel__status" data-phase={phase}>
          {phase}
        </div>
      </div>

      <div className="crosshook-launch-panel__info">
        <p className="crosshook-launch-panel__status-text">{statusText}</p>
        <p className="crosshook-launch-panel__hint">{hintText}</p>
        {helperLogPath ? (
          <p className="crosshook-launch-panel__helper-log">Helper log: {helperLogPath}</p>
        ) : null}
        {feedback ? (
          <div
            className="crosshook-launch-panel__feedback"
            data-kind={feedback.kind}
            data-severity={feedbackSeverity}
            role="alert"
          >
            {diagnosticFeedback ? (
              <>
                <div className="crosshook-launch-panel__feedback-header">
                  <span className="crosshook-launch-panel__feedback-badge">
                    {feedbackLabel}
                  </span>
                  <p className="crosshook-launch-panel__feedback-title">
                    {diagnosticFeedback.summary}
                  </p>
                </div>
                <p className="crosshook-launch-panel__feedback-help">
                  {diagnosticFeedback.exit_info.description}
                </p>
                {visibleDiagnosticMatches.length > 0 ? (
                  <ul className="crosshook-launch-panel__feedback-list">
                    {visibleDiagnosticMatches.map((patternMatch) => (
                      <li
                        key={`${diagnosticFeedback.analyzed_at}-${patternMatch.pattern_id}`}
                        className="crosshook-launch-panel__feedback-item"
                      >
                        <div className="crosshook-launch-panel__feedback-header">
                          <span
                            className="crosshook-launch-panel__feedback-badge"
                            data-severity={patternMatch.severity}
                          >
                            {patternMatch.severity}
                          </span>
                          <p className="crosshook-launch-panel__feedback-title">
                            {patternMatch.summary}
                          </p>
                        </div>
                        <p className="crosshook-launch-panel__feedback-help">
                          {patternMatch.suggestion}
                        </p>
                      </li>
                    ))}
                  </ul>
                ) : null}
                <div className="crosshook-launch-panel__feedback-actions">
                  {diagnosticMatches.length > 3 || diagnosticFeedback.suggestions.length > 0 ? (
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary crosshook-launch-panel__feedback-action"
                      onClick={() => setDiagnosticExpanded((current) => !current)}
                    >
                      {diagnosticExpanded ? 'Show Less' : 'Show Details'}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--secondary crosshook-launch-panel__feedback-action"
                    onClick={handleCopyDiagnosticReport}
                  >
                    {diagnosticCopyLabel}
                  </button>
                </div>
                {diagnosticExpanded ? (
                  <div className="crosshook-launch-panel__feedback-details">
                    <p className="crosshook-launch-panel__feedback-help">
                      Exit mode: {diagnosticFeedback.exit_info.failure_mode}
                    </p>
                    <p className="crosshook-launch-panel__feedback-help">
                      Exit code: {diagnosticFeedback.exit_info.code ?? 'n/a'} | Signal: {diagnosticFeedback.exit_info.signal ?? 'n/a'}
                    </p>
                    {diagnosticFeedback.log_tail_path ? (
                      <p className="crosshook-launch-panel__feedback-help">
                        Log tail: {diagnosticFeedback.log_tail_path}
                      </p>
                    ) : null}
                    {diagnosticFeedback.suggestions.length > 0 ? (
                      <ul className="crosshook-launch-panel__feedback-list">
                        {diagnosticFeedback.suggestions.map((suggestion, index) => (
                          <li
                            key={`${diagnosticFeedback.analyzed_at}-suggestion-${index}`}
                            className="crosshook-launch-panel__feedback-item"
                          >
                            <div className="crosshook-launch-panel__feedback-header">
                              <span
                                className="crosshook-launch-panel__feedback-badge"
                                data-severity={suggestion.severity}
                              >
                                {suggestion.severity}
                              </span>
                              <p className="crosshook-launch-panel__feedback-title">
                                {suggestion.title}
                              </p>
                            </div>
                            <p className="crosshook-launch-panel__feedback-help">
                              {suggestion.description}
                            </p>
                          </li>
                        ))}
                      </ul>
                    ) : null}
                  </div>
                ) : null}
              </>
            ) : validationFeedback ? (
              <>
                <div className="crosshook-launch-panel__feedback-header">
                  <span className="crosshook-launch-panel__feedback-badge">
                    {feedbackLabel}
                  </span>
                  <p className="crosshook-launch-panel__feedback-title">
                    {validationFeedback.message}
                  </p>
                </div>
                <p className="crosshook-launch-panel__feedback-help">
                  {validationFeedback.help}
                </p>
              </>
            ) : (
              <p className="crosshook-launch-panel__feedback-title">
                {runtimeFeedback}
              </p>
            )}
          </div>
        ) : null}
      </div>

      <div className="crosshook-launch-panel__actions">
        <button
          type="button"
          className="crosshook-button crosshook-launch-panel__action"
          onClick={primaryAction}
          disabled={!canLaunch || isBusy}
        >
          {actionLabel}
        </button>

        <button
          type="button"
          className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
          onClick={() => request && requestPreview(buildGameOnlyRequest(request))}
          disabled={previewDisabled}
        >
          {loading ? 'Loading Preview\u2026' : 'Preview Launch'}
        </button>

        <button
          type="button"
          className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
          onClick={reset}
        >
          Reset
        </button>
      </div>

      {previewError ? (
        <p className="crosshook-preview-modal__preview-error" role="alert">
          Preview failed: {previewError}
        </p>
      ) : null}

      <div
        className="crosshook-launch-panel__indicator"
        data-state={isSessionActive ? 'active' : isWaitingForTrainer ? 'waiting' : 'idle'}
      >
        <div className="crosshook-launch-panel__indicator-row">
          <span className="crosshook-launch-panel__indicator-dot" aria-hidden="true" />
          <span className="crosshook-launch-panel__indicator-label">
            {method === 'steam_applaunch'
              ? 'Steam runner selected'
              : method === 'proton_run'
                ? 'Proton runner selected'
                : 'Native runner selected'}
          </span>
        </div>
        <div className="crosshook-launch-panel__indicator-copy">
          {request ? 'Profile request is loaded.' : 'No profile request is loaded yet.'}
        </div>
      </div>

      {showPreview && preview ? (
        <PreviewModal
          preview={preview}
          profileId={profileId}
          onClose={handleClosePreview}
          onLaunch={handleLaunchFromPreview}
        />
      ) : null}
    </section>
  );
}

export default LaunchPanel;
