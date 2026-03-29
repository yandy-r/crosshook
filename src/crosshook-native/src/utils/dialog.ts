import { open, save } from '@tauri-apps/plugin-dialog';

function dialogFailureMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export async function chooseFile(
  title: string,
  filters?: { name: string; extensions: string[] }[],
): Promise<string | null> {
  try {
    const result = await open({
      directory: false,
      multiple: false,
      title,
      filters,
    });

    if (Array.isArray(result)) {
      return result[0] ?? null;
    }

    return result ?? null;
  } catch (err) {
    console.error('chooseFile failed', err);
    window.alert(`Could not open file dialog: ${dialogFailureMessage(err)}`);
    return null;
  }
}

export async function chooseSaveFile(
  title: string,
  options?: { defaultPath?: string; filters?: { name: string; extensions: string[] }[] },
): Promise<string | null> {
  try {
    const result = await save({
      title,
      defaultPath: options?.defaultPath,
      filters: options?.filters,
    });
    return result ?? null;
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
      return result[0] ?? null;
    }

    return result ?? null;
  } catch (err) {
    console.error('chooseDirectory failed', err);
    window.alert(`Could not open folder dialog: ${dialogFailureMessage(err)}`);
    return null;
  }
}
