import type {
  LaunchSessionChangedPayload,
  LaunchSessionKind,
  RunningSession,
  RunningSessionsSnapshot,
} from '../../../types/launch-sessions';
import { LAUNCH_SESSION_CHANGED_EVENT } from '../../../types/launch-sessions';
import { getActiveFixture } from '../../fixture';
import { emitMockEvent } from '../eventBus';
import type { Handler } from './types';

// ---------------------------------------------------------------------------
// Mock launch-session registry. Mirrors the backend: every insert/removal bumps
// `seq` and emits `launch-session-changed`; `list_running_sessions` returns the
// sessions plus the current seq. Module-scope state (NOT on MockStore).
// ---------------------------------------------------------------------------

const sessions = new Map<string, RunningSession>();
let seq = 0;
let nextSessionNumber = 1;

function emitSessionChanged(payload: LaunchSessionChangedPayload): void {
  emitMockEvent(LAUNCH_SESSION_CHANGED_EVENT, payload);
}

/** Registers a mock session and emits `started`. Returns the new `session_id`. */
export function startMockSession(
  profileName: string | null | undefined,
  sessionKind: LaunchSessionKind,
  idPrefix: string
): string {
  const session: RunningSession = {
    session_id: `${idPrefix}-${nextSessionNumber++}`,
    profile_name: profileName?.trim() || null,
    session_kind: sessionKind,
  };
  sessions.set(session.session_id, session);
  seq += 1;
  emitSessionChanged({ kind: 'started', ...session, seq });
  return session.session_id;
}

/** Removes a mock session and emits `exited`; a no-op when it is already gone. */
export function endMockSession(sessionId: string, code: number | null, signal: number | null): void {
  const session = sessions.get(sessionId);
  if (!session) {
    return;
  }
  sessions.delete(sessionId);
  seq += 1;
  emitSessionChanged({ kind: 'exited', ...session, code, signal, seq });
}

export function registerLaunchSessions(map: Map<string, Handler>): void {
  // list_running_sessions — snapshot of the mock registry plus its seq.
  map.set('list_running_sessions', async (): Promise<RunningSessionsSnapshot> => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return { sessions: [], seq };
    if (fixture === 'loading') {
      return new Promise<RunningSessionsSnapshot>(() => {
        /* intentionally never resolves */
      });
    }
    // `error` resolves: a failed snapshot only keeps the previous running set
    // until the next event retries it, which exercises no meaningful error UX.
    return { sessions: [...sessions.values()], seq };
  });

  // launch_reset_sessions — ends every mock session for the profile and
  // returns how many were ended (each emits `exited`).
  map.set('launch_reset_sessions', async (args): Promise<number> => {
    if (getActiveFixture() === 'loading') {
      return new Promise<number>(() => {
        /* intentionally never resolves */
      });
    }
    // `empty` and `error` resolve too — reset is best-effort by contract.
    const profileName = typeof args === 'object' && args !== null && 'profileName' in args ? args.profileName : '';
    const name = typeof profileName === 'string' ? profileName.trim() : '';
    const matching = [...sessions.values()].filter((session) => session.profile_name === name);
    for (const session of matching) {
      endMockSession(session.session_id, null, null);
    }
    return matching.length;
  });

  // Dev/test helper: mark a saved profile's game as running (or stopped).
  map.set('_mock_set_profile_running', async (args): Promise<null> => {
    const { profileName, running } = args as { profileName: string; running: boolean };
    const name = profileName.trim();
    const existing = [...sessions.values()].filter(
      (session) => session.session_kind === 'game' && session.profile_name === name
    );
    if (running && existing.length === 0) {
      startMockSession(name, 'game', 'mock-seeded-game-session');
    } else if (!running) {
      for (const session of existing) {
        endMockSession(session.session_id, null, null);
      }
    }
    return null;
  });
}

export function resetLaunchSessionsMockState(): void {
  sessions.clear();
  seq = 0;
  nextSessionNumber = 1;
}
