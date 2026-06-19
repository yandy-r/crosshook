use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use tempfile::TempDir;

use crate::profile::lutris_import::paths::{
    discover_lutris_root, list_game_configs, read_pga_games, PgaGame,
};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn write_pga_db(root: &Path, games: &[(&str, &str, &str, &str, &str)]) {
    let db_path = root.join("pga.db");
    let conn = Connection::open(&db_path).expect("open pga.db fixture");
    conn.execute_batch(
        "CREATE TABLE games (
            name TEXT,
            slug TEXT,
            runner TEXT,
            directory TEXT,
            configpath TEXT
        );",
    )
    .expect("create games table");
    for (name, slug, runner, directory, configpath) in games {
        conn.execute(
            "INSERT INTO games (name, slug, runner, directory, configpath)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (*name, *slug, *runner, *directory, *configpath),
        )
        .expect("insert pga row");
    }
}

struct HomeEnvGuard {
    home: Option<String>,
    xdg_config_home: Option<String>,
    xdg_data_home: Option<String>,
}

impl Drop for HomeEnvGuard {
    fn drop(&mut self) {
        restore_env_var("HOME", self.home.as_deref());
        restore_env_var("XDG_CONFIG_HOME", self.xdg_config_home.as_deref());
        restore_env_var("XDG_DATA_HOME", self.xdg_data_home.as_deref());
    }
}

fn restore_env_var(key: &str, value: Option<&str>) {
    match value {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}

fn set_home_env(tmp: &TempDir) -> HomeEnvGuard {
    let guard = HomeEnvGuard {
        home: std::env::var("HOME").ok(),
        xdg_config_home: std::env::var("XDG_CONFIG_HOME").ok(),
        xdg_data_home: std::env::var("XDG_DATA_HOME").ok(),
    };
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(".config"));
    std::env::set_var("XDG_DATA_HOME", tmp.path().join(".local/share"));
    guard
}

#[test]
fn discover_lutris_root_prefers_config_dir() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let tmp = TempDir::new().expect("tempdir");
    let config_root = tmp.path().join(".config/lutris");
    let data_root = tmp.path().join(".local/share/lutris");
    std::fs::create_dir_all(&config_root).expect("config lutris dir");
    std::fs::create_dir_all(&data_root).expect("data lutris dir");
    let _home_env = set_home_env(&tmp);

    assert_eq!(discover_lutris_root(), Some(config_root));
}

#[test]
fn discover_lutris_root_falls_back_to_data_local_dir() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let tmp = TempDir::new().expect("tempdir");
    let data_root = tmp.path().join(".local/share/lutris");
    std::fs::create_dir_all(&data_root).expect("data lutris dir");
    let _home_env = set_home_env(&tmp);

    assert_eq!(discover_lutris_root(), Some(data_root));
}

#[test]
fn discover_lutris_root_falls_back_to_flatpak_config_dir() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let tmp = TempDir::new().expect("tempdir");
    let flatpak_root = tmp.path().join(".var/app/net.lutris.Lutris/config/lutris");
    std::fs::create_dir_all(&flatpak_root).expect("flatpak lutris dir");
    let _home_env = set_home_env(&tmp);

    assert_eq!(discover_lutris_root(), Some(flatpak_root));
}

#[test]
fn list_game_configs_collects_flat_and_balanced_yaml_files() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();
    let games = root.join("games");
    std::fs::create_dir_all(games.join("balanced")).expect("games dirs");
    std::fs::write(games.join("alpha.yml"), "game: {}").expect("alpha.yml");
    std::fs::write(games.join("balanced/beta.yml"), "game: {}").expect("beta.yml");
    std::fs::write(games.join("ignored.txt"), "not yaml").expect("ignored.txt");
    // Deeper than games/*/*.yml is out of scope for Lutris balanced-dir layout.
    std::fs::create_dir_all(games.join("balanced/subdir")).expect("nested dir");
    std::fs::write(games.join("balanced/subdir/gamma.yml"), "game: {}").expect("gamma.yml");

    let configs = list_game_configs(root);
    assert_eq!(
        configs,
        vec![games.join("alpha.yml"), games.join("balanced/beta.yml"),]
    );
}

#[test]
fn list_game_configs_returns_empty_when_games_dir_missing() {
    let tmp = TempDir::new().expect("tempdir");
    assert!(list_game_configs(tmp.path()).is_empty());
}

#[test]
fn read_pga_games_returns_rows_from_fixture_db() {
    let tmp = TempDir::new().expect("tempdir");
    write_pga_db(
        tmp.path(),
        &[(
            "Test Game",
            "test-game",
            "wine",
            "/home/user/Games/test",
            "test-game",
        )],
    );

    let games = read_pga_games(tmp.path()).expect("read pga games");
    assert_eq!(
        games,
        vec![PgaGame {
            name: "Test Game".to_string(),
            slug: "test-game".to_string(),
            runner: "wine".to_string(),
            directory: Some("/home/user/Games/test".to_string()),
            configpath: Some("test-game".to_string()),
        }]
    );
}

#[test]
fn read_pga_games_errors_when_db_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let error = read_pga_games(tmp.path()).expect_err("missing pga.db");
    assert!(matches!(
        error,
        crate::profile::LutrisImportError::Pga { .. }
    ));
}
