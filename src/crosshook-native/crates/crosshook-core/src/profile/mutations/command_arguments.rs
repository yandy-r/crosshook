//! Launch command-argument mutation rule and its validation.

use crate::launch::command_arguments::{
    resolve_command_arguments_for_method, CommandArgumentResolveError,
};
use crate::launch::request::{
    ValidationError, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crate::profile::{resolve_launch_method, GameProfile, ProfileStoreError};

const MAX_COMMAND_ARGUMENT_TOKEN_LEN: usize = 512;
const MAX_COMMAND_ARGUMENT_TOKENS: usize = 64;

impl GameProfile {
    /// Replaces `launch.command_arguments` in memory.
    ///
    /// Trims ids and custom tokens and drops blanks, then validates them against the launch
    /// method (`resolved_launch_method` when it names a known method, otherwise the profile's
    /// resolved method), the curated catalog, and the 64-token / 512-byte limits.
    pub fn apply_command_arguments(
        &mut self,
        enabled_argument_ids: Vec<String>,
        custom_args: Vec<String>,
        resolved_launch_method: Option<&str>,
    ) -> Result<(), ProfileStoreError> {
        let enabled_argument_ids: Vec<String> = enabled_argument_ids
            .into_iter()
            .filter_map(|raw| {
                let id = raw.trim();
                (!id.is_empty()).then(|| id.to_string())
            })
            .collect();

        let custom_args: Vec<String> = custom_args
            .into_iter()
            .map(|raw| raw.trim().to_string())
            .filter(|token| !token.is_empty())
            .collect();

        validate_profile_command_arguments(
            self,
            &enabled_argument_ids,
            &custom_args,
            resolved_launch_method,
        )?;

        self.launch.command_arguments.enabled_argument_ids = enabled_argument_ids;
        self.launch.command_arguments.custom_args = custom_args;
        Ok(())
    }
}

fn command_argument_token_has_control_character(token: &str) -> bool {
    token.contains('\0') || token.chars().any(char::is_control)
}

fn validate_custom_command_argument_token(token: &str) -> Result<(), ValidationError> {
    if token.is_empty() {
        return Err(ValidationError::CommandArgumentCustomTokenEmpty);
    }
    if command_argument_token_has_control_character(token) {
        return Err(ValidationError::CommandArgumentCustomTokenContainsControlCharacter);
    }
    if token.len() > MAX_COMMAND_ARGUMENT_TOKEN_LEN {
        return Err(ValidationError::CommandArgumentTokenTooLong {
            max_len: MAX_COMMAND_ARGUMENT_TOKEN_LEN,
        });
    }
    Ok(())
}

fn command_argument_resolve_error(error: CommandArgumentResolveError) -> ProfileStoreError {
    match error {
        CommandArgumentResolveError::Unknown(argument_id) => {
            ProfileStoreError::InvalidCommandArgumentId(argument_id)
        }
        CommandArgumentResolveError::Duplicate(argument_id) => {
            ProfileStoreError::CommandArgumentValidation(ValidationError::DuplicateCommandArgument(
                argument_id,
            ))
        }
        CommandArgumentResolveError::NotSupportedForMethod {
            argument_id,
            method,
        } => ProfileStoreError::CommandArgumentValidation(
            ValidationError::CommandArgumentNotSupportedForMethod {
                argument_id,
                method,
            },
        ),
        CommandArgumentResolveError::Incompatible { first, second } => {
            ProfileStoreError::CommandArgumentValidation(
                ValidationError::IncompatibleCommandArguments { first, second },
            )
        }
        CommandArgumentResolveError::UnsupportedLaunchMethod(method) => {
            ProfileStoreError::CommandArgumentValidation(
                ValidationError::CommandArgumentsUnsupportedForMethod(method),
            )
        }
    }
}

fn validate_resolved_command_argument_tokens(tokens: &[String]) -> Result<(), ValidationError> {
    if tokens.len() > MAX_COMMAND_ARGUMENT_TOKENS {
        return Err(ValidationError::CommandArgumentTokenCountExceeded {
            max_count: MAX_COMMAND_ARGUMENT_TOKENS,
        });
    }

    for token in tokens {
        if token.len() > MAX_COMMAND_ARGUMENT_TOKEN_LEN {
            return Err(ValidationError::CommandArgumentTokenTooLong {
                max_len: MAX_COMMAND_ARGUMENT_TOKEN_LEN,
            });
        }
    }

    Ok(())
}

fn resolved_launch_method_for_command_argument_validation(
    profile: &GameProfile,
    resolved_launch_method: Option<&str>,
) -> String {
    if let Some(raw) = resolved_launch_method {
        let trimmed = raw.trim();
        if matches!(
            trimmed,
            METHOD_NATIVE | METHOD_PROTON_RUN | METHOD_STEAM_APPLAUNCH
        ) {
            return trimmed.to_string();
        }
    }

    resolve_launch_method(profile).to_string()
}

fn validate_profile_command_arguments(
    profile: &GameProfile,
    enabled_argument_ids: &[String],
    custom_args: &[String],
    resolved_launch_method: Option<&str>,
) -> Result<(), ProfileStoreError> {
    if enabled_argument_ids.is_empty() && custom_args.is_empty() {
        return Ok(());
    }

    let resolved_method =
        resolved_launch_method_for_command_argument_validation(profile, resolved_launch_method);
    if resolved_method == METHOD_NATIVE {
        return Err(ProfileStoreError::CommandArgumentValidation(
            ValidationError::CommandArgumentsUnsupportedForMethod(resolved_method.to_string()),
        ));
    }

    for custom_arg in custom_args {
        validate_custom_command_argument_token(custom_arg)
            .map_err(ProfileStoreError::CommandArgumentValidation)?;
    }

    let resolved =
        resolve_command_arguments_for_method(enabled_argument_ids, custom_args, &resolved_method)
            .map_err(command_argument_resolve_error)?;

    validate_resolved_command_argument_tokens(&resolved.tokens)
        .map_err(ProfileStoreError::CommandArgumentValidation)?;

    Ok(())
}
