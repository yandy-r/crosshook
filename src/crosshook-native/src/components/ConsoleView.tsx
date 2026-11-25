import { memo, type UIEvent, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import {
  appendLogRingBuffer,
  CONSOLE_MAX_LINES,
  EMPTY_LOG_RING_BUFFER,
  type LogRingBufferState,
} from '@/utils/logRingBuffer';

import { type LogPayload, normalizeLogMessage } from '../utils/log';

type ConsoleLine = {
  id: number;
  timestamp: string;
  text: string;
};

let cachedTimestampSecond = -1;
let cachedTimestampText = '';

function formatTimestampNow(): string {
  const now = Date.now();
  const second = Math.floor(now / 1000);
  if (second !== cachedTimestampSecond) {
    cachedTimestampSecond = second;
    cachedTimestampText = new Date(now).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  }
  return cachedTimestampText;
}

const ConsoleLineRow = memo(function ConsoleLineRow({ line }: { line: ConsoleLine }) {
  return (
    <div className="crosshook-console__line">
      <span className="crosshook-console__timestamp">[{line.timestamp}]</span>
      <pre className="crosshook-console__code">{line.text}</pre>
    </div>
  );
});

export interface ConsoleViewProps {
  /**
   * When false (e.g., the surrounding drawer is collapsed), incoming lines keep
   * buffering in the ring buffer without reconciling DOM rows until visible.
   */
  renderLines?: boolean;
}

export function ConsoleView({ renderLines = true }: ConsoleViewProps = {}) {
  const [collapsed, setCollapsed] = useState(false);
  const [logState, setLogState] = useState<LogRingBufferState<ConsoleLine>>(EMPTY_LOG_RING_BUFFER);
  const nextId = useRef(1);
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const shouldFollowRef = useRef(true);
  // Lines arrive far faster than the view can usefully re-render, so each event
  // only queues its line; one state update per animation frame drains the queue.
  const pendingLinesRef = useRef<ConsoleLine[]>([]);
  const pendingDroppedRef = useRef(0);
  const rafIdRef = useRef<number | null>(null);

  const scrollToBottom = useCallback(() => {
    const body = bodyRef.current;
    if (!body) {
      return;
    }

    body.scrollTop = body.scrollHeight;
  }, []);

  function updateFollowState(element: HTMLDivElement) {
    const distanceFromBottom = element.scrollHeight - (element.scrollTop + element.clientHeight);
    shouldFollowRef.current = distanceFromBottom <= 24;
  }

  function handleBodyScroll(event: UIEvent<HTMLDivElement>) {
    updateFollowState(event.currentTarget);
  }

  const discardPendingLines = useCallback(() => {
    if (rafIdRef.current !== null) {
      window.cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }
    pendingLinesRef.current = [];
    pendingDroppedRef.current = 0;
  }, []);

  useEffect(() => {
    let active = true;

    const flushPending = () => {
      // Clear the handle first: a later log event must schedule a new frame.
      rafIdRef.current = null;
      const batch = pendingLinesRef.current;
      const preDropped = pendingDroppedRef.current;
      pendingLinesRef.current = [];
      pendingDroppedRef.current = 0;
      setLogState((prev) => appendLogRingBuffer(prev, batch, CONSOLE_MAX_LINES, preDropped));
    };

    const handler = (event: { payload: LogPayload }) => {
      const text = normalizeLogMessage(event.payload).trimEnd();
      if (!text || !active) {
        return;
      }

      pendingLinesRef.current.push({
        id: nextId.current++,
        timestamp: formatTimestampNow(),
        text,
      });

      // Keep the queue bounded even when one frame delivers far more lines than
      // the buffer holds; the trimmed lines are folded into the dropped count.
      if (pendingLinesRef.current.length > CONSOLE_MAX_LINES * 2) {
        const overflow = pendingLinesRef.current.length - CONSOLE_MAX_LINES;
        pendingLinesRef.current.splice(0, overflow);
        pendingDroppedRef.current += overflow;
      }

      if (rafIdRef.current === null) {
        rafIdRef.current = window.requestAnimationFrame(flushPending);
      }
    };

    const unlistenLaunch = subscribeEvent<LogPayload>('launch-log', handler);
    const unlistenUpdate = subscribeEvent<LogPayload>('update-log', handler);

    return () => {
      active = false;
      // A frame still queued must not flush into an unmounted view.
      discardPendingLines();
      void unlistenLaunch.then((unlisten) => unlisten());
      void unlistenUpdate.then((unlisten) => unlisten());
    };
  }, [discardPendingLines]);

  // `logState.lines` is not read here on purpose: listing it re-runs the effect
  // after every flushed batch so the view keeps following new output.
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentional extra dependency — re-scroll when a batch lands
  useLayoutEffect(() => {
    if (!collapsed && renderLines && shouldFollowRef.current) {
      scrollToBottom();
    }
  }, [collapsed, renderLines, logState.lines, scrollToBottom]);

  const handleClear = useCallback(() => {
    discardPendingLines();
    setLogState(EMPTY_LOG_RING_BUFFER);
  }, [discardPendingLines]);

  return (
    <section aria-label="Console log" className="crosshook-console">
      <header className="crosshook-console__header">
        <div>
          <div className="crosshook-heading-eyebrow">Runtime Console</div>
          <div className="crosshook-heading-copy">Helper log stream</div>
        </div>

        <div>
          <button type="button" onClick={handleClear} className="crosshook-button crosshook-button--secondary">
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
          {renderLines ? (
            <>
              {logState.droppedCount > 0 ? (
                <div className="crosshook-console__dropped" role="note">
                  {logState.droppedCount === 1
                    ? '1 earlier line dropped'
                    : `${logState.droppedCount} earlier lines dropped`}
                </div>
              ) : null}
              {logState.lines.length === 0 && logState.droppedCount === 0 ? (
                <div className="crosshook-console__empty">
                  <div className="crosshook-heading-copy">Waiting for log output</div>
                  <div className="crosshook-heading-copy">
                    Launch a game, apply an update, or run an installer to stream helper output here. New lines appear
                    automatically.
                  </div>
                </div>
              ) : (
                <div>
                  {logState.lines.map((line) => (
                    <ConsoleLineRow key={line.id} line={line} />
                  ))}
                </div>
              )}
            </>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

export default ConsoleView;
