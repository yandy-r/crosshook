import { useEffect, useMemo, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import type { InjectionLogEvent } from '@/types/injection';
import { isInjectionLogEvent } from '@/types/injection';

const MAX_INJECTION_LOG_ROWS = 200;

interface InjectionLogRow {
  id: number;
  event: InjectionLogEvent;
}

export interface InjectionLogTailProps {
  profileName?: string;
  sessionId?: string;
  sessionKind?: InjectionLogEvent['session_kind'];
}

function formatTimestamp(timestamp: string): string {
  const parsed = new Date(timestamp);
  if (Number.isNaN(parsed.getTime())) {
    return timestamp;
  }
  return parsed.toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

function eventMatchesScope(event: InjectionLogEvent, props: InjectionLogTailProps): boolean {
  const expectedProfileName = props.profileName?.trim();
  if (expectedProfileName && event.profile_name.trim() !== expectedProfileName) {
    return false;
  }
  if (props.sessionId && event.session_id !== props.sessionId) {
    return false;
  }
  if (props.sessionKind && event.session_kind !== props.sessionKind) {
    return false;
  }
  return true;
}

function levelLabel(level: InjectionLogEvent['level']): string {
  switch (level) {
    case 'error':
      return 'Error';
    case 'warning':
      return 'Warning';
    default:
      return 'Info';
  }
}

export function InjectionLogTail({ profileName, sessionId, sessionKind }: InjectionLogTailProps) {
  const [rows, setRows] = useState<InjectionLogRow[]>([]);
  const nextRowIdRef = useRef(1);

  const scope = useMemo(
    () => ({
      profileName,
      sessionId,
      sessionKind,
    }),
    [profileName, sessionId, sessionKind]
  );

  useEffect(() => {
    let active = true;

    const unlisten = subscribeEvent<InjectionLogEvent>('injection-log', (event) => {
      if (!active || !isInjectionLogEvent(event.payload) || !eventMatchesScope(event.payload, scope)) {
        return;
      }

      const row = {
        id: nextRowIdRef.current++,
        event: event.payload,
      };
      setRows((current) => [...current, row].slice(-MAX_INJECTION_LOG_ROWS));
    });

    return () => {
      active = false;
      void unlisten.then((fn) => fn());
    };
  }, [scope]);

  return (
    <section className="crosshook-hero-detail__trainer-log" aria-label="Recent injection log">
      <div className="crosshook-hero-detail__trainer-section-header">
        <div>
          <h3 className="crosshook-hero-detail__section-title">Recent injection log</h3>
          <p className="crosshook-hero-detail__muted">
            Runtime-only trainer lifecycle events for the selected profile.
          </p>
        </div>
        <span className="crosshook-hero-detail__trainer-log-count">{rows.length}</span>
      </div>

      {rows.length === 0 ? (
        <p className="crosshook-hero-detail__hook-empty" role="status">
          No trainer or injection events recorded for this profile in this session.
        </p>
      ) : (
        <ol className="crosshook-hero-detail__trainer-log-rows" aria-live="polite">
          {rows.map((row) => (
            <li
              key={row.id}
              className={[
                'crosshook-hero-detail__trainer-log-row',
                `crosshook-hero-detail__trainer-log-row--${row.event.level}`,
                row.event.unsupported_runtime ? 'crosshook-hero-detail__trainer-log-row--unsupported' : '',
              ]
                .filter(Boolean)
                .join(' ')}
            >
              <div className="crosshook-hero-detail__trainer-log-meta">
                <span className="crosshook-hero-detail__mono">{formatTimestamp(row.event.timestamp)}</span>
                <span>{levelLabel(row.event.level)}</span>
                <span>{row.event.source}</span>
                {row.event.hook_name ? <span>{row.event.hook_name}</span> : null}
              </div>
              <p>{row.event.message}</p>
              {row.event.unsupported_runtime ? (
                <p className="crosshook-hero-detail__muted">Stored configuration only; no DLL injection engine ran.</p>
              ) : null}
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

export default InjectionLogTail;
