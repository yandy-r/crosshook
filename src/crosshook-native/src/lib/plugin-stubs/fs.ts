import type {
  CopyFileOptions,
  CreateOptions,
  DirEntry,
  ExistsOptions,
  FileHandle,
  FileInfo,
  MkdirOptions,
  OpenOptions,
  ReadDirOptions,
  ReadFileOptions,
  RemoveOptions,
  RenameOptions,
  StatOptions,
  TruncateOptions,
  WriteFileOptions,
} from '@tauri-apps/plugin-fs';
import { isTauri } from '../runtime';

export type {
  CopyFileOptions,
  CreateOptions,
  DirEntry,
  ExistsOptions,
  FileHandle,
  FileInfo,
  MkdirOptions,
  OpenOptions,
  ReadDirOptions,
  ReadFileOptions,
  RemoveOptions,
  RenameOptions,
  StatOptions,
  TruncateOptions,
  WriteFileOptions,
};

// Minimal synthetic FileInfo returned by stat/lstat in browser mode.
const STUB_FILE_INFO: FileInfo = {
  isFile: false,
  isDirectory: false,
  isSymlink: false,
  size: 0,
  mtime: null,
  atime: null,
  birthtime: null,
  readonly: true,
  fileAttributes: null,
  dev: null,
  ino: null,
  mode: null,
  nlink: null,
  uid: null,
  gid: null,
  rdev: null,
  blksize: null,
  blocks: null,
};

// ---------------------------------------------------------------------------
// Read operations — return synthetic stubs with a [plugin-stub] warn
// ---------------------------------------------------------------------------

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode returns an empty Uint8Array and warns.
 */
export async function readFile(path: string | URL, options?: ReadFileOptions): Promise<Uint8Array<ArrayBuffer>> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.readFile(path, options);
  }
  console.warn('[plugin-stub] fs.readFile returning empty Uint8Array for:', path);
  return new Uint8Array(0) as Uint8Array<ArrayBuffer>;
}

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode returns an empty string and warns.
 */
export async function readTextFile(path: string | URL, options?: ReadFileOptions): Promise<string> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.readTextFile(path, options);
  }
  console.warn('[plugin-stub] fs.readTextFile returning empty string for:', path);
  return '';
}

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode returns an empty array and warns.
 */
export async function readDir(path: string | URL, options?: ReadDirOptions): Promise<DirEntry[]> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.readDir(path, options);
  }
  console.warn('[plugin-stub] fs.readDir returning empty array for:', path);
  return [];
}

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode returns false and warns.
 */
export async function exists(path: string | URL, options?: ExistsOptions): Promise<boolean> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.exists(path, options);
  }
  console.warn('[plugin-stub] fs.exists returning false for:', path);
  return false;
}

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode returns a minimal synthetic FileInfo and warns.
 */
export async function stat(path: string | URL, options?: StatOptions): Promise<FileInfo> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.stat(path, options);
  }
  console.warn('[plugin-stub] fs.stat returning stub FileInfo for:', path);
  return { ...STUB_FILE_INFO };
}

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode returns a minimal synthetic FileInfo and warns.
 */
export async function lstat(path: string | URL, options?: StatOptions): Promise<FileInfo> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.lstat(path, options);
  }
  console.warn('[plugin-stub] fs.lstat returning stub FileInfo for:', path);
  return { ...STUB_FILE_INFO };
}

// ---------------------------------------------------------------------------
// Write / destroy operations — throw per D4 to surface bugs in Phase 2 flows
// ---------------------------------------------------------------------------

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode throws — silent no-ops would mask install/update flow bugs.
 */
export async function writeFile(
  path: string | URL,
  data: Uint8Array | ReadableStream<Uint8Array>,
  options?: WriteFileOptions
): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.writeFile(path, data, options);
  }
  throw new Error('[plugin-stub] fs.writeFile is not available in browser dev mode');
}

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode throws — silent no-ops would mask install/update flow bugs.
 */
export async function writeTextFile(path: string | URL, data: string, options?: WriteFileOptions): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.writeTextFile(path, data, options);
  }
  throw new Error('[plugin-stub] fs.writeTextFile is not available in browser dev mode');
}

/**
 * Tauri v2 unified remove (replaces removeFile / removeDir).
 * In Tauri mode delegates to the real plugin.
 * In browser mode throws.
 */
export async function remove(path: string | URL, options?: RemoveOptions): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.remove(path, options);
  }
  throw new Error('[plugin-stub] fs.remove is not available in browser dev mode');
}

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode throws.
 */
export async function rename(oldPath: string | URL, newPath: string | URL, options?: RenameOptions): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.rename(oldPath, newPath, options);
  }
  throw new Error('[plugin-stub] fs.rename is not available in browser dev mode');
}

/**
 * Tauri v2 mkdir (replaces createDir).
 * In Tauri mode delegates to the real plugin.
 * In browser mode throws.
 */
export async function mkdir(path: string | URL, options?: MkdirOptions): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.mkdir(path, options);
  }
  throw new Error('[plugin-stub] fs.mkdir is not available in browser dev mode');
}

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode throws.
 */
export async function copyFile(fromPath: string | URL, toPath: string | URL, options?: CopyFileOptions): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.copyFile(fromPath, toPath, options);
  }
  throw new Error('[plugin-stub] fs.copyFile is not available in browser dev mode');
}

/**
 * In Tauri mode delegates to the real plugin.
 * In browser mode throws.
 */
export async function truncate(path: string | URL, len?: number, options?: TruncateOptions): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.truncate(path, len, options);
  }
  throw new Error('[plugin-stub] fs.truncate is not available in browser dev mode');
}

/**
 * open() returns a writable FileHandle — treat as a write operation.
 * In Tauri mode delegates to the real plugin.
 * In browser mode throws.
 */
export async function open(path: string | URL, options?: OpenOptions): Promise<FileHandle> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.open(path, options);
  }
  throw new Error('[plugin-stub] fs.open is not available in browser dev mode');
}

/**
 * create() returns a writable FileHandle — treat as a write operation.
 * In Tauri mode delegates to the real plugin.
 * In browser mode throws.
 */
export async function create(path: string | URL, options?: CreateOptions): Promise<FileHandle> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.create(path, options);
  }
  throw new Error('[plugin-stub] fs.create is not available in browser dev mode');
}
