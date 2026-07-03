use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::super::detection::{
    mark_already_registered, scan_game_directory, DetectedModCandidate, DetectionScanReport,
    ModDetectionError, MAX_SCAN_ENTRIES, PROXY_DLL_NAMES,
};
use super::super::model::{ModCategory, ModProvenance, ProfileModRecord};

fn touch(root: &Path, name: &str) {
    fs::write(root.join(name), b"").unwrap();
}

fn scan(root: &Path) -> DetectionScanReport {
    scan_game_directory(root).unwrap()
}

fn find<'a>(report: &'a DetectionScanReport, detector_id: &str) -> &'a DetectedModCandidate {
    report
        .candidates
        .iter()
        .find(|candidate| candidate.detector_id == detector_id)
        .unwrap_or_else(|| {
            panic!(
                "candidate {detector_id} not found in {:?}",
                report.candidates
            )
        })
}

#[test]
fn empty_dir_yields_no_candidates_and_counts_entries() {
    let dir = TempDir::new().unwrap();
    let report = scan(dir.path());

    assert!(report.candidates.is_empty());
    assert_eq!(report.entries_scanned, 0);
    assert!(!report.truncated);
    assert_eq!(report.scanned_root, dir.path().to_string_lossy());
}

#[test]
fn proxy_dll_in_root_detected() {
    for dll in PROXY_DLL_NAMES {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), dll);

        let report = scan(dir.path());
        assert_eq!(
            report.candidates.len(),
            1,
            "one candidate expected for {dll}"
        );
        let candidate = find(&report, &format!("proxy_dll:{dll}"));
        assert_eq!(candidate.category, ModCategory::OverlayInjection);
        assert_eq!(candidate.suggested_name, format!("Proxy DLL ({dll})"));
        assert_eq!(candidate.matched_paths, vec![dll.to_string()]);
        assert!(!candidate.already_registered);
    }

    // Matching is case-insensitive; the detector id is normalized lowercase.
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "DXGI.DLL");
    let report = scan(dir.path());
    let candidate = find(&report, "proxy_dll:dxgi.dll");
    assert_eq!(candidate.matched_paths, vec!["DXGI.DLL".to_string()]);
}

#[test]
fn reshade_ini_detected() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "ReShade.ini");

    let report = scan(dir.path());
    let candidate = find(&report, "reshade");
    assert_eq!(candidate.suggested_name, "ReShade");
    assert_eq!(candidate.category, ModCategory::OverlayInjection);
    assert_eq!(candidate.matched_paths, vec!["ReShade.ini".to_string()]);
}

#[test]
fn reshade_shaders_dir_detected() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("reshade-shaders")).unwrap();

    let report = scan(dir.path());
    let candidate = find(&report, "reshade");
    assert_eq!(candidate.matched_paths, vec!["reshade-shaders".to_string()]);
}

#[test]
fn enb_markers_detected() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("enbseries")).unwrap();
    touch(dir.path(), "enblocal.ini");

    let report = scan(dir.path());
    assert_eq!(report.candidates.len(), 1);
    let candidate = find(&report, "enb");
    assert_eq!(candidate.suggested_name, "ENB Series");
    assert_eq!(candidate.category, ModCategory::OverlayInjection);
    assert_eq!(
        candidate.matched_paths,
        vec!["enblocal.ini".to_string(), "enbseries".to_string()]
    );
}

#[test]
fn reshade_absorbs_proxy_dlls_no_separate_candidate() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "ReShade.ini");
    touch(dir.path(), "dxgi.dll");

    let report = scan(dir.path());
    assert_eq!(report.candidates.len(), 1);
    let candidate = find(&report, "reshade");
    assert!(candidate.matched_paths.contains(&"dxgi.dll".to_string()));
    assert!(!report
        .candidates
        .iter()
        .any(|c| c.detector_id.starts_with("proxy_dll:")));
}

#[test]
fn enb_absorbs_proxy_dlls_when_no_reshade() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "enbseries.ini");
    touch(dir.path(), "version.dll");

    let report = scan(dir.path());
    assert_eq!(report.candidates.len(), 1);
    let candidate = find(&report, "enb");
    assert!(candidate.matched_paths.contains(&"version.dll".to_string()));
    assert!(!report
        .candidates
        .iter()
        .any(|c| c.detector_id.starts_with("proxy_dll:")));
}

#[test]
fn skse64_loader_and_companion_dlls_grouped() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "skse64_loader.exe");
    touch(dir.path(), "skse64_1_5_97.dll");
    touch(dir.path(), "skse64_steam_loader.dll");
    fs::create_dir_all(dir.path().join("Data").join("SKSE")).unwrap();

    let report = scan(dir.path());
    assert_eq!(report.candidates.len(), 1);
    let candidate = find(&report, "script_extender:skse64");
    assert_eq!(candidate.suggested_name, "SKSE64");
    assert_eq!(candidate.category, ModCategory::ScriptExtender);
    assert_eq!(
        candidate.matched_paths,
        vec![
            "Data/SKSE".to_string(),
            "skse64_1_5_97.dll".to_string(),
            "skse64_loader.exe".to_string(),
            "skse64_steam_loader.dll".to_string(),
        ]
    );
}

#[test]
fn data_skse_dir_without_loader_not_reported() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("Data").join("SKSE")).unwrap();

    let report = scan(dir.path());
    assert!(
        report.candidates.is_empty(),
        "a Data/SKSE dir alone is too weak a signal: {:?}",
        report.candidates
    );
}

#[test]
fn symlinked_marker_file_skipped() {
    let outside = TempDir::new().unwrap();
    touch(outside.path(), "real-reshade.ini");

    let dir = TempDir::new().unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("real-reshade.ini"),
        dir.path().join("ReShade.ini"),
    )
    .unwrap();

    let report = scan(dir.path());
    assert!(
        report.candidates.is_empty(),
        "symlinked markers must be skipped"
    );
    assert_eq!(report.entries_scanned, 1, "the symlink still costs budget");
}

#[test]
fn entry_budget_truncates_and_returns_partial() {
    let dir = TempDir::new().unwrap();
    for index in 0..(MAX_SCAN_ENTRIES + 8) {
        touch(dir.path(), &format!("filler-{index}.txt"));
    }

    let report = scan(dir.path());
    assert!(report.truncated);
    assert_eq!(report.entries_scanned, MAX_SCAN_ENTRIES);
}

#[test]
fn missing_root_is_root_unreadable_error() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("does-not-exist");

    let error = scan_game_directory(&missing).unwrap_err();
    match &error {
        ModDetectionError::RootUnreadable { path, .. } => {
            assert_eq!(path, &missing.to_string_lossy());
        }
    }
    assert!(error.to_string().contains("game directory is not readable"));
}

fn registered_mod(name: &str, category: ModCategory, paths: &[&str]) -> ProfileModRecord {
    ProfileModRecord {
        mod_id: format!("mod-{name}"),
        profile_id: "profile-1".to_string(),
        name: name.to_string(),
        category,
        paths: paths.iter().map(ToString::to_string).collect(),
        enabled: true,
        provenance: ModProvenance::Manual,
        source_url: None,
        created_at: "2026-01-01T00:00:00+00:00".to_string(),
        updated_at: "2026-01-01T00:00:00+00:00".to_string(),
    }
}

fn report_with_candidates(candidates: Vec<DetectedModCandidate>) -> DetectionScanReport {
    DetectionScanReport {
        scanned_root: "/games/skyrim".to_string(),
        candidates,
        entries_scanned: 4,
        truncated: false,
    }
}

fn overlay_candidate(detector_id: &str, name: &str, paths: &[&str]) -> DetectedModCandidate {
    DetectedModCandidate {
        detector_id: detector_id.to_string(),
        suggested_name: name.to_string(),
        category: ModCategory::OverlayInjection,
        matched_paths: paths.iter().map(ToString::to_string).collect(),
        already_registered: false,
    }
}

#[test]
fn enb_registration_does_not_suppress_distinct_reshade_candidate() {
    let mut report = report_with_candidates(vec![overlay_candidate(
        "reshade",
        "ReShade",
        &["ReShade.ini", "reshade-shaders"],
    )]);
    let registered = [registered_mod(
        "ENB Series",
        ModCategory::OverlayInjection,
        &["enblocal.ini", "enbseries", "d3d11.dll"],
    )];

    mark_already_registered(&mut report, &registered, Path::new("/games/skyrim"));
    assert!(
        !report.candidates[0].already_registered,
        "a different mod with no shared paths must not suppress the candidate"
    );
}

#[test]
fn different_category_with_shared_path_does_not_suppress_candidate() {
    let mut report = report_with_candidates(vec![overlay_candidate(
        "reshade",
        "ReShade",
        &["ReShade.ini", "dxgi.dll"],
    )]);
    let registered = [registered_mod(
        "Texture Pack",
        ModCategory::FileReplacement,
        &["dxgi.dll"],
    )];

    mark_already_registered(&mut report, &registered, Path::new("/games/skyrim"));
    assert!(
        !report.candidates[0].already_registered,
        "shared paths only count within the same category"
    );
}

#[test]
fn absolute_registered_path_under_game_dir_suppresses_candidate() {
    let mut report = report_with_candidates(vec![overlay_candidate(
        "reshade",
        "ReShade",
        &["ReShade.ini"],
    )]);
    let registered = [registered_mod(
        "ReShade (custom preset)",
        ModCategory::OverlayInjection,
        &["/games/skyrim/reshade.INI"],
    )];

    mark_already_registered(&mut report, &registered, Path::new("/games/skyrim"));
    assert!(
        report.candidates[0].already_registered,
        "absolute registered paths under the scanned root must match root-relative"
    );
}

#[test]
fn absolute_registered_path_outside_game_dir_does_not_match() {
    let mut report = report_with_candidates(vec![overlay_candidate(
        "reshade",
        "ReShade",
        &["ReShade.ini"],
    )]);
    let registered = [registered_mod(
        "ReShade (other install)",
        ModCategory::OverlayInjection,
        &["/games/oblivion/ReShade.ini"],
    )];

    mark_already_registered(&mut report, &registered, Path::new("/games/skyrim"));
    assert!(!report.candidates[0].already_registered);
}

#[test]
fn renamed_registration_with_same_category_and_paths_suppresses_candidate() {
    let mut report = report_with_candidates(vec![overlay_candidate(
        "reshade",
        "ReShade",
        &["ReShade.ini", "dxgi.dll"],
    )]);
    let registered = [registered_mod(
        "My Post-Processing Stack",
        ModCategory::OverlayInjection,
        &["dxgi.dll"],
    )];

    mark_already_registered(&mut report, &registered, Path::new("/games/skyrim"));
    assert!(
        report.candidates[0].already_registered,
        "same category + overlapping path must suppress even after a rename"
    );
}

#[test]
fn case_insensitive_name_match_suppresses_candidate() {
    let mut report = report_with_candidates(vec![overlay_candidate(
        "reshade",
        "ReShade",
        &["ReShade.ini"],
    )]);
    let registered = [registered_mod("reshade", ModCategory::Other, &[])];

    mark_already_registered(&mut report, &registered, Path::new("/games/skyrim"));
    assert!(report.candidates[0].already_registered);
}

#[test]
fn matched_paths_are_relative_and_sorted() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "ReShade.ini");
    fs::create_dir(dir.path().join("reshade-shaders")).unwrap();
    touch(dir.path(), "dxgi.dll");

    let report = scan(dir.path());
    let candidate = find(&report, "reshade");
    assert_eq!(
        candidate.matched_paths,
        vec![
            "ReShade.ini".to_string(),
            "dxgi.dll".to_string(),
            "reshade-shaders".to_string(),
        ]
    );
    assert!(
        candidate.matched_paths.iter().all(|p| !p.starts_with('/')),
        "matched paths must be relative to the scan root"
    );
}
