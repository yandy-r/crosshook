import { callCommand } from '@/lib/ipc';
import { open, save } from '@/lib/plugin-stubs/dialog';

/**
 * How a picked path will be consumed. The backend resolves it accordingly:
 * - `host`: the path is handed to a host-executed process or the host desktop (game/trainer
 *   executables, Proton, prefixes, launcher icon). Flatpak document-portal paths are resolved to
 *   their real host location; unresolvable ones are rejected.
 * - `sandbox`: the path is read or written by CrossHook itself inside the Flatpak sandbox
 *   (imports, exports, settings directories). Document-portal paths are kept as-is.
 */
export type PickedPathUse = 'host' | 'sandbox';

type DialogFilter = { name: string; extensions: string[] };

function dialogFailureMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function firstSelection(result: string | string[] | null | undefined): string | null {
  if (Array.isArray(result)) {
    return result[0] ?? null;
  }
  return result ?? null;
}

/** Sends the picked path to `normalize_host_path`. On failure, alerts the user and returns `null`. */
async function resolvePickedPath(path: string | null, use: PickedPathUse): Promise<string | null> {
  if (!path) {
    return null;
  }

  try {
    return await callCommand<string>('normalize_host_path', { path, pathUse: use });
  } catch (err) {
    console.error('normalize_host_path failed', err);
    window.alert(`Could not use the selected path: ${dialogFailureMessage(err)}`);
    return null;
  }
}

export async function chooseFile(title: string, use: PickedPathUse, filters?: DialogFilter[]): Promise<string | null> {
  let selection: string | null;
  try {
    selection = firstSelection(
      await open({
        directory: false,
        multiple: false,
        title,
        filters,
      })
    );
  } catch (err) {
    console.error('chooseFile failed', err);
    window.alert(`Could not open file dialog: ${dialogFailureMessage(err)}`);
    return null;
  }

  return resolvePickedPath(selection, use);
}

export async function chooseSaveFile(
  title: string,
  use: PickedPathUse,
  options?: { defaultPath?: string; filters?: DialogFilter[] }
): Promise<string | null> {
  let selection: string | null;
  try {
    selection = firstSelection(
      await save({
        title,
        defaultPath: options?.defaultPath,
        filters: options?.filters,
      })
    );
  } catch (err) {
    console.error('chooseSaveFile failed', err);
    window.alert(`Could not open save dialog: ${dialogFailureMessage(err)}`);
    return null;
  }

  return resolvePickedPath(selection, use);
}

export async function chooseDirectory(title: string, use: PickedPathUse): Promise<string | null> {
  let selection: string | null;
  try {
    selection = firstSelection(
      await open({
        directory: true,
        multiple: false,
        title,
      })
    );
  } catch (err) {
    console.error('chooseDirectory failed', err);
    window.alert(`Could not open folder dialog: ${dialogFailureMessage(err)}`);
    return null;
  }

  return resolvePickedPath(selection, use);
}
