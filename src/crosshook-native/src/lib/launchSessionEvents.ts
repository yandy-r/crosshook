import {
  LAUNCH_SESSION_KINDS,
  type LaunchSessionChangedPayload,
  type LaunchSessionExitedPayload,
  type LaunchSessionKind,
  type RunningSession,
  type RunningSessionsSnapshot,
} from '../types/launch-sessions';

function isSeq(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isCodeOrSignal(value: unknown): value is number | null {
  return value === null || (typeof value === 'number' && Number.isInteger(value));
}

function parseRunningSession(value: unknown): RunningSession | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }
  const { session_id, profile_name, session_kind } = value as Partial<RunningSession>;
  if (typeof session_id !== 'string' || session_id.length === 0) {
    return null;
  }
  if (profile_name !== null && typeof profile_name !== 'string') {
    return null;
  }
  if (!LAUNCH_SESSION_KINDS.includes(session_kind as LaunchSessionKind)) {
    return null;
  }
  return { session_id, profile_name, session_kind: session_kind as LaunchSessionKind };
}

/**
 * Validates a `launch-session-changed` payload at the event boundary. Returns
 * `null` for anything malformed; callers treat `null` as a resync (re-fetch the
 * snapshot) rather than guessing at partial data.
 */
export function parseLaunchSessionChanged(payload: unknown): LaunchSessionChangedPayload | null {
  if (typeof payload !== 'object' || payload === null) {
    return null;
  }
  const candidate = payload as Partial<Omit<LaunchSessionExitedPayload, 'kind'>> & { kind?: unknown };
  if (candidate.kind === 'resync') {
    return { kind: 'resync' };
  }
  if (candidate.kind !== 'started' && candidate.kind !== 'exited') {
    return null;
  }
  const session = parseRunningSession(payload);
  const { seq } = candidate;
  if (session === null || !isSeq(seq)) {
    return null;
  }
  const { kind } = candidate;
  if (kind === 'started') {
    return { kind, ...session, seq };
  }
  const { code, signal } = candidate;
  if (!isCodeOrSignal(code) || !isCodeOrSignal(signal)) {
    return null;
  }
  return { kind: 'exited', ...session, code, signal, seq };
}

/** Validates the `list_running_sessions` response; `null` for anything malformed. */
export function parseRunningSessionsSnapshot(value: unknown): RunningSessionsSnapshot | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }
  const { sessions: rawSessions, seq } = value as Partial<RunningSessionsSnapshot>;
  if (!isSeq(seq) || !Array.isArray(rawSessions)) {
    return null;
  }
  const sessions: RunningSession[] = [];
  for (const entry of rawSessions) {
    const session = parseRunningSession(entry);
    if (session === null) {
      return null;
    }
    sessions.push(session);
  }
  return { sessions, seq };
}
