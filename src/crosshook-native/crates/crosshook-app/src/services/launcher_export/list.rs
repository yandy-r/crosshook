//! Launcher listing, orphan detection and re-export by slug.

use std::collections::HashMap;

use crosshook_core::export::{
    check_launcher_exists_for_request, check_launcher_for_profile, export_launchers,
    find_orphaned_launchers, list_launchers, validate, LauncherInfo,
    SteamExternalLauncherExportRequest, SteamExternalLauncherExportResult,
};
use crosshook_core::profile::{resolve_launch_method, GameProfile, ProfileStore};
use crosshook_core::settings::{SettingsStore, UmuPreference};

use super::export::{export_error, validation_error};
use super::LauncherExportService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

impl LauncherExportService {
    /// Lists launcher files under the resolved launcher directory, flagging
    /// launchers whose owning profile(s) are out of date. A settings failure
    /// falls back to `Auto`, unloadable profiles are skipped, and a slug owned
    /// by several profiles is stale when any of them is stale.
    pub async fn list(
        &self,
        target_home_path: String,
        steam_client_install_path: String,
    ) -> Vec<LauncherInfo> {
        let profiles = self.profiles.clone();
        let settings = self.settings.clone();
        let outcome = run_blocking(move || {
            let mut launchers = list_launchers(&target_home_path, &steam_client_install_path);
            let stale_by_slug = stale_flags_by_profile_slug(
                &profiles,
                &settings,
                &target_home_path,
                &steam_client_install_path,
            );
            apply_stale_flags(&mut launchers, &stale_by_slug);
            Ok(launchers)
        })
        .await;
        outcome.unwrap_or_else(|error| {
            tracing::warn!(%error, "launcher listing task failed");
            Vec::new()
        })
    }

    /// Lists launchers whose slugs are not in `known_profile_slugs`.
    pub async fn find_orphaned(
        &self,
        known_profile_slugs: Vec<String>,
        target_home_path: String,
        steam_client_install_path: String,
    ) -> Vec<LauncherInfo> {
        let outcome = run_blocking(move || {
            Ok(find_orphaned_launchers(
                &known_profile_slugs,
                &target_home_path,
                &steam_client_install_path,
            ))
        })
        .await;
        outcome.unwrap_or_else(|error| {
            tracing::warn!(%error, "orphaned launcher scan task failed");
            Vec::new()
        })
    }

    /// Re-exports a launcher using the profile that currently owns its slug.
    /// The final settings reload is a hard error (preserved inconsistency with
    /// the soft fallback used while matching the slug).
    pub async fn reexport_by_slug(
        &self,
        launcher_slug: String,
        target_home_path: String,
        steam_client_install_path: String,
    ) -> Result<SteamExternalLauncherExportResult, AppError> {
        let profiles = self.profiles.clone();
        let settings = self.settings.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile_name = find_profile_name_for_launcher_slug(
                &profiles,
                &settings,
                &launcher_slug,
                &target_home_path,
                &steam_client_install_path,
            )?
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("No profile found for launcher slug '{launcher_slug}'"),
                )
            })?;

            let profile = profiles.load(&profile_name)?;
            let request = build_export_request_for_profile(
                &profile,
                Some(&profile_name),
                &target_home_path,
                &steam_client_install_path,
                settings.load()?.umu_preference,
            )?;
            validate(&request).map_err(validation_error)?;
            let result = export_launchers(&request).map_err(export_error)?;

            if let Err(error) = metadata.observe_launcher_exported(
                request.profile_name.as_deref(),
                &result.launcher_slug,
                &result.display_name,
                &result.script_path,
                &result.desktop_entry_path,
            ) {
                tracing::warn!(
                    %error,
                    launcher_slug = %result.launcher_slug,
                    "metadata sync after reexport_launcher_by_slug failed"
                );
            }
            Ok(result)
        })
        .await
    }
}

fn build_export_request_for_profile(
    profile: &GameProfile,
    profile_name: Option<&str>,
    target_home_path: &str,
    steam_client_install_path: &str,
    global_umu_preference: UmuPreference,
) -> Result<SteamExternalLauncherExportRequest, AppError> {
    let method = resolve_launch_method(profile).to_string();
    if method == "native" {
        return Err(AppError::new(
            AppErrorKind::Validation { field: "method" },
            "Profile launch method does not support launcher export",
        ));
    }

    let (prefix_path, proton_path) = if method == "steam_applaunch" {
        (
            profile.steam.compatdata_path.clone(),
            profile.steam.proton_path.clone(),
        )
    } else {
        (
            profile.runtime.prefix_path.clone(),
            profile.runtime.proton_path.clone(),
        )
    };

    Ok(SteamExternalLauncherExportRequest {
        method,
        launcher_name: profile.steam.launcher.display_name.clone(),
        trainer_path: profile.trainer.path.clone(),
        trainer_loading_mode: profile.trainer.loading_mode,
        launcher_icon_path: profile.steam.launcher.icon_path.clone(),
        prefix_path,
        proton_path,
        steam_app_id: profile.steam.app_id.clone(),
        steam_client_install_path: steam_client_install_path.to_string(),
        target_home_path: target_home_path.to_string(),
        profile_name: profile_name.map(std::string::ToString::to_string),
        runtime_steam_app_id: profile.runtime.steam_app_id.clone(),
        umu_game_id: profile.runtime.umu_game_id.clone(),
        umu_preference: profile
            .runtime
            .umu_preference
            .unwrap_or(global_umu_preference),
        network_isolation: profile.launch.network_isolation,
        gamescope: profile.launch.resolved_trainer_gamescope(),
        custom_env_vars: profile.launch.custom_env_vars.clone(),
    })
}

fn apply_stale_flags(launchers: &mut [LauncherInfo], stale_by_slug: &HashMap<String, bool>) {
    for launcher in launchers {
        launcher.is_stale = stale_by_slug
            .get(&launcher.launcher_slug)
            .copied()
            .unwrap_or(false);
    }
}

fn stale_flags_by_profile_slug(
    store: &ProfileStore,
    settings_store: &SettingsStore,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> HashMap<String, bool> {
    let global_umu_preference = settings_store
        .load()
        .map(|settings| settings.umu_preference)
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "failed to load settings for launcher stale detection");
            UmuPreference::Auto
        });

    let profile_names = match store.list() {
        Ok(names) => names,
        Err(error) => {
            tracing::warn!(%error, "failed to list profiles for launcher stale detection");
            return HashMap::new();
        }
    };

    let mut stale_by_slug: HashMap<String, bool> = HashMap::new();
    for profile_name in profile_names {
        let profile = match store.load(&profile_name) {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(%error, profile_name, "failed to load profile while computing launcher staleness");
                continue;
            }
        };

        if resolve_launch_method(&profile) == "native" {
            continue;
        }

        let launcher_info = match check_launcher_for_profile(
            &profile,
            target_home_path,
            steam_client_install_path,
            global_umu_preference,
        ) {
            Ok(info) => info,
            Err(error) => {
                tracing::warn!(
                    %error,
                    profile_name,
                    "failed to compute launcher stale status for profile"
                );
                continue;
            }
        };

        if launcher_info.launcher_slug.is_empty() {
            continue;
        }
        stale_by_slug
            .entry(launcher_info.launcher_slug)
            .and_modify(|is_stale| *is_stale = *is_stale || launcher_info.is_stale)
            .or_insert(launcher_info.is_stale);
    }

    stale_by_slug
}

fn find_profile_name_for_launcher_slug(
    store: &ProfileStore,
    settings_store: &SettingsStore,
    launcher_slug: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<Option<String>, AppError> {
    let global_umu_preference = settings_store
        .load()
        .map(|settings| settings.umu_preference)
        .unwrap_or(UmuPreference::Auto);
    let profile_names = store.list()?;
    for profile_name in profile_names {
        let profile = match store.load(&profile_name) {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(%error, profile_name, "failed to load profile while matching launcher slug");
                continue;
            }
        };

        let Ok(request) = build_export_request_for_profile(
            &profile,
            Some(&profile_name),
            target_home_path,
            steam_client_install_path,
            global_umu_preference,
        ) else {
            continue;
        };
        let launcher_info = match check_launcher_exists_for_request(
            &request.launcher_name,
            &request,
        ) {
            Ok(info) => info,
            Err(error) => {
                tracing::warn!(%error, profile_name, "failed to compute launcher slug while matching profile");
                continue;
            }
        };

        if launcher_info.launcher_slug == launcher_slug {
            return Ok(Some(profile_name));
        }
    }

    Ok(None)
}
