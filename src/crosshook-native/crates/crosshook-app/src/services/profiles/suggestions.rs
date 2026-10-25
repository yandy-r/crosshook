//! ProtonDB suggestion accept facade.
//!
//! Owned by YAN-811 Task 1.5. Both branches re-validate the suggestion at
//! write time (never trusting cached suggestion data), persist once, observe
//! the write, capture a `ProtonDbSuggestionApply` revision and publish `Saved`.

use crosshook_core::launch::catalog::global_catalog;
use crosshook_core::metadata::{ConfigRevisionSource, SyncSource};
use crosshook_core::profile::GameProfile;
use crosshook_core::protondb::{
    validate_env_suggestion, AcceptSuggestionRequest, AcceptSuggestionResult,
};

use super::types::{Effect, Persisted, ProfileCommit, ProfileMutation};
use super::writer::CommitContext;
use super::ProfileWriter;
use crate::{AppError, ProfileChangeReason};

impl ProfileWriter {
    /// Applies an accepted ProtonDB suggestion (catalog optimization or raw
    /// env var) to the named profile.
    ///
    /// # Errors
    /// `Validation` for blank names, an unknown catalog entry or an env pair
    /// that fails `validate_env_suggestion` (nothing is written); otherwise
    /// the profile load/save error.
    pub async fn accept_protondb_suggestion(
        &self,
        request: AcceptSuggestionRequest,
    ) -> Result<ProfileCommit<AcceptSuggestionResult>, AppError> {
        self.commit(AcceptSuggestion(request), ProfileChangeReason::Saved)
            .await
    }
}

/// Commit mutation for [`ProfileWriter::accept_protondb_suggestion`].
struct AcceptSuggestion(AcceptSuggestionRequest);

impl ProfileMutation for AcceptSuggestion {
    type Output = AcceptSuggestionResult;

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::Saved)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<Self::Output>, AppError> {
        match self.0 {
            AcceptSuggestionRequest::Catalog {
                profile_name,
                catalog_entry_id,
            } => accept_catalog(ctx, &profile_name, &catalog_entry_id),
            AcceptSuggestionRequest::EnvVar {
                profile_name,
                env_key,
                env_value,
            } => accept_env_var(ctx, &profile_name, env_key, env_value),
        }
    }
}

fn accept_catalog(
    ctx: &CommitContext<'_>,
    profile_name: &str,
    catalog_entry_id: &str,
) -> Result<Persisted<AcceptSuggestionResult>, AppError> {
    let profile_name = required_profile_name(profile_name)?;
    let entry_id = catalog_entry_id.trim().to_string();
    if entry_id.is_empty() {
        return Err(AppError::validation(
            "catalog_entry_id",
            "catalog_entry_id is required",
        ));
    }

    let entry = global_catalog().find_by_id(&entry_id).ok_or_else(|| {
        AppError::validation(
            "catalog_entry_id",
            format!("unknown catalog entry: {entry_id}"),
        )
    })?;

    // Validate every env pair of the catalog entry before touching the profile.
    for [key, value] in &entry.env {
        if !validate_env_suggestion(key, value) {
            return Err(AppError::validation(
                "catalog_entry_id",
                format!("catalog entry '{entry_id}' contains unsafe env pair: {key}={value}"),
            ));
        }
    }

    let mut profile = ctx.profiles.load(&profile_name)?;
    let enabled = &mut profile.launch.optimizations.enabled_option_ids;
    let already_present = enabled.contains(&entry_id);
    if !already_present {
        enabled.push(entry_id.clone());
    }
    ctx.profiles.save(&profile_name, &profile)?;

    let output = AcceptSuggestionResult {
        updated_profile: profile.clone(),
        applied_keys: entry.env.iter().map(|[key, _]| key.clone()).collect(),
        toggled_option_ids: if already_present {
            Vec::new()
        } else {
            vec![entry_id]
        },
    };
    Ok(applied(profile_name, profile, output))
}

fn accept_env_var(
    ctx: &CommitContext<'_>,
    profile_name: &str,
    env_key: String,
    env_value: String,
) -> Result<Persisted<AcceptSuggestionResult>, AppError> {
    let profile_name = required_profile_name(profile_name)?;

    // Re-validate at write time — never trust cached suggestion data.
    if !validate_env_suggestion(&env_key, &env_value) {
        return Err(AppError::validation(
            "env_key",
            format!("env var suggestion failed safety validation: {env_key}={env_value}"),
        ));
    }

    let mut profile = ctx.profiles.load(&profile_name)?;
    profile
        .launch
        .custom_env_vars
        .insert(env_key.clone(), env_value);
    ctx.profiles.save(&profile_name, &profile)?;

    let output = AcceptSuggestionResult {
        updated_profile: profile.clone(),
        applied_keys: vec![env_key],
        toggled_option_ids: Vec::new(),
    };
    Ok(applied(profile_name, profile, output))
}

fn required_profile_name(profile_name: &str) -> Result<String, AppError> {
    let profile_name = profile_name.trim();
    if profile_name.is_empty() {
        return Err(AppError::validation(
            "profile_name",
            "profile_name is required",
        ));
    }
    Ok(profile_name.to_string())
}

/// Effects and event shared by both accept branches.
fn applied(
    profile_name: String,
    profile: GameProfile,
    output: AcceptSuggestionResult,
) -> Persisted<AcceptSuggestionResult> {
    Persisted {
        output,
        effects: vec![
            Effect::ObserveWrite {
                name: profile_name.clone(),
                profile: profile.clone(),
                source: SyncSource::AppWrite,
                source_profile_id: None,
            },
            Effect::Revision {
                name: profile_name.clone(),
                profile,
                source: ConfigRevisionSource::ProtonDbSuggestionApply,
                source_revision_id: None,
            },
        ],
        names: Some(vec![profile_name]),
    }
}
