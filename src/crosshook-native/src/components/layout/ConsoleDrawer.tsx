import { useEffect, useId, useState, type RefObject } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { PanelImperativeHandle } from 'react-resizable-panels';

import ConsoleView from '../ConsoleView';

function normalizeLogMessage(payload: unknown): string {
  if (typeof payload === 'string') {
    return payload;
  }

  if (payload === null || typeof payload !== 'object') {
    return '';
  }

  const record = payload as Record<string, unknown>;

  if ('line' in record && typeof record.line === 'string') {
    return record.line;
  }

  if ('message' in record && typeof record.message === 'string') {
    return record.message;
  }

  if ('text' in record && typeof record.text === 'string') {
    return record.text;
  }

  return '';
}

function countLogLines(payload: unknown): number {
  const text = normalizeLogMessage(payload).trimEnd();
  if (!text) {
    return 0;
  }

  return text
    .split(/\r?\n/)
    .filter((line) => line.trim().length > 0).length;
}

function formatLineCount(count: number): string {
  return count === 1 ? '1 line' : `${count} lines`;
}

interface ConsoleDrawerProps {
  panelRef: RefObject<PanelImperativeHandle | null>;
}

export function ConsoleDrawer({ panelRef }: ConsoleDrawerProps) {
  const [collapsed, setCollapsed] = useState(true);
  const [lineCount, setLineCount] = useState(0);
  const bodyId = useId();

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

    // Mirror the launch-log stream so the badge stays in sync without changing ConsoleView.
    const unlistenPromise = listen<unknown>('launch-log', (event) => {
      const nextLineCount = countLogLines(event.payload);
      if (nextLineCount === 0 || !active) {
        return;
      }

      setCollapsed((current) => {
        if (current) {
          panelRef.current?.expand();
          return false;
        }
        return current;
      });
      setLineCount((current) => current + nextLineCount);
    });

    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [panelRef]);

  return (
    <section
      aria-label="Runtime console"
      className={`crosshook-console-drawer${collapsed ? ' crosshook-console-drawer--collapsed' : ''}`}
    >
      <button
        type="button"
        aria-controls={bodyId}
        aria-expanded={!collapsed}
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

      <div
        id={bodyId}
        aria-hidden={collapsed}
        className="crosshook-console-drawer__body"
      >
        <ConsoleView />
      </div>
    </section>
  );
}

export default ConsoleDrawer;
