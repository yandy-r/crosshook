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
//! - `documents` — `org.freedesktop.portal.Documents.GetHostPaths` (crate
//!   internal) mapping a document-portal pick (`/run/user/<uid>/doc/…` or
//!   `/run/flatpak/doc/…`) back to its host path when the
//!   `user.document-portal.host-path` xattr is missing. Read-only, no
//!   permission prompt, 2 s timeout.
//!
//! `background_supported` is unit-testable. The D-Bus entry points
//! (`probe_and_register_via_portal`, `request_background`) are guarded by
//! `is_flatpak()` and need a live session bus.

pub mod background;
pub(crate) mod documents;
pub mod gamemode;
