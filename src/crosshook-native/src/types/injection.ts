export const INJECTION_LOG_LEVELS = ['info', 'warning', 'error'] as const;
export type InjectionLogLevel = (typeof INJECTION_LOG_LEVELS)[number];

export const INJECTION_LOG_SOURCES = ['trainer', 'injection', 'runtime'] as const;
export type InjectionLogSource = (typeof INJECTION_LOG_SOURCES)[number];

export const INJECTION_LOG_SESSION_KINDS = ['game', 'trainer'] as const;
export type InjectionLogSessionKind = (typeof INJECTION_LOG_SESSION_KINDS)[number];

export interface InjectionLogEvent {
  timestamp: string;
  profile_name: string;
  session_id: string;
  session_kind: InjectionLogSessionKind;
  level: InjectionLogLevel;
  source: InjectionLogSource;
  message: string;
  hook_id?: string;
  hook_name?: string;
  unsupported_runtime?: boolean;
}

function optionalString(value: unknown): value is string | undefined {
  return value === undefined || typeof value === 'string';
}

export function isInjectionLogEvent(value: unknown): value is InjectionLogEvent {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Partial<InjectionLogEvent>;

  return (
    typeof candidate.timestamp === 'string' &&
    typeof candidate.profile_name === 'string' &&
    typeof candidate.session_id === 'string' &&
    INJECTION_LOG_SESSION_KINDS.includes(candidate.session_kind as InjectionLogSessionKind) &&
    INJECTION_LOG_LEVELS.includes(candidate.level as InjectionLogLevel) &&
    INJECTION_LOG_SOURCES.includes(candidate.source as InjectionLogSource) &&
    typeof candidate.message === 'string' &&
    optionalString(candidate.hook_id) &&
    optionalString(candidate.hook_name) &&
    (candidate.unsupported_runtime === undefined || typeof candidate.unsupported_runtime === 'boolean')
  );
}
