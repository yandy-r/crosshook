//! Test-only [`TapGitRunner`] that executes the driver's invocations with real
//! `git` through the host gateway. The production runner lives in
//! `crosshook-app` (supervised, cancellable); core keeps no process executor.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;

use crate::platform::host_command_with_env_and_directory;

use super::runner::{GitInvocation, GitOutput, GitRunError, TapGitRunner};

pub(super) struct RealGitRunner;

impl TapGitRunner for RealGitRunner {
    fn run<'a>(
        &'a self,
        invocation: GitInvocation,
    ) -> Pin<Box<dyn Future<Output = Result<GitOutput, GitRunError>> + Send + 'a>> {
        Box::pin(async move {
            let directory = invocation
                .directory
                .as_deref()
                .map(|path| path.to_string_lossy().into_owned());
            let mut command = host_command_with_env_and_directory(
                "git",
                &invocation.env,
                directory.as_deref(),
                &BTreeMap::new(),
            );
            command.args(&invocation.args).stdin(Stdio::null());
            let output = command.output().await.map_err(GitRunError::Failed)?;
            Ok(GitOutput {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        })
    }
}
