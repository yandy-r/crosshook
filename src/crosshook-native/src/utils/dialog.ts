import { open, save } from '@/lib/plugin-stubs/dialog';
import { callCommand } from '@/lib/ipc';

function dialogFailureMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function normalizeHostDialogPath(path: string | null): string | null {
  if (path === null) {
    return null;
  }

  if (path === '/run/host' || path === '/run/host/') {
    return '/';
  }

  if (path.startsWith('/run/host/')) {
    const remainder = path.slice('/run/host/'.length);
    return `/${remainder.replace(/^\/+/, '')}`;
  }

  return path;
}

/** Matches Flatpak document-portal paths (`platform.rs` `looks_like_document_portal_path`). */
function looksLikeDocumentPortalPath(path: string): boolean {
  return path.startsWith('/run/user/') && path.includes('/doc/');
}

async function resolveDialogPath(path: string | null): Promise<string | null> {
  const normalized = normalizeHostDialogPath(path);
  if (normalized === null) {
    return null;
  }

  if (looksLikeDocumentPortalPath(normalized)) {
    return normalized;
  }

  try {
    return await callCommand<string>('normalize_host_path', { path: normalized });
  } catch {
    return normalized;
  }
}

export async function chooseFile(
  title: string,
  filters?: { name: string; extensions: string[] }[]
): Promise<string | null> {
  try {
    const result = await open({
      directory: false,
      multiple: false,
      title,
      filters,
    });

    if (Array.isArray(result)) {
      return resolveDialogPath(result[0] ?? null);
    }

    return resolveDialogPath(result ?? null);
  } catch (err) {
    console.error('chooseFile failed', err);
    window.alert(`Could not open file dialog: ${dialogFailureMessage(err)}`);
    return null;
  }
}

export async function chooseSaveFile(
  title: string,
  options?: { defaultPath?: string; filters?: { name: string; extensions: string[] }[] }
): Promise<string | null> {
  try {
    const result = await save({
      title,
      defaultPath: options?.defaultPath,
      filters: options?.filters,
    });
    return resolveDialogPath(result ?? null);
  } catch (err) {
    console.error('chooseSaveFile failed', err);
    window.alert(`Could not open save dialog: ${dialogFailureMessage(err)}`);
    return null;
  }
}

export async function chooseDirectory(title: string): Promise<string | null> {
  try {
    const result = await open({
      directory: true,
      multiple: false,
      title,
    });

    if (Array.isArray(result)) {
      return resolveDialogPath(result[0] ?? null);
    }

    return resolveDialogPath(result ?? null);
  } catch (err) {
    console.error('chooseDirectory failed', err);
    window.alert(`Could not open folder dialog: ${dialogFailureMessage(err)}`);
    return null;
  }
}
