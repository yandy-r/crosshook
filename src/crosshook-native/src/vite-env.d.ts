interface ImportMetaEnv {
  readonly DEV: boolean;
  readonly PROD: boolean;
  readonly MODE: string;
  readonly BASE_URL: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

declare const __WEB_DEV_MODE__: boolean;

/** Browser dev + Playwright: optional hook for E2E to invoke mock IPC. Stripped in production. */
interface CrosshookDev {
  callCommand: (name: string, args?: import('@tauri-apps/api/core').InvokeArgs) => Promise<unknown>;
}

interface Window {
  __CROSSHOOK_DEV__?: CrosshookDev;
}
