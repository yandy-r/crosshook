use std::path::{Path, PathBuf};

use crate::launch::request::METHOD_PROTON_RUN;
use crate::profile::{
    GameProfile, GameSection, GamescopeConfig, GamescopeFilter, InjectionSection,
    LaunchOptimizationsSection, LaunchSection, LocalOverrideSection, RuntimeSection, SteamSection,
    TrainerSection,
};
use crate::settings::expand_path_with_tilde;

use super::parse::{LutrisGameConfig, LutrisGameSection, LutrisSystemSection, LutrisWineSection};
use super::paths::PgaGame;
use super::runner::{classify_runner, resolve_runner_path, RunnerKind, RunnerResolution};

#[derive(Debug, Clone)]
pub struct MappedLutrisInput {
    pub config: LutrisGameConfig,
    pub pga: Option<PgaGame>,
    pub lutris_root: PathBuf,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LutrisMapResult {
    pub profile: GameProfile,
    pub warnings: Vec<String>,
    pub importable: bool,
    pub game_name: String,
    pub runner: String,
}

pub fn map_lutris_to_profile(input: MappedLutrisInput) -> LutrisMapResult {
    let mut warnings = Vec::new();
    let game_section = input.config.game.as_ref();
    let system = input.config.system.as_ref();
    let wine = input.config.wine.as_ref();

    let game_name = derive_game_name(&input);
    let runner = input
        .pga
        .as_ref()
        .map(|p| p.runner.clone())
        .unwrap_or_else(|| "wine".to_string());

    let exe = game_section.and_then(|g| g.exe.clone());
    let importable = exe.as_ref().is_some_and(|e| !e.trim().is_empty());
    if !importable {
        warnings.push("No game executable configured; entry cannot be imported.".to_string());
    }

    let runner_version = wine.and_then(|w| w.version.as_deref()).unwrap_or("");
    if !runner_version.is_empty() {
        match classify_runner(runner_version) {
            RunnerKind::Proton => {
                if wine.is_some_and(|w| w.vkd3d == Some(false)) {
                    warnings.push(
                        "Lutris vkd3d setting is ignored for Proton runners (Proton manages VKD3D)."
                            .to_string(),
                    );
                }
                if wine.is_some_and(|w| w.dxvk_nvapi == Some(false)) {
                    warnings.push(
                        "Lutris DXVK NVAPI setting is ignored for Proton runners.".to_string(),
                    );
                }
            }
            RunnerKind::Wine => {
                warnings.push(format!(
                    "Wine runner '{runner_version}' will be imported with launch method proton_run."
                ));
            }
        }
    }

    let proton_path = match wine.and_then(|w| w.version.as_deref()) {
        Some(version) if !version.trim().is_empty() => {
            match resolve_runner_path(&input.lutris_root, version) {
                RunnerResolution::Installed(path) => path.display().to_string(),
                RunnerResolution::Missing(version) => {
                    warnings.push(format!(
                        "Referenced runner '{version}' is not installed under Lutris runners/wine/."
                    ));
                    String::new()
                }
            }
        }
        _ => String::new(),
    };

    let prefix_path = resolve_prefix_path(game_section, input.pga.as_ref(), &mut warnings);
    let working_directory = resolve_working_directory(game_section, exe.as_deref(), &mut warnings);

    let mut enabled_option_ids = collect_optimization_ids(wine, system);
    enabled_option_ids.sort();
    enabled_option_ids.dedup();

    let custom_env_vars = system.and_then(|s| s.env.clone()).unwrap_or_default();

    let custom_args = game_section
        .and_then(|g| g.args.as_ref())
        .map(|args| shell_split_args(args))
        .unwrap_or_default();

    let gamescope = map_gamescope_config(system);

    let mut launch = LaunchSection {
        method: METHOD_PROTON_RUN.to_string(),
        optimizations: LaunchOptimizationsSection {
            extra: toml::Table::new(),
            enabled_option_ids,
        },
        custom_env_vars,
        gamescope,
        ..LaunchSection::default()
    };
    launch.command_arguments.custom_args = custom_args;

    let profile = GameProfile {
        extra: toml::Table::new(),
        game: GameSection {
            extra: toml::Table::new(),
            name: game_name.clone(),
            executable_path: exe.unwrap_or_default(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection::default(),
        injection: InjectionSection::default(),
        steam: SteamSection::default(),
        runtime: RuntimeSection {
            prefix_path,
            proton_path,
            working_directory,
            ..RuntimeSection::default()
        },
        launch,
        local_override: LocalOverrideSection::default(),
        pre_launch_hooks: Vec::new(),
        post_exit_hooks: Vec::new(),
    };

    LutrisMapResult {
        profile,
        warnings,
        importable,
        game_name,
        runner,
    }
}

fn derive_game_name(input: &MappedLutrisInput) -> String {
    if let Some(name) = input
        .config
        .name
        .as_ref()
        .map(|n| n.trim())
        .filter(|n| !n.is_empty())
    {
        return name.to_string();
    }
    if let Some(pga) = &input.pga {
        if !pga.name.trim().is_empty() {
            return pga.name.trim().to_string();
        }
    }
    if let Some(slug) = input
        .config
        .game_slug
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return slug.to_string();
    }
    input
        .source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("lutris-game")
        .to_string()
}

fn resolve_prefix_path(
    game: Option<&LutrisGameSection>,
    pga: Option<&PgaGame>,
    warnings: &mut Vec<String>,
) -> String {
    if let Some(prefix) = game.and_then(|g| g.prefix.as_ref()) {
        let trimmed = prefix.trim();
        if !trimmed.is_empty() {
            return expand_path(trimmed, warnings);
        }
    }
    if let Some(pga) = pga {
        if let Some(directory) = pga.directory.as_deref() {
            let trimmed = directory.trim();
            if !trimmed.is_empty() {
                return expand_path(trimmed, warnings);
            }
        }
    }
    String::new()
}

fn resolve_working_directory(
    game: Option<&LutrisGameSection>,
    exe: Option<&str>,
    warnings: &mut Vec<String>,
) -> String {
    if let Some(working_dir) = game.and_then(|g| g.working_dir.as_ref()) {
        let trimmed = working_dir.trim();
        if !trimmed.is_empty() {
            return expand_path(trimmed, warnings);
        }
    }
    if let Some(exe_path) = exe.map(str::trim).filter(|p| !p.is_empty()) {
        if let Ok(expanded) = expand_path_with_tilde(exe_path) {
            if let Some(parent) = expanded.parent() {
                return parent.display().to_string();
            }
        }
        if let Some(parent) = Path::new(exe_path).parent() {
            return parent.display().to_string();
        }
    }
    String::new()
}

fn expand_path(raw: &str, warnings: &mut Vec<String>) -> String {
    match expand_path_with_tilde(raw) {
        Ok(path) => path.display().to_string(),
        Err(message) => {
            warnings.push(format!("Could not resolve path '{raw}': {message}"));
            raw.to_string()
        }
    }
}

fn collect_optimization_ids(
    wine: Option<&LutrisWineSection>,
    system: Option<&LutrisSystemSection>,
) -> Vec<String> {
    let mut ids = Vec::new();

    if wine.is_some_and(|w| w.esync == Some(false)) {
        ids.push("disable_esync".to_string());
    }
    if wine.is_some_and(|w| w.fsync == Some(false)) {
        ids.push("disable_fsync".to_string());
    }
    if system.is_some_and(|s| s.gamemode == Some(true)) {
        ids.push("use_gamemode".to_string());
    }
    if system.is_some_and(|s| s.mangohud == Some(true)) {
        ids.push("show_mangohud_overlay".to_string());
    }

    ids
}

fn map_gamescope_config(system: Option<&LutrisSystemSection>) -> GamescopeConfig {
    let Some(system) = system else {
        return GamescopeConfig::default();
    };
    if system.gamescope != Some(true) {
        return GamescopeConfig::default();
    }

    let mut config = GamescopeConfig {
        enabled: true,
        ..GamescopeConfig::default()
    };

    if system.gamescope_force_grab_cursor == Some(true) {
        config.force_grab_cursor = true;
    }
    if system.gamescope_hdr == Some(true) {
        config.hdr_enabled = true;
    }

    if let Some(res) = system.gamescope_output_res.as_deref() {
        if let Some((w, h)) = parse_resolution(res) {
            config.output_width = Some(w);
            config.output_height = Some(h);
        }
    }
    if let Some(res) = system.gamescope_game_res.as_deref() {
        if let Some((w, h)) = parse_resolution(res) {
            config.internal_width = Some(w);
            config.internal_height = Some(h);
        }
    }

    if let Some(mode) = system.gamescope_window_mode.as_deref() {
        match mode {
            "fullscreen" => config.fullscreen = true,
            "borderless" => config.borderless = true,
            _ => {}
        }
    }

    if let Some(sharpness) = system.gamescope_fsr_sharpness.as_deref() {
        if let Ok(value) = sharpness.parse::<u8>() {
            config.fsr_sharpness = Some(value);
            config.upscale_filter = Some(GamescopeFilter::Fsr);
        }
    }

    if let Some(fps) = system.gamescope_fps_limiter.as_deref() {
        if let Ok(limit) = fps.parse::<u32>() {
            config.frame_rate_limit = Some(limit);
        }
    }

    if let Some(flags) = system.gamescope_flags.as_deref() {
        let trimmed = flags.trim();
        if !trimmed.is_empty() {
            config.extra_args = shell_split_args(trimmed);
        }
    }

    config
}

fn parse_resolution(value: &str) -> Option<(u32, u32)> {
    let mut parts = value.split('x');
    let width = parts.next()?.parse().ok()?;
    let height = parts.next()?.parse().ok()?;
    Some((width, height))
}

fn shell_split_args(raw: &str) -> Vec<String> {
    shell_words::split(raw).unwrap_or_else(|_| raw.split_whitespace().map(str::to_string).collect())
}

pub fn is_wine_or_proton_runner(runner: &str) -> bool {
    let lower = runner.trim().to_ascii_lowercase();
    lower == "wine" || lower.contains("proton")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::lutris_import::parse::LutrisGameConfig;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn base_input(config: LutrisGameConfig) -> MappedLutrisInput {
        MappedLutrisInput {
            config,
            pga: None,
            lutris_root: PathBuf::from("/tmp/lutris"),
            source_path: PathBuf::from("/tmp/lutris/games/test.yml"),
        }
    }

    #[test]
    fn map_esync_false_adds_disable_esync() {
        let mut config = LutrisGameConfig::default();
        config.game = Some(LutrisGameSection {
            exe: Some("/games/foo.exe".to_string()),
            ..Default::default()
        });
        config.wine = Some(LutrisWineSection {
            esync: Some(false),
            ..Default::default()
        });

        let result = map_lutris_to_profile(base_input(config));
        assert!(result.importable);
        assert!(result
            .profile
            .launch
            .optimizations
            .enabled_option_ids
            .contains(&"disable_esync".to_string()));
    }

    #[test]
    fn map_esync_true_does_not_add_disable_esync() {
        let mut config = LutrisGameConfig::default();
        config.game = Some(LutrisGameSection {
            exe: Some("/games/foo.exe".to_string()),
            ..Default::default()
        });
        config.wine = Some(LutrisWineSection {
            esync: Some(true),
            ..Default::default()
        });

        let result = map_lutris_to_profile(base_input(config));
        assert!(!result
            .profile
            .launch
            .optimizations
            .enabled_option_ids
            .contains(&"disable_esync".to_string()));
    }

    #[test]
    fn map_missing_exe_is_not_importable() {
        let config = LutrisGameConfig::default();
        let result = map_lutris_to_profile(base_input(config));
        assert!(!result.importable);
    }

    #[test]
    fn parse_resolution_valid_dimensions() {
        assert_eq!(parse_resolution("1920x1080"), Some((1920, 1080)));
    }

    #[test]
    fn parse_resolution_empty_returns_none() {
        assert_eq!(parse_resolution(""), None);
    }

    #[test]
    fn parse_resolution_missing_height_returns_none() {
        assert_eq!(parse_resolution("1920"), None);
    }

    #[test]
    fn parse_resolution_invalid_returns_none() {
        assert_eq!(parse_resolution("axb"), None);
    }

    #[test]
    fn map_gamescope_config_enabled_maps_fields() {
        let system = LutrisSystemSection {
            gamescope: Some(true),
            gamescope_output_res: Some("1920x1080".to_string()),
            gamescope_game_res: Some("1280x720".to_string()),
            gamescope_window_mode: Some("fullscreen".to_string()),
            gamescope_fsr_sharpness: Some("5".to_string()),
            gamescope_fps_limiter: Some("60".to_string()),
            gamescope_flags: Some("-w 1920 -h 1080".to_string()),
            gamescope_hdr: Some(true),
            gamescope_force_grab_cursor: Some(true),
            ..Default::default()
        };

        let config = map_gamescope_config(Some(&system));
        assert!(config.enabled);
        assert_eq!(config.output_width, Some(1920));
        assert_eq!(config.output_height, Some(1080));
        assert_eq!(config.internal_width, Some(1280));
        assert_eq!(config.internal_height, Some(720));
        assert!(config.fullscreen);
        assert_eq!(config.fsr_sharpness, Some(5));
        assert_eq!(config.upscale_filter, Some(GamescopeFilter::Fsr));
        assert_eq!(config.frame_rate_limit, Some(60));
        assert!(config.hdr_enabled);
        assert!(config.force_grab_cursor);
        assert_eq!(config.extra_args, vec!["-w", "1920", "-h", "1080"]);
    }

    #[test]
    fn map_preserves_env_strings_verbatim() {
        let mut config = LutrisGameConfig::default();
        config.game = Some(LutrisGameSection {
            exe: Some("/games/foo.exe".to_string()),
            ..Default::default()
        });
        let mut env = BTreeMap::new();
        env.insert("WINEDLLOVERRIDES".to_string(), "d3d11=".to_string());
        env.insert("X".to_string(), "1".to_string());
        config.system = Some(LutrisSystemSection {
            env: Some(env),
            ..Default::default()
        });

        let result = map_lutris_to_profile(base_input(config));
        assert_eq!(
            result.profile.launch.custom_env_vars.get("X"),
            Some(&"1".to_string())
        );
    }
}
