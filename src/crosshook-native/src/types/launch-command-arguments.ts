/** Profile-scoped curated command-argument IDs and custom argv tokens. */
export interface LaunchCommandArguments {
  enabled_argument_ids: string[];
  custom_args: string[];
}

/** A single command-argument catalog entry from the Rust backend. */
export interface CommandArgumentEntry {
  id: string;
  tokens: string[];
  label: string;
  description: string;
  help_text: string;
  category: string;
  advanced: boolean;
  community: boolean;
  applicable_methods: string[];
  conflicts_with: string[];
}

/** Full command-argument catalog payload returned by the Tauri IPC command. */
export interface CommandArgumentCatalogPayload {
  catalog_version: number;
  entries: CommandArgumentEntry[];
}

export const DEFAULT_LAUNCH_COMMAND_ARGUMENTS: LaunchCommandArguments = {
  enabled_argument_ids: [],
  custom_args: [],
};

export function isLaunchCommandArgumentsEmpty(args: LaunchCommandArguments): boolean {
  return args.enabled_argument_ids.length === 0 && args.custom_args.length === 0;
}
