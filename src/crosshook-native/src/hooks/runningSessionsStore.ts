import type { UnlistenFn } from '@tauri-apps/api/event';
import { useSyncExternalStore } from 'react';
import { subscribeEvent } from '@/lib/events';
import { callCommand } from '@/lib/ipc';
import { parseLaunchSessionChanged, parseRunningSessionsSnapshot } from '@/lib/launchSessionEvents';
import {
  LAUNCH_SESSION_CHANGED_EVENT,
  type LaunchSessionExitedPayload,
  type LaunchSessionStartedPayload,
  type RunningSession,
} from '@/types/launch-sessions';

/**
 * Module-level, ref-counted view of the backend launch-session registry.
 *
 * The first consumer awaits the `launch-session-changed` subscription, then
 * fetches the `list_running_sessions` snapshot. Events that arrive while a
 * snapshot is pending are buffered; once it resolves, only events with
 * `seq > snapshot.seq` are applied (earlier ones are already in the snapshot).
 * Seqs are compared against the snapshot only — never assumed contiguous.
 * `resync` (or a malformed payload) re-fetches and replaces the state. A failed
 * or malformed snapshot keeps the previous state and keeps buffering; a failed
 * subscription leaves the store unsubscribed. Both are retried by event-driven
 * triggers only: the next received event (snapshot), the window regaining focus
 * or the document becoming visible, and another consumer subscribing. The DOM
 * listeners exist only while the store has consumers. Session ids seen in an
 * `exited` event are remembered (bounded) so a launch can detect an exit that
 * arrived before it learned its own session id. The last consumer tears
 * everything down. No timers.
 */

type SessionEvent = LaunchSessionStartedPayload | LaunchSessionExitedPayload;
type Listener = () => void;

const EMPTY_SESSIONS: ReadonlyMap<string, RunningSession> = new Map();
const EMPTY_NAMES: ReadonlySet<string> = new Set();
/** Upper bound on remembered exited session ids; the oldest is evicted first. */
const MAX_EXITED_SESSION_IDS = 64;

const listeners = new Set<Listener>();
let refCount = 0;
/** Bumped on teardown so late subscription/snapshot results from a dead lifecycle are discarded. */
let lifecycle = 0;
/** Bumped per snapshot request so only the latest in-flight fetch applies. */
let fetchToken = 0;
let unlisten: UnlistenFn | null = null;
/** True while the current lifecycle's `subscribeEvent` call is in flight. */
let subscribing = false;
/** `null` while a snapshot is pending; events are buffered until it resolves. */
let baselineSeq: number | null = null;
let buffered: SessionEvent[] = [];
/** Set when the latest snapshot failed; the next recovery trigger retries it. */
let snapshotRetryPending = false;
let sessions: ReadonlyMap<string, RunningSession> = EMPTY_SESSIONS;
let gameProfileNames: ReadonlySet<string> = EMPTY_NAMES;
/** Session ids seen in `exited` events this lifecycle, oldest first (insertion order). */
const exitedSessionIds = new Set<string>();

function deriveGameProfileNames(next: ReadonlyMap<string, RunningSession>): ReadonlySet<string> {
  const names = new Set<string>();
  for (const session of next.values()) {
    if (session.session_kind === 'game' && session.profile_name !== null) {
      names.add(session.profile_name);
    }
  }
  if (names.size === gameProfileNames.size && [...names].every((name) => gameProfileNames.has(name))) {
    return gameProfileNames;
  }
  return names.size === 0 ? EMPTY_NAMES : names;
}

function notify(): void {
  for (const listener of listeners) {
    listener();
  }
}

function commit(next: ReadonlyMap<string, RunningSession>): void {
  if (next === sessions) {
    return;
  }
  sessions = next;
  gameProfileNames = deriveGameProfileNames(next);
  notify();
}

function recordExit(sessionId: string): void {
  if (exitedSessionIds.has(sessionId)) {
    return;
  }
  exitedSessionIds.add(sessionId);
  if (exitedSessionIds.size > MAX_EXITED_SESSION_IDS) {
    const oldest = exitedSessionIds.values().next().value;
    if (oldest !== undefined) {
      exitedSessionIds.delete(oldest);
    }
  }
  notify();
}

/** Applies one started/exited event to `base`; returns `base` itself when nothing changed. */
function applyEvent(
  base: ReadonlyMap<string, RunningSession>,
  event: SessionEvent
): ReadonlyMap<string, RunningSession> {
  if (event.kind === 'started') {
    if (base.has(event.session_id)) {
      return base;
    }
    const next = new Map(base);
    next.set(event.session_id, {
      session_id: event.session_id,
      profile_name: event.profile_name,
      session_kind: event.session_kind,
    });
    return next;
  }
  if (!base.has(event.session_id)) {
    return base;
  }
  const next = new Map(base);
  next.delete(event.session_id);
  return next;
}

function requestSnapshot(): void {
  const owner = lifecycle;
  const token = ++fetchToken;
  baselineSeq = null;
  snapshotRetryPending = false;
  void callCommand<unknown>('list_running_sessions')
    .then((raw) => parseRunningSessionsSnapshot(raw))
    .catch(() => null)
    .then((snapshot) => {
      if (owner !== lifecycle || token !== fetchToken) {
        return;
      }
      if (snapshot === null) {
        // Keep the previous state and keep buffering (baseline stays unset);
        // the next recovery trigger retries the snapshot.
        snapshotRetryPending = true;
        return;
      }
      let next: ReadonlyMap<string, RunningSession> = new Map(
        snapshot.sessions.map((session) => [session.session_id, session])
      );
      for (const event of buffered) {
        if (event.seq > snapshot.seq) {
          next = applyEvent(next, event);
        }
      }
      buffered = [];
      baselineSeq = snapshot.seq;
      commit(next);
    });
}

function handlePayload(payload: unknown): void {
  const parsed = parseLaunchSessionChanged(payload);
  if (parsed === null || parsed.kind === 'resync') {
    requestSnapshot();
    return;
  }
  if (parsed.kind === 'exited') {
    recordExit(parsed.session_id);
  }
  if (baselineSeq === null) {
    buffered.push(parsed);
    if (snapshotRetryPending) {
      requestSnapshot();
    }
    return;
  }
  if (parsed.seq <= baselineSeq) {
    return;
  }
  commit(applyEvent(sessions, parsed));
}

async function start(): Promise<void> {
  const owner = lifecycle;
  baselineSeq = null;
  subscribing = true;
  let unsubscribe: UnlistenFn;
  try {
    unsubscribe = await subscribeEvent<unknown>(LAUNCH_SESSION_CHANGED_EVENT, (event) => {
      if (owner === lifecycle) {
        handlePayload(event.payload);
      }
    });
  } catch {
    if (owner === lifecycle) {
      // Stay unsubscribed (`unlisten === null`) so the next recovery trigger retries.
      subscribing = false;
      buffered = [];
    }
    return;
  }
  if (owner !== lifecycle) {
    unsubscribe();
    return;
  }
  subscribing = false;
  unlisten = unsubscribe;
  requestSnapshot();
}

/** Retries a failed subscription or snapshot; a no-op while healthy or while a retry is in flight. */
function recover(): void {
  if (refCount === 0) {
    return;
  }
  if (unlisten === null) {
    if (!subscribing) {
      void start();
    }
    return;
  }
  if (snapshotRetryPending) {
    requestSnapshot();
  }
}

function handleVisibilityChange(): void {
  if (document.visibilityState === 'visible') {
    recover();
  }
}

function addRecoveryTriggers(): void {
  window.addEventListener('focus', recover);
  document.addEventListener('visibilitychange', handleVisibilityChange);
}

function removeRecoveryTriggers(): void {
  window.removeEventListener('focus', recover);
  document.removeEventListener('visibilitychange', handleVisibilityChange);
}

function stop(): void {
  lifecycle += 1;
  fetchToken += 1;
  removeRecoveryTriggers();
  unlisten?.();
  unlisten = null;
  subscribing = false;
  baselineSeq = null;
  buffered = [];
  snapshotRetryPending = false;
  exitedSessionIds.clear();
  sessions = EMPTY_SESSIONS;
  gameProfileNames = EMPTY_NAMES;
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  refCount += 1;
  if (refCount === 1) {
    addRecoveryTriggers();
    void start();
  } else {
    recover();
  }
  return () => {
    listeners.delete(listener);
    refCount -= 1;
    if (refCount === 0) {
      stop();
    }
  };
}

function getSessions(): ReadonlyMap<string, RunningSession> {
  return sessions;
}

function getGameProfileNames(): ReadonlySet<string> {
  return gameProfileNames;
}

/** All registered launch sessions (games and trainers, named and anonymous), keyed by `session_id`. */
export function useRunningSessions(): ReadonlyMap<string, RunningSession> {
  return useSyncExternalStore(subscribe, getSessions);
}

/** Profile names with a running game session; anonymous and trainer sessions excluded. */
export function useRunningGameProfileNames(): ReadonlySet<string> {
  return useSyncExternalStore(subscribe, getGameProfileNames);
}

/**
 * True once an `exited` event for `sessionId` was received in the current store
 * lifecycle, even when its `started` event was never applied. Absence from
 * `useRunningSessions()` alone is never treated as an exit.
 */
export function useSessionExited(sessionId: string | null): boolean {
  return useSyncExternalStore(subscribe, () => sessionId !== null && exitedSessionIds.has(sessionId));
}
