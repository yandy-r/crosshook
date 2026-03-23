# v0.2.0

CrossHook Native is the first full Linux-native release of CrossHook.

This release replaces the legacy Windows-first loader flow with a native desktop application built on Rust, Tauri v2, and React. CrossHook itself now runs natively on Linux and Steam Deck; only the games and trainers continue to run under Proton or WINE when needed.

## Highlights

- Native Linux desktop app distributed as an AppImage
- Steam Deck-friendly UI with controller/gamepad navigation
- Three launch modes: `steam_applaunch`, `proton_run`, and `native`
- Steam auto-populate for game detection, App ID lookup, compatdata/prefix paths, and Proton discovery
- Shared Proton install picker with readable detected versions and manual override support
- TOML-based profiles and settings, including recent files and last-used profile tracking
- Real-time launch console and structured logging
- Community taps for discovering, importing, and sharing profiles
- Compatibility viewer for community profile metadata
- Launcher export as shell scripts and `.desktop` entries
- Reproducible containerized AppImage builds for local and CI release workflows

## Native UI And Launching

- Replaced the old WinForms workflow with a native Linux UI built for desktop and handheld use
- Added explicit runner selection so Steam, Proton, and native Linux launches each expose only the relevant fields
- Added method-specific validation for Steam-backed launches, direct Proton launches, and native Linux executables
- Improved Proton path selection by listing discovered installs while preserving manual edit/browse flows
- Added startup auto-load behavior, settings management, and recent-profile handling in the native shell

## Steam And Proton Integration

- Detects Steam roots, library folders, app manifests, compatdata locations, and Proton installs
- Supports both Steam-managed launches and direct Proton launches for non-Steam setups
- Normalizes prefix-path labeling across runner modes for clearer setup
- Preserves manual control when auto-detection is incomplete or ambiguous

## Community And Export Features

- Added git-backed community taps and manifest indexing
- Added native import/export between local TOML profiles and community profile exchange data
- Added an in-app browser for community profiles and compatibility data
- Added launcher export for standalone scripts and desktop entries

## Packaging And Distribution

- CrossHook is now shipped as a Linux AppImage instead of a Windows binary
- Release automation builds the native app, runs `crosshook-core` tests, and uploads the versioned AppImage artifact
- Local builds also produce a stable alias AppImage for launchers and Steam shortcuts

## Removed And Replaced

- Legacy WinForms UI
- C# / .NET application runtime
- Windows-only distribution artifacts
- The old assumption that CrossHook itself must run under Proton or WINE

## Notes

- This is a major architectural rewrite rather than a small incremental patch
- Existing release tags should match the native workspace version so artifact names and embedded app metadata stay aligned

Thanks to everyone helping validate the native transition.
