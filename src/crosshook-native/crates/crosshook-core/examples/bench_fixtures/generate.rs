//! Canonical fixture-tree generation. Drives only public `crosshook-core` APIs so the
//! artifacts mirror exactly what the app itself writes.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use crosshook_core::community::taps::{
    CommunityTapSubscription, CommunityTapSyncResult, CommunityTapSyncStatus, CommunityTapWorkspace,
};
use crosshook_core::community::CommunityProfileIndex;
use crosshook_core::launch::diagnostics::{ExitCodeInfo, FailureMode};
use crosshook_core::launch::ValidationSeverity;
use crosshook_core::launch::{DiagnosticReport, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use crosshook_core::metadata::{sha256_hex, MetadataStore, ProtonCatalogRow};
use crosshook_core::profile::{
    CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating, GameProfile,
    GameSection, LaunchSection, ProfileStore, RuntimeSection, SteamSection, TrainerSection,
};
use crosshook_core::protonup::ProtonUpAvailableVersion;

use crate::guard;
use crate::layout::{Layout, TOKEN};
use crate::rng::Rng;

// ponytail: one committed gradient JPEG is written as the cover for every profile — identical
// pixels everywhere. Upgrade to per-profile variants if decode-cache effects ever matter.
static PORTRAIT_JPEG: &[u8] = include_bytes!("assets/portrait.jpg");

use crate::normalize::EPOCH_ANCHOR;
/// Far-future expiry so no image/catalog refresh is triggered by the app.
const FAR_FUTURE: &str = "2099-01-01T00:00:00+00:00";

#[derive(Clone)]
pub struct BenchConfig {
    pub seed: u64,
    pub profiles: usize,
    pub community_profiles: usize,
    pub proton_rows: usize,
    pub log_lines: usize,
    pub empty: bool,
}

impl BenchConfig {
    pub fn full(seed: u64) -> Self {
        Self {
            seed,
            profiles: 500,
            community_profiles: 1_000,
            proton_rows: 300,
            log_lines: 50_000,
            empty: false,
        }
    }

    #[cfg(test)]
    pub fn reduced(seed: u64) -> Self {
        Self {
            seed,
            profiles: 12,
            community_profiles: 24,
            proton_rows: 8,
            log_lines: 400,
            empty: false,
        }
    }

    pub fn empty(seed: u64) -> Self {
        Self {
            seed,
            profiles: 0,
            community_profiles: 0,
            proton_rows: 0,
            log_lines: 0,
            empty: true,
        }
    }
}

#[derive(serde::Serialize)]
struct ManifestFile {
    schema_version: u32,
    generator: &'static str,
    seed: u64,
    empty: bool,
    token: &'static str,
    env: std::collections::BTreeMap<&'static str, &'static str>,
    layout: std::collections::BTreeMap<&'static str, &'static str>,
    counts: std::collections::BTreeMap<&'static str, usize>,
    log: &'static str,
}

pub fn generate(out: &Path, cfg: &BenchConfig) -> Result<(), String> {
    let started = Instant::now();
    let out = guard::check_output_dir_real_env(out).map_err(|e| e.to_string())?;
    if out.exists()
        && std::fs::read_dir(&out)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        return Err(format!(
            "output directory {} exists and is not empty; bench fixtures refuse to mix trees",
            out.display()
        ));
    }
    let layout = Layout::new(&out);
    layout
        .create_base_dirs()
        .map_err(|e| format!("mkdir {}: {e}", out.display()))?;

    let mut rng = Rng::new(cfg.seed);
    if !cfg.empty {
        write_profiles(&layout, cfg, &mut rng)?;
    }

    // Always open the store: migrations must run for --empty too, yielding a valid DB.
    let store = MetadataStore::with_path(&layout.metadata_db)
        .map_err(|e| format!("open metadata store: {e}"))?;
    if !cfg.empty {
        write_profile_metadata(&store, &layout, cfg, &mut rng)?;
        write_collections(&store, cfg)?;
        write_proton_catalog(&store, cfg)?;
        write_community(&store, &layout, cfg, &mut rng)?;
        write_images(&store, &layout, cfg)?;
        write_launch_log(&layout, cfg, &mut rng)?;
    }
    drop(store);

    write_manifest(&layout, cfg)?;

    crate::normalize::normalize_db(&layout.metadata_db, &layout, cfg.seed)?;
    eprintln!(
        "bench_fixtures: wrote {} (seed {}, {} profiles, {} community, {} proton rows, {} log lines) in {:?}",
        out.display(),
        cfg.seed,
        cfg.profiles,
        cfg.community_profiles,
        cfg.proton_rows,
        cfg.log_lines,
        started.elapsed()
    );
    Ok(())
}

fn profile_name(i: usize) -> String {
    format!("bench-game-{i:04}")
}

fn steam_app_id(i: usize) -> String {
    format!("{}", 9_900_000 + i)
}

fn write_profiles(layout: &Layout, cfg: &BenchConfig, rng: &mut Rng) -> Result<(), String> {
    let store = ProfileStore::with_base_path(layout.profiles_dir.clone());
    for i in 0..cfg.profiles {
        let name = profile_name(i);
        let app_id = steam_app_id(i);
        let method = if rng.below(2) == 0 {
            METHOD_STEAM_APPLAUNCH
        } else {
            METHOD_PROTON_RUN
        };

        let mut profile = GameProfile {
            game: GameSection {
                name: format!("Bench Game {i:04}"),
                executable_path: layout
                    .tokenize(&layout.data_dir.join("games").join(&name).join("game.exe")),
                ..GameSection::default()
            },
            trainer: TrainerSection {
                path: layout.tokenize(
                    &layout
                        .data_dir
                        .join("trainers")
                        .join(&name)
                        .join("trainer.exe"),
                ),
                kind: "fling".to_string(),
                ..TrainerSection::default()
            },
            steam: SteamSection {
                enabled: method == METHOD_STEAM_APPLAUNCH,
                app_id: app_id.clone(),
                ..SteamSection::default()
            },
            runtime: RuntimeSection {
                steam_app_id: app_id,
                proton_path: layout
                    .tokenize(&layout.data_dir.join("proton").join("GE-Proton9-bench")),
                ..RuntimeSection::default()
            },
            launch: LaunchSection {
                method: method.to_string(),
                ..LaunchSection::default()
            },
            ..GameProfile::default()
        };
        profile.launch.normalize_preset_selection();
        store
            .save(&name, &profile)
            .map_err(|e| format!("save profile {name}: {e}"))?;
    }
    Ok(())
}

fn write_profile_metadata(
    store: &MetadataStore,
    layout: &Layout,
    cfg: &BenchConfig,
    rng: &mut Rng,
) -> Result<(), String> {
    let profile_store = ProfileStore::with_base_path(layout.profiles_dir.clone());
    store
        .sync_profiles_from_store(&profile_store)
        .map_err(|e| format!("sync profiles: {e}"))?;

    let log_path = layout.tokenize(&layout.logs_dir.join("launch-50k.log"));
    for i in 0..cfg.profiles {
        let name = profile_name(i);
        let Some(profile_id) = store
            .lookup_profile_id(&name)
            .map_err(|e| format!("lookup profile id {name}: {e}"))?
        else {
            return Err(format!("profile row missing for {name}"));
        };

        // Health snapshot per profile, mixed statuses.
        let status = *rng.pick(&["healthy", "healthy", "stale", "broken"]);
        let issue_count = rng.below(5) as usize;
        store
            .upsert_health_snapshot(&profile_id, status, issue_count, EPOCH_ANCHOR)
            .map_err(|e| format!("health snapshot {name}: {e}"))?;

        // Launch history: one clean success, plus a recent failure for every fifth profile.
        let method = if i % 2 == 0 {
            METHOD_STEAM_APPLAUNCH
        } else {
            METHOD_PROTON_RUN
        };
        let op = store
            .record_launch_started(Some(&name), method, Some(&log_path))
            .map_err(|e| format!("launch start {name}: {e}"))?;
        store
            .record_launch_finished(&op, Some(0), None, &success_report(method, &log_path))
            .map_err(|e| format!("launch finish {name}: {e}"))?;

        if i % 5 == 0 {
            let op = store
                .record_launch_started(Some(&name), method, Some(&log_path))
                .map_err(|e| format!("failure start {name}: {e}"))?;
            store
                .record_launch_finished(&op, Some(1), None, &failure_report(method, &log_path))
                .map_err(|e| format!("failure finish {name}: {e}"))?;
        }
    }
    Ok(())
}

fn base_report(method: &str, log_path: &str) -> DiagnosticReport {
    DiagnosticReport {
        severity: ValidationSeverity::Info,
        summary: String::new(),
        exit_info: ExitCodeInfo {
            code: None,
            signal: None,
            signal_name: None,
            core_dumped: false,
            failure_mode: FailureMode::CleanExit,
            description: String::new(),
            severity: ValidationSeverity::Info,
        },
        pattern_matches: Vec::new(),
        suggestions: Vec::new(),
        launch_method: method.to_string(),
        log_tail_path: Some(log_path.to_string()),
        analyzed_at: EPOCH_ANCHOR.to_string(),
        teardown_reason: None,
        coexistence_advisories: Vec::new(),
    }
}

fn success_report(method: &str, log_path: &str) -> DiagnosticReport {
    let mut report = base_report(method, log_path);
    report.summary = "bench fixture: clean exit".to_string();
    report.exit_info.code = Some(0);
    report
}

fn failure_report(method: &str, log_path: &str) -> DiagnosticReport {
    let mut report = base_report(method, log_path);
    report.severity = ValidationSeverity::Warning;
    report.summary = "bench fixture: non-zero exit".to_string();
    report.exit_info.code = Some(1);
    report.exit_info.failure_mode = FailureMode::NonZeroExit;
    report.exit_info.description = "game process exited with status 1".to_string();
    report.exit_info.severity = ValidationSeverity::Warning;
    report
}

fn write_collections(store: &MetadataStore, cfg: &BenchConfig) -> Result<(), String> {
    let favorites = store
        .create_collection("Bench Favorites")
        .map_err(|e| format!("create collection: {e}"))?;
    let rpgs = store
        .create_collection("Bench RPGs")
        .map_err(|e| format!("create collection: {e}"))?;
    for i in 0..cfg.profiles {
        let name = profile_name(i);
        if i % 2 == 0 {
            store
                .add_profile_to_collection(&favorites, &name)
                .map_err(|e| format!("collection add {name}: {e}"))?;
        }
        if i % 4 == 0 {
            store
                .add_profile_to_collection(&rpgs, &name)
                .map_err(|e| format!("collection add {name}: {e}"))?;
        }
        if i % 10 == 0 {
            store
                .set_profile_favorite(&name, true)
                .map_err(|e| format!("favorite {name}: {e}"))?;
        }
    }
    Ok(())
}

fn write_proton_catalog(store: &MetadataStore, cfg: &BenchConfig) -> Result<(), String> {
    let providers = ["ge-proton", "proton-cachyos", "proton-em"];
    let scopes = ["stable", "prereleases"];
    let mut rows = Vec::with_capacity(cfg.proton_rows);
    for (n, i) in (0..cfg.proton_rows).enumerate() {
        let logical = providers[n % providers.len()];
        let scope = scopes[(n / providers.len()) % scopes.len()];
        let tag = format!("{logical}-9-{}-s{}", i, cfg.seed);
        let version = ProtonUpAvailableVersion {
            provider: logical.to_string(),
            version: tag.clone(),
            release_url: Some(format!("https://example.com/release/{tag}")),
            download_url: Some(format!("https://example.com/{tag}.tar.gz")),
            checksum_url: Some(format!("https://example.com/{tag}.sha512sum")),
            checksum_kind: Some("sha512".to_string()),
            asset_size: Some(400_000_000 + i as u64),
            published_at: Some(EPOCH_ANCHOR.to_string()),
        };
        rows.push(ProtonCatalogRow {
            provider_id: format!("{logical}:{scope}"),
            version_tag: tag,
            payload_json: serde_json::to_string(&version).expect("serialize proton payload"),
            release_url: version.release_url.clone(),
            download_url: version.download_url.clone(),
            checksum_url: version.checksum_url.clone(),
            checksum_kind: version.checksum_kind.clone(),
            asset_size: version.asset_size.map(|s| s as i64),
            fetched_at: EPOCH_ANCHOR.to_string(),
            expires_at: Some(FAR_FUTURE.to_string()),
        });
    }
    store
        .put_proton_catalog(&rows)
        .map_err(|e| format!("put proton catalog: {e}"))
}

fn write_community(
    store: &MetadataStore,
    layout: &Layout,
    cfg: &BenchConfig,
    rng: &mut Rng,
) -> Result<(), String> {
    let tap_dir = layout.community_taps_dir.join("bench-tap");
    let games_dir = tap_dir.join("games");
    std::fs::create_dir_all(&games_dir).map_err(|e| format!("mkdir tap games: {e}"))?;

    for i in 0..cfg.community_profiles {
        let manifest = CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: format!("Bench Community Game {i:04}"),
                game_version: format!("1.{}", i % 9),
                trainer_name: format!("FLiNG Trainer {i:04}"),
                trainer_version: format!("v{}.{}", i % 5, i % 7),
                proton_version: format!("GE-Proton9-{}", 1 + (i % 20)),
                platform_tags: vec!["linux".to_string(), "steam-deck".to_string()],
                compatibility_rating: [
                    CompatibilityRating::Platinum,
                    CompatibilityRating::Working,
                    CompatibilityRating::Partial,
                    CompatibilityRating::Broken,
                    CompatibilityRating::Unknown,
                ][i % 5]
                    .clone(),
                author: format!("bench-author-{}", i % 12),
                description: format!("Bench community profile {i:04} for seed {}", cfg.seed),
                trainer_sha256: Some(format!(
                    "{:016x}{:016x}{:016x}{:016x}",
                    rng.next_u64(),
                    rng.next_u64(),
                    rng.next_u64(),
                    rng.next_u64()
                )),
            },
            GameProfile::default(),
        );
        let dir = games_dir.join(format!("game-{i:04}"));
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir community entry: {e}"))?;
        let json = serde_json::to_string_pretty(&manifest).expect("serialize community manifest");
        std::fs::write(dir.join("community-profile.json"), json)
            .map_err(|e| format!("write community manifest: {e}"))?;
    }

    let workspace = CommunityTapWorkspace {
        subscription: CommunityTapSubscription {
            url: "https://github.com/crosshook-bench/taps".to_string(),
            branch: Some("main".to_string()),
            pinned_commit: None,
            extra: toml::Table::new(),
        },
        local_path: tap_dir,
    };
    let index: CommunityProfileIndex =
        crosshook_core::community::index::index_taps(std::slice::from_ref(&workspace))
            .map_err(|e| format!("index tap: {e}"))?;
    let result = CommunityTapSyncResult {
        workspace,
        status: CommunityTapSyncStatus::Cloned,
        head_commit: rng.hex40(),
        index,
        from_cache: false,
        last_sync_at: None,
    };
    store
        .index_community_tap_result(&result)
        .map_err(|e| format!("index community tap result: {e}"))
}

fn write_images(store: &MetadataStore, layout: &Layout, cfg: &BenchConfig) -> Result<(), String> {
    let content_hash = sha256_hex(PORTRAIT_JPEG);
    let file_size = PORTRAIT_JPEG.len() as i64;
    for i in 0..cfg.profiles {
        let app_id = steam_app_id(i);
        let dest = layout
            .image_cache_dir
            .join(&app_id)
            .join("portrait_steam_cdn.jpg");
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir image cache: {e}"))?;
        }
        std::fs::write(&dest, PORTRAIT_JPEG).map_err(|e| format!("write portrait: {e}"))?;
        let source_url = format!(
            "https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
        );
        store
            .upsert_game_image(
                &app_id,
                "portrait",
                "steam_cdn",
                &dest.to_string_lossy(),
                Some(file_size),
                Some(&content_hash),
                Some("image/jpeg"),
                Some(&source_url),
                Some(FAR_FUTURE),
            )
            .map_err(|e| format!("upsert game image {app_id}: {e}"))?;
    }
    Ok(())
}

/// 50k-line Proton/DXVK-shaped launch log. Content derives from the seeded PRNG only.
fn write_launch_log(layout: &Layout, cfg: &BenchConfig, rng: &mut Rng) -> Result<(), String> {
    if cfg.profiles == 0 && cfg.log_lines > 0 {
        return Err("launch log requires at least one profile".to_string());
    }
    let path = layout.logs_dir.join("launch-50k.log");
    let file = std::fs::File::create(&path).map_err(|e| format!("create launch log: {e}"))?;
    let mut w = std::io::BufWriter::new(file);
    let mut line = String::with_capacity(160);
    for n in 0..cfg.log_lines {
        line.clear();
        let minute = (n / 60) % 60;
        let second = n % 60;
        let ms = rng.below(1000);
        let game = rng.below(cfg.profiles as u64);
        let pid = 1000 + rng.below(9000);
        let kind = rng.below(4);
        let stamp = format!("{minute:02}:{second:02}.{ms:03}");
        let body = match kind {
            0 => format!(
                "Proton:emulation:: {stamp} - [info] GE-Proton9-bench pid={pid} game=\"bench-game-{game:04}\" wineserver ready"
            ),
            1 => format!(
                "Proton:emulation:: {stamp} - [info] dxvk: Game: game.exe monitor 1920x1080 (output 0)"
            ),
            2 => format!(
                "Proton:emulation:: {stamp} - [warn] dxvk: DxvkGraphicsPipeline: Compiling graphics pipeline (shaders={})",
                8 + rng.below(64)
            ),
            _ => format!(
                "Proton:emulation:: {stamp} - [info] mangohud: bench session frame metrics pid={pid} game={game:04}"
            ),
        };
        writeln!(w, "2024-01-01 12:{minute:02}:{second:02}.{ms:03} {body}")
            .map_err(|e| format!("write launch log: {e}"))?;
    }
    w.flush().map_err(|e| format!("flush launch log: {e}"))?;
    Ok(())
}

fn write_manifest(layout: &Layout, cfg: &BenchConfig) -> Result<(), String> {
    let mut env = std::collections::BTreeMap::new();
    env.insert("XDG_CONFIG_HOME", "config");
    env.insert("XDG_DATA_HOME", "data");
    env.insert("XDG_CACHE_HOME", "cache");
    env.insert("XDG_STATE_HOME", "state");
    env.insert("HOME", "home");
    let mut files = std::collections::BTreeMap::new();
    files.insert("profiles", "config/crosshook/profiles");
    files.insert("metadata_db", "data/crosshook/metadata.db");
    files.insert("image_cache", "data/crosshook/cache/images");
    files.insert("community_taps", "data/crosshook/community/taps");
    files.insert("launch_log", "logs/launch-50k.log");
    let mut counts = std::collections::BTreeMap::new();
    counts.insert("profiles", cfg.profiles);
    counts.insert("community_profiles", cfg.community_profiles);
    counts.insert("proton_catalog_rows", cfg.proton_rows);
    counts.insert("launch_log_lines", cfg.log_lines);
    let manifest = ManifestFile {
        schema_version: 1,
        generator: "bench_fixtures",
        seed: cfg.seed,
        empty: cfg.empty,
        token: TOKEN,
        env,
        layout: files,
        counts,
        log: "logs/launch-50k.log",
    };
    let json = serde_json::to_string_pretty(&manifest).expect("serialize bench manifest");
    std::fs::write(&layout.manifest, json).map_err(|e| format!("write manifest: {e}"))
}
