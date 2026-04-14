/// Variables cleared before launching a trainer via Proton to prevent host-session bleed.
///
/// The runtime helper shell scripts maintain a parallel unset list for the
/// `steam_applaunch` path. Those scripts also unset `WINEPREFIX` (which is in
/// `REQUIRED_PROTON_VARS` here, not this list, because the Rust path sets it
/// rather than clearing it). Keep both lists in sync — see the "Keep in sync"
/// comments in the shell scripts.
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
];

pub const REQUIRED_PROTON_VARS: &[&str] = &[
    "STEAM_COMPAT_DATA_PATH",
    "STEAM_COMPAT_CLIENT_INSTALL_PATH",
    "WINEPREFIX",
];

/// Builtin set of env vars used by the default optimization catalog.
///
/// The runtime allowlist is `global_catalog().allowed_env_keys` — this constant
/// is kept as a compile-time reference for env-clearing code and test isolation.
pub const BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS: &[&str] = &[
    "PROTON_NO_STEAMINPUT",
    "PROTON_PREFER_SDL",
    "PROTON_NO_WM_DECORATION",
    "PROTON_ENABLE_HDR",
    "PROTON_ENABLE_WAYLAND",
    "PROTON_USE_NTSYNC",
    "PROTON_NO_ESYNC",
    "PROTON_NO_FSYNC",
    "PROTON_ENABLE_NVAPI",
    "PROTON_FORCE_LARGE_ADDRESS_AWARE",
    "PROTON_LOG",
    "PROTON_LOCAL_SHADER_CACHE",
    "DXVK_ASYNC",
    "DXVK_FRAME_RATE",
    "VKD3D_CONFIG",
    "PROTON_FSR4_UPGRADE",
    "PROTON_FSR4_RDNA3_UPGRADE",
    "PROTON_XESS_UPGRADE",
    "PROTON_DLSS_UPGRADE",
    "PROTON_DLSS_INDICATOR",
    "PROTON_NVIDIA_LIBS",
    "SteamDeck",
];

pub const PASSTHROUGH_DISPLAY_VARS: &[&str] = &[
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

#[cfg(test)]
mod tests {
    use super::{
        BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS, PASSTHROUGH_DISPLAY_VARS, REQUIRED_PROTON_VARS,
        WINE_ENV_VARS_TO_CLEAR,
    };

    #[test]
    fn wine_env_vars_match_expected_list() {
        assert_eq!(WINE_ENV_VARS_TO_CLEAR.len(), 34);
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"WINESERVER"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"WINE_HEAP_DELAY_FREE"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PROTON_ENABLE_NVAPI"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"VKD3D_DEBUG"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PROTON_VERB"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"STEAM_COMPAT_LIBRARY_PATHS"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PRESSURE_VESSEL_FILESYSTEMS_RW"));
    }

    #[test]
    fn proton_vars_match_expected_list() {
        assert_eq!(
            REQUIRED_PROTON_VARS,
            &[
                "STEAM_COMPAT_DATA_PATH",
                "STEAM_COMPAT_CLIENT_INSTALL_PATH",
                "WINEPREFIX",
            ]
        );
    }

    #[test]
    fn launch_optimization_vars_match_expected_list() {
        assert_eq!(BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS.len(), 22);
        assert!(BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS.contains(&"PROTON_NO_STEAMINPUT"));
        assert!(BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS.contains(&"PROTON_NO_ESYNC"));
        assert!(BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS.contains(&"PROTON_FORCE_LARGE_ADDRESS_AWARE"));
        assert!(BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS.contains(&"DXVK_ASYNC"));
        assert!(BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS.contains(&"VKD3D_CONFIG"));
        assert!(BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS.contains(&"PROTON_ENABLE_HDR"));
        assert!(BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS.contains(&"SteamDeck"));
    }

    #[test]
    fn display_vars_match_expected_list() {
        assert_eq!(
            PASSTHROUGH_DISPLAY_VARS,
            &[
                "DISPLAY",
                "WAYLAND_DISPLAY",
                "XDG_RUNTIME_DIR",
                "DBUS_SESSION_BUS_ADDRESS",
            ]
        );
    }
}
