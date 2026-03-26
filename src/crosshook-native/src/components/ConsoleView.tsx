import { useEffect, useLayoutEffect, useRef, useState, type UIEvent } from 'react';
import { listen } from '@tauri-apps/api/event';

type ConsoleLine = {
  id: number;
  timestamp: string;
  text: string;
};

function formatTimestamp(date: Date) {
  return date.toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

function normalizeLogMessage(payload: unknown) {
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

export function ConsoleView() {
  const [collapsed, setCollapsed] = useState(false);
  const [lines, setLines] = useState<ConsoleLine[]>([]);
  const nextId = useRef(1);
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const shouldFollowRef = useRef(true);

  function scrollToBottom() {
    const body = bodyRef.current;
    if (!body) {
      return;
    }

    body.scrollTop = body.scrollHeight;
  }

  function updateFollowState(element: HTMLDivElement) {
    const distanceFromBottom = element.scrollHeight - (element.scrollTop + element.clientHeight);
    shouldFollowRef.current = distanceFromBottom <= 24;
  }

  function handleBodyScroll(event: UIEvent<HTMLDivElement>) {
    updateFollowState(event.currentTarget);
  }

  useEffect(() => {
    let active = true;

    const unlistenPromise = listen<unknown>('launch-log', (event) => {
      const text = normalizeLogMessage(event.payload).trimEnd();
      if (!text) {
        return;
      }

      const entry = {
        id: nextId.current++,
        timestamp: formatTimestamp(new Date()),
        text,
      };

      if (active) {
        setLines((current) => [...current, entry]);
      }
    });

    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useLayoutEffect(() => {
    if (!collapsed && shouldFollowRef.current) {
      scrollToBottom();
    }
  }, [collapsed, lines.length]);

  return (
    <section aria-label="Console log" className="crosshook-console">
      <header className="crosshook-console__header">
        <div>
          <div className="crosshook-heading-eyebrow">Runtime Console</div>
          <div className="crosshook-heading-copy">
            Helper log stream
          </div>
        </div>

        <div>
          <button type="button" onClick={() => setLines([])} className="crosshook-button crosshook-button--secondary">
            Clear
          </button>
          {' '}
          <button
            type="button"
            onClick={() => setCollapsed((value) => !value)}
            className="crosshook-button crosshook-button--secondary"
          >
            {collapsed ? 'Expand' : 'Collapse'}
          </button>
        </div>
      </header>

      {!collapsed ? (
        <div ref={bodyRef} onScroll={handleBodyScroll} className="crosshook-console__body">
          {lines.length === 0 ? (
            <div className="crosshook-console__empty">
              <div className="crosshook-heading-copy">
                Waiting for log output
              </div>
              <div className="crosshook-heading-copy">
                Launch a game or trainer to stream helper output here. New lines appear automatically as CrossHook emits
                `launch-log` events.
              </div>
            </div>
          ) : (
            <div>
              {lines.map((line) => (
                <div key={line.id} className="crosshook-console__line">
                  <span className="crosshook-console__timestamp">[{line.timestamp}]</span>
                  <pre className="crosshook-console__code">{line.text}</pre>
                </div>
              ))}
            </div>
          )}
        </div>
      ) : null}
    </section>
  );
}

export default ConsoleView;
