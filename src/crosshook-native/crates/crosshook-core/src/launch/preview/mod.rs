mod builder;
mod command;
mod details;
mod display;
mod environment;
mod sections;
mod types;

#[cfg(test)]
mod tests;

pub use builder::build_launch_preview;
pub use types::{
    EnvVarSource, GamescopeDecisionPreview, LaunchPreview, PreviewEnvVar, PreviewTrainerInfo,
    PreviewValidation, PreviewWrapperDetail, PreviewWrapperSource, ProtonSetup,
    ResolvedLaunchMethod, UmuDecisionPreview,
};
