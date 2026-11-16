import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = join(__dirname, '..', '..', '..');
const BENCH_DIR = join(SRC, 'lib', 'bench');
// Bench-only modules: the app reaches them only through the dynamic import in `useBenchReady`.
const BENCH_ONLY = /(?:^|\/)lib\/bench\/(benchScenarios|healthTable|healthScrollProbe|benchProbe)$/;
const STATIC_IMPORT = /^\s*(?:import|export)\s[^'";]*?(?:from\s*)?['"]([^'"]+)['"]/gm;
const SOURCE_FILE = /\.(?:ts|tsx)$/;

function sourceFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      return sourceFiles(path);
    }
    return SOURCE_FILE.test(entry.name) ? [path] : [];
  });
}

function staticSpecifiers(text: string): string[] {
  return [...text.matchAll(STATIC_IMPORT)].map((match) => match[1]);
}

describe('bench probe isolation', () => {
  it('finds static import specifiers the way the scan needs them', () => {
    const text = [
      "import { a } from '@/lib/bench/healthTable';",
      "import '@/lib/bench/benchProbe';",
      "export { b } from '../lib/bench/benchScenarios';",
      "import type { C } from '@/lib/bench/healthScrollProbe';",
      "const lazy = await import('@/lib/bench/benchProbe');",
    ].join('\n');
    const found = staticSpecifiers(text);
    expect(found).toEqual([
      '@/lib/bench/healthTable',
      '@/lib/bench/benchProbe',
      '../lib/bench/benchScenarios',
      '@/lib/bench/healthScrollProbe',
    ]);
    expect(found.every((specifier) => BENCH_ONLY.test(specifier))).toBe(true);
  });

  it('is never statically imported from outside src/lib/bench', () => {
    const files = sourceFiles(SRC).filter((file) => !file.startsWith(BENCH_DIR + sep));
    expect(files.length).toBeGreaterThan(0);
    const offenders = files.flatMap((file) =>
      staticSpecifiers(readFileSync(file, 'utf8'))
        .filter((specifier) => BENCH_ONLY.test(specifier))
        .map((specifier) => `${relative(SRC, file)} -> ${specifier}`)
    );
    expect(offenders).toEqual([]);
  });

  it('is loaded by the app only through the dynamic import in useBenchReady', () => {
    const text = readFileSync(join(SRC, 'hooks', 'useBenchReady.ts'), 'utf8');
    expect(text).toContain("await import('@/lib/bench/benchProbe')");
    expect(staticSpecifiers(text).filter((specifier) => specifier.includes('lib/bench'))).toEqual([]);
  });
});
