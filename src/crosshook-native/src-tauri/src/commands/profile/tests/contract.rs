use crate::commands::profile::*;

#[test]
fn command_names_match_expected_ipc_contract() {
    // Async commands cannot be cast to function pointers; pin their exported names.
    // Lifecycle
    let _ = profile_list;
    let _ = profile_load;
    let _ = profile_list_summaries;
    let _ = profile_save;
    let _ = profile_delete;
    let _ = profile_duplicate;
    let _ = profile_rename;
    let _ = profile_import_legacy;
    let _ = profile_export_toml;
    // Favorites
    let _ = profile_set_favorite;
    let _ = profile_list_favorites;
    // Optimizations, sections and presets
    let _ = profile_save_launch_optimizations;
    let _ = profile_save_mangohud_config;
    let _ = profile_save_gamescope_config;
    let _ = profile_save_trainer_gamescope_config;
    let _ = profile_list_bundled_optimization_presets;
    let _ = profile_apply_bundled_optimization_preset;
    let _ = profile_save_manual_optimization_preset;
    // Command arguments
    let _ = profile_save_command_arguments;
    // Config history
    let _ = profile_config_history;
    let _ = profile_config_diff;
    let _ = profile_config_rollback;
    let _ = profile_mark_known_good;
}
