import type { AppRoute } from '@/components/layout/Sidebar';

export type CommandPaletteCommandId = string;

export type CommandPaletteIconId =
  | 'browse'
  | 'compatibility'
  | 'discover'
  | 'health'
  | 'host_tools'
  | 'install'
  | 'launch'
  | 'library'
  | 'profiles'
  | 'proton_manager'
  | 'settings';

export type CommandPaletteAction = 'route' | 'launch_profile' | 'edit_profile';

export interface CommandPaletteCommandBase {
  readonly id: CommandPaletteCommandId;
  readonly title: string;
  readonly subtitle?: string;
  readonly keywords?: readonly string[];
  readonly icon: CommandPaletteIconId;
  readonly hint?: string;
  readonly disabled?: boolean;
}

interface RouteCommand extends CommandPaletteCommandBase {
  readonly action: 'route';
  readonly route: AppRoute;
}

interface ProfileCommand extends CommandPaletteCommandBase {
  readonly action: 'launch_profile' | 'edit_profile';
  readonly profileName: string;
}

export type CommandPaletteCommand = RouteCommand | ProfileCommand;

export interface CommandPaletteCommandSection {
  readonly route: readonly CommandPaletteCommand[];
}

export function isCommandPaletteCommandEnabled(command: CommandPaletteCommand): boolean {
  return command.disabled !== true;
}

function commandSearchHaystack(command: CommandPaletteCommand): string {
  return [command.title, command.subtitle ?? '', ...(command.keywords ?? [])].filter(Boolean).join(' ').toLowerCase();
}

export function filterCommandPaletteCommands(
  commands: readonly CommandPaletteCommand[],
  query: string
): CommandPaletteCommand[] {
  const normalizedQuery = query.trim().toLowerCase();
  if (!normalizedQuery) {
    return [...commands];
  }

  return commands.filter((command) => commandSearchHaystack(command).includes(normalizedQuery));
}

const ROUTE_TITLES: Record<AppRoute, string> = {
  library: 'Go to Library',
  profiles: 'Go to Profiles',
  launch: 'Go to Launch',
  install: 'Go to Install & Run',
  community: 'Go to Browse',
  discover: 'Go to Discover',
  compatibility: 'Go to Compatibility',
  settings: 'Go to Settings',
  health: 'Go to Health',
  'host-tools': 'Go to Host Tools',
  'proton-manager': 'Go to Proton Manager',
};

const ROUTE_KEYWORDS: Record<AppRoute, readonly string[]> = {
  library: ['games', 'browse'],
  profiles: ['editor', 'profile'],
  launch: ['run', 'play'],
  install: ['setup', 'exe', 'msi'],
  community: ['community', 'taps'],
  discover: ['search', 'trainers'],
  compatibility: ['compatibility', 'proton'],
  settings: ['preferences', 'configuration'],
  health: ['diagnostics', 'dashboard'],
  'host-tools': ['dependencies', 'readiness'],
  'proton-manager': ['proton', 'compatibility tool'],
};

const ROUTE_ICON: Record<AppRoute, CommandPaletteIconId> = {
  library: 'library',
  profiles: 'profiles',
  launch: 'launch',
  install: 'install',
  community: 'browse',
  discover: 'discover',
  compatibility: 'compatibility',
  settings: 'settings',
  health: 'health',
  'host-tools': 'host_tools',
  'proton-manager': 'proton_manager',
};

const ROUTE_SUBTITLE: Record<AppRoute, string> = {
  library: 'Browse your saved profiles, favorites, and launch shortcuts in one place.',
  profiles: 'Create, select, and maintain profiles for each game and trainer setup.',
  launch: 'Run the game or trainer with the active profile’s launch configuration.',
  install: 'Install games, apply updates, or run an arbitrary Windows EXE or MSI without committing it.',
  community: 'Search shared profiles from your taps and import them into your library.',
  discover: 'Search community trainer sources linked from CrossHook (external sites only).',
  compatibility: 'Inspect trainer compatibility data and manage Proton installs.',
  settings: 'Startup behavior, storage paths, integrations, and recent history.',
  health: 'Validate profiles, review issues, and track launch health across your library.',
  'host-tools': 'Inspect detected host tools, capability state, and install guidance.',
  'proton-manager': 'Download, install, and manage Proton versions for your Steam library.',
};

export const ROUTE_COMMANDS: readonly CommandPaletteCommand[] = (
  Object.entries(ROUTE_TITLES) as Array<[AppRoute, string]>
).map(([route, title]) => ({
  id: `route:${route}`,
  action: 'route',
  route,
  title,
  subtitle: ROUTE_SUBTITLE[route],
  keywords: ROUTE_KEYWORDS[route],
  icon: ROUTE_ICON[route],
}));

export function createProfileCommands(activeProfileName: string): readonly CommandPaletteCommand[] {
  const trimmed = activeProfileName.trim();
  if (!trimmed) {
    return [];
  }

  return [
    {
      id: `profile:launch-current:${trimmed}`,
      action: 'launch_profile',
      profileName: trimmed,
      title: `Launch ${trimmed}`,
      subtitle: 'Load the selected profile and switch to Launch.',
      keywords: ['run', 'active profile'],
      icon: 'launch',
      hint: 'Active profile',
    },
    {
      id: `profile:edit-current:${trimmed}`,
      action: 'edit_profile',
      profileName: trimmed,
      title: `Edit ${trimmed}`,
      subtitle: 'Load the selected profile in the Profiles editor.',
      keywords: ['profiles', 'active profile'],
      icon: 'profiles',
      hint: 'Active profile',
    },
  ];
}
