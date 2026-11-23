//! Launch view-models: the prefix dependency gate, the launch pipeline and
//! its detail panel, and the validation-issue to pipeline-node mapping.
//!
//! Parity oracle: React `useLaunchDepGate.ts`, `derivePipelineNodes.ts`,
//! `derivePipelineDetail.ts`, `mapValidationToNode.ts` and
//! `launchPreviewPresentation.ts`. Classic keeps its TypeScript copies; the
//! committed fixtures under `tests/fixtures/ts_parity/` freeze the behaviour
//! both must match (see the crate `README.md`, § _Launch_).
//!
//! All state is runtime-only (nothing persists).

#[cfg(feature = "app")]
mod app;
pub mod dep_gate;
pub mod pipeline;
pub mod pipeline_detail;
pub mod preview;
pub mod validation_map;

#[cfg(test)]
mod parity_tests;

pub use dep_gate::{
    DepGate, DependencyState, DependencyStatus, GateEffect, GateIntent, GateScope, GateState,
    InstallCompletion, OpId, RepairStatus, RestoreState,
};
pub use pipeline::{
    derive_pipeline, PipelineNode, PipelineNodeId, PipelineNodeStatus, PipelineTone,
};
pub use preview::{LaunchPreview, PipelineProfile};

/// Launch method after inference (`resolved_method` in core).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum LaunchMethod {
    /// Proton run outside Steam (`proton_run`).
    ProtonRun,
    /// Launched through Steam (`steam_applaunch`).
    SteamApplaunch,
    /// Native Linux executable (`native`).
    Native,
}

/// Live launch phase of the current session, as the launch panel tracks it.
///
/// An exited session (successful or not) is back to [`LaunchPhase::Idle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub enum LaunchPhase {
    /// Nothing is launching or running.
    #[default]
    Idle,
    /// The game process is starting.
    GameLaunching,
    /// The game runs and the trainer has not started yet.
    WaitingForTrainer,
    /// The trainer process is starting.
    TrainerLaunching,
    /// The session runs.
    SessionActive,
}

/// What the user asked to launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LaunchAction {
    /// The game.
    Game,
    /// The trainer.
    Trainer,
}
