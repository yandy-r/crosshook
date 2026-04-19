import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react()],
  define: {
    __WEB_DEV_MODE__: true,
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  test: {
    environment: 'happy-dom',
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}', 'src/**/*.spec.{ts,tsx}'],
    exclude: ['tests/**', 'src-tauri/**', 'dist/**', 'node_modules/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'json-summary'],
      // PRD Phase 4: Critical surfaces only
      include: [
        'src/hooks/**/*.{ts,tsx}',
        'src/lib/ipc.ts',
        'src/lib/events.ts',
        'src/lib/runtime.ts',
        'src/components/pages/*.tsx',
      ],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/**/*.spec.{ts,tsx}',
        'src/test/**',
        'src/lib/mocks/**',
        // Deferred per PRD §4.6 (Phase 4 note)
        'src/hooks/useProfile.ts',
        // Hook utilities/subdirectories
        'src/hooks/install/**',
        'src/hooks/profile/**',
      ],
      thresholds: {
        lines: 60,
        functions: 60,
        branches: 60,
        statements: 60,
      },
    },
  },
});
