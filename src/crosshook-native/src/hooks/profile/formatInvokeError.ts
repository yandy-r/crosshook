/** Tauri invoke failures are sometimes plain objects, not Error instances. */
export function formatInvokeError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === 'string') {
    return err;
  }
  if (err && typeof err === 'object') {
    const message = (err as { message?: unknown }).message;
    if (typeof message === 'string' && message.length > 0) {
      return message;
    }
  }
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}
