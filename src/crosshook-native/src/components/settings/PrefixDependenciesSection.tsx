import { useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { AppSettingsData } from '../../types';
import { chooseFile } from '../../utils/dialog';
import { CollapsibleSection } from '../ui/CollapsibleSection';

interface PrefixDependenciesSectionProps {
  settings: AppSettingsData;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
}

/** Collapsible section for configuring the winetricks/protontricks binary path. */
export function PrefixDependenciesSection({ settings, onPersistSettings }: PrefixDependenciesSectionProps) {
  const [binaryDetection, setBinaryDetection] = useState<{
    found: boolean;
    binary_name: string;
    source: string;
  } | null>(null);

  useEffect(() => {
    let active = true;
    try {
      void callCommand<{ found: boolean; binary_name: string; source: string }>('detect_protontricks_binary')
        .then((result) => {
          if (active) setBinaryDetection(result);
        })
        .catch(() => {
          if (active) setBinaryDetection(null);
        });
    } catch {
      if (active) setBinaryDetection(null);
    }
    return () => {
      active = false;
    };
  }, []);

  return (
    <CollapsibleSection
      title="Prefix Dependencies"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">Winetricks / Protontricks</span>}
    >
      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="protontricks-binary-path">
          Winetricks/Protontricks Binary Path
        </label>
        <div className="crosshook-settings-input-row">
          <input
            id="protontricks-binary-path"
            key={`ptbp-${settings.protontricks_binary_path}`}
            className="crosshook-input"
            defaultValue={settings.protontricks_binary_path}
            placeholder="/usr/bin/protontricks"
            onBlur={(event) => {
              const v = event.target.value.trim();
              if (v !== settings.protontricks_binary_path.trim()) {
                void onPersistSettings({ protontricks_binary_path: v });
              }
            }}
          />
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => {
              void (async () => {
                const path = await chooseFile('Select winetricks or protontricks binary');
                if (path) void onPersistSettings({ protontricks_binary_path: path });
              })();
            }}
          >
            Browse…
          </button>
        </div>
      </div>
      <p className="crosshook-muted crosshook-settings-note">
        If left empty, CrossHook will auto-detect winetricks/protontricks from PATH.
      </p>
      {binaryDetection ? (
        <p
          className={binaryDetection.found ? 'crosshook-success' : 'crosshook-warning'}
          style={{ fontSize: '0.85rem', margin: '4px 0 0' }}
          aria-live="polite"
        >
          {binaryDetection.found
            ? `Binary found: ${binaryDetection.binary_name} (source: ${binaryDetection.source})`
            : 'No winetricks or protontricks binary found'}
        </p>
      ) : null}

      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={settings.auto_install_prefix_deps}
          onChange={(event) => void onPersistSettings({ auto_install_prefix_deps: event.target.checked })}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Auto-install prefix dependencies on first launch</span>
          <p className="crosshook-muted crosshook-settings-note">
            When enabled, CrossHook will automatically install any required winetricks/protontricks dependencies into
            the Wine prefix before launching for the first time.
          </p>
        </span>
      </label>
    </CollapsibleSection>
  );
}
