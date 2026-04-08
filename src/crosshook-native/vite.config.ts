import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const host = process.env.TAURI_DEV_HOST;
const isDebug = !!process.env.TAURI_ENV_DEBUG;

export default defineConfig(({ mode }) => ({
  plugins: [react()],
  clearScreen: false,
  define: {
    __WEB_DEV_MODE__: mode === 'webdev',
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server:
    mode === 'webdev'
      ? {
          port: 5173,
          strictPort: true,
          // security: webdev mode binds loopback only
          host: '127.0.0.1',
          watch: {
            ignored: ['**/src-tauri/**'],
          },
        }
      : {
          port: 5173,
          strictPort: true,
          host: host || false,
          hmr: host
            ? {
                protocol: 'ws',
                host,
                port: 1421,
              }
            : undefined,
          watch: {
            ignored: ['**/src-tauri/**'],
          },
        },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: isDebug ? false : 'oxc',
    sourcemap: isDebug,
  },
}));
