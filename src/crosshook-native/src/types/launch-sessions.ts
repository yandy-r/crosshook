/**
 * Launch-session registry wire types. Mirrors the Tauri `launch-session-changed`
 * event and the `list_running_sessions` snapshot command (snake_case DTOs).
 * `profile_name: null` marks an anonymous (unsaved-profile) session.
 */
export const LAUNCH_SESSION_CHANGED_EVENT = 'launch-session-changed';

export const LAUNCH_SESSION_KINDS = ['game', 'trainer'] as const;
export type LaunchSessionKind = (typeof LAUNCH_SESSION_KINDS)[number];

export interface RunningSession {
  session_id: string;
  profile_name: string | null;
  session_kind: LaunchSessionKind;
}

export interface LaunchSessionStartedPayload extends RunningSession {
  kind: 'started';
  seq: number;
}

export interface LaunchSessionExitedPayload extends RunningSession {
  kind: 'exited';
  code: number | null;
  signal: number | null;
  seq: number;
}

/** Bus lag or bridge restart: drop local state and re-fetch the snapshot. */
export interface LaunchSessionResyncPayload {
  kind: 'resync';
}

export type LaunchSessionChangedPayload =
  | LaunchSessionStartedPayload
  | LaunchSessionExitedPayload
  | LaunchSessionResyncPayload;

/** Response of `list_running_sessions`; `seq` is read under the same lock as `sessions`. */
export interface RunningSessionsSnapshot {
  sessions: RunningSession[];
  seq: number;
}
