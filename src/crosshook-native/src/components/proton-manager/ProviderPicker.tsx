import type { ProtonUpProviderDescriptor } from '../../types/protonup';

interface ProviderPickerProps {
  providers: ProtonUpProviderDescriptor[];
  /** `null` means the "All" option is selected. */
  selectedProviderId: string | null;
  onSelect: (id: string | null) => void;
}

function checksumLabel(kind: ProtonUpProviderDescriptor['checksum_kind']): string {
  switch (kind) {
    case 'sha512-sidecar':
      return 'SHA-512';
    case 'sha256-manifest':
      return 'SHA-256';
    case 'none':
      return 'no checksum';
    default: {
      const _exhaustive: never = kind;
      return String(_exhaustive);
    }
  }
}

export function ProviderPicker({ providers, selectedProviderId, onSelect }: ProviderPickerProps) {
  if (providers.length === 0) {
    return null;
  }

  const isAllSelected = selectedProviderId === null;

  return (
    <fieldset className="crosshook-provider-picker">
      <legend className="crosshook-provider-picker__legend">Provider</legend>
      <div className="crosshook-provider-picker__options" role="radiogroup">
        <label className="crosshook-provider-picker__option crosshook-provider-picker__option--all">
          <input
            type="radio"
            className="crosshook-provider-picker__radio"
            name="proton-provider"
            value="__all__"
            checked={isAllSelected}
            onChange={() => onSelect(null)}
            aria-label="All providers"
          />
          <span>All</span>
        </label>

        <span className="crosshook-provider-picker__divider" aria-hidden="true" />

        {providers.map((provider) => {
          const isSelected = selectedProviderId === provider.id;
          const isCatalogOnly = !provider.supports_install;

          return (
            <label key={provider.id} className="crosshook-provider-picker__option">
              <input
                type="radio"
                className="crosshook-provider-picker__radio"
                name="proton-provider"
                value={provider.id}
                checked={isSelected}
                onChange={() => onSelect(provider.id)}
                aria-label={provider.display_name}
              />
              <span>{provider.display_name}</span>
              <span className="crosshook-provider-picker__badge">{checksumLabel(provider.checksum_kind)}</span>
              {isCatalogOnly ? (
                <span className="crosshook-provider-picker__badge crosshook-provider-picker__badge--catalog-only">
                  catalog-only
                </span>
              ) : null}
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}
