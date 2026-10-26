//! Read-only launch commands, session reset and running-profile queries
//! (Task 2.6).
//!
//! Everything here is either pure ([`LaunchService::validate`],
//! [`LaunchService::steam_launch_options_command`]) or in-memory
//! ([`LaunchService::reset_sessions`], [`LaunchService::running_profiles`]) and
//! therefore synchronous (S7). [`LaunchService::preview`] and
//! [`LaunchService::launch_history`] touch the stores; store I/O runs through
//! [`run_blocking`].

use std::collections::BTreeMap;

use crosshook_core::launch::{
    self, LaunchPreview, LaunchRequest, LaunchValidationIssue, METHOD_STEAM_APPLAUNCH, SessionKind,
    TeardownReason, build_launch_preview, escape_steam_token, resolve_command_arguments_for_method,
    session_profile_key_for_name,
};
use crosshook_core::metadata::{LaunchHistoryEntry, MAX_HISTORY_LIST_LIMIT, MetadataStore};
use crosshook_core::profile::GamescopeConfig;
use crosshook_core::settings::{AppSettingsData, SettingsStore};
use crosshook_core::umu_database;

use super::warnings::collect_mod_coexistence_warnings;
use super::{LaunchService, run_blocking};
use crate::AppError;

/// Default number of history rows returned by [`LaunchService::launch_history`]
/// when the caller passes no limit (the Tauri command's default).
const DEFAULT_HISTORY_LIST_LIMIT: u32 = 20;

/// Populates `req.resolved_umu_game_id` from settings and the umu database.
///
/// Settings default on a load error; the lookup itself is fail-soft (a failed
/// or disabled lookup resolves to the fallback GAMEID, never an error).
pub(super) async fn enrich_umu_gameid(
    settings: &SettingsStore,
    metadata: &MetadataStore,
    req: &mut LaunchRequest,
) {
    let loaded = settings
        .load()
        .unwrap_or_else(|_| AppSettingsData::default());
    req.resolved_umu_game_id =
        Some(umu_database::resolve_umu_game_id(req, loaded.umu_database_lookup, metadata).await);
}

impl LaunchService {
    /// Validates a launch request, returning the first issue.
    ///
    /// # Errors
    /// [`LaunchValidationIssue`] describing the first failing rule.
    pub fn validate(&self, req: &LaunchRequest) -> Result<(), LaunchValidationIssue> {
        launch::validate(req).map_err(|error| error.issue())
    }

    /// Builds the dry-run preview: UMU enrichment first, then the core
    /// preview, then **only** the mod-coexistence warnings appended to
    /// `preview.validation.issues` (I-16 — never low-disk, offline or
    /// trainer-hash warnings).
    ///
    /// # Errors
    /// [`AppError`] with the core preview builder's message verbatim.
    pub async fn preview(&self, req: LaunchRequest) -> Result<LaunchPreview, AppError> {
        let mut req = req;
        enrich_umu_gameid(&self.settings, &self.metadata, &mut req).await;
        let mut preview = build_launch_preview(&req).map_err(|error| {
            AppError::new(crate::AppErrorKind::Validation { field: "launch" }, error)
        })?;
        let mut coexistence =
            collect_mod_coexistence_warnings(&req, &self.profiles, &self.metadata).await;
        preview.validation.issues.append(&mut coexistence);
        Ok(preview)
    }

    /// Builds a Steam per-game "Launch Options" line from the optimization IDs
    /// plus profile custom env vars (custom wins on duplicate keys).
    ///
    /// When `gamescope` is enabled the compositor is inserted as a wrapper
    /// before `%command%`; resolved command-argument tokens are appended after
    /// it when `enabled_argument_ids` or `custom_command_args` are non-empty.
    ///
    /// # Errors
    /// [`AppError`] when an option or argument ID fails to resolve.
    pub fn steam_launch_options_command(
        &self,
        enabled_option_ids: &[String],
        custom_env_vars: &BTreeMap<String, String>,
        gamescope: Option<&GamescopeConfig>,
        enabled_argument_ids: &[String],
        custom_command_args: &[String],
    ) -> Result<String, AppError> {
        let mut command = launch::build_steam_launch_options_command(
            enabled_option_ids,
            custom_env_vars,
            gamescope,
        )
        .map_err(|error| {
            AppError::new(
                crate::AppErrorKind::Validation { field: "launch" },
                error.to_string(),
            )
        })?;

        if enabled_argument_ids.is_empty() && custom_command_args.is_empty() {
            return Ok(command);
        }

        // `CommandArgumentResolveError` has no `Display` impl (flagged
        // follow-up); the `{:?}` text is today's wire behaviour.
        let resolved = resolve_command_arguments_for_method(
            enabled_argument_ids,
            custom_command_args,
            METHOD_STEAM_APPLAUNCH,
        )
        .map_err(|error| {
            AppError::new(
                crate::AppErrorKind::Validation {
                    field: "command_arguments",
                },
                format!("{error:?}"),
            )
        })?;

        for token in resolved.tokens {
            command.push(' ');
            command.push_str(&escape_steam_token(&token));
        }

        Ok(command)
    }

    /// Recent launch rows for a profile (newest first), without diagnostic
    /// JSON. An unavailable metadata store yields an empty list.
    ///
    /// # Errors
    /// [`AppError`] validation when the trimmed `profile_name` is empty, or a
    /// store error mapped to its sanitized kind.
    pub async fn launch_history(
        &self,
        profile_name: String,
        limit: Option<u32>,
    ) -> Result<Vec<LaunchHistoryEntry>, AppError> {
        let profile_name = profile_name.trim().to_string();
        if profile_name.is_empty() {
            return Err(AppError::validation(
                "profile_name",
                "profile_name is required",
            ));
        }
        if !self.metadata.is_available() {
            return Ok(Vec::new());
        }
        let cap = limit
            .unwrap_or(DEFAULT_HISTORY_LIST_LIMIT)
            .min(MAX_HISTORY_LIST_LIMIT as u32) as usize;
        let metadata = self.metadata.clone();
        run_blocking(move || Ok(metadata.query_launch_history_for_profile(&profile_name, cap)?))
            .await
    }

    /// User-initiated launch reset: cancels every active session (game and
    /// trainer) registered for the profile. Returns the number of sessions
    /// signalled — 0 when nothing was registered.
    pub fn reset_sessions(&self, profile_name: &str) -> usize {
        let profile_key = session_profile_key_for_name(Some(profile_name));
        self.registry
            .cancel_sessions_for_profile(&profile_key, TeardownReason::UserRequest)
    }

    /// Profile keys with an active game session, in registration order.
    #[must_use]
    pub fn running_profiles(&self) -> Vec<String> {
        self.registry.active_profile_keys(Some(SessionKind::Game))
    }
}
