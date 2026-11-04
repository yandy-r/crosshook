import { useCallback, useEffect, useState } from 'react';
import { fetchStartupReport } from '@/lib/startupReport';

// sessionStorage: resets on window restart so the banner re-surfaces until dismissed again.
export const INSTANCE_UNGUARDED_DISMISS_KEY = 'crosshook.instance.unguarded.dismissed';

function isDismissed(): boolean {
  try {
    return sessionStorage.getItem(INSTANCE_UNGUARDED_DISMISS_KEY) === '1';
  } catch {
    // sessionStorage unavailable — show the banner anyway.
    return false;
  }
}

/** App-level, session-dismissible banner shown when no single-instance guard protects this process. */
export function InstanceGuardBanner() {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    let active = true;
    void fetchStartupReport()
      .then((report) => {
        if (!active) return;
        if (report.degraded.instance_unguarded === true && !isDismissed()) {
          setVisible(true);
        }
      })
      .catch(() => {
        // Startup report unavailable — no instance warning.
      });

    return () => {
      active = false;
    };
  }, []);

  const dismiss = useCallback(() => {
    setVisible(false);
    try {
      sessionStorage.setItem(INSTANCE_UNGUARDED_DISMISS_KEY, '1');
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, []);

  if (!visible) return null;

  return (
    <div
      className="crosshook-warning-banner crosshook-instance-guard-banner"
      role="status"
      aria-label="Single-instance warning"
    >
      <span>
        CrossHook could not confirm it is the only running copy (no runtime directory or session bus). Avoid opening a
        second CrossHook window.
      </span>
      <button
        type="button"
        className="crosshook-rename-toast-dismiss"
        aria-label="Dismiss single-instance warning"
        onClick={dismiss}
      >
        ×
      </button>
    </div>
  );
}

export default InstanceGuardBanner;
