import { useEffect, useMemo, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { AppSettingsData } from '../../types';
import type { InstallRootDescriptor, ProtonUpProviderDescriptor } from '../../types/protonup';
import { CollapsibleSection } from '../ui/CollapsibleSection';

interface ProtonManagerDefaultsSectionProps {
  settings: AppSettingsData;
  steamClientInstallPath: string;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
}

/** Collapsible section for configuring the native Proton download manager defaults. */
export function ProtonManagerDefaultsSection({
  settings,
  steamClientInstallPath,
  onPersistSettings,
}: ProtonManagerDefaultsSectionProps) {
  const [providers, setProviders] = useState<ProtonUpProviderDescriptor[]>([]);
  const [roots, setRoots] = useState<InstallRootDescriptor[]>([]);
  const [providersDisabled, setProvidersDisabled] = useState(false);
  const [rootsDisabled, setRootsDisabled] = useState(false);

  useEffect(() => {
    let active = true;

    void callCommand<ProtonUpProviderDescriptor[]>('protonup_list_providers')
      .then((result) => {
        if (!active) return;
        if (result.length === 0 && settings.protonup_default_provider) {
          setProviders([
            {
              id: settings.protonup_default_provider,
              display_name: settings.protonup_default_provider,
              supports_install: true,
              checksum_kind: 'none',
            },
          ]);
          setProvidersDisabled(true);
          return;
        }
        setProviders(result);
        setProvidersDisabled(false);
      })
      .catch(() => {
        if (!active) return;
        if (settings.protonup_default_provider) {
          setProviders([
            {
              id: settings.protonup_default_provider,
              display_name: settings.protonup_default_provider,
              supports_install: true,
              checksum_kind: 'none',
            },
          ]);
        } else {
          setProviders([]);
        }
        setProvidersDisabled(true);
      });

    void callCommand<InstallRootDescriptor[]>('protonup_resolve_install_roots', {
      steam_client_install_path: steamClientInstallPath.length > 0 ? steamClientInstallPath : undefined,
    })
      .then((result) => {
        if (!active) return;
        if (result.length === 0 && settings.protonup_default_install_root) {
          setRoots([
            {
              kind: 'native-steam',
              path: settings.protonup_default_install_root,
              writable: false,
              reason: 'saved-but-unavailable',
            },
          ]);
          setRootsDisabled(true);
          return;
        }
        setRoots(result);
        setRootsDisabled(false);
      })
      .catch(() => {
        if (!active) return;
        if (settings.protonup_default_install_root) {
          setRoots([
            {
              kind: 'native-steam',
              path: settings.protonup_default_install_root,
              writable: false,
              reason: 'saved-but-unavailable',
            },
          ]);
        } else {
          setRoots([]);
        }
        setRootsDisabled(true);
      });

    return () => {
      active = false;
    };
  }, [steamClientInstallPath, settings.protonup_default_provider, settings.protonup_default_install_root]);

  const installableProviders = useMemo(() => providers.filter((p) => p.supports_install), [providers]);

  return (
    <CollapsibleSection
      title="Proton manager defaults"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">Native Proton download manager</span>}
    >
      <p className="crosshook-muted crosshook-settings-help">
        These settings control the native Proton download manager. They do not affect the legacy ProtonUp-Qt advisory
        suggestions.
      </p>

      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="protonup-default-provider">
          Default provider
        </label>
        <select
          id="protonup-default-provider"
          className="crosshook-input"
          value={settings.protonup_default_provider ?? ''}
          onChange={(event) => void onPersistSettings({ protonup_default_provider: event.target.value })}
          disabled={providersDisabled || installableProviders.length === 0}
        >
          <option value="">Auto (first available)</option>
          {installableProviders.map((p) => (
            <option key={p.id} value={p.id}>
              {providersDisabled ? `${p.display_name} (saved but unavailable)` : p.display_name}
            </option>
          ))}
        </select>
        {providersDisabled && settings.protonup_default_provider ? (
          <p className="crosshook-muted crosshook-settings-note">
            Saved default provider is currently unavailable. You can clear this setting once providers are reachable.
          </p>
        ) : null}
      </div>

      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="protonup-default-install-root">
          Default install root
        </label>
        <select
          id="protonup-default-install-root"
          className="crosshook-input"
          value={settings.protonup_default_install_root ?? ''}
          onChange={(event) => void onPersistSettings({ protonup_default_install_root: event.target.value })}
          disabled={rootsDisabled || roots.length === 0}
        >
          <option value="">Auto-pick (first writable)</option>
          {roots.map((r) => (
            <option key={r.path} value={r.path} disabled={!r.writable}>
              {r.path}
              {r.writable ? '' : ' (read-only)'}
            </option>
          ))}
        </select>
        {rootsDisabled && settings.protonup_default_install_root ? (
          <p className="crosshook-muted crosshook-settings-note">
            Saved default install root is currently unavailable. You can clear this setting once install roots are
            reachable.
          </p>
        ) : null}
      </div>

      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={settings.protonup_include_prereleases ?? false}
          onChange={(event) => void onPersistSettings({ protonup_include_prereleases: event.target.checked })}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Include pre-release versions</span>
          <p className="crosshook-muted crosshook-settings-note">
            When enabled, the native Proton manager catalog will include release candidates and beta versions.
          </p>
        </span>
      </label>
    </CollapsibleSection>
  );
}
