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
    "GAMEID",
    "PROTON_LOG",
    "PROTON_DUMP_DEBUG_COMMANDS",
    "PROTON_USE_WINED3D",
    "PROTON_NO_ESYNC",
    "PROTON_NO_FSYNC",
    "PROTON_ENABLE_NVAPI",
    "DXVK_CONFIG_FILE",
    "DXVK_STATE_CACHE_PATH",
    "DXVK_LOG_PATH",
    "VKD3D_CONFIG",
    "VKD3D_DEBUG",
];

pub const REQUIRED_PROTON_VARS: &[&str] = &[
    "STEAM_COMPAT_DATA_PATH",
    "STEAM_COMPAT_CLIENT_INSTALL_PATH",
    "WINEPREFIX",
];

pub const PASSTHROUGH_DISPLAY_VARS: &[&str] = &[
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

#[cfg(test)]
mod tests {
    use super::{PASSTHROUGH_DISPLAY_VARS, REQUIRED_PROTON_VARS, WINE_ENV_VARS_TO_CLEAR};

    #[test]
    fn wine_env_vars_match_expected_list() {
        assert_eq!(WINE_ENV_VARS_TO_CLEAR.len(), 31);
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"WINESERVER"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"WINE_HEAP_DELAY_FREE"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PROTON_ENABLE_NVAPI"));
        assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"VKD3D_DEBUG"));
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
