/// Variables cleared before launching a trainer via Proton to prevent host-session bleed.
///
/// The runtime helper shell scripts maintain a parallel unset list for the
/// `steam_applaunch` path. Those scripts also unset `WINEPREFIX` (which the
/// Rust path sets rather than clearing). Keep both lists in sync — see the
/// "Keep in sync" comments in the shell scripts.
pub const WINE_ENV_VARS_TO_CLEAR: &[&str] = &[
    "WINESERVER",
    "WINELOADER",
    "WINEDLLPATH",
    "WINEDLLOVERRIDES",
    "WINEDEBUG",
    "WINEESYNC",
    "WINEFSYNC",
    "WINELOADERNOEXEC",
    "WINE_LARGE_ADDRESS_AWARE",
    "WINE_DISABLE_KERNEL_WRITEWATCH",
    "WINE_HEAP_DELAY_FREE",
    "WINEFSYNC_SPINCOUNT",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "GST_PLUGIN_PATH",
    "GST_PLUGIN_SYSTEM_PATH",
    "GST_PLUGIN_SYSTEM_PATH_1_0",
    "SteamGameId",
    "SteamAppId",
    "GAMEID", // Cleared for direct Proton; set per-command when umu-run is active
    "PROTON_LOG",
    "PROTON_DUMP_DEBUG_COMMANDS",
    "PROTON_USE_WINED3D",
    "PROTON_NO_ESYNC",
    "PROTON_NO_FSYNC",
    "PROTON_ENABLE_NVAPI",
    "PROTON_VERB", // Cleared for direct Proton; set per-command by builders (runinprefix for trainers, waitforexitandrun for games).
    "DXVK_CONFIG_FILE",
    "DXVK_STATE_CACHE_PATH",
    "DXVK_LOG_PATH",
    "VKD3D_CONFIG",
    "VKD3D_DEBUG",
    "STEAM_COMPAT_LIBRARY_PATHS", // Cleared for direct Proton; set per-command by builders (pressure-vessel RW allowlist).
    "PRESSURE_VESSEL_FILESYSTEMS_RW", // Cleared for direct Proton; set per-command by builders (pressure-vessel RW allowlist, paired with STEAM_COMPAT_LIBRARY_PATHS).
    "PROTONPATH", // Cleared for direct Proton; set per-command by builders when umu-run is active.
];

#[cfg(test)]
mod tests {
    use super::WINE_ENV_VARS_TO_CLEAR;

    #[test]
    fn wine_env_vars_match_expected_list() {
        assert_eq!(WINE_ENV_VARS_TO_CLEAR.len(), 35);
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"WINESERVER"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"WINE_HEAP_DELAY_FREE"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PROTON_ENABLE_NVAPI"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"VKD3D_DEBUG"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PROTON_VERB"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"STEAM_COMPAT_LIBRARY_PATHS"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PRESSURE_VESSEL_FILESYSTEMS_RW"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PROTONPATH"));
    }
}
