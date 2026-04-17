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
//! Both modules expose pure decision helpers that are testable without a
//! live D-Bus connection; the D-Bus side lives behind a trait seam so
//! `#[cfg(test)]` fakes can be injected.

pub mod background;
pub mod gamemode;
