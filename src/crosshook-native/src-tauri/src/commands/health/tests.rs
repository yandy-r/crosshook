use super::*;

#[test]
fn command_names_match_expected_ipc_contract() {
    // Async commands cannot be cast to function pointers; pin their exported names.
    let _ = batch_validate_profiles;
    let _ = get_profile_health;
    let _ = get_cached_health_snapshots;
    let _ = get_cached_offline_readiness_snapshots;
}
