use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamLaunchRequest {
    pub game_path: String,
    pub trainer_path: String,
    pub trainer_host_path: String,
    pub steam_app_id: String,
    pub steam_compat_data_path: String,
    pub steam_proton_path: String,
    pub steam_client_install_path: String,
    pub launch_trainer_only: bool,
    pub launch_game_only: bool,
}

impl SteamLaunchRequest {
    pub fn game_executable_name(&self) -> String {
        let trimmed_path = self.game_path.trim();

        if trimmed_path.is_empty() {
            return String::new();
        }

        let separator_index = trimmed_path
            .char_indices()
            .rev()
            .find_map(|(index, character)| matches!(character, '/' | '\\').then_some(index));

        match separator_index {
            Some(index) if index + 1 < trimmed_path.len() => trimmed_path[index + 1..].to_string(),
            Some(_) => String::new(),
            None => trimmed_path.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    GamePathRequired,
    TrainerPathRequired,
    TrainerHostPathRequired,
    SteamAppIdRequired,
    SteamCompatDataPathRequired,
    SteamCompatDataPathMissing,
    SteamCompatDataPathNotDirectory,
    SteamProtonPathRequired,
    SteamProtonPathMissing,
    SteamProtonPathNotExecutable,
    SteamClientInstallPathRequired,
    TrainerHostPathMissing,
    TrainerHostPathNotFile,
}

impl ValidationError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::GamePathRequired => {
                "Steam mode requires a game executable path so CrossHook can identify the game process."
            }
            Self::TrainerPathRequired => "Steam mode requires a trainer path.",
            Self::TrainerHostPathRequired => "Steam mode requires a trainer host path.",
            Self::SteamAppIdRequired => "Steam mode requires a Steam App ID.",
            Self::SteamCompatDataPathRequired => "Steam mode requires a compatdata path.",
            Self::SteamCompatDataPathMissing => "Steam mode compatdata path does not exist.",
            Self::SteamCompatDataPathNotDirectory => {
                "Steam mode compatdata path must be a directory."
            }
            Self::SteamProtonPathRequired => "Steam mode requires a Proton path.",
            Self::SteamProtonPathMissing => "Steam mode Proton path does not exist.",
            Self::SteamProtonPathNotExecutable => "Steam mode Proton path must be executable.",
            Self::SteamClientInstallPathRequired => {
                "Steam mode requires a Steam client install path."
            }
            Self::TrainerHostPathMissing => "Steam mode trainer host path does not exist.",
            Self::TrainerHostPathNotFile => "Steam mode trainer host path must be a file.",
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl Error for ValidationError {}

pub fn validate(request: &SteamLaunchRequest) -> Result<(), ValidationError> {
    if !request.launch_trainer_only && request.game_path.trim().is_empty() {
        return Err(ValidationError::GamePathRequired);
    }

    if request.trainer_path.trim().is_empty() {
        return Err(ValidationError::TrainerPathRequired);
    }

    if request.trainer_host_path.trim().is_empty() {
        return Err(ValidationError::TrainerHostPathRequired);
    }

    if request.steam_app_id.trim().is_empty() {
        return Err(ValidationError::SteamAppIdRequired);
    }

    if request.steam_compat_data_path.trim().is_empty() {
        return Err(ValidationError::SteamCompatDataPathRequired);
    }

    let compatdata_path = Path::new(request.steam_compat_data_path.trim());
    if !compatdata_path.exists() {
        return Err(ValidationError::SteamCompatDataPathMissing);
    }
    if !compatdata_path.is_dir() {
        return Err(ValidationError::SteamCompatDataPathNotDirectory);
    }

    if request.steam_proton_path.trim().is_empty() {
        return Err(ValidationError::SteamProtonPathRequired);
    }

    let proton_path = Path::new(request.steam_proton_path.trim());
    if !proton_path.exists() {
        return Err(ValidationError::SteamProtonPathMissing);
    }
    if !is_executable_file(proton_path) {
        return Err(ValidationError::SteamProtonPathNotExecutable);
    }

    if request.steam_client_install_path.trim().is_empty() {
        return Err(ValidationError::SteamClientInstallPathRequired);
    }

    let trainer_host_path = Path::new(request.trainer_host_path.trim());
    if !trainer_host_path.exists() {
        return Err(ValidationError::TrainerHostPathMissing);
    }
    if !trainer_host_path.is_file() {
        return Err(ValidationError::TrainerHostPathNotFile);
    }

    Ok(())
}

fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RequestFixture {
        _temp_dir: tempfile::TempDir,
        request: SteamLaunchRequest,
    }

    fn write_executable_file(path: &Path) {
        fs::write(path, b"test").expect("write file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod");
        }
    }

    fn valid_request() -> RequestFixture {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let compatdata = temp_dir.path().join("compat");
        let trainer = temp_dir.path().join("trainer.exe");
        let proton = temp_dir.path().join("proton");

        fs::create_dir_all(&compatdata).expect("compatdata dir");
        fs::write(&trainer, b"trainer").expect("trainer file");
        write_executable_file(&proton);

        RequestFixture {
            _temp_dir: temp_dir,
            request: SteamLaunchRequest {
                game_path: "/games/test/game.exe".to_string(),
                trainer_path: trainer.to_string_lossy().into_owned(),
                trainer_host_path: trainer.to_string_lossy().into_owned(),
                steam_app_id: "12345".to_string(),
                steam_compat_data_path: compatdata.to_string_lossy().into_owned(),
                steam_proton_path: proton.to_string_lossy().into_owned(),
                steam_client_install_path: "/tmp/steam".to_string(),
                launch_trainer_only: false,
                launch_game_only: false,
            },
        }
    }

    #[test]
    fn validates_successful_request() {
        let fixture = valid_request();
        assert_eq!(validate(&fixture.request), Ok(()));
    }

    #[test]
    fn requires_game_path_when_launching_game() {
        let mut fixture = valid_request();
        fixture.request.game_path.clear();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::GamePathRequired)
        );
    }

    #[test]
    fn allows_missing_game_path_for_trainer_only_launches() {
        let mut fixture = valid_request();
        fixture.request.game_path.clear();
        fixture.request.launch_trainer_only = true;

        assert_eq!(validate(&fixture.request), Ok(()));
    }

    #[test]
    fn requires_trainer_path() {
        let mut fixture = valid_request();
        fixture.request.trainer_path.clear();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::TrainerPathRequired)
        );
    }

    #[test]
    fn requires_trainer_host_path() {
        let mut fixture = valid_request();
        fixture.request.trainer_host_path.clear();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::TrainerHostPathRequired)
        );
    }

    #[test]
    fn requires_app_id() {
        let mut fixture = valid_request();
        fixture.request.steam_app_id.clear();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::SteamAppIdRequired)
        );
    }

    #[test]
    fn requires_compatdata_directory() {
        let mut fixture = valid_request();
        fixture.request.steam_compat_data_path.clear();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::SteamCompatDataPathRequired)
        );
    }

    #[test]
    fn rejects_missing_compatdata_directory() {
        let mut fixture = valid_request();
        fixture.request.steam_compat_data_path = "/does/not/exist".to_string();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::SteamCompatDataPathMissing)
        );
    }

    #[test]
    fn rejects_non_directory_compatdata_path() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let file_path = temp_dir.path().join("compat");
        fs::write(&file_path, b"not a dir").expect("file");

        let mut fixture = valid_request();
        fixture.request.steam_compat_data_path = file_path.to_string_lossy().into_owned();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::SteamCompatDataPathNotDirectory)
        );
    }

    #[test]
    fn requires_proton_path() {
        let mut fixture = valid_request();
        fixture.request.steam_proton_path.clear();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::SteamProtonPathRequired)
        );
    }

    #[test]
    fn rejects_missing_proton_path() {
        let mut fixture = valid_request();
        fixture.request.steam_proton_path = "/does/not/exist/proton".to_string();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::SteamProtonPathMissing)
        );
    }

    #[test]
    fn rejects_non_executable_proton_path() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let proton = temp_dir.path().join("proton");
        fs::write(&proton, b"proton").expect("proton file");

        let mut fixture = valid_request();
        fixture.request.steam_proton_path = proton.to_string_lossy().into_owned();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::SteamProtonPathNotExecutable)
        );
    }

    #[test]
    fn requires_steam_client_install_path() {
        let mut fixture = valid_request();
        fixture.request.steam_client_install_path.clear();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::SteamClientInstallPathRequired)
        );
    }

    #[test]
    fn requires_trainer_host_file() {
        let mut fixture = valid_request();
        fixture.request.trainer_host_path = "/does/not/exist/trainer.exe".to_string();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::TrainerHostPathMissing)
        );
    }

    #[test]
    fn rejects_trainer_host_directory() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut fixture = valid_request();
        fixture.request.trainer_host_path = temp_dir.path().to_string_lossy().into_owned();

        assert_eq!(
            validate(&fixture.request),
            Err(ValidationError::TrainerHostPathNotFile)
        );
    }
}
