//! Golden byte compatibility for launcher export (YAN-847 Task 0.1, AC3).
//!
//! Each case drives `LauncherExportService` (`export`, `preview_script`,
//! `preview_desktop`) with a fixed request and compares the written script and desktop entry (plus the
//! preview strings for two cases) byte-for-byte with the fixtures under
//! `fixtures/launcher_export/`. The fixtures were captured from the unmodified
//! Tauri-era code before any source moved. They are frozen: a mismatch is an
//! export regression, never a reason to regenerate them.
//!
//! The tempdir `$HOME` is substituted with the fixed `@HOME@` placeholder on
//! the produced side before comparison so the fixtures are machine-independent.
//! The frozen fixtures were captured with the same substitution.
//!
//! Fixture SHA-256 (frozen at capture time):
//!
//! ```text
//! 21591cc300dfe5cd32e0bb184a657526459fb7d752e2c85c2eaf99cdee6b02e7  proton_run_copy_to_prefix/desktop.desktop
//! dc82612f66d3aba4b70f1424a944668a9b6a6dde4c2d75ca2f66af8a6b856152  proton_run_copy_to_prefix/script.sh
//! 7a35e3b7c66f110d8adb0e420326fe69fbb7b7666c9a113de2bcae7e68c31ee1  proton_run_network_env/desktop.desktop
//! 8622805a3ee69268e090a61c03e831d7bf51fe5244b57485de3b31a9e400c2a3  proton_run_network_env/preview-desktop.txt
//! 62c43bcb3277f151565c2b33e2d5447c3caf07c2db97d815c693115dd034a23f  proton_run_network_env/preview-script.txt
//! d6cf83b26eb1d97f5499018e5fa058f3a4f526bb1b13bbd1c3419ffeabd3e253  proton_run_network_env/script.sh
//! 68087b9ce4d8d048e25ffac824c2bec1701276d06328cae5c4a118ff4210616e  proton_run_plain/desktop.desktop
//! 508fc291f488e57b8d771b8a6ae5ce7c6293042645b0d9bda748c9e561e2b043  proton_run_plain/script.sh
//! 0ca662dd00579a35fffe69b6472f06793430f0c540d1126dd6e3b32e2eb7fc44  proton_run_umu_proton/desktop.desktop
//! c3c1de5bfa50c71d57095a40bf183fbeae7f992ea028c6ae0ea8a138982146c0  proton_run_umu_proton/script.sh
//! ee33a2880aab208d9db395bf415240a90724972b147fa7197f89a3bfb36e365f  steam_applaunch_gamescope/desktop.desktop
//! 8c40ffe2611e63e805c4f0e03c9c2e19120f20456afabecc77c12bcbafca06ba  steam_applaunch_gamescope/script.sh
//! b0c8e64c1dc0248a171185551a9cd2370cfc68b2b8e8b59329a6b9271a97846f  steam_applaunch_plain/desktop.desktop
//! a357bc6d85f84b891795159974cdd7fa3bfdf3cfa254e478103922746ee83afa  steam_applaunch_plain/preview-desktop.txt
//! da8e1caf319ae5a552c11ff826506dae0b1c31ce6d1a3fdb9673836fec39a0d0  steam_applaunch_plain/preview-script.txt
//! e3a79518d83987f68ec1fb9f5882c716ec038665bb2d966ca6f6fb1e5f1e60ee  steam_applaunch_plain/script.sh
//! ```
//!
//! This is a `harness = false` binary. `main` points `HOME` at a tempdir before
//! any runtime or other thread exists, then builds a current-thread runtime and
//! runs the cases sequentially, so no thread can observe the environment while
//! it changes and the real user's launchers are never touched.

use std::fs;
use std::future::Future;
use std::os::unix::fs::PermissionsExt;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::pin::Pin;

use crosshook_app::LauncherExportService;
use crosshook_core::export::SteamExternalLauncherExportRequest;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{GamescopeConfig, ProfileStore, TrainerLoadingMode};
use crosshook_core::settings::{SettingsStore, UmuPreference};

const HOME_PLACEHOLDER: &str = "@HOME@";

/// What one case produced, with the tempdir home replaced by `@HOME@`.
struct Produced {
    script: Vec<u8>,
    desktop: Vec<u8>,
    preview_script: Option<Vec<u8>>,
    preview_desktop: Option<Vec<u8>>,
}

fn base_request(
    home: &Path,
    method: &str,
    launcher_name: &str,
) -> SteamExternalLauncherExportRequest {
    let trainer = home.join("trainers").join("Stub Trainer.exe");
    fs::create_dir_all(trainer.parent().expect("trainer parent")).expect("trainer dir");
    fs::write(&trainer, b"MZ stub trainer").expect("stub trainer");

    SteamExternalLauncherExportRequest {
        method: method.to_string(),
        launcher_name: launcher_name.to_string(),
        trainer_path: trainer.to_string_lossy().into_owned(),
        trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
        launcher_icon_path: String::new(),
        prefix_path: "/fake/prefix".to_string(),
        proton_path: "/fake/proton/proton".to_string(),
        steam_app_id: "12345".to_string(),
        steam_client_install_path: "/fake/steam".to_string(),
        target_home_path: home.to_string_lossy().into_owned(),
        profile_name: None,
        runtime_steam_app_id: String::new(),
        umu_game_id: String::new(),
        umu_preference: UmuPreference::Auto,
        network_isolation: false,
        gamescope: GamescopeConfig::default(),
        custom_env_vars: Default::default(),
    }
}

fn steam_applaunch_plain(home: &Path) -> SteamExternalLauncherExportRequest {
    base_request(home, "steam_applaunch", "Golden Steam Plain")
}

fn steam_applaunch_gamescope(home: &Path) -> SteamExternalLauncherExportRequest {
    let mut request = base_request(home, "steam_applaunch", "Golden Steam Gamescope");
    request.gamescope = GamescopeConfig {
        enabled: true,
        internal_width: Some(1280),
        internal_height: Some(720),
        output_width: Some(1920),
        output_height: Some(1080),
        frame_rate_limit: Some(60),
        fullscreen: true,
        ..GamescopeConfig::default()
    };
    request
}

fn proton_run_plain(home: &Path) -> SteamExternalLauncherExportRequest {
    base_request(home, "proton_run", "Golden Proton Plain")
}

fn proton_run_network_env(home: &Path) -> SteamExternalLauncherExportRequest {
    let mut request = base_request(home, "proton_run", "Golden Proton Network Env");
    request.network_isolation = true;
    // Inserted out of order on purpose: the map must emit sorted by key.
    request
        .custom_env_vars
        .insert("ZED_VAR".to_string(), "it's z".to_string());
    request
        .custom_env_vars
        .insert("ALPHA_VAR".to_string(), "alpha value".to_string());
    request
}

fn proton_run_umu_proton(home: &Path) -> SteamExternalLauncherExportRequest {
    let mut request = base_request(home, "proton_run", "Golden Proton Umu");
    request.umu_preference = UmuPreference::Proton;
    request
}

fn proton_run_copy_to_prefix(home: &Path) -> SteamExternalLauncherExportRequest {
    let mut request = base_request(home, "proton_run", "Golden Proton Copy");
    request.trainer_loading_mode = TrainerLoadingMode::CopyToPrefix;
    request
}

/// Exports `request` under `home`, asserts the on-disk modes, and returns the
/// placeholder-substituted bytes. Previews are produced when `with_previews`.
async fn produce(
    service: &LauncherExportService,
    home: &Path,
    request: &SteamExternalLauncherExportRequest,
    with_previews: bool,
) -> Produced {
    let home_str = home.to_string_lossy().into_owned();
    let normalize = |text: String| text.replace(&home_str, HOME_PLACEHOLDER).into_bytes();

    let result = service
        .export(request.clone())
        .await
        .expect("export launchers");
    let script_mode = fs::metadata(&result.script_path)
        .expect("script metadata")
        .permissions()
        .mode()
        & 0o777;
    let desktop_mode = fs::metadata(&result.desktop_entry_path)
        .expect("desktop metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(script_mode, 0o700, "script mode for {}", result.script_path);
    assert_eq!(
        desktop_mode, 0o644,
        "desktop mode for {}",
        result.desktop_entry_path
    );

    let script = fs::read_to_string(&result.script_path).expect("read script");
    let desktop = fs::read_to_string(&result.desktop_entry_path).expect("read desktop entry");

    let (preview_script, preview_desktop) = if with_previews {
        (
            Some(normalize(
                service
                    .preview_script(request.clone())
                    .await
                    .expect("preview script"),
            )),
            Some(normalize(
                service
                    .preview_desktop(request.clone())
                    .await
                    .expect("preview desktop"),
            )),
        )
    } else {
        (None, None)
    };

    Produced {
        script: normalize(script),
        desktop: normalize(desktop),
        preview_script,
        preview_desktop,
    }
}

// ---- compare half ----

struct Expected {
    script: &'static [u8],
    desktop: &'static [u8],
    preview_script: Option<&'static [u8]>,
    preview_desktop: Option<&'static [u8]>,
}

macro_rules! fixture {
    ($case:literal, $file:literal) => {
        include_bytes!(concat!("fixtures/launcher_export/", $case, "/", $file))
    };
}

fn assert_bytes(label: &str, actual: &[u8], expected: &[u8]) {
    assert!(
        actual == expected,
        "{label} differs from the frozen golden fixture\n--- expected ---\n{}\n--- actual ---\n{}",
        String::from_utf8_lossy(expected),
        String::from_utf8_lossy(actual),
    );
}

async fn check_case(
    service: &LauncherExportService,
    case: &str,
    request: fn(&Path) -> SteamExternalLauncherExportRequest,
    expected: &Expected,
) {
    // Each case exports into its own subdirectory of the pinned `$HOME` tempdir
    // so cases cannot observe each other's launcher files.
    let case_home =
        std::path::PathBuf::from(std::env::var_os("HOME").expect("HOME pinned by main")).join(case);
    fs::create_dir_all(&case_home).expect("case home");
    let request = request(&case_home);
    let with_previews = expected.preview_script.is_some();
    let produced = produce(service, &case_home, &request, with_previews).await;
    assert_bytes(
        &format!("{case}/script.sh"),
        &produced.script,
        expected.script,
    );
    assert_bytes(
        &format!("{case}/desktop.desktop"),
        &produced.desktop,
        expected.desktop,
    );
    if let (Some(actual), Some(want)) = (&produced.preview_script, expected.preview_script) {
        assert_bytes(&format!("{case}/preview-script.txt"), actual, want);
    }
    if let (Some(actual), Some(want)) = (&produced.preview_desktop, expected.preview_desktop) {
        assert_bytes(&format!("{case}/preview-desktop.txt"), actual, want);
    }
}

async fn steam_applaunch_plain_matches_golden(service: LauncherExportService) {
    check_case(
        &service,
        "steam_applaunch_plain",
        steam_applaunch_plain,
        &Expected {
            script: fixture!("steam_applaunch_plain", "script.sh"),
            desktop: fixture!("steam_applaunch_plain", "desktop.desktop"),
            preview_script: Some(fixture!("steam_applaunch_plain", "preview-script.txt")),
            preview_desktop: Some(fixture!("steam_applaunch_plain", "preview-desktop.txt")),
        },
    )
    .await;
}

async fn steam_applaunch_gamescope_matches_golden(service: LauncherExportService) {
    check_case(
        &service,
        "steam_applaunch_gamescope",
        steam_applaunch_gamescope,
        &Expected {
            script: fixture!("steam_applaunch_gamescope", "script.sh"),
            desktop: fixture!("steam_applaunch_gamescope", "desktop.desktop"),
            preview_script: None,
            preview_desktop: None,
        },
    )
    .await;
}

async fn proton_run_plain_matches_golden(service: LauncherExportService) {
    check_case(
        &service,
        "proton_run_plain",
        proton_run_plain,
        &Expected {
            script: fixture!("proton_run_plain", "script.sh"),
            desktop: fixture!("proton_run_plain", "desktop.desktop"),
            preview_script: None,
            preview_desktop: None,
        },
    )
    .await;
}

async fn proton_run_network_env_matches_golden(service: LauncherExportService) {
    check_case(
        &service,
        "proton_run_network_env",
        proton_run_network_env,
        &Expected {
            script: fixture!("proton_run_network_env", "script.sh"),
            desktop: fixture!("proton_run_network_env", "desktop.desktop"),
            preview_script: Some(fixture!("proton_run_network_env", "preview-script.txt")),
            preview_desktop: Some(fixture!("proton_run_network_env", "preview-desktop.txt")),
        },
    )
    .await;
}

async fn proton_run_umu_proton_matches_golden(service: LauncherExportService) {
    check_case(
        &service,
        "proton_run_umu_proton",
        proton_run_umu_proton,
        &Expected {
            script: fixture!("proton_run_umu_proton", "script.sh"),
            desktop: fixture!("proton_run_umu_proton", "desktop.desktop"),
            preview_script: None,
            preview_desktop: None,
        },
    )
    .await;
}

async fn proton_run_copy_to_prefix_matches_golden(service: LauncherExportService) {
    check_case(
        &service,
        "proton_run_copy_to_prefix",
        proton_run_copy_to_prefix,
        &Expected {
            script: fixture!("proton_run_copy_to_prefix", "script.sh"),
            desktop: fixture!("proton_run_copy_to_prefix", "desktop.desktop"),
            preview_script: None,
            preview_desktop: None,
        },
    )
    .await;
}

type GoldenFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Every golden case, by libtest-style name.
const TESTS: [(&str, fn(LauncherExportService) -> GoldenFuture); 6] = [
    ("steam_applaunch_plain_matches_golden", |service| {
        Box::pin(steam_applaunch_plain_matches_golden(service))
    }),
    ("steam_applaunch_gamescope_matches_golden", |service| {
        Box::pin(steam_applaunch_gamescope_matches_golden(service))
    }),
    ("proton_run_plain_matches_golden", |service| {
        Box::pin(proton_run_plain_matches_golden(service))
    }),
    ("proton_run_network_env_matches_golden", |service| {
        Box::pin(proton_run_network_env_matches_golden(service))
    }),
    ("proton_run_umu_proton_matches_golden", |service| {
        Box::pin(proton_run_umu_proton_matches_golden(service))
    }),
    ("proton_run_copy_to_prefix_matches_golden", |service| {
        Box::pin(proton_run_copy_to_prefix_matches_golden(service))
    }),
];

/// The subset of the libtest command line cargo, nextest and IDEs pass.
struct Selection {
    list: bool,
    ignored: bool,
    exact: bool,
    filters: Vec<String>,
    skips: Vec<String>,
}

impl Selection {
    fn parse(args: &[String]) -> Self {
        let mut selection = Self {
            list: false,
            ignored: false,
            exact: false,
            filters: Vec::new(),
            skips: Vec::new(),
        };
        let mut rest = args.iter().skip(1);
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--list" => selection.list = true,
                "--ignored" => selection.ignored = true,
                "--exact" => selection.exact = true,
                "--skip" => selection.skips.extend(rest.next().cloned()),
                "--test-threads" | "--format" | "--color" | "--logfile" | "--report-time"
                | "--shuffle-seed" | "--ensure-time" | "-Z" => {
                    rest.next();
                }
                flag if flag.starts_with('-') => {}
                filter => selection.filters.push(filter.to_owned()),
            }
        }
        selection
    }

    fn matches(&self, pattern: &str, name: &str) -> bool {
        if self.exact {
            pattern == name
        } else {
            name.contains(pattern)
        }
    }

    /// No case is `#[ignore]`d, so `--ignored` selects nothing.
    fn selects(&self, name: &str) -> bool {
        !self.ignored
            && (self.filters.is_empty() || self.filters.iter().any(|f| self.matches(f, name)))
            && !self.skips.iter().any(|s| self.matches(s, name))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let selection = Selection::parse(&args);
    let selected: Vec<_> = TESTS
        .iter()
        .filter(|(name, _)| selection.selects(name))
        .collect();

    if selection.list {
        for (name, _) in &selected {
            println!("{name}: test");
        }
        return;
    }

    let home = tempfile::tempdir().expect("home tempdir");
    // SAFETY: harness = false; no thread exists yet, and the cases below run
    // sequentially on this thread after this write.
    unsafe {
        std::env::set_var("HOME", home.path());
    }

    // The runtime (and its threads) starts only after the `HOME` write above.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let service = LauncherExportService::new(
        ProfileStore::with_base_path(home.path().join("profiles")),
        SettingsStore::with_base_path(home.path().join("config")),
        MetadataStore::disabled(),
    );

    println!("\nrunning {} tests", selected.len());
    let mut failed = Vec::new();
    for (name, test) in &selected {
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
            runtime.block_on(test(service.clone()));
        }));
        match outcome {
            Ok(()) => println!("test {name} ... ok"),
            Err(_) => {
                println!("test {name} ... FAILED");
                failed.push(*name);
            }
        }
    }
    drop(runtime);
    drop(home);

    let passed = selected.len() - failed.len();
    if failed.is_empty() {
        println!("\ntest result: ok. {passed} passed; 0 failed\n");
        return;
    }
    println!("\nfailures:");
    for name in &failed {
        println!("    {name}");
    }
    println!(
        "\ntest result: FAILED. {passed} passed; {} failed\n",
        failed.len()
    );
    std::process::exit(1);
}
