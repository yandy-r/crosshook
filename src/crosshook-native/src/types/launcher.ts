export interface LauncherInfo {
  display_name: string;
  launcher_slug: string;
  script_path: string;
  desktop_entry_path: string;
  script_exists: boolean;
  desktop_entry_exists: boolean;
  is_stale: boolean;
}

export interface LauncherDeleteResult {
  script_deleted: boolean;
  desktop_entry_deleted: boolean;
  script_path: string;
  desktop_entry_path: string;
  script_skipped_reason?: string | null;
  desktop_entry_skipped_reason?: string | null;
}

export interface LauncherRenameResult {
  old_slug: string;
  new_slug: string;
  new_script_path: string;
  new_desktop_entry_path: string;
  script_renamed: boolean;
  desktop_entry_renamed: boolean;
  old_script_cleanup_warning?: string | null;
  old_desktop_entry_cleanup_warning?: string | null;
}
