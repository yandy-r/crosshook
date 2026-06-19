use tempfile::tempdir;

use super::super::parse::{parse_lutris_yaml, LutrisGameConfig, LutrisGameSection};

#[test]
fn parse_full_yaml_populates_exe_prefix_env_and_wine_version() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("elden-ring.yml");
    std::fs::write(
        &config_path,
        r#"name: Elden Ring
game-slug: elden-ring
game:
  exe: /home/user/Games/elden-ring/eldenring.exe
  args: -windowed
  working_dir: /home/user/Games/elden-ring
  prefix: /home/user/Games/elden-ring
  arch: win64
system:
  env:
    WINEDLLOVERRIDES: 'd3d11='
    SOME_FLAG: '1'
  terminal: false
  gamemode: true
  mangohud: false
  prefer_system_libs: true
  gamescope: false
wine:
  version: lutris-GE-Proton8-14-x86_64
  dxvk: true
  vkd3d: true
  dxvk_nvapi: false
  esync: true
  fsync: true
"#,
    )
    .unwrap();

    let parsed = parse_lutris_yaml(&config_path).unwrap();

    assert_eq!(parsed.name.as_deref(), Some("Elden Ring"));
    assert_eq!(parsed.game_slug.as_deref(), Some("elden-ring"));

    let game = parsed.game.as_ref().unwrap();
    assert_eq!(
        game.exe.as_deref(),
        Some("/home/user/Games/elden-ring/eldenring.exe")
    );
    assert_eq!(game.prefix.as_deref(), Some("/home/user/Games/elden-ring"));

    let system = parsed.system.as_ref().unwrap();
    let env = system.env.as_ref().unwrap();
    assert_eq!(
        env.get("WINEDLLOVERRIDES").map(String::as_str),
        Some("d3d11=")
    );
    assert_eq!(env.get("SOME_FLAG").map(String::as_str), Some("1"));

    let wine = parsed.wine.as_ref().unwrap();
    assert_eq!(wine.version.as_deref(), Some("lutris-GE-Proton8-14-x86_64"));
}

#[test]
fn parse_minimal_yaml_with_exe_only_leaves_other_fields_none() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("minimal.yml");
    std::fs::write(
        &config_path,
        "game:\n  exe: /home/user/Games/minimal/game.exe\n",
    )
    .unwrap();

    let parsed = parse_lutris_yaml(&config_path).unwrap();

    assert_eq!(
        parsed,
        LutrisGameConfig {
            game: Some(LutrisGameSection {
                exe: Some("/home/user/Games/minimal/game.exe".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }
    );
    assert!(parsed.name.is_none());
    assert!(parsed.game_slug.is_none());
    assert!(parsed.system.is_none());
    assert!(parsed.wine.is_none());
}
