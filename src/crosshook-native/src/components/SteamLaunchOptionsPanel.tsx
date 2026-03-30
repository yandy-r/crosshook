import { useEffect, useId, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { LaunchOptimizationId } from '../types/launch-optimizations';
import { copyToClipboard } from '../utils/clipboard';

export interface SteamLaunchOptionsPanelProps {
  enabledOptionIds: readonly LaunchOptimizationId[];
  /** Profile `launch.custom_env_vars` — merged into the Steam launch options prefix after optimizations. */
  customEnvVars?: Readonly<Record<string, string>>;
  className?: string;
}

export function SteamLaunchOptionsPanel({
  enabledOptionIds,
  customEnvVars = {},
  className,
}: SteamLaunchOptionsPanelProps) {
  const titleId = useId();
  const [command, setCommand] = useState<string>('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [copyLabel, setCopyLabel] = useState('Copy');

  useEffect(() => {
    let cancelled = false;
    const ids = [...enabledOptionIds];

    setLoading(true);
    setError(null);

    void (async () => {
      try {
        const line = await invoke<string>('build_steam_launch_options_command', {
          enabled_option_ids: ids,
          custom_env_vars: { ...customEnvVars },
        });
        if (!cancelled) {
          setCommand(line);
          setError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setCommand('');
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [enabledOptionIds, customEnvVars]);

  async function handleCopy() {
    if (!command.trim()) {
      return;
    }

    try {
      await copyToClipboard(command);
      setCopyLabel('Copied');
      window.setTimeout(() => {
        setCopyLabel('Copy');
      }, 2000);
    } catch {
      setCopyLabel('Copy failed');
      window.setTimeout(() => {
        setCopyLabel('Copy');
      }, 2000);
    }
  }

  const rootClass = ['crosshook-panel', 'crosshook-steam-launch-options', className]
    .filter(Boolean)
    .join(' ');

  return (
    <section className={rootClass} aria-labelledby={titleId}>
      <div className="crosshook-steam-launch-options__header">
        <h2 id={titleId} className="crosshook-steam-launch-options__title">
          Steam launch options
        </h2>
        <p className="crosshook-help-text crosshook-steam-launch-options__intro">
          Paste this single line into the game&apos;s <strong>Properties → General → Launch Options</strong> in
          Steam. It matches the same Proton optimization env vars and wrappers as a direct{' '}
          <code>proton_run</code> launch, and must end with <code>%command%</code>.
        </p>
      </div>

      {error ? <div className="crosshook-error-banner crosshook-error-banner--section">{error}</div> : null}

      <div className="crosshook-steam-launch-options__row">
        <pre
          className="crosshook-steam-launch-options__preview crosshook-console__code"
          aria-busy={loading}
        >
          {loading ? 'Generating…' : error ? '—' : command}
        </pre>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => void handleCopy()}
          disabled={loading || !command.trim()}
        >
          {copyLabel}
        </button>
      </div>
    </section>
  );
}

export default SteamLaunchOptionsPanel;
