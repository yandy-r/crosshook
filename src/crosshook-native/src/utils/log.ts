/**
 * Payload shape emitted by backend log events (`launch-log`, `update-log`).
 * The backend currently emits plain strings, but this type defensively handles
 * alternate object shapes in case the payload format changes.
 */
export type LogPayload = string | { line: string } | { message: string } | { text: string };

/**
 * Extract a displayable message from a log event payload (`launch-log`, `update-log`).
 * Falls back to `JSON.stringify` for unrecognized object shapes so that
 * unexpected payloads are visible rather than silently dropped.
 */
export function normalizeLogMessage(payload: unknown): string {
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

  return JSON.stringify(payload);
}
