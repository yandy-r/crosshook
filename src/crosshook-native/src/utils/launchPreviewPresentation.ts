import type { EnvVarSource, PreviewEnvVar } from '@/types/launch';

/** Returns a readable label for a launch preview method identifier. */
export function launchMethodLabel(method: string): string {
  switch (method) {
    case 'steam_applaunch':
      return 'Steam launch';
    case 'proton_run':
      return 'Proton launch';
    case 'native':
      return 'Native launch';
    default:
      return method;
  }
}

/** Returns a readable label for the source of a preview environment variable. */
export function envSourceLabel(source: EnvVarSource): string {
  switch (source) {
    case 'proton_runtime':
      return 'Proton runtime';
    case 'launch_optimization':
      return 'Launch optimization';
    case 'host':
      return 'Host';
    case 'steam_proton':
      return 'Steam + Proton';
    case 'profile_custom':
      return 'Profile custom';
  }
}

/** Groups preview environment variables by their display source label. */
export function groupPreviewEnvBySource(vars: PreviewEnvVar[]): [string, PreviewEnvVar[]][] {
  const groups = new Map<string, PreviewEnvVar[]>();
  for (const envVar of vars) {
    const label = envSourceLabel(envVar.source);
    const list = groups.get(label);
    if (list) {
      list.push(envVar);
    } else {
      groups.set(label, [envVar]);
    }
  }
  return Array.from(groups.entries());
}
