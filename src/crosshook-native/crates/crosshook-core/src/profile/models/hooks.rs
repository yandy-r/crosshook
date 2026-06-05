use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

/// When a launch hook fires relative to the game lifecycle.
///
/// Serializes kebab-case: `"pre-launch"` / `"post-exit"` — this exact wire
/// format is the Phase 6 stage-pill contract (issue #471).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/launch_hooks.ts"))]
pub enum HookStage {
    /// Runs before the launch command is executed.
    #[default]
    PreLaunch,
    /// Runs after the game process exits.
    PostExit,
}

/// A user-declared script invoked around the launch lifecycle.
///
/// - `id` is an opaque client-minted identifier (frontend `crypto.randomUUID()`
///   at attach time); the backend never mints or interprets it.
/// - `path` is a host-side absolute path. Per ADR-0001's scope boundary it is a
///   user variable (not a denylisted tool name); execution applies
///   `normalize_flatpak_host_path` and routes through the host gateway.
/// - `stage` mirrors the containing profile vec (`pre_launch_hooks` /
///   `post_exit_hooks`), which is authoritative. Producers keep them aligned, and
///   [`GameProfile::normalize_hooks`](crate::profile::GameProfile::normalize_hooks)
///   re-derives `stage` from the container on every load/import so a mismatched
///   serialized value can never persist. The same step drops entries with an
///   empty `id` (identity-less hooks are unusable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/launch_hooks.ts"))]
pub struct LaunchHook {
    pub id: String,
    pub name: String,
    pub path: String,
    pub stage: HookStage,
    pub enabled: bool,
}
