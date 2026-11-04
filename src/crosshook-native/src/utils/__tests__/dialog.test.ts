import { afterEach, beforeEach, describe, expect, it, type MockInstance, vi } from 'vitest';
import { callCommand } from '@/lib/ipc';
import { open, save } from '@/lib/plugin-stubs/dialog';
import { chooseDirectory, chooseFile, chooseSaveFile, type PickedPathUse } from '../dialog';

vi.mock('@/lib/ipc', () => ({
  callCommand: vi.fn(),
}));

vi.mock('@/lib/plugin-stubs/dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

const callCommandMock = vi.mocked(callCommand);
const openMock = vi.mocked(open);
const saveMock = vi.mocked(save);

const DOC_PORTAL_PATH = '/run/user/1000/doc/abc123/game.exe';
const DOC_PORTAL_DIR = '/run/user/1000/doc/def456/Prefix';
const DOC_PORTAL_SAVE = '/run/user/1000/doc/aaa/export.json';

type Picker = 'file' | 'directory' | 'save';

function pick(picker: Picker, use: PickedPathUse): Promise<string | null> {
  switch (picker) {
    case 'file':
      return chooseFile('Pick', use);
    case 'directory':
      return chooseDirectory('Pick', use);
    case 'save':
      return chooseSaveFile('Save', use, { defaultPath: 'export.json' });
  }
}

describe('dialog pickers', () => {
  // happy-dom does not implement window.alert, so it is stubbed as a global.
  const alertMock = vi.fn<(message?: string) => void>();
  let consoleErrorSpy: MockInstance<typeof console.error>;

  beforeEach(() => {
    callCommandMock.mockReset();
    openMock.mockReset();
    saveMock.mockReset();
    alertMock.mockReset();
    vi.stubGlobal('alert', alertMock);
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    consoleErrorSpy.mockRestore();
  });

  // The backend owns resolution: the client forwards the raw pick and returns whatever the
  // backend answers. The mocked answers model D5 — host resolves portal paths to real host
  // paths, sandbox keeps portal paths as-is, and both repair `/run/host` prefixes.
  it.each<{ picker: Picker; use: PickedPathUse; picked: string; resolved: string }>([
    { picker: 'file', use: 'host', picked: DOC_PORTAL_PATH, resolved: '/home/user/Games/game.exe' },
    { picker: 'file', use: 'sandbox', picked: DOC_PORTAL_PATH, resolved: DOC_PORTAL_PATH },
    { picker: 'file', use: 'host', picked: '/run/host/usr/bin/protontricks', resolved: '/usr/bin/protontricks' },
    { picker: 'file', use: 'sandbox', picked: '/run/host/usr/bin/protontricks', resolved: '/usr/bin/protontricks' },
    { picker: 'directory', use: 'host', picked: DOC_PORTAL_DIR, resolved: '/srv/Prefix' },
    { picker: 'directory', use: 'sandbox', picked: DOC_PORTAL_DIR, resolved: DOC_PORTAL_DIR },
    { picker: 'save', use: 'host', picked: DOC_PORTAL_SAVE, resolved: '/home/user/export.json' },
    { picker: 'save', use: 'sandbox', picked: DOC_PORTAL_SAVE, resolved: DOC_PORTAL_SAVE },
  ])('$picker picker forwards $picked verbatim with pathUse $use', async ({ picker, use, picked, resolved }) => {
    if (picker === 'save') {
      saveMock.mockResolvedValue(picked);
    } else {
      openMock.mockResolvedValue(picked);
    }
    callCommandMock.mockResolvedValue(resolved);

    await expect(pick(picker, use)).resolves.toBe(resolved);
    if (picker !== 'save') {
      expect(openMock).toHaveBeenCalledWith(expect.objectContaining({ directory: picker === 'directory' }));
    }
    expect(callCommandMock).toHaveBeenCalledTimes(1);
    expect(callCommandMock).toHaveBeenCalledWith('normalize_host_path', { path: picked, pathUse: use });
    expect(alertMock).not.toHaveBeenCalled();
  });

  it('returns null and alerts without calling the backend when the file dialog fails to open', async () => {
    openMock.mockRejectedValue(new Error('portal unavailable'));

    await expect(chooseFile('Pick', 'host')).resolves.toBeNull();
    expect(alertMock).toHaveBeenCalledWith('Could not open file dialog: portal unavailable');
    expect(callCommandMock).not.toHaveBeenCalled();
  });

  it('returns null without calling the backend when the file dialog is cancelled', async () => {
    openMock.mockResolvedValue(null);

    await expect(chooseFile('Pick', 'host')).resolves.toBeNull();
    expect(callCommandMock).not.toHaveBeenCalled();
    expect(alertMock).not.toHaveBeenCalled();
  });

  it('returns null without calling the backend when the directory or save dialog is cancelled', async () => {
    openMock.mockResolvedValue([]);
    saveMock.mockResolvedValue(null);

    await expect(chooseDirectory('Pick', 'sandbox')).resolves.toBeNull();
    await expect(chooseSaveFile('Save', 'sandbox')).resolves.toBeNull();
    expect(callCommandMock).not.toHaveBeenCalled();
  });

  it('returns null and alerts with the backend message when resolution fails', async () => {
    openMock.mockResolvedValue(DOC_PORTAL_PATH);
    callCommandMock.mockRejectedValue(new Error('unresolved document portal path'));

    await expect(chooseFile('Pick', 'host')).resolves.toBeNull();
    expect(alertMock).toHaveBeenCalledWith('Could not use the selected path: unresolved document portal path');
    expect(consoleErrorSpy).toHaveBeenCalled();
  });

  it('alerts with a string rejection from the backend', async () => {
    openMock.mockResolvedValue('/some/dir');
    callCommandMock.mockRejectedValue('portal timed out');

    await expect(chooseDirectory('Pick', 'host')).resolves.toBeNull();
    expect(alertMock).toHaveBeenCalledWith('Could not use the selected path: portal timed out');
  });

  it('uses the first element when the dialog returns an array', async () => {
    openMock.mockResolvedValue(['/first/game.exe', '/second/game.exe']);
    callCommandMock.mockResolvedValue('/first/game.exe');

    await expect(chooseFile('Pick', 'host')).resolves.toBe('/first/game.exe');
    expect(callCommandMock).toHaveBeenCalledTimes(1);
    expect(callCommandMock).toHaveBeenCalledWith('normalize_host_path', { path: '/first/game.exe', pathUse: 'host' });
  });
});
