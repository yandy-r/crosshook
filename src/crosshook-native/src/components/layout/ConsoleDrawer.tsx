import { type RefObject, useEffect, useId, useMemo, useState } from 'react';
import type { PanelImperativeHandle } from 'react-resizable-panels';
import { useHostReadinessContext } from '@/context/HostReadinessContext';
import { subscribeEvent } from '@/lib/events';
import type { Capability } from '@/types/onboarding';
import { type LogPayload, normalizeLogMessage } from '../../utils/log';
import ConsoleView from '../ConsoleView';

function countLogLines(payload: unknown): number {
  const text = normalizeLogMessage(payload).trimEnd();
  if (!text) {
    return 0;
  }

  return text.split(/\r?\n/).filter((line) => line.trim().length > 0).length;
}

function formatLineCount(count: number): string {
  return count === 1 ? '1 line' : `${count} lines`;
}

function resolveCountTone(readyCount: number, totalCount: number): 'success' | 'warning' | 'danger' | 'muted' {
  // Match CapabilitySummaryStrip: zero total means nothing to wait on (treated as available/success).
  if (totalCount === 0) {
    return 'success';
  }
  if (readyCount >= totalCount) {
    return 'success';
  }
  if (readyCount === 0) {
    return 'danger';
  }
  return 'warning';
}

function countAvailableCapabilities(capabilities: Capability[]): number {
  return capabilities.filter((capability) => capability.state === 'available').length;
}

export type ConsoleMode = 'drawer' | 'status';

function ConsoleStatusBar({ lineCount }: { lineCount: number }) {
  const { snapshot, capabilities, isStale, isRefreshing, error } = useHostReadinessContext();

  const readinessSummary = useMemo(() => {
    const toolChecks = snapshot?.tool_checks ?? [];
    const requiredTotal = toolChecks.filter((tool) => tool.is_required).length;
    const requiredReady = toolChecks.filter((tool) => tool.is_required && tool.is_available).length;
    const optionalTotal = capabilities.length;
    const optionalAvailable = countAvailableCapabilities(capabilities);
    return {
      requiredReady,
      requiredTotal,
      optionalAvailable,
      optionalTotal,
      requiredTone: resolveCountTone(requiredReady, requiredTotal),
      optionalTone: resolveCountTone(optionalAvailable, optionalTotal),
    };
  }, [capabilities, snapshot?.tool_checks]);

  return (
    <section
      aria-label="Runtime console status"
      className="crosshook-console-drawer crosshook-console-drawer--status"
      data-testid="console-status-bar"
    >
      <div className="crosshook-console-drawer__status-inner">
        <span className="crosshook-console-drawer__status-label">Runtime console</span>
        <div className="crosshook-console-drawer__status-chips" role="status" aria-live="polite">
          <span className="crosshook-status-chip crosshook-status-chip--muted">{formatLineCount(lineCount)}</span>
          {error ? (
            <span className="crosshook-status-chip crosshook-status-chip--warning">Host tools unavailable</span>
          ) : snapshot === null && isRefreshing ? (
            <span className="crosshook-status-chip crosshook-status-chip--muted">Checking host tools</span>
          ) : (
            <>
              <span className={`crosshook-status-chip crosshook-status-chip--${readinessSummary.requiredTone}`}>
                {readinessSummary.requiredTotal > 0
                  ? `${readinessSummary.requiredReady}/${readinessSummary.requiredTotal} required`
                  : 'Required tools ready'}
              </span>
              <span className={`crosshook-status-chip crosshook-status-chip--${readinessSummary.optionalTone}`}>
                {readinessSummary.optionalTotal > 0
                  ? `${readinessSummary.optionalAvailable}/${readinessSummary.optionalTotal} capabilities`
                  : 'No capabilities'}
              </span>
              {isStale ? (
                <span className="crosshook-status-chip crosshook-status-chip--warning">Stale data</span>
              ) : null}
            </>
          )}
        </div>
        <span className="crosshook-console-drawer__status-tip">⌘K commands</span>
      </div>
    </section>
  );
}

interface ConsoleDrawerProps {
  panelRef: RefObject<PanelImperativeHandle | null>;
  mode?: ConsoleMode;
  /** When true (default), the drawer starts collapsed until the user expands it. */
  defaultCollapsed?: boolean;
}

export function ConsoleDrawer({ panelRef, mode = 'drawer', defaultCollapsed = true }: ConsoleDrawerProps) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);
  const [lineCount, setLineCount] = useState(0);
  const bodyId = useId();

  useEffect(() => {
    if (mode === 'drawer') {
      setCollapsed(defaultCollapsed);
    }
  }, [defaultCollapsed, mode]);

  const collapse = () => {
    setCollapsed(true);
    panelRef.current?.collapse();
  };

  const expand = () => {
    setCollapsed(false);
    panelRef.current?.expand();
  };

  const toggle = () => {
    if (collapsed) {
      expand();
    } else {
      collapse();
    }
  };

  useEffect(() => {
    let active = true;

    // Mirror the launch-log and update-log streams so the badge stays in sync without changing ConsoleView.
    const handler = (event: { payload: LogPayload }) => {
      const nextLineCount = countLogLines(event.payload);
      if (nextLineCount === 0 || !active) {
        return;
      }

      setLineCount((current) => current + nextLineCount);
    };

    const unlistenLaunch = subscribeEvent<LogPayload>('launch-log', handler);
    const unlistenUpdate = subscribeEvent<LogPayload>('update-log', handler);

    return () => {
      active = false;
      void unlistenLaunch.then((unlisten) => unlisten());
      void unlistenUpdate.then((unlisten) => unlisten());
    };
  }, []);

  if (mode === 'status') {
    return <ConsoleStatusBar lineCount={lineCount} />;
  }

  return (
    <section
      aria-label="Runtime console"
      className={`crosshook-console-drawer${collapsed ? ' crosshook-console-drawer--collapsed' : ''}`}
      data-testid="console-drawer"
    >
      <button
        type="button"
        aria-controls={bodyId}
        aria-expanded={!collapsed}
        aria-label={collapsed ? 'Expand runtime console' : 'Collapse runtime console'}
        className="crosshook-console-drawer__toggle"
        onClick={toggle}
      >
        <span
          style={{
            display: 'grid',
            gap: '2px',
            textAlign: 'left',
          }}
        >
          <span
            style={{
              color: 'var(--crosshook-color-accent-strong)',
              fontSize: '0.72rem',
              fontWeight: 700,
              letterSpacing: '0.18em',
              textTransform: 'uppercase',
            }}
          >
            Runtime Console
          </span>
          <span
            style={{
              color: 'var(--crosshook-color-text-muted)',
              fontSize: '0.88rem',
            }}
          >
            Helper log stream
          </span>
        </span>

        <span
          style={{
            minWidth: '5.75rem',
            padding: '6px 10px',
            borderRadius: '999px',
            background: 'rgba(255, 255, 255, 0.06)',
            color: 'var(--crosshook-color-text)',
            fontSize: '0.8rem',
            fontWeight: 700,
            textAlign: 'center',
          }}
        >
          {formatLineCount(lineCount)}
        </span>

        <span
          aria-hidden="true"
          style={{
            color: 'var(--crosshook-color-text-subtle)',
            fontSize: '0.8rem',
            fontWeight: 700,
            letterSpacing: '0.08em',
            textTransform: 'uppercase',
          }}
        >
          {collapsed ? 'Expand' : 'Collapse'}
        </span>
      </button>

      <div id={bodyId} aria-hidden={collapsed} className="crosshook-console-drawer__body">
        <ConsoleView />
      </div>
    </section>
  );
}

export default ConsoleDrawer;
