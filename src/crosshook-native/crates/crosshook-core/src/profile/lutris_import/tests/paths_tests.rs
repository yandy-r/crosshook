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

mod relative_root {
    //! The root may reach the service relative to the process working directory;
    //! joins and containment must agree on the canonical spelling. Tests cannot
    //! change the process CWD safely in parallel, so the fixture lives inside
    //! the current CWD and is addressed through a relative path.
    use super::*;
    use std::path::{Path, PathBuf};

    static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    struct CwdFixture {
        root: PathBuf,
    }

    impl CwdFixture {
        fn new() -> Self {
            let cwd = std::env::current_dir().expect("current dir");
            let root = cwd.join(format!(
                "lutris-relative-root-test-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&root).expect("create fixture root");
            Self { root }
        }

        /// `self.root` relative to the CWD (a `./name`-style path).
        fn relative_root(&self) -> PathBuf {
            let cwd = std::env::current_dir().expect("current dir");
            Path::new(".").join(
                self.root
                    .strip_prefix(&cwd)
                    .expect("fixture lives under CWD"),
            )
        }
    }

    impl Drop for CwdFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn list_game_configs_accepts_relative_root() {
        let fixture = CwdFixture::new();
        let games = fixture.root.join("games");
        std::fs::create_dir_all(&games).expect("games dir");
        std::fs::write(games.join("alpha.yml"), "game: {}").expect("alpha.yml");

        let configs = list_game_configs(&fixture.relative_root());
        assert_eq!(
            configs,
            vec![std::fs::canonicalize(games.join("alpha.yml")).expect("canonical alpha.yml")],
            "a root relative to the CWD must still scan its games/ directory"
        );
    }

    #[test]
    fn read_pga_games_accepts_relative_root() {
        let fixture = CwdFixture::new();
        write_pga_db(
            &fixture.root,
            &[("Rel Game", "rel-game", "wine", "/games/rel", "rel-game")],
        );

        let games = read_pga_games(&fixture.relative_root()).expect("read pga games");
        assert_eq!(
            games,
            vec![PgaGame {
                name: "Rel Game".to_string(),
                slug: "rel-game".to_string(),
                runner: "wine".to_string(),
                directory: Some("/games/rel".to_string()),
                configpath: Some("rel-game".to_string()),
            }]
        );
    }
}

#[cfg(unix)]
mod containment {
    use super::*;
    use std::os::unix::fs::symlink;

    const CONTAINMENT_MESSAGE: &str =
        "Lutris database path could not be resolved inside the library root";

    #[test]
    fn list_game_configs_skips_outward_symlinked_yaml() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().join("library");
        let games = root.join("games");
        std::fs::create_dir_all(&games).expect("games dir");
        std::fs::write(games.join("real.yml"), "game: {}").expect("real.yml");
        // A YAML payload living outside the root, linked into games/.
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).expect("outside dir");
        std::fs::write(outside.join("evil.yml"), "game: {}").expect("evil.yml");
        symlink(outside.join("evil.yml"), games.join("evil.yml")).expect("symlink yaml");

        let configs = list_game_configs(&root);
        assert_eq!(
            configs,
            vec![std::fs::canonicalize(games.join("real.yml")).expect("canonical real.yml")],
            "only the contained config may be returned"
        );
    }

    #[test]
    fn list_game_configs_skips_outward_symlinked_games_and_nested_dirs() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().join("library");
        std::fs::create_dir_all(&root).expect("root dir");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(outside.join("nested")).expect("outside dirs");
        std::fs::write(outside.join("evil.yml"), "game: {}").expect("evil.yml");
        std::fs::write(outside.join("nested/deep.yml"), "game: {}").expect("deep.yml");
        // games/ itself is a link out, and later a nested dir links out.
        symlink(&outside, root.join("games")).expect("symlink games");
        assert!(
            list_game_configs(&root).is_empty(),
            "outward games/ link must yield no configs"
        );

        std::fs::remove_file(root.join("games")).expect("unlink games");
        std::fs::create_dir_all(root.join("games")).expect("real games dir");
        std::fs::write(root.join("games/ok.yml"), "game: {}").expect("ok.yml");
        symlink(outside.join("nested"), root.join("games/nested")).expect("symlink nested");

        let configs = list_game_configs(&root);
        assert_eq!(
            configs,
            vec![std::fs::canonicalize(root.join("games/ok.yml")).expect("canonical ok.yml")],
            "outward nested link must be skipped while valid config remains"
        );
    }

    #[test]
    fn read_pga_games_rejects_outward_symlinked_db() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().join("library");
        std::fs::create_dir_all(&root).expect("root dir");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).expect("outside dir");
        write_pga_db(&outside, &[("Evil", "evil", "wine", "/games/evil", "evil")]);
        symlink(outside.join("pga.db"), root.join("pga.db")).expect("symlink pga.db");

        let error = read_pga_games(&root).expect_err("outward pga.db link");
        assert!(matches!(
            &error,
            crate::profile::LutrisImportError::Pga { message } if message == CONTAINMENT_MESSAGE
        ));
    }

    #[test]
    fn read_pga_games_reports_containment_message_when_db_missing() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().join("library");
        std::fs::create_dir_all(&root).expect("root dir");

        let error = read_pga_games(&root).expect_err("missing pga.db");
        assert!(matches!(
            &error,
            crate::profile::LutrisImportError::Pga { message } if message == CONTAINMENT_MESSAGE
        ));
    }
}
