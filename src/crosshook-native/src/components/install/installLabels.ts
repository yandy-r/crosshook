import type { InstallGamePrefixPathState } from '../../types/install';

export function prefixStateLabel(state: InstallGamePrefixPathState): string {
  switch (state) {
    case 'loading':
      return 'Resolving default prefix...';
    case 'ready':
      return 'Default prefix resolved';
    case 'failed':
      return 'Default prefix unavailable';
    case 'idle':
    default:
      return 'Awaiting profile name';
  }
}
