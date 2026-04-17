import type { InstallRootDescriptor, InstallRootKind } from '../../types/protonup';

interface InstallRootBadgeProps {
  root: InstallRootDescriptor;
  isDefault: boolean;
  isSelected: boolean;
  onSelect: (path: string) => void;
}

function kindLabel(kind: InstallRootKind): string {
  switch (kind) {
    case 'native-steam':
      return 'Native Steam';
    case 'flatpak-steam':
      return 'Flatpak Steam';
    default: {
      const _exhaustive: never = kind;
      return String(_exhaustive);
    }
  }
}

function truncatePath(path: string): string {
  if (path.length <= 48) return path;
  return `${path.slice(0, 20)}...${path.slice(-24)}`;
}

export function InstallRootBadge({ root, isDefault, isSelected, onSelect }: InstallRootBadgeProps) {
  const muted = !root.writable;

  const classNames = [
    'crosshook-install-root-badge',
    isSelected ? 'crosshook-install-root-badge--selected' : '',
    muted ? 'crosshook-install-root-badge--muted' : '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <button
      type="button"
      className={classNames}
      onClick={() => onSelect(root.path)}
      title={root.path}
      aria-pressed={isSelected}
    >
      <span>{kindLabel(root.kind)}</span>
      <span className="crosshook-install-root-badge__path">{truncatePath(root.path)}</span>
      <span
        className={`crosshook-install-root-badge__pill ${root.writable ? 'crosshook-install-root-badge__pill--writable' : 'crosshook-install-root-badge__pill--readonly'}`}
      >
        {root.writable ? 'writable' : 'read-only'}
      </span>
      {isDefault ? <span className="crosshook-install-root-badge__default-tag">Default</span> : null}
    </button>
  );
}
