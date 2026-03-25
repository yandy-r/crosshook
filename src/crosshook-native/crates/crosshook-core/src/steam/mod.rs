//! Steam discovery and auto-populate foundations.

pub mod auto_populate;
pub mod diagnostics;
pub mod discovery;
pub mod libraries;
pub mod manifest;
mod models;
pub mod proton;
pub mod vdf;

pub use auto_populate::attempt_auto_populate;
pub use diagnostics::DiagnosticCollector;
pub use discovery::discover_steam_root_candidates;
pub use models::{
    ProtonInstall, SteamAutoPopulateFieldState, SteamAutoPopulateRequest, SteamAutoPopulateResult,
    SteamGameMatch, SteamLibrary,
};
pub use proton::discover_compat_tools;
