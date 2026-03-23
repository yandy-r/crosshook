use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SteamExternalLauncherExportRequest {
    pub launcher_name: String,
    pub trainer_path: String,
    pub launcher_icon_path: String,
    pub steam_app_id: String,
    pub steam_compat_data_path: String,
    pub steam_proton_path: String,
    pub steam_client_install_path: String,
    pub target_home_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SteamExternalLauncherExportResult {
    pub display_name: String,
    pub launcher_slug: String,
    pub script_path: String,
    pub desktop_entry_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteamExternalLauncherExportValidationError {
    TrainerPathRequired,
    SteamAppIdRequired,
    SteamCompatDataPathRequired,
    SteamProtonPathRequired,
    SteamClientInstallPathRequired,
    TargetHomePathRequired,
    LauncherIconPathNotFound,
    LauncherIconPathInvalidExtension,
}

impl SteamExternalLauncherExportValidationError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::TrainerPathRequired => "External launcher export requires a trainer path.",
            Self::SteamAppIdRequired => "External launcher export requires a Steam App ID.",
            Self::SteamCompatDataPathRequired => {
                "External launcher export requires a compatdata path."
            }
            Self::SteamProtonPathRequired => "External launcher export requires a Proton path.",
            Self::SteamClientInstallPathRequired => {
                "External launcher export requires a Steam client install path."
            }
            Self::TargetHomePathRequired => "External launcher export requires a host home path.",
            Self::LauncherIconPathNotFound => "External launcher export icon path does not exist.",
            Self::LauncherIconPathInvalidExtension => {
                "External launcher export icon must be a PNG or JPG image."
            }
        }
    }
}

impl fmt::Display for SteamExternalLauncherExportValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl Error for SteamExternalLauncherExportValidationError {}

#[derive(Debug)]
pub enum SteamExternalLauncherExportError {
    InvalidRequest(SteamExternalLauncherExportValidationError),
    CouldNotResolveHomePath,
    Io(io::Error),
}

impl fmt::Display for SteamExternalLauncherExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(error) => f.write_str(error.message()),
            Self::CouldNotResolveHomePath => {
                f.write_str("Could not resolve a host home path for launcher export.")
            }
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl Error for SteamExternalLauncherExportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for SteamExternalLauncherExportError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn validate(
    request: &SteamExternalLauncherExportRequest,
) -> Result<(), SteamExternalLauncherExportValidationError> {
    if request.trainer_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::TrainerPathRequired);
    }

    if request.steam_app_id.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::SteamAppIdRequired);
    }

    if request.steam_compat_data_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::SteamCompatDataPathRequired);
    }

    if request.steam_proton_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::SteamProtonPathRequired);
    }

    if request.steam_client_install_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::SteamClientInstallPathRequired);
    }

    if request.target_home_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::TargetHomePathRequired);
    }

    if !request.launcher_icon_path.trim().is_empty() {
        let normalized_icon_path = normalize_host_unix_path(&request.launcher_icon_path);
        let icon_path = Path::new(&normalized_icon_path);

        if !icon_path.exists() {
            return Err(SteamExternalLauncherExportValidationError::LauncherIconPathNotFound);
        }

        let extension = icon_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();

        if !matches!(
            extension.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg"
        ) {
            return Err(
                SteamExternalLauncherExportValidationError::LauncherIconPathInvalidExtension,
            );
        }
    }

    Ok(())
}

pub fn export_launchers(
    request: &SteamExternalLauncherExportRequest,
) -> Result<SteamExternalLauncherExportResult, SteamExternalLauncherExportError> {
    validate(request).map_err(SteamExternalLauncherExportError::InvalidRequest)?;

    let display_name = resolve_display_name(
        &request.launcher_name,
        &request.steam_app_id,
        &request.trainer_path,
    );
    let launcher_slug = sanitize_launcher_slug(&display_name);
    let target_home_path = resolve_target_home_path(
        &request.target_home_path,
        &request.steam_client_install_path,
    );

    if target_home_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportError::CouldNotResolveHomePath);
    }

    let script_path = combine_host_unix_path(
        &target_home_path,
        ".local/share/crosshook/launchers",
        &format!("{launcher_slug}-trainer.sh"),
    );
    let desktop_entry_path = combine_host_unix_path(
        &target_home_path,
        ".local/share/applications",
        &format!("crosshook-{launcher_slug}-trainer.desktop"),
    );

    write_host_text_file(
        &script_path,
        &build_trainer_script_content(request, &display_name),
    )?;
    write_host_text_file(
        &desktop_entry_path,
        &build_desktop_entry_content(&display_name, &script_path, &request.launcher_icon_path),
    )?;

    Ok(SteamExternalLauncherExportResult {
        display_name,
        launcher_slug,
        script_path,
        desktop_entry_path,
    })
}

fn resolve_display_name(preferred_name: &str, steam_app_id: &str, trainer_path: &str) -> String {
    if !preferred_name.trim().is_empty() {
        return preferred_name.trim().to_string();
    }

    let trainer_name = Path::new(trainer_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .trim()
        .to_string();

    if !trainer_name.is_empty() {
        return trainer_name;
    }

    format!("steam-{steam_app_id}-trainer")
}

pub fn sanitize_launcher_slug(value: &str) -> String {
    if value.trim().is_empty() {
        return "crosshook-trainer".to_string();
    }

    let mut slug = String::with_capacity(value.len());
    let mut last_character_was_separator = false;

    for character in value.trim().chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            slug.push(character);
            last_character_was_separator = false;
            continue;
        }

        if last_character_was_separator {
            continue;
        }

        slug.push('-');
        last_character_was_separator = true;
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "crosshook-trainer".to_string()
    } else {
        slug
    }
}

fn combine_host_unix_path(root_path: &str, segment_one: &str, segment_two: &str) -> String {
    let normalized_root_path = normalize_host_unix_path(root_path);
    let normalized_root_path = normalized_root_path.trim_end_matches('/');
    if normalized_root_path.is_empty() {
        return String::new();
    }

    let mut result = normalized_root_path.to_string();
    for segment in [segment_one, segment_two] {
        let normalized_segment = normalize_host_unix_path(segment);
        let normalized_segment = normalized_segment.trim_matches('/');
        if normalized_segment.is_empty() {
            continue;
        }

        result.push('/');
        result.push_str(normalized_segment);
    }

    result
}

fn build_trainer_script_content(
    request: &SteamExternalLauncherExportRequest,
    display_name: &str,
) -> String {
    let mut content = String::new();
    content.push_str("#!/usr/bin/env bash\n");
    content.push_str("set -euo pipefail\n\n");
    content.push_str(&format!("# {display_name} - Trainer launcher\n"));
    content.push_str("# Generated by CrossHook\n");
    content.push_str("# https://github.com/yandy-r/crosshook\n");
    content.push_str("# Launch this after the Steam game has reached the in-game menu.\n");
    content.push_str("# Stages the trainer bundle into the Proton prefix before launch.\n");
    content.push_str(&format!(
        "export STEAM_COMPAT_DATA_PATH={}\n",
        shell_single_quoted(&request.steam_compat_data_path)
    ));
    content.push_str(&format!(
        "export STEAM_COMPAT_CLIENT_INSTALL_PATH={}\n",
        shell_single_quoted(&request.steam_client_install_path)
    ));
    content.push_str("export WINEPREFIX=\"$STEAM_COMPAT_DATA_PATH/pfx\"\n");
    content.push_str(&format!(
        "PROTON={}\n",
        shell_single_quoted(&request.steam_proton_path)
    ));
    content.push_str(&format!(
        "TRAINER_HOST_PATH={}\n",
        shell_single_quoted(&request.trainer_path)
    ));
    content.push_str(
        r#"

copy_support_directory_if_present() {
  local source_dir="$1"
  local target_dir="$2"
  local child_name="$3"

  if [[ -d "$source_dir/$child_name" ]]; then
    mkdir -p "$target_dir"
    cp -R "$source_dir/$child_name" "$target_dir/"
  fi
}

stage_trainer_support_files() {
  local trainer_source_dir="$1"
  local staged_target_dir="$2"
  local trainer_file_name="$3"
  local trainer_base_name="$4"
  local sibling_file
  local sibling_name

  shopt -s nullglob

  for sibling_file in "$trainer_source_dir"/*; do
    sibling_name="$(basename "$sibling_file")"

    if [[ "$sibling_name" == "$trainer_file_name" ]]; then
      continue
    fi

    if [[ -f "$sibling_file" ]]; then
      case "$sibling_name" in
        "$trainer_base_name".*.json|\
        "$trainer_base_name".*.config|\
        "$trainer_base_name".*.ini|\
        "$trainer_base_name".*.dll|\
        "$trainer_base_name".*.bin|\
        "$trainer_base_name".*.dat|\
        "$trainer_base_name".*.pak)
          cp -f "$sibling_file" "$staged_target_dir/"
          ;;
        *.dll|*.json|*.config|*.ini|*.pak|*.dat|*.bin)
          cp -f "$sibling_file" "$staged_target_dir/"
          ;;
      esac
    fi
  done

  shopt -u nullglob

  for support_dir in assets data lib bin runtimes plugins locales cef resources; do
    copy_support_directory_if_present "$trainer_source_dir" "$staged_target_dir" "$support_dir"
  done
}

trainer_host_path="$(realpath "$TRAINER_HOST_PATH")"
trainer_file_name="$(basename "$trainer_host_path")"
trainer_base_name="${trainer_file_name%.*}"
trainer_source_dir="$(dirname "$trainer_host_path")"
staged_trainer_root="$STEAM_COMPAT_DATA_PATH/pfx/drive_c/CrossHook/StagedTrainers"
staged_trainer_dir="$staged_trainer_root/$trainer_base_name"
staged_trainer_host_path="$staged_trainer_dir/$trainer_file_name"
staged_trainer_windows_path="C:\\CrossHook\\StagedTrainers\\$trainer_base_name\\$trainer_file_name"

mkdir -p "$staged_trainer_dir"
cp -f "$trainer_host_path" "$staged_trainer_host_path"
stage_trainer_support_files "$trainer_source_dir" "$staged_trainer_dir" "$trainer_file_name" "$trainer_base_name"

exec "$PROTON" run "$staged_trainer_windows_path"
"#,
    );
    content
}

fn build_desktop_entry_content(
    display_name: &str,
    script_path: &str,
    launcher_icon_path: &str,
) -> String {
    let mut content = String::new();
    content.push_str("[Desktop Entry]\n");
    content.push_str("Type=Application\n");
    content.push_str("Version=1.0\n");
    content.push_str(&format!("Name={display_name} - Trainer\n"));
    content.push_str(&format!(
        "Comment=Trainer launcher for {display_name}. Generated by CrossHook: https://github.com/yandy-r/crosshook\n"
    ));
    content.push_str(&format!(
        "Exec=/bin/bash {}\n",
        escape_desktop_exec_argument(script_path)
    ));
    content.push_str("Terminal=false\n");
    content.push_str("Categories=Game;\n");
    content.push_str(&format!(
        "Icon={}\n",
        resolve_desktop_icon_value(launcher_icon_path)
    ));
    content.push_str("StartupNotify=false\n");
    content
}

fn write_host_text_file(host_path: &str, content: &str) -> Result<(), io::Error> {
    let writable_path = PathBuf::from(host_path);
    let directory_path = writable_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("could not resolve a parent directory for '{host_path}'"),
        )
    })?;

    fs::create_dir_all(directory_path)?;
    fs::write(&writable_path, content.replace("\r\n", "\n"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&writable_path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&writable_path, permissions)?;
    }

    Ok(())
}

pub fn resolve_target_home_path(
    preferred_home_path: &str,
    steam_client_install_path: &str,
) -> String {
    let normalized_preferred_home_path = normalize_host_unix_path(preferred_home_path);
    if looks_like_usable_host_unix_path(&normalized_preferred_home_path) {
        return normalized_preferred_home_path;
    }

    let normalized_steam_client_install_path = normalize_host_unix_path(steam_client_install_path);
    if let Some(derived_home_path) =
        try_resolve_home_from_steam_client_install_path(&normalized_steam_client_install_path)
    {
        return derived_home_path;
    }

    normalized_preferred_home_path
}

fn try_resolve_home_from_steam_client_install_path(
    steam_client_install_path: &str,
) -> Option<String> {
    const LOCAL_SHARE_STEAM_SUFFIX: &str = "/.local/share/Steam";
    const DOT_STEAM_ROOT_SUFFIX: &str = "/.steam/root";

    if steam_client_install_path.trim().is_empty() {
        return None;
    }

    if let Some(home_path) = steam_client_install_path.strip_suffix(LOCAL_SHARE_STEAM_SUFFIX) {
        let home_path = home_path.trim();
        if !home_path.is_empty() {
            return Some(home_path.to_string());
        }
    }

    if let Some(home_path) = steam_client_install_path.strip_suffix(DOT_STEAM_ROOT_SUFFIX) {
        let home_path = home_path.trim();
        if !home_path.is_empty() {
            return Some(home_path.to_string());
        }
    }

    None
}

fn resolve_desktop_icon_value(launcher_icon_path: &str) -> String {
    let normalized_launcher_icon_path = normalize_host_unix_path(launcher_icon_path);
    if normalized_launcher_icon_path.trim().is_empty() {
        "applications-games".to_string()
    } else {
        normalized_launcher_icon_path
    }
}

fn normalize_host_unix_path(path: &str) -> String {
    path.trim().replace('\\', "/")
}

fn looks_like_usable_host_unix_path(path: &str) -> bool {
    !path.trim().is_empty() && path.starts_with('/') && !path.contains("/compatdata/")
}

fn shell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn escape_desktop_exec_argument(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(' ', "\\ ")
        .replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn slug_generation_collapses_non_alphanumeric_runs() {
        assert_eq!(
            sanitize_launcher_slug("  CrossHook: Trainer 2026!!  "),
            "crosshook-trainer-2026"
        );
        assert_eq!(sanitize_launcher_slug(""), "crosshook-trainer");
        assert_eq!(sanitize_launcher_slug("---"), "crosshook-trainer");
    }

    #[test]
    fn shell_single_quote_escaping_matches_posix_pattern() {
        assert_eq!(shell_single_quoted("abc"), "'abc'");
        assert_eq!(shell_single_quoted("a'b"), "'a'\"'\"'b'");
    }

    #[test]
    fn desktop_exec_escaping_matches_csharp_rules() {
        assert_eq!(
            escape_desktop_exec_argument("/tmp/Cross Hook/runner\".sh"),
            "/tmp/Cross\\ Hook/runner\\\".sh"
        );
    }

    #[test]
    fn export_writes_expected_paths_and_content() {
        let temp_home = tempdir().expect("temp home");
        let icon_path = temp_home.path().join("launcher icon.png");
        fs::write(&icon_path, b"icon").expect("icon");

        let request = SteamExternalLauncherExportRequest {
            launcher_name: "Elden Ring Deluxe".to_string(),
            trainer_path: "/opt/Trainers/Trainer's Edition.exe".to_string(),
            launcher_icon_path: icon_path.to_string_lossy().into_owned(),
            steam_app_id: "1245620".to_string(),
            steam_compat_data_path: "/tmp/compatdata/1245620".to_string(),
            steam_proton_path: "/opt/Proton/proton".to_string(),
            steam_client_install_path: temp_home
                .path()
                .join(".local/share/Steam")
                .to_string_lossy()
                .into_owned(),
            target_home_path: "/tmp/not-a-home/compatdata/steam".to_string(),
        };

        let result = export_launchers(&request).expect("export");

        assert_eq!(result.display_name, "Elden Ring Deluxe");
        assert_eq!(result.launcher_slug, "elden-ring-deluxe");
        assert_eq!(
            result.script_path,
            temp_home
                .path()
                .join(".local/share/crosshook/launchers/elden-ring-deluxe-trainer.sh")
                .to_string_lossy()
                .into_owned()
        );
        assert_eq!(
            result.desktop_entry_path,
            temp_home
                .path()
                .join(".local/share/applications/crosshook-elden-ring-deluxe-trainer.desktop")
                .to_string_lossy()
                .into_owned()
        );

        let script_content = fs::read_to_string(&result.script_path).expect("script");
        assert!(script_content.contains("export STEAM_COMPAT_DATA_PATH='/tmp/compatdata/1245620'"));
        assert!(script_content.contains("export STEAM_COMPAT_CLIENT_INSTALL_PATH='"));
        assert!(script_content.contains("PROTON='/opt/Proton/proton'"));
        assert!(script_content.contains("TRAINER_HOST_PATH='/opt/Trainers/Trainer'\"'\"'s Edition.exe'"));
        assert!(script_content.contains("staged_trainer_root=\"$STEAM_COMPAT_DATA_PATH/pfx/drive_c/CrossHook/StagedTrainers\""));
        assert!(script_content.contains("staged_trainer_windows_path=\"C:\\\\CrossHook\\\\StagedTrainers\\\\$trainer_base_name\\\\$trainer_file_name\""));
        assert!(script_content.contains("exec \"$PROTON\" run \"$staged_trainer_windows_path\""));

        let desktop_content = fs::read_to_string(&result.desktop_entry_path).expect("desktop");
        assert!(desktop_content.contains("Name=Elden Ring Deluxe - Trainer"));
        assert!(desktop_content.contains("Exec=/bin/bash "));
        assert!(desktop_content.contains("Icon="));
        assert!(desktop_content.contains(&icon_path.to_string_lossy().replace('\\', "\\\\")));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&result.script_path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o755);
        }
    }

    #[test]
    fn desktop_icon_falls_back_to_applications_games() {
        let content = build_desktop_entry_content("Test", "/tmp/launcher.sh", "");
        assert!(content.contains("Icon=applications-games"));
    }

    #[test]
    fn resolves_home_from_steam_client_suffix() {
        assert_eq!(
            resolve_target_home_path(
                "/tmp/wrong/compatdata/steam",
                "/home/user/.local/share/Steam"
            ),
            "/home/user"
        );
    }
}
