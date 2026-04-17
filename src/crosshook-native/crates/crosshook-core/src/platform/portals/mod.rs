//! Flatpak desktop-portal integrations.
//!
//! This module is additive to ADR-0001 (host-command gateway). ADR-0002 in
//! `docs/architecture/adr-0002-flatpak-portal-contracts.md` describes the
//! contracts in detail. Current portals:
//!
//! - [`gamemode`] — `org.freedesktop.portal.GameMode` for registering
//!   CrossHook's own sandbox-side PID with the host's `gamemoded`. Host
//!   games continue to use `gamemoderun` via `platform::host_command*`.
//! - [`background`] — `org.freedesktop.portal.Background.RequestBackground`
//!   to keep CrossHook running (and its `gamescope_watchdog` alive) while
//!   the window is minimized during long game sessions.
//!
//! Pure decision helpers (`resolve_backend`, `background_supported`) are
//! unit-testable; the D-Bus entry points (`portal_available`,
//! `request_background`) are guarded by `is_flatpak()` and documented as
//! requiring a live session bus.

pub mod background;
pub mod gamemode;
