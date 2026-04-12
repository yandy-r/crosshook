import { type UIEvent, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';

import { type LogPayload, normalizeLogMessage } from '../utils/log';

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

    const handler = (event: { payload: LogPayload }) => {
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
    };

    const unlistenLaunch = subscribeEvent<LogPayload>('launch-log', handler);
    const unlistenUpdate = subscribeEvent<LogPayload>('update-log', handler);

    return () => {
      active = false;
      void unlistenLaunch.then((unlisten) => unlisten());
      void unlistenUpdate.then((unlisten) => unlisten());
    };
  }, []);

  useLayoutEffect(() => {
    if (!collapsed && shouldFollowRef.current) {
      scrollToBottom();
    }
  }, [collapsed, scrollToBottom]);

  return (
    <section aria-label="Console log" className="crosshook-console">
      <header className="crosshook-console__header">
        <div>
          <div className="crosshook-heading-eyebrow">Runtime Console</div>
          <div className="crosshook-heading-copy">Helper log stream</div>
        </div>

        <div>
          <button type="button" onClick={() => setLines([])} className="crosshook-button crosshook-button--secondary">
            Clear
          </button>{' '}
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
              <div className="crosshook-heading-copy">Waiting for log output</div>
              <div className="crosshook-heading-copy">
                Launch a game, apply an update, or run an installer to stream helper output here. New lines appear
                automatically.
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
